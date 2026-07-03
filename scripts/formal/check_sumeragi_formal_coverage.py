#!/usr/bin/env python3
"""Check that Sumeragi formal modes stay wired across runner, CI, and docs."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from functools import cache
from pathlib import Path
from typing import Callable


ROOT_DIR = Path(__file__).resolve().parents[2]
SPEC_DIR = ROOT_DIR / "docs" / "formal" / "sumeragi"
APALACHE_RUNNER = ROOT_DIR / "scripts" / "formal" / "sumeragi_apalache.sh"
TLC_RUNNER = ROOT_DIR / "scripts" / "formal" / "sumeragi_tlc.sh"
SUMERAGI_FAST_CFG = SPEC_DIR / "Sumeragi_fast.cfg"
SUMERAGI_DEEP_CFG = SPEC_DIR / "Sumeragi_deep.cfg"
SUMERAGI_TLC_FAST_CFG = SPEC_DIR / "Sumeragi_tlc_fast.cfg"
SUMERAGI_ROOT_PROPERTY = "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope"
SUMERAGI_STATE_INVARIANT_ROOT = "SumeragiConsensusCoreStateMatchesEnvelope"
SUMERAGI_STATE_MATCHES_ENVELOPE_CONJUNCT_CONTRACTS = {
    "SumeragiConsensusCoreStateMatchesEnvelope": (
        "CommitImpliesQuorum",
        "CommitImpliesStakeQuorum",
        "CommitCertificateMatchesFinality",
        "LiveCommitGateMatchesFinality",
        "LiveCommitGateRbcEvidenceMatches",
        "CommitImpliesLiveVoteQuorum",
        "CommitImpliesLiveStakeQuorum",
        "CommitImpliesHonestSupport",
        "CommitImpliesDelivered",
        "CommitImpliesRbcEvidence",
        "FinalityCertificateStackComplete",
        "FinalityCertificateStackMatchesFinality",
        "FinalityClearsNewViewHandoff",
        "CommitDisablesProgressActions",
        "CommitDisablesByzantineCommitVote",
        "CommittedPhaseMatchesFinality",
        "CommitViewMatchesFinality",
        "CommitViewDoesNotLeadCurrentView",
        "GstElapsedGateMatchesPreGst",
        "CommittedPreGstOnlyEnablesGstElapsed",
        "TimeoutTickGateMatchesStalledProgress",
        "ByzantineCommitVoteDoesNotBlockTimeoutStall",
        "ViewEvidenceMatchesActiveView",
        "ViewEvidenceWitnessRequiresNonzeroActiveView",
        "NewViewPhaseBelowQuorum",
        "LiveNewViewVotesStayInHandoff",
        "HonestProposeGateMatchesHandoffEvidence",
        "NewViewVoteGateMatchesFreshViewEvidence",
        "NewViewVoteQuorumGateMatchesNextEvidence",
        "NewViewVotePendingGateMatchesMissingNextEvidence",
        "ViewEvidenceIsCompleteOrEmpty",
        "PreCommitPhasesHaveNoCommitVotes",
        "PrePreparePhasesHaveNoPrepareVotes",
        "LivePrepareVotesStayInHandoff",
        "PrepareVoteGateMatchesProposalEvidence",
        "PrepareVoteQuorumGateMatchesNextEvidence",
        "PrepareVotePendingGateMatchesMissingNextEvidence",
        "CommitImpliesViewQuorumEvidence",
        "CommitVotePhaseRequiresPrepareQuorum",
        "LiveCommitVotesRequirePrepareQuorum",
        "CommitVoteGateMatchesPrepareEvidence",
        "ByzantineCommitVoteGateMatchesPrepareEvidence",
        "HonestCommitVoteFinalityGateMatchesNextEvidence",
        "HonestCommitVotePendingGateMatchesMissingNextEvidence",
        "ByzantineCommitVoteFinalityGateMatchesNextEvidence",
        "ByzantineCommitVotePendingGateMatchesMissingNextEvidence",
        "LiveCommitVotesStayInCommitHandoff",
        "CommitImpliesPrepareQuorum",
        "CommitEvidenceMatchesVoteCounters",
        "CommitEvidenceIsCompleteOrEmpty",
        "CommitEvidenceIsBounded",
        "VoteCountersRespectRosterBudgets",
        "StakeSignedMatchesVoteCounters",
        "LiveStakeSignedIsBounded",
        "NoCommitEvidenceBeforeCommit",
        "NoCommitViewBeforeCommit",
        "DeliverImpliesEvidence",
        "RbcDeliveredWithoutFinalityHasNoCommitCertificate",
        "RbcDeliveredWithoutFinalityWaitsForCommitEvidence",
        "RbcProgressEvidenceMatchesState",
        "RbcPartialProgressEvidenceMatchesState",
        "RbcCorruptedNeverHasValidDigest",
        "RbcCorruptedRetainsHeaderEvidence",
        "RbcCorruptedHasNoFinalityArtifacts",
        "RbcCorruptedOnlyEnablesInitRepairProgress",
        "RbcMissingHeaderRequiresIdle",
        "RbcHeaderEvidenceRequiresNonIdle",
        "RbcValidDigestRequiresHeader",
        "RbcValidDigestRequiresActiveState",
        "RbcChunkEvidenceRequiresHeader",
        "RbcChunkEvidenceRequiresChunkOrCorruptedState",
        "RbcPartialChunkEvidenceRequiresChunkingOrCorruption",
        "RbcFullChunkCoverageRequiresCoveredOrCorruptedState",
        "RbcZeroChunkEvidenceRequiresPreChunkOrCorruption",
        "RbcReadyVotesRequireChunkHeaderEvidence",
        "RbcReadyVotesRequireReadyOrCorruptedState",
        "RbcPartialReadyEvidenceRequiresReadyPartialOrCorruption",
        "RbcReadyQuorumEvidenceRequiresQuorumOrCorruptedState",
        "RbcZeroReadyEvidenceRequiresPreReadyOrCorruption",
        "RbcCounterEvidenceRequiresValidDigestOrCorruption",
        "RbcInvalidDigestRequiresIdleOrCorruption",
        "ByzantineFaultGateMatchesCorruptibleRbc",
        "RbcInitGateMatchesRepairableState",
        "RbcChunkGateMatchesHeaderDigestEvidence",
        "RbcReadyGateMatchesChunkEvidence",
        "RbcDeliverGateMatchesCompleteEvidence",
        "RbcReadyQuorumEnablesDeliverGate",
        "RbcDeliverFinalityGateMatchesBufferedCommitEvidence",
        "RbcDeliverPendingGateMatchesMissingBufferedCommitEvidence",
        "LiveHeaderDigestEvidenceStayInRbcHandoff",
        "LiveChunkEvidenceStayInRbcHandoff",
        "LiveReadyVotesStayInRbcHandoff",
    ),
}
SUMERAGI_TEMPORAL_PROPERTY_ROOTS_REQUIRING_CFG_COVERAGE = (
    "SumeragiConsensusCoreAlwaysMatchesEndToEndSafetyEnvelope",
    "RbcLifecycleAlwaysMatchesEndToEndEnvelope",
    "RbcProgressMutationAlwaysPreservesLiveEvidenceEnvelope",
    "RbcCorruptionRepairAlwaysMatchesFaultEnvelope",
    "RbcChunkReadyDeliverAlwaysMatchesAvailabilityEnvelope",
    "RbcDeliveryEntryAlwaysMatchesCompleteOutcomeEnvelope",
    "RbcDeliveredStateAlwaysMatchesCompleteLifecycleEnvelope",
)
SUMERAGI_CONSENSUS_CORE_ROOT_CONJUNCT_CONTRACTS = {
    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope": (
        "TypeInvariant",
        "SumeragiConsensusCoreAlwaysMatchesExactness",
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
        "EventuallyCommit",
    ),
    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope": (
        "SumeragiConsensusCoreAlwaysMatchesStateSafetyEnvelope",
        "SumeragiConsensusCoreAlwaysMatchesEndToEndSafetyEnvelope",
    ),
    "SumeragiConsensusCoreAlwaysMatchesExactness": (
        "SumeragiConsensusCoreStateMatchesEnvelope",
    ),
    "SumeragiConsensusCoreFastCorrectnessEnvelope": (
        "TypeInvariant",
        "SumeragiConsensusCoreAlwaysMatchesExactness",
    ),
}
SUMERAGI_CONSENSUS_CORE_ROOT_CFG_CHECK_CONTRACTS = {
    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope": {
        "TypeInvariant": "INVARIANT",
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope": "PROPERTY",
        "EventuallyCommit": "PROPERTY",
    },
}
SUMERAGI_END_TO_END_SAFETY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "SumeragiConsensusCoreAlwaysMatchesEndToEndSafetyEnvelope": (
        "CommittedStateAlwaysMatchesTerminalEnvelope",
        "PostFinalityStateAlwaysMatchesStabilityEnvelope",
        "TimeoutRecoveryAlwaysMatchesViewChangeEnvelope",
        "FinalityInstallationAlwaysMatchesCertifiedCommitEnvelope",
        "PreCommitHandoffAlwaysMatchesProposalPrepareEnvelope",
        "CommitVoteHandoffAlwaysMatchesFinalityEnvelope",
        "FinalizedCertificateEvidenceAlwaysMatchesRetentionEnvelope",
        "PendingProtocolStepsNeverChangeGst",
        "RbcLifecycleAlwaysMatchesEndToEndEnvelope",
    ),
}
SUMERAGI_TEMPORAL_ALWAYS_THEOREM_CONTRACTS = {
    "TimeoutTickGateNeverBypassesStalledProgress": "TimeoutTickGateMatchesStalledProgress",
    "ViewQuorumEvidenceNeverDiverges": "ViewEvidenceMatchesActiveView",
    "ViewEvidenceWitnessNeverTargetsZeroOrNewView": "ViewEvidenceWitnessRequiresNonzeroActiveView",
    "NewViewQuorumHandoffNeverStalls": "NewViewPhaseBelowQuorum",
    "LiveNewViewVotesNeverLeakPastHandoff": "LiveNewViewVotesStayInHandoff",
    "HonestProposeGateNeverBypassesHandoffEvidence": "HonestProposeGateMatchesHandoffEvidence",
    "NewViewVoteGateNeverBypassesFreshViewEvidence": "NewViewVoteGateMatchesFreshViewEvidence",
    "NewViewVoteQuorumGateNeverBypassesNextEvidence": "NewViewVoteQuorumGateMatchesNextEvidence",
    "NewViewVotePendingGateNeverBypassesMissingNextEvidence": "NewViewVotePendingGateMatchesMissingNextEvidence",
    "ViewEvidenceNeverPartial": "ViewEvidenceIsCompleteOrEmpty",
    "PreCommitVotesNeverCarryAcrossViews": "PreCommitPhasesHaveNoCommitVotes",
    "PrePrepareVotesNeverCarryAcrossViews": "PrePreparePhasesHaveNoPrepareVotes",
    "LivePrepareVotesNeverBypassPrepareHandoff": "LivePrepareVotesStayInHandoff",
    "PrepareVoteGateNeverBypassesProposalEvidence": "PrepareVoteGateMatchesProposalEvidence",
    "PrepareVoteQuorumGateNeverBypassesNextEvidence": "PrepareVoteQuorumGateMatchesNextEvidence",
    "PrepareVotePendingGateNeverBypassesMissingNextEvidence": "PrepareVotePendingGateMatchesMissingNextEvidence",
    "CommittedPhaseAlwaysMatchesFinality": "CommittedPhaseMatchesFinality",
    "CommitCertificateAlwaysMatchesFinality": "CommitCertificateMatchesFinality",
    "LiveCommitGateAlwaysMatchesFinality": "LiveCommitGateMatchesFinality",
    "LiveCommitGateRbcEvidenceAlwaysMatches": "LiveCommitGateRbcEvidenceMatches",
    "CommittedGstNeverEnablesActions": "CommittedGstDisablesEveryAction",
}
SUMERAGI_TEMPORAL_ACTION_THEOREM_CONTRACTS = {
    "PendingProtocolStepsNeverChangeGst": "PendingProtocolStepsPreserveGst",
    "TimeoutTickStepAlwaysStartsFreshNewView": "TimeoutTickStepStartsFreshNewView",
    "TimeoutTickStepNeverPreemptsProgress": "TimeoutTickStepNeverPreemptsProgressStep",
    "TimeoutTickStepAlwaysClearsCommitVoteGates": "TimeoutTickStepClearsCommitVoteGates",
    "TimeoutTickStepAlwaysStartsNewViewVoteHandoff": "TimeoutTickStepStartsNewViewVoteHandoff",
    "TimeoutTickStepAlwaysPreservesRbcEvidence": "TimeoutTickStepPreservesRbcEvidence",
    "ViewAdvanceOnlyComesFromTimeout": "ViewAdvanceOnlyComesFromTimeoutStep",
    "LiveProgressResetOnlyByTimeout": "LiveProgressResetOnlyByTimeoutStep",
    "ViewEvidenceOnlyChangesByQuorumOrTimeout": "ViewEvidenceChangesOnlyByQuorumOrTimeoutStep",
    "NewViewVotesOnlyChangeByVoteOrReset": "NewViewVotesChangeOnlyByVoteOrResetStep",
    "PrepareVotesOnlyChangeByVoteOrTimeout": "PrepareVotesChangeOnlyByVoteOrTimeoutStep",
    "CommitVoteCountersOnlyChangeByVoteOrTimeout": "CommitVoteCountersChangeOnlyByVoteOrTimeoutStep",
    "PhaseOnlyChangesByProtocol": "PhaseOnlyChangesByProtocolStep",
    "PreparePhaseEntryOnlyByProposal": "PreparePhaseEntryOnlyByProposalStep",
    "CommitVotePhaseEntryOnlyByPrepareQuorum": "CommitVotePhaseEntryOnlyByPrepareQuorumStep",
    "ProposePhaseEntryOnlyByNewViewQuorum": "ProposePhaseEntryOnlyByNewViewQuorumStep",
    "NewViewPhaseEntryOnlyByTimeout": "NewViewPhaseEntryOnlyByTimeoutStep",
    "HonestProposeStepAlwaysStartsPrepareAndRbc": "HonestProposeStepStartsPrepareAndRbc",
    "HonestProposeStepAlwaysStartsPrepareVoteHandoff": "HonestProposeStepStartsPrepareVoteHandoff",
    "NewViewVoteQuorumStepAlwaysInstallsViewEvidence": "NewViewVoteQuorumStepInstallsViewEvidence",
    "NewViewVoteQuorumStepAlwaysStartsProposalHandoff": "NewViewVoteQuorumStepStartsProposalHandoff",
    "NewViewVotePendingStepNeverInstallsViewEvidence": "NewViewVotePendingStepPreservesPreProposalArtifacts",
    "PrepareVoteQuorumStepAlwaysEntersCommitVote": "PrepareVoteQuorumStepEntersCommitVote",
    "PrepareVoteQuorumStepAlwaysStartsCommitVoteHandoff": "PrepareVoteQuorumStepStartsCommitVoteHandoff",
    "PrepareVotePendingStepNeverMutatesCommitArtifacts": "PrepareVotePendingStepPreservesPreCommitArtifacts",
    "PrepareVotePendingStepAlwaysKeepsPrepareVoteHandoff": "PrepareVotePendingStepKeepsPrepareVoteHandoff",
    "CommittedConsensusStateNeverChanges": "CommittedConsensusStateStableStep",
    "CommittedOnlyGstObservationCanChange": "CommittedOnlyGstObservationCanMoveStep",
    "CommittedPreGstOnlyGstElapsedCanMove": "CommittedPreGstOnlyGstElapsedCanMoveStep",
    "CommittedPreGstNextOnlyGstElapsed": "CommittedPreGstNextOnlyGstElapsedStep",
    "CommittedPreGstSpecStepStuttersOrObservesGst": "CommittedPreGstSpecStepStuttersOrObservesGstStep",
    "CommittedGstStateNeverChanges": "CommittedGstStateStableStep",
    "CommittedGstOnlyAllowsStuttering": "CommittedGstRejectsNextStep",
    "CommittedGstSpecStepOnlyStutters": "CommittedGstSpecStepOnlyStuttersStep",
    "CommittedSpecNonStutteringOnlyObservesGst": "CommittedSpecNonStutteringOnlyObservesGstStep",
    "CommittedSpecStepStuttersOrObservesGst": "CommittedSpecStepStuttersOrObservesGstStep",
    "CommittedSpecStepPreservesFinalityStack": "CommittedSpecStepPreservesFinalityStackStep",
    "CommittedSpecStepOnlyChangesGstFlag": "CommittedSpecStepOnlyChangesGstFlagStep",
    "CommittedSpecStepNeverRunsProtocolActions": "CommittedSpecStepNeverRunsProtocolActionsStep",
    "CommittedSpecStepKeepsProgressActionsQuiescent": "CommittedSpecStepKeepsProgressActionsQuiescentStep",
    "CommittedSpecStepPreservesBudgetedRbcEvidence": "CommittedSpecStepPreservesBudgetedRbcEvidenceStep",
    "RbcDeliveredFinalityOnlyComesFromCommitVote": "RbcDeliveredFinalityOnlyByCommitVoteStep",
    "RbcDeliveredFinalityAlwaysCompletesCommittedDelivery": "RbcDeliveredFinalityStepCompletesCommittedDelivery",
    "RbcDeliveredFinalityAlwaysCommitsCurrentView": "RbcDeliveredFinalityCommitsCurrentViewStep",
    "RbcDeliveredFinalityOnlyLeavesGstElapsedGate": "RbcDeliveredFinalityLeavesOnlyGstElapsedGateStep",
    "RbcDeliveredFinalityAlwaysInstallsCommitCertificateWitnesses": "RbcDeliveredFinalityInstallsCommitCertificateWitnessesStep",
    "RbcDeliveredFinalityAlwaysMatchesCommitCertificateWitnessChange": "RbcDeliveredFinalityMatchesCommitCertificateWitnessChangeStep",
    "RbcDeliveredFinalityAlwaysMatchesCommitViewWitnessChange": "RbcDeliveredFinalityMatchesCommitViewWitnessChangeStep",
    "RbcDeliveredFinalityAlwaysMatchesLiveCommitGateCrossing": "RbcDeliveredFinalityMatchesLiveCommitGateCrossingStep",
    "RbcDeliveredFinalityAlwaysDisablesProgressAfterCommittedDelivery": "RbcDeliveredFinalityDisablesProgressAfterCommittedDeliveryStep",
    "RbcDeliveredFinalityAlwaysMatchesCertifiedSourceStack": "RbcDeliveredFinalityMatchesCertifiedSourceStackStep",
    "RbcDeliveredFinalityAlwaysInstallsFinalityCertificateStack": "RbcDeliveredFinalityInstallsFinalityCertificateStackStep",
    "RbcDeliveredFinalityAlwaysMatchesCommittedPhaseEntry": "RbcDeliveredFinalityMatchesCommittedPhaseEntryStep",
    "RbcDeliveredFinalityAlwaysMatchesCommitArtifactsChange": "RbcDeliveredFinalityMatchesCommitArtifactsChangeStep",
    "RbcDeliveredFinalityAlwaysCouplesLatchAndCommitArtifacts": "RbcDeliveredFinalityCouplesLatchAndCommitArtifactsStep",
    "RbcDeliveredFinalityAlwaysRecordsExactCommitVoteWitnesses": "RbcDeliveredFinalityRecordsExactCommitVoteWitnessesStep",
    "RbcDeliveredFinalityAlwaysPreservesDeliveredRbcEvidence": "RbcDeliveredFinalityPreservesDeliveredRbcEvidenceStep",
    "RbcDeliveredFinalityAlwaysPreservesViewPrepareHandoffEvidence": "RbcDeliveredFinalityPreservesViewPrepareHandoffEvidenceStep",
    "RbcDeliveredFinalityAlwaysHasExactProtocolFrame": "RbcDeliveredFinalityHasExactProtocolFrameStep",
    "RbcDeliveredFinalityAlwaysHasExactCommitVoteActionFrame": "RbcDeliveredFinalityHasExactCommitVoteActionFrameStep",
    "RbcDeliveredFinalityAlwaysInstallsCommittedPostStateInvariants": "RbcDeliveredFinalityInstallsCommittedPostStateInvariantsStep",
    "RbcDeliveredFinalityAlwaysSplitsPostStateGate": "RbcDeliveredFinalityPostStateGateSplitStep",
    "RbcDeliveredFinalityPreGstPostStateOnlyLeavesGstElapsed": "RbcDeliveredFinalityPreGstPostStateLeavesOnlyGstElapsedStep",
    "RbcDeliveredFinalityPostGstPostStateIsTerminal": "RbcDeliveredFinalityPostGstPostStateIsTerminalStep",
    "RbcDeliveredEvidenceNeverRegresses": "RbcDeliveredEvidenceStableStep",
    "RbcDeliveredPendingHonestCommitVoteAlwaysKeepsWaitState": "RbcDeliveredPendingHonestCommitVoteStepKeepsWaitState",
    "RbcDeliveredPendingByzantineCommitVoteAlwaysKeepsWaitState": "RbcDeliveredPendingByzantineCommitVoteStepKeepsWaitState",
    "RbcDeliveredPendingHonestCommitVoteAlwaysCompletesFinality": "RbcDeliveredPendingHonestCommitVoteStepCompletesFinality",
    "RbcDeliveredPendingByzantineCommitVoteAlwaysCompletesFinality": "RbcDeliveredPendingByzantineCommitVoteStepCompletesFinality",
    "RbcDeliveredPendingPrepareVoteAlwaysKeepsWaitState": "RbcDeliveredPendingPrepareVoteStepKeepsWaitState",
    "RbcDeliveredPendingPrepareVoteAlwaysStartsCommitVoteWaitState": "RbcDeliveredPendingPrepareVoteStepStartsCommitVoteWaitState",
    "RbcDeliveredPendingTimeoutAlwaysStartsNewViewWaitState": "RbcDeliveredPendingTimeoutStepStartsNewViewWaitState",
    "RbcDeliveredPendingNewViewVoteAlwaysKeepsWaitState": "RbcDeliveredPendingNewViewVoteStepKeepsWaitState",
    "RbcDeliveredPendingNewViewVoteAlwaysStartsProposalWaitState": "RbcDeliveredPendingNewViewVoteStepStartsProposalWaitState",
    "RbcDeliveredPendingHonestProposeAlwaysStartsPrepareWaitState": "RbcDeliveredPendingHonestProposeStepStartsPrepareWaitState",
    "RbcDeliveredPendingGstElapsedAlwaysKeepsWaitState": "RbcDeliveredPendingGstElapsedStepKeepsWaitState",
    "RbcDeliveredPendingNextAlwaysCoveredByHandoffs": "RbcDeliveredPendingNextStepCoveredByHandoffs",
    "RbcDeliveredPendingSpecStepAlwaysStuttersOrTakesCoveredHandoff": "RbcDeliveredPendingSpecStepStuttersOrTakesCoveredHandoffStep",
    "RbcDeliveredPendingSpecStepAlwaysEndsInFinalityOrWaitState": "RbcDeliveredPendingSpecStepEndsInFinalityOrWaitStateStep",
    "RbcDeliveredPendingSpecStepAlwaysPreservesDeliveredRbcEvidence": "RbcDeliveredPendingSpecStepPreservesDeliveredRbcEvidenceStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesCommitArtifactsOutcome": "RbcDeliveredPendingSpecStepCommitArtifactsMatchOutcomeStep",
    "RbcDeliveredPendingSpecStepAlwaysChangesGstOnlyByElapsed": "RbcDeliveredPendingSpecStepGstChangesOnlyByElapsedStep",
    "RbcDeliveredPendingSpecStepAlwaysChangesViewOnlyByTimeout": "RbcDeliveredPendingSpecStepViewChangesOnlyByTimeoutStep",
    "RbcDeliveredPendingSpecStepAlwaysChangesViewEvidenceOnlyByNewViewOrTimeout": "RbcDeliveredPendingSpecStepViewEvidenceChangesOnlyByNewViewOrTimeoutStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesVoteCounterHandoff": "RbcDeliveredPendingSpecStepVoteCountersMatchHandoffStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesPostGateHandoff": "RbcDeliveredPendingSpecStepPostGatesMatchHandoffStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesTimerGateHandoff": "RbcDeliveredPendingSpecStepTimerGatesMatchHandoffStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesFinalitySource": "RbcDeliveredPendingSpecStepFinalitySourceMatchesCommitVoteStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityWitnessFrame": "RbcDeliveredPendingSpecStepFinalityWitnessFrameStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityStackOutcome": "RbcDeliveredPendingSpecStepFinalityStackMatchesOutcomeStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityGateOutcome": "RbcDeliveredPendingSpecStepFinalityGateOutcomeStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityQuorumOutcome": "RbcDeliveredPendingSpecStepFinalityQuorumOutcomeStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesNonFinalHandoffPhaseShape": "RbcDeliveredPendingSpecStepNonFinalHandoffPhaseShapeStep",
    "RbcDeliveredPendingSpecStepAlwaysClosesActionSurface": "RbcDeliveredPendingSpecStepActionSurfaceClosedStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesPhaseChangeAction": "RbcDeliveredPendingSpecStepPhaseChangeMatchesActionStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesCounterChangeAction": "RbcDeliveredPendingSpecStepCounterChangesMatchActionStep",
    "RbcDeliveredPendingSpecStepAlwaysHasExclusiveActionSource": "RbcDeliveredPendingSpecStepActionSourcesExclusiveStep",
    "RbcDeliveredPendingSpecStepAlwaysPreservesActionSurfaceOnStutter": "RbcDeliveredPendingSpecStepStutterPreservesActionSurfaceStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesCommitArtifactChangeSource": "RbcDeliveredPendingSpecStepCommitArtifactChangeMatchesSourceStep",
    "RbcDeliveredPendingSpecStepAlwaysInstallsCertifiedDeliveryOnCommitArtifactChange": "RbcDeliveredPendingSpecStepCommitArtifactChangeInstallsCertifiedDeliveryStep",
    "RbcDeliveredPendingSpecStepAlwaysInstallsExactSourceCertifiedDeliveryOnCommitArtifactChange": "RbcDeliveredPendingSpecStepCommitArtifactChangeExactSourceCertifiedDeliveryStep",
    "RbcDeliveredPendingSpecStepAlwaysKeepsNonFinalHandoffOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsStayNonFinalHandoffStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesNonFinalSourceOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsMatchNonFinalSourceStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesCounterFootprintOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsCounterFootprintStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesPhaseGateFootprintOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsPhaseGateFootprintStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesTimerFootprintOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsTimerFootprintStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesViewFootprintOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsViewFootprintStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityFootprintOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsFinalityFootprintStep",
    "RbcDeliveredPendingSpecStepAlwaysMatchesRbcSurfaceOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsRbcSurfaceStep",
    "RbcDeliveredPendingSpecStepAlwaysClosesCompleteWaitStateOnStableCommitArtifacts": "RbcDeliveredPendingSpecStepStableCommitArtifactsCompleteWaitStateStep",
    "RbcDeliveryEntryOnlyByDeliver": "RbcDeliveryEntryOnlyByDeliverStep",
    "RbcDeliveryEntryAlwaysMatchesReadyQuorumExitAndCommitBranch": "RbcDeliveryEntryMatchesReadyQuorumExitAndCommitBranchStep",
    "RbcDeliveryEntryFinalityAlwaysCompletesCommittedDelivery": "RbcDeliveryEntryFinalityCompletesCommittedDeliveryStep",
    "RbcDeliveryEntryPendingAlwaysInstallsCompleteWaitState": "RbcDeliveryEntryPendingInstallsCompleteWaitStateStep",
    "RbcDeliveryEntryAlwaysCompletesFinalityOrWaitState": "RbcDeliveryEntryCompletesFinalityOrWaitStateStep",
    "RbcDeliveryEntryAlwaysMatchesCommitArtifactOutcome": "RbcDeliveryEntryCommitArtifactsMatchOutcomeStep",
    "RbcDeliveryEntryAlwaysMatchesPostGateSurfaceOutcome": "RbcDeliveryEntryPostGateSurfaceMatchesOutcomeStep",
    "RbcDeliveryEntryAlwaysMatchesConsensusFrameOutcome": "RbcDeliveryEntryConsensusFrameMatchesOutcomeStep",
    "RbcDeliveryEntryFinalityAlwaysMatchesCertifiedSourceStack": "RbcDeliveryEntryFinalityMatchesCertifiedSourceStackStep",
    "RbcDeliveryEntryFinalityAlwaysInstallsCommittedPostStateInvariants": "RbcDeliveryEntryFinalityInstallsCommittedPostStateInvariantsStep",
    "RbcDeliveryEntryFinalityAlwaysSplitsPostStateGate": "RbcDeliveryEntryFinalityPostStateGateSplitStep",
    "RbcDeliveryEntryFinalityPreGstPostStateOnlyLeavesGstElapsed": "RbcDeliveryEntryFinalityPreGstPostStateLeavesOnlyGstElapsedStep",
    "RbcDeliveryEntryFinalityPostGstPostStateIsTerminal": "RbcDeliveryEntryFinalityPostGstPostStateIsTerminalStep",
    "RbcDeliveryEntryPendingAlwaysMatchesNonFinalWaitSurface": "RbcDeliveryEntryPendingMatchesNonFinalWaitSurfaceStep",
    "RbcDeliveryEntryPendingAlwaysSplitsPostStateTimerGate": "RbcDeliveryEntryPendingPostStateTimerGateSplitStep",
    "RbcDeliveryEntryPendingPreGstPostStateAlwaysKeepsWaitTimers": "RbcDeliveryEntryPendingPreGstPostStateKeepsWaitTimersStep",
    "RbcDeliveryEntryPendingPostGstPostStateAlwaysTracksProgressTimeout": "RbcDeliveryEntryPendingPostGstPostStateTimeoutTracksProgressStep",
    "RbcDeliveryEntryPendingAlwaysInstallsDeliveredWaitPredicate": "RbcDeliveryEntryPendingInstallsDeliveredWaitPredicateStep",
    "RbcDeliveryEntryPendingAlwaysOpensDeliveredPendingContinuationSurface": "RbcDeliveryEntryPendingOpensDeliveredPendingContinuationSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysOpensExactContinuation": "RbcDeliveryEntryCommitEvidenceBranchOpensExactContinuationStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExclusiveOutcome": "RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveOutcomeStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExclusiveGateOutcome": "RbcDeliveryEntryCommitEvidenceBranchMatchesExclusiveGateOutcomeStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactConsensusFrame": "RbcDeliveryEntryCommitEvidenceBranchMatchesExactConsensusFrameStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactActionSource": "RbcDeliveryEntryCommitEvidenceBranchMatchesExactActionSourceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCertifiedOrPendingStack": "RbcDeliveryEntryCommitEvidenceBranchMatchesCertifiedOrPendingStackStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactWitnessSurface": "RbcDeliveryEntryCommitEvidenceBranchMatchesExactWitnessSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesLiveCommitGateCrossing": "RbcDeliveryEntryCommitEvidenceBranchMatchesLiveCommitGateCrossingStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesContinuationMode": "RbcDeliveryEntryCommitEvidenceBranchMatchesContinuationModeStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesViewHandoffSurface": "RbcDeliveryEntryCommitEvidenceBranchMatchesViewHandoffSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesDeliveredEvidenceSurface": "RbcDeliveryEntryCommitEvidenceBranchMatchesDeliveredEvidenceSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesGstTimerSurface": "RbcDeliveryEntryCommitEvidenceBranchMatchesGstTimerSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesProgressActionSurface": "RbcDeliveryEntryCommitEvidenceBranchMatchesProgressActionSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesVoteBudgetSurface": "RbcDeliveryEntryCommitEvidenceBranchMatchesVoteBudgetSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesThresholdClassifier": "RbcDeliveryEntryCommitEvidenceBranchMatchesThresholdClassifierStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingCommitVoteProgressSplit": "RbcDeliveryEntryCommitEvidenceBranchMatchesPendingCommitVoteProgressSplitStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingNonCommitVoteProgressSplit": "RbcDeliveryEntryCommitEvidenceBranchMatchesPendingNonCommitVoteProgressSplitStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingProgressPartition": "RbcDeliveryEntryCommitEvidenceBranchMatchesPendingProgressPartitionStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPostStateClassifier": "RbcDeliveryEntryCommitEvidenceBranchMatchesPostStateClassifierStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCertificateProgressDisjointness": "RbcDeliveryEntryCommitEvidenceBranchMatchesCertificateProgressDisjointnessStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesActionFamilyClassifier": "RbcDeliveryEntryCommitEvidenceBranchMatchesActionFamilyClassifierStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesByzantineCommitVoteBoundary": "RbcDeliveryEntryCommitEvidenceBranchMatchesByzantineCommitVoteBoundaryStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesResidualGatePartition": "RbcDeliveryEntryCommitEvidenceBranchMatchesResidualGatePartitionStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCompleteHandoff": "RbcDeliveryEntryCommitEvidenceBranchMatchesCompleteHandoffStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsContinuationState": "RbcDeliveryEntryCommitEvidenceBranchSeedsContinuationStateStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingActionSurface": "RbcDeliveryEntryCommitEvidenceBranchSeedsPendingActionSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingTimerSurface": "RbcDeliveryEntryCommitEvidenceBranchSeedsPendingTimerSurfaceStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingCounterFrame": "RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCounterFrameStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingCompleteWaitState": "RbcDeliveryEntryCommitEvidenceBranchSeedsPendingCompleteWaitStateStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysHandsOffToDeliveredPendingWaitState": "RbcDeliveryEntryCommitEvidenceBranchHandsOffToDeliveredPendingWaitStateStep",
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCompleteContinuation": "RbcDeliveryEntryCommitEvidenceBranchMatchesCompleteContinuationStep",
    "DeliveredPendingCompleteWaitStateSpecStepAlwaysCloses": "DeliveredPendingCompleteWaitStateSpecStepClosesStep",
    "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysSplits": "DeliveredPendingCompleteWaitStateCommitVoteStepSplitsStep",
    "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysPreservesWaitState": "DeliveredPendingCompleteWaitStateCommitVoteStepPreservesWaitStateStep",
    "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysCompletesFinality": "DeliveredPendingCompleteWaitStateCommitVoteStepCompletesFinalityStep",
    "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysMatchesCertifiedCommitEnvelope": "DeliveredPendingCompleteWaitStateCommitVoteStepMatchesCertifiedCommitEnvelopeStep",
    "DeliveredPendingCompleteWaitStatePrepareVoteStepAlwaysSplits": "DeliveredPendingCompleteWaitStatePrepareVoteStepSplitsStep",
    "DeliveredPendingCompleteWaitStateTimeoutStepAlwaysStartsNewView": "DeliveredPendingCompleteWaitStateTimeoutStepStartsNewViewStep",
    "DeliveredPendingCompleteWaitStateNewViewVoteStepAlwaysSplits": "DeliveredPendingCompleteWaitStateNewViewVoteStepSplitsStep",
    "DeliveredPendingCompleteWaitStateHonestProposeStepAlwaysStartsPrepare": "DeliveredPendingCompleteWaitStateHonestProposeStepStartsPrepareStep",
    "DeliveredPendingCompleteWaitStateGstElapsedStepAlwaysKeepsWaitState": "DeliveredPendingCompleteWaitStateGstElapsedStepKeepsWaitStateStep",
    "DeliveredPendingCompleteWaitStateNextStepAlwaysMatchesNamedActionBranch": "DeliveredPendingCompleteWaitStateNextStepMatchesNamedActionBranchStep",
    "DeliveredPendingCompleteWaitStateStutterStepAlwaysKeepsWaitState": "DeliveredPendingCompleteWaitStateStutterStepKeepsWaitStateStep",
    "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesCompleteBranchClassifier": "DeliveredPendingCompleteWaitStateSpecStepMatchesCompleteBranchClassifierStep",
    "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesCommittedCertifiedEnvelope": "DeliveredPendingCompleteWaitStateSpecStepCommittedOutcomeMatchesCertifiedEnvelopeStep",
    "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesNonCommittedWaitEnvelope": "DeliveredPendingCompleteWaitStateSpecStepNonCommittedOutcomeMatchesWaitEnvelopeStep",
    "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesCompleteOutcomeEnvelope": "DeliveredPendingCompleteWaitStateSpecStepMatchesCompleteOutcomeEnvelopeStep",
}
SUMERAGI_COMMITTED_TERMINAL_ENVELOPE_CONJUNCT_CONTRACTS = {
    "CommittedStateAlwaysMatchesTerminalEnvelope": (
        "CommitNeverRevoked",
        "CommittedPhaseAlwaysMatchesFinality",
        "CommitCertificateAlwaysMatchesFinality",
        "LiveCommitGateAlwaysMatchesFinality",
        "LiveCommitGateRbcEvidenceAlwaysMatches",
        "CommittedPhaseNeverLeaves",
        "CommittedConsensusStateNeverChanges",
        "CommittedOnlyGstObservationCanChange",
        "CommittedPreGstOnlyGstElapsedCanMove",
        "CommittedPreGstNextOnlyGstElapsed",
        "CommittedPreGstSpecStepStuttersOrObservesGst",
        "CommittedGstStateNeverChanges",
        "CommittedGstNeverEnablesActions",
        "CommittedGstOnlyAllowsStuttering",
        "CommittedGstSpecStepOnlyStutters",
        "CommittedSpecNonStutteringOnlyObservesGst",
        "CommittedSpecStepStuttersOrObservesGst",
        "CommittedSpecStepPreservesFinalityStack",
        "CommittedSpecStepOnlyChangesGstFlag",
        "CommittedSpecStepNeverRunsProtocolActions",
        "CommittedSpecStepKeepsProgressActionsQuiescent",
        "CommittedSpecStepPreservesBudgetedRbcEvidence",
    ),
}
SUMERAGI_POST_FINALITY_STABILITY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "PostFinalityStateAlwaysMatchesStabilityEnvelope": (
        "CommitViewNeverChanges",
        "CommittedViewWitnessAlwaysStaysAtCommittedView",
        "CommitViewNeverLeadsCurrentView",
        "GstElapsedGateNeverBypassesPreGst",
        "GstElapsedStepAlwaysOnlySetsGst",
        "GstOnlyChangesByElapsed",
        "GstNeverRegresses",
        "ViewNeverRegresses",
        "CommitViewNeverRegresses",
        "CommitEvidenceNeverRegresses",
    ),
}
SUMERAGI_TIMEOUT_RECOVERY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "TimeoutRecoveryAlwaysMatchesViewChangeEnvelope": (
        "TimeoutTickGateNeverBypassesStalledProgress",
        "TimeoutTickStepAlwaysStartsFreshNewView",
        "TimeoutTickStepNeverPreemptsProgress",
        "TimeoutTickStepAlwaysClearsCommitVoteGates",
        "TimeoutTickStepAlwaysStartsNewViewVoteHandoff",
        "TimeoutTickStepAlwaysPreservesRbcEvidence",
        "ViewAdvanceOnlyComesFromTimeout",
        "LiveProgressResetOnlyByTimeout",
        "ViewEvidenceOnlyChangesByQuorumOrTimeout",
        "NewViewVotesOnlyChangeByVoteOrReset",
        "PrepareVotesOnlyChangeByVoteOrTimeout",
        "CommitVoteCountersOnlyChangeByVoteOrTimeout",
        "PhaseOnlyChangesByProtocol",
    ),
}
SUMERAGI_FINALITY_INSTALLATION_ENVELOPE_CONJUNCT_CONTRACTS = {
    "FinalityInstallationAlwaysMatchesCertifiedCommitEnvelope": (
        "CommitArtifactsOnlyInstallAtFinality",
        "CommitArtifactsOnlyChangeByFinalitySource",
        "CommitArtifactsChangeAlwaysMatchesCertifiedFinalityStack",
        "CommitArtifactsChangeAlwaysCompletesCommittedDeliveryFromExactSource",
        "CommitArtifactsChangeAlwaysCommitsCurrentView",
        "CommitArtifactsChangeNeverChangesGst",
        "CommitArtifactsChangeOnlyLeavesGstElapsedGate",
        "FinalityLatchOnlySetsCompleteStack",
        "FinalityLatchAndArtifactsAlwaysChangeTogether",
        "CommittedPhaseOnlyEntersWithCompleteStack",
        "CommittedPhaseEntryAlwaysMatchesFinalityLatch",
        "FinalityLatchChangeOnlyEntersCommittedPhase",
        "FinalityLatchChangeAlwaysMatchesLiveCommitGateCrossing",
        "CommitCertificateWitnessesAlwaysInstallWithFinalityLatch",
        "CommitCertificateWitnessComponentsAlwaysChangeTogether",
        "CommitCertificateWitnessChangeAlwaysMatchesCertifiedFinalityStack",
        "CommitCertificateWitnessChangeAlwaysInstallsCommitViewWitness",
        "CommitCertificateWitnessChangeAlwaysCompletesCommittedDeliveryFromExactSource",
        "CommitCertificateWitnessChangeNeverChangesGst",
        "CommitCertificateWitnessChangeOnlyLeavesGstElapsedGate",
        "CommitViewWitnessOnlyChangesOnNonzeroFinality",
        "CommitViewWitnessAlwaysInstallsWithFinalityLatch",
        "CommitViewWitnessChangeAlwaysMatchesCertifiedFinalityStack",
        "CommitViewWitnessChangeAlwaysInstallsCommitCertificateWitnesses",
        "CommitViewWitnessChangeAlwaysCompletesCommittedDeliveryFromExactSource",
        "CommitViewWitnessChangeNeverChangesGst",
        "CommitViewWitnessChangeOnlyLeavesGstElapsedGate",
        "FinalityLatchNeverCarriesNewViewHandoff",
        "FinalityLatchOnlyComesFromCommitOrDelivery",
        "FinalityLatchChangeNeverChangesGst",
        "FinalityLatchChangeOnlyLeavesGstElapsedGate",
        "FinalityLatchSourceEffectsAlwaysExact",
        "FinalityLatchSourceQuorumGatesAlwaysHold",
        "FinalitySourceActionAlwaysCompletesCommittedDeliveryFromExactSource",
        "FinalitySourceActionAlwaysMatchesCertifiedSourceStack",
        "FinalitySourceActionAlwaysMatchesFinalityLatchChange",
        "FinalitySourceActionAlwaysMatchesCommittedPhaseEntry",
        "FinalitySourceActionAlwaysInstallsFinalityCertificateStack",
        "FinalitySourceActionSourceAlwaysIsCommitOrDelivery",
        "FinalitySourceActionSourceEffectsAlwaysExact",
        "FinalitySourceActionQuorumGatesAlwaysHold",
        "FinalitySourceActionAlwaysMatchesCommitArtifactsChange",
        "FinalitySourceActionAlwaysMatchesLiveCommitGateCrossing",
        "FinalitySourceActionAlwaysDisablesProgressAfterCommittedDelivery",
        "FinalitySourceActionNeverChangesGst",
        "FinalitySourceActionOnlyLeavesGstElapsedGate",
        "FinalitySourceActionAlwaysInstallsCommitCertificateWitnesses",
        "FinalitySourceActionAlwaysMatchesCommitCertificateWitnessChange",
        "FinalitySourceActionAlwaysMatchesCommitViewWitnessChange",
        "FinalitySourceActionAlwaysInstallsCommitViewWitness",
        "FinalitySourceActionNeverCarriesNewViewHandoff",
        "FinalitySourceActionAlwaysCommitsCurrentView",
        "FinalityLatchChangeAlwaysMatchesCertifiedSourceStack",
        "FinalityLatchChangeAlwaysCompletesCommittedDeliveryFromExactSource",
        "CommittedPhaseEntryOnlyByFinalitySource",
        "CommittedPhaseEntryAlwaysMatchesCertifiedFinalityStack",
        "CommittedPhaseEntryAlwaysInstallsCommitCertificateWitnesses",
        "CommittedPhaseEntryAlwaysMatchesCommitCertificateWitnessChange",
        "CommittedPhaseEntryAlwaysMatchesCommitViewWitnessChange",
        "CommittedPhaseEntryAlwaysInstallsCommitViewWitness",
        "CommittedPhaseEntryAlwaysMatchesLiveCommitGateCrossing",
        "CommittedPhaseEntryAlwaysMatchesCommitArtifactsChange",
        "CommittedPhaseEntryAlwaysMatchesExactFinalitySourceEffects",
        "CommittedPhaseEntryNeverCarriesNewViewHandoff",
        "CommittedPhaseEntryAlwaysCommitsCurrentView",
        "CommittedPhaseEntryNeverChangesGst",
        "CommittedPhaseEntryOnlyLeavesGstElapsedGate",
        "CommittedPhaseEntryAlwaysDisablesProgressActions",
        "CommittedPhaseEntryAlwaysCompletesCommittedDeliveryFromExactSource",
    ),
}
SUMERAGI_PRE_COMMIT_HANDOFF_ENVELOPE_CONJUNCT_CONTRACTS = {
    "PreCommitHandoffAlwaysMatchesProposalPrepareEnvelope": (
        "PreparePhaseEntryOnlyByProposal",
        "CommitVotePhaseEntryOnlyByPrepareQuorum",
        "ProposePhaseEntryOnlyByNewViewQuorum",
        "NewViewPhaseEntryOnlyByTimeout",
        "ViewQuorumEvidenceNeverDiverges",
        "ViewEvidenceWitnessNeverTargetsZeroOrNewView",
        "NewViewQuorumHandoffNeverStalls",
        "LiveNewViewVotesNeverLeakPastHandoff",
        "HonestProposeGateNeverBypassesHandoffEvidence",
        "HonestProposeStepAlwaysStartsPrepareAndRbc",
        "HonestProposeStepAlwaysStartsPrepareVoteHandoff",
        "NewViewVoteGateNeverBypassesFreshViewEvidence",
        "NewViewVoteQuorumGateNeverBypassesNextEvidence",
        "NewViewVoteQuorumStepAlwaysInstallsViewEvidence",
        "NewViewVoteQuorumStepAlwaysStartsProposalHandoff",
        "NewViewVotePendingGateNeverBypassesMissingNextEvidence",
        "NewViewVotePendingStepNeverInstallsViewEvidence",
        "ViewEvidenceNeverPartial",
        "PreCommitVotesNeverCarryAcrossViews",
        "PrePrepareVotesNeverCarryAcrossViews",
        "LivePrepareVotesNeverBypassPrepareHandoff",
        "PrepareVoteGateNeverBypassesProposalEvidence",
        "PrepareVoteQuorumGateNeverBypassesNextEvidence",
        "PrepareVoteQuorumStepAlwaysEntersCommitVote",
        "PrepareVoteQuorumStepAlwaysStartsCommitVoteHandoff",
        "PrepareVotePendingGateNeverBypassesMissingNextEvidence",
        "PrepareVotePendingStepNeverMutatesCommitArtifacts",
        "PrepareVotePendingStepAlwaysKeepsPrepareVoteHandoff",
    ),
}
SUMERAGI_COMMIT_VOTE_HANDOFF_ENVELOPE_CONJUNCT_CONTRACTS = {
    "CommitVoteHandoffAlwaysMatchesFinalityEnvelope": (
        "CommitEvidenceNeverPartial",
        "CommitPhasesNeverBypassPrepareQuorum",
        "LiveCommitVotesNeverBypassPrepareQuorum",
        "CommitVoteGateNeverBypassesPrepareEvidence",
        "ByzantineCommitVoteGateNeverBypassesPrepareEvidence",
        "HonestCommitVoteFinalityGateNeverBypassesNextEvidence",
        "HonestCommitVoteFinalityStepAlwaysInstallsCommitArtifacts",
        "HonestCommitVoteFinalityStepAlwaysCompletesCommittedDelivery",
        "HonestCommitVotePendingGateNeverBypassesMissingNextEvidence",
        "HonestCommitVotePendingStepNeverMutatesCommitArtifacts",
        "HonestCommitVotePendingStepAlwaysKeepsCommitVoteHandoff",
        "ByzantineCommitVoteFinalityGateNeverBypassesNextEvidence",
        "ByzantineCommitVoteFinalityStepAlwaysInstallsCommitArtifacts",
        "ByzantineCommitVoteFinalityStepAlwaysCompletesCommittedDelivery",
        "ByzantineCommitVotePendingGateNeverBypassesMissingNextEvidence",
        "ByzantineCommitVotePendingStepNeverMutatesCommitArtifacts",
        "ByzantineCommitVotePendingStepAlwaysKeepsCommitVoteHandoff",
        "LiveCommitVotesNeverBypassCommitHandoff",
        "PreFinalityCommitArtifactsNeverAppear",
    ),
}
SUMERAGI_FINALIZED_CERTIFICATE_RETENTION_ENVELOPE_CONJUNCT_CONTRACTS = {
    "FinalizedCertificateEvidenceAlwaysMatchesRetentionEnvelope": (
        "FinalityCertificateStackNeverIncomplete",
        "FinalityCertificateStackAlwaysMatchesFinality",
        "FinalityNeverRetainsNewViewHandoff",
        "CommitViewQuorumEvidenceNeverLost",
        "PrepareQuorumNeverLostAfterCommit",
        "LiveCommitQuorumNeverLost",
        "CommitHonestSupportNeverLost",
        "CommitRbcEvidenceNeverLost",
        "CommitProgressActionsNeverReenabled",
        "ByzantineCommitVoteNeverReenabledAfterCommit",
        "CommitEvidenceNeverDivergesFromVoteCounters",
        "StakeAccountingNeverDiverges",
        "LiveStakeNeverExceedsRosterBudget",
        "CommitEvidenceNeverExceedsRosterBudget",
        "VoteCountersNeverExceedRosterBudgets",
        "CommitEvidenceNeverLost",
    ),
}
SUMERAGI_RBC_DELIVERED_FINALITY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcDeliveredFinalityAlwaysMatchesCertifiedCommitEnvelope": (
        "RbcDeliveredFinalityOnlyComesFromCommitVote",
        "RbcDeliveredFinalityAlwaysCompletesCommittedDelivery",
        "RbcDeliveredFinalityAlwaysCommitsCurrentView",
        "RbcDeliveredFinalityOnlyLeavesGstElapsedGate",
        "RbcDeliveredFinalityAlwaysInstallsCommitCertificateWitnesses",
        "RbcDeliveredFinalityAlwaysMatchesCommitCertificateWitnessChange",
        "RbcDeliveredFinalityAlwaysMatchesCommitViewWitnessChange",
        "RbcDeliveredFinalityAlwaysMatchesLiveCommitGateCrossing",
        "RbcDeliveredFinalityAlwaysDisablesProgressAfterCommittedDelivery",
        "RbcDeliveredFinalityAlwaysMatchesCertifiedSourceStack",
        "RbcDeliveredFinalityAlwaysInstallsFinalityCertificateStack",
        "RbcDeliveredFinalityAlwaysMatchesCommittedPhaseEntry",
        "RbcDeliveredFinalityAlwaysMatchesCommitArtifactsChange",
        "RbcDeliveredFinalityAlwaysCouplesLatchAndCommitArtifacts",
        "RbcDeliveredFinalityAlwaysRecordsExactCommitVoteWitnesses",
        "RbcDeliveredFinalityAlwaysPreservesDeliveredRbcEvidence",
        "RbcDeliveredFinalityAlwaysPreservesViewPrepareHandoffEvidence",
        "RbcDeliveredFinalityAlwaysHasExactProtocolFrame",
        "RbcDeliveredFinalityAlwaysHasExactCommitVoteActionFrame",
        "RbcDeliveredFinalityAlwaysInstallsCommittedPostStateInvariants",
        "RbcDeliveredFinalityAlwaysSplitsPostStateGate",
        "RbcDeliveredFinalityPreGstPostStateOnlyLeavesGstElapsed",
        "RbcDeliveredFinalityPostGstPostStateIsTerminal",
    ),
}
SUMERAGI_RBC_DELIVERED_STATE_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcDeliveredStateAlwaysMatchesCompleteLifecycleEnvelope": (
        "RbcDeliveryNeverLost",
        "RbcDeliveredEvidenceNeverRegresses",
        "RbcDeliveredWithoutFinalityNeverCarriesCommitCertificate",
        "RbcDeliveredFinalityAlwaysMatchesCertifiedCommitEnvelope",
        "RbcDeliveredNeverEnablesRbcProgress",
        "RbcDeliveredWithoutFinalityAlwaysWaitsForCommitEvidence",
        "RbcDeliveredPendingHonestCommitVoteAlwaysKeepsWaitState",
        "RbcDeliveredPendingByzantineCommitVoteAlwaysKeepsWaitState",
        "RbcDeliveredPendingHonestCommitVoteAlwaysCompletesFinality",
        "RbcDeliveredPendingByzantineCommitVoteAlwaysCompletesFinality",
        "RbcDeliveredPendingPrepareVoteAlwaysKeepsWaitState",
        "RbcDeliveredPendingPrepareVoteAlwaysStartsCommitVoteWaitState",
        "RbcDeliveredPendingTimeoutAlwaysStartsNewViewWaitState",
        "RbcDeliveredPendingNewViewVoteAlwaysKeepsWaitState",
        "RbcDeliveredPendingNewViewVoteAlwaysStartsProposalWaitState",
        "RbcDeliveredPendingHonestProposeAlwaysStartsPrepareWaitState",
        "RbcDeliveredPendingGstElapsedAlwaysKeepsWaitState",
        "RbcDeliveredPendingNextAlwaysCoveredByHandoffs",
        "RbcDeliveredPendingSpecStepAlwaysMatchesCompleteHandoffEnvelope",
        "DeliveredPendingCompleteWaitStateAlwaysMatchesNamedActionEnvelope",
    ),
}
SUMERAGI_RBC_DELIVERED_PENDING_HANDOFF_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcDeliveredPendingSpecStepAlwaysMatchesCompleteHandoffEnvelope": (
        "RbcDeliveredPendingSpecStepAlwaysStuttersOrTakesCoveredHandoff",
        "RbcDeliveredPendingSpecStepAlwaysEndsInFinalityOrWaitState",
        "RbcDeliveredPendingSpecStepAlwaysPreservesDeliveredRbcEvidence",
        "RbcDeliveredPendingSpecStepAlwaysMatchesCommitArtifactsOutcome",
        "RbcDeliveredPendingSpecStepAlwaysChangesGstOnlyByElapsed",
        "RbcDeliveredPendingSpecStepAlwaysChangesViewOnlyByTimeout",
        "RbcDeliveredPendingSpecStepAlwaysChangesViewEvidenceOnlyByNewViewOrTimeout",
        "RbcDeliveredPendingSpecStepAlwaysMatchesVoteCounterHandoff",
        "RbcDeliveredPendingSpecStepAlwaysMatchesPostGateHandoff",
        "RbcDeliveredPendingSpecStepAlwaysMatchesTimerGateHandoff",
        "RbcDeliveredPendingSpecStepAlwaysMatchesFinalitySource",
        "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityWitnessFrame",
        "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityStackOutcome",
        "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityGateOutcome",
        "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityQuorumOutcome",
        "RbcDeliveredPendingSpecStepAlwaysMatchesNonFinalHandoffPhaseShape",
        "RbcDeliveredPendingSpecStepAlwaysClosesActionSurface",
        "RbcDeliveredPendingSpecStepAlwaysMatchesPhaseChangeAction",
        "RbcDeliveredPendingSpecStepAlwaysMatchesCounterChangeAction",
        "RbcDeliveredPendingSpecStepAlwaysHasExclusiveActionSource",
        "RbcDeliveredPendingSpecStepAlwaysPreservesActionSurfaceOnStutter",
        "RbcDeliveredPendingSpecStepAlwaysMatchesCommitArtifactChangeSource",
        "RbcDeliveredPendingSpecStepAlwaysInstallsCertifiedDeliveryOnCommitArtifactChange",
        "RbcDeliveredPendingSpecStepAlwaysInstallsExactSourceCertifiedDeliveryOnCommitArtifactChange",
        "RbcDeliveredPendingSpecStepAlwaysKeepsNonFinalHandoffOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesNonFinalSourceOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesCounterFootprintOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesPhaseGateFootprintOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesTimerFootprintOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesViewFootprintOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesFinalityFootprintOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysMatchesRbcSurfaceOnStableCommitArtifacts",
        "RbcDeliveredPendingSpecStepAlwaysClosesCompleteWaitStateOnStableCommitArtifacts",
    ),
}
SUMERAGI_DELIVERED_PENDING_COMPLETE_WAIT_STATE_ENVELOPE_CONJUNCT_CONTRACTS = {
    "DeliveredPendingCompleteWaitStateAlwaysMatchesNamedActionEnvelope": (
        "DeliveredPendingCompleteWaitStateSpecStepAlwaysCloses",
        "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysSplits",
        "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysPreservesWaitState",
        "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysCompletesFinality",
        "DeliveredPendingCompleteWaitStateCommitVoteStepAlwaysMatchesCertifiedCommitEnvelope",
        "DeliveredPendingCompleteWaitStatePrepareVoteStepAlwaysSplits",
        "DeliveredPendingCompleteWaitStateTimeoutStepAlwaysStartsNewView",
        "DeliveredPendingCompleteWaitStateNewViewVoteStepAlwaysSplits",
        "DeliveredPendingCompleteWaitStateHonestProposeStepAlwaysStartsPrepare",
        "DeliveredPendingCompleteWaitStateGstElapsedStepAlwaysKeepsWaitState",
        "DeliveredPendingCompleteWaitStateNextStepAlwaysMatchesNamedActionBranch",
        "DeliveredPendingCompleteWaitStateStutterStepAlwaysKeepsWaitState",
        "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesCompleteBranchClassifier",
        "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesCommittedCertifiedEnvelope",
        "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesNonCommittedWaitEnvelope",
        "DeliveredPendingCompleteWaitStateSpecStepAlwaysMatchesCompleteOutcomeEnvelope",
    ),
}
SUMERAGI_RBC_DELIVERY_ENTRY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcDeliveryEntryAlwaysMatchesCompleteOutcomeEnvelope": (
        "RbcDeliveryEntryOnlyByDeliver",
        "RbcDeliveryEntryAlwaysMatchesReadyQuorumExitAndCommitBranch",
        "RbcDeliveryEntryFinalityAlwaysCompletesCommittedDelivery",
        "RbcDeliveryEntryPendingAlwaysInstallsCompleteWaitState",
        "RbcDeliveryEntryAlwaysCompletesFinalityOrWaitState",
        "RbcDeliveryEntryAlwaysMatchesCommitArtifactOutcome",
        "RbcDeliveryEntryAlwaysMatchesPostGateSurfaceOutcome",
        "RbcDeliveryEntryAlwaysMatchesConsensusFrameOutcome",
        "RbcDeliveryEntryFinalityAlwaysMatchesCertifiedSourceStack",
        "RbcDeliveryEntryFinalityAlwaysInstallsCommittedPostStateInvariants",
        "RbcDeliveryEntryFinalityAlwaysSplitsPostStateGate",
        "RbcDeliveryEntryFinalityPreGstPostStateOnlyLeavesGstElapsed",
        "RbcDeliveryEntryFinalityPostGstPostStateIsTerminal",
        "RbcDeliveryEntryPendingAlwaysMatchesNonFinalWaitSurface",
        "RbcDeliveryEntryPendingAlwaysSplitsPostStateTimerGate",
        "RbcDeliveryEntryPendingPreGstPostStateAlwaysKeepsWaitTimers",
        "RbcDeliveryEntryPendingPostGstPostStateAlwaysTracksProgressTimeout",
        "RbcDeliveryEntryPendingAlwaysInstallsDeliveredWaitPredicate",
        "RbcDeliveryEntryPendingAlwaysOpensDeliveredPendingContinuationSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesContinuationEnvelope",
    ),
}
SUMERAGI_RBC_DELIVERY_ENTRY_CONTINUATION_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesContinuationEnvelope": (
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysOpensExactContinuation",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExclusiveOutcome",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExclusiveGateOutcome",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactConsensusFrame",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactActionSource",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCertifiedOrPendingStack",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesExactWitnessSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesLiveCommitGateCrossing",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesContinuationMode",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesViewHandoffSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesDeliveredEvidenceSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesGstTimerSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesProgressActionSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesVoteBudgetSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesThresholdClassifier",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingCommitVoteProgressSplit",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingNonCommitVoteProgressSplit",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPendingProgressPartition",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesPostStateClassifier",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCertificateProgressDisjointness",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesActionFamilyClassifier",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesByzantineCommitVoteBoundary",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesResidualGatePartition",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCompleteHandoff",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsContinuationState",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingActionSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingTimerSurface",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingCounterFrame",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysSeedsPendingCompleteWaitState",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysHandsOffToDeliveredPendingWaitState",
        "RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesCompleteContinuation",
    ),
}
SUMERAGI_RBC_LIFECYCLE_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcLifecycleAlwaysMatchesEndToEndEnvelope": (
        "RbcProgressMutationAlwaysPreservesLiveEvidenceEnvelope",
        "RbcCorruptionRepairAlwaysMatchesFaultEnvelope",
        "RbcChunkReadyDeliverAlwaysMatchesAvailabilityEnvelope",
        "RbcDeliveryEntryAlwaysMatchesCompleteOutcomeEnvelope",
        "RbcDeliveredStateAlwaysMatchesCompleteLifecycleEnvelope",
    ),
}
SUMERAGI_RBC_CORRUPTION_REPAIR_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcCorruptionRepairAlwaysMatchesFaultEnvelope": (
        "ByzantineFaultGateNeverBypassesCorruptibleRbc",
        "ByzantineFaultStepAlwaysCorruptsOnlyRbcDigest",
        "RbcDigestInvalidationOnlyByFault",
        "RbcCorruptionEntryOnlyByFault",
        "RbcCorruptedDigestNeverValid",
        "RbcCorruptedAlwaysRetainsHeaderEvidence",
        "RbcCorruptedNeverCarriesFinalityArtifacts",
        "RbcCorruptedNeverBypassesInitRepairProgress",
        "RbcCorruptionExitOnlyByInit",
        "RbcCorruptedInitRepairAlwaysResetsEvidence",
        "RbcInitGateNeverBypassesRepairableState",
        "RbcInitStepAlwaysInstallsHeaderDigestEvidence",
        "RbcInitStepAlwaysStartsChunkOnlyHandoff",
    ),
}
SUMERAGI_RBC_CHUNK_READY_DELIVER_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcChunkReadyDeliverAlwaysMatchesAvailabilityEnvelope": (
        "RbcChunkGateNeverBypassesHeaderDigestEvidence",
        "RbcChunkStepAlwaysAdvancesChunkEvidence",
        "RbcChunkStepAlwaysHandsOffByCoverage",
        "RbcInitExitOnlyByChunkOrFault",
        "RbcChunkCountIncreaseOnlyByChunk",
        "RbcChunkCountDecreaseOnlyByProposalOrInit",
        "RbcChunkingEntryOnlyByChunk",
        "RbcChunkingExitOnlyByChunkOrFault",
        "RbcChunkCompletionEntryOnlyByChunk",
        "RbcChunksCompleteExitOnlyByReadyOrFault",
        "RbcReadyGateNeverBypassesChunkEvidence",
        "RbcReadyStepAlwaysAdvancesReadyEvidence",
        "RbcReadyStepAlwaysHandsOffByQuorum",
        "RbcReadyQuorumStepAlwaysEnablesDeliverHandoff",
        "RbcReadyVotesIncreaseOnlyByReady",
        "RbcReadyVotesDecreaseOnlyByProposalOrInit",
        "RbcReadyPartialEntryOnlyByReady",
        "RbcReadyPartialExitOnlyByReadyOrFault",
        "RbcReadyQuorumEntryOnlyByReady",
        "RbcReadyQuorumExitOnlyByDeliverOrFault",
        "RbcDeliverGateNeverBypassesCompleteEvidence",
        "RbcReadyQuorumNeverLacksDeliverGate",
        "RbcDeliverStepAlwaysPreservesCompleteEvidence",
        "RbcDeliverStepAlwaysHandsOffByCommitEvidence",
        "RbcDeliverFinalityGateNeverBypassesBufferedCommitEvidence",
        "RbcDeliverFinalityStepAlwaysInstallsCommitArtifacts",
        "RbcDeliverFinalityStepAlwaysCompletesCommittedDelivery",
        "RbcDeliverPendingGateNeverBypassesMissingBufferedCommitEvidence",
        "RbcDeliverPendingStepNeverMutatesCommitArtifacts",
        "RbcDeliverPendingStepAlwaysKeepsDeliveredEvidenceWithoutFinality",
    ),
}
SUMERAGI_RBC_PROGRESS_MUTATION_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcProgressMutationAlwaysPreservesLiveEvidenceEnvelope": (
        "RbcProgressStateEvidenceAlwaysMatchesEnvelope",
        "RbcProgressMutationAlwaysMatchesLocalClassification",
        "LiveHeaderDigestEvidenceNeverBypassRbcHandoff",
        "LiveChunkEvidenceNeverBypassRbcHandoff",
        "LiveReadyVotesNeverBypassRbcHandoff",
    ),
}
SUMERAGI_RBC_PROGRESS_LOCAL_CLASSIFICATION_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcProgressMutationAlwaysMatchesLocalClassification": (
        "RbcStateOnlyChangesByProtocolOrFault",
        "RbcStateChangeAlwaysMatchesLocalExitClassification",
        "RbcEvidenceOnlyChangesByProtocolOrFault",
        "RbcEvidenceChangeAlwaysMatchesLocalEffectClassification",
        "RbcStartupAndDefensiveBoundaryAlwaysMatchesEnvelope",
        "RbcDeliveryEntryOnlyByDeliver",
        "RbcDeliveryEntryAlwaysMatchesReadyQuorumExitAndCommitBranch",
        "RbcCorruptionEntryOnlyByFault",
        "RbcCorruptionExitOnlyByInit",
    ),
}
SUMERAGI_RBC_STARTUP_BOUNDARY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcStartupAndDefensiveBoundaryAlwaysMatchesEnvelope": (
        "RbcIdleExitOnlyByProposalOrInit",
        "RbcInitEntryOnlyByProposalOrInit",
        "RbcWithheldNeverReached",
        "RbcWithheldEntryOnlyByStutteringFromWithheld",
    ),
}
SUMERAGI_RBC_PROGRESS_STATE_EVIDENCE_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcProgressStateEvidenceAlwaysMatchesEnvelope": (
        "RbcProgressEvidenceNeverDiverges",
        "RbcPartialProgressEvidenceNeverDiverges",
        "RbcLiveEvidenceCausalityAlwaysMatchesEnvelope",
    ),
}
SUMERAGI_RBC_LIVE_EVIDENCE_CAUSALITY_ENVELOPE_CONJUNCT_CONTRACTS = {
    "RbcLiveEvidenceCausalityAlwaysMatchesEnvelope": (
        "RbcHeaderInstallationOnlyByProposalOrInit",
        "RbcHeaderEvidenceNeverLost",
        "RbcMissingHeaderNeverLeavesIdle",
        "RbcHeaderEvidenceNeverReturnsToIdle",
        "RbcDigestInstallationOnlyByProposalInitOrChunk",
        "RbcValidDigestNeverOutrunsHeader",
        "RbcValidDigestNeverLeavesActiveStates",
        "RbcChunkEvidenceNeverOutrunsHeader",
        "RbcChunkEvidenceNeverLeavesChunkOrCorruptedHandoff",
        "RbcPartialChunkEvidenceNeverLeavesChunkingOrCorruptedHandoff",
        "RbcFullChunkCoverageNeverLeavesCoveredOrCorruptedHandoff",
        "RbcZeroChunkEvidenceNeverLeavesPreChunkOrCorruptedHandoff",
        "RbcReadyVotesNeverOutrunChunkHeaderEvidence",
        "RbcReadyVotesNeverLeaveReadyOrCorruptedHandoff",
        "RbcPartialReadyEvidenceNeverLeavesPartialOrCorruptedHandoff",
        "RbcReadyQuorumEvidenceNeverLeavesQuorumOrCorruptedHandoff",
        "RbcZeroReadyEvidenceNeverLeavesPreReadyOrCorruptedHandoff",
        "RbcCounterEvidenceNeverOutrunsValidDigestOrCorruption",
        "RbcInvalidDigestNeverLeavesIdleOrCorruption",
    ),
}
SUMERAGI_BYZANTINE_TOP_COMMON_CONJUNCTS = (
    "TlcByzantineDirectCommitCorridor",
    "CommitImpliesHonestSupport",
    "FinalityCertificateStackComplete",
    "CommitDisablesByzantineCommitVote",
    "ByzantineCommitVoteGateMatchesPrepareEvidence",
    "ByzantineCommitVoteFinalityGateMatchesNextEvidence",
    "ByzantineCommitVotePendingGateMatchesMissingNextEvidence",
    "RbcDeliverFinalityGateMatchesBufferedCommitEvidence",
    "RbcDeliverPendingGateMatchesMissingBufferedCommitEvidence",
    "CommitEvidenceMatchesVoteCounters",
    "VoteCountersRespectRosterBudgets",
    "StakeSignedMatchesVoteCounters",
    "NoCommitEvidenceBeforeCommit",
)
SUMERAGI_BYZANTINE_TOP_WAIT_CONJUNCTS = (
    "RbcDeliveredWithoutFinalityHasNoCommitCertificate",
    "RbcDeliveredWithoutFinalityWaitsForCommitEvidence",
)
SUMERAGI_BYZANTINE_TOP_CONJUNCT_CONTRACTS = {
    "ByzantineDeliveredFirstTopExactness": SUMERAGI_BYZANTINE_TOP_COMMON_CONJUNCTS,
    "ByzantineDeliveredFirstTopCorrectnessEnvelope": (
        "TypeInvariant",
        "ByzantineDeliveredFirstTopExactness",
    ),
    "ByzantineVoteFirstTopExactness": (
        *SUMERAGI_BYZANTINE_TOP_COMMON_CONJUNCTS,
        *SUMERAGI_BYZANTINE_TOP_WAIT_CONJUNCTS,
    ),
    "ByzantineVoteFirstTopCorrectnessEnvelope": (
        "TypeInvariant",
        "ByzantineVoteFirstTopExactness",
    ),
    "ByzantineDirectTopExactness": (
        *SUMERAGI_BYZANTINE_TOP_COMMON_CONJUNCTS,
        *SUMERAGI_BYZANTINE_TOP_WAIT_CONJUNCTS,
    ),
    "ByzantineDirectTopCorrectnessEnvelope": (
        "TypeInvariant",
        "ByzantineDirectTopExactness",
    ),
    "ByzantineDirectTopCoversOrderedTopCorridors": (
        "ByzantineDeliveredFirstTopExactness",
        "ByzantineVoteFirstTopExactness",
    ),
}
SUMERAGI_BYZANTINE_TOP_IMPLICATION_CONTRACTS = {
    "ByzantineDirectTopCoversOrderedTopCorridors": "ByzantineDirectTopExactness",
}
SUMERAGI_DIRECT_INTERLEAVING_GATE_CONJUNCT_CONTRACTS = {
    "DirectCommitInterleavingExactness": (
        "RbcEvidenceShape",
        "VoteHandoffShape",
        "CommitCertificateShape",
        "BufferedVotesWaitForDelivery",
        "DeliveredWithBufferedVotesCommits",
    ),
    "DirectCommitInterleavingCorrectnessEnvelope": (
        "TypeInvariant",
        "DirectCommitInterleavingExactness",
    ),
    "DirectCommitProgressSafetyEnvelope": (
        "TypeInvariant",
        "DirectCommitInterleavingExactness",
    ),
}
SUMERAGI_DIRECT_DELIVERED_FIRST_GATE_CONJUNCT_CONTRACTS = {
    "DirectDeliveredFirstCorridorExactness": (
        "PhaseMatchesSpec",
        "RbcStateMatchesSpec",
        "RbcEvidenceMatchesSpec",
        "VoteCountersMatchSpec",
        "CommitEvidenceMatchesSpec",
        "FinalityRequiresDeliveredQuorumAndStake",
    ),
    "DirectDeliveredFirstCorridorCorrectnessEnvelope": (
        "TypeInvariant",
        "DirectDeliveredFirstCorridorExactness",
    ),
    "DirectDeliveredFirstProgressSafetyEnvelope": (
        "TypeInvariant",
        "DirectDeliveredFirstCorridorExactness",
    ),
}
SUMERAGI_DIRECT_VOTE_FIRST_GATE_CONJUNCT_CONTRACTS = {
    "DirectVoteFirstCorridorExactness": (
        "PhaseMatchesSpec",
        "RbcStateMatchesSpec",
        "RbcEvidenceMatchesSpec",
        "VoteCountersMatchSpec",
        "CommitEvidenceMatchesSpec",
        "BufferedCommitWaitHasNoCertificate",
        "FinalityRequiresBufferedVotesAndDelivery",
    ),
    "DirectVoteFirstCorridorCorrectnessEnvelope": (
        "TypeInvariant",
        "DirectVoteFirstCorridorExactness",
    ),
    "DirectVoteFirstProgressSafetyEnvelope": (
        "TypeInvariant",
        "DirectVoteFirstCorridorExactness",
    ),
}
SUMERAGI_BYZANTINE_INTERLEAVING_GATE_CONJUNCT_CONTRACTS = {
    "ByzantineCommitInterleavingExactness": (
        "RbcEvidenceShape",
        "ProposedRoundInitializesRbc",
        "VoteHandoffShape",
        "CommitCertificateShape",
        "BufferedVotesWaitForDelivery",
        "DeliveredWithBufferedVotesCommits",
    ),
    "ByzantineCommitInterleavingCorrectnessEnvelope": (
        "TypeInvariant",
        "ByzantineCommitInterleavingExactness",
    ),
    "ByzantineCommitProgressSafetyEnvelope": (
        "TypeInvariant",
        "ByzantineCommitInterleavingExactness",
    ),
}
BYZANTINE_INTERLEAVING_EXTRA_EXACTNESS_CONJUNCTS = (
    "ProposedRoundInitializesRbc",
)
SOURCE_PROGRESS_SAFETY_ENVELOPE_ALIGNMENT_CONTRACTS = (
    (
        "direct delivered-first",
        SUMERAGI_DIRECT_DELIVERED_FIRST_GATE_CONJUNCT_CONTRACTS,
        "DirectDeliveredFirstProgressSafetyEnvelope",
        ("TypeInvariant", "DirectDeliveredFirstCorridorExactness"),
    ),
    (
        "direct vote-first",
        SUMERAGI_DIRECT_VOTE_FIRST_GATE_CONJUNCT_CONTRACTS,
        "DirectVoteFirstProgressSafetyEnvelope",
        ("TypeInvariant", "DirectVoteFirstCorridorExactness"),
    ),
    (
        "direct interleaving",
        SUMERAGI_DIRECT_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
        "DirectCommitProgressSafetyEnvelope",
        ("TypeInvariant", "DirectCommitInterleavingExactness"),
    ),
    (
        "Byzantine interleaving",
        SUMERAGI_BYZANTINE_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
        "ByzantineCommitProgressSafetyEnvelope",
        ("TypeInvariant", "ByzantineCommitInterleavingExactness"),
    ),
)
SUMERAGI_PROJECTION_GATE_CONJUNCT_CONTRACTS = {
    "ProjectedByzantineDeliveredFirstTopExactness": (
        "ProjectedTlcByzantineDirectCommitCorridor",
        "ProjectedCommitImpliesHonestSupport",
        "ProjectedFinalityCertificateStackComplete",
        "ProjectedCommitDisablesByzantineCommitVote",
        "ProjectedByzantineCommitVoteGateMatchesPrepareEvidence",
        "ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence",
        "ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence",
        "ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence",
        "ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence",
        "ProjectedCommitEvidenceMatchesVoteCounters",
        "ProjectedVoteCountersRespectRosterBudgets",
        "ProjectedStakeSignedMatchesVoteCounters",
        "ProjectedNoCommitEvidenceBeforeCommit",
    ),
    "ProjectedByzantineDeliveredFirstTopCorrectnessEnvelope": (
        "TypeInvariant",
        "ProjectedByzantineDeliveredFirstTopExactness",
    ),
    "ProjectedByzantineVoteFirstTopExactness": (
        "ProjectedTlcByzantineDirectCommitCorridor",
        "ProjectedCommitImpliesHonestSupport",
        "ProjectedFinalityCertificateStackComplete",
        "ProjectedCommitDisablesByzantineCommitVote",
        "ProjectedByzantineCommitVoteGateMatchesPrepareEvidence",
        "ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence",
        "ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence",
        "ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence",
        "ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence",
        "ProjectedCommitEvidenceMatchesVoteCounters",
        "ProjectedVoteCountersRespectRosterBudgets",
        "ProjectedStakeSignedMatchesVoteCounters",
        "ProjectedNoCommitEvidenceBeforeCommit",
        "ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate",
        "ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence",
    ),
    "ProjectedByzantineVoteFirstTopCorrectnessEnvelope": (
        "TypeInvariant",
        "ProjectedByzantineVoteFirstTopExactness",
    ),
    "ProjectedByzantineDirectTopExactness": (
        "ProjectedTlcByzantineDirectCommitCorridor",
        "ProjectedCommitImpliesHonestSupport",
        "ProjectedFinalityCertificateStackComplete",
        "ProjectedCommitDisablesByzantineCommitVote",
        "ProjectedByzantineCommitVoteGateMatchesPrepareEvidence",
        "ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence",
        "ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence",
        "ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence",
        "ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence",
        "ProjectedCommitEvidenceMatchesVoteCounters",
        "ProjectedVoteCountersRespectRosterBudgets",
        "ProjectedStakeSignedMatchesVoteCounters",
        "ProjectedNoCommitEvidenceBeforeCommit",
        "ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate",
        "ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence",
    ),
    "ProjectedByzantineDirectTopCorrectnessEnvelope": (
        "TypeInvariant",
        "ProjectedByzantineDirectTopExactness",
    ),
    "ProjectionBridgeCoversOrderedTopCorridors": (
        "ProjectedByzantineDeliveredFirstTopExactness",
        "ProjectedByzantineVoteFirstTopExactness",
    ),
    "ProjectionBridgeMatchesInterleavingCore": (
        "RbcEvidenceShape",
        "ProposedRoundInitializesRbc",
        "VoteHandoffShape",
        "CommitCertificateShape",
        "BufferedVotesWaitForDelivery",
        "DeliveredWithBufferedVotesCommits",
    ),
    "ProjectionBridgeMatchesInterleavingExactness": (
        "ProjectedTlcByzantineDirectCommitCorridor",
        "ProjectedCommitImpliesHonestSupport",
        "ProjectedFinalityCertificateStackComplete",
        "ProjectedCommitDisablesByzantineCommitVote",
        "ProjectedByzantineCommitVoteGateMatchesPrepareEvidence",
        "ProjectedByzantineCommitVoteFinalityGateMatchesNextEvidence",
        "ProjectedByzantineCommitVotePendingGateMatchesMissingNextEvidence",
        "ProjectedRbcDeliverFinalityGateMatchesBufferedCommitEvidence",
        "ProjectedRbcDeliverPendingGateMatchesMissingBufferedCommitEvidence",
        "ProjectedCommitEvidenceMatchesVoteCounters",
        "ProjectedVoteCountersRespectRosterBudgets",
        "ProjectedStakeSignedMatchesVoteCounters",
        "ProjectedNoCommitEvidenceBeforeCommit",
        "ProjectedRbcDeliveredWithoutFinalityHasNoCommitCertificate",
        "ProjectedRbcDeliveredWithoutFinalityWaitsForCommitEvidence",
        "RbcEvidenceShape",
        "ProposedRoundInitializesRbc",
        "VoteHandoffShape",
        "CommitCertificateShape",
        "BufferedVotesWaitForDelivery",
        "DeliveredWithBufferedVotesCommits",
    ),
    "ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope": (
        "TypeInvariant",
        "ProjectionBridgeMatchesInterleavingExactness",
    ),
    "ProjectedCommitProgressSafetyEnvelope": (
        "ProjectionBridgeCoversOrderedTopCorridors",
        "ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope",
    ),
}
SUMERAGI_PROJECTED_COMMIT_PROGRESS_FAIRNESS_ACTIONS = (
    "HonestPropose",
    "PrepareVote",
    "HonestCommitVote",
    "ByzantineCommitVote",
    "RbcChunk",
    "RbcReady",
    "RbcDeliver",
)
SUMERAGI_PROJECTED_COMMIT_PROGRESS_SPEC_CONJUNCT_CONTRACTS = {
    "ProjectedCommitProgressSpec": (
        "Init",
        "ProjectedCommitProgressFairness",
    ),
}
SUMERAGI_SOURCE_COMMIT_PROGRESS_SPEC_CONTRACTS = (
    (
        SPEC_DIR / "SumeragiDirectDeliveredFirstCorridorGate.tla",
        "DirectDeliveredFirstCorridorProgressSpec",
        "DirectDeliveredFirstCorridorProgressFairness",
        "[][DeliveredFirstPathNext]_vars",
        ("DeliveredFirstPathAdvance",),
        "direct delivered-first progress",
    ),
    (
        SPEC_DIR / "SumeragiDirectVoteFirstCorridorGate.tla",
        "DirectVoteFirstCorridorProgressSpec",
        "DirectVoteFirstCorridorProgressFairness",
        "[][VoteFirstPathNext]_vars",
        ("VoteFirstPathAdvance",),
        "direct vote-first progress",
    ),
    (
        SPEC_DIR / "SumeragiDirectCommitInterleavingGate.tla",
        "DirectCommitInterleavingProgressSpec",
        "DirectCommitInterleavingProgressFairness",
        "[][Next]_vars",
        (
            "HonestPropose",
            "PrepareVote",
            "CommitVote",
            "RbcChunk",
            "RbcReady",
            "RbcDeliver",
        ),
        "direct commit interleaving progress",
    ),
    (
        SPEC_DIR / "SumeragiByzantineCommitInterleavingGate.tla",
        "ByzantineCommitInterleavingProgressSpec",
        "ByzantineCommitInterleavingProgressFairness",
        "[][Next]_vars",
        (
            "HonestPropose",
            "PrepareVote",
            "HonestCommitVote",
            "ByzantineCommitVote",
            "RbcChunk",
            "RbcReady",
            "RbcDeliver",
        ),
        "Byzantine commit interleaving progress",
    ),
)
SUMERAGI_TOP_LEVEL_COMMIT_SPEC_CONTRACTS = (
    (
        SPEC_DIR / "Sumeragi.tla",
        "Spec",
        "Fairness",
        "[][Next]_vars",
        (
            "HonestPropose",
            "HonestPrepareVote",
            "HonestCommitVote",
            "HonestNewViewVote",
            "RbcInit",
            "RbcChunkGood",
            "RbcReadyGood",
            "RbcDeliverGood",
        ),
        "top-level Sumeragi spec",
    ),
    (
        SPEC_DIR / "Sumeragi.tla",
        "DirectCommitSpec",
        "DirectCommitFairness",
        "[][DirectCommitNext]_vars",
        (
            "HonestPropose",
            "HonestPrepareVote",
            "HonestCommitVote",
            "RbcInit",
            "RbcChunkGood",
            "RbcReadyGood",
            "RbcDeliverGood",
        ),
        "top-level direct commit spec",
    ),
    (
        SPEC_DIR / "Sumeragi.tla",
        "ByzantineDirectCommitSpec",
        "DirectCommitFairness",
        "[][ByzantineDirectCommitNext]_vars",
        (
            "HonestPropose",
            "HonestPrepareVote",
            "HonestCommitVote",
            "RbcInit",
            "RbcChunkGood",
            "RbcReadyGood",
            "RbcDeliverGood",
        ),
        "top-level Byzantine direct commit spec",
    ),
    (
        SPEC_DIR / "Sumeragi.tla",
        "ByzantineDeliveredFirstCommitSpec",
        "DirectDeliveredFirstCommitFairness",
        "[][ByzantineDeliveredFirstCommitNext]_vars",
        (
            "HonestPropose",
            "HonestPrepareVote",
            "HonestCommitVote",
            "RbcChunkGood",
            "RbcReadyGood",
            "RbcDeliverGood",
        ),
        "top-level Byzantine delivered-first commit spec",
    ),
    (
        SPEC_DIR / "Sumeragi.tla",
        "DirectDeliveredFirstCommitSpec",
        "DirectDeliveredFirstCommitFairness",
        "[][DirectDeliveredFirstCommitNext]_vars",
        (
            "HonestPropose",
            "HonestPrepareVote",
            "HonestCommitVote",
            "RbcChunkGood",
            "RbcReadyGood",
            "RbcDeliverGood",
        ),
        "top-level direct delivered-first commit spec",
    ),
    (
        SPEC_DIR / "Sumeragi.tla",
        "ByzantineVoteFirstCommitSpec",
        "DirectVoteFirstCommitFairness",
        "[][ByzantineVoteFirstCommitNext]_vars",
        (
            "HonestPropose",
            "HonestPrepareVote",
            "HonestCommitVote",
            "RbcChunkGood",
            "RbcReadyGood",
            "RbcDeliverGood",
        ),
        "top-level Byzantine vote-first commit spec",
    ),
)
SUMERAGI_PROJECTION_GATE_IMPLICATION_CONTRACTS = {
    "ProjectionBridgeCoversOrderedTopCorridors": "ProjectedByzantineDirectTopExactness",
    "ProjectionBridgeMatchesInterleavingCore": "ProjectedByzantineDirectTopExactness",
}
SUMERAGI_BYZANTINE_TOP_TO_PROJECTION_OPERATOR_CONTRACTS = {
    "ByzantineDeliveredFirstTopExactness": (
        "ProjectedByzantineDeliveredFirstTopExactness"
    ),
    "ByzantineDeliveredFirstTopCorrectnessEnvelope": (
        "ProjectedByzantineDeliveredFirstTopCorrectnessEnvelope"
    ),
    "ByzantineVoteFirstTopExactness": "ProjectedByzantineVoteFirstTopExactness",
    "ByzantineVoteFirstTopCorrectnessEnvelope": (
        "ProjectedByzantineVoteFirstTopCorrectnessEnvelope"
    ),
    "ByzantineDirectTopExactness": "ProjectedByzantineDirectTopExactness",
    "ByzantineDirectTopCorrectnessEnvelope": (
        "ProjectedByzantineDirectTopCorrectnessEnvelope"
    ),
    "ByzantineDirectTopCoversOrderedTopCorridors": (
        "ProjectionBridgeCoversOrderedTopCorridors"
    ),
}
SUMERAGI_TOP_TO_PROJECTION_LITERAL_CONJUNCTS = {
    "TypeInvariant": "TypeInvariant",
}
FAST_CI = ROOT_DIR / "ci" / "check_sumeragi_formal.sh"
EXPECTED_FAILURE_CI = ROOT_DIR / "ci" / "check_sumeragi_formal_expected_failures.sh"
PR_WORKFLOW = ROOT_DIR / ".github" / "workflows" / "pr.yml"
NIGHTLY_WORKFLOW = ROOT_DIR / ".github" / "workflows" / "nightly_sumeragi_formal.yml"
README = SPEC_DIR / "README.md"

FORMAL_COVERAGE_COMMAND = "python3 scripts/formal/check_sumeragi_formal_coverage.py"
FORMAL_BASELINE_COMMAND = "bash ci/check_sumeragi_formal.sh"
FORMAL_EXPECTED_FAILURE_COMMAND = (
    "bash ci/check_sumeragi_formal_expected_failures.sh"
)
FRONTIER_NIGHTLY_COMMAND = (
    "bash scripts/formal/sumeragi_apalache.sh frontier-nightly"
)
APALACHE_COMMAND_PREFIX = "bash scripts/formal/sumeragi_apalache.sh"
TLC_COMMAND_PREFIX = "bash scripts/formal/sumeragi_tlc.sh"
INSTALL_APALACHE_COMMAND_PREFIX = "bash scripts/formal/install_apalache.sh"
APALACHE_EXPECTED_FAILURE_SNIPPETS = (
    'if [[ "$expect_failure" == "1" ]]; then',
    'if [[ "$status" == "0" ]]; then',
    'if [[ "$status" != "12" ]]; then',
    "expected Apalache invariant rejection",
    "expected Apalache rejection observed",
)
TLC_EXPECTED_FAILURE_SNIPPETS = (
    'if [[ "$expect_failure" -eq 1 ]]; then',
    'if [[ "$tlc_status" -eq 0 ]]; then',
    "Invariant .* is violated|Error: Invariant",
    "failed without the expected invariant violation",
    "produced the expected failure",
)
APALACHE_INVOCATION_SNIPPETS = (
    'check --length="$apalache_length" --config="$cfg_file" --run-dir="$run_dir" "$spec_file"',
    'check --length="$apalache_length" --config="$cfg_rel" --run-dir="$run_rel" "$spec_rel"',
)
TLC_INVOCATION_SNIPPETS = (
    'java ${TLC_JAVA_OPTS:-} -cp "$tlc_jar" tlc2.TLC',
    '-workers "$workers"',
    '-metadir "$run_dir"',
    '-config "$cfg_file"',
    '"$module"',
)
APALACHE_ONLY_PR_MODES = {
    "deep",
    "byzantine-direct-top-fast",
    "byzantine-delivered-first-top-fast",
    "byzantine-vote-first-top-fast",
}
APALACHE_ONLY_PR_MODE_README_SNIPPETS = (
    "`deep` is intentionally Apalache-only in PR CI",
    "`byzantine-direct-top-fast` is Apalache-only in PR CI",
    "`byzantine-delivered-first-top-fast` is Apalache-only in PR CI",
    "`byzantine-vote-first-top-fast` is Apalache-only in PR CI",
    "Every non-allowlisted PR baseline mode must have both a TLC runner case and README command.",
)
APALACHE_TYPECHECK_ONLY_MODES = {
    "fast",
    "byzantine-direct-top-fast",
    "byzantine-delivered-first-top-fast",
    "byzantine-vote-first-top-fast",
}
APALACHE_TYPECHECK_ONLY_README_SNIPPETS = (
    "The Apalache `fast` mode is intentionally a monolithic-module typecheck smoke.",
    "`byzantine-direct-top-fast` is a focused top-level `Sumeragi.tla`",
    "`byzantine-delivered-first-top-fast` is a focused top-level `Sumeragi.tla`",
    "`byzantine-vote-first-top-fast` is a focused top-level `Sumeragi.tla`",
)
FORMAL_README_GUARD_CONTRACT_SNIPPETS = (
    "Constants and variables share a single",
    "TLA declaration namespace",
    "Declared constants and variables must also remain",
    "disjoint from top-level operator definitions and `RECURSIVE` declarations",
    "same operator name must not be reused across behavior",
    "constraint, and proof-check roles",
    "TLA operator definitions must be non-LOCAL",
    "TLA `RECURSIVE` declaration directives must be top-level",
    "Malformed `RECURSIVE` starts are rejected",
    "Top-level no-separator `RECURSIVE` starts are rejected",
    "aliases must be duplicate-free",
    "top-level proof-target operators",
    "be duplicate-free, use non-reserved static module identifiers",
    "be top-level",
    "EXTENDS entries must appear before declarations and definitions",
    "INSTANCE entries must appear before operator definitions",
    "without `WITH` substitutions",
    "Malformed `EXTENDS`/`INSTANCE` starts are rejected",
    "No-separator `EXTENDS`/`INSTANCE` starts are rejected",
    "Malformed named `INSTANCE` aliases are rejected",
    "No-separator named `INSTANCE` aliases are rejected",
    "INSTANCE declarations must be non-LOCAL",
    "Local TLA dependency files are followed transitively",
    "DirectCommitProgressSafetyEnvelope",
    "DirectDeliveredFirstProgressSafetyEnvelope",
    "DirectVoteFirstProgressSafetyEnvelope",
    "ByzantineCommitProgressSafetyEnvelope",
    "compares the source/projection Byzantine mutation suffix families",
    "top/projection Byzantine direct-commit contracts stay aligned",
    "projection bridge interleaving exactness composes projected direct-top and source core obligations",
    "same module-header, declaration, and assumption/proof guards",
    "Assumption/proof directive starts are rejected even when indented",
    "No-separator assumption/proof directive starts are rejected even when indented",
    "TLA module headers and terminators must be top-level",
    "Decorative all-`=` separator lines are allowed before that terminator",
    "Decorative all-`=` separator lines must not have trailing content",
    "Malformed TLA module header starts are rejected",
    "No-separator TLA module header starts are rejected",
    "Malformed TLA terminator starts are rejected",
    "TLA constant and variable declaration directives must be top-level",
    "Malformed TLA constant/variable declaration starts are rejected",
    "Top-level no-separator TLA constant/variable declaration starts are rejected",
    "Top-level no-separator TLA declaration block entries are rejected",
    "Malformed TLA `vars` tuple starts are rejected",
    "Directive-prefixed TLA declaration block entries remain valid",
    "Malformed supported CFG directive starts are rejected",
    "Directive-prefixed CFG block entries remain valid",
    "Indented no-separator supported CFG directive starts are rejected",
    "Malformed CHECK_DEADLOCK starts are rejected",
    "temporal CFGs keep CHECK_DEADLOCK FALSE",
    "Malformed CFG constant binding starts are rejected",
    "Top-level no-separator CFG constant binding starts are rejected",
    "Indented no-separator CFG constant binding directive starts are rejected",
    "Malformed CFG operator-reference directive starts are rejected",
    "Top-level no-separator CFG operator-reference directive starts are rejected",
    "Indented no-separator CFG operator-reference directive starts are rejected",
    "control-flow, implication, or equivalence exactness definitions must name",
    "conjuncts must be named concrete predicates before composition",
    "conjuncts must compose an existing concrete matches predicate directly",
    "compose named predicates before the exactness bundle composes them",
    "Parameterized exactness conjuncts must be lifted behind zero-arity",
    "Parameterized helper call checks parse expression arguments, including comparisons",
    "Compound exactness helper operands must not hide expression-argument parameterized helper calls",
    "Quantified formula exactness conjuncts must be lifted behind named",
    "Formula equality exactness conjuncts must be lifted behind named",
    "Formula equivalence exactness conjuncts must be lifted behind named",
    "Non-named exactness conjuncts are rejected even when mixed",
    "Named exactness predicates must not hide generic correctness",
    "Transitive named exactness predicate chains must not hide generic correctness",
    "pins the three top-level Byzantine CFG check surfaces",
    "top-level Byzantine CFG constants pin quorum and fault envelopes",
    "projection progress spec composes the named fairness aggregate",
    "source progress specs compose their named fairness aggregates",
    "top-level Sumeragi specs compose their named fairness aggregates",
    "temporal CFGs bind their documented behavior operators",
    "top-level Sumeragi CFG constants pin quorum and fault envelopes",
    'clean temporal progress CFGs bind Bug = "none"',
    "progress mutation CFGs bind Bug to their file suffix",
    "safety mutation CFGs bind Bug to their file suffix",
    "quoted-string mutation CFG Bug constants match their file suffix",
    "safety mutation CFGs bind INIT Init and NEXT Next",
    'clean safety CFGs bind Bug = "none"',
    "clean safety CFGs bind INIT Init and NEXT Next",
    "Transitive exactness predicate chains must not hide repeated helper conjuncts",
    "Unary-temporal exactness helper wrappers must not hide repeated helper conjuncts",
    "Unary-temporal exactness helper wrappers must not hide single-helper conjunct aliases",
    "Literal-gated exactness helper wrappers must not hide single-helper conjunct aliases",
    "Literal-gated exactness helper wrappers must not hide zero-arity helper aliases",
    "Literal-gated zero-arity helper alias checks recurse through nested identity gates",
    "Literal-gated exactness helper wrappers must not hide negated helper operands",
    "Literal-gated negated helper operand checks recurse through nested identity gates",
    "Compound exactness helper operands must not hide repeated helper conjuncts",
    "Helper conjunct repetition checks traverse unary-temporal wrappers",
    "Repeated helper same-polarity checks split top-level boolean operands before peeling temporal or negated wrappers",
    "Transitive exactness predicate chains must not hide repeated helper operands",
    "Helper operand repetition checks traverse unary-temporal wrappers",
    "Repeated helper operand checks include chained implication and equivalence operands",
    "Transitive exactness predicate chains must not hide contradictory helper operands",
    "Transitive exactness predicate chains must not hide excluded-middle helper operands",
    "Transitive exactness predicate chains must not hide complementary-equivalence helper operands",
    "Complementary-equivalence checks include chained equivalence operands",
    "Helper operand polarity checks traverse unary-temporal wrappers",
    "Helper operand polarity checks unwrap one-line `LET` helper aliases",
    "Transitive exactness predicate chains must not hide undefined helpers",
    "Quantified exactness helper formulas must not hide undefined helpers",
    "Undefined helper scans preserve quantified binding scope",
    "Undefined helper scans preserve unbounded quantified binding scope",
    "Undefined helper scans reject relation-bearing quantified binding prefixes",
    "Undefined helper scans preserve tuple-pattern quantifier domains",
    "Undefined helper scans preserve LET binding scope",
    "Undefined helper scans preserve parameterized LET operator scope",
    "Undefined helper scans preserve CHOOSE binding scope",
    "Undefined helper scans preserve LAMBDA binding scope",
    "Undefined helper scans reject relation-bearing CHOOSE/LAMBDA binding prefixes",
    "Undefined helper scans preserve standard TLA set/operator identifiers",
    "Undefined helper scans preserve ENABLED/UNCHANGED operand scope",
    "Undefined helper scans preserve CASE branch scope",
    "Undefined helper scans preserve relation operand scope",
    "Undefined helper scans preserve operator-call argument scope",
    "Undefined helper scans preserve arithmetic/set infix operand scope",
    "Undefined helper scans preserve sequence/function infix operand scope",
    "Undefined helper scans preserve explicit set literal element scope",
    "Undefined helper scans preserve unary set-operator operand scope",
    "Undefined helper scans preserve set-comprehension binding scope",
    "Undefined helper scans preserve set-comprehension outer enclosure scope",
    "Undefined helper scans preserve function-constructor binding scope",
    "Undefined helper scans preserve function-set domain and range scope",
    "Function-set scans preserve CASE domain branch arrows",
    "Function-set scans preserve record maplet CASE values",
    "Function-set scans preserve record set/update CASE values",
    "Undefined helper scans preserve record field label scope",
    "Undefined helper scans preserve record set field label scope",
    "Undefined helper scans preserve record update field label scope",
    "Undefined helper scans preserve record selector field label scope",
    "Undefined helper scans preserve comma-shared set/function binding scope",
    "Undefined helper scans preserve operator parameter scope",
    "Quantified exactness helper formulas must not be vacuous",
    "Quantified helper formulas must not restate empty-domain, singleton-domain, bound-domain, self-membership, or empty-set membership facts",
    "Quantified helper restatement checks reject pure top-level boolean compositions",
    "Quantified helper restatement checks reject identity-literal gates",
    "Quantified helper restatement checks propagate known truth values",
    "Quantified formula prefix scans preserve escaped string literal colons",
    "Quantified formula prefix scans preserve tuple literal maplet colons",
    "Quantified helper formula scans require scoped binding prefixes",
    "Quantified bound identifier scans preserve escaped string literal domains",
    "Quantified helper bound-domain checks preserve escaped string literal domains",
    "Quantified helper bound-domain checks include comma-shared bindings",
    "Quantified helper bound-domain checks skip tuple-pattern component domains",
    "Quantified helper singleton-domain checks preserve tuple literal elements",
    "Quantified helper vacuity checks include unbounded static bodies",
    "Line comment scans preserve escaped string literal comment markers",
    "Static outer wrapper scans preserve escaped string literal parentheses",
    "Semantic identifier scans ignore escaped string literal contents",
    "Top-level relation and boolean scans preserve tuple literal operators",
    "Top-level relation scans reject whole-body control/action wrappers",
    "Top-level boolean scans preserve escaped string literal operators",
    "Top-level boolean/equality detector helpers preserve tuple literal operators",
    "Top-level keyword scans preserve tuple literal keywords",
    "Top-level CASE branch scans preserve tuple literal arms and conditions",
    "Top-level keyword and CASE branch scans preserve escaped string literal delimiters",
    "Top-level CASE branch scans distinguish unary temporal boxes from arm separators",
    "Quantified exactness helper formulas must use their bound identifiers",
    "Quantified exactness helper formulas must not duplicate bound identifiers",
    "Quantified unused-bound checks include later binding groups",
    "Quantified unused-bound checks include unbounded bindings",
    "Quantified bound identifier scans include later tuple-pattern binding groups",
    "Quantified exactness helper formulas must not select predicates with control flow",
    "Quantified exactness helper formulas must not appear below top-level negation operands",
    "Quantified exactness helper formulas are checked through boolean operands",
    "Negated quantified helper checks unwrap one-line `LET` helper aliases",
    "Negated quantified helper checks split top-level boolean operands before peeling negation",
    "Quantified helper body checks unwrap one-line `LET` helper aliases",
    "Quantified helper body control-flow checks reject non-transparent `LET` bodies",
    "Existential quantified exactness helper formulas must not weaken exactness chains",
    "Transitive exactness predicate chains must not hide whole-body control-flow predicate-selection helpers",
    "Transitive exactness predicate chains must not hide nested control-flow predicate-selection helpers",
    "Nested control-flow predicate-selection checks unwrap one-line `LET` branch aliases",
    "Control-flow predicate-selection checks unwrap one-line `LET` control aliases",
    "Nested control-flow predicate-selection checks include non-branch control operators",
    "Unary-temporal exactness helper wrappers must not hide control-flow predicate selection",
    "Unary-temporal exactness LET-alias helper wrappers must name concrete model predicates",
    "Transitive exactness predicate chains must not hide whole-body raw-predicate boolean-composition helpers",
    "Raw-predicate exactness boolean-composition helper operands are checked through top-level negation",
    "Raw-predicate exactness boolean-composition helper operands are checked through stacked top-level negation",
    "Raw-predicate exactness boolean-composition helper operands are checked through unary-temporal wrappers",
    "Raw-predicate exactness boolean-composition helper operands are checked through boolean operands",
    "Transitive exactness predicate chains must not hide whole-body parameterized-call boolean-composition helpers",
    "Parameterized-call exactness boolean-composition helper operands are checked through top-level negation",
    "Parameterized-call exactness boolean-composition helper operands are checked through stacked top-level negation",
    "Parameterized-call exactness boolean-composition helper operands are checked through unary-temporal wrappers",
    "Parameterized-call exactness boolean-composition helper operands are checked through boolean operands",
    "Literal-gated parameterized-call exactness boolean-composition helper operands are checked through identity literals",
    "Unary-temporal exactness helper wrappers must not hide parameterized helper calls",
    "Transitive exactness predicate chains must not hide whole-body quantified-predicate boolean-composition helpers",
    "Quantified-predicate exactness boolean-composition helper operands are checked through top-level negation",
    "Quantified-predicate exactness boolean-composition helper operands are checked through stacked top-level negation",
    "Quantified-predicate exactness boolean-composition helper operands are checked through unary-temporal wrappers",
    "Quantified-predicate exactness boolean-composition helper operands are checked through boolean operands",
    "Literal-gated quantified-predicate exactness boolean-composition helper operands are checked through identity literals",
    "Exactness boolean-composition checks unwrap one-line `LET` helper aliases",
    "Unary-temporal exactness helper wrappers must not hide quantified formulas",
    "Static action/set/choice exactness helper wrappers must not hide quantified formulas",
    "Static action/set/choice exactness helper wrappers traverse structured operands",
    "Structured exactness helper operands must not hide quantified formulas",
    "Structured exactness helper operands must not hide control-flow predicate selection",
    "Unary-temporal quantified, parameterized-call, and control-flow checks split top-level boolean operands before peeling temporal wrappers",
    "Unary-temporal quantified checks unwrap one-line `LET` helper aliases",
    "Unary-temporal parameterized-call checks unwrap one-line `LET` helper aliases",
    "Transitive exactness predicate chains must not hide literal or alias helpers",
    "Transitive exactness predicate chains must not hide single-helper conjunct aliases",
    "Transitive exactness predicate chains must not hide self-equality helpers",
    "Transitive exactness predicate chains must not hide self-inequality helpers",
    "Unary-temporal self-equality exactness helper wrappers count as self-equality helpers",
    "Unary-temporal self-inequality exactness helper wrappers count as self-inequality helpers",
    "Constant-relation exactness helpers count as literal helpers",
    "Constant-relation helper checks unwrap one-line `LET`, unary-temporal, and negated wrappers",
    "Static and unary-temporal boolean-only exactness helper wrappers count as",
    "Static IF literal exactness helpers count as literal helpers",
    "Static temporal literal checks split top-level boolean operands before peeling temporal or negated wrappers",
    "Negated unary-temporal boolean-only helper wrappers count as literal helpers",
    "Compound boolean-only temporal helper wrappers count as literal helpers",
    "Compound exactness helper traversal includes disjunction, implication, equivalence, and negation operands",
    "Helper reference traversal unwraps one-line `LET` helper aliases",
    "Exactness vacuous-helper checks inspect static and structured operands",
    "LET helper alias unwrapping preserves static unary result wrappers",
    "LET binding scans preserve tuple literal definition bodies",
    "LET binding scans preserve escaped string literal definition bodies",
    "LET helper alias unwrapping resolves chained one-line bindings",
    "LET alias substitution respects later quantified binding groups",
    "LET alias substitution respects escaped string literal domain binding groups",
    "LET alias substitution preserves escaped string literal result bodies",
    "LET helper alias unwrapping substitutes simple chained binding references",
    "Temporal literal checks unwrap one-line `LET` helper aliases",
    "Non-named correctness-envelope conjuncts are rejected even when mixed",
    "Allowlisted temporal correctness-envelope conjuncts must be non-literal",
    "Allowlisted temporal correctness-envelope conjuncts must be non-self-equality",
    "Allowlisted temporal correctness-envelope conjuncts must be non-self-inequality",
    "Whole-body control-flow temporal side conjuncts must name",
    "Whole-body boolean-composition temporal side conjuncts must name",
    "Unary-temporal boolean composition over temporal helpers must name",
    "Unary `[]`/`<>` boolean-only temporal wrappers count as literal temporal helpers",
    "Static IF literal temporal helpers count as literal temporal helpers",
    "Transitive allowlisted temporal correctness-envelope conjunct chains must not",
    "Transitive allowlisted temporal helper chains must not hide undefined helpers",
    "Quantified temporal helper formulas must not hide undefined helpers",
    "Undefined helper scans preserve quantified binding scope",
    "Undefined helper scans preserve unbounded quantified binding scope",
    "Undefined helper scans reject relation-bearing quantified binding prefixes",
    "Undefined helper scans preserve tuple-pattern quantifier domains",
    "Undefined helper scans preserve LET binding scope",
    "Undefined helper scans preserve parameterized LET operator scope",
    "Undefined helper scans preserve CHOOSE binding scope",
    "Undefined helper scans preserve LAMBDA binding scope",
    "Undefined helper scans reject relation-bearing CHOOSE/LAMBDA binding prefixes",
    "Undefined helper scans preserve standard TLA set/operator identifiers",
    "Undefined helper scans preserve ENABLED/UNCHANGED operand scope",
    "Undefined helper scans preserve CASE branch scope",
    "Undefined helper scans preserve relation operand scope",
    "Undefined helper scans preserve operator-call argument scope",
    "Undefined helper scans preserve arithmetic/set infix operand scope",
    "Undefined helper scans preserve sequence/function infix operand scope",
    "Undefined helper scans preserve explicit set literal element scope",
    "Undefined helper scans preserve unary set-operator operand scope",
    "Undefined helper scans preserve set-comprehension binding scope",
    "Undefined helper scans preserve set-comprehension outer enclosure scope",
    "Undefined helper scans preserve function-constructor binding scope",
    "Undefined helper scans preserve function-set domain and range scope",
    "Function-set scans preserve CASE domain branch arrows",
    "Function-set scans preserve record maplet CASE values",
    "Function-set scans preserve record set/update CASE values",
    "Undefined helper scans preserve record field label scope",
    "Undefined helper scans preserve record set field label scope",
    "Undefined helper scans preserve record update field label scope",
    "Undefined helper scans preserve record selector field label scope",
    "Undefined helper scans preserve comma-shared set/function binding scope",
    "Undefined helper scans preserve operator parameter scope",
    "Quantified temporal helper formulas must not be vacuous",
    "Quantified helper formulas must not restate empty-domain, singleton-domain, bound-domain, self-membership, or empty-set membership facts",
    "Quantified helper restatement checks reject pure top-level boolean compositions",
    "Quantified helper restatement checks reject identity-literal gates",
    "Quantified helper restatement checks propagate known truth values",
    "Quantified formula prefix scans preserve escaped string literal colons",
    "Quantified formula prefix scans preserve tuple literal maplet colons",
    "Quantified helper formula scans require scoped binding prefixes",
    "Quantified bound identifier scans preserve escaped string literal domains",
    "Quantified helper bound-domain checks preserve escaped string literal domains",
    "Quantified helper bound-domain checks include comma-shared bindings",
    "Quantified helper bound-domain checks skip tuple-pattern component domains",
    "Quantified helper singleton-domain checks preserve tuple literal elements",
    "Quantified helper vacuity checks include unbounded static bodies",
    "Line comment scans preserve escaped string literal comment markers",
    "Static outer wrapper scans preserve escaped string literal parentheses",
    "Semantic identifier scans ignore escaped string literal contents",
    "Top-level relation and boolean scans preserve tuple literal operators",
    "Top-level relation scans reject whole-body control/action wrappers",
    "Top-level boolean/equality detector helpers preserve tuple literal operators",
    "Top-level keyword scans preserve tuple literal keywords",
    "Top-level CASE branch scans preserve tuple literal arms and conditions",
    "Top-level CASE branch scans distinguish unary temporal boxes from arm separators",
    "Quantified temporal helper formulas must use their bound identifiers",
    "Quantified temporal helper formulas must not duplicate bound identifiers",
    "Quantified unused-bound checks include later binding groups",
    "Quantified unused-bound checks include unbounded bindings",
    "Quantified bound identifier scans include later tuple-pattern binding groups",
    "Quantified temporal helper formulas must not select predicates with control flow",
    "Quantified temporal helper formulas must not appear below top-level negation operands",
    "Quantified temporal helper formulas are checked through boolean operands",
    "Existential quantified temporal helper formulas must not weaken allowlisted temporal chains",
    "Compound temporal helper operands must not hide undefined helpers",
    "Transitive allowlisted temporal helper chains must not hide repeated helper conjuncts",
    "Allowlisted temporal helper conjunct repetition checks use the same unary-temporal traversal",
    "Transitive allowlisted temporal helper chains must not hide repeated helper operands",
    "Transitive allowlisted temporal helper chains must not hide contradictory helper operands",
    "Transitive allowlisted temporal helper chains must not hide excluded-middle helper operands",
    "Transitive allowlisted temporal helper chains must not hide complementary-equivalence helper operands",
    "Unary-temporal temporal helper wrappers must not hide repeated helper conjuncts",
    "Unary-temporal temporal helper wrappers must not hide single-helper conjunct aliases",
    "Literal-gated temporal helper wrappers must not hide single-helper conjunct aliases",
    "Literal-gated temporal helper wrappers must not hide zero-arity helper aliases",
    "Literal-gated temporal helper wrappers must not hide negated helper operands",
    "Compound temporal helper operands must not hide repeated helper conjuncts",
    "Transitive allowlisted temporal helper chains must not hide whole-body control-flow predicate-selection helpers",
    "Transitive allowlisted temporal helper chains must not hide nested control-flow predicate-selection helpers",
    "Unary-temporal temporal helper wrappers must not hide control-flow predicate selection",
    "Static action/set/choice temporal helper wrappers must not hide quantified formulas",
    "Static action/set/choice temporal helper wrappers traverse structured operands",
    "Structured temporal helper operands must not hide quantified formulas",
    "Structured temporal helper operands must not hide control-flow predicate selection",
    "Unary-temporal temporal LET-alias helper wrappers must name concrete temporal predicates",
    "Transitive allowlisted temporal helper chains must not hide whole-body temporal-helper boolean-composition helpers",
    "Temporal-helper boolean-composition checks traverse boolean operands",
    "Unary-temporal LET-alias temporal side conjuncts must name concrete temporal predicates",
    "Transitive allowlisted temporal helper chains must not hide literal or alias helpers",
    "Transitive allowlisted temporal helper chains must not hide single-helper conjunct aliases",
    "Transitive allowlisted temporal helper chains must not hide self-equality helpers",
    "Transitive allowlisted temporal helper chains must not hide self-inequality helpers",
    "Constant-relation temporal helpers count as literal temporal helpers",
    "Compound `[]`/`<>` temporal helper bodies are traversed for helper references",
    "Parameterized temporal helper calls must be lifted behind zero-arity predicates",
    "Compound temporal helper traversal includes disjunction operands",
    "Compound temporal helper traversal includes implication operands",
    "Compound temporal helper traversal includes equivalence operands",
    "Compound temporal helper traversal includes negation operands",
    "Temporal vacuous-helper checks inspect static and structured operands",
    "Exactness and correctness-envelope conjunct references must resolve to zero-arity",
    "Transitive exactness predicate chains must also resolve through zero-arity",
    "Every top-level Sumeragi property checked by the deep/TLC-fast configs must be reachable",
    "from `SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope` through zero-arity",
    "operator references",
    "Every non-TypeInvariant top-level Sumeragi invariant checked by the deep/TLC-fast configs must be reachable",
    "from `SumeragiConsensusCoreStateMatchesEnvelope` through zero-arity",
    "The consensus-core aggregate proof roots must keep their exact direct conjunct contracts",
    "The correctness root composes `TypeInvariant`, `SumeragiConsensusCoreAlwaysMatchesExactness`,",
    "`SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope`, and `EventuallyCommit` directly",
    "The state+temporal, exactness, and fast roots keep their documented direct conjuncts",
    "Sumeragi_fast.cfg pins the fast correctness envelope",
    "Every direct conjunct of `SumeragiConsensusCoreStateMatchesEnvelope` must be checked",
    "as a top-level deep/TLC-fast `INVARIANT`",
    "`SumeragiConsensusCoreStateMatchesEnvelope` must keep the documented state direct conjunct contract",
    "Every direct conjunct of `SumeragiConsensusCoreAlwaysMatchesEndToEndSafetyEnvelope` must be checked",
    "as a top-level deep/TLC-fast `PROPERTY`",
    "Every direct conjunct of `RbcLifecycleAlwaysMatchesEndToEndEnvelope` must be checked",
    "Nested RBC lifecycle aggregate conjuncts use the same top-level `PROPERTY` coverage rule",
    "First-level RBC lifecycle aggregate conjuncts use the same top-level `PROPERTY` coverage rule",
    "RBC progress, corruption repair, chunk/ready/deliver, delivery-entry, and delivered-state roots stay decomposed",
    "Reachable aggregate temporal property roots recursively use the same top-level `PROPERTY` coverage rule",
    "Finalized certificate retention names the Byzantine commit-vote closure property directly",
    "Root coverage checks require each selected deep/TLC-fast CFG to carry every protected conjunct independently",
    "Correctness-root reachability requires the root property in every selected deep/TLC-fast CFG",
    "Correctness-root direct TypeInvariant stays a top-level `INVARIANT` in every selected deep/TLC-fast CFG",
    "Correctness-root direct temporal obligations stay top-level `PROPERTY` checks in every selected deep/TLC-fast CFG",
    "`EventuallyCommit` must keep the direct `[] (gst => <> committed)` liveness shape with exact lowercase state-variable names",
    "`CommitNeverRevoked` must keep the direct `[] (committed => [] committed)` finality-latch monotonicity shape with exact lowercase state-variable names",
    "Finality `AlwaysMatches` temporal wrappers must keep direct `[]` shapes over their matching zero-arity predicates",
    "`TimeoutTickGateNeverBypassesStalledProgress` must keep the direct `[] TimeoutTickGateMatchesStalledProgress` timeout-gate wrapper shape",
    "Pre-commit handoff `Never`/`Always` predicate wrappers must keep direct `[] Predicate` shapes over their documented zero-arity predicates",
    "`CommittedPhaseNeverLeaves` must keep the direct `[] (phase = \"Committed\" => [] (phase = \"Committed\"))` phase permanence shape",
    "Timeout-recovery action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "Pre-commit handoff action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`Committed*` action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`RbcDeliveredFinality*` action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`RbcDeliveredEvidenceNeverRegresses` and `RbcDeliveredPending*` lifecycle action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`RbcDeliveredPendingSpecStep*` action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`RbcDeliveryEntry*` action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`RbcDeliveryEntryCommitEvidenceBranch*` action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`DeliveredPendingCompleteWaitState*` action-wrapper temporal theorems must keep direct `[] [MatchingStep]_vars` shapes over their documented zero-arity step operators",
    "`PendingProtocolStepsNeverChangeGst` must keep the direct `[] [PendingProtocolStepsPreserveGst]_vars` GST-preservation action-wrapper shape",
    "`CommittedGstNeverEnablesActions` must keep the direct `[] CommittedGstDisablesEveryAction` terminal action-disable shape",
    "`CommittedStateAlwaysMatchesTerminalEnvelope` must keep the documented terminal-state direct conjunct contract",
    "`PostFinalityStateAlwaysMatchesStabilityEnvelope` must keep the documented post-finality stability direct conjunct contract",
    "`TimeoutRecoveryAlwaysMatchesViewChangeEnvelope` must keep the documented timeout-recovery direct conjunct contract",
    "`FinalityInstallationAlwaysMatchesCertifiedCommitEnvelope` must keep the documented certified-commit direct conjunct contract",
    "`PreCommitHandoffAlwaysMatchesProposalPrepareEnvelope` must keep the documented pre-commit handoff direct conjunct contract",
    "`CommitVoteHandoffAlwaysMatchesFinalityEnvelope` must keep the documented commit-vote handoff direct conjunct contract",
    "`FinalizedCertificateEvidenceAlwaysMatchesRetentionEnvelope` must keep the documented finalized-certificate retention direct conjunct contract",
    "`RbcDeliveredFinalityAlwaysMatchesCertifiedCommitEnvelope` must keep the documented RBC delivered-finality direct conjunct contract",
    "`RbcDeliveredStateAlwaysMatchesCompleteLifecycleEnvelope` must keep the documented RBC delivered-state lifecycle direct conjunct contract",
    "`RbcDeliveredPendingSpecStepAlwaysMatchesCompleteHandoffEnvelope` must keep the documented RBC delivered-pending handoff direct conjunct contract",
    "`DeliveredPendingCompleteWaitStateAlwaysMatchesNamedActionEnvelope` must keep the documented delivered-pending complete wait-state direct conjunct contract",
    "`RbcDeliveryEntryAlwaysMatchesCompleteOutcomeEnvelope` must keep the documented RBC delivery-entry outcome direct conjunct contract",
    "`RbcDeliveryEntryCommitEvidenceBranchAlwaysMatchesContinuationEnvelope` must keep the documented RBC delivery-entry continuation direct conjunct contract",
    "`RbcLifecycleAlwaysMatchesEndToEndEnvelope` must keep the documented RBC lifecycle direct conjunct contract",
    "`RbcCorruptionRepairAlwaysMatchesFaultEnvelope` must keep the documented RBC corruption-repair direct conjunct contract",
    "`RbcChunkReadyDeliverAlwaysMatchesAvailabilityEnvelope` must keep the documented RBC chunk/ready/deliver availability direct conjunct contract",
    "`RbcProgressMutationAlwaysPreservesLiveEvidenceEnvelope` must keep the documented RBC progress-mutation direct conjunct contract",
    "`RbcProgressMutationAlwaysMatchesLocalClassification` must keep the documented RBC progress local-classification direct conjunct contract",
    "`RbcStartupAndDefensiveBoundaryAlwaysMatchesEnvelope` must keep the documented RBC startup-boundary direct conjunct contract",
    "`RbcProgressStateEvidenceAlwaysMatchesEnvelope` must keep the documented RBC progress-state evidence direct conjunct contract",
    "`RbcLiveEvidenceCausalityAlwaysMatchesEnvelope` must keep the documented RBC live-evidence causality direct conjunct contract",
    "`SumeragiConsensusCoreAlwaysMatchesEndToEndSafetyEnvelope` must keep the documented end-to-end safety direct conjunct contract",
)
# Historical escape hatch for fast envelopes without direct *Exactness coverage.
# Keep empty; new entries should be justified by an explicit formal debt note.
LEGACY_FAST_ENVELOPE_WITHOUT_EXACTNESS = set()
GENERIC_CORRECTNESS_CHECKS = {"NoBugInvariant", "Safety", "SafetyFast"}
# The top-level TLC temporal property remains direct so TLC can level-analyze it.
# Keep this narrow and stale-checked; helper envelopes should use *Exactness.
TEMPORAL_CORRECTNESS_ENVELOPE_EXTRAS = {
    (
        "Sumeragi.tla",
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
    ): {
        "EventuallyCommit",
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
    },
}

COMMAND_MODE_PATTERN = r"[A-Za-z0-9_.:/-]+"
COMMAND_MODE_RE = re.compile(rf"^{COMMAND_MODE_PATTERN}$")
APALACHE_COMMAND_RE = re.compile(
    rf"\b{re.escape(APALACHE_COMMAND_PREFIX)}\s+({COMMAND_MODE_PATTERN})"
)
TLC_COMMAND_RE = re.compile(
    rf"\b{re.escape(TLC_COMMAND_PREFIX)}\s+({COMMAND_MODE_PATTERN})"
)
CONFLICT_MARKER_RE = re.compile(r"^(?:<{7}|={7}|>{7})(?:\s|$)")
CASE_LABEL_RE = re.compile(r"^  ([A-Za-z0-9_-]+(?:-\*)?)\)\s*$", re.MULTILINE)
CASE_LABEL_LINE_RE = re.compile(
    r"^  (?:[A-Za-z0-9_-]+(?:-\*)?|\*)\)\s*$"
)
ASSIGN_RE = re.compile(
    r'^\s*(spec_file|cfg_file)="\$spec_dir/([^"]+)"\s*$', re.MULTILINE
)
SHELL_ASSIGNMENT_DECLARATION_PREFIX = (
    r"(?:(?:declare|local|typeset|readonly|export)(?:\s+-[A-Za-z]+)*\s+)?"
)
MODULE_ASSIGN_RE = re.compile(r'^\s*module="([^"]+)"\s*$', re.MULTILINE)
TLC_CONSTRAINT_ASSIGN_RE = re.compile(
    r'^\s*tlc_constraint="([^"]*)"\s*$', re.MULTILINE
)
APALACHE_LENGTH_ASSIGN_RE = re.compile(
    r"^\s*apalache_length=([^\s#]+)\s*$", re.MULTILINE
)
EXPECT_FAILURE_ASSIGN_RE = re.compile(
    r"^\s*expect_failure=([01])\s*$", re.MULTILINE
)
TYPECHECK_ONLY_ASSIGN_RE = re.compile(
    r"^\s*typecheck_only=([01])\s*$", re.MULTILINE
)
RUNNER_APALACHE_VERSION_RE = re.compile(
    r'^\s*apalache_version="\$\{APALACHE_VERSION:-([0-9]+\.[0-9]+\.[0-9]+)\}"\s*$',
    re.MULTILINE,
)
INSTALLER_APALACHE_VERSION_RE = re.compile(
    r'^\s*version="\$\{1:-([0-9]+\.[0-9]+\.[0-9]+)\}"\s*$',
    re.MULTILINE,
)
INSTALL_APALACHE_COMMAND_VERSION_RE = re.compile(
    r"\bbash\s+scripts/formal/install_apalache\.sh\s+([0-9]+\.[0-9]+\.[0-9]+)\b"
)
APALACHE_TOOLCHAIN_PATH_VERSION_RE = re.compile(
    r"\btarget/apalache/toolchains/v([0-9]+\.[0-9]+\.[0-9]+)/"
)
APALACHE_DOCKER_IMAGE_VERSION_RE = re.compile(
    r"\bghcr\.io/apalache-mc/apalache:([0-9]+\.[0-9]+\.[0-9]+)\b"
)


def shell_mutation_candidate_re(*variables: str) -> re.Pattern[str]:
    """Return a regex for shell lines that can mutate the given variables."""
    names = "|".join(re.escape(variable) for variable in variables)
    return re.compile(
        rf"^\s*(?:"
        rf"{SHELL_ASSIGNMENT_DECLARATION_PREFIX}"
        rf"(?:{names})(?:\[[^]]+\])?\s*\+?\s*="
        rf"|printf\b(?=[^#\n]*\s-v\s+(?:{names})\b)"
        rf"|read\b(?=[^#\n]*\b(?:{names})\b)"
        rf"|unset\b(?=[^#\n]*\b(?:{names})\b)"
        rf"|eval\b(?=[^#\n]*\b(?:{names})(?:\[[^]]+\])?\s*\+?\s*=)"
        rf")"
    )


PROOF_INPUT_MUTATION_RE = shell_mutation_candidate_re("spec_file", "cfg_file")
EXPECT_FAILURE_MUTATION_RE = shell_mutation_candidate_re("expect_failure")
TYPECHECK_ONLY_MUTATION_RE = shell_mutation_candidate_re("typecheck_only")
TLA_MODULE_RE = re.compile(
    r"^-{4}\s+MODULE\s+([A-Za-z_][A-Za-z0-9_]*)\s+-{4}\s*$"
)
TLA_MODULE_START_RE = re.compile(r"^\s*-{4}\s+MODULE\b")
TLA_MODULE_PREFIX_START_RE = re.compile(r"^\s*-{4}\s+MODULE(?=\S)")
TLA_TERMINATOR_RE = re.compile(r"^={4}\s*$")
TLA_EQUALS_MARKER_TRAILING_RE = re.compile(r"^={4,}(?:[^=\s]|\s+\S)")
TLA_IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
TLA_ACTION_BOX_RE = re.compile(
    r"^\[\s*([A-Za-z_][A-Za-z0-9_]*)\s*\]_\s*([A-Za-z_][A-Za-z0-9_]*)$"
)
TLA_IDENTIFIER_EQUALITY_RE = re.compile(
    r"^[A-Za-z_][A-Za-z0-9_]*\s*=\s*[A-Za-z_][A-Za-z0-9_]*$"
)
TLA_IDENTIFIER_SELF_EQUALITY_RE = re.compile(
    r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\1$"
)
TLA_IDENTIFIER_SELF_INEQUALITY_RE = re.compile(
    r"^([A-Za-z_][A-Za-z0-9_]*)\s*(?:#|/=)\s*\1$"
)
TLA_ACTIONS_MATCH_QUANTIFIER_RE = re.compile(
    r"^\\A\s+([A-Za-z_][A-Za-z0-9_]*)\s+\\in\s+(?:Cases|Candidates):\s+"
    r"ImplementationActions\(\1\)\s*=\s*SpecActions\(\1\)$"
)
TLA_DIRECT_ACTIONS_MATCH_RE = re.compile(
    r"^ImplementationActions\(([A-Za-z_][A-Za-z0-9_]*)\)\s*=\s*"
    r"SpecActions\(\1\)$"
)
TLA_MATCHES_QUANTIFIER_RE = re.compile(
    r"^\\A\s+([A-Za-z_][A-Za-z0-9_]*)\s+\\in\s+"
    r"(?:AllCases|Cases|Candidates):\s+Matches\(\1\)$"
)
TLA_DIRECT_MATCHES_CALL_RE = re.compile(r"^Matches\(.+\)$")
TLA_WHOLE_BODY_QUANTIFIER_RE = re.compile(r"^\\[AE]\b")
TLA_QUANTIFIER_BINDING_RE = re.compile(r"\\[AE]\s+(.+?)\s+\\in\b", re.DOTALL)
TLA_WHOLE_BODY_CONTROL_RE = re.compile(
    r"^(IF|CASE|LET|CHOOSE|ENABLED|UNCHANGED)\b"
)
TLA_QUANTIFIED_BODY_PREDICATE_SELECTION_RE = re.compile(
    r"^(IF|CASE|LET|CHOOSE|ENABLED|UNCHANGED)\b"
)
TLA_QUANTIFIER_IDENTIFIER_TOKENS = {"A", "E"}
TLA_UNARY_SET_OPERATOR_IDENTIFIERS = {"DOMAIN", "SUBSET", "UNION"}
TLA_STATIC_INFIX_OPERATORS = (
    "\\setminus",
    "\\intersect",
    "\\union",
    "\\cdot",
    "\\div",
    "\\cup",
    "\\cap",
    "\\X",
    "\\o",
    "@@",
    ":>",
    "..",
    "\\",
    "+",
    "-",
    "*",
    "%",
    "^",
)
TLA_STANDARD_OPERATOR_IDENTIFIERS = {
    "Any",
    "Append",
    "Assert",
    "BOOLEAN",
    "BagCardinality",
    "BagIn",
    "BagOfAll",
    "Cardinality",
    "CopiesIn",
    "EmptyBag",
    "Head",
    "Int",
    "IsFiniteSet",
    "JavaTime",
    "Len",
    "Nat",
    "Permutations",
    "Print",
    "PrintT",
    "RandomElement",
    "Real",
    "SelectSeq",
    "Seq",
    "STRING",
    "SortSeq",
    "SubSeq",
    "TLCGet",
    "TLCSet",
    "Tail",
}
TLA_RESERVED_WORDS = {
    "ASSUME",
    "ASSUMPTION",
    "AXIOM",
    "CASE",
    "CHOOSE",
    "CONSTANT",
    "CONSTANTS",
    "DOMAIN",
    "ELSE",
    "ENABLED",
    "EXCEPT",
    "EXTENDS",
    "FALSE",
    "IF",
    "IN",
    "INSTANCE",
    "LAMBDA",
    "LET",
    "LOCAL",
    "MODULE",
    "OTHER",
    "SF_",
    "SUBSET",
    "THEN",
    "THEOREM",
    "TRUE",
    "UNCHANGED",
    "UNION",
    "VARIABLE",
    "VARIABLES",
    "WF_",
    "WITH",
}
TLA_BOOLEAN_LITERAL_TOKEN_RE = re.compile(r"TRUE|FALSE|/\\|\\/|~|\(|\)")
TLA_DECLARATION_LIST_RE = re.compile(
    r"^[A-Za-z_][A-Za-z0-9_]*"
    r"(?:\s*,\s*[A-Za-z_][A-Za-z0-9_]*)*,?\s*$"
)
TLA_OPERATOR_DEFINITION_BODY_RE = re.compile(
    r"^\s*(?P<local>LOCAL\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
    r"(?:\s*\((?P<params>[^()]*)\))?\s*==\s*(?P<body>.*)$"
)
TLA_OPERATOR_DEFINITION_START_RE = re.compile(
    r"^\s*(?:LOCAL\s+)?[A-Za-z_][A-Za-z0-9_]*"
)
TLA_RECURSIVE_RE = re.compile(r"^\s*RECURSIVE\s+(.+)$")
TLA_RECURSIVE_START_RE = re.compile(r"^\s*RECURSIVE\b")
TLA_RECURSIVE_ENTRY_RE = re.compile(
    r"^([A-Za-z_][A-Za-z0-9_]*)(?:\((.*)\))?$"
)
TLA_EXTENDS_RE = re.compile(r"^\s*EXTENDS\s+(.+)$")
TLA_EXTENDS_START_RE = re.compile(r"^\s*EXTENDS\b")
TLA_INSTANCE_RE = re.compile(
    r"^\s*(?P<local>LOCAL\s+)?"
    r"(?:(?P<alias>[A-Za-z_][A-Za-z0-9_]*)\s*==\s*)?"
    r"INSTANCE\s+(?P<module>[A-Za-z_][A-Za-z0-9_]*)"
    r"\s*$"
)
TLA_INSTANCE_WITH_RE = re.compile(
    r"^\s*(?P<local>LOCAL\s+)?"
    r"(?:(?:[A-Za-z_][A-Za-z0-9_]*)\s*==\s*)?"
    r"INSTANCE\s+[A-Za-z_][A-Za-z0-9_]*\s+WITH\b"
)
TLA_NAMED_INSTANCE_START_RE = re.compile(
    r"^\s*(?P<local>LOCAL\s+)?"
    r"(?P<alias>.*?)\s*==\s*INSTANCE\b"
)
TLA_INSTANCE_START_RE = re.compile(
    r"^\s*(?:LOCAL\s+)?"
    r"(?:(?:[A-Za-z_][A-Za-z0-9_]*)\s*==\s*)?"
    r"INSTANCE\b"
)
TLA_INSTANCE_BODY_RE = re.compile(r"^INSTANCE\b")
TLA_FORBIDDEN_DIRECTIVE_RE = re.compile(
    r"^(ASSUME|ASSUMPTION|AXIOM|THEOREM|PROOF|QED|SUFFICES|HAVE|TAKE|PICK|"
    r"WITNESS|OBVIOUS|OMITTED)\b"
)
TLA_ASSUMPTION_DIRECTIVE_WORDS = {
    "ASSUME",
    "ASSUMPTION",
    "AXIOM",
}
TLA_PROOF_DIRECTIVE_WORDS = {
    "HAVE",
    "OBVIOUS",
    "OMITTED",
    "PICK",
    "PROOF",
    "QED",
    "SUFFICES",
    "TAKE",
    "THEOREM",
    "WITNESS",
}
TLA_VARS_DEFINITION_RE = re.compile(r"^\s*vars\s*==\s*(.*)$")
TLA_VARS_DEFINITION_START_RE = re.compile(r"^\s*vars\b")
TLA_IDENTIFIER_SCAN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
TLA_STANDARD_MODULES = {
    "Bags",
    "FiniteSets",
    "Integers",
    "Naturals",
    "Randomization",
    "Reals",
    "RealTime",
    "Sequences",
    "TLC",
}
TLA_CONSTANT_DECLARATION_DIRECTIVES = {"CONSTANT", "CONSTANTS"}
TLA_CONSTANT_COLLECTION_STOP_DIRECTIVES = {
    "VARIABLE",
    "VARIABLES",
    "ASSUME",
    "ASSUMPTION",
    "AXIOM",
    "EXTENDS",
    "INSTANCE",
    "LOCAL",
    "RECURSIVE",
} | TLA_PROOF_DIRECTIVE_WORDS
TLA_VARIABLE_DECLARATION_DIRECTIVES = {"VARIABLE", "VARIABLES"}
TLA_VARIABLE_COLLECTION_STOP_DIRECTIVES = {
    "CONSTANT",
    "CONSTANTS",
    "ASSUME",
    "ASSUMPTION",
    "AXIOM",
    "EXTENDS",
    "INSTANCE",
    "LOCAL",
    "RECURSIVE",
} | TLA_PROOF_DIRECTIVE_WORDS


def is_tla_user_identifier(value: str) -> bool:
    """Return whether value can name a user-defined TLA symbol."""
    return (
        bool(TLA_IDENTIFIER_RE.match(value))
        and value not in TLA_RESERVED_WORDS
        and not value.startswith(("SF_", "WF_"))
    )


def is_tla_operator_name(value: str) -> bool:
    """Return whether value can name a Sumeragi TLA proof target."""
    return is_tla_user_identifier(value)


def is_tla_module_name(value: str) -> bool:
    """Return whether value can name a TLA module dependency."""
    return is_tla_user_identifier(value)


README_APALACHE_LENGTH_TABLE_HEADER = "| Mode | Length | Intended use |"
README_TABLE_SEPARATOR_RE = re.compile(
    r"^\|\s*:?-{3,}:?\s*\|\s*:?-{3,}:?\s*\|\s*:?-{3,}:?\s*\|\s*$"
)
README_APALACHE_LENGTH_TABLE_ROW_RE = re.compile(
    r"^\|\s*`([A-Za-z0-9_-]+)`\s*\|\s*([^|]*?)\s*\|\s*([^|]+?)\s*\|\s*$"
)
CFG_CONSTANT_BINDING_LINE_RE = re.compile(
    r"^([A-Za-z_][A-Za-z0-9_]*)\s*(?:=|<-)\s*(.+)$"
)
CFG_NESTED_CONSTANT_BINDING_RE = re.compile(
    r"(^|\s)([A-Za-z_][A-Za-z0-9_]*)\s*(?:=|<-)"
)
TLA_WF_VARS_RE = re.compile(r"\bWF_vars\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*\)")
CFG_CONSTANT_DIRECTIVES = {"CONSTANT", "CONSTANTS"}
CFG_CHECK_DIRECTIVES = {"INVARIANT", "INVARIANTS", "PROPERTY", "PROPERTIES"}
CFG_MISC_DIRECTIVES = {"CHECK_DEADLOCK"}
CFG_BEHAVIOR_DIRECTIVES = {"SPECIFICATION", "INIT", "NEXT"}
CFG_SINGLE_OPERATOR_DIRECTIVES = {
    "SPECIFICATION",
    "INIT",
    "NEXT",
    "CONSTRAINT",
    "INVARIANT",
    "PROPERTY",
}
CFG_MULTI_OPERATOR_DIRECTIVES = {"INVARIANTS", "PROPERTIES"}
CFG_ALLOWED_DIRECTIVES = (
    CFG_CONSTANT_DIRECTIVES
    | CFG_CHECK_DIRECTIVES
    | CFG_MISC_DIRECTIVES
    | CFG_SINGLE_OPERATOR_DIRECTIVES
    | CFG_MULTI_OPERATOR_DIRECTIVES
)
CFG_ALLOWED_DIRECTIVE_PREFIXES = tuple(
    sorted(CFG_ALLOWED_DIRECTIVES, key=len, reverse=True)
)
CFG_NON_PROOF_OPERATOR_REFERENCES = {"vars"}
TLC_SPECIFIC_MUTATION_CFG_PREFIXES = ("commit-roots-bug-",)
SUMERAGI_TOP_LEVEL_CFG_REQUIRED_BEHAVIORS = (
    (
        SUMERAGI_FAST_CFG,
        (("INIT", "Init"), ("NEXT", "Next")),
        "top-level fast coverage",
    ),
    (
        SUMERAGI_DEEP_CFG,
        (("INIT", "Init"), ("NEXT", "Next")),
        "top-level deep coverage",
    ),
    (
        SUMERAGI_TLC_FAST_CFG,
        (("SPECIFICATION", "Spec"),),
        "top-level TLC fast coverage",
    ),
)
SUMERAGI_TOP_LEVEL_CFG_REQUIRED_DEADLOCK_POLICIES = (
    (
        SUMERAGI_TLC_FAST_CFG,
        "FALSE",
        "top-level TLC fast coverage",
    ),
)
SUMERAGI_FAST_CONSTANT_VALUES = (
    ("N", "4"),
    ("F", "1"),
    ("CommitQuorum", "3"),
    ("ViewQuorum", "3"),
    ("StakeQuorum", "8"),
    ("StakePerHonestVote", "3"),
    ("StakePerByzVote", "1"),
    ("MaxView", "4"),
    ("MaxChunks", "2"),
)
SUMERAGI_DEEP_CONSTANT_VALUES = (
    ("N", "7"),
    ("F", "2"),
    ("CommitQuorum", "5"),
    ("ViewQuorum", "5"),
    ("StakeQuorum", "16"),
    ("StakePerHonestVote", "3"),
    ("StakePerByzVote", "1"),
    ("MaxView", "5"),
    ("MaxChunks", "2"),
)
SUMERAGI_TOP_LEVEL_CFG_REQUIRED_CONSTANT_VALUES = (
    (
        SUMERAGI_FAST_CFG,
        SUMERAGI_FAST_CONSTANT_VALUES,
        "top-level fast coverage",
    ),
    (
        SUMERAGI_DEEP_CFG,
        SUMERAGI_DEEP_CONSTANT_VALUES,
        "top-level deep coverage",
    ),
    (
        SUMERAGI_TLC_FAST_CFG,
        SUMERAGI_FAST_CONSTANT_VALUES,
        "top-level TLC fast coverage",
    ),
)
SUMERAGI_FAST_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("SumeragiConsensusCoreFastCorrectnessEnvelope", "INVARIANT"),
)
BYZANTINE_DELIVERED_FIRST_TOP_CFG = (
    SPEC_DIR / "Sumeragi_byzantine_delivered_first_top_fast.cfg"
)
BYZANTINE_VOTE_FIRST_TOP_CFG = (
    SPEC_DIR / "Sumeragi_byzantine_vote_first_top_fast.cfg"
)
BYZANTINE_DIRECT_TOP_CFG = SPEC_DIR / "Sumeragi_byzantine_direct_top_fast.cfg"
BYZANTINE_TOP_CFG_REQUIRED_CHECKS = (
    (
        BYZANTINE_DELIVERED_FIRST_TOP_CFG,
        (
            ("TypeInvariant", "INVARIANT"),
            ("TlcByzantineDirectCommitCorridor", "INVARIANT"),
            ("ByzantineDeliveredFirstTopCorrectnessEnvelope", "INVARIANT"),
        ),
        "Byzantine delivered-first top coverage",
    ),
    (
        BYZANTINE_VOTE_FIRST_TOP_CFG,
        (
            ("TypeInvariant", "INVARIANT"),
            ("TlcByzantineDirectCommitCorridor", "INVARIANT"),
            ("ByzantineVoteFirstTopCorrectnessEnvelope", "INVARIANT"),
        ),
        "Byzantine vote-first top coverage",
    ),
    (
        BYZANTINE_DIRECT_TOP_CFG,
        (
            ("TypeInvariant", "INVARIANT"),
            ("TlcByzantineDirectCommitCorridor", "INVARIANT"),
            ("ByzantineDirectTopCorrectnessEnvelope", "INVARIANT"),
            ("ByzantineDirectTopCoversOrderedTopCorridors", "INVARIANT"),
        ),
        "Byzantine direct top coverage",
    ),
)
BYZANTINE_TOP_CFG_REQUIRED_BEHAVIORS = (
    (
        BYZANTINE_DELIVERED_FIRST_TOP_CFG,
        (("INIT", "Init"), ("NEXT", "ByzantineDeliveredFirstCommitNext")),
        "Byzantine delivered-first top coverage",
    ),
    (
        BYZANTINE_VOTE_FIRST_TOP_CFG,
        (("INIT", "Init"), ("NEXT", "ByzantineVoteFirstCommitNext")),
        "Byzantine vote-first top coverage",
    ),
    (
        BYZANTINE_DIRECT_TOP_CFG,
        (("INIT", "Init"), ("NEXT", "ByzantineDirectCommitNext")),
        "Byzantine direct top coverage",
    ),
)
BYZANTINE_TOP_CFG_REQUIRED_BEHAVIOR_BY_NAME = {
    cfg_path.name: (required_behavior, coverage_label)
    for cfg_path, required_behavior, coverage_label in (
        BYZANTINE_TOP_CFG_REQUIRED_BEHAVIORS
    )
}
BYZANTINE_TOP_CFG_REQUIRED_CONSTANT_VALUES = (
    (
        BYZANTINE_DELIVERED_FIRST_TOP_CFG,
        SUMERAGI_FAST_CONSTANT_VALUES,
        "Byzantine delivered-first top coverage",
    ),
    (
        BYZANTINE_VOTE_FIRST_TOP_CFG,
        SUMERAGI_FAST_CONSTANT_VALUES,
        "Byzantine vote-first top coverage",
    ),
    (
        BYZANTINE_DIRECT_TOP_CFG,
        SUMERAGI_FAST_CONSTANT_VALUES,
        "Byzantine direct top coverage",
    ),
)
BYZANTINE_TOP_CFG_REQUIRED_CONSTANT_VALUES_BY_NAME = {
    cfg_path.name: (required_values, coverage_label)
    for cfg_path, required_values, coverage_label in (
        BYZANTINE_TOP_CFG_REQUIRED_CONSTANT_VALUES
    )
}
DIRECT_DELIVERED_FIRST_MUTATION_CFG_GLOB = (
    "SumeragiDirectDeliveredFirstCorridorGate_bug_*.cfg"
)
DIRECT_DELIVERED_FIRST_MUTATION_STEM_PREFIX = (
    "SumeragiDirectDeliveredFirstCorridorGate_bug_"
)
DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_CFG_GLOB = (
    "SumeragiDirectDeliveredFirstCorridorGate_progress_bug_*.cfg"
)
DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_STEM_PREFIX = (
    "SumeragiDirectDeliveredFirstCorridorGate_progress_bug_"
)
DIRECT_DELIVERED_FIRST_FAST_CFG = (
    SPEC_DIR / "SumeragiDirectDeliveredFirstCorridorGate_fast.cfg"
)
DIRECT_DELIVERED_FIRST_PROGRESS_CFG = (
    SPEC_DIR / "SumeragiDirectDeliveredFirstCorridorGate_progress.cfg"
)
DIRECT_DELIVERED_FIRST_FAST_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectDeliveredFirstCorridorExactness", "INVARIANT"),
    ("DirectDeliveredFirstCorridorCorrectnessEnvelope", "INVARIANT"),
)
DIRECT_DELIVERED_FIRST_PROGRESS_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectDeliveredFirstProgressSafetyEnvelope", "INVARIANT"),
    ("EventualDirectDeliveredFirstFinalityStack", "PROPERTY"),
)
DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectDeliveredFirstProgressSafetyEnvelope", "INVARIANT"),
    ("EventualDirectDeliveredFirstFinalityStack", "PROPERTY"),
)
DIRECT_DELIVERED_FIRST_PROGRESS_CFG_REQUIRED_BEHAVIOR = (
    ("SPECIFICATION", "DirectDeliveredFirstCorridorProgressSpec"),
)
DIRECT_VOTE_FIRST_MUTATION_CFG_GLOB = (
    "SumeragiDirectVoteFirstCorridorGate_bug_*.cfg"
)
DIRECT_VOTE_FIRST_MUTATION_STEM_PREFIX = (
    "SumeragiDirectVoteFirstCorridorGate_bug_"
)
DIRECT_VOTE_FIRST_PROGRESS_MUTATION_CFG_GLOB = (
    "SumeragiDirectVoteFirstCorridorGate_progress_bug_*.cfg"
)
DIRECT_VOTE_FIRST_PROGRESS_MUTATION_STEM_PREFIX = (
    "SumeragiDirectVoteFirstCorridorGate_progress_bug_"
)
DIRECT_VOTE_FIRST_FAST_CFG = (
    SPEC_DIR / "SumeragiDirectVoteFirstCorridorGate_fast.cfg"
)
DIRECT_VOTE_FIRST_PROGRESS_CFG = (
    SPEC_DIR / "SumeragiDirectVoteFirstCorridorGate_progress.cfg"
)
DIRECT_VOTE_FIRST_FAST_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectVoteFirstCorridorExactness", "INVARIANT"),
    ("DirectVoteFirstCorridorCorrectnessEnvelope", "INVARIANT"),
)
DIRECT_VOTE_FIRST_PROGRESS_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectVoteFirstProgressSafetyEnvelope", "INVARIANT"),
    ("EventualDirectVoteFirstFinalityStack", "PROPERTY"),
)
DIRECT_VOTE_FIRST_PROGRESS_MUTATION_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectVoteFirstProgressSafetyEnvelope", "INVARIANT"),
    ("EventualDirectVoteFirstFinalityStack", "PROPERTY"),
)
DIRECT_VOTE_FIRST_PROGRESS_CFG_REQUIRED_BEHAVIOR = (
    ("SPECIFICATION", "DirectVoteFirstCorridorProgressSpec"),
)
DIRECT_INTERLEAVING_MUTATION_CFG_GLOB = (
    "SumeragiDirectCommitInterleavingGate_bug_*.cfg"
)
DIRECT_INTERLEAVING_MUTATION_STEM_PREFIX = (
    "SumeragiDirectCommitInterleavingGate_bug_"
)
DIRECT_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB = (
    "SumeragiDirectCommitInterleavingGate_progress_bug_*.cfg"
)
DIRECT_INTERLEAVING_PROGRESS_MUTATION_STEM_PREFIX = (
    "SumeragiDirectCommitInterleavingGate_progress_bug_"
)
DIRECT_INTERLEAVING_FAST_CFG = (
    SPEC_DIR / "SumeragiDirectCommitInterleavingGate_fast.cfg"
)
DIRECT_INTERLEAVING_PROGRESS_CFG = (
    SPEC_DIR / "SumeragiDirectCommitInterleavingGate_progress.cfg"
)
DIRECT_INTERLEAVING_FAST_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectCommitInterleavingExactness", "INVARIANT"),
    ("DirectCommitInterleavingCorrectnessEnvelope", "INVARIANT"),
)
DIRECT_INTERLEAVING_PROGRESS_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectCommitProgressSafetyEnvelope", "INVARIANT"),
    ("EventualDirectCommitFinalityStack", "PROPERTY"),
)
DIRECT_INTERLEAVING_PROGRESS_MUTATION_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("DirectCommitProgressSafetyEnvelope", "INVARIANT"),
    ("EventualDirectCommitFinalityStack", "PROPERTY"),
)
DIRECT_INTERLEAVING_PROGRESS_CFG_REQUIRED_BEHAVIOR = (
    ("SPECIFICATION", "DirectCommitInterleavingProgressSpec"),
)
BYZANTINE_INTERLEAVING_MUTATION_CFG_GLOB = (
    "SumeragiByzantineCommitInterleavingGate_bug_*.cfg"
)
BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB = (
    "SumeragiByzantineCommitInterleavingGate_progress_bug_*.cfg"
)
BYZANTINE_INTERLEAVING_MUTATION_STEM_PREFIX = (
    "SumeragiByzantineCommitInterleavingGate_bug_"
)
BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_STEM_PREFIX = (
    "SumeragiByzantineCommitInterleavingGate_progress_bug_"
)
BYZANTINE_INTERLEAVING_FAST_CFG = (
    SPEC_DIR / "SumeragiByzantineCommitInterleavingGate_fast.cfg"
)
BYZANTINE_INTERLEAVING_PROGRESS_CFG = (
    SPEC_DIR / "SumeragiByzantineCommitInterleavingGate_progress.cfg"
)
BYZANTINE_INTERLEAVING_FAST_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ByzantineCommitInterleavingExactness", "INVARIANT"),
    ("ByzantineCommitInterleavingCorrectnessEnvelope", "INVARIANT"),
)
BYZANTINE_INTERLEAVING_PROGRESS_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ByzantineCommitProgressSafetyEnvelope", "INVARIANT"),
    ("EventualByzantineCommitFinalityStack", "PROPERTY"),
)
BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ByzantineCommitProgressSafetyEnvelope", "INVARIANT"),
    ("EventualByzantineCommitFinalityStack", "PROPERTY"),
)
BYZANTINE_INTERLEAVING_PROGRESS_CFG_REQUIRED_BEHAVIOR = (
    ("SPECIFICATION", "ByzantineCommitInterleavingProgressSpec"),
)
DIRECT_DELIVERED_FIRST_PROGRESS_SAFETY_ONLY_MUTATIONS = frozenset()
DIRECT_VOTE_FIRST_PROGRESS_SAFETY_ONLY_MUTATIONS = frozenset(
    {
        "commit_before_delivery",
        "commit_evidence_before_delivery",
        "phase_committed_before_delivery",
    }
)
DIRECT_INTERLEAVING_PROGRESS_SAFETY_ONLY_MUTATIONS = frozenset(
    {
        "commit_before_delivery",
        "commit_evidence_before_delivery",
        "commit_quorum_under_counted",
        "prepare_quorum_under_counted",
        "stake_not_recorded",
    }
)
SOURCE_SAFETY_MUTATION_CFG_REQUIRED_CHECKS = (
    (
        DIRECT_DELIVERED_FIRST_MUTATION_CFG_GLOB,
        DIRECT_DELIVERED_FIRST_FAST_CFG_REQUIRED_CHECKS,
        "direct delivered-first safety mutation coverage",
    ),
    (
        DIRECT_VOTE_FIRST_MUTATION_CFG_GLOB,
        DIRECT_VOTE_FIRST_FAST_CFG_REQUIRED_CHECKS,
        "direct vote-first safety mutation coverage",
    ),
    (
        DIRECT_INTERLEAVING_MUTATION_CFG_GLOB,
        DIRECT_INTERLEAVING_FAST_CFG_REQUIRED_CHECKS,
        "direct interleaving safety mutation coverage",
    ),
    (
        BYZANTINE_INTERLEAVING_MUTATION_CFG_GLOB,
        BYZANTINE_INTERLEAVING_FAST_CFG_REQUIRED_CHECKS,
        "Byzantine interleaving safety mutation coverage",
    ),
)
SAFETY_MUTATION_CFG_REQUIRED_BEHAVIOR = (
    ("INIT", "Init"),
    ("NEXT", "Next"),
)
CLEAN_SAFETY_CFG_REQUIRED_BEHAVIOR = (
    ("INIT", "Init"),
    ("NEXT", "Next"),
)
CLEAN_SAFETY_BUG_CONSTANT_VALUE = '"none"'
PROJECTION_FAST_CFG = SPEC_DIR / "SumeragiByzantineCommitProjectionGate_fast.cfg"
PROJECTION_PROGRESS_CFG = (
    SPEC_DIR / "SumeragiByzantineCommitProjectionGate_progress.cfg"
)
PROJECTION_FAST_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ProjectedByzantineDeliveredFirstTopExactness", "INVARIANT"),
    ("ProjectedByzantineDeliveredFirstTopCorrectnessEnvelope", "INVARIANT"),
    ("ProjectedByzantineVoteFirstTopExactness", "INVARIANT"),
    ("ProjectedByzantineVoteFirstTopCorrectnessEnvelope", "INVARIANT"),
    ("ProjectedByzantineDirectTopExactness", "INVARIANT"),
    ("ProjectedByzantineDirectTopCorrectnessEnvelope", "INVARIANT"),
    ("ProjectionBridgeCoversOrderedTopCorridors", "INVARIANT"),
    ("ProjectionBridgeMatchesInterleavingCore", "INVARIANT"),
    ("ProjectionBridgeMatchesInterleavingExactness", "INVARIANT"),
    ("ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope", "INVARIANT"),
)
PROJECTION_PROGRESS_CFG_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ProjectedCommitProgressSafetyEnvelope", "INVARIANT"),
    ("EventualProjectedCommitFinalityStack", "PROPERTY"),
)
PROJECTION_MUTATION_CFG_GLOB = "SumeragiByzantineCommitProjectionGate_bug_*.cfg"
PROJECTION_MUTATION_STEM_PREFIX = "SumeragiByzantineCommitProjectionGate_bug_"
PROJECTION_MUTATION_BRIDGE_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ProjectedByzantineDirectTopExactness", "INVARIANT"),
    ("ProjectedByzantineDirectTopCorrectnessEnvelope", "INVARIANT"),
    ("ProjectionBridgeCoversOrderedTopCorridors", "INVARIANT"),
    ("ProjectionBridgeMatchesInterleavingCore", "INVARIANT"),
    ("ProjectionBridgeMatchesInterleavingExactness", "INVARIANT"),
    ("ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope", "INVARIANT"),
)
PROJECTION_PROGRESS_MUTATION_CFG_GLOB = (
    "SumeragiByzantineCommitProjectionGate_progress_bug_*.cfg"
)
PROJECTION_PROGRESS_MUTATION_STEM_PREFIX = (
    "SumeragiByzantineCommitProjectionGate_progress_bug_"
)
PROJECTION_PROGRESS_MUTATION_REQUIRED_CHECKS = (
    ("TypeInvariant", "INVARIANT"),
    ("ProjectedCommitProgressSafetyEnvelope", "INVARIANT"),
    ("EventualProjectedCommitFinalityStack", "PROPERTY"),
)
PROJECTION_PROGRESS_CFG_REQUIRED_BEHAVIOR = (
    ("SPECIFICATION", "ProjectedCommitProgressSpec"),
)
CLEAN_PROGRESS_BUG_CONSTANT_VALUE = '"none"'
BYZANTINE_PROGRESS_SAFETY_ONLY_MUTATIONS = frozenset(
    {
        "byzantine_stake_over_counted",
        "byzantine_vote_over_budget",
        "commit_before_delivery",
        "commit_evidence_before_delivery",
        "honest_stake_not_recorded",
        "prepare_quorum_under_counted",
    }
)
FORMAL_FILE_SUFFIXES = {".cfg", ".tla"}
TLA_MODULE_VALIDATION_MODE_MARKER = "__SUMERAGI_FORMAL_MODE__"


@dataclass(frozen=True)
class RunnerCase:
    """A parsed mode branch from a Sumeragi formal runner."""

    label: str
    body: str
    line: int

    @property
    def is_wildcard(self) -> bool:
        return self.label.endswith("*")

    @property
    def wildcard_prefix(self) -> str:
        return self.label[:-1]


@dataclass(frozen=True)
class TlaLetBinding:
    """A parsed one-line local LET operator definition."""

    name: str
    params: frozenset[str]
    operand: str


_RUNNER_CASE_WILDCARD_LOOKUPS: dict[
    int, tuple[dict[str, RunnerCase], dict[str, RunnerCase]]
] = {}
_EXACTNESS_PARAMETERIZED_CALL_BOOLEAN_KIND_DIRECT_CACHE: dict[
    tuple[str, int, int], str | None
] = {}
_EXACTNESS_QUANTIFIED_BOOLEAN_KIND_DIRECT_CACHE: dict[
    tuple[str, int, int], str | None
] = {}
_EXACTNESS_DEFINITION_SHAPE_ERROR_TEMPLATES: dict[
    tuple[Path, str, int], tuple[str, ...]
] = {}
EXACTNESS_SHAPE_TEMPLATE_CFG = Path("__SUMERAGI_FORMAL_CFG__")
EXACTNESS_SHAPE_TEMPLATE_LINE = -1
EXACTNESS_SHAPE_TEMPLATE_RUNNER = "__SUMERAGI_FORMAL_RUNNER__"
EXACTNESS_SHAPE_TEMPLATE_REFERENCE = "__SUMERAGI_FORMAL_REFERENCE__"


@cache
def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def tla_line_without_comment(line: str) -> str:
    """Strip a TLA line comment while preserving escaped quoted strings."""

    in_string = False
    escaped = False
    index = 0
    while index < len(line):
        char = line[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if line.startswith("\\*", index):
            return line[:index]
        index += 1
    return line


def display_path(path: Path) -> Path:
    try:
        return path.relative_to(ROOT_DIR)
    except ValueError:
        return path


def indented_cfg_directive(line: str, directive: str) -> bool:
    return line[:1].isspace() and directive in CFG_ALLOWED_DIRECTIVES


def is_cfg_operator_reference_name(value: str) -> bool:
    """Return whether value can be used as a CFG proof/behavior operator."""
    return (
        is_tla_operator_name(value)
        and value not in CFG_NON_PROOF_OPERATOR_REFERENCES
    )


def command_modes(
    path: Path, command_re: re.Pattern[str] = APALACHE_COMMAND_RE
) -> list[str]:
    modes: list[str] = []
    for line in read_text(path).splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        modes.extend(match.group(1) for match in command_re.finditer(line))
    return modes


def command_shape_errors(path: Path, command_prefix: str, owner: str) -> list[str]:
    errors: list[str] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue

        start = 0
        while True:
            index = line.find(command_prefix, start)
            if index == -1:
                break
            tail = line[index + len(command_prefix) :]
            match = re.match(r"\s+(\S+)\s*$", tail)
            if match is None:
                errors.append(
                    f"{owner} {display_path(path)}:{line_number} has "
                    f"malformed command: {stripped}"
                )
            else:
                mode = match.group(1)
                if not COMMAND_MODE_RE.match(mode):
                    errors.append(
                        f"{owner} {display_path(path)}:{line_number} "
                        f"has invalid mode token {mode!r}"
                    )
            start = index + len(command_prefix)
    return errors


def conflict_marker_errors(paths: tuple[Path, ...]) -> list[str]:
    errors: list[str] = []
    for path in paths:
        for line_number, line in enumerate(read_text(path).splitlines(), 1):
            if CONFLICT_MARKER_RE.match(line):
                errors.append(
                    f"{display_path(path)}:{line_number} contains merge "
                    f"conflict marker: {line.strip()}"
                )
    return errors


def formal_artifact_paths(spec_dir: Path | None = None) -> tuple[Path, ...]:
    if spec_dir is None:
        spec_dir = SPEC_DIR
    return tuple(
        sorted(
            path
            for suffix in FORMAL_FILE_SUFFIXES
            for path in spec_dir.rglob(f"*{suffix}")
        )
    )


def documented_fast_table_modes(path: Path = README) -> list[str]:
    return [
        mode
        for mode, _length in documented_apalache_length_rows(path)
        if mode.endswith("-fast")
    ]


def apalache_length_table_body_lines(path: Path = README) -> tuple[
    list[tuple[int, str]], list[str]
]:
    lines = read_text(path).splitlines()
    header_indices = [
        index
        for index, line in enumerate(lines)
        if line.strip() == README_APALACHE_LENGTH_TABLE_HEADER
    ]
    if len(header_indices) != 1:
        return (
            [],
            [
                f"{display_path(path)}: README Apalache length table header "
                f"{README_APALACHE_LENGTH_TABLE_HEADER!r} appears "
                f"{len(header_indices)} times"
            ],
        )

    errors: list[str] = []
    header_index = header_indices[0]
    separator_index = header_index + 1
    if separator_index >= len(lines) or not README_TABLE_SEPARATOR_RE.match(
        lines[separator_index]
    ):
        errors.append(
            f"{display_path(path)}:{header_index + 2}: README Apalache "
            "length table is missing a Markdown separator row"
        )
        first_row_index = header_index + 1
    else:
        first_row_index = separator_index + 1

    body_lines: list[tuple[int, str]] = []
    for index, line in enumerate(lines[first_row_index:], start=first_row_index):
        if not line.startswith("|"):
            break
        body_lines.append((index + 1, line.rstrip()))
    return body_lines, errors


def documented_apalache_length_rows(path: Path = README) -> list[tuple[str, int]]:
    rows: list[tuple[str, int]] = []
    body_lines, _ = apalache_length_table_body_lines(path)
    for _, line in body_lines:
        match = README_APALACHE_LENGTH_TABLE_ROW_RE.match(line)
        if match is None:
            continue
        mode, length, _intended_use = match.groups()
        length = length.strip()
        if length.isdigit():
            rows.append((mode, int(length)))
    return rows


def apalache_length_table_shape_errors(path: Path = README) -> list[str]:
    body_lines, errors = apalache_length_table_body_lines(path)
    for line_number, line in body_lines:
        match = README_APALACHE_LENGTH_TABLE_ROW_RE.match(line)
        if match is None:
            errors.append(
                f"{display_path(path)}:{line_number}: malformed README "
                f"Apalache length table row: {line.strip()}"
            )
            continue
        mode, length, intended_use = match.groups()
        length = length.strip()
        if not length.isdigit():
            if not length:
                length = "<empty>"
            errors.append(
                f"{display_path(path)}:{line_number}: README Apalache "
                f"length for {mode} is not a non-negative integer: {length}"
            )
        if not intended_use.strip():
            errors.append(
                f"{display_path(path)}:{line_number}: README Apalache "
                f"length row for {mode} has an empty intended-use cell"
            )
    return errors


def runner_case_labels(path: Path) -> list[str]:
    return CASE_LABEL_RE.findall(read_text(path))


def runner_case_shape_errors(path: Path, runner_name: str) -> list[str]:
    errors: list[str] = []
    lines = read_text(path).splitlines()
    label_lines: list[tuple[int, int, str]] = []
    starts = [
        index for index, line in enumerate(lines) if line == 'case "$mode" in'
    ]
    if len(starts) != 1:
        errors.append(
            f"{runner_name} runner {display_path(path)} declares "
            f'{len(starts)} mode case blocks'
        )
        return errors

    start = starts[0]
    try:
        end = next(
            index for index, line in enumerate(lines[start + 1 :], start + 1)
            if line == "esac"
        )
    except StopIteration:
        errors.append(
            f"{runner_name} runner {display_path(path)} mode case block has no esac"
        )
        return errors

    for index, line in enumerate(lines[start + 1 : end], start + 2):
        stripped = line.strip()
        if not stripped:
            continue
        if line.startswith("  ") and not line.startswith("    "):
            if CASE_LABEL_LINE_RE.fullmatch(line) is None:
                errors.append(
                    f"{runner_name} runner {display_path(path)}:{index} "
                    f"has malformed case label: {stripped}"
                )
            else:
                label_lines.append((index - 1, index, stripped))
        elif not line.startswith("    "):
            errors.append(
                f"{runner_name} runner {display_path(path)}:{index} "
                f"has malformed case content: {stripped}"
            )
        if stripped.startswith((";;", ";&", ";;&")) and line != "    ;;":
            errors.append(
                f"{runner_name} runner {display_path(path)}:{index} "
                f"has malformed case terminator: {stripped}"
            )

    for position, (line_index, line_number, label) in enumerate(label_lines):
        next_line_index = (
            label_lines[position + 1][0]
            if position + 1 < len(label_lines)
            else end
        )
        if "    ;;" not in lines[line_index + 1 : next_line_index]:
            errors.append(
                f"{runner_name} runner {display_path(path)}:{line_number} "
                f"case label has no exact terminator: {label}"
            )
    return errors


def exact_fast_runner_modes(cases: dict[str, RunnerCase]) -> set[str]:
    return {label for label in cases if "*" not in label and label.endswith("-fast")}


def pr_tlc_cross_check_errors(
    pr_baseline_modes: set[str],
    modes_with_tlc_runner: set[str],
    readme_tlc_modes: set[str],
    apalache_only_modes: set[str] = APALACHE_ONLY_PR_MODES,
) -> list[str]:
    """Return coverage errors for PR baseline modes that need TLC parity."""
    checked_modes = pr_baseline_modes - apalache_only_modes
    errors: list[str] = []

    missing_runner_modes = sorted_unique(checked_modes - modes_with_tlc_runner)
    if missing_runner_modes:
        errors.append(
            "Sumeragi PR baseline modes without TLC runner cases "
            "(not explicitly Apalache-only):\n"
            + format_items(missing_runner_modes)
        )

    missing_readme_modes = sorted_unique(checked_modes - readme_tlc_modes)
    if missing_readme_modes:
        errors.append(
            "Sumeragi PR baseline modes without README TLC commands "
            "(not explicitly Apalache-only):\n"
            + format_items(missing_readme_modes)
        )

    stale_allowlist_modes = sorted_unique(apalache_only_modes - pr_baseline_modes)
    if stale_allowlist_modes:
        errors.append(
            "Sumeragi Apalache-only PR mode allowlist entries are stale:\n"
            + format_items(stale_allowlist_modes)
        )

    allowlisted_runner_modes = sorted_unique(
        apalache_only_modes & modes_with_tlc_runner
    )
    if allowlisted_runner_modes:
        errors.append(
            "Sumeragi Apalache-only PR modes unexpectedly have TLC runner cases:\n"
            + format_items(allowlisted_runner_modes)
        )

    allowlisted_readme_modes = sorted_unique(
        apalache_only_modes & readme_tlc_modes
    )
    if allowlisted_readme_modes:
        errors.append(
            "Sumeragi Apalache-only PR modes unexpectedly have README TLC commands:\n"
            + format_items(allowlisted_readme_modes)
        )

    return errors


def used_runner_case_labels(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
) -> set[str]:
    used: set[str] = set()
    for mode in modes:
        case = matching_case(mode, cases)
        if case is not None:
            used.add(case.label)
    return used


def unused_runner_case_labels(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
) -> list[str]:
    return sorted(set(cases) - used_runner_case_labels(modes, cases))


def runner_case_shadow_errors(
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    errors: list[str] = []
    ordered_cases = sorted(cases.values(), key=lambda case: case.line)
    for index, case in enumerate(ordered_cases):
        for prior in ordered_cases[:index]:
            if not prior.is_wildcard:
                continue
            if case.label.startswith(prior.wildcard_prefix):
                errors.append(
                    f"{runner_name} runner case {case.label!r} at line {case.line} "
                    f"is shadowed by earlier wildcard case {prior.label!r} "
                    f"at line {prior.line}"
                )
                break
    return errors


def bug_modes(modes: list[str]) -> set[str]:
    return {mode for mode in modes if "-bug-" in mode}


def parse_runner_cases(path: Path = APALACHE_RUNNER) -> dict[str, RunnerCase]:
    text = read_text(path)
    cases: dict[str, RunnerCase] = {}
    for match in CASE_LABEL_RE.finditer(text):
        label = match.group(1)
        end = text.find("\n    ;;", match.end())
        if end == -1:
            line = text.count("\n", 0, match.start()) + 1
            raise ValueError(f"runner case {label!r} at line {line} has no terminator")
        line = text.count("\n", 0, match.start()) + 1
        cases[label] = RunnerCase(label=label, body=text[match.end() : end], line=line)
    return cases


def matching_case(mode: str, cases: dict[str, RunnerCase]) -> RunnerCase | None:
    exact = cases.get(mode)
    if exact is not None:
        return exact

    wildcard_by_prefix = runner_case_wildcard_lookup(cases)
    for length in range(len(mode), -1, -1):
        case = wildcard_by_prefix.get(mode[:length])
        if case is not None:
            return case
    return None


def runner_case_wildcard_lookup(
    cases: dict[str, RunnerCase],
) -> dict[str, RunnerCase]:
    """Return wildcard runner cases keyed by their mode prefix."""

    cache_key = id(cases)
    cached = _RUNNER_CASE_WILDCARD_LOOKUPS.get(cache_key)
    if cached is not None and cached[0] is cases:
        return cached[1]

    wildcard_by_prefix = {
        case.wildcard_prefix: case for case in cases.values() if case.is_wildcard
    }
    _RUNNER_CASE_WILDCARD_LOOKUPS[cache_key] = (cases, wildcard_by_prefix)
    return wildcard_by_prefix


def resolve_spec_path(mode: str, case: RunnerCase, value: str) -> str:
    if case.is_wildcard:
        bug_name = mode[len(case.wildcard_prefix) :]
        value = value.replace("${bug_name}", bug_name)
        value = value.replace("${cfg_bug_name}", bug_name.replace("-", "_"))
    value = value.replace("$mode", mode)
    return value


def formal_file_path(
    mode: str,
    case: RunnerCase,
    variable: str,
    resolved: str,
) -> tuple[Path | None, list[str]]:
    candidate = Path(resolved)
    expected_suffix = {"spec_file": ".tla", "cfg_file": ".cfg"}.get(variable)
    if expected_suffix is not None and candidate.suffix != expected_suffix:
        return (
            None,
            [
                f"{mode}: {variable} in runner case {case.label!r} "
                f"must reference a {expected_suffix} file: {resolved}"
            ],
        )
    if candidate.name != resolved:
        return (
            None,
            [
                f"{mode}: {variable} in runner case {case.label!r} "
                f"must reference a flat Sumeragi formal file: {resolved}"
            ],
        )
    path = SPEC_DIR / candidate
    if candidate.is_absolute() or path.parent.resolve() != SPEC_DIR.resolve():
        return (
            None,
            [
                f"{mode}: {variable} in runner case {case.label!r} "
                f"escapes Sumeragi formal directory: {resolved}"
            ],
        )
    return path, []


def referenced_files(
    mode: str,
    case: RunnerCase,
    required_variables: tuple[str, ...] = ("spec_file", "cfg_file"),
) -> tuple[list[Path], list[str]]:
    assignments: dict[str, list[str]] = {}
    errors: list[str] = []
    for offset, line in enumerate(case.body.splitlines(), 1):
        if PROOF_INPUT_MUTATION_RE.match(line) and ASSIGN_RE.match(line) is None:
            line_number = case.line + offset - 1
            errors.append(
                f"{mode}: runner case {case.label!r} line {line_number} "
                f"has malformed proof-input assignment: {line.strip()}"
            )
    for variable, value in ASSIGN_RE.findall(case.body):
        assignments.setdefault(variable, []).append(value)
    files: list[Path] = []

    for variable in required_variables:
        values = assignments.get(variable, [])
        if len(values) != 1:
            errors.append(
                f"{mode}: runner case {case.label!r} at line {case.line} "
                f"assigns {variable} {len(values)} times"
            )
            continue
        value = values[0]
        resolved = resolve_spec_path(mode, case, value)
        if "$" in resolved or "{" in resolved:
            errors.append(
                f"{mode}: {variable} in runner case {case.label!r} "
                f"did not resolve statically: {resolved}"
            )
            continue
        path, path_errors = formal_file_path(mode, case, variable, resolved)
        errors.extend(path_errors)
        if path is not None:
            files.append(path)

    return files, errors


def malformed_scalar_assignment_errors(
    mode: str,
    case: RunnerCase,
    variable: str,
    assignment_re: re.Pattern[str],
    owner: str,
) -> list[str]:
    errors: list[str] = []
    candidate_re = shell_mutation_candidate_re(variable)
    for offset, line in enumerate(case.body.splitlines(), 1):
        if candidate_re.match(line) and assignment_re.match(line) is None:
            line_number = case.line + offset - 1
            errors.append(
                f"{mode}: {owner} case {case.label!r} line {line_number} "
                f"has malformed {variable} assignment: {line.strip()}"
            )
    return errors


def tlc_module_files(mode: str, case: RunnerCase) -> tuple[list[Path], list[str]]:
    errors = malformed_scalar_assignment_errors(
        mode, case, "module", MODULE_ASSIGN_RE, "TLC runner"
    )
    modules = MODULE_ASSIGN_RE.findall(case.body)
    if len(modules) != 1:
        return (
            [],
            errors
            + [
                f"{mode}: TLC runner case {case.label!r} at line {case.line} "
                f"assigns module {len(modules)} times"
            ],
        )

    module = modules[0]
    if "$" in module or "{" in module or "/" in module:
        return (
            [],
            errors
            + [
                f"{mode}: module in TLC runner case {case.label!r} "
                f"did not resolve statically: {module}"
            ],
        )
    if not TLA_IDENTIFIER_RE.match(module):
        return (
            [],
            errors
            + [
                f"{mode}: module in TLC runner case {case.label!r} "
                f"must be a TLA identifier: {module}"
            ],
        )

    return [SPEC_DIR / f"{module}.tla"], errors


def tlc_runner_constraint_errors(
    mode: str,
    case: RunnerCase,
    module_path: Path,
) -> list[str]:
    errors = malformed_scalar_assignment_errors(
        mode, case, "tlc_constraint", TLC_CONSTRAINT_ASSIGN_RE, "TLC runner"
    )
    values = TLC_CONSTRAINT_ASSIGN_RE.findall(case.body)
    if len(values) > 1:
        errors.append(
            f"{mode}: TLC runner case {case.label!r} at line {case.line} "
            f"assigns tlc_constraint {len(values)} times"
        )
        return errors
    if len(values) == 0:
        return errors

    constraint = values[0]
    if not TLA_IDENTIFIER_RE.match(constraint):
        errors.append(
            f"{mode}: tlc_constraint in TLC runner case {case.label!r} "
            f"does not name a static TLA operator: {constraint}"
        )
        return errors
    if not module_path.exists():
        return errors
    definitions = tla_operator_definitions(module_path)
    if constraint in definitions:
        signature = tla_operator_signatures(module_path).get(constraint)
        if signature is not None and signature[1] != 0:
            definition_line, arity = signature
            errors.append(
                f"{mode}: TLC runner case {case.label!r} appends CONSTRAINT "
                f"{constraint}, but {display_path(module_path)}:{definition_line} "
                f"defines it with arity {arity}; TLC runner constraints must "
                "target zero-arity operators"
            )
            return errors

        trivial_chains = tla_trivial_operator_chains(module_path)
        chain = trivial_chains.get(constraint)
        if chain is None:
            return errors

        definition_line = chain[0][1]
        value = chain[-1][2]
        if len(chain) == 1 and value in {"TRUE", "FALSE"}:
            errors.append(
                f"{mode}: TLC runner case {case.label!r} appends CONSTRAINT "
                f"{constraint}, but {display_path(module_path)}:{definition_line} "
                f"defines it as literal {value}"
            )
            return errors
        if len(chain) == 1 and value == "TypeInvariant":
            errors.append(
                f"{mode}: TLC runner case {case.label!r} appends CONSTRAINT "
                f"{constraint}, but {display_path(module_path)}:{definition_line} "
                "aliases TypeInvariant directly"
            )
            return errors

        chain_text = " -> ".join(
            f"{name}@{display_path(module_path)}:{chain_line}"
            for name, chain_line, _ in chain
        )
        if value in {"TRUE", "FALSE"}:
            terminal = f"literal {value}"
        else:
            terminal = value
        errors.append(
            f"{mode}: TLC runner case {case.label!r} appends CONSTRAINT "
            f"{constraint}, but {chain_text} resolves to {terminal}"
        )
        return errors

    errors.append(
        f"{mode}: TLC runner case {case.label!r} appends CONSTRAINT "
        f"{constraint}, but {display_path(module_path)} does not define it"
    )
    return errors


def apalache_length_value(
    mode: str,
    case: RunnerCase,
) -> tuple[int | None, list[str]]:
    errors = malformed_scalar_assignment_errors(
        mode, case, "apalache_length", APALACHE_LENGTH_ASSIGN_RE, "runner"
    )
    values = APALACHE_LENGTH_ASSIGN_RE.findall(case.body)
    if len(values) != 1:
        return (
            None,
            errors
            + [
                f"{mode}: runner case {case.label!r} at line {case.line} "
                f"assigns apalache_length {len(values)} times"
            ],
        )

    value = values[0]
    try:
        length = int(value)
    except ValueError:
        return (
            None,
            errors
            + [
                f"{mode}: apalache_length in runner case {case.label!r} "
                f"is not a non-negative integer: {value}"
            ],
        )
    if length < 0:
        return (
            None,
            errors
            + [
                f"{mode}: apalache_length in runner case {case.label!r} "
                f"is not a non-negative integer: {value}"
            ],
        )
    return length, errors


def apalache_length_errors(mode: str, case: RunnerCase) -> list[str]:
    _, errors = apalache_length_value(mode, case)
    return errors


def tla_module_header_errors(mode: str, paths: list[Path]) -> list[str]:
    errors: list[str] = []
    for path in paths:
        if path.suffix != ".tla" or not path.exists():
            continue

        headers: list[tuple[int, str]] = []
        terminators: list[int] = []
        first_nonempty_line: int | None = None
        for line_number, line in enumerate(read_text(path).splitlines(), 1):
            stripped = line.strip()
            if first_nonempty_line is None and stripped:
                first_nonempty_line = line_number
            match = TLA_MODULE_RE.match(stripped)
            if match is not None:
                headers.append((line_number, match.group(1)))
                if line != line.lstrip():
                    errors.append(
                        f"{mode}: {display_path(path)}:{line_number} "
                        "TLA MODULE declaration must be top-level"
                    )
            elif TLA_MODULE_START_RE.match(stripped):
                errors.append(
                    f"{mode}: {display_path(path)}:{line_number} malformed "
                    f"TLA MODULE declaration: {stripped}"
                )
            elif TLA_MODULE_PREFIX_START_RE.match(stripped):
                errors.append(
                    f"{mode}: {display_path(path)}:{line_number} malformed "
                    f"TLA MODULE declaration: {stripped}"
                )
            if TLA_TERMINATOR_RE.match(stripped):
                terminators.append(line_number)
                if line != line.lstrip():
                    errors.append(
                        f"{mode}: {display_path(path)}:{line_number} "
                        "TLA terminator must be top-level"
                    )
            elif TLA_EQUALS_MARKER_TRAILING_RE.match(stripped):
                errors.append(
                    f"{mode}: {display_path(path)}:{line_number} malformed "
                    f"TLA terminator: {stripped}"
                )

        relative = display_path(path)
        if not headers:
            errors.append(f"{mode}: {relative} has no TLA MODULE declaration")
            continue

        if len(headers) != 1:
            errors.append(
                f"{mode}: {relative} declares TLA MODULE {len(headers)} times"
            )

        line_number, declared = headers[0]
        if first_nonempty_line is not None and line_number != first_nonempty_line:
            errors.append(
                f"{mode}: {relative}:{line_number} declares MODULE after "
                f"content at line {first_nonempty_line}"
            )
        if not is_tla_module_name(declared):
            errors.append(
                f"{mode}: {relative}:{line_number} declares reserved TLA "
                f"MODULE name {declared}"
            )
        if declared != path.stem:
            errors.append(
                f"{mode}: {relative} declares MODULE {declared}, "
                f"expected {path.stem}"
            )
        if len(terminators) != 1:
            errors.append(
                f"{mode}: {relative} declares TLA terminator {len(terminators)} times"
            )
        elif any(
            line.strip()
            for line in read_text(path).splitlines()[terminators[0] :]
        ):
            errors.append(
                f"{mode}: {relative}:{terminators[0]} has content after "
                "TLA terminator"
            )
    return errors


def cfg_shape_errors(mode: str, paths: list[Path]) -> list[str]:
    errors: list[str] = []
    for path in paths:
        if path.suffix != ".cfg" or not path.exists():
            continue

        relative = display_path(path)
        text = read_text(path)
        if not text.strip():
            errors.append(f"{mode}: {relative} is empty")
            continue
        errors.extend(f"{mode}: {error}" for error in cfg_directive_errors(path))

        directives = {
            stripped.split()[0]
            for line in text.splitlines()
            if (stripped := tla_line_without_comment(line).strip())
            and not line[:1].isspace()
        }
        has_specification = "SPECIFICATION" in directives
        has_init = "INIT" in directives
        has_next = "NEXT" in directives
        if has_specification and (has_init or has_next):
            errors.append(
                f"{mode}: {relative} mixes SPECIFICATION with INIT/NEXT behavior"
            )
        elif not has_specification and not (has_init and has_next):
            errors.append(
                f"{mode}: {relative} must define SPECIFICATION or both INIT and NEXT"
            )

        if not (CFG_CHECK_DIRECTIVES & directives):
            errors.append(f"{mode}: {relative} has no invariant or property checks")
    return errors


@cache
def cfg_directive_errors(path: Path) -> list[str]:
    errors: list[str] = []
    collecting: str | None = None
    seen_check_deadlock_line: int | None = None

    def malformed_supported_directive_start(text: str) -> str | None:
        for allowed_directive in CFG_ALLOWED_DIRECTIVE_PREFIXES:
            if not text.startswith(allowed_directive):
                continue
            rest = text[len(allowed_directive) :]
            if rest and not rest[:1].isspace():
                return allowed_directive
            return None
        return None

    def indented_no_separator_supported_directive_start(text: str) -> str | None:
        directive = malformed_supported_directive_start(text)
        if directive is None:
            return None
        rest = text[len(directive) :]
        if rest.startswith("_"):
            return None
        return directive

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line).strip()
        if not stripped:
            collecting = None
            continue

        parts = stripped.split()
        directive = parts[0]
        malformed_directive_start = malformed_supported_directive_start(stripped)
        is_indented = line[:1].isspace()
        if indented_cfg_directive(line, directive):
            errors.append(
                f"{display_path(path)}:{line_number} indented CFG directive "
                f"{directive} must be top-level"
            )
            collecting = None
            continue
        indented_no_separator_directive = (
            indented_no_separator_supported_directive_start(stripped)
            if is_indented
            else None
        )
        if indented_no_separator_directive is not None:
            errors.append(
                f"{display_path(path)}:{line_number} indented CFG directive "
                f"{indented_no_separator_directive} must be top-level"
            )
            collecting = None
            continue
        if collecting is not None and is_indented:
            continue
        if malformed_directive_start is not None and is_indented:
            errors.append(
                f"{display_path(path)}:{line_number} indented CFG directive "
                f"{malformed_directive_start} must be top-level"
            )
            collecting = None
            continue

        if malformed_directive_start is not None:
            errors.append(
                f"{display_path(path)}:{line_number} malformed "
                f"CFG directive {malformed_directive_start}: {stripped}"
            )
            collecting = None
            continue

        if directive not in CFG_ALLOWED_DIRECTIVES:
            errors.append(
                f"{display_path(path)}:{line_number} unknown CFG directive "
                f"{directive}"
            )
            collecting = None
            continue

        if directive == "CHECK_DEADLOCK":
            if len(parts) != 2 or parts[1] not in {"TRUE", "FALSE"}:
                errors.append(
                    f"{display_path(path)}:{line_number} CHECK_DEADLOCK "
                    "must be TRUE or FALSE"
                )
            if seen_check_deadlock_line is not None:
                errors.append(
                    f"{display_path(path)}:{line_number} repeats "
                    "CHECK_DEADLOCK directive first declared at line "
                    f"{seen_check_deadlock_line}"
                )
            else:
                seen_check_deadlock_line = line_number
            collecting = None
            continue

        if (
            directive in CFG_CONSTANT_DIRECTIVES | CFG_MULTI_OPERATOR_DIRECTIVES
            and len(parts) == 1
        ):
            collecting = directive
        else:
            collecting = None

    return errors


@cache
def cfg_operator_references(path: Path) -> tuple[list[tuple[int, str, str]], list[str]]:
    references: list[tuple[int, str, str]] = []
    errors: list[str] = []
    collecting: str | None = None
    collecting_line: int | None = None
    collecting_entries = 0
    collecting_invalid = False

    def close_collecting() -> None:
        nonlocal collecting, collecting_line, collecting_entries, collecting_invalid
        if (
            collecting is not None
            and collecting_line is not None
            and collecting_entries == 0
            and not collecting_invalid
        ):
            errors.append(
                f"{display_path(path)}:{collecting_line} {collecting} block "
                "must reference at least one static TLA operator"
            )
        collecting = None
        collecting_line = None
        collecting_entries = 0
        collecting_invalid = False

    def malformed_operator_directive_start(text: str) -> str | None:
        for operator_directive in sorted(
            CFG_SINGLE_OPERATOR_DIRECTIVES | CFG_MULTI_OPERATOR_DIRECTIVES,
            key=len,
            reverse=True,
        ):
            if re.match(rf"^{re.escape(operator_directive)}\b", text):
                return operator_directive
        return None

    def no_separator_operator_directive_start(text: str) -> str | None:
        for operator_directive in sorted(
            CFG_SINGLE_OPERATOR_DIRECTIVES | CFG_MULTI_OPERATOR_DIRECTIVES,
            key=len,
            reverse=True,
        ):
            if not text.startswith(operator_directive):
                continue
            rest = text[len(operator_directive) :]
            if rest and not rest[:1].isspace():
                return operator_directive
            return None
        return None

    def indented_no_separator_operator_directive_start(text: str) -> str | None:
        directive = no_separator_operator_directive_start(text)
        if directive is None:
            return None
        rest = text[len(directive) :]
        if rest.startswith("_"):
            return None
        return directive

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line).strip()
        if not stripped:
            close_collecting()
            continue

        parts = stripped.split()
        directive = parts[0]
        if indented_cfg_directive(line, directive):
            if collecting is not None:
                collecting_invalid = True
            errors.append(
                f"{display_path(path)}:{line_number} indented CFG directive "
                f"{directive} must be top-level"
            )
            close_collecting()
            continue

        if directive in CFG_SINGLE_OPERATOR_DIRECTIVES:
            close_collecting()
            if len(parts) != 2:
                errors.append(
                    f"{display_path(path)}:{line_number} directive {directive} "
                    f"must reference exactly one operator"
                )
            elif parts[1] in CFG_NON_PROOF_OPERATOR_REFERENCES:
                errors.append(
                    f"{display_path(path)}:{line_number} directive {directive} "
                    "must reference a TLA operator other than "
                    f"{parts[1]} tuple"
                )
            elif not is_cfg_operator_reference_name(parts[1]):
                errors.append(
                    f"{display_path(path)}:{line_number} directive {directive} "
                    f"must reference a static TLA operator: {parts[1]}"
                )
            else:
                references.append((line_number, directive, parts[1]))
            continue

        if directive in CFG_MULTI_OPERATOR_DIRECTIVES:
            if len(parts) > 1:
                close_collecting()
                for operator in parts[1:]:
                    if operator in CFG_NON_PROOF_OPERATOR_REFERENCES:
                        errors.append(
                            f"{display_path(path)}:{line_number} directive "
                            f"{directive} must reference TLA operators other "
                            f"than {operator} tuple"
                        )
                    elif not is_cfg_operator_reference_name(operator):
                        errors.append(
                            f"{display_path(path)}:{line_number} directive "
                            f"{directive} must reference static TLA operators: "
                            f"{operator}"
                        )
                    else:
                        references.append((line_number, directive, operator))
            else:
                close_collecting()
                collecting = directive
                collecting_line = line_number
                collecting_entries = 0
                collecting_invalid = False
            continue

        malformed_directive = malformed_operator_directive_start(stripped)
        if malformed_directive is not None:
            if collecting is not None:
                collecting_invalid = True
            if line[:1].isspace():
                errors.append(
                    f"{display_path(path)}:{line_number} indented CFG "
                    f"directive {malformed_directive} must be top-level"
                )
            elif malformed_directive in CFG_SINGLE_OPERATOR_DIRECTIVES:
                errors.append(
                    f"{display_path(path)}:{line_number} directive "
                    f"{malformed_directive} must reference exactly one "
                    f"operator: {stripped}"
                )
            else:
                errors.append(
                    f"{display_path(path)}:{line_number} directive "
                    f"{malformed_directive} must reference static TLA "
                    f"operators: {stripped}"
                )
            close_collecting()
            continue

        no_separator_directive = no_separator_operator_directive_start(stripped)
        if no_separator_directive is not None and not line[:1].isspace():
            if collecting is not None:
                collecting_invalid = True
            if no_separator_directive in CFG_SINGLE_OPERATOR_DIRECTIVES:
                errors.append(
                    f"{display_path(path)}:{line_number} directive "
                    f"{no_separator_directive} must reference exactly one "
                    f"operator: {stripped}"
                )
            else:
                errors.append(
                    f"{display_path(path)}:{line_number} directive "
                    f"{no_separator_directive} must reference static TLA "
                    f"operators: {stripped}"
                )
            close_collecting()
            continue

        if collecting is not None and line[:1].isspace():
            no_separator_directive = indented_no_separator_operator_directive_start(
                stripped
            )
            if no_separator_directive is not None:
                collecting_invalid = True
                errors.append(
                    f"{display_path(path)}:{line_number} indented CFG "
                    f"directive {no_separator_directive} must be top-level"
                )
                close_collecting()
                continue
            if len(parts) != 1:
                collecting_invalid = True
                errors.append(
                    f"{display_path(path)}:{line_number} {collecting} "
                    "block line must reference exactly one static TLA operator"
                )
            elif parts[0] in CFG_NON_PROOF_OPERATOR_REFERENCES:
                collecting_invalid = True
                errors.append(
                    f"{display_path(path)}:{line_number} {collecting} "
                    "block line must reference a TLA operator other than "
                    f"{parts[0]} tuple"
                )
            elif not is_cfg_operator_reference_name(parts[0]):
                collecting_invalid = True
                errors.append(
                    f"{display_path(path)}:{line_number} {collecting} "
                    "block line must reference exactly one static TLA operator"
                )
            else:
                references.append((line_number, collecting, parts[0]))
                collecting_entries += 1
            continue

        close_collecting()

    close_collecting()
    return references, errors


def cfg_check_operator_names(path: Path) -> tuple[set[str], list[str]]:
    """Return proof-check operators referenced by a CFG file."""
    operator_kinds, errors = cfg_check_operator_kinds(path)
    return set(operator_kinds), errors


def cfg_check_operator_kinds(path: Path) -> tuple[dict[str, str], list[str]]:
    """Return proof-check operators and their normalized CFG check kind."""
    references, errors = cfg_operator_references(path)
    operator_kinds: dict[str, str] = {}
    for _, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        kind = (
            "INVARIANT"
            if directive in {"INVARIANT", "INVARIANTS"}
            else "PROPERTY"
        )
        operator_kinds[operator] = kind
    return operator_kinds, errors


def cfg_behavior_operator_references(
    path: Path,
) -> tuple[dict[str, list[tuple[int, str]]], list[str]]:
    """Return behavior operators referenced by a CFG file."""
    references, errors = cfg_operator_references(path)
    behavior_entries: dict[str, list[tuple[int, str]]] = {
        directive: [] for directive in CFG_BEHAVIOR_DIRECTIVES
    }
    for line_number, directive, operator in references:
        if directive in CFG_BEHAVIOR_DIRECTIVES:
            behavior_entries[directive].append((line_number, operator))
    return behavior_entries, errors


def cfg_behavior_contract_label(required_behavior: tuple[tuple[str, str], ...]) -> str:
    """Return a compact human-readable behavior contract label."""
    expected = dict(required_behavior)
    specification = expected.get("SPECIFICATION")
    init = expected.get("INIT")
    next_operator = expected.get("NEXT")
    if specification is not None:
        return f"SPECIFICATION {specification}"
    if init is not None and next_operator is not None:
        return f"INIT {init} and NEXT {next_operator}"
    return " and ".join(
        f"{directive} {operator}" for directive, operator in required_behavior
    )


def cfg_required_behavior_contract_errors(
    cfg_path: Path,
    required_behavior: tuple[tuple[str, str], ...],
    coverage_label: str,
) -> list[str]:
    """Return errors when a CFG file is not bound to the expected behavior."""

    if not cfg_path.exists():
        return [
            f"{display_path(cfg_path)} is missing required {coverage_label} behavior"
        ]

    behavior_entries, cfg_errors = cfg_behavior_operator_references(cfg_path)
    if cfg_errors:
        return [f"{display_path(cfg_path)}: {error}" for error in cfg_errors]

    required = dict(required_behavior)
    required_directives = set(required)
    expected_label = cfg_behavior_contract_label(required_behavior)
    errors: list[str] = []

    for directive in ("SPECIFICATION", "INIT", "NEXT"):
        entries = behavior_entries[directive]
        expected_operator = required.get(directive)
        if expected_operator is None:
            for line_number, operator in entries:
                errors.append(
                    f"{display_path(cfg_path)}:{line_number} binds unexpected "
                    f"{directive} {operator}; expected {expected_label} for "
                    f"{coverage_label}"
                )
            continue

        if not entries:
            errors.append(
                f"{display_path(cfg_path)} must bind {directive} "
                f"{expected_operator} for {coverage_label}"
            )
            continue
        if len(entries) > 1:
            first_line = entries[0][0]
            for line_number, operator in entries[1:]:
                errors.append(
                    f"{display_path(cfg_path)}:{line_number} repeats "
                    f"{directive} behavior binding {operator} first declared "
                    f"at line {first_line}; expected {expected_label} for "
                    f"{coverage_label}"
                )
        for line_number, operator in entries:
            if operator == expected_operator:
                continue
            errors.append(
                f"{display_path(cfg_path)}:{line_number} binds {directive} "
                f"{operator}, expected {expected_operator} for {coverage_label}"
            )

    if required_directives == {"SPECIFICATION"}:
        return errors
    if required_directives == {"INIT", "NEXT"}:
        return errors
    errors.append(
        f"{display_path(cfg_path)} has unsupported CFG behavior contract "
        f"{expected_label} for {coverage_label}"
    )
    return errors


def cfg_check_deadlock_entries(path: Path) -> tuple[list[tuple[int, str]], list[str]]:
    """Return valid top-level CHECK_DEADLOCK entries from a CFG file."""

    directive_errors = cfg_directive_errors(path)
    if directive_errors:
        return [], directive_errors

    entries: list[tuple[int, str]] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line).strip()
        if not stripped or line[:1].isspace():
            continue
        parts = stripped.split()
        if parts[0] == "CHECK_DEADLOCK":
            entries.append((line_number, parts[1]))
    return entries, []


def cfg_required_check_deadlock_contract_errors(
    cfg_path: Path,
    expected_value: str,
    coverage_label: str,
) -> list[str]:
    """Return errors when a CFG file does not pin CHECK_DEADLOCK."""

    if not cfg_path.exists():
        return [
            f"{display_path(cfg_path)} is missing required {coverage_label} "
            "deadlock policy"
        ]

    entries, cfg_errors = cfg_check_deadlock_entries(cfg_path)
    if cfg_errors:
        return [f"{display_path(cfg_path)}: {error}" for error in cfg_errors]

    if not entries:
        return [
            f"{display_path(cfg_path)} must set CHECK_DEADLOCK {expected_value} "
            f"for {coverage_label}"
        ]

    errors: list[str] = []
    for line_number, value in entries:
        if value == expected_value:
            continue
        errors.append(
            f"{display_path(cfg_path)}:{line_number} sets CHECK_DEADLOCK "
            f"{value}, expected {expected_value} for {coverage_label}"
        )
    return errors


def cfg_required_constant_value_contract_errors(
    cfg_path: Path,
    constant: str,
    expected_value: str,
    coverage_label: str,
) -> list[str]:
    """Return errors when a CFG file does not pin a constant binding value."""

    if not cfg_path.exists():
        return [
            f"{display_path(cfg_path)} is missing required {coverage_label} "
            f"constant binding {constant}"
        ]

    bindings, parse_errors = cfg_constant_binding_values(cfg_path)
    if parse_errors:
        return parse_errors

    entries = [
        (line_number, value)
        for line_number, name, value in bindings
        if name == constant
    ]
    if not entries:
        return [
            f"{display_path(cfg_path)} must bind constant {constant} = "
            f"{expected_value} for {coverage_label}"
        ]

    errors: list[str] = []
    for line_number, value in entries:
        if value == expected_value:
            continue
        errors.append(
            f"{display_path(cfg_path)}:{line_number} binds constant "
            f"{constant} = {value}, expected {expected_value} for "
            f"{coverage_label}"
        )
    return errors


def cfg_required_constant_values_contract_errors(
    cfg_path: Path,
    required_values: tuple[tuple[str, str], ...],
    coverage_label: str,
) -> list[str]:
    """Return errors when CFG constants do not match required values."""

    errors: list[str] = []
    for constant, expected_value in required_values:
        errors.extend(
            cfg_required_constant_value_contract_errors(
                cfg_path,
                constant,
                expected_value,
                coverage_label,
            )
        )
    return errors


def cfg_required_bug_suffix_constant_errors(
    cfg_path: Path,
    stem_prefix: str,
    coverage_label: str,
) -> list[str]:
    """Return errors when a mutation CFG Bug constant drifts from its suffix."""

    stem = cfg_path.stem
    if not stem.startswith(stem_prefix):
        return [
            f"{display_path(cfg_path)} must use stem prefix {stem_prefix} "
            f"for {coverage_label}"
        ]
    expected_value = f'"{stem[len(stem_prefix):]}"'
    return cfg_required_constant_value_contract_errors(
        cfg_path,
        "Bug",
        expected_value,
        coverage_label,
    )


def cfg_required_inferred_bug_suffix_constant_errors(
    cfg_path: Path,
    coverage_label: str,
) -> list[str]:
    """Return errors when a mutation CFG Bug constant does not match its suffix."""

    marker = "_bug_"
    stem = cfg_path.stem
    marker_index = stem.find(marker)
    if marker_index == -1:
        return [
            f"{display_path(cfg_path)} must include {marker} in its stem for "
            f"{coverage_label}"
        ]
    stem_prefix = stem[: marker_index + len(marker)]
    return cfg_required_bug_suffix_constant_errors(
        cfg_path,
        stem_prefix,
        coverage_label,
    )


def quoted_bug_suffix_constant_errors(
    spec_dir: Path = SPEC_DIR,
    cfg_glob: str = "*_bug_*.cfg",
) -> list[str]:
    """Return errors when quoted Bug constants drift from mutation CFG suffixes."""

    errors: list[str] = []
    for cfg_path in sorted(spec_dir.glob(cfg_glob)):
        stem = cfg_path.stem
        if "_bug_" not in stem:
            continue
        expected_value = stem.split("_bug_", 1)[1]
        bindings, parse_errors = cfg_constant_binding_values(cfg_path)
        errors.extend(parse_errors)
        if parse_errors:
            continue
        for line_number, constant, value in bindings:
            if constant != "Bug":
                continue
            if not (value.startswith('"') and value.endswith('"')):
                continue
            actual_value = value.strip('"')
            if actual_value == expected_value:
                continue
            errors.append(
                f"{display_path(cfg_path)}:{line_number} binds quoted "
                f'Bug = {value}, expected "{expected_value}" from mutation '
                "file suffix"
            )
    return errors


def top_level_cfg_behavior_errors(
    cfg_contracts: tuple[
        tuple[Path, tuple[tuple[str, str], ...], str],
        ...,
    ] = SUMERAGI_TOP_LEVEL_CFG_REQUIRED_BEHAVIORS,
) -> list[str]:
    """Return errors if root Sumeragi CFG behavior bindings drift."""

    errors: list[str] = []
    for cfg_path, required_behavior, coverage_label in cfg_contracts:
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                required_behavior,
                coverage_label,
            )
        )
    return errors


def top_level_cfg_deadlock_errors(
    cfg_contracts: tuple[
        tuple[Path, str, str],
        ...,
    ] = SUMERAGI_TOP_LEVEL_CFG_REQUIRED_DEADLOCK_POLICIES,
) -> list[str]:
    """Return errors if root Sumeragi CFG deadlock policy drifts."""

    errors: list[str] = []
    for cfg_path, expected_value, coverage_label in cfg_contracts:
        errors.extend(
            cfg_required_check_deadlock_contract_errors(
                cfg_path,
                expected_value,
                coverage_label,
            )
        )
    return errors


def top_level_cfg_constant_errors(
    cfg_contracts: tuple[
        tuple[Path, tuple[tuple[str, str], ...], str],
        ...,
    ] = SUMERAGI_TOP_LEVEL_CFG_REQUIRED_CONSTANT_VALUES,
) -> list[str]:
    """Return errors if root Sumeragi CFG model constants drift."""

    errors: list[str] = []
    for cfg_path, required_values, coverage_label in cfg_contracts:
        errors.extend(
            cfg_required_constant_values_contract_errors(
                cfg_path,
                required_values,
                coverage_label,
            )
        )
    return errors


def top_level_cfg_check_parity_errors(
    deep_cfg: Path = SUMERAGI_DEEP_CFG,
    tlc_fast_cfg: Path = SUMERAGI_TLC_FAST_CFG,
) -> list[str]:
    """Return errors if top-level Apalache and TLC CFG checks diverge."""
    deep_check_kinds, deep_errors = cfg_check_operator_kinds(deep_cfg)
    tlc_check_kinds, tlc_errors = cfg_check_operator_kinds(tlc_fast_cfg)
    if deep_errors or tlc_errors:
        return []

    errors: list[str] = []
    deep_checks = set(deep_check_kinds)
    tlc_checks = set(tlc_check_kinds)
    for operator in sorted_unique(deep_checks - tlc_checks):
        errors.append(
            f"{display_path(tlc_fast_cfg)} is missing top-level check "
            f"{operator} from {display_path(deep_cfg)}"
        )
    for operator in sorted_unique(tlc_checks - deep_checks):
        errors.append(
            f"{display_path(deep_cfg)} is missing top-level check "
            f"{operator} from {display_path(tlc_fast_cfg)}"
        )
    for operator in sorted_unique(deep_checks & tlc_checks):
        deep_kind = deep_check_kinds[operator]
        tlc_kind = tlc_check_kinds[operator]
        if deep_kind != tlc_kind:
            errors.append(
                f"top-level check {operator} is {deep_kind} in "
                f"{display_path(deep_cfg)} but {tlc_kind} in "
                f"{display_path(tlc_fast_cfg)}"
            )
    return errors


def cfg_property_root_reachability_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
    cfg_paths: tuple[Path, ...] = (SUMERAGI_DEEP_CFG, SUMERAGI_TLC_FAST_CFG),
    root_property: str = SUMERAGI_ROOT_PROPERTY,
) -> list[str]:
    """Return errors for checked properties not reachable from the root property."""

    if not module_path.exists() or any(not path.exists() for path in cfg_paths):
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    cfg_properties: dict[str, list[tuple[Path, int]]] = {}
    cfg_properties_by_path: dict[Path, set[str]] = {}
    for cfg_path in cfg_paths:
        cfg_properties_by_path[cfg_path] = set()
        references, parse_errors = cfg_operator_references(cfg_path)
        if parse_errors:
            return []
        for line_number, directive, operator in references:
            if normalized_cfg_check_directive(directive) != "PROPERTY":
                continue
            cfg_properties_by_path[cfg_path].add(operator)
            cfg_properties.setdefault(operator, []).append((cfg_path, line_number))

    errors: list[str] = []
    root_missing_cfgs = [
        cfg_path
        for cfg_path, cfg_properties_for_path in cfg_properties_by_path.items()
        if root_property not in cfg_properties_for_path
    ]
    if root_missing_cfgs:
        cfg_list = ", ".join(str(display_path(path)) for path in root_missing_cfgs)
        errors.append(
            f"{display_path(module_path)} root property {root_property} is not "
            f"checked as a top-level PROPERTY by {cfg_list}"
        )
    if root_property not in definitions:
        errors.append(
            f"{display_path(module_path)} does not define root property "
            f"{root_property}"
        )
        return errors

    root_signature = signatures.get(root_property)
    if root_signature is not None and root_signature[1] != 0:
        root_line, root_arity = root_signature
        errors.append(
            f"{display_path(module_path)}:{root_line} defines root property "
            f"{root_property} with arity {root_arity}; root property reachability "
            "requires a zero-arity operator"
        )
        return errors

    reachable: set[str] = set()
    stack = [root_property]
    while stack:
        operator = stack.pop()
        if operator in reachable:
            continue
        reachable.add(operator)
        definition = definitions.get(operator)
        if definition is None:
            continue
        _, expression = definition
        for identifier in tla_free_static_identifiers(expression):
            if identifier in reachable or identifier not in definitions:
                continue
            signature = signatures.get(identifier)
            if signature is not None and signature[1] != 0:
                continue
            stack.append(identifier)

    for operator in sorted(set(cfg_properties) - reachable):
        locations = ", ".join(
            f"{display_path(cfg_path)}:{line_number}"
            for cfg_path, line_number in cfg_properties[operator]
        )
        errors.append(
            f"{locations} checks PROPERTY {operator}, but it is not reachable "
            f"from root property {root_property} through zero-arity TLA "
            "operator references"
        )
    return errors


def cfg_state_invariant_root_reachability_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
    cfg_paths: tuple[Path, ...] = (SUMERAGI_DEEP_CFG, SUMERAGI_TLC_FAST_CFG),
    root_invariant: str = SUMERAGI_STATE_INVARIANT_ROOT,
    exempt_invariants: frozenset[str] = frozenset({"TypeInvariant"}),
) -> list[str]:
    """Return errors for checked state invariants outside the state root."""

    if not module_path.exists() or any(not path.exists() for path in cfg_paths):
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    cfg_invariants: dict[str, list[tuple[Path, int]]] = {}
    for cfg_path in cfg_paths:
        references, parse_errors = cfg_operator_references(cfg_path)
        if parse_errors:
            return []
        for line_number, directive, operator in references:
            if normalized_cfg_check_directive(directive) != "INVARIANT":
                continue
            if operator in exempt_invariants:
                continue
            cfg_invariants.setdefault(operator, []).append((cfg_path, line_number))

    if root_invariant not in definitions:
        return [
            f"{display_path(module_path)} does not define state invariant root "
            f"{root_invariant}"
        ]

    root_signature = signatures.get(root_invariant)
    if root_signature is not None and root_signature[1] != 0:
        root_line, root_arity = root_signature
        return [
            f"{display_path(module_path)}:{root_line} defines state invariant "
            f"root {root_invariant} with arity {root_arity}; state invariant "
            "reachability requires a zero-arity operator"
        ]

    reachable: set[str] = set()
    stack = [root_invariant]
    while stack:
        operator = stack.pop()
        if operator in reachable:
            continue
        reachable.add(operator)
        definition = definitions.get(operator)
        if definition is None:
            continue
        _, expression = definition
        for identifier in tla_free_static_identifiers(expression):
            if identifier in reachable or identifier not in definitions:
                continue
            signature = signatures.get(identifier)
            if signature is not None and signature[1] != 0:
                continue
            stack.append(identifier)

    errors: list[str] = []
    for operator in sorted(set(cfg_invariants) - reachable):
        locations = ", ".join(
            f"{display_path(cfg_path)}:{line_number}"
            for cfg_path, line_number in cfg_invariants[operator]
        )
        errors.append(
            f"{locations} checks INVARIANT {operator}, but it is not reachable "
            f"from state invariant root {root_invariant} through zero-arity TLA "
            "operator references"
        )
    return errors


def state_invariant_root_cfg_coverage_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
    cfg_paths: tuple[Path, ...] = (SUMERAGI_DEEP_CFG, SUMERAGI_TLC_FAST_CFG),
    root_invariant: str = SUMERAGI_STATE_INVARIANT_ROOT,
    exempt_invariants: frozenset[str] = frozenset({"TypeInvariant"}),
) -> list[str]:
    """Return errors for state-root conjuncts missing top-level CFG checks."""

    if not module_path.exists() or any(not path.exists() for path in cfg_paths):
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    cfg_invariants_by_path: dict[Path, set[str]] = {}
    for cfg_path in cfg_paths:
        cfg_invariants_by_path[cfg_path] = set()
        references, parse_errors = cfg_operator_references(cfg_path)
        if parse_errors:
            return []
        for _, directive, operator in references:
            if normalized_cfg_check_directive(directive) != "INVARIANT":
                continue
            if operator in exempt_invariants:
                continue
            cfg_invariants_by_path[cfg_path].add(operator)

    definition = definitions.get(root_invariant)
    if definition is None:
        return [
            f"{display_path(module_path)} does not define state invariant root "
            f"{root_invariant}"
        ]

    root_signature = signatures.get(root_invariant)
    if root_signature is not None and root_signature[1] != 0:
        root_line, root_arity = root_signature
        return [
            f"{display_path(module_path)}:{root_line} defines state invariant "
            f"root {root_invariant} with arity {root_arity}; state invariant "
            "CFG coverage requires a zero-arity operator"
        ]

    definition_line, body = definition
    errors: list[str] = []
    for conjunct in tla_top_level_conjuncts(body):
        compact_conjunct = " ".join(
            strip_static_outer_parentheses(conjunct).split()
        )
        if (
            TLA_IDENTIFIER_RE.fullmatch(compact_conjunct)
            and is_tla_user_identifier(compact_conjunct)
        ):
            continue
        errors.append(
            f"{display_path(module_path)}:{definition_line} defines "
            f"{root_invariant}, but contains direct non-named state invariant "
            f"conjunct {compact_conjunct}; compose named zero-arity state "
            "predicates directly"
        )

    root_conjuncts = tuple(tla_zero_arity_conjunct_references(body))
    repeated_conjuncts = duplicate_values(root_conjuncts)
    if repeated_conjuncts:
        errors.append(
            f"{display_path(module_path)}:{definition_line} defines "
            f"{root_invariant}, but repeats direct state invariant conjunct(s) "
            f"{', '.join(repeated_conjuncts)}; each state obligation must be "
            "counted once"
        )

    nonzero_arity_conjuncts = nonzero_arity_conjunct_references(
        body,
        signatures,
    )
    if nonzero_arity_conjuncts:
        errors.append(
            f"{display_path(module_path)}:{definition_line} defines "
            f"{root_invariant}, but contains non-zero-arity state invariant "
            "conjunct "
            f"{format_nonzero_arity_references(nonzero_arity_conjuncts, module_path)}; "
            "state-root conjuncts must compose zero-arity predicates directly"
        )

    illegal_exempt_conjuncts = [
        conjunct for conjunct in root_conjuncts if conjunct in exempt_invariants
    ]
    if illegal_exempt_conjuncts:
        errors.append(
            f"{display_path(module_path)}:{definition_line} defines "
            f"{root_invariant}, but includes exempt invariant conjunct(s) "
            f"{', '.join(illegal_exempt_conjuncts)}; keep type checks outside "
            "the non-type state root"
        )

    for cfg_path, cfg_invariants in cfg_invariants_by_path.items():
        missing_cfg_checks = [
            conjunct
            for conjunct in root_conjuncts
            if conjunct not in exempt_invariants and conjunct not in cfg_invariants
        ]
        if missing_cfg_checks:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{root_invariant}, but direct state invariant conjunct(s) "
                f"{', '.join(missing_cfg_checks)} are not checked as top-level "
                f"INVARIANT entries by {display_path(cfg_path)}"
            )
    return errors


def temporal_property_root_cfg_coverage_closure(
    definitions: dict[str, tuple[int, str]],
    cfg_properties: set[str],
    root_properties: tuple[str, ...],
) -> tuple[str, ...]:
    """Return aggregate property roots reachable through checked properties."""

    closure: list[str] = []
    seen: set[str] = set()
    queue = list(root_properties)
    while queue:
        root_property = queue.pop(0)
        if root_property in seen:
            continue
        seen.add(root_property)
        closure.append(root_property)

        definition = definitions.get(root_property)
        if definition is None:
            continue
        _, body = definition
        for child_property in tla_zero_arity_conjunct_references(body):
            if child_property in seen or child_property not in cfg_properties:
                continue
            child_definition = definitions.get(child_property)
            if child_definition is None:
                continue
            _, child_body = child_definition
            if len(tla_top_level_conjuncts(child_body)) > 1:
                queue.append(child_property)

    return tuple(closure)


def temporal_property_root_cfg_coverage_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
    cfg_paths: tuple[Path, ...] = (SUMERAGI_DEEP_CFG, SUMERAGI_TLC_FAST_CFG),
    root_properties: tuple[
        str, ...
    ] = SUMERAGI_TEMPORAL_PROPERTY_ROOTS_REQUIRING_CFG_COVERAGE,
) -> list[str]:
    """Return errors for temporal root conjuncts missing top-level CFG checks."""

    if not module_path.exists() or any(not path.exists() for path in cfg_paths):
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    cfg_properties_by_path: dict[Path, set[str]] = {}
    for cfg_path in cfg_paths:
        cfg_properties_by_path[cfg_path] = set()
        references, parse_errors = cfg_operator_references(cfg_path)
        if parse_errors:
            return []
        for _, directive, operator in references:
            if normalized_cfg_check_directive(directive) != "PROPERTY":
                continue
            cfg_properties_by_path[cfg_path].add(operator)
    cfg_properties = set().union(*cfg_properties_by_path.values())

    root_properties = temporal_property_root_cfg_coverage_closure(
        definitions,
        cfg_properties,
        root_properties,
    )

    errors: list[str] = []
    for root_property in root_properties:
        definition = definitions.get(root_property)
        if definition is None:
            errors.append(
                f"{display_path(module_path)} does not define temporal "
                f"property root {root_property}"
            )
            continue

        root_signature = signatures.get(root_property)
        if root_signature is not None and root_signature[1] != 0:
            root_line, root_arity = root_signature
            errors.append(
                f"{display_path(module_path)}:{root_line} defines temporal "
                f"property root {root_property} with arity {root_arity}; "
                "temporal property CFG coverage requires a zero-arity operator"
            )
            continue

        definition_line, body = definition
        root_missing_cfgs = [
            cfg_path
            for cfg_path, cfg_properties_for_path in cfg_properties_by_path.items()
            if root_property not in cfg_properties_for_path
        ]
        if root_missing_cfgs:
            cfg_list = ", ".join(
                str(display_path(cfg_path)) for cfg_path in root_missing_cfgs
            )
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"temporal property root {root_property}, but the root is not "
                f"checked as a top-level PROPERTY by {cfg_list}"
            )

        for conjunct in tla_top_level_conjuncts(body):
            compact_conjunct = " ".join(
                strip_static_outer_parentheses(conjunct).split()
            )
            if (
                TLA_IDENTIFIER_RE.fullmatch(compact_conjunct)
                and is_tla_user_identifier(compact_conjunct)
            ):
                continue
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{root_property}, but contains direct non-named temporal "
                f"property conjunct {compact_conjunct}; compose named "
                "zero-arity temporal predicates directly"
            )

        root_conjuncts = tuple(tla_zero_arity_conjunct_references(body))
        repeated_conjuncts = duplicate_values(root_conjuncts)
        if repeated_conjuncts:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{root_property}, but repeats direct temporal property "
                f"conjunct(s) {', '.join(repeated_conjuncts)}; each temporal "
                "obligation must be counted once"
            )

        nonzero_arity_conjuncts = nonzero_arity_conjunct_references(
            body,
            signatures,
        )
        if nonzero_arity_conjuncts:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{root_property}, but contains non-zero-arity temporal "
                "property conjunct "
                f"{format_nonzero_arity_references(nonzero_arity_conjuncts, module_path)}; "
                "temporal-root conjuncts must compose zero-arity predicates "
                "directly"
            )

        for cfg_path, cfg_properties_for_path in cfg_properties_by_path.items():
            missing_cfg_checks = [
                conjunct
                for conjunct in root_conjuncts
                if conjunct not in cfg_properties_for_path
            ]
            if missing_cfg_checks:
                errors.append(
                    f"{display_path(module_path)}:{definition_line} defines "
                    f"{root_property}, but direct temporal property conjunct(s) "
                    f"{', '.join(missing_cfg_checks)} are not checked as "
                    "top-level PROPERTY entries by "
                    f"{display_path(cfg_path)}"
                )
    return errors


def consensus_core_root_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
    contracts: dict[str, tuple[str, ...]] = SUMERAGI_CONSENSUS_CORE_ROOT_CONJUNCT_CONTRACTS,
    *,
    root_kind: str = "consensus-core proof root",
    zero_arity_requirement: str = (
        "aggregate proof roots must be zero-arity operators"
    ),
    duplicate_requirement: str = (
        "each consensus-core root obligation must be counted once"
    ),
    unexpected_requirement: str = (
        "keep consensus-core aggregate proof roots on the documented conjunct "
        "contract"
    ),
) -> list[str]:
    """Return errors for aggregate proof roots with drifted conjuncts."""

    if not module_path.exists():
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    errors: list[str] = []
    for operator, expected_conjuncts in contracts.items():
        definition = definitions.get(operator)
        if definition is None:
            errors.append(
                f"{display_path(module_path)} does not define "
                f"{root_kind} {operator}"
            )
            continue

        signature = signatures.get(operator)
        if signature is not None and signature[1] != 0:
            root_line, root_arity = signature
            errors.append(
                f"{display_path(module_path)}:{root_line} defines "
                f"{root_kind} {operator} with arity {root_arity}; "
                f"{zero_arity_requirement}"
            )
            continue

        definition_line, body = definition
        direct_conjuncts = tuple(tla_zero_arity_conjunct_references(body))
        direct_conjunct_set = set(direct_conjuncts)
        expected_conjunct_set = set(expected_conjuncts)
        repeated_conjuncts = duplicate_values(direct_conjuncts)
        if repeated_conjuncts:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{operator}, but repeats direct conjunct(s) "
                f"{', '.join(repeated_conjuncts)}; {duplicate_requirement}"
            )

        missing_conjuncts = [
            conjunct
            for conjunct in expected_conjuncts
            if conjunct not in direct_conjunct_set
        ]
        if missing_conjuncts:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{operator}, but is missing required direct conjunct(s) "
                f"{', '.join(missing_conjuncts)}"
            )

        unexpected_conjuncts = [
            conjunct
            for conjunct in direct_conjuncts
            if conjunct not in expected_conjunct_set
        ]
        if unexpected_conjuncts:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{operator}, but contains unexpected direct conjunct(s) "
                f"{', '.join(unexpected_conjuncts)}; {unexpected_requirement}"
            )
    return errors


def state_matches_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for Sumeragi state-safety aggregate invariant drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_STATE_MATCHES_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="state matches envelope",
        zero_arity_requirement=(
            "state matches envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each state-safety obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep state-safety aggregate proof envelope on the documented "
            "conjunct contract"
        ),
    )


def committed_terminal_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for committed terminal-state aggregate proof drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_COMMITTED_TERMINAL_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="committed terminal-state envelope",
        zero_arity_requirement=(
            "committed terminal-state envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each committed terminal-state obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep committed terminal-state aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def end_to_end_safety_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for the Sumeragi end-to-end safety aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_END_TO_END_SAFETY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="end-to-end safety envelope",
        zero_arity_requirement=(
            "end-to-end safety envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each end-to-end safety obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep end-to-end safety aggregate proof envelope on the documented "
            "conjunct contract"
        ),
    )


def post_finality_stability_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for post-finality stability aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_POST_FINALITY_STABILITY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="post-finality stability envelope",
        zero_arity_requirement=(
            "post-finality stability envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each post-finality stability obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep post-finality stability aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def timeout_recovery_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for timeout-recovery aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_TIMEOUT_RECOVERY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="timeout-recovery envelope",
        zero_arity_requirement=(
            "timeout-recovery envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each timeout-recovery obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep timeout-recovery aggregate proof envelope on the documented "
            "conjunct contract"
        ),
    )


def finality_installation_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for certified-commit installation aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_FINALITY_INSTALLATION_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="certified-commit installation envelope",
        zero_arity_requirement=(
            "certified-commit installation envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each certified-commit installation obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep certified-commit installation aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def pre_commit_handoff_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for pre-commit proposal/prepare handoff aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_PRE_COMMIT_HANDOFF_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="pre-commit handoff envelope",
        zero_arity_requirement=(
            "pre-commit handoff envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each pre-commit handoff obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep pre-commit handoff aggregate proof envelope on the documented "
            "conjunct contract"
        ),
    )


def commit_vote_handoff_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for commit-vote/finality handoff aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_COMMIT_VOTE_HANDOFF_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="commit-vote handoff envelope",
        zero_arity_requirement=(
            "commit-vote handoff envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each commit-vote handoff obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep commit-vote handoff aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def finalized_certificate_retention_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for finalized certificate/evidence retention drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_FINALIZED_CERTIFICATE_RETENTION_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="finalized certificate retention envelope",
        zero_arity_requirement=(
            "finalized certificate retention envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each finalized certificate retention obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep finalized certificate retention aggregate proof envelope on "
            "the documented conjunct contract"
        ),
    )


def rbc_delivered_finality_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC delivered-finality certified-commit drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_DELIVERED_FINALITY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC delivered-finality envelope",
        zero_arity_requirement=(
            "RBC delivered-finality envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC delivered-finality obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC delivered-finality aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_delivered_state_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC delivered-state lifecycle aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_DELIVERED_STATE_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC delivered-state lifecycle envelope",
        zero_arity_requirement=(
            "RBC delivered-state lifecycle envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each RBC delivered-state lifecycle obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC delivered-state lifecycle aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_delivered_pending_handoff_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC delivered-pending handoff aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_DELIVERED_PENDING_HANDOFF_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC delivered-pending handoff envelope",
        zero_arity_requirement=(
            "RBC delivered-pending handoff envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC delivered-pending handoff obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC delivered-pending handoff aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def delivered_pending_complete_wait_state_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for delivered-pending complete wait-state aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_DELIVERED_PENDING_COMPLETE_WAIT_STATE_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="delivered-pending complete wait-state envelope",
        zero_arity_requirement=(
            "delivered-pending complete wait-state envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each delivered-pending complete wait-state obligation must be "
            "counted once"
        ),
        unexpected_requirement=(
            "keep delivered-pending complete wait-state aggregate proof envelope "
            "on the documented conjunct contract"
        ),
    )


def rbc_delivery_entry_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC delivery-entry complete outcome drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_DELIVERY_ENTRY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC delivery-entry outcome envelope",
        zero_arity_requirement=(
            "RBC delivery-entry outcome envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC delivery-entry outcome obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC delivery-entry outcome aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_delivery_entry_continuation_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC delivery-entry commit-evidence continuation drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_DELIVERY_ENTRY_CONTINUATION_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC delivery-entry continuation envelope",
        zero_arity_requirement=(
            "RBC delivery-entry continuation envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each RBC delivery-entry continuation obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC delivery-entry continuation aggregate proof envelope on "
            "the documented conjunct contract"
        ),
    )


def rbc_lifecycle_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC lifecycle end-to-end aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_LIFECYCLE_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC lifecycle end-to-end envelope",
        zero_arity_requirement=(
            "RBC lifecycle end-to-end envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC lifecycle end-to-end obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC lifecycle aggregate proof envelope on the documented "
            "conjunct contract"
        ),
    )


def rbc_corruption_repair_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC corruption-repair aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_CORRUPTION_REPAIR_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC corruption-repair envelope",
        zero_arity_requirement=(
            "RBC corruption-repair envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC corruption-repair obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC corruption-repair aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_chunk_ready_deliver_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC chunk/ready/deliver availability aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_CHUNK_READY_DELIVER_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC chunk/ready/deliver availability envelope",
        zero_arity_requirement=(
            "RBC chunk/ready/deliver availability envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each RBC chunk/ready/deliver availability obligation must be "
            "counted once"
        ),
        unexpected_requirement=(
            "keep RBC chunk/ready/deliver availability aggregate proof envelope "
            "on the documented conjunct contract"
        ),
    )


def rbc_progress_mutation_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC progress-mutation aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_PROGRESS_MUTATION_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC progress-mutation envelope",
        zero_arity_requirement=(
            "RBC progress-mutation envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC progress-mutation obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC progress-mutation aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_progress_local_classification_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC progress local-classification aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_PROGRESS_LOCAL_CLASSIFICATION_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC progress local-classification envelope",
        zero_arity_requirement=(
            "RBC progress local-classification envelopes must be zero-arity "
            "operators"
        ),
        duplicate_requirement=(
            "each RBC progress local-classification obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC progress local-classification aggregate proof envelope on "
            "the documented conjunct contract"
        ),
    )


def rbc_startup_boundary_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC startup/defensive boundary aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_STARTUP_BOUNDARY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC startup-boundary envelope",
        zero_arity_requirement=(
            "RBC startup-boundary envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC startup-boundary obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC startup-boundary aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_progress_state_evidence_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC progress-state evidence aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_PROGRESS_STATE_EVIDENCE_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC progress-state evidence envelope",
        zero_arity_requirement=(
            "RBC progress-state evidence envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC progress-state evidence obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC progress-state evidence aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def rbc_live_evidence_causality_envelope_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for RBC live-evidence causality aggregate drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_RBC_LIVE_EVIDENCE_CAUSALITY_ENVELOPE_CONJUNCT_CONTRACTS,
        root_kind="RBC live-evidence causality envelope",
        zero_arity_requirement=(
            "RBC live-evidence causality envelopes must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each RBC live-evidence causality obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep RBC live-evidence causality aggregate proof envelope on the "
            "documented conjunct contract"
        ),
    )


def implication_antecedent_contract_errors(
    module_path: Path,
    contracts: dict[str, str],
) -> list[str]:
    """Return errors for proof operators that must keep a top-level implication."""

    if not module_path.exists():
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    errors: list[str] = []
    for operator, expected_antecedent in contracts.items():
        definition = definitions.get(operator)
        if definition is None:
            continue
        definition_line, body = definition
        operands = tla_top_level_implication_operands(body)
        if len(operands) != 2:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{operator}, but it must keep a top-level implication guarded "
                f"by {expected_antecedent}"
            )
            continue
        antecedent = " ".join(operands[0].split())
        if antecedent != expected_antecedent:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{operator}, but its implication antecedent is {antecedent}; "
                f"expected {expected_antecedent}"
            )
    return errors


def byzantine_top_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for top-level Byzantine corridor aggregate proof drift."""

    errors = consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_BYZANTINE_TOP_CONJUNCT_CONTRACTS,
        root_kind="top-level Byzantine direct-commit aggregate",
        zero_arity_requirement=(
            "top-level Byzantine direct-commit aggregate operators must be "
            "zero-arity operators"
        ),
        duplicate_requirement=(
            "each top-level Byzantine direct-commit obligation must be counted "
            "once"
        ),
        unexpected_requirement=(
            "keep top-level Byzantine direct-commit aggregate proof operators "
            "on the documented conjunct contract"
        ),
    )
    errors.extend(
        implication_antecedent_contract_errors(
            module_path,
            SUMERAGI_BYZANTINE_TOP_IMPLICATION_CONTRACTS,
        )
    )
    return errors


def projected_byzantine_top_conjunct(conjunct: str) -> str:
    """Return the projected counterpart of a top-level Byzantine obligation."""

    return SUMERAGI_TOP_TO_PROJECTION_LITERAL_CONJUNCTS.get(
        conjunct,
        f"Projected{conjunct}",
    )


def byzantine_top_projection_contract_alignment_errors(
    top_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_BYZANTINE_TOP_CONJUNCT_CONTRACTS,
    projection_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_PROJECTION_GATE_CONJUNCT_CONTRACTS,
    operator_map: dict[
        str,
        str,
    ] = SUMERAGI_BYZANTINE_TOP_TO_PROJECTION_OPERATOR_CONTRACTS,
    top_implication_contracts: dict[
        str,
        str,
    ] = SUMERAGI_BYZANTINE_TOP_IMPLICATION_CONTRACTS,
    projection_implication_contracts: dict[
        str,
        str,
    ] = SUMERAGI_PROJECTION_GATE_IMPLICATION_CONTRACTS,
) -> list[str]:
    """Return errors if projection top contracts drift from central top contracts."""

    errors: list[str] = []
    for top_operator, projected_operator in operator_map.items():
        top_conjuncts = top_contracts.get(top_operator)
        if top_conjuncts is None:
            errors.append(
                "Byzantine top/projection alignment references missing top "
                f"contract {top_operator}"
            )
            continue
        projected_conjuncts = projection_contracts.get(projected_operator)
        if projected_conjuncts is None:
            errors.append(
                "Byzantine top/projection alignment references missing "
                f"projection contract {projected_operator}"
            )
            continue

        expected_projected_conjuncts = tuple(
            projected_byzantine_top_conjunct(conjunct)
            for conjunct in top_conjuncts
        )
        expected_projected_set = set(expected_projected_conjuncts)
        projected_set = set(projected_conjuncts)
        missing_conjuncts = [
            conjunct
            for conjunct in expected_projected_conjuncts
            if conjunct not in projected_set
        ]
        if missing_conjuncts:
            errors.append(
                f"{projected_operator} must mirror {top_operator} projected "
                "conjunct contract; missing projected conjunct(s) "
                f"{', '.join(missing_conjuncts)}"
            )

        unexpected_conjuncts = [
            conjunct
            for conjunct in projected_conjuncts
            if conjunct not in expected_projected_set
        ]
        if unexpected_conjuncts:
            errors.append(
                f"{projected_operator} must mirror {top_operator} projected "
                "conjunct contract; unexpected projected conjunct(s) "
                f"{', '.join(unexpected_conjuncts)}"
            )

    for top_operator, top_antecedent in top_implication_contracts.items():
        projected_operator = operator_map.get(top_operator)
        if projected_operator is None:
            errors.append(
                "Byzantine top/projection alignment cannot map implication "
                f"operator {top_operator}"
            )
            continue
        expected_antecedent = projected_byzantine_top_conjunct(top_antecedent)
        actual_antecedent = projection_implication_contracts.get(projected_operator)
        if actual_antecedent != expected_antecedent:
            actual = actual_antecedent or "<missing>"
            errors.append(
                f"{projected_operator} implication antecedent must mirror "
                f"{top_operator}; expected {expected_antecedent}, found {actual}"
            )

    return errors


def byzantine_top_corridor_contract_alignment_errors(
    top_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_BYZANTINE_TOP_CONJUNCT_CONTRACTS,
    common_conjuncts: tuple[str, ...] = SUMERAGI_BYZANTINE_TOP_COMMON_CONJUNCTS,
    wait_conjuncts: tuple[str, ...] = SUMERAGI_BYZANTINE_TOP_WAIT_CONJUNCTS,
    delivered_operator: str = "ByzantineDeliveredFirstTopExactness",
    vote_operator: str = "ByzantineVoteFirstTopExactness",
    direct_operator: str = "ByzantineDirectTopExactness",
) -> list[str]:
    """Return errors if top-level Byzantine corridor contracts drift internally."""

    expected_contracts = {
        delivered_operator: common_conjuncts,
        vote_operator: (*common_conjuncts, *wait_conjuncts),
        direct_operator: (*common_conjuncts, *wait_conjuncts),
    }
    errors: list[str] = []
    for operator, expected_conjuncts in expected_contracts.items():
        actual_conjuncts = top_contracts.get(operator)
        if actual_conjuncts is None:
            errors.append(
                "Byzantine top corridor alignment references missing "
                f"contract {operator}"
            )
            continue

        expected_set = set(expected_conjuncts)
        actual_set = set(actual_conjuncts)
        missing_conjuncts = [
            conjunct for conjunct in expected_conjuncts if conjunct not in actual_set
        ]
        if missing_conjuncts:
            errors.append(
                f"{operator} must keep the Byzantine top corridor contract; "
                f"missing conjunct(s) {', '.join(missing_conjuncts)}"
            )

        unexpected_conjuncts = [
            conjunct for conjunct in actual_conjuncts if conjunct not in expected_set
        ]
        if unexpected_conjuncts:
            errors.append(
                f"{operator} must keep the Byzantine top corridor contract; "
                f"unexpected conjunct(s) {', '.join(unexpected_conjuncts)}"
            )

    return errors


def projection_bridge_interleaving_contract_alignment_errors(
    projection_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_PROJECTION_GATE_CONJUNCT_CONTRACTS,
    top_operator: str = "ProjectedByzantineDirectTopExactness",
    core_operator: str = "ProjectionBridgeMatchesInterleavingCore",
    bridge_operator: str = "ProjectionBridgeMatchesInterleavingExactness",
) -> list[str]:
    """Return errors if the full projection bridge drifts from its components."""

    errors: list[str] = []
    missing_inputs = [
        operator
        for operator in (top_operator, core_operator, bridge_operator)
        if operator not in projection_contracts
    ]
    if missing_inputs:
        errors.append(
            "projection bridge interleaving alignment references missing "
            f"contract(s) {', '.join(missing_inputs)}"
        )
        return errors

    expected_conjuncts = (
        *projection_contracts[top_operator],
        *projection_contracts[core_operator],
    )
    expected_set = set(expected_conjuncts)
    bridge_conjuncts = projection_contracts[bridge_operator]
    bridge_set = set(bridge_conjuncts)

    missing_conjuncts = [
        conjunct for conjunct in expected_conjuncts if conjunct not in bridge_set
    ]
    if missing_conjuncts:
        errors.append(
            f"{bridge_operator} must compose {top_operator} and "
            f"{core_operator}; missing conjunct(s) {', '.join(missing_conjuncts)}"
        )

    unexpected_conjuncts = [
        conjunct for conjunct in bridge_conjuncts if conjunct not in expected_set
    ]
    if unexpected_conjuncts:
        errors.append(
            f"{bridge_operator} must compose {top_operator} and "
            f"{core_operator}; unexpected conjunct(s) "
            f"{', '.join(unexpected_conjuncts)}"
        )

    return errors


def source_progress_safety_contract_alignment_errors(
    alignment_contracts: tuple[
        tuple[str, dict[str, tuple[str, ...]], str, tuple[str, ...]],
        ...,
    ] = SOURCE_PROGRESS_SAFETY_ENVELOPE_ALIGNMENT_CONTRACTS,
) -> list[str]:
    """Return errors if source progress safety envelopes drift from exactness."""

    errors: list[str] = []
    for (
        family,
        conjunct_contracts,
        envelope_operator,
        expected_components,
    ) in alignment_contracts:
        envelope_conjuncts = conjunct_contracts.get(envelope_operator)
        if envelope_conjuncts is None:
            errors.append(
                f"{family} progress safety alignment references missing "
                f"contract {envelope_operator}"
            )
            continue

        expected_set = set(expected_components)
        envelope_set = set(envelope_conjuncts)
        component_summary = " and ".join(expected_components)

        missing_conjuncts = [
            conjunct for conjunct in expected_components if conjunct not in envelope_set
        ]
        if missing_conjuncts:
            errors.append(
                f"{envelope_operator} must compose {component_summary} for "
                f"{family} progress safety; missing conjunct(s) "
                f"{', '.join(missing_conjuncts)}"
            )

        unexpected_conjuncts = [
            conjunct for conjunct in envelope_conjuncts if conjunct not in expected_set
        ]
        if unexpected_conjuncts:
            errors.append(
                f"{envelope_operator} must compose {component_summary} for "
                f"{family} progress safety; unexpected conjunct(s) "
                f"{', '.join(unexpected_conjuncts)}"
            )

    return errors


def byzantine_interleaving_exactness_alignment_errors(
    direct_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_DIRECT_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
    byzantine_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_BYZANTINE_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
    direct_operator: str = "DirectCommitInterleavingExactness",
    byzantine_operator: str = "ByzantineCommitInterleavingExactness",
    byzantine_extra_conjuncts: tuple[str, ...] = (
        BYZANTINE_INTERLEAVING_EXTRA_EXACTNESS_CONJUNCTS
    ),
) -> list[str]:
    """Return errors if Byzantine interleaving stops extending the direct core."""

    errors: list[str] = []
    direct_conjuncts = direct_contracts.get(direct_operator)
    byzantine_conjuncts = byzantine_contracts.get(byzantine_operator)
    missing_inputs = []
    if direct_conjuncts is None:
        missing_inputs.append(direct_operator)
    if byzantine_conjuncts is None:
        missing_inputs.append(byzantine_operator)
    if missing_inputs:
        errors.append(
            "Byzantine interleaving exactness alignment references missing "
            f"contract(s) {', '.join(missing_inputs)}"
        )
        return errors

    expected_conjuncts = (*direct_conjuncts, *byzantine_extra_conjuncts)
    expected_set = set(expected_conjuncts)
    byzantine_set = set(byzantine_conjuncts)

    missing_conjuncts = [
        conjunct for conjunct in expected_conjuncts if conjunct not in byzantine_set
    ]
    if missing_conjuncts:
        errors.append(
            f"{byzantine_operator} must extend {direct_operator} with "
            "Byzantine-only exactness conjuncts; missing conjunct(s) "
            f"{', '.join(missing_conjuncts)}"
        )

    unexpected_conjuncts = [
        conjunct for conjunct in byzantine_conjuncts if conjunct not in expected_set
    ]
    if unexpected_conjuncts:
        errors.append(
            f"{byzantine_operator} must extend {direct_operator} with "
            "Byzantine-only exactness conjuncts; unexpected conjunct(s) "
            f"{', '.join(unexpected_conjuncts)}"
        )

    return errors


def projection_bridge_core_source_alignment_errors(
    source_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_BYZANTINE_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
    projection_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_PROJECTION_GATE_CONJUNCT_CONTRACTS,
    source_operator: str = "ByzantineCommitInterleavingExactness",
    bridge_core_operator: str = "ProjectionBridgeMatchesInterleavingCore",
) -> list[str]:
    """Return errors if projection bridge core drifts from source exactness."""

    errors: list[str] = []
    source_conjuncts = source_contracts.get(source_operator)
    bridge_core_conjuncts = projection_contracts.get(bridge_core_operator)
    missing_inputs = []
    if source_conjuncts is None:
        missing_inputs.append(source_operator)
    if bridge_core_conjuncts is None:
        missing_inputs.append(bridge_core_operator)
    if missing_inputs:
        errors.append(
            "projection bridge core/source alignment references missing "
            f"contract(s) {', '.join(missing_inputs)}"
        )
        return errors

    source_set = set(source_conjuncts)
    bridge_core_set = set(bridge_core_conjuncts)

    missing_conjuncts = [
        conjunct for conjunct in source_conjuncts if conjunct not in bridge_core_set
    ]
    if missing_conjuncts:
        errors.append(
            f"{bridge_core_operator} must mirror {source_operator}; "
            f"missing conjunct(s) {', '.join(missing_conjuncts)}"
        )

    unexpected_conjuncts = [
        conjunct for conjunct in bridge_core_conjuncts if conjunct not in source_set
    ]
    if unexpected_conjuncts:
        errors.append(
            f"{bridge_core_operator} must mirror {source_operator}; "
            f"unexpected conjunct(s) {', '.join(unexpected_conjuncts)}"
        )

    return errors


def projected_commit_progress_contract_alignment_errors(
    projection_contracts: dict[
        str,
        tuple[str, ...],
    ] = SUMERAGI_PROJECTION_GATE_CONJUNCT_CONTRACTS,
    envelope_operator: str = "ProjectedCommitProgressSafetyEnvelope",
    expected_components: tuple[str, ...] = (
        "ProjectionBridgeCoversOrderedTopCorridors",
        "ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope",
    ),
) -> list[str]:
    """Return errors if projected progress safety drifts from bridge components."""

    errors: list[str] = []
    missing_inputs = [
        operator
        for operator in (*expected_components, envelope_operator)
        if operator not in projection_contracts
    ]
    if missing_inputs:
        errors.append(
            "projected commit progress alignment references missing "
            f"contract(s) {', '.join(missing_inputs)}"
        )
        return errors

    expected_set = set(expected_components)
    envelope_conjuncts = projection_contracts[envelope_operator]
    envelope_set = set(envelope_conjuncts)
    component_summary = " and ".join(expected_components)

    missing_conjuncts = [
        conjunct for conjunct in expected_components if conjunct not in envelope_set
    ]
    if missing_conjuncts:
        errors.append(
            f"{envelope_operator} must compose {component_summary}; "
            f"missing conjunct(s) {', '.join(missing_conjuncts)}"
        )

    unexpected_conjuncts = [
        conjunct for conjunct in envelope_conjuncts if conjunct not in expected_set
    ]
    if unexpected_conjuncts:
        errors.append(
            f"{envelope_operator} must compose {component_summary}; "
            f"unexpected conjunct(s) {', '.join(unexpected_conjuncts)}"
        )

    return errors


def tla_wf_vars_operator_references(body: str) -> tuple[str, ...]:
    """Return simple operator operands referenced by WF_vars clauses."""

    comment_free_body = "\n".join(
        tla_line_without_comment(line) for line in body.splitlines()
    )
    return tuple(TLA_WF_VARS_RE.findall(comment_free_body))


def commit_progress_spec_contract_errors(
    module_path: Path,
    spec_operator: str,
    fairness_operator: str,
    next_closure: str,
    expected_fairness_actions: tuple[str, ...],
    root_kind: str,
) -> list[str]:
    """Return errors if a progress spec/fairness wiring contract drifts."""

    errors = consensus_core_root_conjunct_contract_errors(
        module_path,
        {spec_operator: ("Init", fairness_operator)},
        root_kind=root_kind,
        zero_arity_requirement=(
            f"{root_kind} operators must be zero-arity operators"
        ),
        duplicate_requirement=(
            f"each {root_kind} obligation must be counted once"
        ),
        unexpected_requirement=(
            f"keep {root_kind}s on the documented conjunct contract"
        ),
    )
    if not module_path.exists():
        return errors

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)

    spec_definition = definitions.get(spec_operator)
    if spec_definition is not None:
        definition_line, body = spec_definition
        compact_body = "".join(
            "".join(tla_line_without_comment(line).split())
            for line in body.splitlines()
        )
        if next_closure not in compact_body:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{spec_operator}, but must keep direct {next_closure} "
                "transition closure"
            )

        raw_fairness_actions = tla_wf_vars_operator_references(body)
        if raw_fairness_actions:
            errors.append(
                f"{display_path(module_path)}:{definition_line} defines "
                f"{spec_operator}, but must compose {fairness_operator} "
                "instead of raw WF_vars fairness clauses: "
                f"{', '.join(raw_fairness_actions)}"
            )

    fairness_definition = definitions.get(fairness_operator)
    if fairness_definition is None:
        errors.append(
            f"{display_path(module_path)} does not define {root_kind} "
            f"fairness {fairness_operator}"
        )
        return errors

    signature = signatures.get(fairness_operator)
    if signature is not None and signature[1] != 0:
        root_line, root_arity = signature
        errors.append(
            f"{display_path(module_path)}:{root_line} defines {root_kind} "
            f"fairness {fairness_operator} with arity {root_arity}; "
            f"{root_kind} fairness operators must be zero-arity"
        )
        return errors

    fairness_line, fairness_body = fairness_definition
    fairness_actions = tla_wf_vars_operator_references(fairness_body)
    fairness_action_set = set(fairness_actions)
    expected_action_set = set(expected_fairness_actions)

    repeated_actions = duplicate_values(fairness_actions)
    if repeated_actions:
        errors.append(
            f"{display_path(module_path)}:{fairness_line} defines "
            f"{fairness_operator}, but repeats WF_vars action(s) "
            f"{', '.join(repeated_actions)}; each {root_kind} "
            "fairness action must be counted once"
        )

    missing_actions = [
        action
        for action in expected_fairness_actions
        if action not in fairness_action_set
    ]
    if missing_actions:
        errors.append(
            f"{display_path(module_path)}:{fairness_line} defines "
            f"{fairness_operator}, but is missing WF_vars action(s) "
            f"{', '.join(missing_actions)}"
        )

    unexpected_actions = [
        action for action in fairness_actions if action not in expected_action_set
    ]
    if unexpected_actions:
        errors.append(
            f"{display_path(module_path)}:{fairness_line} defines "
            f"{fairness_operator}, but contains unexpected WF_vars action(s) "
            f"{', '.join(unexpected_actions)}; keep {root_kind} "
            "fairness on the documented action contract"
        )

    return errors


def projected_commit_progress_spec_contract_errors(
    module_path: Path = SPEC_DIR / "SumeragiByzantineCommitProjectionGate.tla",
    spec_operator: str = "ProjectedCommitProgressSpec",
    fairness_operator: str = "ProjectedCommitProgressFairness",
    expected_fairness_actions: tuple[
        str, ...
    ] = SUMERAGI_PROJECTED_COMMIT_PROGRESS_FAIRNESS_ACTIONS,
) -> list[str]:
    """Return errors if projected progress spec/fairness wiring drifts."""

    return commit_progress_spec_contract_errors(
        module_path,
        spec_operator,
        fairness_operator,
        "[][Next]_vars",
        expected_fairness_actions,
        "projected commit progress",
    )


def source_commit_progress_spec_contract_errors(
    contracts: tuple[
        tuple[Path, str, str, str, tuple[str, ...], str],
        ...,
    ] = SUMERAGI_SOURCE_COMMIT_PROGRESS_SPEC_CONTRACTS,
) -> list[str]:
    """Return errors if source progress spec/fairness wiring drifts."""

    errors: list[str] = []
    for (
        module_path,
        spec_operator,
        fairness_operator,
        next_closure,
        expected_fairness_actions,
        root_kind,
    ) in contracts:
        errors.extend(
            commit_progress_spec_contract_errors(
                module_path,
                spec_operator,
                fairness_operator,
                next_closure,
                expected_fairness_actions,
                root_kind,
            )
        )
    return errors


def top_level_commit_spec_contract_errors(
    contracts: tuple[
        tuple[Path, str, str, str, tuple[str, ...], str],
        ...,
    ] = SUMERAGI_TOP_LEVEL_COMMIT_SPEC_CONTRACTS,
) -> list[str]:
    """Return errors if top-level Sumeragi spec/fairness wiring drifts."""

    errors: list[str] = []
    for (
        module_path,
        spec_operator,
        fairness_operator,
        next_closure,
        expected_fairness_actions,
        root_kind,
    ) in contracts:
        errors.extend(
            commit_progress_spec_contract_errors(
                module_path,
                spec_operator,
                fairness_operator,
                next_closure,
                expected_fairness_actions,
                root_kind,
            )
        )
    return errors


def direct_delivered_first_gate_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "SumeragiDirectDeliveredFirstCorridorGate.tla",
) -> list[str]:
    """Return errors for delivered-first progress-safety drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_DIRECT_DELIVERED_FIRST_GATE_CONJUNCT_CONTRACTS,
        root_kind="direct delivered-first progress safety aggregate",
        zero_arity_requirement=(
            "direct delivered-first progress safety operators must be "
            "zero-arity operators"
        ),
        duplicate_requirement=(
            "each direct delivered-first progress safety obligation must be "
            "counted once"
        ),
        unexpected_requirement=(
            "keep direct delivered-first progress safety operators on the "
            "documented conjunct contract"
        ),
    )


def direct_vote_first_gate_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "SumeragiDirectVoteFirstCorridorGate.tla",
) -> list[str]:
    """Return errors for vote-first progress-safety drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_DIRECT_VOTE_FIRST_GATE_CONJUNCT_CONTRACTS,
        root_kind="direct vote-first progress safety aggregate",
        zero_arity_requirement=(
            "direct vote-first progress safety operators must be "
            "zero-arity operators"
        ),
        duplicate_requirement=(
            "each direct vote-first progress safety obligation must be "
            "counted once"
        ),
        unexpected_requirement=(
            "keep direct vote-first progress safety operators on the "
            "documented conjunct contract"
        ),
    )


def byzantine_interleaving_gate_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "SumeragiByzantineCommitInterleavingGate.tla",
) -> list[str]:
    """Return errors for Byzantine interleaving progress-safety drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_BYZANTINE_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
        root_kind="Byzantine interleaving progress safety aggregate",
        zero_arity_requirement=(
            "Byzantine interleaving progress safety operators must be "
            "zero-arity operators"
        ),
        duplicate_requirement=(
            "each Byzantine interleaving progress safety obligation must be "
            "counted once"
        ),
        unexpected_requirement=(
            "keep Byzantine interleaving progress safety operators on the "
            "documented conjunct contract"
        ),
    )


def direct_interleaving_gate_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "SumeragiDirectCommitInterleavingGate.tla",
) -> list[str]:
    """Return errors for direct interleaving progress-safety drift."""

    return consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_DIRECT_INTERLEAVING_GATE_CONJUNCT_CONTRACTS,
        root_kind="direct interleaving progress safety aggregate",
        zero_arity_requirement=(
            "direct interleaving progress safety operators must be "
            "zero-arity operators"
        ),
        duplicate_requirement=(
            "each direct interleaving progress safety obligation must be "
            "counted once"
        ),
        unexpected_requirement=(
            "keep direct interleaving progress safety operators on the "
            "documented conjunct contract"
        ),
    )


def projection_gate_conjunct_contract_errors(
    module_path: Path = SPEC_DIR / "SumeragiByzantineCommitProjectionGate.tla",
) -> list[str]:
    """Return errors for Byzantine projection gate aggregate proof drift."""

    errors = consensus_core_root_conjunct_contract_errors(
        module_path,
        SUMERAGI_PROJECTION_GATE_CONJUNCT_CONTRACTS,
        root_kind="projection gate aggregate",
        zero_arity_requirement=(
            "projection gate aggregate operators must be zero-arity operators"
        ),
        duplicate_requirement=(
            "each projection gate aggregate obligation must be counted once"
        ),
        unexpected_requirement=(
            "keep projection gate aggregate proof operators on the documented "
            "conjunct contract"
        ),
    )
    errors.extend(
        implication_antecedent_contract_errors(
            module_path,
            SUMERAGI_PROJECTION_GATE_IMPLICATION_CONTRACTS,
        )
    )
    return errors


def consensus_core_root_cfg_check_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
    cfg_paths: tuple[Path, ...] = (SUMERAGI_DEEP_CFG, SUMERAGI_TLC_FAST_CFG),
    contracts: dict[
        str, dict[str, str]
    ] = SUMERAGI_CONSENSUS_CORE_ROOT_CFG_CHECK_CONTRACTS,
) -> list[str]:
    """Return errors for consensus-core roots with drifted CFG check roles."""

    if not module_path.exists() or any(not path.exists() for path in cfg_paths):
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    cfg_check_kinds_by_path: dict[Path, dict[str, str]] = {}
    for cfg_path in cfg_paths:
        check_kinds, parse_errors = cfg_check_operator_kinds(cfg_path)
        if parse_errors:
            return []
        cfg_check_kinds_by_path[cfg_path] = check_kinds

    errors: list[str] = []
    for root_operator, conjunct_contracts in contracts.items():
        definition = definitions.get(root_operator)
        if definition is None:
            errors.append(
                f"{display_path(module_path)} does not define "
                f"consensus-core proof root {root_operator}"
            )
            continue

        definition_line, body = definition
        direct_conjuncts = set(tla_zero_arity_conjunct_references(body))
        for conjunct, expected_kind in conjunct_contracts.items():
            if conjunct not in direct_conjuncts:
                errors.append(
                    f"{display_path(module_path)}:{definition_line} defines "
                    f"{root_operator}, but expected CFG-checked direct "
                    f"conjunct {conjunct} is not a direct root conjunct"
                )
                continue
            for cfg_path, check_kinds in cfg_check_kinds_by_path.items():
                actual_kind = check_kinds.get(conjunct)
                if actual_kind is None:
                    errors.append(
                        f"{display_path(module_path)}:{definition_line} "
                        f"defines {root_operator}, but direct conjunct "
                        f"{conjunct} is not checked as a top-level "
                        f"{expected_kind} by {display_path(cfg_path)}"
                    )
                elif actual_kind != expected_kind:
                    errors.append(
                        f"{display_path(module_path)}:{definition_line} "
                        f"defines {root_operator}, but direct conjunct "
                        f"{conjunct} is checked as {actual_kind} by "
                        f"{display_path(cfg_path)}; expected {expected_kind}"
                    )
    return errors


@cache
def tla_operator_definition_entries(path: Path) -> list[tuple[int, str]]:
    entries, _ = tla_operator_definition_entries_and_errors(path)
    return entries


@cache
def tla_operator_definition_entries_and_errors(
    path: Path,
) -> tuple[list[tuple[int, str]], list[str]]:
    signature_entries, errors = tla_operator_definition_signature_entries_and_errors(
        path
    )
    return [(line_number, name) for line_number, name, _ in signature_entries], errors


@cache
def tla_operator_definition_signature_entries_and_errors(
    path: Path,
) -> tuple[list[tuple[int, str, int]], list[str]]:
    entries: list[tuple[int, str, int]] = []
    errors: list[str] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line)
        if stripped.startswith((" ", "\t")):
            continue
        if TLA_FORBIDDEN_DIRECTIVE_RE.match(stripped.strip()):
            continue

        match = TLA_OPERATOR_DEFINITION_BODY_RE.match(stripped)
        if match is None:
            if "==" in stripped and TLA_OPERATOR_DEFINITION_START_RE.match(stripped):
                errors.append(
                    f"{display_path(path)}:{line_number} TLA operator "
                    f"definition must use a static signature: {stripped.strip()}"
                )
            continue

        if match.group("local") is not None:
            errors.append(
                f"{display_path(path)}:{line_number} TLA operator "
                f"definition must be non-LOCAL: {stripped.strip()}"
            )
            continue

        body = match.group("body").strip()
        if TLA_INSTANCE_BODY_RE.match(body):
            continue
        operator = match.group("name")
        if not is_tla_operator_name(operator):
            errors.append(
                f"{display_path(path)}:{line_number} TLA operator "
                "definition must use a non-reserved static name: "
                f"{stripped.strip()}"
            )
            continue
        params = match.group("params")
        arity = 0
        if params is not None:
            param_names = [param.strip() for param in params.split(",")]
            if not param_names or any(
                not is_tla_operator_name(param) for param in param_names
            ):
                errors.append(
                    f"{display_path(path)}:{line_number} TLA operator "
                    "definition must use static parameters: "
                    f"{stripped.strip()}"
                )
                continue
            if len(set(param_names)) != len(param_names):
                errors.append(
                    f"{display_path(path)}:{line_number} TLA operator "
                    "definition must use unique static parameters: "
                    f"{stripped.strip()}"
                )
                continue
            arity = len(param_names)
        entries.append((line_number, operator, arity))
    return entries, errors


def tla_operator_definition_parse_errors(path: Path) -> list[str]:
    _, errors = tla_operator_definition_entries_and_errors(path)
    return errors


@cache
def tla_recursive_declaration_entries(path: Path) -> list[tuple[int, str]]:
    entries, _ = tla_recursive_declaration_entries_and_errors(path)
    return entries


def split_top_level_commas(text: str) -> tuple[list[str], str | None]:
    parts: list[str] = []
    start = 0
    depth = 0
    for index, char in enumerate(text):
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth < 0:
                return [], "unbalanced parentheses"
        elif char == "," and depth == 0:
            parts.append(text[start:index].strip())
            start = index + 1
    if depth != 0:
        return [], "unbalanced parentheses"
    parts.append(text[start:].strip())
    if any(not part for part in parts):
        return [], "empty recursive declaration entry"
    return parts, None


@cache
def tla_recursive_declaration_entries_and_errors(
    path: Path,
) -> tuple[list[tuple[int, str]], list[str]]:
    signature_entries, errors = tla_recursive_declaration_signature_entries_and_errors(
        path
    )
    return [(line_number, name) for line_number, name, _ in signature_entries], errors


@cache
def tla_recursive_declaration_signature_entries_and_errors(
    path: Path,
) -> tuple[list[tuple[int, str, int]], list[str]]:
    entries: list[tuple[int, str, int]] = []
    errors: list[str] = []

    def no_separator_recursive_declaration_start(text: str) -> str | None:
        directive = "RECURSIVE"
        if "==" in text or not text.startswith(directive):
            return None
        rest = text[len(directive) :]
        if rest and not rest[:1].isspace():
            return directive
        return None

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line)
        if stripped.startswith((" ", "\t")):
            if TLA_RECURSIVE_START_RE.match(stripped.lstrip()):
                errors.append(
                    f"{display_path(path)}:{line_number} RECURSIVE "
                    f"declaration directive must be top-level: {stripped.strip()}"
                )
            continue

        match = TLA_RECURSIVE_RE.match(stripped)
        if match is None:
            no_separator_directive = no_separator_recursive_declaration_start(stripped)
            if no_separator_directive is not None:
                errors.append(
                    f"{display_path(path)}:{line_number} malformed "
                    f"RECURSIVE declaration directive {no_separator_directive}: "
                    f"{stripped.strip()}"
                )
            elif TLA_RECURSIVE_START_RE.match(stripped):
                errors.append(
                    f"{display_path(path)}:{line_number} RECURSIVE declaration "
                    f"must list static operator declarations: {stripped.strip()}"
                )
            continue

        parts, split_error = split_top_level_commas(match.group(1).strip())
        if split_error is not None:
            errors.append(
                f"{display_path(path)}:{line_number} RECURSIVE declaration "
                f"must list static operator declarations: {split_error}"
            )
            continue

        line_entries: list[tuple[int, str, int]] = []
        line_errors: list[str] = []
        line_duplicate_parameter_errors: list[str] = []
        for part in parts:
            entry_match = TLA_RECURSIVE_ENTRY_RE.match(part)
            if entry_match is None:
                line_errors.append(part)
                continue
            if not is_tla_operator_name(entry_match.group(1)):
                line_errors.append(part)
                continue
            params = entry_match.group(2)
            arity = 0
            if params is not None:
                param_names = [param.strip() for param in params.split(",")]
                if not param_names or any(
                    param != "_" and not is_tla_operator_name(param)
                    for param in param_names
                ):
                    line_errors.append(part)
                    continue
                named_params = [param for param in param_names if param != "_"]
                if len(set(named_params)) != len(named_params):
                    line_duplicate_parameter_errors.append(part)
                    continue
                arity = len(param_names)
            line_entries.append((line_number, entry_match.group(1), arity))

        if line_errors or line_duplicate_parameter_errors:
            if line_errors:
                errors.append(
                    f"{display_path(path)}:{line_number} RECURSIVE declaration "
                    "must list static operator declarations: "
                    + ", ".join(line_errors)
                )
            if line_duplicate_parameter_errors:
                errors.append(
                    f"{display_path(path)}:{line_number} RECURSIVE declaration "
                    "must use unique static operator parameters: "
                    + ", ".join(line_duplicate_parameter_errors)
                )
            continue
        entries.extend(line_entries)
    return entries, errors


def tla_recursive_declaration_parse_errors(path: Path) -> list[str]:
    _, errors = tla_recursive_declaration_entries_and_errors(path)
    return errors


@cache
def tla_operator_definitions(path: Path) -> set[str]:
    definitions = {name for _, name in tla_operator_definition_entries(path)}
    return definitions


@cache
def tla_operator_signatures(path: Path) -> dict[str, tuple[int, int]]:
    signatures: dict[str, tuple[int, int]] = {}
    for line_number, name, arity in tla_operator_definition_signature_entries_and_errors(
        path
    )[0]:
        signatures[name] = (line_number, arity)
    return signatures


@cache
def tla_operator_parameter_names(path: Path) -> dict[str, frozenset[str]]:
    parameter_names: dict[str, frozenset[str]] = {}
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line)
        if stripped.startswith((" ", "\t")):
            continue
        if TLA_FORBIDDEN_DIRECTIVE_RE.match(stripped.strip()):
            continue

        match = TLA_OPERATOR_DEFINITION_BODY_RE.match(stripped)
        if match is None:
            continue
        if match.group("local") is not None:
            continue
        body = match.group("body").strip()
        if TLA_INSTANCE_BODY_RE.match(body):
            continue
        name = match.group("name")
        if not is_tla_operator_name(name):
            continue

        params = match.group("params")
        names: list[str] = []
        if params is not None:
            names = [param.strip() for param in params.split(",")]
            if not names or any(not is_tla_operator_name(param) for param in names):
                continue
            if len(set(names)) != len(names):
                continue
        parameter_names[name] = frozenset(names)
    return parameter_names


@cache
def tla_single_expression_operator_definitions(path: Path) -> dict[str, tuple[int, str]]:
    entries: dict[str, tuple[int, str]] = {}
    lines = read_text(path).splitlines()
    for index, line in enumerate(lines):
        line_number = index + 1
        stripped = tla_line_without_comment(line)
        if stripped.startswith((" ", "\t")):
            continue
        if TLA_FORBIDDEN_DIRECTIVE_RE.match(stripped.strip()):
            continue
        match = TLA_OPERATOR_DEFINITION_BODY_RE.match(stripped)
        if match is None:
            continue
        if match.group("local") is not None:
            continue
        body = match.group("body").strip()
        if TLA_INSTANCE_BODY_RE.match(body):
            continue
        name = match.group("name")
        if not is_tla_operator_name(name):
            continue
        params = match.group("params")
        if params is not None:
            param_names = [param.strip() for param in params.split(",")]
            if not param_names or any(
                not is_tla_operator_name(param) for param in param_names
            ):
                continue
        if body:
            entries[name] = (line_number, body)
            continue

        body_lines: list[tuple[int, str]] = []
        for body_index, body_line in enumerate(lines[index + 1 :], line_number + 1):
            body_stripped = tla_line_without_comment(body_line)
            if not body_stripped.strip():
                continue
            if not body_stripped.startswith((" ", "\t")):
                break
            body_lines.append((body_index, body_stripped.strip()))

        if body_lines:
            entries[name] = (
                body_lines[0][0],
                " ".join(body for _, body in body_lines),
            )
    return entries


@cache
def tla_literal_operator_definitions(path: Path) -> dict[str, tuple[int, str]]:
    return {
        name: entry
        for name, entry in tla_single_expression_operator_definitions(path).items()
        if tla_static_boolean_literal(entry[1]) is not None
    }


@cache
def tla_type_invariant_alias_definitions(path: Path) -> dict[str, tuple[int, str]]:
    return {
        name: entry
        for name, entry in tla_single_expression_operator_definitions(path).items()
        if tla_static_identifier_alias(entry[1]) == "TypeInvariant"
    }


def strip_static_outer_parentheses(expression: str) -> str:
    stripped = expression.strip()
    while stripped.startswith("(") and stripped.endswith(")"):
        depth = 0
        in_string = False
        escaped = False
        encloses_full_expression = True
        for index, char in enumerate(stripped):
            if in_string:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    in_string = False
                continue
            if char == '"':
                in_string = True
                continue
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth < 0:
                    return stripped
                if depth == 0 and index != len(stripped) - 1:
                    encloses_full_expression = False
                    break
        if depth != 0 or in_string or not encloses_full_expression:
            return stripped
        stripped = stripped[1:-1].strip()
    return stripped


def tla_static_identifier_alias(expression: str) -> str | None:
    stripped = strip_static_outer_parentheses(expression)
    if TLA_IDENTIFIER_RE.match(stripped):
        return stripped
    return None


def tla_static_boolean_literal(expression: str) -> str | None:
    tokens: list[str] = []
    index = 0
    while index < len(expression):
        if expression[index].isspace():
            index += 1
            continue
        match = TLA_BOOLEAN_LITERAL_TOKEN_RE.match(expression, index)
        if match is None:
            return None
        tokens.append(match.group(0))
        index = match.end()
    if not tokens:
        return None

    position = 0

    def peek() -> str | None:
        if position >= len(tokens):
            return None
        return tokens[position]

    def parse_atom() -> bool | None:
        nonlocal position
        token = peek()
        if token is None:
            return None
        if token == "~":
            position += 1
            value = parse_atom()
            if value is None:
                return None
            return not value
        if token in {"/\\", "\\/"}:
            position += 1
            return parse_atom()
        if token == "(":
            position += 1
            value = parse_or()
            if value is None or peek() != ")":
                return None
            position += 1
            return value
        if token == "TRUE":
            position += 1
            return True
        if token == "FALSE":
            position += 1
            return False
        return None

    def parse_and() -> bool | None:
        nonlocal position
        value = parse_atom()
        if value is None:
            return None
        while peek() == "/\\":
            position += 1
            rhs = parse_atom()
            if rhs is None:
                return None
            value = value and rhs
        return value

    def parse_or() -> bool | None:
        nonlocal position
        value = parse_and()
        if value is None:
            return None
        while peek() == "\\/":
            position += 1
            rhs = parse_and()
            if rhs is None:
                return None
            value = value or rhs
        return value

    value = parse_or()
    if value is None or position != len(tokens):
        return None
    return "TRUE" if value else "FALSE"


def tla_static_temporal_boolean_literal(expression: str) -> str | None:
    """Return a static boolean literal through unary temporal wrappers."""

    memo: dict[str, str | None] = {}
    visiting: set[str] = set()

    def collect(body: str) -> str | None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if normalized in memo:
            return memo[normalized]
        if normalized in visiting:
            return None
        visiting.add(normalized)

        literal = tla_static_boolean_literal(normalized)
        if literal is not None:
            visiting.remove(normalized)
            memo[normalized] = literal
            return literal

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            result = collect(let_operand)
            visiting.remove(normalized)
            memo[normalized] = result
            return result

        compound_literal = tla_compound_temporal_boolean_literal(normalized, collect)
        if compound_literal is not None:
            visiting.remove(normalized)
            memo[normalized] = compound_literal
            return compound_literal

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            result = collect(operand)
            visiting.remove(normalized)
            memo[normalized] = result
            return result

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            negated_literal = collect(negated_operand)
            result = None
            if negated_literal == "TRUE":
                result = "FALSE"
            elif negated_literal == "FALSE":
                result = "TRUE"
            visiting.remove(normalized)
            memo[normalized] = result
            return result

        result = None
        visiting.remove(normalized)
        memo[normalized] = result
        return result

    return collect(expression)


def tla_compound_temporal_boolean_literal(
    expression: str, literal_of: Callable[[str], str | None]
) -> str | None:
    """Return a literal result for compound boolean temporal expressions."""

    conjuncts = tla_top_level_conjuncts(expression)
    if len(conjuncts) > 1:
        values = [literal_of(conjunct) for conjunct in conjuncts]
        if "FALSE" in values:
            return "FALSE"
        if all(value == "TRUE" for value in values):
            return "TRUE"
        return None

    disjuncts = tla_top_level_disjuncts(expression)
    if len(disjuncts) > 1:
        values = [literal_of(disjunct) for disjunct in disjuncts]
        if "TRUE" in values:
            return "TRUE"
        if all(value == "FALSE" for value in values):
            return "FALSE"
        return None

    implication_operands = tla_top_level_implication_operands(expression)
    if len(implication_operands) > 1:
        antecedent = literal_of(implication_operands[0])
        consequent = literal_of(implication_operands[1])
        if antecedent == "FALSE" or consequent == "TRUE":
            return "TRUE"
        if antecedent == "TRUE" and consequent == "FALSE":
            return "FALSE"
        return None

    equivalence_operands = tla_top_level_equivalence_operands(expression)
    if len(equivalence_operands) > 1:
        left = literal_of(equivalence_operands[0])
        right = literal_of(equivalence_operands[1])
        if left is None or right is None:
            return None
        return "TRUE" if left == right else "FALSE"

    return None


def tla_trivial_terminal_expression(expression: str) -> str | None:
    literal = tla_static_boolean_literal(expression)
    if literal is not None:
        return literal
    alias = tla_static_identifier_alias(expression)
    if alias == "TypeInvariant":
        return alias
    return None


@cache
def tla_trivial_operator_chains(
    path: Path,
) -> dict[str, list[tuple[str, int, str]]]:
    single_expressions = tla_single_expression_operator_definitions(path)
    chains: dict[str, list[tuple[str, int, str]]] = {}

    for operator in single_expressions:
        current = operator
        seen: set[str] = set()
        chain: list[tuple[str, int, str]] = []
        while current in single_expressions and current not in seen:
            seen.add(current)
            line_number, body = single_expressions[current]
            terminal = tla_trivial_terminal_expression(body)
            if terminal is not None:
                chain.append((current, line_number, terminal))
                chains[operator] = chain
                break
            target = tla_static_identifier_alias(body)
            if target is None:
                break
            chain.append((current, line_number, target))
            current = target

    return chains


def tla_duplicate_operator_definition_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors = [
        f"{mode}: {error}" for error in tla_recursive_declaration_parse_errors(path)
    ]
    errors.extend(
        f"{mode}: {error}" for error in tla_operator_definition_parse_errors(path)
    )
    seen_definitions: dict[str, int] = {}
    for line_number, operator in tla_operator_definition_entries(path):
        previous_line = seen_definitions.get(operator)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA operator definition {operator} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_definitions[operator] = line_number

    seen_recursive: dict[str, int] = {}
    for line_number, operator in tla_recursive_declaration_entries(path):
        previous_line = seen_recursive.get(operator)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA RECURSIVE declaration {operator} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_recursive[operator] = line_number

    definition_signatures = {
        name: (line_number, arity)
        for line_number, name, arity in tla_operator_definition_signature_entries_and_errors(
            path
        )[0]
    }
    for line_number, operator, arity in tla_recursive_declaration_signature_entries_and_errors(
        path
    )[0]:
        definition = definition_signatures.get(operator)
        if definition is None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} declares "
                f"TLA RECURSIVE operator {operator}, but no top-level "
                "definition exists"
            )
            continue
        definition_line, definition_arity = definition
        if definition_arity != arity:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} declares "
                f"TLA RECURSIVE operator {operator} with arity {arity}, but "
                f"definition at line {definition_line} has arity "
                f"{definition_arity}"
            )
    return errors


@cache
def tla_module_dependency_references(
    path: Path,
) -> tuple[list[tuple[int, str, str]], list[str]]:
    references: list[tuple[int, str, str]] = []
    errors: list[str] = []
    dependency_window_closed_line: int | None = None
    declarations_seen = False
    declaration_block_open = False

    def declaration_start(text: str) -> bool:
        return re.match(r"^(?:CONSTANTS?|VARIABLES?)\b", text) is not None

    def declaration_list_continuation(text: str) -> bool:
        return TLA_DECLARATION_LIST_RE.match(text) is not None

    def malformed_dependency_prefix_start(text: str) -> str | None:
        def missing_separator_after(prefix: str) -> bool:
            rest = text[len(prefix) :]
            return bool(rest) and not rest[:1].isspace()

        if re.match(r"^(?:LOCAL\s+)?[^=]+==\s*INSTANCE(?=\S)", text):
            return "INSTANCE"
        if "==" in text:
            return None
        if text.startswith("EXTENDS") and missing_separator_after("EXTENDS"):
            return "EXTENDS"
        if text.startswith("INSTANCE") and missing_separator_after("INSTANCE"):
            return "INSTANCE"
        if text.startswith("LOCAL INSTANCE") and missing_separator_after(
            "LOCAL INSTANCE"
        ):
            return "INSTANCE"
        if text.startswith("LOCALINSTANCE"):
            return "INSTANCE"
        return None

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        dependency_text = tla_line_without_comment(line)
        stripped = dependency_text.strip()
        if not stripped:
            continue
        malformed_dependency_prefix = malformed_dependency_prefix_start(stripped)
        dependency_candidate = (
            TLA_EXTENDS_START_RE.match(stripped) is not None
            or TLA_INSTANCE_START_RE.match(stripped) is not None
            or TLA_NAMED_INSTANCE_START_RE.match(stripped) is not None
            or malformed_dependency_prefix is not None
        )
        extends_candidate = TLA_EXTENDS_START_RE.match(stripped) is not None
        instance_candidate = (
            TLA_INSTANCE_START_RE.match(stripped) is not None
            or TLA_NAMED_INSTANCE_START_RE.match(stripped) is not None
            or malformed_dependency_prefix == "INSTANCE"
        )
        if dependency_text != dependency_text.lstrip() and dependency_candidate:
            errors.append(
                f"{display_path(path)}:{line_number} TLA dependency "
                f"declarations must be top-level: {stripped}"
            )
            continue
        if (
            dependency_window_closed_line is not None
            and dependency_candidate
        ):
            errors.append(
                f"{display_path(path)}:{line_number} TLA dependency "
                "declarations must appear before operator definitions: "
                f"{stripped}"
            )
            continue
        if declarations_seen and extends_candidate:
            errors.append(
                f"{display_path(path)}:{line_number} EXTENDS declarations "
                "must appear before declarations and definitions: "
                f"{stripped}"
            )
            continue

        if malformed_dependency_prefix is not None:
            errors.append(
                f"{display_path(path)}:{line_number} malformed "
                f"{malformed_dependency_prefix} dependency declaration: {stripped}"
            )
            continue

        match = TLA_EXTENDS_RE.match(stripped)
        if match is not None:
            modules = [module.strip() for module in match.group(1).split(",")]
            if not modules or any(
                not TLA_IDENTIFIER_RE.match(module) for module in modules
            ):
                errors.append(
                    f"{display_path(path)}:{line_number} EXTENDS "
                    "must list static module identifiers: "
                    f"{match.group(1).strip()}"
                )
                continue
            if any(not is_tla_module_name(module) for module in modules):
                errors.append(
                    f"{display_path(path)}:{line_number} EXTENDS "
                    "must list non-reserved static module identifiers: "
                    f"{match.group(1).strip()}"
                )
                continue
            references.extend(
                (line_number, "EXTENDS", module) for module in modules
            )
            continue

        if TLA_EXTENDS_START_RE.match(stripped):
            errors.append(
                f"{display_path(path)}:{line_number} EXTENDS "
                f"must list static module identifiers: {stripped}"
            )
            continue

        match = TLA_INSTANCE_RE.match(stripped)
        if match is not None:
            if match.group("local") is not None:
                errors.append(
                    f"{display_path(path)}:{line_number} INSTANCE "
                    f"declarations must be non-LOCAL: {stripped}"
                )
                continue
            alias = match.group("alias")
            if alias is not None and not is_tla_user_identifier(alias):
                errors.append(
                    f"{display_path(path)}:{line_number} INSTANCE alias "
                    "must be a non-reserved static identifier: "
                    f"{stripped}"
                )
                continue
            module = match.group("module")
            if not is_tla_module_name(module):
                errors.append(
                    f"{display_path(path)}:{line_number} INSTANCE "
                    "must reference a non-reserved static module identifier: "
                    f"{stripped}"
                )
                continue
            references.append((line_number, "INSTANCE", module))
            continue

        if TLA_INSTANCE_WITH_RE.match(stripped):
            local_instance = TLA_INSTANCE_WITH_RE.match(stripped)
            if local_instance is not None and local_instance.group("local") is not None:
                errors.append(
                    f"{display_path(path)}:{line_number} INSTANCE "
                    f"declarations must be non-LOCAL: {stripped}"
                )
                continue
            errors.append(
                f"{display_path(path)}:{line_number} INSTANCE "
                "substitutions are not supported; use a static module "
                f"identifier without WITH: {stripped}"
            )
            continue

        if TLA_INSTANCE_START_RE.match(stripped):
            errors.append(
                f"{display_path(path)}:{line_number} INSTANCE "
                f"must reference a static module identifier: {stripped}"
            )
            continue

        match = TLA_NAMED_INSTANCE_START_RE.match(stripped)
        if match is not None:
            if match.group("local") is not None:
                errors.append(
                    f"{display_path(path)}:{line_number} INSTANCE "
                    f"declarations must be non-LOCAL: {stripped}"
                )
                continue
            alias = match.group("alias").strip()
            if not is_tla_user_identifier(alias):
                errors.append(
                    f"{display_path(path)}:{line_number} INSTANCE alias "
                    "must be a non-reserved static identifier: "
                    f"{stripped}"
                )
                continue
            errors.append(
                f"{display_path(path)}:{line_number} INSTANCE "
                f"must reference a static module identifier: {stripped}"
            )
            continue

        if (
            dependency_window_closed_line is None
            and not TLA_MODULE_RE.match(stripped)
            and not TLA_TERMINATOR_RE.match(stripped)
        ):
            if declaration_start(stripped):
                declarations_seen = True
                declaration_block_open = True
                continue
            if declaration_block_open and declaration_list_continuation(stripped):
                continue
            declaration_block_open = False
            dependency_window_closed_line = line_number

    return references, errors


@cache
def tla_instance_alias_entries(path: Path) -> list[tuple[int, str]]:
    entries: list[tuple[int, str]] = []
    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line).strip()
        if not stripped:
            continue
        match = TLA_INSTANCE_RE.match(stripped)
        if match is None:
            continue
        if match.group("local") is not None:
            continue
        alias = match.group("alias")
        if alias is None:
            continue
        module = match.group("module")
        if is_tla_user_identifier(alias) and is_tla_module_name(module):
            entries.append((line_number, alias))
    return entries


def tla_module_dependency_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    references, parse_errors = tla_module_dependency_references(path)
    errors = [f"{mode}: {error}" for error in parse_errors]
    seen_dependencies: dict[str, tuple[int, str]] = {}
    for line_number, directive, module in references:
        previous = seen_dependencies.get(module)
        if previous is not None:
            previous_line, previous_directive = previous
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA module dependency {module} first referenced as "
                f"{previous_directive} at line {previous_line}"
            )
            continue
        seen_dependencies[module] = (line_number, directive)
        if module in TLA_STANDARD_MODULES:
            continue
        dependency_path = path.with_name(f"{module}.tla")
        if dependency_path.exists():
            continue
        errors.append(
            f"{mode}: {display_path(path)}:{line_number} references "
            f"{directive} module {module}, but neither TLA standard module "
            f"nor {display_path(dependency_path)} exists"
        )
    return errors


@cache
def tla_local_dependency_files(path: Path) -> tuple[Path, ...]:
    if not path.exists():
        return ()

    references, _ = tla_module_dependency_references(path)
    dependencies: list[Path] = []
    seen_dependencies: set[Path] = set()
    for _, _, module in references:
        if module in TLA_STANDARD_MODULES:
            continue
        dependency_path = path.with_name(f"{module}.tla")
        if not dependency_path.exists() or dependency_path in seen_dependencies:
            continue
        seen_dependencies.add(dependency_path)
        dependencies.append(dependency_path)
    return tuple(dependencies)


@cache
def tla_reachable_module_files(path: Path) -> tuple[Path, ...]:
    reachable: list[Path] = [path]
    seen: set[Path] = {path}
    pending = list(tla_local_dependency_files(path))

    while pending:
        dependency_path = pending.pop(0)
        if dependency_path in seen:
            continue
        seen.add(dependency_path)
        reachable.append(dependency_path)
        pending.extend(tla_local_dependency_files(dependency_path))

    return tuple(reachable)


def tla_instance_alias_namespace_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    namespace_lines: dict[str, tuple[int, str]] = {}
    for line_number, constant in tla_constant_declaration_entries(path):
        namespace_lines.setdefault(constant, (line_number, "constant declaration"))
    for line_number, variable in tla_variable_declaration_entries(path):
        namespace_lines.setdefault(variable, (line_number, "variable declaration"))
    for line_number, operator in tla_operator_definition_entries(path):
        namespace_lines.setdefault(operator, (line_number, "TLA operator definition"))
    for line_number, operator in tla_recursive_declaration_entries(path):
        namespace_lines.setdefault(operator, (line_number, "TLA RECURSIVE declaration"))

    errors: list[str] = []
    seen_aliases: dict[str, int] = {}
    for line_number, alias in tla_instance_alias_entries(path):
        previous_line = seen_aliases.get(alias)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"INSTANCE alias {alias} first declared at line {previous_line}"
            )
        else:
            seen_aliases[alias] = line_number

        namespace = namespace_lines.get(alias)
        if namespace is None:
            continue
        namespace_line, namespace_kind = namespace
        errors.append(
            f"{mode}: {display_path(path)}:{line_number} INSTANCE alias "
            f"{alias} overlaps with {namespace_kind} at line {namespace_line}"
        )
    return errors


def tla_forbidden_directive_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors: list[str] = []

    def no_separator_forbidden_directive_start(text: str) -> str | None:
        if "==" in text:
            return None
        for directive in sorted(
            TLA_ASSUMPTION_DIRECTIVE_WORDS | TLA_PROOF_DIRECTIVE_WORDS,
            key=len,
            reverse=True,
        ):
            if not text.startswith(directive):
                continue
            rest = text[len(directive) :]
            if rest and not rest[:1].isspace():
                return directive
            return None
        return None

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line)
        stripped_directive = stripped.strip()
        if not stripped_directive:
            continue
        match = TLA_FORBIDDEN_DIRECTIVE_RE.match(stripped_directive)
        no_separator_directive = None
        if match is None:
            no_separator_directive = no_separator_forbidden_directive_start(
                stripped_directive
            )
            if no_separator_directive is None:
                continue
        directive = match.group(1) if match is not None else no_separator_directive
        if directive in TLA_PROOF_DIRECTIVE_WORDS:
            reason = "proof-free"
        else:
            reason = "assumption-free"
        placement = "indented" if stripped.startswith((" ", "\t")) else "top-level"
        if match is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} uses {placement} "
                f"{directive} directive; Sumeragi formal modules must be "
                f"{reason}"
            )
        else:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} uses {placement} "
                f"{directive} directive start: {stripped_directive}; Sumeragi "
                f"formal modules must be {reason}"
            )
    return errors


@cache
def tla_declaration_block_entries(
    path: Path,
    declaration_directives: frozenset[str],
    stop_directives: frozenset[str],
    label: str,
) -> tuple[list[tuple[int, str]], list[str]]:
    entries: list[tuple[int, str]] = []
    errors: list[str] = []
    collecting_label: str | None = None
    collecting_line: int | None = None
    collecting_entries = 0
    collecting_invalid = False
    collecting_pending_comma_line: int | None = None

    def close_collecting() -> None:
        nonlocal collecting_label, collecting_line, collecting_entries
        nonlocal collecting_invalid, collecting_pending_comma_line
        if (
            collecting_label is not None
            and collecting_line is not None
            and collecting_entries == 0
            and not collecting_invalid
        ):
            errors.append(
                f"{display_path(path)}:{collecting_line} {label} block "
                "must declare at least one identifier"
            )
        elif (
            collecting_label is not None
            and collecting_pending_comma_line is not None
            and not collecting_invalid
        ):
            errors.append(
                f"{display_path(path)}:{collecting_pending_comma_line} "
                f"{label} declaration block ends with trailing comma"
            )
        collecting_label = None
        collecting_line = None
        collecting_entries = 0
        collecting_invalid = False
        collecting_pending_comma_line = None

    def parse_declaration_line(
        line_number: int, declaration: str
    ) -> tuple[list[str], bool]:
        if not TLA_DECLARATION_LIST_RE.match(declaration):
            errors.append(
                f"{display_path(path)}:{line_number} {label} declaration "
                f"line must list static identifiers: {declaration}"
            )
            return [], False
        names = TLA_IDENTIFIER_SCAN_RE.findall(declaration)
        if any(not is_tla_user_identifier(name) for name in names):
            errors.append(
                f"{display_path(path)}:{line_number} {label} declaration "
                f"line must list non-reserved static identifiers: {declaration}"
            )
            return [], False
        return names, declaration.endswith(",")

    def malformed_declaration_directive_start(text: str) -> str | None:
        for declaration_directive in sorted(
            declaration_directives, key=len, reverse=True
        ):
            if re.match(rf"^{re.escape(declaration_directive)}\b", text):
                return declaration_directive
        return None

    def no_separator_declaration_directive_start(text: str) -> str | None:
        if "==" in text:
            return None
        for declaration_directive in sorted(
            declaration_directives, key=len, reverse=True
        ):
            if not text.startswith(declaration_directive):
                continue
            rest = text[len(declaration_directive) :]
            if rest and not rest[:1].isspace():
                return declaration_directive
            return None
        return None

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        declaration_text = tla_line_without_comment(line)
        stripped = declaration_text.strip()
        if not stripped:
            continue

        parts = stripped.split()
        directive = parts[0]
        if directive in declaration_directives:
            if declaration_text != declaration_text.lstrip():
                close_collecting()
                errors.append(
                    f"{display_path(path)}:{line_number} {label} "
                    f"declaration directive must be top-level: {stripped}"
                )
                continue
            close_collecting()
            rest = stripped[len(directive) :].strip()
            if not rest:
                collecting_label = label
                collecting_line = line_number
                collecting_entries = 0
                collecting_invalid = False
                collecting_pending_comma_line = None
                continue

            names, pending_comma = parse_declaration_line(line_number, rest)
            entries.extend((line_number, name) for name in names)
            if pending_comma:
                collecting_label = label
                collecting_line = line_number
                collecting_entries = len(names)
                collecting_invalid = not names
                collecting_pending_comma_line = line_number
            continue

        if malformed_declaration_directive_start(stripped) is not None:
            close_collecting()
            if declaration_text != declaration_text.lstrip():
                errors.append(
                    f"{display_path(path)}:{line_number} {label} "
                    f"declaration directive must be top-level: {stripped}"
                )
                continue
            errors.append(
                f"{display_path(path)}:{line_number} {label} declaration "
                f"line must list static identifiers: {stripped}"
            )
            continue

        no_separator_directive = no_separator_declaration_directive_start(stripped)
        if collecting_label is None:
            if no_separator_directive is not None:
                if declaration_text != declaration_text.lstrip():
                    continue
                errors.append(
                    f"{display_path(path)}:{line_number} malformed {label} "
                    f"declaration directive {no_separator_directive}: {stripped}"
                )
                continue
            continue
        if (
            no_separator_directive is not None
            and declaration_text == declaration_text.lstrip()
        ):
            close_collecting()
            errors.append(
                f"{display_path(path)}:{line_number} malformed {label} "
                f"declaration directive {no_separator_directive}: {stripped}"
            )
            continue
        if directive in stop_directives or "==" in stripped:
            close_collecting()
            continue
        names, pending_comma = parse_declaration_line(line_number, stripped)
        entries.extend((line_number, name) for name in names)
        if not names:
            collecting_invalid = True
            collecting_pending_comma_line = None
            continue
        collecting_pending_comma_line = None
        collecting_entries += len(names)
        if pending_comma:
            collecting_pending_comma_line = line_number

    close_collecting()
    return entries, errors


@cache
def tla_constant_declaration_entries(path: Path) -> list[tuple[int, str]]:
    entries, _ = tla_declaration_block_entries(
        path,
        frozenset(TLA_CONSTANT_DECLARATION_DIRECTIVES),
        frozenset(TLA_CONSTANT_COLLECTION_STOP_DIRECTIVES),
        "CONSTANTS",
    )
    return entries


def tla_constant_declaration_parse_errors(path: Path) -> list[str]:
    _, errors = tla_declaration_block_entries(
        path,
        frozenset(TLA_CONSTANT_DECLARATION_DIRECTIVES),
        frozenset(TLA_CONSTANT_COLLECTION_STOP_DIRECTIVES),
        "CONSTANTS",
    )
    return errors


@cache
def tla_constant_declarations(path: Path) -> set[str]:
    declarations = {name for _, name in tla_constant_declaration_entries(path)}
    return declarations


def tla_duplicate_constant_declaration_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors = [
        f"{mode}: {error}" for error in tla_constant_declaration_parse_errors(path)
    ]
    seen: dict[str, int] = {}
    for line_number, constant in tla_constant_declaration_entries(path):
        previous_line = seen.get(constant)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA constant declaration {constant} first declared at line "
                f"{previous_line}"
            )
        else:
            seen[constant] = line_number
    return errors


def tla_constant_variable_overlap_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    constant_lines: dict[str, int] = {}
    for line_number, constant in tla_constant_declaration_entries(path):
        constant_lines.setdefault(constant, line_number)

    errors: list[str] = []
    for line_number, variable in tla_variable_declaration_entries(path):
        constant_line = constant_lines.get(variable)
        if constant_line is None:
            continue
        errors.append(
            f"{mode}: {display_path(path)}:{line_number} declares TLA "
            f"variable {variable}, but line {constant_line} already declares "
            "it as a constant"
        )
    return errors


def tla_declaration_operator_overlap_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    declaration_lines: dict[str, tuple[int, str]] = {}
    for line_number, constant in tla_constant_declaration_entries(path):
        declaration_lines.setdefault(constant, (line_number, "constant"))
    for line_number, variable in tla_variable_declaration_entries(path):
        declaration_lines.setdefault(variable, (line_number, "variable"))

    operator_lines: dict[str, tuple[int, str]] = {}
    operator_entries = [
        (line_number, operator, "TLA operator definition")
        for line_number, operator in tla_operator_definition_entries(path)
    ]
    operator_entries.extend(
        (line_number, operator, "TLA RECURSIVE declaration")
        for line_number, operator in tla_recursive_declaration_entries(path)
    )
    for line_number, operator, operator_kind in sorted(operator_entries):
        operator_lines.setdefault(operator, (line_number, operator_kind))

    errors: list[str] = []
    for operator in sorted(operator_lines):
        declaration = declaration_lines.get(operator)
        if declaration is None:
            continue
        declaration_line, declaration_kind = declaration
        operator_line, operator_kind = operator_lines[operator]
        errors.append(
            f"{mode}: {display_path(path)}:{operator_line} {operator_kind} "
            f"{operator} overlaps with {declaration_kind} declaration at "
            f"line {declaration_line}"
        )
    return errors


@cache
def tla_variable_declaration_entries(path: Path) -> list[tuple[int, str]]:
    entries, _ = tla_declaration_block_entries(
        path,
        frozenset(TLA_VARIABLE_DECLARATION_DIRECTIVES),
        frozenset(TLA_VARIABLE_COLLECTION_STOP_DIRECTIVES),
        "VARIABLES",
    )
    return entries


def tla_variable_declaration_parse_errors(path: Path) -> list[str]:
    _, errors = tla_declaration_block_entries(
        path,
        frozenset(TLA_VARIABLE_DECLARATION_DIRECTIVES),
        frozenset(TLA_VARIABLE_COLLECTION_STOP_DIRECTIVES),
        "VARIABLES",
    )
    return errors


@cache
def tla_vars_tuple_entries(
    path: Path,
) -> tuple[list[tuple[int, str]], list[str]]:
    entries: list[tuple[int, str]] = []
    errors: list[str] = []
    definitions: list[tuple[int, str]] = []
    lines = read_text(path).splitlines()

    for index, line in enumerate(lines):
        stripped = tla_line_without_comment(line)
        if stripped.startswith((" ", "\t")):
            continue
        match = TLA_VARS_DEFINITION_RE.match(stripped)
        if match is None:
            if TLA_VARS_DEFINITION_START_RE.match(stripped):
                errors.append(
                    f"{display_path(path)}:{index + 1} malformed vars tuple "
                    f"definition: {stripped.strip()}"
                )
            continue

        body = [match.group(1)]
        if ">>" not in body[0]:
            for continuation in lines[index + 1 :]:
                continuation = tla_line_without_comment(continuation)
                body.append(continuation)
                if ">>" in continuation:
                    break
                if continuation.strip() and not continuation.startswith((" ", "\t")):
                    break
        definitions.append((index + 1, "\n".join(body)))

    if len(definitions) != 1:
        errors.append(
            f"{display_path(path)} defines vars tuple {len(definitions)} times"
        )
        return entries, errors

    line_number, body = definitions[0]
    start = body.find("<<")
    end = body.rfind(">>")
    if (
        start == -1
        or end == -1
        or end <= start
        or body[:start].strip()
        or body[end + 2 :].strip()
    ):
        errors.append(
            f"{display_path(path)}:{line_number} vars must be a static tuple"
        )
        return entries, errors

    content = body[start + 2 : end]
    names = [name.strip() for name in content.split(",")]
    if not names or any(not name for name in names):
        errors.append(
            f"{display_path(path)}:{line_number} vars must list static variables"
        )
        return entries, errors

    for name in names:
        if not TLA_IDENTIFIER_RE.match(name):
            errors.append(
                f"{display_path(path)}:{line_number} vars must list "
                f"static variables: {name}"
            )
        elif not is_tla_user_identifier(name):
            errors.append(
                f"{display_path(path)}:{line_number} vars must list "
                f"non-reserved static variables: {name}"
            )
        else:
            entries.append((line_number, name))
    return entries, errors


def tla_variable_surface_errors(mode: str, path: Path) -> list[str]:
    if not path.exists():
        return []

    errors: list[str] = []
    declarations = tla_variable_declaration_entries(path)
    vars_entries, parse_errors = tla_vars_tuple_entries(path)
    errors.extend(
        f"{mode}: {error}"
        for error in tla_variable_declaration_parse_errors(path)
    )
    errors.extend(f"{mode}: {error}" for error in parse_errors)

    seen_declarations: dict[str, int] = {}
    for line_number, variable in declarations:
        previous_line = seen_declarations.get(variable)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"TLA variable declaration {variable} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_declarations[variable] = line_number

    seen_vars: dict[str, int] = {}
    for line_number, variable in vars_entries:
        previous_line = seen_vars.get(variable)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(path)}:{line_number} repeats "
                f"vars tuple variable {variable} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_vars[variable] = line_number

    declared = set(seen_declarations)
    tupled = set(seen_vars)
    for variable in sorted(declared - tupled):
        errors.append(
            f"{mode}: {display_path(path)} declares variable {variable} "
            "but vars does not include it"
        )
    for variable in sorted(tupled - declared):
        errors.append(
            f"{mode}: {display_path(path)} vars includes undeclared variable "
            f"{variable}"
        )
    return errors


def tla_module_validation_errors(mode: str, path: Path) -> list[str]:
    marker_prefix = f"{TLA_MODULE_VALIDATION_MODE_MARKER}:"
    mode_prefix = f"{mode}:"
    return [
        error.replace(marker_prefix, mode_prefix, 1)
        for error in tla_module_validation_error_templates(path)
    ]


@cache
def tla_module_validation_error_templates(path: Path) -> tuple[str, ...]:
    return tuple(
        tla_module_validation_errors_uncached(
            TLA_MODULE_VALIDATION_MODE_MARKER,
            path,
        )
    )


def tla_module_validation_errors_uncached(mode: str, path: Path) -> list[str]:
    errors: list[str] = []
    module_files = list(tla_reachable_module_files(path))
    errors.extend(tla_module_header_errors(mode, module_files))
    for module_file in module_files:
        errors.extend(tla_module_dependency_errors(mode, module_file))
        errors.extend(tla_instance_alias_namespace_errors(mode, module_file))
        errors.extend(tla_forbidden_directive_errors(mode, module_file))
        errors.extend(tla_duplicate_constant_declaration_errors(mode, module_file))
        errors.extend(tla_duplicate_operator_definition_errors(mode, module_file))
        errors.extend(tla_variable_surface_errors(mode, module_file))
        errors.extend(tla_constant_variable_overlap_errors(mode, module_file))
        errors.extend(tla_declaration_operator_overlap_errors(mode, module_file))
    return errors


@cache
def cfg_constant_binding_values(
    path: Path,
) -> tuple[list[tuple[int, str, str]], list[str]]:
    bindings: list[tuple[int, str, str]] = []
    errors: list[str] = []
    collecting = False
    collecting_line: int | None = None
    collecting_entries = 0
    collecting_invalid = False

    def close_collecting() -> None:
        nonlocal collecting, collecting_line, collecting_entries, collecting_invalid
        if (
            collecting
            and collecting_line is not None
            and collecting_entries == 0
            and not collecting_invalid
        ):
            errors.append(
                f"{display_path(path)}:{collecting_line} CONSTANTS block "
                "must bind at least one constant"
            )
        collecting = False
        collecting_line = None
        collecting_entries = 0
        collecting_invalid = False

    def parse_binding(
        line_number: int,
        text: str,
        context: str,
    ) -> tuple[str, str] | None:
        match = CFG_CONSTANT_BINDING_LINE_RE.match(text)
        if match is None:
            errors.append(
                f"{display_path(path)}:{line_number} {context} "
                "must bind exactly one constant"
            )
            return None
        constant = match.group(1)
        if not is_tla_user_identifier(constant):
            errors.append(
                f"{display_path(path)}:{line_number} {context} "
                f"must bind a non-reserved static constant: {constant}"
            )
            return None
        rhs = match.group(2).strip()
        nested_match = CFG_NESTED_CONSTANT_BINDING_RE.search(rhs)
        if nested_match is not None:
            errors.append(
                f"{display_path(path)}:{line_number} {context} contains "
                f"nested binding-looking token {nested_match.group(2)}"
            )
            return None
        return constant, rhs

    def malformed_constant_directive_start(text: str) -> bool:
        return any(
            re.match(rf"^{re.escape(directive)}\b", text)
            for directive in CFG_CONSTANT_DIRECTIVES
        )

    def no_separator_constant_directive_start(text: str) -> str | None:
        for constant_directive in sorted(CFG_CONSTANT_DIRECTIVES, key=len, reverse=True):
            if not text.startswith(constant_directive):
                continue
            rest = text[len(constant_directive) :]
            if rest and not rest[:1].isspace():
                return constant_directive
            return None
        return None

    def indented_no_separator_constant_directive_start(text: str) -> str | None:
        directive = no_separator_constant_directive_start(text)
        if directive is None:
            return None
        rest = text[len(directive) :]
        if rest.startswith("_"):
            return None
        return directive

    for line_number, line in enumerate(read_text(path).splitlines(), 1):
        stripped = tla_line_without_comment(line).strip()
        if not stripped:
            close_collecting()
            continue

        parts = stripped.split()
        directive = parts[0]
        if indented_cfg_directive(line, directive):
            errors.append(
                f"{display_path(path)}:{line_number} indented CFG directive "
                f"{directive} must be top-level"
            )
            collecting_invalid = True
            close_collecting()
            continue

        if directive in CFG_CONSTANT_DIRECTIVES:
            close_collecting()
            rest = stripped[len(directive) :].strip()
            if not rest:
                collecting = True
                collecting_line = line_number
                collecting_entries = 0
                collecting_invalid = False
                continue
            binding = parse_binding(
                line_number, rest, f"directive {directive}"
            )
            if binding is not None:
                constant, value = binding
                bindings.append((line_number, constant, value))
            continue

        if malformed_constant_directive_start(stripped):
            close_collecting()
            if line[:1].isspace():
                errors.append(
                    f"{display_path(path)}:{line_number} indented CFG directive "
                    f"{directive} must be top-level"
                )
                continue
            errors.append(
                f"{display_path(path)}:{line_number} directive {directive} "
                "must bind exactly one constant"
            )
            continue

        no_separator_directive = no_separator_constant_directive_start(stripped)
        if no_separator_directive is not None and not line[:1].isspace():
            close_collecting()
            errors.append(
                f"{display_path(path)}:{line_number} malformed CFG constant "
                f"binding directive {no_separator_directive}: {stripped}"
            )
            continue

        if not collecting:
            continue
        if not line[:1].isspace():
            close_collecting()
            continue
        no_separator_directive = indented_no_separator_constant_directive_start(
            stripped
        )
        if no_separator_directive is not None:
            errors.append(
                f"{display_path(path)}:{line_number} indented CFG directive "
                f"{no_separator_directive} must be top-level"
            )
            collecting_invalid = True
            close_collecting()
            continue
        binding = parse_binding(line_number, stripped, "CONSTANTS block line")
        if binding is None:
            collecting_invalid = True
            continue
        constant, value = binding
        bindings.append((line_number, constant, value))
        collecting_entries += 1

    close_collecting()
    return bindings, errors


@cache
def cfg_constant_bindings(path: Path) -> tuple[list[tuple[int, str]], list[str]]:
    bindings, errors = cfg_constant_binding_values(path)
    return [
        (line_number, constant)
        for line_number, constant, _value in bindings
    ], errors


def cfg_constant_binding_errors(mode: str, module_path: Path, cfg_path: Path) -> list[str]:
    if not module_path.exists() or not cfg_path.exists():
        return []

    bindings, parse_errors = cfg_constant_bindings(cfg_path)
    declarations = tla_constant_declarations(module_path)
    errors = [f"{mode}: {error}" for error in parse_errors]
    bound_constants: set[str] = set()
    for line_number, constant in bindings:
        bound_constants.add(constant)
        if constant not in declarations:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} binds constant "
                f"{constant}, but {display_path(module_path)} does not declare it"
            )
    for constant in sorted(declarations - bound_constants):
        errors.append(
            f"{mode}: {display_path(cfg_path)} does not bind constant {constant} "
            f"declared by {display_path(module_path)}"
        )
    return errors


def cfg_module_ownership_errors(
    mode: str,
    module_path: Path,
    cfg_path: Path,
) -> list[str]:
    if (
        cfg_path.stem == module_path.stem
        or cfg_path.stem.startswith(f"{module_path.stem}_")
    ):
        return []
    return [
        f"{mode}: CFG {display_path(cfg_path)} does not belong to TLA module "
        f"{display_path(module_path)}; expected filename stem {module_path.stem} "
        f"or {module_path.stem}_*"
    ]


def cfg_duplicate_constant_binding_errors(mode: str, cfg_path: Path) -> list[str]:
    if not cfg_path.exists():
        return []

    bindings, parse_errors = cfg_constant_bindings(cfg_path)
    if parse_errors:
        return []

    errors: list[str] = []
    seen: dict[str, int] = {}
    for line_number, constant in bindings:
        previous_line = seen.get(constant)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} repeats "
                f"constant binding {constant} first declared at line "
                f"{previous_line}"
            )
        else:
            seen[constant] = line_number
    return errors


def cfg_operator_reference_errors(mode: str, module_path: Path, cfg_path: Path) -> list[str]:
    if not module_path.exists() or not cfg_path.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_path)
    definitions = tla_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    errors = [f"{mode}: {error}" for error in parse_errors]
    for line_number, directive, operator in references:
        if operator not in definitions:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} references "
                f"{directive} operator {operator}, but {display_path(module_path)} "
                f"does not define it"
            )
            continue
        signature = signatures.get(operator)
        if signature is not None and signature[1] != 0:
            definition_line, arity = signature
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} references "
                f"{directive} operator {operator}, but "
                f"{display_path(module_path)}:{definition_line} defines it "
                f"with arity {arity}; CFG references must target zero-arity "
                "operators"
            )
    return errors


def normalized_cfg_check_directive(directive: str) -> str:
    if directive in {"INVARIANT", "INVARIANTS"}:
        return "INVARIANT"
    if directive in {"PROPERTY", "PROPERTIES"}:
        return "PROPERTY"
    return directive


def cfg_duplicate_operator_reference_errors(mode: str, cfg_path: Path) -> list[str]:
    if not cfg_path.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_path)
    if parse_errors:
        return []

    errors: list[str] = []
    seen_singleton: dict[str, int] = {}
    seen_checks: dict[tuple[str, str], int] = {}
    seen_check_kinds: dict[str, tuple[str, int]] = {}
    seen_roles: dict[str, tuple[str, str, int]] = {}

    def record_role(
        line_number: int,
        operator: str,
        role_key: str,
        role_label: str,
    ) -> None:
        previous = seen_roles.get(operator)
        if previous is None:
            seen_roles[operator] = (role_key, role_label, line_number)
            return
        previous_key, previous_label, previous_line = previous
        if previous_key == role_key:
            return
        errors.append(
            f"{mode}: {display_path(cfg_path)}:{line_number} references "
            f"{role_label} {operator}, but line {previous_line} already "
            f"references it as {previous_label}; CFG behavior, constraint, "
            "and proof targets must be role-disjoint"
        )

    for line_number, directive, operator in references:
        if directive in {"SPECIFICATION", "INIT", "NEXT", "CONSTRAINT"}:
            record_role(line_number, operator, directive, f"{directive} operator")
            previous_line = seen_singleton.get(directive)
            if previous_line is not None:
                label = (
                    f"{directive} behavior directive"
                    if directive in {"SPECIFICATION", "INIT", "NEXT"}
                    else f"{directive} directive"
                )
                errors.append(
                    f"{mode}: {display_path(cfg_path)}:{line_number} repeats "
                    f"{label} first declared at line {previous_line}"
                )
            else:
                seen_singleton[directive] = line_number
            continue

        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        normalized = normalized_cfg_check_directive(directive)
        record_role(line_number, operator, "CHECK", f"{normalized} check")
        key = (normalized, operator)
        previous_line = seen_checks.get(key)
        if previous_line is not None:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} repeats "
                f"{normalized} check {operator} first declared at line "
                f"{previous_line}"
            )
        else:
            seen_checks[key] = line_number
        previous_kind = seen_check_kinds.get(operator)
        if previous_kind is None:
            seen_check_kinds[operator] = (normalized, line_number)
            continue
        previous_normalized, previous_kind_line = previous_kind
        if previous_normalized != normalized:
            errors.append(
                f"{mode}: {display_path(cfg_path)}:{line_number} references "
                f"{normalized} check {operator}, but line {previous_kind_line} "
                f"already references it as {previous_normalized}; CFG proof "
                "targets must not be both INVARIANT and PROPERTY"
            )
    return errors


def cfg_semantic_check_errors(
    mode: str,
    cfg_file: Path,
    runner_name: str,
) -> list[str]:
    if not cfg_file.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_file)
    if parse_errors:
        return []
    checks = [
        operator
        for _, directive, operator in references
        if directive in CFG_CHECK_DIRECTIVES
    ]
    semantic_checks = [operator for operator in checks if operator != "TypeInvariant"]
    if semantic_checks:
        return []
    return [
        f"{mode}: {runner_name} cfg {display_path(cfg_file)} "
        "has no non-TypeInvariant invariant/property check"
    ]


def cfg_fast_generic_check_errors(
    mode: str,
    cfg_file: Path,
    runner_name: str,
) -> list[str]:
    if not cfg_file.name.endswith("_fast.cfg") or "_bug_" in cfg_file.name:
        return []

    references, parse_errors = cfg_operator_references(cfg_file)
    if parse_errors:
        return []

    errors: list[str] = []
    has_correctness_envelope = False
    for line_number, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        if operator.endswith("CorrectnessEnvelope"):
            has_correctness_envelope = True
        if operator not in GENERIC_CORRECTNESS_CHECKS:
            continue
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_file)}:{line_number} "
            f"references generic check {operator}; fast configs must use a "
            "model-specific direct invariant"
        )
    if not has_correctness_envelope:
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_file)} has no "
            "model-specific *CorrectnessEnvelope invariant/property check"
        )
    return errors


def tla_static_identifiers(expression: str) -> set[str]:
    """Return static TLA identifiers mentioned in an expression body."""

    return {
        identifier
        for identifier in TLA_IDENTIFIER_SCAN_RE.findall(expression)
        if is_tla_user_identifier(identifier)
    }


def tla_static_non_string_identifiers(expression: str) -> set[str]:
    """Return static TLA identifiers outside quoted string literals."""

    return tla_static_identifiers(tla_without_string_literals(expression))


def tla_free_static_identifiers(
    expression: str,
    bound: frozenset[str] = frozenset(),
) -> set[str]:
    """Return static TLA identifiers not hidden by quantified binders."""

    normalized = strip_static_outer_parentheses(" ".join(expression.split()))
    if not normalized:
        return set()

    set_scope = tla_set_comprehension_scope(normalized)
    if set_scope is not None:
        domains, body, local_bound = set_scope
        identifiers: set[str] = set()
        for domain in domains:
            identifiers.update(tla_free_static_identifiers(domain, bound))
        identifiers.update(
            tla_free_static_identifiers(body, bound | frozenset(local_bound))
        )
        return identifiers

    set_elements = tla_explicit_set_elements(normalized)
    if set_elements is not None:
        identifiers: set[str] = set()
        for element in set_elements:
            identifiers.update(tla_free_static_identifiers(element, bound))
        return identifiers

    function_scope = tla_function_constructor_scope(normalized)
    if function_scope is not None:
        domains, body, local_bound = function_scope
        identifiers: set[str] = set()
        for domain in domains:
            identifiers.update(tla_free_static_identifiers(domain, bound))
        identifiers.update(
            tla_free_static_identifiers(body, bound | frozenset(local_bound))
        )
        return identifiers

    function_set_scope = tla_function_set_scope(normalized)
    if function_set_scope is not None:
        domain, range_expression = function_set_scope
        identifiers: set[str] = set()
        identifiers.update(tla_free_static_identifiers(domain, bound))
        identifiers.update(tla_free_static_identifiers(range_expression, bound))
        return identifiers

    record_values = tla_record_literal_values(normalized)
    if record_values is not None:
        identifiers: set[str] = set()
        for value in record_values:
            identifiers.update(tla_free_static_identifiers(value, bound))
        return identifiers

    record_domains = tla_record_set_field_domains(normalized)
    if record_domains is not None:
        identifiers: set[str] = set()
        for domain in record_domains:
            identifiers.update(tla_free_static_identifiers(domain, bound))
        return identifiers

    record_update = tla_record_update_scope(normalized)
    if record_update is not None:
        base, selectors, replacements = record_update
        identifiers: set[str] = set()
        identifiers.update(tla_free_static_identifiers(base, bound))
        for selector in selectors:
            identifiers.update(tla_free_static_identifiers(selector, bound))
        for replacement in replacements:
            identifiers.update(tla_free_static_identifiers(replacement, bound))
        return identifiers

    tuple_values = tla_tuple_literal_values(normalized)
    if tuple_values is not None:
        identifiers: set[str] = set()
        for value in tuple_values:
            identifiers.update(tla_free_static_identifiers(value, bound))
        return identifiers

    lambda_scope = tla_lambda_scope(normalized)
    if lambda_scope is not None:
        domains, body, local_bound = lambda_scope
        identifiers: set[str] = set()
        for domain in domains:
            identifiers.update(tla_free_static_identifiers(domain, bound))
        identifiers.update(
            tla_free_static_identifiers(body, bound | frozenset(local_bound))
        )
        return identifiers

    choose_split = tla_choose_prefix_and_body(normalized)
    if choose_split is not None:
        prefix, body = choose_split
        local_bound = tla_choose_binding_identifiers(prefix)
        identifiers: set[str] = set()
        if local_bound is not None:
            for domain in tla_choose_bound_domains(prefix):
                identifiers.update(tla_free_static_identifiers(domain, bound))
            identifiers.update(
                tla_free_static_identifiers(body, bound | frozenset(local_bound))
            )
            return identifiers
        prefix_expression = re.sub(
            r"^CHOOSE\s+", "", prefix.strip(), count=1
        ).strip()
        for binding in tla_top_level_argument_parts(prefix_expression):
            identifiers.update(tla_free_static_identifiers(binding, bound))
        identifiers.update(tla_free_static_identifiers(body, bound))
        return identifiers

    split = quantified_formula_prefix_and_body(normalized)
    if split is not None:
        quantifier_scope = tla_quantifier_scope(normalized)
        identifiers: set[str] = set()
        if quantifier_scope is not None:
            domains, body, local_bound = quantifier_scope
            for domain in domains:
                identifiers.update(tla_free_static_identifiers(domain, bound))
            identifiers.update(
                tla_free_static_identifiers(body, bound | frozenset(local_bound))
            )
            return identifiers
        prefix, body = split
        prefix_expression = re.sub(
            r"^\\[AE]\s+", "", prefix.strip(), count=1
        ).strip()
        for binding in tla_top_level_argument_parts(prefix_expression):
            identifiers.update(tla_free_static_identifiers(binding, bound))
        identifiers.update(tla_free_static_identifiers(body, bound))
        return identifiers

    if re.match(r"^LET\b", normalized):
        in_index = tla_top_level_keyword_index(normalized, "IN", start=len("LET"))
        if in_index is not None:
            binding = normalized[len("LET") : in_index].strip()
            result = strip_static_outer_parentheses(
                normalized[in_index + len("IN") :].strip()
            )
            let_bindings = tla_static_let_binding_entries(binding)
            if let_bindings:
                let_bound = frozenset(entry.name for entry in let_bindings)
                identifiers: set[str] = set()
                for entry in let_bindings:
                    identifiers.update(
                        tla_free_static_identifiers(
                            entry.operand,
                            bound | let_bound | entry.params,
                        )
                    )
                identifiers.update(
                    tla_free_static_identifiers(result, bound | let_bound)
                )
                return identifiers

    case_branches = tla_top_level_case_condition_result_branches(normalized)
    if case_branches:
        identifiers: set[str] = set()
        for condition, result in case_branches:
            identifiers.update(tla_free_static_identifiers(condition, bound))
            identifiers.update(tla_free_static_identifiers(result, bound))
        return identifiers

    boolean_parts = tla_top_level_boolean_parts(normalized)
    if len(boolean_parts) > 1:
        identifiers: set[str] = set()
        for part in boolean_parts:
            identifiers.update(tla_free_static_identifiers(part, bound))
        return identifiers

    negated_operand = tla_static_negation_operand(normalized)
    if negated_operand is not None:
        return tla_free_static_identifiers(negated_operand, bound)

    temporal_operand = tla_unary_temporal_operand(normalized)
    if temporal_operand is not None:
        return tla_free_static_identifiers(temporal_operand, bound)

    action_operand = tla_unary_action_operand(normalized)
    if action_operand is not None:
        return tla_free_static_identifiers(action_operand, bound)

    unary_set_operand = tla_unary_set_operator_operand(normalized)
    if unary_set_operand is not None:
        return tla_free_static_identifiers(unary_set_operand, bound)

    if_parts = tla_top_level_if_parts(normalized)
    if if_parts is not None:
        identifiers: set[str] = set()
        for part in if_parts:
            identifiers.update(tla_free_static_identifiers(part, bound))
        return identifiers

    relation_parts = tla_top_level_relation_parts(normalized)
    if relation_parts is not None:
        left, _, right = relation_parts
        identifiers: set[str] = set()
        identifiers.update(tla_free_static_identifiers(left, bound))
        identifiers.update(tla_free_static_identifiers(right, bound))
        return identifiers

    infix_operands = tla_top_level_static_infix_operands(normalized)
    if infix_operands is not None:
        identifiers: set[str] = set()
        for operand in infix_operands:
            identifiers.update(tla_free_static_identifiers(operand, bound))
        return identifiers

    call_arguments = tla_direct_operator_call_arguments(normalized)
    if call_arguments is not None:
        callee = tla_direct_operator_call_name(normalized)
        identifiers: set[str] = set()
        if callee is not None and callee not in bound:
            identifiers.add(callee)
        for argument in tla_top_level_argument_parts(call_arguments):
            if argument:
                identifiers.update(tla_free_static_identifiers(argument, bound))
        return identifiers

    selector_scope = tla_selector_scope(normalized)
    if selector_scope is not None:
        base, selectors = selector_scope
        identifiers: set[str] = set()
        identifiers.update(tla_free_static_identifiers(base, bound))
        for selector in selectors:
            identifiers.update(tla_free_static_identifiers(selector, bound))
        return identifiers

    return tla_static_non_string_identifiers(normalized) - set(bound)


def tla_without_string_literals(expression: str) -> str:
    """Return expression text with quoted string contents blanked out."""

    chars: list[str] = []
    in_string = False
    escaped = False
    for char in expression:
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            chars.append(" ")
            continue
        if char == '"':
            in_string = True
            chars.append(" ")
            continue
        chars.append(char)
    return "".join(chars)


def tla_quantified_bound_identifiers(expression: str) -> set[str]:
    """Return identifiers bound by simple TLA quantifier clauses."""

    bound: set[str] = set()
    for prefix in tla_quantifier_prefixes(expression):
        bound.update(tla_quantifier_binding_identifiers(prefix))
    return bound


def tla_quantifier_prefixes(expression: str) -> list[str]:
    """Return quantifier prefixes from a static expression."""

    prefixes: list[str] = []
    index = 0
    while index < len(expression):
        start: int | None = None
        in_string = False
        escaped = False
        scan = index
        while scan < len(expression):
            char = expression[scan]
            if in_string:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    in_string = False
                scan += 1
                continue
            if char == '"':
                in_string = True
                scan += 1
                continue
            if (
                expression.startswith("\\A", scan)
                or expression.startswith("\\E", scan)
            ) and (
                scan + 2 == len(expression)
                or not (
                    expression[scan + 2].isalnum()
                    or expression[scan + 2] == "_"
                )
            ):
                start = scan
                break
            scan += 1
        if start is None:
            break
        depth = 0
        in_string = False
        escaped = False
        scan = start
        while scan < len(expression):
            char = expression[scan]
            if in_string:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    in_string = False
                scan += 1
                continue
            if char == '"':
                in_string = True
                scan += 1
                continue
            if expression.startswith("<<", scan):
                depth += 1
                scan += 2
                continue
            if expression.startswith(">>", scan) and depth > 0:
                depth -= 1
                scan += 2
                continue
            if char in "([{":
                depth += 1
                scan += 1
                continue
            if char in ")]}" and depth > 0:
                depth -= 1
                scan += 1
                continue
            if depth == 0 and char == ":":
                prefixes.append(expression[start:scan].strip())
                index = scan + 1
                break
            scan += 1
        else:
            index = start + 2
    return prefixes


def tla_quantifier_binding_identifiers(prefix: str) -> set[str]:
    """Return bound identifiers declared by a quantifier prefix."""

    text = re.sub(
        r"^\\[AE]\s+",
        "",
        prefix.strip(),
        count=1,
    ).strip()
    if not text:
        return set()

    _, bound = tla_binding_domains_from_prefix(text)
    if bound:
        return bound
    if tla_binding_prefix_has_relation(text):
        return set()

    bound: set[str] = set()
    for binding in tla_top_level_argument_parts(text):
        bound.update(tla_binding_identifiers_from_names(binding))
    return bound


def tla_quantifier_scope(
    expression: str,
) -> tuple[list[str], str, set[str]] | None:
    """Return domains, body, and local binders for a whole quantified formula."""

    split = quantified_formula_prefix_and_body(expression)
    if split is None:
        return None
    prefix, body = split
    text = re.sub(r"^\\[AE]\s+", "", prefix.strip(), count=1).strip()
    if not text:
        return None

    domains, bound = tla_binding_domains_from_prefix(text)
    if domains and bound:
        return domains, body, bound
    if tla_binding_prefix_has_relation(text):
        return None

    local_bound: set[str] = set()
    for binding in tla_top_level_argument_parts(text):
        if not binding:
            return None
        binding_bound = tla_binding_identifiers_from_names(binding)
        if not binding_bound:
            return None
        local_bound.update(binding_bound)
    if not local_bound:
        return None
    return [], body, local_bound


def tla_choose_prefix_and_body(expression: str) -> tuple[str, str] | None:
    """Return a whole-body CHOOSE prefix and body, if present."""

    normalized = strip_static_outer_parentheses(" ".join(expression.split()))
    if not re.match(r"^CHOOSE\b", normalized):
        return None
    depth = 0
    in_string = False
    escaped = False
    index = len("CHOOSE")
    while index < len(normalized):
        char = normalized[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if normalized.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if normalized.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth == 0 and char == ":":
            prefix = normalized[:index].strip()
            body = normalized[index + 1 :].strip()
            if not prefix or not body:
                return None
            return prefix, strip_static_outer_parentheses(body)
        index += 1
    return None


def tla_choose_binding_identifiers(prefix: str) -> set[str] | None:
    """Return identifiers bound by a simple CHOOSE prefix."""

    text = re.sub(r"^CHOOSE\s+", "", prefix.strip(), count=1).strip()
    if not text:
        return None
    bound: set[str] = set()
    for binding in tla_top_level_argument_parts(text):
        membership = tla_top_level_membership_parts(binding)
        if membership is not None:
            if membership[1] != "\\in":
                return None
            names = membership[0]
        elif tla_top_level_relation_parts(binding) is not None:
            return None
        else:
            names = binding
        for identifier in TLA_IDENTIFIER_SCAN_RE.findall(
            tla_without_string_literals(names)
        ):
            if is_tla_user_identifier(identifier):
                bound.add(identifier)
    return bound or None


def tla_choose_bound_domains(prefix: str) -> list[str]:
    """Return explicit domains from a simple CHOOSE prefix."""

    text = re.sub(r"^CHOOSE\s+", "", prefix.strip(), count=1).strip()
    domains: list[str] = []
    for binding in tla_top_level_argument_parts(text):
        membership = tla_top_level_membership_parts(binding)
        if membership is not None and membership[1] == "\\in":
            domains.append(membership[2])
    return domains


def tla_lambda_scope(expression: str) -> tuple[list[str], str, set[str]] | None:
    """Return domains, body, and local binders for simple LAMBDA expressions."""

    normalized = strip_static_outer_parentheses(" ".join(expression.split()))
    if not re.match(r"^LAMBDA\b", normalized):
        return None
    colon_index = tla_top_level_symbol_index(normalized, ":", start=len("LAMBDA"))
    if colon_index is None:
        return None
    prefix = normalized[len("LAMBDA") : colon_index].strip()
    body = normalized[colon_index + 1 :].strip()
    if not prefix or not body:
        return None

    domains, bound = tla_binding_domains_from_prefix(prefix)
    if domains and bound:
        return domains, strip_static_outer_parentheses(body), bound

    if tla_binding_prefix_has_relation(prefix):
        return None

    local_bound: set[str] = set()
    for part in tla_top_level_argument_parts(prefix):
        if not part:
            return None
        part_bound = tla_binding_identifiers_from_names(part)
        if not part_bound:
            return None
        local_bound.update(part_bound)
    if not local_bound:
        return None
    return [], strip_static_outer_parentheses(body), local_bound


def tla_binding_prefix_has_relation(prefix: str) -> bool:
    """Return whether a would-be plain binding prefix contains a relation."""

    return any(
        tla_top_level_relation_parts(part) is not None
        for part in tla_top_level_argument_parts(prefix)
    )


def tla_top_level_symbol_index(text: str, symbol: str, start: int = 0) -> int | None:
    """Return a top-level symbol occurrence, preserving TLA delimiters."""

    depth = 0
    in_string = False
    escaped = False
    index = start
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth == 0 and text.startswith(symbol, index):
            return index
        index += 1
    return None


def tla_top_level_function_set_arrow_index(text: str) -> int | None:
    """Return a top-level function-set arrow, excluding record/function constructors."""

    case_branch_arrows = tla_top_level_case_branch_arrow_indices(text)
    start = 0
    while True:
        index = tla_top_level_symbol_index(text, "->", start=start)
        if index is None:
            return None
        previous = text[index - 1] if index > 0 else ""
        if previous != "|" and index not in case_branch_arrows:
            return index
        start = index + len("->")


def tla_top_level_case_branch_arrow_indices(text: str) -> set[int]:
    """Return absolute indexes of top-level CASE arm arrows."""

    if not re.match(r"^CASE\b", text):
        return set()

    arrows: set[int] = set()
    current: list[str] = []
    depth = 0
    in_string = False
    escaped = False
    arm_arrow_seen = False
    index = len("CASE")
    while index < len(text):
        char = text[index]
        if in_string:
            current.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            current.append(char)
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            current.append("<<")
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            current.append(">>")
            index += 2
            continue
        if depth == 0 and text.startswith("[]", index) and (
            tla_case_arm_has_result("".join(current))
        ):
            current = []
            arm_arrow_seen = False
            index += 2
            continue
        if depth == 0 and text.startswith("->", index):
            previous = text[index - 1] if index > 0 else ""
            if previous != "|" and not arm_arrow_seen:
                arrows.add(index)
                arm_arrow_seen = True
            current.append("->")
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        current.append(char)
        index += 1

    return arrows


def tla_delimited_expression_end(
    text: str,
    start: int,
    opener: str,
    closer: str,
) -> int | None:
    """Return the matching closing delimiter for a TLA expression."""

    if start >= len(text) or text[start] != opener:
        return None

    depth = 1
    in_string = False
    escaped = False
    index = start + 1
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}":
            depth -= 1
            if depth == 0:
                return index if char == closer else None
            index += 1
            continue
        index += 1
    return None


def tla_square_bracket_expression_end(text: str, start: int) -> int | None:
    """Return the matching closing bracket for a square-bracket expression."""

    return tla_delimited_expression_end(text, start, "[", "]")


def tla_outer_square_brackets_enclose_expression(text: str) -> bool:
    """Return whether outer square brackets enclose the full expression."""

    return (
        text.startswith("[")
        and text.endswith("]")
        and tla_square_bracket_expression_end(text, 0) == len(text) - 1
    )


def tla_curly_brace_expression_end(text: str, start: int) -> int | None:
    """Return the matching closing brace for a curly-brace expression."""

    return tla_delimited_expression_end(text, start, "{", "}")


def tla_outer_curly_braces_enclose_expression(text: str) -> bool:
    """Return whether outer curly braces enclose the full expression."""

    return (
        text.startswith("{")
        and text.endswith("}")
        and tla_curly_brace_expression_end(text, 0) == len(text) - 1
    )


def tla_tuple_expression_end(text: str, start: int) -> int | None:
    """Return the matching closing token for a tuple expression."""

    if start >= len(text) or not text.startswith("<<", start):
        return None

    depth = 1
    in_string = False
    escaped = False
    index = start + 2
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index):
            depth -= 1
            if depth == 0:
                return index
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}":
            depth -= 1
            if depth <= 0:
                return None
            index += 1
            continue
        index += 1
    return None


def tla_outer_tuple_brackets_enclose_expression(text: str) -> bool:
    """Return whether outer tuple brackets enclose the full expression."""

    return (
        text.startswith("<<")
        and text.endswith(">>")
        and tla_tuple_expression_end(text, 0) == len(text) - 2
    )


def tla_binding_identifiers_from_names(names: str) -> set[str]:
    """Return identifiers bound by a static binding-name pattern."""

    return set(tla_binding_identifier_sequence_from_names(names))


def tla_binding_identifier_sequence_from_names(names: str) -> list[str]:
    """Return bound identifiers from a static binding-name pattern in order."""

    text = strip_static_outer_parentheses(" ".join(names.split()))
    if not text:
        return []
    if TLA_IDENTIFIER_RE.fullmatch(text) and is_tla_user_identifier(text):
        return [text]
    if not tla_outer_tuple_brackets_enclose_expression(text):
        return []

    identifiers: list[str] = []
    for part in tla_top_level_argument_parts(text[2:-2]):
        nested = tla_binding_identifier_sequence_from_names(part)
        if not nested:
            return []
        identifiers.extend(nested)
    return identifiers


def tla_binding_identifier_sequence_from_prefix(prefix: str) -> list[str]:
    """Return bound identifiers from a whole quantifier binding prefix."""

    text = prefix.strip()
    if not text:
        return []

    identifiers: list[str] = []
    pending_names: list[str] = []
    saw_domain = False
    for binding in tla_top_level_argument_parts(text):
        membership = tla_top_level_membership_parts(binding)
        if membership is None:
            pending_names.append(binding)
            continue
        if membership[1] != "\\in":
            return []
        saw_domain = True
        names, _, _ = membership
        for name_part in [*pending_names, names]:
            name_identifiers = tla_binding_identifier_sequence_from_names(name_part)
            if not name_identifiers:
                return []
            identifiers.extend(name_identifiers)
        pending_names = []

    if pending_names and saw_domain:
        return []
    if not saw_domain and tla_binding_prefix_has_relation(text):
        return []
    for name_part in pending_names:
        name_identifiers = tla_binding_identifier_sequence_from_names(name_part)
        if not name_identifiers:
            return []
        identifiers.extend(name_identifiers)
    return identifiers


def tla_binding_domains_from_prefix(prefix: str) -> tuple[list[str], set[str]]:
    """Return domains and local binders from top-level membership bindings."""

    domains: list[str] = []
    bound: set[str] = set()
    pending_names: list[str] = []
    for binding in tla_top_level_argument_parts(prefix):
        membership = tla_top_level_membership_parts(binding)
        if membership is None:
            pending_names.append(binding)
            continue
        if membership[1] != "\\in":
            return [], set()
        names, _, domain = membership
        local_bound: set[str] = set()
        for name_part in [*pending_names, names]:
            name_bound = tla_binding_identifiers_from_names(name_part)
            if not name_bound:
                return [], set()
            local_bound.update(name_bound)
        if not local_bound:
            return [], set()
        bound.update(local_bound)
        domains.append(domain)
        pending_names = []
    if pending_names:
        return [], set()
    return domains, bound


def tla_set_comprehension_scope(
    expression: str,
) -> tuple[list[str], str, set[str]] | None:
    """Return domains, body, and local binders for simple set comprehensions."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_curly_braces_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return None
    colon_index = tla_top_level_symbol_index(inner, ":")
    if colon_index is None:
        return None
    left = inner[:colon_index].strip()
    right = inner[colon_index + 1 :].strip()
    if not left or not right:
        return None

    domains, bound = tla_binding_domains_from_prefix(left)
    if domains and bound:
        return domains, right, bound

    domains, bound = tla_binding_domains_from_prefix(right)
    if domains and bound:
        return domains, left, bound

    return None


def tla_function_constructor_scope(
    expression: str,
) -> tuple[list[str], str, set[str]] | None:
    """Return domains, body, and local binders for function constructors."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_square_brackets_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return None
    arrow_index = tla_top_level_symbol_index(inner, "|->")
    if arrow_index is None:
        return None
    prefix = inner[:arrow_index].strip()
    body = inner[arrow_index + len("|->") :].strip()
    if not prefix or not body:
        return None
    domains, bound = tla_binding_domains_from_prefix(prefix)
    if not domains or not bound:
        return None
    return domains, body, bound


def tla_function_set_scope(expression: str) -> tuple[str, str] | None:
    """Return domain and range expressions for whole-expression function sets."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_square_brackets_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return None
    if tla_top_level_symbol_index(inner, "|->") is not None:
        return None
    arrow_index = tla_top_level_function_set_arrow_index(inner)
    if arrow_index is None:
        return None
    colon_index = tla_top_level_symbol_index(inner, ":")
    if colon_index is not None and colon_index < arrow_index:
        return None
    except_index = tla_top_level_keyword_index(inner, "EXCEPT")
    if except_index is not None and except_index < arrow_index:
        return None
    domain = inner[:arrow_index].strip()
    range_expression = inner[arrow_index + len("->") :].strip()
    if not domain or not range_expression:
        return None
    return domain, range_expression


def tla_record_literal_values(expression: str) -> list[str] | None:
    """Return record-literal values while ignoring static field labels."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_square_brackets_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return None

    values: list[str] = []
    for entry in tla_top_level_argument_parts(inner):
        arrow_index = tla_top_level_symbol_index(entry, "|->")
        if arrow_index is None:
            return None
        field = entry[:arrow_index].strip()
        value = entry[arrow_index + len("|->") :].strip()
        if not field or not value:
            return None
        if TLA_IDENTIFIER_RE.fullmatch(field) is None or not is_tla_user_identifier(
            field
        ):
            return None
        values.append(value)
    return values


def tla_record_set_field_domains(expression: str) -> list[str] | None:
    """Return record-set domains while ignoring static field labels."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_square_brackets_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return None

    domains: list[str] = []
    for entry in tla_top_level_argument_parts(inner):
        colon_index = tla_top_level_symbol_index(entry, ":")
        if colon_index is None:
            return None
        field = entry[:colon_index].strip()
        domain = entry[colon_index + 1 :].strip()
        if not field or not domain:
            return None
        if TLA_IDENTIFIER_RE.fullmatch(field) is None or not is_tla_user_identifier(
            field
        ):
            return None
        domains.append(domain)
    return domains


def tla_record_update_scope(expression: str) -> tuple[str, list[str], list[str]] | None:
    """Return base, dynamic selector expressions, and values for record updates."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_square_brackets_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return None

    except_index = tla_top_level_keyword_index(inner, "EXCEPT")
    if except_index is None:
        return None
    base = inner[:except_index].strip()
    updates = inner[except_index + len("EXCEPT") :].strip()
    if not base or not updates:
        return None

    selectors: list[str] = []
    replacements: list[str] = []
    for update in tla_top_level_argument_parts(updates):
        relation = tla_top_level_equality_relation_parts(update)
        if relation is None or relation[1] != "=":
            return None
        path, _, replacement = relation
        path_selectors = tla_record_update_path_selector_expressions(path)
        if path_selectors is None:
            return None
        selectors.extend(path_selectors)
        replacements.append(replacement)
    return base, selectors, replacements


def tla_tuple_literal_values(expression: str) -> list[str] | None:
    """Return tuple elements as recursively scanned expression values."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_tuple_brackets_enclose_expression(text):
        return None
    inner = text[2:-2].strip()
    if not inner:
        return []
    values = tla_top_level_argument_parts(inner)
    if not values or any(not value for value in values):
        return None
    return values


def tla_record_update_path_selector_expressions(path: str) -> list[str] | None:
    """Return dynamic expressions from an EXCEPT selector path."""

    text = " ".join(path.split())
    if not text.startswith("!"):
        return None

    selectors: list[str] = []
    index = 1
    while index < len(text):
        char = text[index]
        if char.isspace():
            index += 1
            continue
        if char == ".":
            index += 1
            while index < len(text) and text[index].isspace():
                index += 1
            match = TLA_IDENTIFIER_SCAN_RE.match(text, index)
            if match is None:
                return None
            field = match.group(0)
            if not is_tla_user_identifier(field):
                return None
            index = match.end()
            continue
        if char == "[":
            end = tla_square_bracket_expression_end(text, index)
            if end is None:
                return None
            selector = text[index + 1 : end].strip()
            if not selector:
                return None
            selectors.append(selector)
            index = end + 1
            continue
        return None
    return selectors


def tla_selector_scope(expression: str) -> tuple[str, list[str]] | None:
    """Return base and dynamic selector expressions for a selector chain."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    selector_start = tla_top_level_selector_start(text)
    if selector_start is None:
        return None
    base = text[:selector_start].strip()
    if not base:
        return None
    selectors = tla_selector_chain_dynamic_expressions(text[selector_start:])
    if selectors is None:
        return None
    return base, selectors


def tla_top_level_selector_start(text: str) -> int | None:
    """Return the start of a top-level field/index selector chain."""

    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if depth == 0 and char == "." and text[:index].strip():
            next_char = text[index + 1] if index + 1 < len(text) else ""
            previous_char = text[index - 1] if index > 0 else ""
            if not (previous_char.isdigit() and next_char.isdigit()):
                return index
        if depth == 0 and char == "[" and text[:index].strip():
            return index
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        index += 1
    return None


def tla_selector_chain_dynamic_expressions(chain: str) -> list[str] | None:
    """Return dynamic index expressions from a field/index selector chain."""

    selectors: list[str] = []
    index = 0
    while index < len(chain):
        char = chain[index]
        if char.isspace():
            index += 1
            continue
        if char == ".":
            index += 1
            while index < len(chain) and chain[index].isspace():
                index += 1
            match = TLA_IDENTIFIER_SCAN_RE.match(chain, index)
            if match is None:
                return None
            field = match.group(0)
            if not is_tla_user_identifier(field):
                return None
            index = match.end()
            continue
        if char == "[":
            end = tla_square_bracket_expression_end(chain, index)
            if end is None:
                return None
            selector = chain[index + 1 : end].strip()
            if not selector:
                return None
            selectors.append(selector)
            index = end + 1
            continue
        return None
    return selectors


def undefined_static_helper_identifiers(
    expression: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    *,
    current: str | None = None,
    exactness_operator: str | None = None,
    local_bound: frozenset[str] = frozenset(),
) -> list[str]:
    """Return undefined helper-like identifiers from a static TLA expression."""

    declared_names = {
        *tla_constant_declarations(module_path),
        *(variable for _, variable in tla_variable_declaration_entries(module_path)),
    }
    ignored_identifiers = {
        identifier
        for identifier in (current, exactness_operator)
        if identifier is not None
    }
    ignored_identifiers.update(TLA_QUANTIFIER_IDENTIFIER_TOKENS)
    ignored_identifiers.update(TLA_STANDARD_OPERATOR_IDENTIFIERS)

    return [
        identifier
        for identifier in sorted(
            tla_free_static_identifiers(expression, frozenset(local_bound))
        )
        if identifier not in ignored_identifiers
        and identifier not in declared_names
        and identifier not in definitions
        and is_tla_helper_identifier(identifier)
    ]


def tla_top_level_conjuncts(expression: str) -> list[str]:
    """Return conservative top-level conjunction parts from a static body."""

    text = strip_static_outer_parentheses(expression).strip()
    conjuncts: list[str] = []
    current: list[str] = []
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            current.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            current.append(char)
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            current.append("<<")
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            current.append(">>")
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        if depth == 0 and text.startswith("/\\", index):
            part = "".join(current).strip()
            if part:
                conjuncts.append(part)
            current = []
            index += 2
            continue
        current.append(char)
        index += 1

    part = "".join(current).strip()
    if part:
        conjuncts.append(part)
    return conjuncts


def tla_top_level_disjuncts(expression: str) -> list[str]:
    """Return conservative top-level disjunction parts from a static body."""

    text = strip_static_outer_parentheses(expression).strip()
    disjuncts: list[str] = []
    current: list[str] = []
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            current.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            current.append(char)
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            current.append("<<")
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            current.append(">>")
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        if depth == 0 and text.startswith("\\/", index):
            part = "".join(current).strip()
            if part:
                disjuncts.append(part)
            current = []
            index += 2
            continue
        current.append(char)
        index += 1

    part = "".join(current).strip()
    if part:
        disjuncts.append(part)
    return disjuncts


def tla_top_level_implication_operands(expression: str) -> list[str]:
    """Return conservative top-level implication operands from a static body."""

    text = strip_static_outer_parentheses(expression).strip()
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        if (
            depth == 0
            and text.startswith("=>", index)
            and (index == 0 or text[index - 1] != "<")
        ):
            operands = [text[:index].strip(), text[index + 2 :].strip()]
            return [operand for operand in operands if operand]
        index += 1
    return [text] if text else []


def tla_top_level_equivalence_operands(expression: str) -> list[str]:
    """Return conservative top-level equivalence operands from a static body."""

    text = strip_static_outer_parentheses(expression).strip()
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        if depth == 0 and text.startswith("<=>", index):
            operands = [text[:index].strip(), text[index + 3 :].strip()]
            return [operand for operand in operands if operand]
        index += 1
    return [text] if text else []


def tla_top_level_boolean_parts(expression: str) -> list[str]:
    """Return direct boolean operands split by one top-level connective."""

    text = strip_static_outer_parentheses(expression).strip()
    if not text:
        return []

    for parts in (
        tla_top_level_conjuncts(text),
        tla_top_level_disjuncts(text),
        tla_top_level_implication_operands(text),
        tla_top_level_equivalence_operands(text),
    ):
        if len(parts) > 1:
            return parts
    return [text]


def tla_top_level_operator_chain_operands(expression: str, operator: str) -> list[str]:
    """Return top-level operands split across a repeated binary operator."""

    text = strip_static_outer_parentheses(expression).strip()
    if not text:
        return []

    operands: list[str] = []
    current: list[str] = []
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            current.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            current.append(char)
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            current.append("<<")
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            current.append(">>")
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        is_operator = depth == 0 and text.startswith(operator, index)
        if is_operator and operator == "=>" and index > 0 and text[index - 1] == "<":
            is_operator = False
        if is_operator:
            operand = "".join(current).strip()
            if operand:
                operands.append(operand)
            current = []
            index += len(operator)
            continue
        current.append(char)
        index += 1

    operand = "".join(current).strip()
    if operand:
        operands.append(operand)
    return operands if len(operands) > 1 else ([text] if text else [])


def tla_top_level_implication_chain_operands(expression: str) -> list[str]:
    """Return operands across a top-level implication chain."""

    return tla_top_level_operator_chain_operands(expression, "=>")


def tla_top_level_equivalence_chain_operands(expression: str) -> list[str]:
    """Return operands across a top-level equivalence chain."""

    return tla_top_level_operator_chain_operands(expression, "<=>")


def tla_identity_literal_gated_operand(
    expression: str,
    operand_matches: Callable[[str], bool],
) -> str | None:
    """Return an expression gated by boolean identity literals."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not text:
        return None

    def literal_gated_operand(parts: list[str], neutral: str) -> str | None:
        matched = False
        for part in parts:
            literal = tla_static_temporal_boolean_literal(part)
            if literal is not None:
                if literal != neutral:
                    return None
                continue
            if not operand_matches(part):
                return None
            matched = True
        return text if matched else None

    conjunct_parts = tla_top_level_conjuncts(text)
    if len(conjunct_parts) > 1:
        gated = literal_gated_operand(conjunct_parts, "TRUE")
        if gated is not None:
            return gated

    disjunct_parts = tla_top_level_disjuncts(text)
    if len(disjunct_parts) > 1:
        gated = literal_gated_operand(disjunct_parts, "FALSE")
        if gated is not None:
            return gated

    implication_parts = tla_top_level_implication_operands(text)
    if len(implication_parts) > 1:
        antecedent, consequent = implication_parts
        if (
            tla_static_temporal_boolean_literal(antecedent) == "TRUE"
            and operand_matches(consequent)
        ):
            return text

    equivalence_parts = tla_top_level_equivalence_operands(text)
    if len(equivalence_parts) > 1:
        gated = literal_gated_operand(equivalence_parts, "TRUE")
        if gated is not None:
            return gated

    return None


@cache
def tla_zero_arity_conjunct_references(expression: str) -> list[str]:
    """Return zero-arity operator references used as direct conjunction parts."""

    references: list[str] = []
    for conjunct in tla_top_level_conjuncts(expression):
        normalized = strip_static_outer_parentheses(" ".join(conjunct.split()))
        if TLA_IDENTIFIER_RE.fullmatch(normalized) and is_tla_user_identifier(
            normalized
        ):
            references.append(normalized)
    return references


@cache
def tla_zero_arity_boolean_references(expression: str) -> list[str]:
    """Return zero-arity references used as direct boolean operands."""

    references: set[str] = set()
    seen: set[str] = set()

    def collect(body: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if not normalized or normalized in seen:
            return
        seen.add(normalized)
        if TLA_IDENTIFIER_RE.fullmatch(normalized) and is_tla_user_identifier(
            normalized
        ):
            references.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand)

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(expression)
    return sorted(references)


@cache
def exactness_helper_references(expression: str) -> list[str]:
    """Return helper references reachable from exactness predicate bodies."""

    references: list[str] = []
    seen: set[str] = set()
    for reference in (
        tla_zero_arity_conjunct_references(expression)
        + tla_zero_arity_boolean_references(expression)
    ):
        if reference in seen:
            continue
        seen.add(reference)
        references.append(reference)
    return references


@cache
def hidden_static_structured_helper_references(expression: str) -> list[str]:
    """Return helper references hidden below static wrappers or data operands."""

    references: list[str] = []
    seen_refs: set[str] = set()
    seen_bodies: set[tuple[str, bool, frozenset[str]]] = set()

    def record(reference: str, hidden: bool, bound: frozenset[str]) -> None:
        if (
            hidden
            and reference not in bound
            and reference not in seen_refs
            and is_tla_user_identifier(reference)
        ):
            seen_refs.add(reference)
            references.append(reference)

    def collect(
        current: str,
        hidden: bool = False,
        bound: frozenset[str] = frozenset(),
    ) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, hidden, bound)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)

        if TLA_IDENTIFIER_RE.fullmatch(normalized):
            record(normalized, hidden, bound)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, hidden, bound)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, hidden, bound)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, hidden, bound)
            return

        choose_split = tla_choose_prefix_and_body(normalized)
        if choose_split is not None:
            prefix, choose_body = choose_split
            local_bound = tla_choose_binding_identifiers(prefix)
            if local_bound is not None:
                for domain in tla_choose_bound_domains(prefix):
                    collect(domain, True, bound)
                collect(choose_body, True, bound | frozenset(local_bound))
                return
            prefix_expression = re.sub(
                r"^CHOOSE\s+", "", prefix.strip(), count=1
            ).strip()
            for binding in tla_top_level_argument_parts(prefix_expression):
                collect(binding, True, bound)
            collect(choose_body, True, bound)
            return

        lambda_scope = tla_lambda_scope(normalized)
        if lambda_scope is not None:
            domains, lambda_body, local_bound = lambda_scope
            for domain in domains:
                collect(domain, True, bound)
            collect(lambda_body, True, bound | frozenset(local_bound))
            return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, hidden, bound)
            return

        for marker in ("/\\", "\\/"):
            if normalized.startswith(marker):
                operand = normalized[len(marker) :].strip()
                if operand:
                    collect(operand, hidden, bound)
                return

        action_operand = tla_unary_action_operand(normalized)
        if action_operand is not None:
            collect(action_operand, True, bound)
            return

        unary_set_operand = tla_unary_set_operator_operand(normalized)
        if unary_set_operand is not None:
            collect(unary_set_operand, True, bound)
            return

        tuple_values = tla_tuple_literal_values(normalized)
        if tuple_values is not None:
            for value in tuple_values:
                collect(value, True, bound)
            return

        set_scope = tla_set_comprehension_scope(normalized)
        if set_scope is not None:
            domains, set_body, local_bound = set_scope
            for domain in domains:
                collect(domain, True, bound)
            collect(set_body, True, bound | frozenset(local_bound))
            return

        set_elements = tla_explicit_set_elements(normalized)
        if set_elements is not None:
            for element in set_elements:
                collect(element, True, bound)
            return

        function_scope = tla_function_constructor_scope(normalized)
        if function_scope is not None:
            domains, function_body, local_bound = function_scope
            for domain in domains:
                collect(domain, True, bound)
            collect(function_body, True, bound | frozenset(local_bound))
            return

        function_set_scope = tla_function_set_scope(normalized)
        if function_set_scope is not None:
            domain, range_expression = function_set_scope
            collect(domain, True, bound)
            collect(range_expression, True, bound)
            return

        record_values = tla_record_literal_values(normalized)
        if record_values is not None:
            for value in record_values:
                collect(value, True, bound)
            return

        record_domains = tla_record_set_field_domains(normalized)
        if record_domains is not None:
            for domain in record_domains:
                collect(domain, True, bound)
            return

        record_update = tla_record_update_scope(normalized)
        if record_update is not None:
            base, selectors, replacements = record_update
            collect(base, True, bound)
            for selector in selectors:
                collect(selector, True, bound)
            for replacement in replacements:
                collect(replacement, True, bound)
            return

        relation_parts = tla_top_level_relation_parts(normalized)
        if relation_parts is not None:
            if hidden:
                left, _, right = relation_parts
                collect(left, True, bound)
                collect(right, True, bound)
            return

        infix_operands = tla_top_level_static_infix_operands(normalized)
        if infix_operands is not None:
            if hidden:
                for operand in infix_operands:
                    collect(operand, True, bound)
            return

        call_arguments = tla_direct_operator_call_arguments(normalized)
        if call_arguments is not None:
            for argument in tla_top_level_argument_parts(call_arguments):
                if argument:
                    collect(argument, True, bound)
            return

        selector_scope = tla_selector_scope(normalized)
        if selector_scope is not None:
            base, selectors = selector_scope
            collect(base, True, bound)
            for selector in selectors:
                collect(selector, True, bound)
            return

    collect(expression)
    return references


def is_tla_helper_identifier(identifier: str) -> bool:
    """Return whether an identifier looks like a named helper predicate."""

    return bool(identifier) and identifier[0].isupper()


def duplicate_zero_arity_conjunct_references(expression: str) -> list[str]:
    """Return repeated zero-arity conjunct references in stable order."""

    seen: set[str] = set()
    duplicates: list[str] = []
    for reference in tla_zero_arity_conjunct_references(expression):
        if reference in seen and reference not in duplicates:
            duplicates.append(reference)
        seen.add(reference)
    return duplicates


def duplicate_zero_arity_wrapped_conjunct_references(expression: str) -> list[str]:
    """Return repeated named conjuncts hidden below helper wrappers."""

    duplicates: list[str] = []
    seen_bodies: set[str] = set()

    def record(parts: list[str]) -> None:
        seen_references: set[tuple[str, bool]] = set()
        for part in parts:
            operand = zero_arity_operand_polarity(part)
            if operand is None:
                continue
            name, polarity = operand
            key = (name, polarity)
            if key in seen_references and name not in duplicates:
                duplicates.append(name)
            seen_references.add(key)

    def collect(body: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        conjuncts = tla_top_level_conjuncts(normalized)
        if len(conjuncts) > 1:
            for conjunct in conjuncts:
                collect(conjunct)
            record(conjuncts)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(expression)
    return duplicates


def duplicate_zero_arity_boolean_operand_references(expression: str) -> list[str]:
    """Return repeated named operands in non-conjunctive boolean helpers."""

    duplicates: list[str] = []
    seen_bodies: set[str] = set()

    def record(parts: list[str]) -> None:
        seen_references: set[tuple[str, bool]] = set()
        for part in parts:
            operand = zero_arity_operand_polarity(part)
            if operand is None:
                continue
            name, polarity = operand
            key = (name, polarity)
            if key in seen_references and name not in duplicates:
                duplicates.append(name)
            seen_references.add(key)

    def collect(body: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        for parts in (
            tla_top_level_disjuncts(normalized),
            tla_top_level_implication_chain_operands(normalized),
            tla_top_level_equivalence_chain_operands(normalized),
        ):
            if len(parts) > 1:
                record(parts)
                for part in parts:
                    collect(part)
                return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand)
            return

        for conjunct in tla_top_level_conjuncts(normalized):
            compact_conjunct = strip_static_outer_parentheses(
                " ".join(conjunct.split())
            )
            if compact_conjunct == normalized:
                continue
            collect(conjunct)

    collect(expression)
    return duplicates


def zero_arity_operand_polarity(part: str) -> tuple[str, bool] | None:
    """Return a named boolean operand and whether it is positive."""

    normalized = strip_static_outer_parentheses(" ".join(part.split()))
    polarity = True
    while True:
        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            normalized = strip_static_outer_parentheses(
                " ".join(negated_operand.split())
            )
            polarity = not polarity
            continue
        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            normalized = strip_static_outer_parentheses(" ".join(let_operand.split()))
            continue
        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            normalized = strip_static_outer_parentheses(
                " ".join(temporal_operand.split())
            )
            continue
        break
    if TLA_IDENTIFIER_RE.fullmatch(normalized) and is_tla_user_identifier(normalized):
        return normalized, polarity
    return None


def zero_arity_polarity_conflicts(parts: list[str]) -> list[str]:
    """Return named operands that appear positively and negatively."""

    positive: set[str] = set()
    negative: set[str] = set()
    for part in parts:
        operand = zero_arity_operand_polarity(part)
        if operand is None:
            continue
        name, polarity = operand
        if polarity:
            positive.add(name)
        else:
            negative.add(name)
    return sorted(positive & negative)


def contradictory_zero_arity_conjunct_references(expression: str) -> list[str]:
    """Return named operands paired with their negation in conjunctions."""

    contradictory: list[str] = []
    seen_bodies: set[str] = set()

    def record(parts: list[str]) -> None:
        for name in zero_arity_polarity_conflicts(parts):
            if name not in contradictory:
                contradictory.append(name)

    def collect(body: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)
            return

        conjuncts = tla_top_level_conjuncts(normalized)
        if len(conjuncts) > 1:
            record(conjuncts)
            for conjunct in conjuncts:
                collect(conjunct)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(expression)
    return contradictory


def excluded_middle_zero_arity_disjunct_references(expression: str) -> list[str]:
    """Return named operands paired with their negation in disjunctions."""

    excluded: list[str] = []
    seen_bodies: set[str] = set()

    def record(parts: list[str]) -> None:
        for name in zero_arity_polarity_conflicts(parts):
            if name not in excluded:
                excluded.append(name)

    def collect(body: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)
            return

        disjuncts = tla_top_level_disjuncts(normalized)
        if len(disjuncts) > 1:
            record(disjuncts)
            for disjunct in disjuncts:
                collect(disjunct)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(expression)
    return excluded


def complementary_equivalence_zero_arity_references(expression: str) -> list[str]:
    """Return named operands paired with their negation in equivalences."""

    complementary: list[str] = []
    seen_bodies: set[str] = set()

    def record(parts: list[str]) -> None:
        for name in zero_arity_polarity_conflicts(parts):
            if name not in complementary:
                complementary.append(name)

    def collect(body: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)
            return

        equivalence_parts = tla_top_level_equivalence_chain_operands(normalized)
        if len(equivalence_parts) > 1:
            record(equivalence_parts)
            for part in equivalence_parts:
                collect(part)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(expression)
    return complementary


def single_zero_arity_conjunct_alias(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> str | None:
    """Return a single-helper conjunct alias target, if one is present."""

    seen: set[str] = set()

    def collect(current: str) -> str | None:
        compact_body = " ".join(strip_static_outer_parentheses(current).split())
        if not compact_body or compact_body in seen:
            return None
        seen.add(compact_body)
        if TLA_IDENTIFIER_RE.fullmatch(compact_body):
            return None

        let_operand = tla_static_let_alias_operand(compact_body)
        if let_operand is not None:
            return collect(let_operand)

        negated_operand = tla_static_negation_operand(compact_body)
        if negated_operand is not None:
            return collect(negated_operand)

        temporal_operand = tla_unary_temporal_operand(compact_body)
        if temporal_operand is not None:
            return collect(temporal_operand)

        def literal_gated_alias(parts: list[str], neutral: str) -> str | None:
            aliases: list[str] = []
            for part in parts:
                literal = tla_static_temporal_boolean_literal(part)
                if literal is not None:
                    if literal != neutral:
                        return None
                    continue
                alias = collect(part)
                if alias is None:
                    return None
                aliases.append(alias)
            if len(aliases) != 1:
                return None
            return aliases[0]

        conjunct_parts = tla_top_level_conjuncts(compact_body)
        if len(conjunct_parts) > 1:
            alias = literal_gated_alias(conjunct_parts, "TRUE")
            if alias is not None:
                return alias

        disjunct_parts = tla_top_level_disjuncts(compact_body)
        if len(disjunct_parts) > 1:
            alias = literal_gated_alias(disjunct_parts, "FALSE")
            if alias is not None:
                return alias

        implication_parts = tla_top_level_implication_operands(compact_body)
        if len(implication_parts) > 1:
            antecedent, consequent = implication_parts
            if tla_static_temporal_boolean_literal(antecedent) == "TRUE":
                alias = collect(consequent)
                if alias is not None:
                    return alias

        equivalence_parts = tla_top_level_equivalence_operands(compact_body)
        if len(equivalence_parts) > 1:
            alias = literal_gated_alias(equivalence_parts, "TRUE")
            if alias is not None:
                return alias

        conjuncts = tla_top_level_conjuncts(compact_body)
        references = tla_zero_arity_conjunct_references(compact_body)
        if len(conjuncts) != 1 or len(references) != 1:
            return None
        reference = references[0]
        compact_conjunct = " ".join(
            strip_static_outer_parentheses(conjuncts[0]).split()
        )
        if compact_conjunct != reference or reference not in definitions:
            return None
        return reference

    return collect(expression)


def literal_gated_zero_arity_helper_alias(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> str | None:
    """Return a helper alias hidden behind identity boolean literals."""

    seen: set[str] = set()

    def direct_helper_operand(current: str) -> str | None:
        compact = " ".join(strip_static_outer_parentheses(current).split())
        if not compact:
            return None
        if TLA_IDENTIFIER_RE.fullmatch(compact) and compact in definitions:
            return compact
        let_operand = tla_static_let_alias_operand(compact)
        if let_operand is not None:
            return direct_helper_operand(let_operand)
        return None

    def helper_alias_operand(current: str) -> str | None:
        alias = direct_helper_operand(current)
        if alias is not None:
            return alias
        return collect(current)

    def literal_gated_alias(parts: list[str], neutral: str) -> str | None:
        aliases: list[str] = []
        for part in parts:
            literal = tla_static_temporal_boolean_literal(part)
            if literal is not None:
                if literal != neutral:
                    return None
                continue
            alias = helper_alias_operand(part)
            if alias is None:
                return None
            aliases.append(alias)
        if len(aliases) != 1:
            return None
        return aliases[0]

    def collect(current: str) -> str | None:
        compact_body = " ".join(strip_static_outer_parentheses(current).split())
        if not compact_body or compact_body in seen:
            return None
        seen.add(compact_body)

        let_operand = tla_static_let_alias_operand(compact_body)
        if let_operand is not None:
            return collect(let_operand)

        temporal_operand = tla_unary_temporal_operand(compact_body)
        if temporal_operand is not None:
            return collect(temporal_operand)

        conjunct_parts = tla_top_level_conjuncts(compact_body)
        if len(conjunct_parts) > 1:
            alias = literal_gated_alias(conjunct_parts, "TRUE")
            if alias is not None:
                return alias

        disjunct_parts = tla_top_level_disjuncts(compact_body)
        if len(disjunct_parts) > 1:
            alias = literal_gated_alias(disjunct_parts, "FALSE")
            if alias is not None:
                return alias

        implication_parts = tla_top_level_implication_operands(compact_body)
        if len(implication_parts) > 1:
            antecedent, consequent = implication_parts
            if tla_static_temporal_boolean_literal(antecedent) == "TRUE":
                return helper_alias_operand(consequent)

        equivalence_parts = tla_top_level_equivalence_operands(compact_body)
        if len(equivalence_parts) > 1:
            alias = literal_gated_alias(equivalence_parts, "TRUE")
            if alias is not None:
                return alias

        return None

    return collect(expression)


def literal_gated_negated_zero_arity_helper_operand(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> str | None:
    """Return a negated helper hidden behind identity boolean literals."""

    seen: set[str] = set()

    def negated_helper_operand(current: str) -> str | None:
        compact = " ".join(strip_static_outer_parentheses(current).split())
        if not compact:
            return None
        let_operand = tla_static_let_alias_operand(compact)
        if let_operand is not None:
            return negated_helper_operand(let_operand)
        temporal_operand = tla_unary_temporal_operand(compact)
        if temporal_operand is not None:
            return negated_helper_operand(temporal_operand)
        negated_operand = tla_static_negation_operand(compact)
        if negated_operand is None:
            return None
        operand = exactness_boolean_helper_operand_name(negated_operand)
        if TLA_IDENTIFIER_RE.fullmatch(operand) and operand in definitions:
            return operand
        return None

    def negated_helper_alias_operand(current: str) -> str | None:
        operand = negated_helper_operand(current)
        if operand is not None:
            return operand
        return collect(current)

    def literal_gated_operand(parts: list[str], neutral: str) -> str | None:
        operands: list[str] = []
        for part in parts:
            literal = tla_static_temporal_boolean_literal(part)
            if literal is not None:
                if literal != neutral:
                    return None
                continue
            operand = negated_helper_alias_operand(part)
            if operand is None:
                return None
            operands.append(operand)
        if len(operands) != 1:
            return None
        return operands[0]

    def collect(current: str) -> str | None:
        compact_body = " ".join(strip_static_outer_parentheses(current).split())
        if not compact_body or compact_body in seen:
            return None
        seen.add(compact_body)

        let_operand = tla_static_let_alias_operand(compact_body)
        if let_operand is not None:
            return collect(let_operand)

        temporal_operand = tla_unary_temporal_operand(compact_body)
        if temporal_operand is not None:
            return collect(temporal_operand)

        conjunct_parts = tla_top_level_conjuncts(compact_body)
        if len(conjunct_parts) > 1:
            operand = literal_gated_operand(conjunct_parts, "TRUE")
            if operand is not None:
                return operand

        disjunct_parts = tla_top_level_disjuncts(compact_body)
        if len(disjunct_parts) > 1:
            operand = literal_gated_operand(disjunct_parts, "FALSE")
            if operand is not None:
                return operand

        implication_parts = tla_top_level_implication_operands(compact_body)
        if len(implication_parts) > 1:
            antecedent, consequent = implication_parts
            if tla_static_temporal_boolean_literal(antecedent) == "TRUE":
                return negated_helper_alias_operand(consequent)

        equivalence_parts = tla_top_level_equivalence_operands(compact_body)
        if len(equivalence_parts) > 1:
            operand = literal_gated_operand(equivalence_parts, "TRUE")
            if operand is not None:
                return operand

        return None

    return collect(expression)


def tla_static_self_equality(expression: str) -> str | None:
    """Return a simple identifier self-equality, if present."""

    compact = " ".join(strip_static_outer_parentheses(expression).split())
    match = TLA_IDENTIFIER_SELF_EQUALITY_RE.fullmatch(compact)
    if match is None or not is_tla_user_identifier(match.group(1)):
        return None
    return compact


def tla_static_self_inequality(expression: str) -> str | None:
    """Return a simple identifier self-inequality, if present."""

    compact = " ".join(strip_static_outer_parentheses(expression).split())
    match = TLA_IDENTIFIER_SELF_INEQUALITY_RE.fullmatch(compact)
    if match is None or not is_tla_user_identifier(match.group(1)):
        return None
    return compact


def nonzero_arity_conjunct_references(
    expression: str,
    signatures: dict[str, tuple[int, int]],
) -> list[tuple[str, int, int]]:
    """Return direct named conjunct references whose definitions have parameters."""

    references: list[tuple[str, int, int]] = []
    for reference in tla_zero_arity_conjunct_references(expression):
        signature = signatures.get(reference)
        if signature is None:
            continue
        line, arity = signature
        if arity != 0:
            references.append((reference, line, arity))
    return references


def format_nonzero_arity_references(
    references: list[tuple[str, int, int]],
    module_path: Path,
) -> str:
    """Format non-zero arity references for checker diagnostics."""

    return ", ".join(
        f"{reference} at {display_path(module_path)}:{line} has arity {arity}"
        for reference, line, arity in references
    )


def tla_has_top_level_disjunction(expression: str) -> bool:
    """Return whether an expression contains a top-level disjunction operator."""

    return len(tla_top_level_disjuncts(expression)) > 1


def tla_has_top_level_implication(expression: str) -> bool:
    """Return whether an expression contains a top-level implication operator."""

    return len(tla_top_level_implication_operands(expression)) > 1


def tla_has_top_level_equivalence(expression: str) -> bool:
    """Return whether an expression contains a top-level equivalence operator."""

    return len(tla_top_level_equivalence_operands(expression)) > 1


def tla_has_top_level_equality(expression: str) -> bool:
    """Return whether an expression contains a top-level equality operator."""

    relation = tla_top_level_equality_relation_parts(expression)
    return relation is not None and relation[1] == "="


def tla_relation_scan_starts_with_wrapper(expression: str) -> bool:
    """Return whether relation scanning should defer to a whole-body wrapper."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    return (
        TLA_WHOLE_BODY_CONTROL_RE.match(text) is not None
        or text.startswith(("[]", "<>", "~"))
    )


def tla_top_level_relation_operator(expression: str) -> str | None:
    """Return a top-level scalar relation operator, if present."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if tla_relation_scan_starts_with_wrapper(text):
        return None
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        if text.startswith("<=>", index):
            index += len("<=>")
            continue
        if text.startswith("=>", index):
            index += len("=>")
            continue
        if text.startswith("\\in", index):
            before = text[index - 1] if index > 0 else ""
            after_index = index + len("\\in")
            after = text[after_index] if after_index < len(text) else ""
            if (
                not (before.isalnum() or before == "_")
                and not (after.isalnum() or after == "_")
            ):
                return "\\in"
        if text.startswith("/=", index):
            return "/="
        if text.startswith("<=", index):
            return "<="
        if text.startswith(">=", index):
            return ">="
        if char == "#":
            return "#"
        if char == "=":
            previous_char = text[index - 1] if index > 0 else ""
            next_char = text[index + 1] if index + 1 < len(text) else ""
            if previous_char not in "<>/" and next_char != ">":
                return "="
        if char == "<":
            next_char = text[index + 1] if index + 1 < len(text) else ""
            if next_char not in "<=>":
                return "<"
        if char == ">":
            previous_char = text[index - 1] if index > 0 else ""
            if previous_char not in "<>":
                return ">"
        index += 1
    return None


def tla_static_constant_relation(expression: str) -> str | None:
    """Return a whole-body constant relation with no model identifiers."""

    memo: dict[str, str | None] = {}
    visiting: set[str] = set()

    def collect(body: str) -> str | None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        if normalized in memo:
            return memo[normalized]
        if normalized in visiting:
            return None
        visiting.add(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            result = normalized if collect(let_operand) is not None else None
            visiting.remove(normalized)
            memo[normalized] = result
            return result

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            result = normalized if collect(temporal_operand) is not None else None
            visiting.remove(normalized)
            memo[normalized] = result
            return result

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            result = normalized if collect(negated_operand) is not None else None
            visiting.remove(normalized)
            memo[normalized] = result
            return result

        result = None
        if tla_top_level_relation_operator(normalized) is not None:
            identifier_scan = tla_without_string_literals(normalized).replace("\\in", " ")
            if not tla_static_identifiers(identifier_scan):
                result = normalized

        visiting.remove(normalized)
        memo[normalized] = result
        return result

    return collect(expression)


def tla_direct_operator_call_name(expression: str) -> str | None:
    """Return the callee for a whole-expression operator call."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    match = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*\(", text)
    if match is None:
        return None
    callee = match.group(1)
    if not is_tla_user_identifier(callee):
        return None
    open_index = text.find("(", match.end(1))
    if open_index == -1 or text[match.end(1) : open_index].strip():
        return None

    depth = 0
    in_string = False
    escaped = False
    for index in range(open_index, len(text)):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue
        if char == '"':
            in_string = True
            continue
        if char == "(":
            depth += 1
            continue
        if char != ")":
            continue
        depth -= 1
        if depth < 0:
            return None
        if depth == 0 and index != len(text) - 1:
            return None
    if depth != 0 or in_string:
        return None
    return callee


def tla_direct_operator_call_arguments(expression: str) -> str | None:
    """Return the argument text for a whole-expression operator call."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if tla_direct_operator_call_name(text) is None:
        return None
    open_index = text.find("(")
    if open_index == -1 or not text.endswith(")"):
        return None
    return text[open_index + 1 : -1].strip()


def tla_top_level_argument_parts(arguments: str) -> list[str]:
    """Return top-level comma-separated call arguments."""

    parts: list[str] = []
    start = 0
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(arguments):
        char = arguments[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if arguments.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if arguments.startswith(">>", index):
            depth -= 1
            if depth < 0:
                return [arguments.strip()]
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}":
            depth -= 1
            if depth < 0:
                return [arguments.strip()]
            index += 1
            continue
        if char == "," and depth == 0:
            parts.append(arguments[start:index].strip())
            start = index + 1
        index += 1
    if depth != 0 or in_string:
        return [arguments.strip()]
    parts.append(arguments[start:].strip())
    return parts


def tla_simple_call_argument(argument: str) -> bool:
    """Return whether a call argument is a simple case/literal anchor."""

    compact = " ".join(argument.split())
    if compact in {"TRUE", "FALSE"}:
        return True
    if re.fullmatch(r"-?\d+", compact) is not None:
        return True
    if re.fullmatch(r'"(?:[^"\\]|\\.)*"', compact) is not None:
        return True
    return TLA_IDENTIFIER_RE.fullmatch(compact) is not None and is_tla_user_identifier(
        compact
    )


def tla_direct_operator_call_has_complex_argument(expression: str) -> bool:
    """Return whether a direct call has a non-atomic expression argument."""

    arguments = tla_direct_operator_call_arguments(expression)
    if arguments is None:
        return False
    parts = tla_top_level_argument_parts(arguments)
    if not parts or any(not part for part in parts):
        return False
    return any(not tla_simple_call_argument(part) for part in parts)


def tla_top_level_keyword_index(text: str, keyword: str, start: int = 0) -> int | None:
    """Return the index of a top-level TLA keyword occurrence, if present."""

    depth = 0
    in_string = False
    escaped = False
    index = start
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth == 0 and text.startswith(keyword, index):
            before = text[index - 1] if index > 0 else ""
            after_index = index + len(keyword)
            after = text[after_index] if after_index < len(text) else ""
            if (
                not (before.isalnum() or before == "_")
                and not (after.isalnum() or after == "_")
            ):
                return index
        index += 1
    return None


def tla_top_level_if_parts(expression: str) -> tuple[str, str, str] | None:
    """Return top-level IF condition and THEN/ELSE branches."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not re.match(r"^IF\b", text):
        return None
    then_index = tla_top_level_keyword_index(text, "THEN", start=2)
    if then_index is None:
        return None
    else_index = tla_top_level_keyword_index(
        text,
        "ELSE",
        start=then_index + len("THEN"),
    )
    if else_index is None:
        return None
    condition = text[2:then_index].strip()
    then_branch = text[then_index + len("THEN") : else_index].strip()
    else_branch = text[else_index + len("ELSE") :].strip()
    if not condition or not then_branch or not else_branch:
        return None
    return condition, then_branch, else_branch


def tla_top_level_if_branches(expression: str) -> tuple[str, str] | None:
    """Return top-level THEN/ELSE branches from a static IF expression."""

    parts = tla_top_level_if_parts(expression)
    if parts is None:
        return None
    _, then_branch, else_branch = parts
    return then_branch, else_branch


def tla_static_if_boolean_literal(expression: str) -> str | None:
    """Return the selected literal for an IF with a static boolean condition."""

    parts = tla_top_level_if_parts(expression)
    if parts is None:
        return None
    condition, then_branch, else_branch = parts
    condition_literal = tla_static_temporal_boolean_literal(condition)
    if condition_literal is None:
        return None
    selected_branch = then_branch if condition_literal == "TRUE" else else_branch
    selected_literal = tla_static_temporal_boolean_literal(selected_branch)
    if selected_literal is not None:
        return selected_literal
    return tla_static_if_boolean_literal(selected_branch)


def tla_control_flow_result_is_static_boolean_literal(expression: str) -> bool:
    """Return whether a control-flow expression bottoms out in boolean literals."""

    stripped = strip_static_outer_parentheses(expression)
    if tla_static_temporal_boolean_literal(stripped) is not None:
        return True
    branches = tla_top_level_if_branches(stripped)
    if branches is None:
        return False
    return all(
        tla_control_flow_result_is_static_boolean_literal(branch)
        for branch in branches
    )


def tla_control_flow_helper_selects_predicate(expression: str) -> bool:
    """Return whether whole-body control flow selects non-literal obligations."""

    compact = " ".join(strip_static_outer_parentheses(expression).split())
    control = TLA_WHOLE_BODY_CONTROL_RE.match(compact)
    if control is None:
        return False
    if (
        control.group(1) == "IF"
        and tla_control_flow_result_is_static_boolean_literal(compact)
    ):
        return False
    return True


def exactness_definition_shape_errors(
    mode: str,
    module_path: Path,
    cfg_file: Path,
    cfg_line_number: int,
    runner_name: str,
    exactness_operator: str,
    definitions: dict[str, tuple[int, str]],
    reference_context: str,
) -> list[str]:
    prefix = exactness_definition_shape_prefix(
        mode,
        cfg_file,
        cfg_line_number,
        runner_name,
        reference_context,
        exactness_operator,
    )
    template_prefix = exactness_definition_shape_prefix(
        TLA_MODULE_VALIDATION_MODE_MARKER,
        EXACTNESS_SHAPE_TEMPLATE_CFG,
        EXACTNESS_SHAPE_TEMPLATE_LINE,
        EXACTNESS_SHAPE_TEMPLATE_RUNNER,
        EXACTNESS_SHAPE_TEMPLATE_REFERENCE,
        exactness_operator,
    )
    cache_key = (module_path, exactness_operator, id(definitions))
    templates = _EXACTNESS_DEFINITION_SHAPE_ERROR_TEMPLATES.get(cache_key)
    if templates is None:
        templates = tuple(
            exactness_definition_shape_errors_uncached(
                TLA_MODULE_VALIDATION_MODE_MARKER,
                module_path,
                EXACTNESS_SHAPE_TEMPLATE_CFG,
                EXACTNESS_SHAPE_TEMPLATE_LINE,
                EXACTNESS_SHAPE_TEMPLATE_RUNNER,
                exactness_operator,
                definitions,
                EXACTNESS_SHAPE_TEMPLATE_REFERENCE,
            )
        )
        _EXACTNESS_DEFINITION_SHAPE_ERROR_TEMPLATES[cache_key] = templates

    return [error.replace(template_prefix, prefix, 1) for error in templates]


def exactness_definition_shape_prefix(
    mode: str,
    cfg_file: Path,
    cfg_line_number: int,
    runner_name: str,
    reference_context: str,
    exactness_operator: str,
) -> str:
    """Return the diagnostic prefix for an exactness-shape check."""

    return (
        f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
        f"{cfg_line_number} {reference_context} {exactness_operator}"
    )


def exactness_definition_shape_errors_uncached(
    mode: str,
    module_path: Path,
    cfg_file: Path,
    cfg_line_number: int,
    runner_name: str,
    exactness_operator: str,
    definitions: dict[str, tuple[int, str]],
    reference_context: str,
) -> list[str]:
    exactness_definition = definitions.get(exactness_operator)
    prefix = exactness_definition_shape_prefix(
        mode,
        cfg_file,
        cfg_line_number,
        runner_name,
        reference_context,
        exactness_operator,
    )
    if exactness_definition is None:
        return [
            f"{prefix} has no static "
            f"single-expression definition in {display_path(module_path)}"
        ]

    exactness_line, exactness_body = exactness_definition
    signatures = tla_operator_signatures(module_path)
    exactness_signature = signatures.get(exactness_operator)
    if exactness_signature is not None and exactness_signature[1] != 0:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_signature[0]} defines "
            f"{exactness_operator} with arity {exactness_signature[1]}; "
            "exactness operators must be zero-arity"
        ]
    exactness_body = strip_static_outer_parentheses(exactness_body)
    exactness_literal = tla_static_boolean_literal(exactness_body)
    if exactness_literal is not None:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is literal "
            f"{exactness_literal}"
        ]
    if exactness_body in GENERIC_CORRECTNESS_CHECKS:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} aliases generic "
            f"{exactness_body}; compose concrete model predicates directly"
        ]
    if TLA_IDENTIFIER_EQUALITY_RE.fullmatch(exactness_body):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is raw scalar "
            f"equality {exactness_body}; name the concrete model predicate and "
            "compose it as a direct exactness conjunct"
        ]
    exactness_compact_body = " ".join(exactness_body.split())
    if exactness_compact_body.startswith("~"):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"negation {exactness_compact_body}; name the concrete model "
            "predicate and compose it as a direct exactness conjunct"
        ]
    whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(exactness_compact_body)
    if whole_body_control is not None:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"{whole_body_control.group(1)} expression {exactness_compact_body}; "
            "name the concrete model predicate and compose it as a direct "
            "exactness conjunct"
        ]
    if (
        not exactness_compact_body.startswith(("/\\", "\\A ", "\\E "))
        and tla_has_top_level_disjunction(exactness_body)
    ):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"disjunction {exactness_compact_body}; name the concrete model "
            "predicate and compose it as a direct exactness conjunct"
        ]
    if (
        not exactness_compact_body.startswith("/\\")
        and tla_has_top_level_implication(exactness_body)
    ):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"implication {exactness_compact_body}; name the concrete model "
            "predicate and compose it as a direct exactness conjunct"
        ]
    if (
        not exactness_compact_body.startswith("/\\")
        and tla_has_top_level_equivalence(exactness_body)
    ):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"equivalence {exactness_compact_body}; name the concrete model "
            "predicate and compose it as a direct exactness conjunct"
        ]
    if TLA_ACTIONS_MATCH_QUANTIFIER_RE.fullmatch(exactness_compact_body):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"implementation/spec action quantifier {exactness_compact_body}; "
            "name the concrete model predicate and compose it as a direct "
            "exactness conjunct"
        ]
    if TLA_MATCHES_QUANTIFIER_RE.fullmatch(exactness_compact_body):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"Matches quantifier {exactness_compact_body}; name the concrete "
            "model predicate and compose it as a direct exactness conjunct"
        ]
    if TLA_WHOLE_BODY_QUANTIFIER_RE.match(exactness_compact_body):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} is whole-body "
            f"quantifier {exactness_compact_body}; name the concrete model "
            "predicate and compose it as a direct exactness conjunct"
        ]
    for conjunct in tla_top_level_conjuncts(exactness_body):
        compact_conjunct = " ".join(
            strip_static_outer_parentheses(conjunct).split()
        )
        if TLA_IDENTIFIER_EQUALITY_RE.fullmatch(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"raw scalar equality conjunct {compact_conjunct}; name the "
                "concrete model predicate and compose it as a direct exactness "
                "conjunct"
            ]
        if TLA_ACTIONS_MATCH_QUANTIFIER_RE.fullmatch(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                "implementation/spec action quantifier conjunct "
                f"{compact_conjunct}; name the concrete model predicate and "
                "compose it as a direct exactness conjunct"
            ]
        if TLA_DIRECT_ACTIONS_MATCH_RE.fullmatch(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains "
                "direct implementation/spec action conjunct "
                f"{compact_conjunct}; name the concrete model predicate and "
                "compose it as a direct exactness conjunct"
            ]
        if TLA_MATCHES_QUANTIFIER_RE.fullmatch(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"Matches quantifier conjunct {compact_conjunct}; name the "
                "concrete matches predicate and compose it as a direct "
                "exactness conjunct"
            ]
        if TLA_WHOLE_BODY_QUANTIFIER_RE.match(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"quantifier conjunct {compact_conjunct}; name the concrete "
                "model predicate and compose it as a direct exactness conjunct"
            ]
        if TLA_DIRECT_MATCHES_CALL_RE.fullmatch(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains "
                f"direct Matches conjunct {compact_conjunct}; name the "
                "concrete model predicate and compose it as a direct "
                "exactness conjunct"
            ]
        if tla_has_top_level_equivalence(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"formula equivalence conjunct {compact_conjunct}; name the "
                "concrete model predicate and compose it as a direct exactness "
                "conjunct"
            ]
        if tla_has_top_level_equality(compact_conjunct):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"formula equality conjunct {compact_conjunct}; name the "
                "concrete model predicate and compose it as a direct exactness "
                "conjunct"
            ]
        if tla_direct_operator_call_name(compact_conjunct) is not None:
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"parameterized exactness conjunct {compact_conjunct}; lift "
                "the predicate behind a zero-arity model predicate before "
                "exactness composition"
            ]
    if (
        exactness_compact_body.startswith("/\\")
        and not tla_zero_arity_conjunct_references(exactness_body)
    ):
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains no direct "
            "named exactness conjuncts; name the concrete model predicate and "
            "compose it as a direct exactness conjunct"
        ]
    for conjunct in tla_top_level_conjuncts(exactness_body):
        compact_conjunct = " ".join(
            strip_static_outer_parentheses(conjunct).split()
        )
        if not (
            TLA_IDENTIFIER_RE.fullmatch(compact_conjunct)
            and is_tla_user_identifier(compact_conjunct)
        ):
            return [
                f"{prefix} at "
                f"{display_path(module_path)}:{exactness_line} contains direct "
                f"non-named exactness conjunct {compact_conjunct}; compose "
                "named zero-arity model predicates directly"
            ]

    exactness_identifiers = tla_static_non_string_identifiers(exactness_body)
    if len(exactness_identifiers) == 1 and exactness_body in exactness_identifiers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} aliases "
            f"{exactness_body}; inline concrete model predicates directly"
        ]

    generic_identifiers = sorted(
        identifier
        for identifier in exactness_identifiers
        if identifier in GENERIC_CORRECTNESS_CHECKS
    )
    if "TypeInvariant" in exactness_identifiers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} mentions "
            "TypeInvariant; keep type invariants in *CorrectnessEnvelope "
            "operators"
        ]
    if generic_identifiers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} mentions generic "
            f"{', '.join(generic_identifiers)}; compose concrete model "
            "predicates directly"
        ]
    nested_exactness_identifiers = sorted(
        identifier
        for identifier in exactness_identifiers
        if identifier.endswith("Exactness") and identifier != exactness_operator
    )
    if nested_exactness_identifiers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} composes nested "
            f"exactness {', '.join(nested_exactness_identifiers)}; inline "
            "concrete model predicates directly"
        ]

    duplicate_conjuncts = duplicate_zero_arity_conjunct_references(exactness_body)
    if duplicate_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} repeats exactness "
            f"conjunct {', '.join(duplicate_conjuncts)}; remove duplicate "
            "conjuncts so every obligation is counted once"
        ]
    nonzero_arity_conjuncts = nonzero_arity_conjunct_references(
        exactness_body,
        signatures,
    )
    if nonzero_arity_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains "
            "non-zero-arity exactness conjunct "
            f"{format_nonzero_arity_references(nonzero_arity_conjuncts, module_path)}; "
            "exactness conjuncts must compose zero-arity model predicates"
        ]
    literal_conjuncts: list[str] = []
    self_equality_conjuncts: list[str] = []
    self_inequality_conjuncts: list[str] = []
    constant_relation_conjuncts: list[str] = []
    aliased_conjuncts: list[str] = []
    undefined_conjuncts: list[str] = []
    hidden_coverage_conjuncts: list[str] = []
    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        conjunct_definition = definitions.get(conjunct_operator)
        if conjunct_definition is None:
            undefined_conjuncts.append(conjunct_operator)
            continue
        conjunct_line, conjunct_body = conjunct_definition
        conjunct_body = strip_static_outer_parentheses(conjunct_body)
        conjunct_literal = tla_static_temporal_boolean_literal(conjunct_body)
        if conjunct_literal is not None:
            literal_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} is literal {conjunct_literal}"
            )
            continue
        conjunct_static_if_literal = tla_static_if_boolean_literal(conjunct_body)
        if conjunct_static_if_literal is not None:
            literal_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} is static IF literal "
                f"{conjunct_static_if_literal}"
            )
            continue
        conjunct_constant_relation = tla_static_constant_relation(conjunct_body)
        if conjunct_constant_relation is not None:
            constant_relation_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} is constant relation "
                f"{conjunct_constant_relation}"
            )
            continue
        conjunct_self_equality = tla_static_self_equality(conjunct_body)
        if conjunct_self_equality is not None:
            self_equality_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} is self-equality {conjunct_self_equality}"
            )
            continue
        conjunct_self_equality_parts = temporal_self_equality_parts(conjunct_body)
        if conjunct_self_equality_parts:
            self_equality_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} contains self-equality "
                f"{', '.join(conjunct_self_equality_parts)}"
            )
            continue
        conjunct_self_inequality = tla_static_self_inequality(conjunct_body)
        if conjunct_self_inequality is not None:
            self_inequality_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} is self-inequality {conjunct_self_inequality}"
            )
            continue
        conjunct_self_inequality_parts = temporal_self_inequality_parts(conjunct_body)
        if conjunct_self_inequality_parts:
            self_inequality_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} contains self-inequality "
                f"{', '.join(conjunct_self_inequality_parts)}"
            )
            continue
        conjunct_identifiers = tla_static_non_string_identifiers(conjunct_body)
        if len(conjunct_identifiers) == 1 and conjunct_body in conjunct_identifiers:
            aliased_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} aliases {conjunct_body}"
            )
            continue
        hidden_coverage_identifiers = sorted(
            identifier
            for identifier in conjunct_identifiers
            if identifier == "TypeInvariant"
            or identifier in GENERIC_CORRECTNESS_CHECKS
            or (
                identifier.endswith("Exactness")
                and identifier != exactness_operator
            )
        )
        if hidden_coverage_identifiers:
            hidden_coverage_conjuncts.append(
                f"{conjunct_operator} at {display_path(module_path)}:"
                f"{conjunct_line} mentions "
                f"{', '.join(hidden_coverage_identifiers)}"
            )
    if undefined_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains undefined "
            f"exactness conjunct {', '.join(undefined_conjuncts)}; define "
            "named concrete model predicates before composing them"
        ]
    if literal_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains literal "
            f"exactness conjunct {', '.join(literal_conjuncts)}; compose "
            "concrete model predicates directly"
        ]
    if self_equality_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains "
            "self-equality exactness conjunct "
            f"{', '.join(self_equality_conjuncts)}; compose concrete model "
            "predicates directly"
        ]
    if self_inequality_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains "
            "self-inequality exactness conjunct "
            f"{', '.join(self_inequality_conjuncts)}; compose satisfiable "
            "concrete model predicates directly"
        ]
    if constant_relation_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains "
            "constant-relation exactness conjunct "
            f"{', '.join(constant_relation_conjuncts)}; compose concrete "
            "model predicates directly"
        ]
    if aliased_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains aliased "
            f"exactness conjunct {', '.join(aliased_conjuncts)}; inline "
            "concrete model predicates directly"
        ]
    if hidden_coverage_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains exactness "
            "conjunct with hidden coverage identifiers "
            f"{', '.join(hidden_coverage_conjuncts)}; keep TypeInvariant, "
            "generic correctness, and nested *Exactness identifiers out of "
            "named exactness predicates"
        ]
    transitive_hidden_conjuncts = transitive_hidden_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if transitive_hidden_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with hidden coverage identifiers "
            f"{', '.join(transitive_hidden_conjuncts)}; keep TypeInvariant, "
            "generic correctness, and nested *Exactness identifiers out of "
            "named exactness predicate chains"
        ]
    transitive_duplicate_conjuncts = transitive_duplicate_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if transitive_duplicate_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with repeated helper conjunct "
            f"{', '.join(transitive_duplicate_conjuncts)}; remove duplicate "
            "helper conjuncts so every obligation is counted once"
        ]
    transitive_contradictory_operands = (
        transitive_contradictory_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if transitive_contradictory_operands:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with contradictory helper operand "
            f"{', '.join(transitive_contradictory_operands)}; name concrete "
            "non-contradictory model predicates before composing exactness "
            "predicate chains"
        ]
    transitive_excluded_middle_operands = (
        transitive_excluded_middle_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if transitive_excluded_middle_operands:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with excluded-middle helper operand "
            f"{', '.join(transitive_excluded_middle_operands)}; name concrete "
            "non-tautological model predicates before composing exactness "
            "predicate chains"
        ]
    transitive_complementary_equivalence_operands = (
        transitive_complementary_equivalence_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if transitive_complementary_equivalence_operands:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with complementary-equivalence helper "
            f"operand {', '.join(transitive_complementary_equivalence_operands)}; "
            "name concrete non-vacuous model predicates before composing "
            "exactness predicate chains"
        ]
    transitive_duplicate_operands = (
        transitive_duplicate_boolean_operand_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if transitive_duplicate_operands:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with repeated helper operand "
            f"{', '.join(transitive_duplicate_operands)}; remove duplicate "
            "helper operands so every obligation is counted once"
        ]
    transitive_control_flow_conjuncts = transitive_control_flow_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if transitive_control_flow_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with whole-body control-flow "
            "predicate-selection helper "
            f"{', '.join(transitive_control_flow_conjuncts)}; name concrete "
            "model predicates before composing exactness predicate chains"
        ]
    nested_control_flow_conjuncts = (
        transitive_nested_control_flow_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if nested_control_flow_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with nested control-flow "
            "predicate-selection helper "
            f"{', '.join(nested_control_flow_conjuncts)}; name concrete "
            "model predicates before composing exactness predicate chains"
        ]
    temporal_control_flow_conjuncts = unary_temporal_control_flow_exactness_helpers(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if temporal_control_flow_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with unary-temporal control-flow "
            "predicate-selection helper "
            f"{', '.join(temporal_control_flow_conjuncts)}; name concrete "
            "model predicates before composing exactness predicate chains"
        ]
    structured_control_flow_conjuncts = (
        structured_operand_control_flow_exactness_helpers(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if structured_control_flow_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with structured control-flow "
            "predicate-selection helper "
            f"{', '.join(structured_control_flow_conjuncts)}; name concrete "
            "model predicates before placing them in structured helper operands"
        ]
    transitive_boolean_composition_conjuncts = (
        transitive_boolean_composition_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if transitive_boolean_composition_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with whole-body raw-predicate "
            "boolean-composition helper "
            f"{', '.join(transitive_boolean_composition_conjuncts)}; name "
            "concrete model predicates before composing exactness predicate "
            "chains"
        ]
    transitive_call_boolean_composition_conjuncts = (
        transitive_parameterized_call_boolean_composition_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            signatures,
            module_path,
        )
    )
    if transitive_call_boolean_composition_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with whole-body parameterized-call "
            "boolean-composition helper "
            f"{', '.join(transitive_call_boolean_composition_conjuncts)}; "
            "name concrete model predicates before composing exactness "
            "predicate chains"
        ]
    transitive_quantified_boolean_composition_conjuncts = (
        transitive_quantified_boolean_composition_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            signatures,
            module_path,
        )
    )
    if transitive_quantified_boolean_composition_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with whole-body quantified-predicate "
            "boolean-composition helper "
            f"{', '.join(transitive_quantified_boolean_composition_conjuncts)}; "
            "name concrete model predicates before composing exactness "
            "predicate chains"
        ]
    quantified_wrappers = unary_temporal_quantified_exactness_helpers(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if quantified_wrappers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with unary-temporal quantified formula "
            f"{', '.join(quantified_wrappers)}; name quantified model "
            "predicates before composing exactness predicate chains"
        ]
    static_wrapped_quantified = static_wrapped_quantified_exactness_helpers(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if static_wrapped_quantified:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with static-wrapper quantified formula "
            f"{', '.join(static_wrapped_quantified)}; name quantified model "
            "predicates before composing exactness predicate chains"
        ]
    structured_quantified = structured_operand_quantified_exactness_helpers(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if structured_quantified:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with structured quantified formula "
            f"{', '.join(structured_quantified)}; name quantified model "
            "predicates before placing them in structured helper operands"
        ]
    undefined_quantified_helpers = (
        transitive_undefined_quantified_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if undefined_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with undefined quantified helper "
            f"{', '.join(undefined_quantified_helpers)}; define named "
            "concrete model predicates before composing exactness predicate "
            "chains"
        ]
    vacuous_quantified_helpers = transitive_vacuous_quantified_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if vacuous_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with vacuous quantified helper "
            f"{', '.join(vacuous_quantified_helpers)}; keep literal and "
            "self-equality, self-inequality, empty-domain, singleton-domain, self-membership, or empty-set membership quantified helper bodies out "
            "of exactness predicate chains"
        ]
    duplicate_bound_quantified_helpers = (
        transitive_duplicate_bound_quantified_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if duplicate_bound_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with duplicate quantified helper binding "
            f"{', '.join(duplicate_bound_quantified_helpers)}; bind each "
            "quantified identifier once before composing exactness predicate "
            "chains"
        ]
    unused_bound_quantified_helpers = (
        transitive_unused_bound_quantified_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if unused_bound_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with unused quantified helper binding "
            f"{', '.join(unused_bound_quantified_helpers)}; use every bound "
            "identifier inside quantified model predicates before composing "
            "exactness predicate chains"
        ]
    control_flow_quantified_helpers = (
        transitive_control_flow_quantified_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if control_flow_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with control-flow quantified helper "
            f"{', '.join(control_flow_quantified_helpers)}; name concrete "
            "quantified model predicates instead of selecting predicates "
            "inside quantified helper bodies"
        ]
    negated_quantified_helpers = transitive_negated_quantified_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if negated_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with negated quantified helper "
            f"{', '.join(negated_quantified_helpers)}; compose positive "
            "quantified model predicates before exactness predicate chains"
        ]
    existential_quantified_helpers = (
        transitive_existential_quantified_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            module_path,
        )
    )
    if existential_quantified_helpers:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with existential quantified helper "
            f"{', '.join(existential_quantified_helpers)}; use universal "
            "quantified model predicates before composing exactness predicate "
            "chains"
        ]
    transitive_undefined_conjuncts = transitive_undefined_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if transitive_undefined_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with undefined conjunct "
            f"{', '.join(transitive_undefined_conjuncts)}; define named "
            "concrete model predicates before composing exactness predicate "
            "chains"
        ]
    transitive_nonzero_arity_conjuncts = (
        transitive_nonzero_arity_exactness_conjuncts(
            exactness_operator,
            exactness_body,
            definitions,
            signatures,
            module_path,
        )
    )
    if transitive_nonzero_arity_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with non-zero-arity conjunct "
            f"{', '.join(transitive_nonzero_arity_conjuncts)}; exactness "
            "predicate chains must compose zero-arity model predicates"
        ]
    parameterized_helper_calls = parameterized_exactness_helper_calls(
        exactness_operator,
        exactness_body,
        definitions,
        signatures,
        module_path,
    )
    if parameterized_helper_calls:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with parameterized helper call "
            f"{', '.join(parameterized_helper_calls)}; lift exactness helper "
            "calls behind zero-arity model predicates"
        ]
    transitive_vacuous_conjuncts = transitive_vacuous_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if transitive_vacuous_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with vacuous conjunct "
            f"{', '.join(transitive_vacuous_conjuncts)}; keep literal, "
            "self-equality, self-inequality, and alias helpers out of named "
            "exactness predicate chains"
        ]
    temporal_let_alias_conjuncts = transitive_unary_temporal_let_alias_exactness_conjuncts(
        exactness_operator,
        exactness_body,
        definitions,
        module_path,
    )
    if temporal_let_alias_conjuncts:
        return [
            f"{prefix} at "
            f"{display_path(module_path)}:{exactness_line} contains transitive "
            "exactness predicate chain with unary-temporal LET alias "
            f"{', '.join(temporal_let_alias_conjuncts)}; name concrete model "
            "predicates before composing exactness predicate chains"
        ]
    return []


def vacuous_helper_leaf_messages(
    root: str,
    current: str,
    chain: list[str],
    line: int,
    body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    *,
    exactness: bool,
) -> list[str]:
    """Return vacuity messages for a helper leaf body."""

    messages: list[str] = []
    stripped_body = strip_static_outer_parentheses(body)
    compact_body = " ".join(stripped_body.split())
    literal_body = tla_static_temporal_boolean_literal(stripped_body)
    if (not exactness or len(chain) > 1) and literal_body is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is literal {literal_body}"
        )
    static_if_literal = tla_static_if_boolean_literal(stripped_body)
    if (not exactness or len(chain) > 1) and static_if_literal is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is static IF literal "
            f"{static_if_literal}"
        )
    constant_relation = tla_static_constant_relation(stripped_body)
    if constant_relation is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is constant relation "
            f"{constant_relation}"
        )
    self_equality_body = tla_static_self_equality(stripped_body)
    if self_equality_body is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is self-equality "
            f"{self_equality_body}"
        )
    else:
        self_equality_parts = temporal_self_equality_parts(stripped_body)
        if self_equality_parts:
            messages.append(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} contains "
                f"self-equality {', '.join(self_equality_parts)}"
            )
    self_inequality_body = tla_static_self_inequality(stripped_body)
    if self_inequality_body is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is self-inequality "
            f"{self_inequality_body}"
        )
    else:
        self_inequality_parts = temporal_self_inequality_parts(stripped_body)
        if self_inequality_parts:
            messages.append(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} contains "
                f"self-inequality {', '.join(self_inequality_parts)}"
            )
    body_identifiers = tla_static_non_string_identifiers(compact_body)
    if (
        (not exactness or len(chain) > 1)
        and len(body_identifiers) == 1
        and compact_body in body_identifiers
    ):
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} aliases {compact_body}"
        )
    literal_gated_alias = literal_gated_zero_arity_helper_alias(
        stripped_body,
        definitions,
    )
    if literal_gated_alias is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} aliases "
            f"{literal_gated_alias} through a literal-gated helper operand"
        )
    single_conjunct_alias = single_zero_arity_conjunct_alias(
        stripped_body,
        definitions,
    )
    if single_conjunct_alias is not None:
        messages.append(
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} aliases "
            f"{single_conjunct_alias} through a single helper conjunct"
        )
    return messages


def transitive_vacuous_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return literal or alias helpers below direct exactness predicates."""

    vacuous: list[str] = []
    seen_messages: set[str] = set()

    def record(message: str) -> None:
        if message in seen_messages:
            return
        seen_messages.add(message)
        vacuous.append(message)

    def inspect_hidden_references(
        root: str,
        chain: list[str],
        body: str,
    ) -> None:
        for reference in hidden_static_structured_helper_references(body):
            if reference == chain[-1] or reference == exactness_operator:
                continue
            definition = definitions.get(reference)
            if definition is None:
                continue
            reference_line, reference_body = definition
            hidden_chain = chain + [reference]
            for message in vacuous_helper_leaf_messages(
                root,
                reference,
                hidden_chain,
                reference_line,
                reference_body,
                definitions,
                module_path,
                exactness=True,
            ):
                record(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        inspect_hidden_references(root, chain, stripped_body)
        literal_body = tla_static_temporal_boolean_literal(stripped_body)
        if len(chain) > 1 and literal_body is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is literal "
                f"{literal_body}"
            )
        static_if_literal = tla_static_if_boolean_literal(stripped_body)
        if len(chain) > 1 and static_if_literal is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is static IF literal "
                f"{static_if_literal}"
            )
        constant_relation = tla_static_constant_relation(stripped_body)
        if constant_relation is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is constant relation "
                f"{constant_relation}"
            )
        self_equality_body = tla_static_self_equality(stripped_body)
        if self_equality_body is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is self-equality "
                f"{self_equality_body}"
            )
        else:
            self_equality_parts = temporal_self_equality_parts(stripped_body)
            if self_equality_parts:
                record(
                    f"{root} reaches {current} through {' -> '.join(chain)} "
                    f"at {display_path(module_path)}:{line} contains "
                    f"self-equality {', '.join(self_equality_parts)}"
                )
        self_inequality_body = tla_static_self_inequality(stripped_body)
        if self_inequality_body is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is self-inequality "
                f"{self_inequality_body}"
            )
        else:
            self_inequality_parts = temporal_self_inequality_parts(stripped_body)
            if self_inequality_parts:
                record(
                    f"{root} reaches {current} through {' -> '.join(chain)} "
                    f"at {display_path(module_path)}:{line} contains "
                    f"self-inequality {', '.join(self_inequality_parts)}"
                )
        body_identifiers = tla_static_non_string_identifiers(compact_body)
        if (
            len(chain) > 1
            and len(body_identifiers) == 1
            and compact_body in body_identifiers
        ):
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} aliases "
                f"{compact_body}"
            )
        literal_gated_alias = literal_gated_zero_arity_helper_alias(
            stripped_body,
            definitions,
        )
        if literal_gated_alias is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} aliases "
                f"{literal_gated_alias} through a literal-gated helper operand"
            )
        single_conjunct_alias = single_zero_arity_conjunct_alias(
            stripped_body,
            definitions,
        )
        if single_conjunct_alias is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} aliases "
                f"{single_conjunct_alias} through a single helper conjunct"
            )
        for reference in exactness_helper_references(stripped_body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return vacuous


def transitive_hidden_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return hidden coverage identifiers reached below direct exactness predicates."""

    hidden: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, identifier: str) -> None:
        message = (
            f"{root} reaches {identifier} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        hidden.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for identifier in sorted(tla_static_non_string_identifiers(body)):
            if identifier == exactness_operator or identifier == current:
                continue
            is_hidden_coverage = (
                identifier == "TypeInvariant"
                or identifier in GENERIC_CORRECTNESS_CHECKS
                or identifier.endswith("Exactness")
            )
            if is_hidden_coverage and len(chain) > 1:
                record(root, chain, line, identifier)
                continue
            if identifier not in definitions:
                continue
            walk(root, identifier, chain + [identifier], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return hidden


def transitive_duplicate_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return repeated named helper conjuncts below direct exactness predicates."""

    duplicates: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, repeated: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} repeats {repeated}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        duplicates.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for repeated in (
            duplicate_zero_arity_conjunct_references(body)
            + duplicate_zero_arity_wrapped_conjunct_references(body)
        ):
            if repeated in definitions:
                record(root, current, chain, line, repeated)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return duplicates


def transitive_duplicate_boolean_operand_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return repeated named boolean operands below direct exactness predicates."""

    duplicates: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, repeated: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} repeats {repeated}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        duplicates.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for repeated in duplicate_zero_arity_boolean_operand_references(body):
            if repeated in definitions:
                record(root, current, chain, line, repeated)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return duplicates


def transitive_contradictory_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return contradictory operands below direct exactness predicates."""

    contradictory: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operand: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} pairs {operand} with ~{operand}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        contradictory.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operand in contradictory_zero_arity_conjunct_references(body):
            if operand in definitions:
                record(root, current, chain, line, operand)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return contradictory


def transitive_excluded_middle_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return excluded-middle operands below direct exactness predicates."""

    excluded: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operand: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} pairs {operand} with ~{operand}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        excluded.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operand in excluded_middle_zero_arity_disjunct_references(body):
            if operand in definitions:
                record(root, current, chain, line, operand)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return excluded


def transitive_complementary_equivalence_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return complementary-equivalence operands below exactness predicates."""

    complementary: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operand: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} pairs {operand} with "
            f"~{operand} under equivalence"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        complementary.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operand in complementary_equivalence_zero_arity_references(body):
            if operand in definitions:
                record(root, current, chain, line, operand)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return complementary


def transitive_control_flow_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return whole-body control-flow helpers below direct exactness predicates."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operator: str, body: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is whole-body {operator} "
            f"expression {body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(compact_body)
        if (
            whole_body_control is not None
            and tla_control_flow_helper_selects_predicate(compact_body)
        ):
            record(
                root,
                current,
                chain,
                line,
                whole_body_control.group(1),
                compact_body,
            )
        for reference in exactness_helper_references(stripped_body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return control_flow


def nested_control_flow_helper_formulas(
    body: str,
    definitions: dict[str, tuple[int, str]],
) -> list[tuple[str, str]]:
    """Return nested control-flow formulas that select named helper predicates."""

    control_flow: list[tuple[str, str]] = []
    seen_bodies: set[tuple[str, bool]] = set()
    seen_control: set[str] = set()

    def collect(current: str, is_root: bool) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, is_root)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, is_root)
            return

        whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(normalized)
        if whole_body_control is not None:
            named_helpers = control_flow_named_helper_branch_operands(
                normalized,
                definitions,
            )
            if named_helpers and not is_root and normalized not in seen_control:
                seen_control.add(normalized)
                control_flow.append((whole_body_control.group(1), normalized))
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, False)
            return

        if tla_unary_temporal_operand(normalized) is not None:
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part, False)

    collect(body, True)
    return control_flow


def control_flow_named_helper_branch_operands(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> list[str]:
    """Return named helper predicates directly selected by IF/CASE branches."""

    compact = strip_static_outer_parentheses(" ".join(expression.split()))
    branches: list[str] = []
    if re.match(r"^IF\b", compact):
        if_branches = tla_top_level_if_branches(compact)
        if if_branches is not None:
            branches.extend(if_branches)
    elif re.match(r"^CASE\b", compact):
        branches.extend(tla_top_level_case_result_branches(compact))

    operands: list[str] = []
    seen: set[str] = set()
    control = TLA_WHOLE_BODY_CONTROL_RE.match(compact)
    if (
        not branches
        and control is not None
        and control.group(1) in {"CHOOSE", "ENABLED"}
    ):
        for identifier in sorted(tla_static_non_string_identifiers(compact)):
            if (
                identifier not in seen
                and identifier in definitions
                and is_tla_helper_identifier(identifier)
                and not identifier.startswith("Bug")
            ):
                seen.add(identifier)
                operands.append(identifier)
        return operands

    for branch in branches:
        operand = exactness_boolean_helper_operand_name(branch)
        if (
            operand not in seen
            and TLA_IDENTIFIER_RE.fullmatch(operand)
            and operand in definitions
            and not operand.startswith("Bug")
        ):
            seen.add(operand)
            operands.append(operand)
    return operands


def helper_definition_is_predicate_like(
    helper: str,
    definitions: dict[str, tuple[int, str]],
    seen: set[str] | None = None,
) -> bool:
    """Return whether a zero-arity helper definition looks predicate-shaped."""

    if seen is None:
        seen = set()
    if helper in seen:
        return False
    seen.add(helper)
    definition = definitions.get(helper)
    if definition is None:
        return False
    _, body = definition
    stripped_body = strip_static_outer_parentheses(body)
    compact_body = " ".join(stripped_body.split())
    if tla_static_temporal_boolean_literal(stripped_body) is not None:
        return True
    if tla_static_if_boolean_literal(stripped_body) is not None:
        return True
    if tla_unary_temporal_operand(stripped_body) is not None:
        return True
    if TLA_WHOLE_BODY_QUANTIFIER_RE.match(compact_body):
        return True
    if tla_top_level_relation_parts(stripped_body) is not None:
        return True
    if len(tla_top_level_boolean_parts(stripped_body)) > 1:
        return True
    if TLA_IDENTIFIER_RE.fullmatch(compact_body) and compact_body in definitions:
        return helper_definition_is_predicate_like(
            compact_body,
            definitions,
            seen.copy(),
        )
    return False


def control_flow_named_predicate_branch_operands(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> list[str]:
    """Return named helper branch operands whose definitions are predicates."""

    return [
        operand
        for operand in control_flow_named_helper_branch_operands(
            expression,
            definitions,
        )
        if helper_definition_is_predicate_like(operand, definitions)
    ]


def tla_top_level_case_result_branches(expression: str) -> list[str]:
    """Return top-level CASE result branches from a static CASE expression."""

    branches: list[str] = []
    for _, result in tla_top_level_case_condition_result_branches(expression):
        branches.append(result)
    return branches


def tla_top_level_case_condition_result_branches(
    expression: str,
) -> list[tuple[str, str]]:
    """Return top-level CASE condition/result branch pairs."""

    branches: list[tuple[str, str]] = []
    for arm in tla_top_level_case_arms(expression):
        arrow_index = tla_top_level_case_arrow_index(arm)
        if arrow_index is None:
            continue
        condition = arm[:arrow_index].strip()
        result = arm[arrow_index + 2 :].strip()
        if condition and result:
            branches.append(
                (
                    strip_static_outer_parentheses(condition),
                    strip_static_outer_parentheses(result),
                )
            )
    return branches


def tla_top_level_case_arms(expression: str) -> list[str]:
    """Return top-level CASE arm text from a static CASE expression."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not re.match(r"^CASE\b", text):
        return []
    arms: list[str] = []
    current: list[str] = []
    depth = 0
    in_string = False
    escaped = False
    index = len("CASE")
    while index < len(text):
        char = text[index]
        if in_string:
            current.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            current.append(char)
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            current.append("<<")
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            current.append(">>")
            index += 2
            continue
        if depth == 0 and text.startswith("[]", index) and (
            tla_case_arm_has_result("".join(current))
        ):
            arm = "".join(current).strip()
            if arm:
                arms.append(arm)
            current = []
            index += 2
            continue
        if char in "([{":
            depth += 1
        elif char in ")]}" and depth > 0:
            depth -= 1
        current.append(char)
        index += 1

    arm = "".join(current).strip()
    if arm:
        arms.append(arm)

    return arms


def tla_case_arm_has_result(text: str) -> bool:
    """Return whether a partial CASE arm has a top-level arrow and result."""

    arrow_index = tla_top_level_case_arrow_index(text)
    if arrow_index is None:
        return False
    return bool(text[arrow_index + 2 :].strip())


def tla_top_level_case_arrow_index(text: str) -> int | None:
    """Return the top-level CASE arm arrow index, if present."""

    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if depth == 0 and text.startswith("->", index):
            return index
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        index += 1
    return None


def transitive_nested_control_flow_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return nested control-flow helpers below direct exactness predicates."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operator: str, body: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} contains nested {operator} "
            f"expression {body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operator, control_body in nested_control_flow_helper_formulas(
            body,
            definitions,
        ):
            record(root, current, chain, line, operator, control_body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return control_flow


def unary_temporal_control_flow_exactness_helpers(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return control-flow formulas below unary-temporal exactness wrappers."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        operator: str,
        body: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is unary-temporal "
            f"{operator} expression {body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operator, control_body in unary_temporal_control_flow_formulas(body):
            record(root, current, chain, line, operator, control_body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return control_flow


def structured_operand_control_flow_exactness_helpers(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return control-flow formulas below structured exactness operands."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in structured_operand_control_flow_formulas(body, definitions):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return control_flow


def transitive_unary_temporal_let_alias_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return unary-temporal LET aliases below exactness helper chains."""

    aliases: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, body_aliases: list[str]) -> None:
        message = (
            f"{root} reaches {chain[-1]} through {' -> '.join(chain)} at "
            f"{display_path(module_path)}:{line} contains "
            f"{', '.join(body_aliases)}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        aliases.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        body_aliases = unary_temporal_let_alias_parts(body)
        if body_aliases:
            record(root, chain, line, body_aliases)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return aliases


def unary_temporal_control_flow_formulas(body: str) -> list[tuple[str, str]]:
    """Return control-flow expressions below unary temporal wrappers."""

    control_flow: list[tuple[str, str]] = []
    seen_bodies: set[tuple[str, bool]] = set()
    seen_control: set[str] = set()

    def collect(current: str, in_temporal: bool) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, in_temporal)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, in_temporal)
            return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, in_temporal)
            return

        whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(normalized)
        if (
            in_temporal
            and whole_body_control is not None
            and tla_control_flow_helper_selects_predicate(normalized)
        ):
            if normalized not in seen_control:
                seen_control.add(normalized)
                control_flow.append((whole_body_control.group(1), normalized))
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, in_temporal)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, True)
            return

    collect(body, False)
    return control_flow


def transitive_boolean_composition_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return whole-body boolean composition over raw predicate helpers."""

    boolean_composition: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        kind: str,
        body: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is whole-body {kind} "
            f"{body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        boolean_composition.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        kind = exactness_helper_boolean_composition_kind(stripped_body, definitions)
        if kind is not None:
            record(root, current, chain, line, kind, compact_body)
        for reference in exactness_helper_references(stripped_body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return boolean_composition


def exactness_helper_boolean_composition_kind(
    body: str,
    definitions: dict[str, tuple[int, str]],
) -> str | None:
    """Return boolean-composition kind when operands are raw predicate helpers."""

    return exactness_boolean_composition_kind_through_operands(
        body,
        lambda expression: exactness_helper_boolean_composition_kind_direct(
            expression,
            definitions,
        ),
    )


def exactness_helper_boolean_composition_kind_direct(
    body: str,
    definitions: dict[str, tuple[int, str]],
) -> str | None:
    """Return direct boolean-composition kind over raw predicate helpers."""

    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    negated_operand = tla_static_negation_operand(compact_body)
    if negated_operand is not None and is_raw_scalar_helper_operand(
        negated_operand,
        definitions,
    ):
        return "negation"
    literal_gated_negated_operand = literal_gated_negated_zero_arity_helper_operand(
        compact_body,
        definitions,
    )
    if (
        literal_gated_negated_operand is not None
        and is_raw_scalar_helper_operand(literal_gated_negated_operand, definitions)
    ):
        return "negation"
    if exactness_boolean_parts_are_raw_scalar_helpers(
        tla_top_level_disjuncts(compact_body),
        definitions,
    ):
        return "disjunction"
    if exactness_boolean_parts_are_raw_scalar_helpers(
        tla_top_level_implication_operands(compact_body),
        definitions,
    ):
        return "implication"
    if exactness_boolean_parts_are_raw_scalar_helpers(
        tla_top_level_equivalence_operands(compact_body),
        definitions,
    ):
        return "equivalence"
    return None


def exactness_boolean_composition_kind_through_operands(
    body: str,
    direct_kind: Callable[[str], str | None],
) -> str | None:
    """Return a boolean-composition kind visible through boolean operands."""

    seen: set[str] = set()

    def collect(current: str, allow_negation: bool) -> str | None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        if not normalized or normalized in seen:
            return None
        seen.add(normalized)

        kind = direct_kind(normalized)
        if kind is not None and (allow_negation or kind != "negation"):
            return kind

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            return collect(let_operand, allow_negation)

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            return collect(negated_operand, False)

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            result = collect(part, False)
            if result is not None:
                return result
        return None

    return collect(body, True)


def exactness_boolean_parts_are_raw_scalar_helpers(
    parts: list[str],
    definitions: dict[str, tuple[int, str]],
) -> bool:
    """Return whether top-level boolean operands are raw predicate helpers."""

    return len(parts) > 1 and all(
        is_raw_scalar_helper_operand(part, definitions) for part in parts
    )


def exactness_boolean_helper_operand_name(expression: str) -> str:
    """Return a normalized helper operand, peeling static unary wrappers."""

    operand = strip_static_outer_parentheses(" ".join(expression.split()))
    while True:
        let_operand = tla_static_let_alias_operand(operand)
        if let_operand is not None:
            operand = strip_static_outer_parentheses(" ".join(let_operand.split()))
            continue
        negated_operand = tla_static_negation_operand(operand)
        if negated_operand is not None:
            operand = strip_static_outer_parentheses(
                " ".join(negated_operand.split())
            )
            continue
        temporal_operand = tla_unary_temporal_operand(operand)
        if temporal_operand is not None:
            operand = strip_static_outer_parentheses(
                " ".join(temporal_operand.split())
            )
            continue
        break
    return operand


def tla_static_let_alias_operand(expression: str) -> str | None:
    """Return the operand of a transparent one-line `LET` alias."""

    compact = strip_static_outer_parentheses(" ".join(expression.split()))
    if not re.match(r"^LET\b", compact):
        return None
    in_index = tla_top_level_keyword_index(compact, "IN", start=len("LET"))
    if in_index is None:
        return None
    binding = compact[len("LET") : in_index].strip()
    result = strip_static_outer_parentheses(compact[in_index + len("IN") :].strip())
    bindings = tla_static_let_binding_definitions(binding)
    if not bindings:
        return None
    return tla_static_resolve_let_alias_result(result, bindings)


def tla_static_let_binding_entries(binding: str) -> list[TlaLetBinding] | None:
    """Return simple one-line LET definitions with optional parameters."""

    def signature_before(operator_index: int) -> tuple[int, str, frozenset[str]] | None:
        signature_end = operator_index
        while signature_end > 0 and binding[signature_end - 1].isspace():
            signature_end -= 1
        if signature_end <= 0:
            return None

        name_end = signature_end
        params: frozenset[str] = frozenset()
        if binding[signature_end - 1] == ")":
            depth = 0
            open_index: int | None = None
            scan = signature_end - 1
            while scan >= 0:
                char = binding[scan]
                if char == ")":
                    depth += 1
                elif char == "(":
                    depth -= 1
                    if depth == 0:
                        open_index = scan
                        break
                scan -= 1
            if open_index is None:
                return None
            param_parts = tla_top_level_argument_parts(
                binding[open_index + 1 : signature_end - 1].strip()
            )
            if not param_parts or any(
                not is_tla_operator_name(param) for param in param_parts
            ):
                return None
            if len(set(param_parts)) != len(param_parts):
                return None
            params = frozenset(param_parts)
            name_end = open_index

        while name_end > 0 and binding[name_end - 1].isspace():
            name_end -= 1
        name_start = name_end
        while name_start > 0 and (
            binding[name_start - 1].isalnum() or binding[name_start - 1] == "_"
        ):
            name_start -= 1
        name = binding[name_start:name_end]
        if not is_tla_operator_name(name):
            return None
        return name_start, name, params

    markers: list[tuple[int, int, int, str, frozenset[str]]] = []
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(binding):
        char = binding[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if binding.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if binding.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth == 0 and binding.startswith("==", index):
            signature = signature_before(index)
            if signature is None:
                return None
            name_start, alias, params = signature
            markers.append((name_start, index, index, alias, params))
            index += 2
            continue
        index += 1

    if not markers or binding[: markers[0][0]].strip():
        return None

    definitions: list[TlaLetBinding] = []
    seen_aliases: set[str] = set()
    for marker_index, (_, _, operator_index, alias, params) in enumerate(markers):
        if alias in seen_aliases:
            return None
        seen_aliases.add(alias)
        operand_start = operator_index + len("==")
        operand_end = (
            markers[marker_index + 1][0]
            if marker_index + 1 < len(markers)
            else len(binding)
        )
        operand = strip_static_outer_parentheses(
            binding[operand_start:operand_end].strip()
        )
        if not operand:
            return None
        definitions.append(TlaLetBinding(alias, params, operand))
    return definitions


def tla_static_let_binding_definitions(binding: str) -> list[tuple[str, str]] | None:
    """Return simple one-line non-parameterized LET definitions."""

    entries = tla_static_let_binding_entries(binding)
    if entries is None or any(entry.params for entry in entries):
        return None
    return [(entry.name, entry.operand) for entry in entries]


def tla_static_resolve_let_alias_result(
    result: str,
    bindings: list[tuple[str, str]],
) -> str | None:
    """Resolve a transparent LET alias result through chained bindings."""

    current = strip_static_outer_parentheses(" ".join(result.split()))
    seen_results: set[str] = set()
    used_alias = False
    while current:
        if current in seen_results:
            return None
        seen_results.add(current)
        for alias, operand in bindings:
            substituted = tla_static_alias_result_operand(current, alias, operand)
            if substituted is None:
                continue
            current = strip_static_outer_parentheses(" ".join(substituted.split()))
            used_alias = True
            break
        else:
            substituted = tla_static_substitute_let_alias_references(
                current,
                bindings,
            )
            if substituted is not None and substituted != current:
                current = strip_static_outer_parentheses(" ".join(substituted.split()))
                used_alias = True
                continue
            return current if used_alias else None
    return None


def tla_static_substitute_let_alias_references(
    expression: str,
    bindings: list[tuple[str, str]],
) -> str | None:
    """Substitute simple unshadowed LET aliases inside an expression."""

    alias_operands = dict(bindings)
    if not alias_operands:
        return None
    without_strings = tla_without_string_literals(expression)
    if re.search(r"\bLET\b", without_strings):
        return None
    bound = tla_quantified_bound_identifiers(expression)
    if bound & set(alias_operands):
        return None

    pieces: list[str] = []
    last_index = 0
    changed = False
    in_string = False
    escaped = False
    index = 0
    while index < len(expression):
        char = expression[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        match = TLA_IDENTIFIER_SCAN_RE.match(expression, index)
        if match is None:
            index += 1
            continue
        identifier = match.group(0)
        operand = alias_operands.get(identifier)
        if operand is None:
            index = match.end()
            continue
        next_index = match.end()
        while next_index < len(expression) and expression[next_index].isspace():
            next_index += 1
        if next_index < len(expression) and expression[next_index] == "(":
            index = match.end()
            continue
        pieces.append(expression[last_index : match.start()])
        pieces.append(f"({operand})")
        last_index = match.end()
        changed = True
        index = match.end()
    if not changed:
        return None
    pieces.append(expression[last_index:])
    return "".join(pieces)


def tla_static_alias_result_operand(
    result: str,
    alias: str,
    operand: str,
) -> str | None:
    """Substitute a LET alias result that only adds static unary wrappers."""

    current = strip_static_outer_parentheses(" ".join(result.split()))
    wrappers: list[str] = []
    while True:
        negated_operand = tla_static_negation_operand(current)
        if negated_operand is not None:
            wrappers.append("~")
            current = strip_static_outer_parentheses(
                " ".join(negated_operand.split())
            )
            continue
        temporal_operand = tla_unary_temporal_operator_operand(current)
        if temporal_operand is not None:
            operator, nested_operand = temporal_operand
            wrappers.append(operator)
            current = strip_static_outer_parentheses(
                " ".join(nested_operand.split())
            )
            continue
        break
    if current != alias:
        return None

    substituted = strip_static_outer_parentheses(operand)
    for wrapper in reversed(wrappers):
        substituted = f"{wrapper} ({substituted})"
    return substituted


def is_raw_scalar_helper_operand(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> bool:
    """Return whether an expression names a raw scalar predicate helper."""

    operand = exactness_boolean_helper_operand_name(expression)
    definition = definitions.get(operand)
    if TLA_IDENTIFIER_RE.fullmatch(operand) is None or definition is None:
        return False
    _, body = definition
    return (
        TLA_IDENTIFIER_EQUALITY_RE.fullmatch(strip_static_outer_parentheses(body))
        is not None
    )


def transitive_parameterized_call_boolean_composition_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
    module_path: Path,
) -> list[str]:
    """Return whole-body boolean composition over parameterized-call helpers."""

    boolean_composition: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        kind: str,
        body: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is whole-body {kind} "
            f"{body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        boolean_composition.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        kind = exactness_parameterized_call_boolean_composition_kind(
            stripped_body,
            definitions,
            signatures,
        )
        if kind is not None:
            record(root, current, chain, line, kind, compact_body)
        for reference in exactness_helper_references(stripped_body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return boolean_composition


def exactness_parameterized_call_boolean_composition_kind(
    body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> str | None:
    """Return boolean-composition kind over parameterized-call helper leaves."""

    return exactness_boolean_composition_kind_through_operands(
        body,
        lambda expression: exactness_parameterized_call_boolean_composition_kind_direct(
            expression,
            definitions,
            signatures,
        ),
    )


def exactness_parameterized_call_boolean_composition_kind_direct(
    body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> str | None:
    """Return direct boolean-composition kind over parameterized-call leaves."""

    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    cache_key = (compact_body, id(definitions), id(signatures))
    if cache_key in _EXACTNESS_PARAMETERIZED_CALL_BOOLEAN_KIND_DIRECT_CACHE:
        return _EXACTNESS_PARAMETERIZED_CALL_BOOLEAN_KIND_DIRECT_CACHE[cache_key]

    kind = exactness_parameterized_call_boolean_composition_kind_direct_uncached(
        compact_body,
        definitions,
        signatures,
    )
    _EXACTNESS_PARAMETERIZED_CALL_BOOLEAN_KIND_DIRECT_CACHE[cache_key] = kind
    return kind


def exactness_parameterized_call_boolean_composition_kind_direct_uncached(
    compact_body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> str | None:
    """Return direct parameterized-call composition kind without memoization."""

    negated_operand = tla_static_negation_operand(compact_body)
    if negated_operand is not None and is_parameterized_call_helper_operand(
        negated_operand,
        definitions,
        signatures,
    ):
        return "negation"
    literal_gated_negated_operand = literal_gated_negated_zero_arity_helper_operand(
        compact_body,
        definitions,
    )
    if (
        literal_gated_negated_operand is not None
        and is_parameterized_call_helper_operand(
            literal_gated_negated_operand,
            definitions,
            signatures,
        )
    ):
        return "negation"
    if exactness_boolean_parts_include_parameterized_call_helpers(
        tla_top_level_disjuncts(compact_body),
        definitions,
        signatures,
    ):
        return "disjunction"
    if exactness_boolean_parts_include_parameterized_call_helpers(
        tla_top_level_implication_operands(compact_body),
        definitions,
        signatures,
    ):
        return "implication"
    if exactness_boolean_parts_include_parameterized_call_helpers(
        tla_top_level_equivalence_operands(compact_body),
        definitions,
        signatures,
    ):
        return "equivalence"
    return None


def exactness_boolean_parts_include_parameterized_call_helpers(
    parts: list[str],
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> bool:
    """Return whether boolean operands include parameterized-call helper leaves."""

    if len(parts) <= 1:
        return False
    helper_statuses = [
        exactness_simple_leaf_helper_status(part, definitions, signatures)
        for part in parts
    ]
    return all(status is not None for status in helper_statuses) and any(
        helper_statuses
    )


def exactness_simple_leaf_helper_status(
    expression: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> bool | None:
    """Return True for parameterized-call leaves, False for raw leaves, else None."""

    if is_parameterized_call_helper_operand(expression, definitions, signatures):
        return True
    if is_raw_scalar_helper_operand(expression, definitions):
        return False
    return None


def is_parameterized_call_helper_operand(
    expression: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> bool:
    """Return whether an expression names a direct parameterized-call helper."""

    operand = exactness_boolean_helper_operand_name(expression)
    definition = definitions.get(operand)
    if TLA_IDENTIFIER_RE.fullmatch(operand) is None or definition is None:
        return False
    _, body = definition
    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    callee = tla_direct_operator_call_name(compact_body)
    if callee is None:
        return False
    signature = signatures.get(callee)
    return signature is not None and signature[1] != 0


def transitive_quantified_boolean_composition_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
    module_path: Path,
) -> list[str]:
    """Return whole-body boolean composition over quantified helper leaves."""

    boolean_composition: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        kind: str,
        body: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is whole-body {kind} "
            f"{body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        boolean_composition.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        kind = exactness_quantified_boolean_composition_kind(
            stripped_body,
            definitions,
            signatures,
        )
        if kind is not None:
            record(root, current, chain, line, kind, compact_body)
        for reference in exactness_helper_references(stripped_body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return boolean_composition


def exactness_quantified_boolean_composition_kind(
    body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> str | None:
    """Return boolean-composition kind over quantified helper leaves."""

    return exactness_boolean_composition_kind_through_operands(
        body,
        lambda expression: exactness_quantified_boolean_composition_kind_direct(
            expression,
            definitions,
            signatures,
        ),
    )


def exactness_quantified_boolean_composition_kind_direct(
    body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> str | None:
    """Return direct boolean-composition kind over quantified helper leaves."""

    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    cache_key = (compact_body, id(definitions), id(signatures))
    if cache_key in _EXACTNESS_QUANTIFIED_BOOLEAN_KIND_DIRECT_CACHE:
        return _EXACTNESS_QUANTIFIED_BOOLEAN_KIND_DIRECT_CACHE[cache_key]

    kind = exactness_quantified_boolean_composition_kind_direct_uncached(
        compact_body,
        definitions,
        signatures,
    )
    _EXACTNESS_QUANTIFIED_BOOLEAN_KIND_DIRECT_CACHE[cache_key] = kind
    return kind


def exactness_quantified_boolean_composition_kind_direct_uncached(
    compact_body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> str | None:
    """Return direct quantified composition kind without memoization."""

    negated_operand = tla_static_negation_operand(compact_body)
    if negated_operand is not None and is_quantified_helper_operand(
        negated_operand,
        definitions,
    ):
        return "negation"
    literal_gated_negated_operand = literal_gated_negated_zero_arity_helper_operand(
        compact_body,
        definitions,
    )
    if (
        literal_gated_negated_operand is not None
        and is_quantified_helper_operand(literal_gated_negated_operand, definitions)
    ):
        return "negation"
    if exactness_boolean_parts_include_quantified_helpers(
        tla_top_level_disjuncts(compact_body),
        definitions,
        signatures,
    ):
        return "disjunction"
    if exactness_boolean_parts_include_quantified_helpers(
        tla_top_level_implication_operands(compact_body),
        definitions,
        signatures,
    ):
        return "implication"
    if exactness_boolean_parts_include_quantified_helpers(
        tla_top_level_equivalence_operands(compact_body),
        definitions,
        signatures,
    ):
        return "equivalence"
    return None


def exactness_boolean_parts_include_quantified_helpers(
    parts: list[str],
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
) -> bool:
    """Return whether boolean operands include quantified helper leaves."""

    if len(parts) <= 1:
        return False
    has_quantified_helper = False
    for part in parts:
        if is_quantified_helper_operand(part, definitions):
            has_quantified_helper = True
            continue
        if exactness_simple_leaf_helper_status(part, definitions, signatures) is None:
            return False
    return has_quantified_helper


def is_quantified_helper_operand(
    expression: str,
    definitions: dict[str, tuple[int, str]],
) -> bool:
    """Return whether an expression names a quantified helper predicate."""

    operand = exactness_boolean_helper_operand_name(expression)
    definition = definitions.get(operand)
    if TLA_IDENTIFIER_RE.fullmatch(operand) is None or definition is None:
        return False
    _, body = definition
    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    return is_scoped_quantified_formula(compact_body)


def is_scoped_quantified_formula(formula: str) -> bool:
    """Return whether a whole-body quantified formula has scoped binders."""

    normalized = strip_static_outer_parentheses(" ".join(formula.split()))
    return (
        TLA_WHOLE_BODY_QUANTIFIER_RE.match(normalized) is not None
        and tla_quantifier_scope(normalized) is not None
    )


def unary_temporal_quantified_exactness_helpers(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return quantified formulas below unary-temporal exactness wrappers."""

    quantified: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        quantified.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in unary_temporal_quantified_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return quantified


def static_wrapped_quantified_exactness_helpers(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return quantified formulas below static wrappers in exactness chains."""

    quantified: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        quantified.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in static_wrapped_quantified_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return quantified


def structured_operand_quantified_exactness_helpers(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return quantified formulas below structured operands in exactness chains."""

    quantified: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        quantified.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in structured_operand_quantified_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return quantified


def unary_temporal_quantified_formulas(body: str) -> list[str]:
    """Return whole-body quantified formulas below unary temporal wrappers."""

    formulas: list[str] = []
    seen_bodies: set[tuple[str, bool]] = set()
    seen_formulas: set[str] = set()

    def collect(current: str, in_temporal: bool) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, in_temporal)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)
        if in_temporal and is_scoped_quantified_formula(normalized):
            if normalized not in seen_formulas:
                seen_formulas.add(normalized)
                formulas.append(normalized)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, in_temporal)
            return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, in_temporal)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, in_temporal)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, True)
            return

    collect(body, False)
    return formulas


def static_wrapped_quantified_formulas(body: str) -> list[str]:
    """Return quantified formulas below static action/set/choice wrappers."""

    wrapped: list[str] = []
    seen_bodies: set[tuple[str, str | None]] = set()
    seen_wrapped: set[str] = set()

    def record(wrapper: str, formula: str) -> None:
        message = f"{wrapper} wraps {formula}"
        if message in seen_wrapped:
            return
        seen_wrapped.add(message)
        wrapped.append(message)

    def collect(current: str, wrapper: str | None) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, wrapper)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)
        if wrapper is not None and is_scoped_quantified_formula(normalized):
            record(wrapper, normalized)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, wrapper)
            return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, wrapper)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, wrapper)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, wrapper)
            return

        tuple_values = tla_tuple_literal_values(normalized)
        if tuple_values is not None:
            for value in tuple_values:
                collect(value, wrapper)
            return

        set_scope = tla_set_comprehension_scope(normalized)
        if set_scope is not None:
            domains, set_body, _ = set_scope
            for domain in domains:
                collect(domain, wrapper)
            collect(set_body, wrapper)
            return

        set_elements = tla_explicit_set_elements(normalized)
        if set_elements is not None:
            for element in set_elements:
                collect(element, wrapper)
            return

        function_scope = tla_function_constructor_scope(normalized)
        if function_scope is not None:
            domains, function_body, _ = function_scope
            for domain in domains:
                collect(domain, wrapper)
            collect(function_body, wrapper)
            return

        function_set_scope = tla_function_set_scope(normalized)
        if function_set_scope is not None:
            domain, range_expression = function_set_scope
            collect(domain, wrapper)
            collect(range_expression, wrapper)
            return

        record_values = tla_record_literal_values(normalized)
        if record_values is not None:
            for value in record_values:
                collect(value, wrapper)
            return

        record_domains = tla_record_set_field_domains(normalized)
        if record_domains is not None:
            for domain in record_domains:
                collect(domain, wrapper)
            return

        record_update = tla_record_update_scope(normalized)
        if record_update is not None:
            base, selectors, replacements = record_update
            collect(base, wrapper)
            for selector in selectors:
                collect(selector, wrapper)
            for replacement in replacements:
                collect(replacement, wrapper)
            return

        action = tla_unary_action_operator_operand(normalized)
        if action is not None:
            operator, operand = action
            collect(operand, operator)
            return

        unary_set = tla_unary_set_operator_expression_operand(normalized)
        if unary_set is not None:
            operator, operand = unary_set
            collect(operand, operator)
            return

        choose_split = tla_choose_prefix_and_body(normalized)
        if choose_split is not None:
            prefix, choose_body = choose_split
            for domain in tla_choose_bound_domains(prefix):
                collect(domain, "CHOOSE")
            collect(choose_body, "CHOOSE")
            return

        lambda_scope = tla_lambda_scope(normalized)
        if lambda_scope is not None:
            domains, lambda_body, _ = lambda_scope
            for domain in domains:
                collect(domain, "LAMBDA")
            collect(lambda_body, "LAMBDA")
            return

        case_branches = tla_top_level_case_condition_result_branches(normalized)
        if case_branches:
            for condition, result in case_branches:
                collect(condition, wrapper)
                collect(result, wrapper)
            return

        if_parts = tla_top_level_if_parts(normalized)
        if if_parts is not None:
            for part in if_parts:
                collect(part, wrapper)
            return

        relation_parts = tla_top_level_relation_parts(normalized)
        if relation_parts is not None:
            left, _, right = relation_parts
            collect(left, wrapper)
            collect(right, wrapper)
            return

        infix_operands = tla_top_level_static_infix_operands(normalized)
        if infix_operands is not None:
            for operand in infix_operands:
                collect(operand, wrapper)
            return

        call_arguments = tla_direct_operator_call_arguments(normalized)
        if call_arguments is not None:
            for argument in tla_top_level_argument_parts(call_arguments):
                if argument:
                    collect(argument, wrapper)
            return

        selector_scope = tla_selector_scope(normalized)
        if selector_scope is not None:
            base, selectors = selector_scope
            collect(base, wrapper)
            for selector in selectors:
                collect(selector, wrapper)
            return

    collect(body, None)
    return wrapped


def structured_operand_quantified_formulas(body: str) -> list[str]:
    """Return quantified formulas hidden inside structured helper operands."""

    structured: list[str] = []
    seen_bodies: set[tuple[str, str | None]] = set()
    seen_structured: set[str] = set()

    def record(container: str, formula: str) -> None:
        message = f"{container} contains {formula}"
        if message in seen_structured:
            return
        seen_structured.add(message)
        structured.append(message)

    def collect(current: str, container: str | None) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, container)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)
        if is_scoped_quantified_formula(normalized):
            if container is not None:
                record(container, normalized)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, container)
            return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, container)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, container)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, container)
            return

        tuple_values = tla_tuple_literal_values(normalized)
        if tuple_values is not None:
            nested = container or "tuple literal"
            for value in tuple_values:
                collect(value, nested)
            return

        set_scope = tla_set_comprehension_scope(normalized)
        if set_scope is not None:
            domains, set_body, _ = set_scope
            nested = container or "set comprehension"
            for domain in domains:
                collect(domain, nested)
            collect(set_body, nested)
            return

        set_elements = tla_explicit_set_elements(normalized)
        if set_elements is not None:
            nested = container or "explicit set literal"
            for element in set_elements:
                collect(element, nested)
            return

        function_scope = tla_function_constructor_scope(normalized)
        if function_scope is not None:
            domains, function_body, _ = function_scope
            nested = container or "function constructor"
            for domain in domains:
                collect(domain, nested)
            collect(function_body, nested)
            return

        function_set_scope = tla_function_set_scope(normalized)
        if function_set_scope is not None:
            domain, range_expression = function_set_scope
            nested = container or "function set"
            collect(domain, nested)
            collect(range_expression, nested)
            return

        record_values = tla_record_literal_values(normalized)
        if record_values is not None:
            nested = container or "record literal"
            for value in record_values:
                collect(value, nested)
            return

        record_domains = tla_record_set_field_domains(normalized)
        if record_domains is not None:
            nested = container or "record set"
            for domain in record_domains:
                collect(domain, nested)
            return

        record_update = tla_record_update_scope(normalized)
        if record_update is not None:
            base, selectors, replacements = record_update
            nested = container or "record update"
            collect(base, nested)
            for selector in selectors:
                collect(selector, nested)
            for replacement in replacements:
                collect(replacement, nested)
            return

        case_branches = tla_top_level_case_condition_result_branches(normalized)
        if case_branches:
            nested = container or "CASE expression"
            for condition, result in case_branches:
                collect(condition, nested)
                collect(result, nested)
            return

        if_parts = tla_top_level_if_parts(normalized)
        if if_parts is not None:
            nested = container or "IF expression"
            for part in if_parts:
                collect(part, nested)
            return

        relation_parts = tla_top_level_relation_parts(normalized)
        if relation_parts is not None:
            left, _, right = relation_parts
            nested = container or "relation expression"
            collect(left, nested)
            collect(right, nested)
            return

        infix_operands = tla_top_level_static_infix_operands(normalized)
        if infix_operands is not None:
            nested = container or "infix expression"
            for operand in infix_operands:
                collect(operand, nested)
            return

        call_arguments = tla_direct_operator_call_arguments(normalized)
        if call_arguments is not None:
            nested = container or "operator call"
            for argument in tla_top_level_argument_parts(call_arguments):
                if argument:
                    collect(argument, nested)
            return

        selector_scope = tla_selector_scope(normalized)
        if selector_scope is not None:
            base, selectors = selector_scope
            nested = container or "selector expression"
            collect(base, nested)
            for selector in selectors:
                collect(selector, nested)
            return

    collect(body, None)
    return structured


def structured_operand_control_flow_formulas(
    body: str,
    definitions: dict[str, tuple[int, str]],
) -> list[str]:
    """Return control-flow formulas hidden inside structured helper operands."""

    structured: list[str] = []
    seen_bodies: set[tuple[str, str | None]] = set()
    seen_structured: set[str] = set()

    def record(container: str, operator: str, formula: str) -> None:
        message = f"{container} contains {operator} expression {formula}"
        if message in seen_structured:
            return
        seen_structured.add(message)
        structured.append(message)

    def inspect_control(normalized: str, container: str | None) -> bool:
        whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(normalized)
        if whole_body_control is None:
            return False
        operator = whole_body_control.group(1)
        if (
            container is not None
            and control_flow_named_predicate_branch_operands(normalized, definitions)
        ):
            record(container, operator, normalized)
        return True

    def collect(current: str, container: str | None) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, container)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, container)
            return

        if inspect_control(normalized, container):
            control_container = container or "control-flow expression"
            case_branches = tla_top_level_case_condition_result_branches(normalized)
            if case_branches:
                for condition, result in case_branches:
                    collect(condition, control_container)
                    collect(result, control_container)
                return
            if_parts = tla_top_level_if_parts(normalized)
            if if_parts is not None:
                for part in if_parts:
                    collect(part, control_container)
                return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, container)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, container)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, container)
            return

        tuple_values = tla_tuple_literal_values(normalized)
        if tuple_values is not None:
            nested = container or "tuple literal"
            for value in tuple_values:
                collect(value, nested)
            return

        set_scope = tla_set_comprehension_scope(normalized)
        if set_scope is not None:
            domains, set_body, _ = set_scope
            nested = container or "set comprehension"
            for domain in domains:
                collect(domain, nested)
            collect(set_body, nested)
            return

        set_elements = tla_explicit_set_elements(normalized)
        if set_elements is not None:
            nested = container or "explicit set literal"
            for element in set_elements:
                collect(element, nested)
            return

        function_scope = tla_function_constructor_scope(normalized)
        if function_scope is not None:
            domains, function_body, _ = function_scope
            nested = container or "function constructor"
            for domain in domains:
                collect(domain, nested)
            collect(function_body, nested)
            return

        function_set_scope = tla_function_set_scope(normalized)
        if function_set_scope is not None:
            domain, range_expression = function_set_scope
            nested = container or "function set"
            collect(domain, nested)
            collect(range_expression, nested)
            return

        record_values = tla_record_literal_values(normalized)
        if record_values is not None:
            nested = container or "record literal"
            for value in record_values:
                collect(value, nested)
            return

        record_domains = tla_record_set_field_domains(normalized)
        if record_domains is not None:
            nested = container or "record set"
            for domain in record_domains:
                collect(domain, nested)
            return

        record_update = tla_record_update_scope(normalized)
        if record_update is not None:
            base, selectors, replacements = record_update
            nested = container or "record update"
            collect(base, nested)
            for selector in selectors:
                collect(selector, nested)
            for replacement in replacements:
                collect(replacement, nested)
            return

        action = tla_unary_action_operator_operand(normalized)
        if action is not None:
            _, operand = action
            nested = container or "action wrapper"
            collect(operand, nested)
            return

        unary_set = tla_unary_set_operator_expression_operand(normalized)
        if unary_set is not None:
            _, operand = unary_set
            nested = container or "unary set wrapper"
            collect(operand, nested)
            return

        choose_split = tla_choose_prefix_and_body(normalized)
        if choose_split is not None:
            prefix, choose_body = choose_split
            nested = container or "CHOOSE expression"
            for domain in tla_choose_bound_domains(prefix):
                collect(domain, nested)
            collect(choose_body, nested)
            return

        lambda_scope = tla_lambda_scope(normalized)
        if lambda_scope is not None:
            domains, lambda_body, _ = lambda_scope
            nested = container or "LAMBDA expression"
            for domain in domains:
                collect(domain, nested)
            collect(lambda_body, nested)
            return

        relation_parts = tla_top_level_relation_parts(normalized)
        if relation_parts is not None:
            left, _, right = relation_parts
            nested = container or "relation expression"
            collect(left, nested)
            collect(right, nested)
            return

        infix_operands = tla_top_level_static_infix_operands(normalized)
        if infix_operands is not None:
            nested = container or "infix expression"
            for operand in infix_operands:
                collect(operand, nested)
            return

        call_arguments = tla_direct_operator_call_arguments(normalized)
        if call_arguments is not None:
            nested = container or "operator call"
            for argument in tla_top_level_argument_parts(call_arguments):
                if argument:
                    collect(argument, nested)
            return

        selector_scope = tla_selector_scope(normalized)
        if selector_scope is not None:
            base, selectors = selector_scope
            nested = container or "selector expression"
            collect(base, nested)
            for selector in selectors:
                collect(selector, nested)
            return

    collect(body, None)
    return structured


def quantified_helper_formulas(body: str) -> list[str]:
    """Return quantified formulas that helper wrappers should inspect."""

    formulas: list[str] = []
    seen: set[str] = set()
    seen_bodies: set[str] = set()
    stripped_body = strip_static_outer_parentheses(body)

    def add(formula: str) -> None:
        if formula in seen:
            return
        seen.add(formula)
        formulas.append(formula)

    def collect(current: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)
        if is_scoped_quantified_formula(normalized):
            add(normalized)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(stripped_body)
    for formula in unary_temporal_quantified_formulas(stripped_body):
        add(formula)
    return formulas


def quantified_formula_body(formula: str) -> str | None:
    """Return a whole-body quantified formula body, if one can be split."""

    split = quantified_formula_prefix_and_body(formula)
    return None if split is None else split[1]


def quantified_formula_prefix_and_body(formula: str) -> tuple[str, str] | None:
    """Return a whole-body quantified formula prefix and body, if present."""

    normalized = strip_static_outer_parentheses(" ".join(formula.split()))
    if TLA_WHOLE_BODY_QUANTIFIER_RE.match(normalized) is None:
        return None
    colon_index = tla_top_level_symbol_index(normalized, ":")
    if colon_index is None:
        return None
    prefix = normalized[:colon_index].strip()
    body = normalized[colon_index + 1 :].strip()
    if not prefix or not body:
        return None
    return prefix, strip_static_outer_parentheses(body)


def quantified_formula_bound_identifiers(formula: str) -> set[str]:
    """Return top-level identifiers bound by a whole-body quantified formula."""

    split = quantified_formula_prefix_and_body(formula)
    if split is None:
        return set()
    prefix, _ = split
    return tla_quantifier_binding_identifiers(prefix)


def quantified_formula_bound_identifier_sequence(formula: str) -> list[str]:
    """Return quantified bound identifiers in binding-prefix order."""

    if tla_quantifier_scope(formula) is None:
        return []
    split = quantified_formula_prefix_and_body(formula)
    if split is None:
        return []
    prefix, _ = split
    prefix = re.sub(r"^\\[AE]\s+", "", prefix.strip(), count=1).strip()
    return tla_binding_identifier_sequence_from_prefix(prefix)


def duplicate_identifiers_in_order(identifiers: list[str]) -> list[str]:
    """Return duplicate identifiers once, preserving first duplicate order."""

    seen: set[str] = set()
    duplicates: list[str] = []
    duplicate_seen: set[str] = set()
    for identifier in identifiers:
        if identifier in seen and identifier not in duplicate_seen:
            duplicates.append(identifier)
            duplicate_seen.add(identifier)
        seen.add(identifier)
    return duplicates


def quantified_formula_duplicate_bound_identifiers(formula: str) -> list[str]:
    """Return duplicated bound identifiers in a quantified formula."""

    return duplicate_identifiers_in_order(
        quantified_formula_bound_identifier_sequence(formula)
    )


def quantified_formula_bound_domains(formula: str) -> dict[str, str]:
    """Return simple top-level quantified bindings mapped to their domains."""

    split = quantified_formula_prefix_and_body(formula)
    if split is None:
        return {}
    prefix, _ = split
    prefix = prefix.strip()
    prefix = re.sub(r"^\\[AE]\s+", "", prefix, count=1).strip()
    domains: dict[str, str] = {}
    pending_names: list[str] = []
    for binding in tla_top_level_argument_parts(prefix):
        match = re.match(r"^(.+?)\s+\\in\s+(.+)$", binding)
        if match is None:
            pending_names.append(binding)
            continue
        names, domain = match.groups()
        compact_domain = strip_static_outer_parentheses(" ".join(domain.split()))
        if not compact_domain:
            pending_names = []
            continue
        for name_part in [*pending_names, names]:
            identifier = strip_static_outer_parentheses(
                " ".join(name_part.split())
            )
            if (
                TLA_IDENTIFIER_RE.fullmatch(identifier)
                and is_tla_user_identifier(identifier)
            ):
                domains[identifier] = compact_domain
        pending_names = []
    return domains


def quantified_formula_domain_expressions(formula: str) -> list[str]:
    """Return top-level quantified domain expressions in scan order."""

    split = quantified_formula_prefix_and_body(formula)
    if split is None:
        return []
    prefix, _ = split
    prefix = prefix.strip()
    prefix = re.sub(r"^\\[AE]\s+", "", prefix, count=1).strip()
    domains: list[str] = []
    for binding in tla_top_level_argument_parts(prefix):
        membership = tla_top_level_membership_parts(binding)
        if membership is None or membership[1] != "\\in":
            continue
        domain = strip_static_outer_parentheses(" ".join(membership[2].split()))
        if domain:
            domains.append(domain)
    return domains


def tla_top_level_membership_parts(
    expression: str,
) -> tuple[str, str, str] | None:
    """Return top-level membership operands from a static expression."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if tla_relation_scan_starts_with_wrapper(text):
        return None
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        for operator in ("\\notin", "\\in"):
            if not text.startswith(operator, index):
                continue
            before = text[index - 1] if index > 0 else ""
            after_index = index + len(operator)
            after = text[after_index] if after_index < len(text) else ""
            if (before.isalnum() or before == "_") or (
                after.isalnum() or after == "_"
            ):
                continue
            left = text[:index].strip()
            right = text[after_index:].strip()
            if left and right:
                return left, operator, right
        index += 1
    return None


def tla_top_level_relation_parts(expression: str) -> tuple[str, str, str] | None:
    """Return top-level relation operands from a static expression."""

    for relation in (
        tla_top_level_membership_parts(expression),
        tla_top_level_subset_relation_parts(expression),
        tla_top_level_equality_relation_parts(expression),
        tla_top_level_order_relation_parts(expression),
    ):
        if relation is not None:
            return relation
    return None


def tla_top_level_subset_relation_parts(
    expression: str,
) -> tuple[str, str, str] | None:
    """Return top-level subset-relation operands from a static expression."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if tla_relation_scan_starts_with_wrapper(text):
        return None
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        operator = "\\subseteq"
        if not text.startswith(operator, index):
            index += 1
            continue
        before = text[index - 1] if index > 0 else ""
        after_index = index + len(operator)
        after = text[after_index] if after_index < len(text) else ""
        if (before.isalnum() or before == "_") or (
            after.isalnum() or after == "_"
        ):
            index += 1
            continue
        left = text[:index].strip()
        right = text[after_index:].strip()
        if left and right:
            return left, operator, right
        index += 1
    return None


def tla_top_level_equality_relation_parts(
    expression: str,
) -> tuple[str, str, str] | None:
    """Return top-level equality or inequality operands from a static expression."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if tla_relation_scan_starts_with_wrapper(text):
        return None
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        for operator in ("/=", "#", "="):
            if operator == "=":
                if char != "=":
                    continue
                previous_char = text[index - 1] if index > 0 else ""
                next_char = text[index + 1] if index + 1 < len(text) else ""
                if previous_char in "<>/" or next_char == ">":
                    continue
            elif not text.startswith(operator, index):
                continue
            left = text[:index].strip()
            right = text[index + len(operator) :].strip()
            if left and right:
                return left, operator, right
        index += 1
    return None


def tla_top_level_order_relation_parts(
    expression: str,
) -> tuple[str, str, str] | None:
    """Return top-level ordering relation operands from a static expression."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if tla_relation_scan_starts_with_wrapper(text):
        return None
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        for operator in ("<=", ">=", "<", ">"):
            if not text.startswith(operator, index):
                continue
            if operator == "<=":
                next_char = text[index + 2] if index + 2 < len(text) else ""
                if next_char == ">":
                    continue
            if operator == "<":
                next_char = text[index + 1] if index + 1 < len(text) else ""
                if next_char in "<=>":
                    continue
            if operator == ">":
                previous_char = text[index - 1] if index > 0 else ""
                if previous_char in "<>=" or previous_char == ":":
                    continue
            left = text[:index].strip()
            right = text[index + len(operator) :].strip()
            if left and right:
                return left, operator, right
        index += 1
    return None


def tla_top_level_static_infix_operands(expression: str) -> list[str] | None:
    """Return operands split by supported top-level static infix operators."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    operands: list[str] = []
    start = 0
    found = False
    depth = 0
    in_string = False
    escaped = False
    index = 0
    while index < len(text):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if text.startswith("<<", index):
            depth += 1
            index += 2
            continue
        if text.startswith(">>", index) and depth > 0:
            depth -= 1
            index += 2
            continue
        if char in "([{":
            depth += 1
            index += 1
            continue
        if char in ")]}" and depth > 0:
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        matched_operator = None
        for operator in TLA_STATIC_INFIX_OPERATORS:
            if not text.startswith(operator, index):
                continue
            if not tla_static_infix_operator_is_binary(text, index, operator):
                continue
            matched_operator = operator
            break
        if matched_operator is None:
            index += 1
            continue
        operand = text[start:index].strip()
        if not operand:
            return None
        operands.append(operand)
        start = index + len(matched_operator)
        found = True
        index = start
    if not found:
        return None
    final_operand = text[start:].strip()
    if not final_operand:
        return None
    operands.append(final_operand)
    return operands


def tla_static_infix_operator_is_binary(text: str, index: int, operator: str) -> bool:
    """Return whether a supported infix operator occurrence is binary."""

    left = text[:index].strip()
    right = text[index + len(operator) :].strip()
    if not left or not right:
        return False
    previous = text[index - 1] if index > 0 else ""
    next_char = text[index + len(operator)] if index + len(operator) < len(text) else ""
    if operator == "-":
        if next_char == ">":
            return False
        previous_nonspace = left[-1]
        if previous_nonspace in "([{+-*/%<>=#":
            return False
    if operator == "\\":
        if next_char.isalpha():
            return False
    if operator.startswith("\\") and len(operator) > 1:
        after = text[index + len(operator)] if index + len(operator) < len(text) else ""
        if after.isalnum() or after == "_":
            return False
    if operator == "..":
        before = text[index - 1] if index > 0 else ""
        after = text[index + 2] if index + 2 < len(text) else ""
        if before == "." or after == ".":
            return False
    return previous != "\\" or operator.startswith("\\")


def tla_explicit_set_elements(expression: str) -> list[str] | None:
    """Return normalized top-level elements from an explicit set literal."""

    text = strip_static_outer_parentheses(" ".join(expression.split()))
    if not tla_outer_curly_braces_enclose_expression(text):
        return None
    inner = text[1:-1].strip()
    if not inner:
        return []
    return [
        strip_static_outer_parentheses(" ".join(element.split()))
        for element in tla_top_level_argument_parts(inner)
    ]


def tla_explicit_set_contains_identifier(expression: str, identifier: str) -> bool:
    """Return whether an explicit set literal contains an identifier element."""

    elements = tla_explicit_set_elements(expression)
    if elements is None:
        return False
    return identifier in elements


def tla_explicit_set_is_empty(expression: str) -> bool:
    """Return whether an expression is an explicit empty set literal."""

    return tla_explicit_set_elements(expression) == []


def tla_explicit_singleton_set_element(expression: str) -> str | None:
    """Return the sole element of an explicit singleton set literal."""

    elements = tla_explicit_set_elements(expression)
    if elements is None or len(elements) != 1:
        return None
    return elements[0]


def quantified_formula_self_membership_body(formula: str) -> str | None:
    """Return a quantified body that only restates bound or empty-set membership."""

    formula_body = quantified_formula_inspection_body(formula)
    if formula_body is None:
        return None
    bound_domains = quantified_formula_bound_domains(formula)
    if not bound_domains:
        return None

    def collect(body: str, seen: set[str] | None = None) -> str | None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        seen = set() if seen is None else seen
        if normalized in seen:
            return None
        seen = {*seen, normalized}

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            return normalized if collect(let_operand, seen) is not None else None

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            return (
                normalized if collect(temporal_operand, seen) is not None else None
            )

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            return normalized if collect(negated_operand, seen) is not None else None

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1 and all(
            collect(part, seen) is not None for part in boolean_parts
        ):
            return normalized

        identity_gated_operand = tla_identity_literal_gated_operand(
            normalized,
            lambda part: collect(part, seen) is not None,
        )
        if identity_gated_operand is not None:
            return normalized

        membership = tla_top_level_membership_parts(normalized)
        if membership is None:
            return None
        left, operator, right = membership
        left = strip_static_outer_parentheses(" ".join(left.split()))
        right = strip_static_outer_parentheses(" ".join(right.split()))
        if (
            operator in {"\\in", "\\notin"}
            and left in bound_domains
            and (
                right == bound_domains[left]
                or tla_explicit_set_contains_identifier(right, left)
                or tla_explicit_set_is_empty(right)
            )
        ):
            return normalized
        return None

    return collect(formula_body)


def quantified_formula_singleton_domain_relation_body(formula: str) -> str | None:
    """Return a quantified body that only restates a singleton domain."""

    formula_body = quantified_formula_inspection_body(formula)
    if formula_body is None:
        return None
    singleton_domains = {
        identifier: element
        for identifier, domain in quantified_formula_bound_domains(formula).items()
        if (element := tla_explicit_singleton_set_element(domain)) is not None
    }
    if not singleton_domains:
        return None

    def collect(body: str, seen: set[str] | None = None) -> str | None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        seen = set() if seen is None else seen
        if normalized in seen:
            return None
        seen = {*seen, normalized}

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            return normalized if collect(let_operand, seen) is not None else None

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            return (
                normalized if collect(temporal_operand, seen) is not None else None
            )

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            return normalized if collect(negated_operand, seen) is not None else None

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1 and all(
            collect(part, seen) is not None for part in boolean_parts
        ):
            return normalized

        identity_gated_operand = tla_identity_literal_gated_operand(
            normalized,
            lambda part: collect(part, seen) is not None,
        )
        if identity_gated_operand is not None:
            return normalized

        relation = tla_top_level_equality_relation_parts(normalized)
        if relation is None:
            return None
        left, operator, right = relation
        if operator not in {"=", "#", "/="}:
            return None
        left = strip_static_outer_parentheses(" ".join(left.split()))
        right = strip_static_outer_parentheses(" ".join(right.split()))
        if singleton_domains.get(left) == right or singleton_domains.get(right) == left:
            return normalized
        return None

    return collect(formula_body)


def tla_negated_boolean_literal(literal: str | None) -> str | None:
    """Return the negated boolean literal, if one is known."""

    if literal == "TRUE":
        return "FALSE"
    if literal == "FALSE":
        return "TRUE"
    return None


def quantified_formula_restatement_literal(formula: str) -> str | None:
    """Return a literal value for quantified restatement-only formulas."""

    formula_body = quantified_formula_inspection_body(formula)
    if formula_body is None:
        return None
    bound_domains = quantified_formula_bound_domains(formula)
    if not bound_domains:
        return None
    singleton_domains = {
        identifier: element
        for identifier, domain in bound_domains.items()
        if (element := tla_explicit_singleton_set_element(domain)) is not None
    }

    def membership_restatement_literal(body: str) -> str | None:
        membership = tla_top_level_membership_parts(body)
        if membership is None:
            return None
        left, operator, right = membership
        left = strip_static_outer_parentheses(" ".join(left.split()))
        right = strip_static_outer_parentheses(" ".join(right.split()))
        if operator not in {"\\in", "\\notin"} or left not in bound_domains:
            return None
        if right == bound_domains[left] or tla_explicit_set_contains_identifier(
            right, left
        ):
            return "TRUE" if operator == "\\in" else "FALSE"
        if tla_explicit_set_is_empty(right):
            return "FALSE" if operator == "\\in" else "TRUE"
        return None

    def singleton_relation_literal(body: str) -> str | None:
        relation = tla_top_level_equality_relation_parts(body)
        if relation is None:
            return None
        left, operator, right = relation
        if operator not in {"=", "#", "/="}:
            return None
        left = strip_static_outer_parentheses(" ".join(left.split()))
        right = strip_static_outer_parentheses(" ".join(right.split()))
        if singleton_domains.get(left) != right and singleton_domains.get(right) != left:
            return None
        return "TRUE" if operator == "=" else "FALSE"

    def collect(body: str, seen: set[str] | None = None) -> str | None:
        normalized = strip_static_outer_parentheses(" ".join(body.split()))
        seen = set() if seen is None else seen
        if normalized in seen:
            return None
        seen = {*seen, normalized}

        literal = tla_static_temporal_boolean_literal(normalized)
        if literal is not None:
            return literal

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            return collect(let_operand, seen)

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            return collect(temporal_operand, seen)

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            return tla_negated_boolean_literal(collect(negated_operand, seen))

        for literal_of in (
            membership_restatement_literal,
            singleton_relation_literal,
        ):
            literal = literal_of(normalized)
            if literal is not None:
                return literal

        return tla_compound_temporal_boolean_literal(
            normalized,
            lambda part: collect(part, seen),
        )

    return collect(formula_body)


def quantified_formula_has_empty_bound_domain(formula: str) -> bool:
    """Return whether any quantified binding has an explicit empty domain."""

    return any(
        tla_explicit_set_is_empty(domain)
        for domain in quantified_formula_bound_domains(formula).values()
    )


def unused_bound_quantified_helper_formulas(body: str) -> list[str]:
    """Return quantified formulas whose body does not use all bound names."""

    unused: list[str] = []
    for formula in quantified_helper_formulas(body):
        formula_body = quantified_formula_body(formula)
        if formula_body is None:
            continue
        bound = quantified_formula_bound_identifiers(formula)
        body_identifiers = tla_static_identifiers(
            tla_without_string_literals(formula_body)
        )
        unused_bound = sorted(bound - body_identifiers)
        if unused_bound:
            unused.append(f"{formula} omits bound {', '.join(unused_bound)}")
    return unused


def control_flow_quantified_helper_formulas(body: str) -> list[str]:
    """Return quantified formulas whose body selects predicates dynamically."""

    control_flow: list[str] = []
    for formula in quantified_helper_formulas(body):
        formula_body = quantified_formula_inspection_body(formula)
        if formula_body is None:
            continue
        compact_body = " ".join(strip_static_outer_parentheses(formula_body).split())
        control = TLA_QUANTIFIED_BODY_PREDICATE_SELECTION_RE.match(compact_body)
        if control is None:
            continue
        control_flow.append(f"{formula} uses {control.group(1)}")
    return control_flow


def negated_quantified_helper_formulas(body: str) -> list[str]:
    """Return quantified formulas hidden behind top-level negation operands."""

    negated: list[str] = []
    seen_bodies: set[tuple[str, int]] = set()
    seen_formulas: set[str] = set()

    def record(formula: str, negations: int) -> None:
        message = f"{formula} under {negations} top-level negation(s)"
        if message in seen_formulas:
            return
        seen_formulas.add(message)
        negated.append(message)

    def collect(current: str, inherited_negations: int) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, inherited_negations)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, inherited_negations)
            return

        negations = inherited_negations
        operand = normalized
        peeled_negation = False
        while True:
            negated_operand = tla_static_negation_operand(operand)
            if negated_operand is None:
                break
            peeled_negation = True
            negations += 1
            operand = strip_static_outer_parentheses(
                " ".join(negated_operand.split())
            )

        if negations and is_scoped_quantified_formula(operand):
            record(operand, negations)
            return

        if peeled_negation:
            collect(operand, negations)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, negations)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, negations)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part, negations)

    collect(body, 0)
    return negated


def vacuous_quantified_helper_formulas(body: str) -> list[str]:
    """Return quantified formulas whose bodies are static or contradictory."""

    vacuous: list[str] = []
    for formula in quantified_helper_formulas(body):
        formula_body = quantified_formula_inspection_body(formula)
        if formula_body is None:
            continue
        if tla_static_temporal_boolean_literal(formula_body) is not None:
            vacuous.append(formula)
            continue
        if temporal_self_equality_parts(formula_body):
            vacuous.append(formula)
            continue
        if temporal_self_inequality_parts(formula_body):
            vacuous.append(formula)
            continue
        if quantified_formula_has_empty_bound_domain(formula):
            vacuous.append(formula)
            continue
        if quantified_formula_restatement_literal(formula) is not None:
            vacuous.append(formula)
            continue
        if quantified_formula_singleton_domain_relation_body(formula) is not None:
            vacuous.append(formula)
            continue
        if quantified_formula_self_membership_body(formula) is not None:
            vacuous.append(formula)
    return vacuous


def quantified_formula_inspection_body(formula: str) -> str | None:
    """Return a quantified body with transparent static LET aliases unwrapped."""

    formula_body = quantified_formula_body(formula)
    if formula_body is None:
        return None
    normalized = strip_static_outer_parentheses(" ".join(formula_body.split()))
    seen: set[str] = set()
    while normalized and normalized not in seen:
        seen.add(normalized)
        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is None:
            break
        normalized = strip_static_outer_parentheses(" ".join(let_operand.split()))
    return normalized


def existential_quantified_helper_formulas(body: str) -> list[str]:
    """Return existential quantified helper formulas that weaken obligations."""

    return [
        formula
        for formula in quantified_helper_formulas(body)
        if formula.startswith("\\E ")
    ]


def duplicate_bound_quantified_helper_formulas(body: str) -> list[str]:
    """Return quantified formulas that duplicate bound identifiers."""

    duplicated: list[str] = []
    for formula in quantified_helper_formulas(body):
        duplicate_bound = quantified_formula_duplicate_bound_identifiers(formula)
        if duplicate_bound:
            duplicated.append(
                f"{formula} duplicates bound {', '.join(duplicate_bound)}"
            )
    return duplicated


def transitive_undefined_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return undefined helpers inside quantified exactness helper formulas."""

    undefined: list[str] = []
    seen_messages: set[str] = set()
    parameter_names = tla_operator_parameter_names(module_path)

    def record(root: str, chain: list[str], line: int, reference: str) -> None:
        message = (
            f"{root} reaches {reference} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        undefined.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        local_bound = parameter_names.get(chain[-1], frozenset())
        for formula in quantified_helper_formulas(body):
            for reference in undefined_static_helper_identifiers(
                formula,
                definitions,
                module_path,
                current=chain[-1],
                exactness_operator=exactness_operator,
                local_bound=local_bound,
            ):
                record(root, chain, line, reference)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference in parameter_names.get(current, frozenset()):
                continue
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return undefined


def transitive_vacuous_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return vacuous quantified helpers below exactness helper chains."""

    vacuous: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        vacuous.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in vacuous_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return vacuous


def transitive_duplicate_bound_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return quantified helpers with duplicate bindings below exactness chains."""

    duplicated: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        duplicated.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in duplicate_bound_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return duplicated


def transitive_unused_bound_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return quantified helpers with unused bindings below exactness chains."""

    unused: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        unused.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in unused_bound_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return unused


def transitive_control_flow_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return quantified helpers with control-flow-selected predicates."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in control_flow_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return control_flow


def transitive_negated_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return negated quantified helpers below exactness chains."""

    negated: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        negated.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in negated_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return negated


def transitive_existential_quantified_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return existential quantified helpers below exactness helper chains."""

    existential: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        existential.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in existential_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return existential


def transitive_undefined_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return undefined helpers below direct exactness predicates."""

    undefined: list[str] = []
    seen_messages: set[str] = set()
    declared_names = {
        *tla_constant_declarations(module_path),
        *(variable for _, variable in tla_variable_declaration_entries(module_path)),
    }
    parameter_names = tla_operator_parameter_names(module_path)

    def record(root: str, chain: list[str], line: int, reference: str) -> None:
        message = (
            f"{root} reaches {reference} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        undefined.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        local_bound = parameter_names.get(current, frozenset())
        for reference in exactness_helper_references(body):
            if reference in local_bound:
                continue
            if reference == current or reference == exactness_operator:
                continue
            if reference in declared_names:
                continue
            if reference not in definitions:
                if is_tla_helper_identifier(reference):
                    record(root, chain, line, reference)
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return undefined


def transitive_nonzero_arity_exactness_conjuncts(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
    module_path: Path,
) -> list[str]:
    """Return non-zero-arity helpers below direct exactness predicates."""

    nonzero_arity: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, arity: int) -> None:
        message = (
            f"{root} reaches {' -> '.join(chain)} at "
            f"{display_path(module_path)}:{line} with arity {arity}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        nonzero_arity.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        _, body = definition
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            signature = signatures.get(reference)
            if signature is not None and signature[1] != 0:
                record(root, chain + [reference], signature[0], signature[1])
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        signature = signatures.get(conjunct_operator)
        if signature is not None and signature[1] != 0:
            continue
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return nonzero_arity


def parameterized_exactness_helper_calls(
    exactness_operator: str,
    exactness_body: str,
    definitions: dict[str, tuple[int, str]],
    signatures: dict[str, tuple[int, int]],
    module_path: Path,
) -> list[str]:
    """Return direct parameterized helper calls in exactness helper chains."""

    calls: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, call: str) -> None:
        message = (
            f"{root} reaches {call} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        calls.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for call in (
            *unary_temporal_parameterized_calls(body),
            *compound_parameterized_helper_calls(body, signatures),
        ):
            record(root, chain, line, call)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in exactness_helper_references(body):
            if reference == current or reference == exactness_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for conjunct_operator in tla_zero_arity_conjunct_references(exactness_body):
        if conjunct_operator not in definitions:
            continue
        walk(conjunct_operator, conjunct_operator, [conjunct_operator], set())
    return calls


def compound_parameterized_helper_calls(
    body: str,
    signatures: dict[str, tuple[int, int]],
) -> list[str]:
    """Return non-root parameterized calls inside compound helper bodies."""

    calls: list[str] = []
    seen_bodies: set[tuple[str, bool]] = set()
    seen_calls: set[str] = set()

    def record(call: str) -> None:
        callee = tla_direct_operator_call_name(call)
        if callee is None:
            return
        signature = signatures.get(callee)
        if signature is None or signature[1] == 0:
            return
        if not tla_direct_operator_call_has_complex_argument(call):
            return
        if call in seen_calls:
            return
        seen_calls.add(call)
        calls.append(call)

    def collect(current: str, is_root: bool) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, is_root)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)

        if tla_direct_operator_call_name(normalized) is not None:
            if not is_root:
                record(normalized)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, is_root)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, False)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, False)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part, False)

    collect(body, True)
    return calls


def unary_temporal_parameterized_calls(body: str) -> list[str]:
    """Return direct parameterized calls below unary temporal wrappers."""

    calls: list[str] = []
    seen_bodies: set[tuple[str, bool]] = set()
    seen_calls: set[str] = set()

    def collect(current: str, in_temporal: bool) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        key = (normalized, in_temporal)
        if not normalized or key in seen_bodies:
            return
        seen_bodies.add(key)
        if (
            in_temporal
            and tla_direct_operator_call_name(normalized) is not None
        ):
            if normalized not in seen_calls:
                seen_calls.add(normalized)
                calls.append(normalized)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, in_temporal)
            return

        boolean_parts = tla_top_level_boolean_parts(normalized)
        if len(boolean_parts) > 1:
            for part in boolean_parts:
                collect(part, in_temporal)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, in_temporal)
            return

        temporal_operand = tla_unary_temporal_operand(normalized)
        if temporal_operand is not None:
            collect(temporal_operand, True)
            return

    collect(body, False)
    return calls


def tla_whole_body_boolean_composition_kind(body: str) -> str | None:
    """Return the top-level boolean-composition shape, if one exists."""

    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    if compact_body.startswith("~"):
        return "negation"
    if compact_body.startswith(("/\\", "[]", "<>")):
        return None
    if tla_has_top_level_disjunction(body):
        return "disjunction"
    if tla_has_top_level_implication(body):
        return "implication"
    if tla_has_top_level_equivalence(body):
        return "equivalence"
    return None


def temporal_extra_definition_shape_errors(
    mode: str,
    module_path: Path,
    cfg_file: Path,
    cfg_line_number: int,
    runner_name: str,
    envelope_operator: str,
    temporal_operator: str,
    definitions: dict[str, tuple[int, str]],
) -> list[str]:
    """Return shape errors for allowlisted temporal envelope side conjuncts."""

    prefix = (
        f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
        f"{cfg_line_number} references correctness envelope {envelope_operator}, "
        f"but allowlisted temporal conjunct {temporal_operator}"
    )
    definition = definitions.get(temporal_operator)
    if definition is None:
        return [
            f"{prefix} has no static single-expression definition in "
            f"{display_path(module_path)}"
        ]
    line, body = definition
    signatures = tla_operator_signatures(module_path)
    temporal_signature = signatures.get(temporal_operator)
    if temporal_signature is not None and temporal_signature[1] != 0:
        return [
            f"{prefix} at {display_path(module_path)}:{temporal_signature[0]} "
            f"defines {temporal_operator} with arity {temporal_signature[1]}; "
            "allowlisted temporal side conjuncts must be zero-arity"
        ]
    body = strip_static_outer_parentheses(body)
    compact_body = " ".join(body.split())
    temporal_literal = tla_static_temporal_boolean_literal(body)
    if temporal_literal is not None:
        return [
            f"{prefix} at {display_path(module_path)}:{line} is literal "
            f"{temporal_literal}; temporal correctness-envelope exceptions must stay "
            "nontrivial"
        ]
    temporal_static_if_literal = tla_static_if_boolean_literal(body)
    if temporal_static_if_literal is not None:
        return [
            f"{prefix} at {display_path(module_path)}:{line} is static IF "
            f"literal {temporal_static_if_literal}; temporal "
            "correctness-envelope exceptions must stay nontrivial"
        ]
    temporal_constant_relation = tla_static_constant_relation(body)
    if temporal_constant_relation is not None:
        return [
            f"{prefix} at {display_path(module_path)}:{line} is constant "
            f"relation {temporal_constant_relation}; temporal "
            "correctness-envelope exceptions must stay nontrivial"
        ]
    self_equality_parts = temporal_self_equality_parts(body)
    if self_equality_parts:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            f"self-equality {', '.join(self_equality_parts)}; temporal "
            "correctness-envelope exceptions must stay nontrivial"
        ]
    self_inequality_parts = temporal_self_inequality_parts(body)
    if self_inequality_parts:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            f"self-inequality {', '.join(self_inequality_parts)}; temporal "
            "correctness-envelope exceptions must stay satisfiable"
        ]
    identifiers = tla_static_non_string_identifiers(body)
    if not identifiers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} has no static "
            "model identifiers; temporal correctness-envelope exceptions must "
            "name concrete model obligations"
        ]
    if TLA_IDENTIFIER_RE.fullmatch(compact_body) and compact_body in identifiers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} aliases "
            f"{compact_body}; temporal correctness-envelope exceptions must "
            "compose concrete temporal obligations directly"
        ]
    whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(compact_body)
    if whole_body_control is not None:
        return [
            f"{prefix} at {display_path(module_path)}:{line} is whole-body "
            f"{whole_body_control.group(1)} expression {compact_body}; name "
            "the concrete temporal predicate before composing it as an "
            "allowlisted temporal side conjunct"
        ]
    unary_temporal_let_aliases = unary_temporal_let_alias_parts(body)
    if unary_temporal_let_aliases:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "unary-temporal LET alias "
            f"{', '.join(unary_temporal_let_aliases)}; name concrete temporal "
            "predicates before composing allowlisted temporal side-conjunct "
            "chains"
        ]
    boolean_composition_kind = tla_whole_body_boolean_composition_kind(body)
    if boolean_composition_kind is not None:
        return [
            f"{prefix} at {display_path(module_path)}:{line} is whole-body "
            f"{boolean_composition_kind} {compact_body}; name the concrete "
            "temporal predicate before composing it as an allowlisted temporal "
            "side conjunct"
        ]
    temporal_helper_boolean_composition = temporal_helper_boolean_composition_parts(
        body,
        definitions,
    )
    if temporal_helper_boolean_composition:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "temporal-helper boolean composition "
            f"{', '.join(temporal_helper_boolean_composition)}; name concrete "
            "temporal predicates before composing allowlisted temporal "
            "side-conjunct chains"
        ]
    hidden_identifiers = sorted(
        identifier
        for identifier in identifiers
        if identifier == "TypeInvariant"
        or identifier in GENERIC_CORRECTNESS_CHECKS
        or identifier.endswith("Exactness")
    )
    if hidden_identifiers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} mentions "
            f"{', '.join(hidden_identifiers)}; keep TypeInvariant, generic "
            "correctness, and *Exactness identifiers out of allowlisted "
            "temporal side conjuncts"
        ]
    parameterized_helper_calls = parameterized_temporal_helper_calls(
        temporal_operator,
        body,
        definitions,
        module_path,
        line,
    )
    if parameterized_helper_calls:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "parameterized temporal helper call "
            f"{', '.join(parameterized_helper_calls)}; lift temporal helper "
            "calls behind zero-arity temporal predicates"
        ]
    nonzero_arity_helpers = nonzero_arity_temporal_helper_references(
        temporal_operator,
        body,
        signatures,
        definitions,
        module_path,
    )
    if nonzero_arity_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "non-zero-arity temporal helper "
            f"{', '.join(nonzero_arity_helpers)}; allowlisted temporal "
            "side-conjunct helper chains must use zero-arity predicates"
        ]
    transitive_undefined_helpers = transitive_undefined_temporal_extra_conjuncts(
        temporal_operator,
        body,
        definitions,
        module_path,
        line,
    )
    if transitive_undefined_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with undefined helper "
            f"{', '.join(transitive_undefined_helpers)}; define named "
            "concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    undefined_quantified_helpers = (
        transitive_undefined_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if undefined_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with undefined quantified "
            f"helper {', '.join(undefined_quantified_helpers)}; define named "
            "concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    static_wrapped_quantified = static_wrapped_quantified_temporal_extra_conjuncts(
        temporal_operator,
        body,
        definitions,
        module_path,
        line,
    )
    if static_wrapped_quantified:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with static-wrapper "
            f"quantified formula {', '.join(static_wrapped_quantified)}; "
            "name quantified temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    structured_quantified = structured_operand_quantified_temporal_extra_conjuncts(
        temporal_operator,
        body,
        definitions,
        module_path,
        line,
    )
    if structured_quantified:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with structured "
            f"quantified formula {', '.join(structured_quantified)}; "
            "name quantified temporal predicates before placing them in "
            "structured helper operands"
        ]
    vacuous_quantified_helpers = (
        transitive_vacuous_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if vacuous_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with vacuous quantified "
            f"helper {', '.join(vacuous_quantified_helpers)}; keep literal "
            "and self-equality, self-inequality, empty-domain, singleton-domain, self-membership, or empty-set membership quantified helper bodies "
            "out of allowlisted temporal side-conjunct chains"
        ]
    duplicate_bound_quantified_helpers = (
        transitive_duplicate_bound_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if duplicate_bound_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with duplicate quantified "
            f"helper binding {', '.join(duplicate_bound_quantified_helpers)}; "
            "bind each quantified identifier once before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    unused_bound_quantified_helpers = (
        transitive_unused_bound_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if unused_bound_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with unused quantified "
            f"helper binding {', '.join(unused_bound_quantified_helpers)}; "
            "use every bound identifier inside quantified temporal predicates "
            "before composing allowlisted temporal side-conjunct chains"
        ]
    control_flow_quantified_helpers = (
        transitive_control_flow_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if control_flow_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with control-flow "
            f"quantified helper {', '.join(control_flow_quantified_helpers)}; "
            "name concrete quantified temporal predicates instead of selecting "
            "predicates inside quantified helper bodies"
        ]
    negated_quantified_helpers = (
        transitive_negated_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if negated_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with negated quantified "
            f"helper {', '.join(negated_quantified_helpers)}; compose positive "
            "quantified temporal predicates before allowlisted temporal "
            "side-conjunct chains"
        ]
    existential_quantified_helpers = (
        transitive_existential_quantified_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if existential_quantified_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with existential "
            f"quantified helper {', '.join(existential_quantified_helpers)}; "
            "use universal quantified temporal predicates before composing "
            "allowlisted temporal side-conjunct chains"
        ]
    transitive_duplicate_helpers = transitive_duplicate_temporal_extra_conjuncts(
        temporal_operator,
        body,
        definitions,
        module_path,
    )
    if transitive_duplicate_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with repeated helper "
            f"conjunct {', '.join(transitive_duplicate_helpers)}; remove "
            "duplicate helper conjuncts so every temporal obligation is "
            "counted once"
        ]
    transitive_contradictory_operands = (
        transitive_contradictory_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if transitive_contradictory_operands:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with contradictory "
            f"helper operand {', '.join(transitive_contradictory_operands)}; "
            "name concrete non-contradictory temporal predicates before "
            "composing allowlisted temporal side-conjunct chains"
        ]
    transitive_excluded_middle_operands = (
        transitive_excluded_middle_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if transitive_excluded_middle_operands:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with excluded-middle "
            f"helper operand {', '.join(transitive_excluded_middle_operands)}; "
            "name concrete non-tautological temporal predicates before "
            "composing allowlisted temporal side-conjunct chains"
        ]
    transitive_complementary_equivalence_operands = (
        transitive_complementary_equivalence_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if transitive_complementary_equivalence_operands:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with "
            "complementary-equivalence helper operand "
            f"{', '.join(transitive_complementary_equivalence_operands)}; "
            "name concrete non-vacuous temporal predicates before composing "
            "allowlisted temporal side-conjunct chains"
        ]
    transitive_duplicate_operands = (
        transitive_duplicate_boolean_operand_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if transitive_duplicate_operands:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with repeated helper "
            f"operand {', '.join(transitive_duplicate_operands)}; remove "
            "duplicate helper operands so every temporal obligation is "
            "counted once"
        ]
    transitive_hidden = transitive_hidden_temporal_extra_conjuncts(
        temporal_operator,
        body,
        definitions,
        module_path,
    )
    if transitive_hidden:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with hidden coverage "
            f"identifiers {', '.join(transitive_hidden)}; keep TypeInvariant, "
            "generic correctness, and *Exactness identifiers out of "
            "allowlisted temporal side-conjunct chains"
        ]
    transitive_control_flow_helpers = (
        transitive_control_flow_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if transitive_control_flow_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with whole-body "
            "control-flow predicate-selection helper "
            f"{', '.join(transitive_control_flow_helpers)}; "
            "name concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    nested_control_flow_helpers = (
        transitive_nested_control_flow_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if nested_control_flow_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with nested control-flow "
            "predicate-selection helper "
            f"{', '.join(nested_control_flow_helpers)}; "
            "name concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    temporal_control_flow_helpers = unary_temporal_control_flow_temporal_helpers(
        temporal_operator,
        body,
        definitions,
        module_path,
    )
    if temporal_control_flow_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with unary-temporal "
            "control-flow predicate-selection helper "
            f"{', '.join(temporal_control_flow_helpers)}; "
            "name concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    structured_control_flow_helpers = (
        structured_operand_control_flow_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
            line,
        )
    )
    if structured_control_flow_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with structured "
            "control-flow predicate-selection helper "
            f"{', '.join(structured_control_flow_helpers)}; "
            "name concrete temporal predicates before placing them in "
            "structured helper operands"
        ]
    transitive_boolean_composition_helpers = (
        transitive_boolean_composition_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if transitive_boolean_composition_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with whole-body "
            "temporal-helper boolean-composition helper "
            f"{', '.join(transitive_boolean_composition_helpers)}; "
            "name concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]
    transitive_vacuous_helpers = transitive_vacuous_temporal_extra_conjuncts(
        temporal_operator,
        body,
        definitions,
        module_path,
    )
    if transitive_vacuous_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with vacuous helper "
            f"{', '.join(transitive_vacuous_helpers)}; keep literal, "
            "self-equality, self-inequality, and alias helpers out of "
            "allowlisted temporal side-conjunct chains"
        ]
    temporal_let_alias_helpers = (
        transitive_unary_temporal_let_alias_temporal_extra_conjuncts(
            temporal_operator,
            body,
            definitions,
            module_path,
        )
    )
    if temporal_let_alias_helpers:
        return [
            f"{prefix} at {display_path(module_path)}:{line} contains "
            "transitive temporal side-conjunct chain with unary-temporal "
            f"LET alias {', '.join(temporal_let_alias_helpers)}; name concrete "
            "temporal predicates before composing allowlisted temporal "
            "side-conjunct chains"
        ]
    if (
        temporal_operator == "EventuallyCommit"
        and not has_direct_post_gst_committed_eventuality(body)
    ):
        return [
            f"{prefix} at {display_path(module_path)}:{line} does not contain "
            "the direct [] (gst => <> committed) liveness shape; "
            "EventuallyCommit must preserve the post-GST commit liveness "
            "obligation"
        ]
    return []


def temporal_helper_references(body: str) -> list[str]:
    """Return direct helper references from temporal side-conjunct bodies."""

    return tla_zero_arity_boolean_references(body)


def temporal_direct_boolean_parts(body: str) -> list[str]:
    """Return direct boolean operand expressions from temporal bodies."""

    parts: list[str] = []
    seen: set[str] = set()

    def collect(current: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        if not normalized or normalized in seen:
            return
        seen.add(normalized)
        parts.append(normalized)

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand)

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand)

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(body)
    return parts


def has_direct_post_gst_committed_eventuality(body: str) -> bool:
    """Return whether a formula has the [] (gst => <> committed) shape."""

    normalized = strip_static_outer_parentheses(" ".join(body.split()))
    operator_operand = tla_unary_temporal_operator_operand(normalized)
    if operator_operand is None:
        return False
    operator, operand = operator_operand
    if operator != "[]":
        return False

    implication_operands = tla_top_level_implication_operands(operand)
    if len(implication_operands) != 2:
        return False
    antecedent = strip_static_outer_parentheses(
        " ".join(implication_operands[0].split())
    )
    if TLA_IDENTIFIER_RE.fullmatch(antecedent) is None or antecedent != "gst":
        return False

    consequent = strip_static_outer_parentheses(
        " ".join(implication_operands[1].split())
    )
    consequent_operator_operand = tla_unary_temporal_operator_operand(consequent)
    if consequent_operator_operand is None:
        return False
    consequent_operator, consequent_operand = consequent_operator_operand
    compact_consequent_operand = strip_static_outer_parentheses(
        " ".join(consequent_operand.split())
    )
    return (
        consequent_operator == "<>"
        and TLA_IDENTIFIER_RE.fullmatch(compact_consequent_operand) is not None
        and compact_consequent_operand == "committed"
    )


def has_direct_commit_never_revoked_shape(body: str) -> bool:
    """Return whether a formula has the [] (committed => [] committed) shape."""

    normalized = strip_static_outer_parentheses(" ".join(body.split()))
    operator_operand = tla_unary_temporal_operator_operand(normalized)
    if operator_operand is None:
        return False
    operator, operand = operator_operand
    if operator != "[]":
        return False

    implication_operands = tla_top_level_implication_operands(operand)
    if len(implication_operands) != 2:
        return False
    antecedent = strip_static_outer_parentheses(
        " ".join(implication_operands[0].split())
    )
    if TLA_IDENTIFIER_RE.fullmatch(antecedent) is None or antecedent != "committed":
        return False

    consequent = strip_static_outer_parentheses(
        " ".join(implication_operands[1].split())
    )
    consequent_operator_operand = tla_unary_temporal_operator_operand(consequent)
    if consequent_operator_operand is None:
        return False
    consequent_operator, consequent_operand = consequent_operator_operand
    compact_consequent_operand = strip_static_outer_parentheses(
        " ".join(consequent_operand.split())
    )
    return (
        consequent_operator == "[]"
        and TLA_IDENTIFIER_RE.fullmatch(compact_consequent_operand) is not None
        and compact_consequent_operand == "committed"
    )


def has_direct_always_predicate_shape(body: str, predicate: str) -> bool:
    """Return whether a formula has the [] Predicate wrapper shape."""

    normalized = strip_static_outer_parentheses(" ".join(body.split()))
    operator_operand = tla_unary_temporal_operator_operand(normalized)
    if operator_operand is None:
        return False
    operator, operand = operator_operand
    compact_operand = strip_static_outer_parentheses(" ".join(operand.split()))
    return (
        operator == "[]"
        and TLA_IDENTIFIER_RE.fullmatch(compact_operand) is not None
        and compact_operand == predicate
    )


def tla_action_box_parts(body: str) -> tuple[str, str] | None:
    """Return the action and subscript from an [Action]_subscript wrapper."""

    compact_body = strip_static_outer_parentheses(" ".join(body.split()))
    match = TLA_ACTION_BOX_RE.fullmatch(compact_body)
    if match is None:
        return None
    return match.group(1), match.group(2)


def has_direct_always_action_wrapper_shape(body: str, action: str) -> bool:
    """Return whether a formula has the [] [Action]_vars wrapper shape."""

    normalized = strip_static_outer_parentheses(" ".join(body.split()))
    operator_operand = tla_unary_temporal_operator_operand(normalized)
    if operator_operand is None:
        return False
    operator, operand = operator_operand
    action_box = tla_action_box_parts(operand)
    if action_box is None:
        return False
    wrapped_action, subscript = action_box
    return operator == "[]" and wrapped_action == action and subscript == "vars"


def is_committed_phase_equality(body: str) -> bool:
    """Return whether a formula is exactly phase = "Committed"."""

    relation = tla_top_level_equality_relation_parts(body)
    if relation is None:
        return False
    left, operator, right = relation
    compact_left = strip_static_outer_parentheses(" ".join(left.split()))
    compact_right = strip_static_outer_parentheses(" ".join(right.split()))
    return compact_left == "phase" and operator == "=" and compact_right == '"Committed"'


def has_direct_committed_phase_never_leaves_shape(body: str) -> bool:
    """Return whether a formula has the committed-phase permanence shape."""

    normalized = strip_static_outer_parentheses(" ".join(body.split()))
    operator_operand = tla_unary_temporal_operator_operand(normalized)
    if operator_operand is None:
        return False
    operator, operand = operator_operand
    if operator != "[]":
        return False

    implication_operands = tla_top_level_implication_operands(operand)
    if len(implication_operands) != 2:
        return False
    if not is_committed_phase_equality(implication_operands[0]):
        return False

    consequent = strip_static_outer_parentheses(
        " ".join(implication_operands[1].split())
    )
    consequent_operator_operand = tla_unary_temporal_operator_operand(consequent)
    if consequent_operator_operand is None:
        return False
    consequent_operator, consequent_operand = consequent_operator_operand
    return consequent_operator == "[]" and is_committed_phase_equality(
        consequent_operand
    )


def sumeragi_temporal_shape_contract_errors(
    module_path: Path = SPEC_DIR / "Sumeragi.tla",
) -> list[str]:
    """Return errors for protected temporal theorem shape drift."""

    if not module_path.exists():
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    errors: list[str] = []

    commit_never_revoked = "CommitNeverRevoked"
    definition = definitions.get(commit_never_revoked)
    if definition is None:
        errors.append(
            f"{display_path(module_path)} does not define temporal theorem "
            f"{commit_never_revoked}"
        )
    else:
        signature = signatures.get(commit_never_revoked)
        if signature is not None and signature[1] != 0:
            line, arity = signature
            errors.append(
                f"{display_path(module_path)}:{line} defines temporal theorem "
                f"{commit_never_revoked} with arity {arity}; temporal shape "
                "contracts require zero-arity operators"
            )
        else:
            line, body = definition
            if not has_direct_commit_never_revoked_shape(body):
                errors.append(
                    f"{display_path(module_path)}:{line} defines "
                    f"{commit_never_revoked}, but it does not keep the direct "
                    "[] (committed => [] committed) finality-latch monotonicity "
                    "shape with exact lowercase state-variable names"
                )

    committed_phase_never_leaves = "CommittedPhaseNeverLeaves"
    definition = definitions.get(committed_phase_never_leaves)
    if definition is None:
        errors.append(
            f"{display_path(module_path)} does not define temporal theorem "
            f"{committed_phase_never_leaves}"
        )
    else:
        signature = signatures.get(committed_phase_never_leaves)
        if signature is not None and signature[1] != 0:
            line, arity = signature
            errors.append(
                f"{display_path(module_path)}:{line} defines temporal theorem "
                f"{committed_phase_never_leaves} with arity {arity}; temporal "
                "shape contracts require zero-arity operators"
            )
        else:
            line, body = definition
            if not has_direct_committed_phase_never_leaves_shape(body):
                errors.append(
                    f"{display_path(module_path)}:{line} defines "
                    f"{committed_phase_never_leaves}, but it does not keep the "
                    'direct [] (phase = "Committed" => [] (phase = "Committed")) '
                    "phase permanence shape"
                )

    for operator, action in SUMERAGI_TEMPORAL_ACTION_THEOREM_CONTRACTS.items():
        definition = definitions.get(operator)
        if definition is None:
            errors.append(
                f"{display_path(module_path)} does not define temporal theorem "
                f"{operator}"
            )
            continue

        signature = signatures.get(operator)
        if signature is not None and signature[1] != 0:
            line, arity = signature
            errors.append(
                f"{display_path(module_path)}:{line} defines temporal theorem "
                f"{operator} with arity {arity}; temporal shape contracts "
                "require zero-arity operators"
            )
            continue

        action_signature = signatures.get(action)
        if action_signature is None:
            errors.append(
                f"{display_path(module_path)} defines temporal theorem "
                f"{operator}, but matching action step {action} is not defined"
            )
        elif action_signature[1] != 0:
            line, arity = action_signature
            errors.append(
                f"{display_path(module_path)}:{line} defines matching action "
                f"step {action} for temporal theorem {operator} with arity "
                f"{arity}; temporal shape contracts require zero-arity action "
                "steps"
            )

        line, body = definition
        if not has_direct_always_action_wrapper_shape(body, action):
            errors.append(
                f"{display_path(module_path)}:{line} defines {operator}, but "
                f"it does not keep the direct [] [{action}]_vars "
                "action-wrapper shape"
            )

    for operator, predicate in SUMERAGI_TEMPORAL_ALWAYS_THEOREM_CONTRACTS.items():
        definition = definitions.get(operator)
        if definition is None:
            errors.append(
                f"{display_path(module_path)} does not define temporal theorem "
                f"{operator}"
            )
            continue

        signature = signatures.get(operator)
        if signature is not None and signature[1] != 0:
            line, arity = signature
            errors.append(
                f"{display_path(module_path)}:{line} defines temporal theorem "
                f"{operator} with arity {arity}; temporal shape contracts "
                "require zero-arity operators"
            )
            continue

        predicate_signature = signatures.get(predicate)
        if predicate_signature is None:
            errors.append(
                f"{display_path(module_path)} defines temporal theorem "
                f"{operator}, but matching predicate {predicate} is not defined"
            )
        elif predicate_signature[1] != 0:
            line, arity = predicate_signature
            errors.append(
                f"{display_path(module_path)}:{line} defines matching predicate "
                f"{predicate} for temporal theorem {operator} with arity "
                f"{arity}; temporal shape contracts require zero-arity "
                "predicates"
            )

        line, body = definition
        if not has_direct_always_predicate_shape(body, predicate):
            errors.append(
                f"{display_path(module_path)}:{line} defines {operator}, but "
                f"it does not keep the direct [] {predicate} wrapper shape"
            )
    return errors


def temporal_self_equality_parts(body: str) -> list[str]:
    """Return self-equality operands in a temporal side-conjunct body."""

    self_equalities: list[str] = []
    seen: set[str] = set()
    for part in temporal_direct_boolean_parts(body):
        self_equality = tla_static_self_equality(part)
        if self_equality is None or self_equality in seen:
            continue
        seen.add(self_equality)
        self_equalities.append(self_equality)
    return self_equalities


def temporal_self_inequality_parts(body: str) -> list[str]:
    """Return self-inequality operands in a temporal side-conjunct body."""

    self_inequalities: list[str] = []
    seen: set[str] = set()
    for part in temporal_direct_boolean_parts(body):
        self_inequality = tla_static_self_inequality(part)
        if self_inequality is None or self_inequality in seen:
            continue
        seen.add(self_inequality)
        self_inequalities.append(self_inequality)
    return self_inequalities


def unary_temporal_let_alias_parts(body: str) -> list[str]:
    """Return unary-temporal operands hiding transparent LET aliases."""

    aliases: list[str] = []
    seen_aliases: set[str] = set()
    seen_bodies: set[str] = set()

    def record(normalized: str, let_operand: str) -> None:
        message = f"{normalized} aliases {let_operand}"
        if message in seen_aliases:
            return
        seen_aliases.add(message)
        aliases.append(message)

    def collect(current: str) -> None:
        normalized = strip_static_outer_parentheses(" ".join(current.split()))
        if not normalized or normalized in seen_bodies:
            return
        seen_bodies.add(normalized)

        operator_operand = tla_unary_temporal_operator_operand(normalized)
        if operator_operand is not None:
            _, operand = operator_operand
            let_operand = tla_static_let_alias_operand(operand)
            if let_operand is not None:
                record(normalized, let_operand)
            collect(operand)
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            collect(part)

    collect(body)
    return aliases


def temporal_helper_boolean_composition_parts(
    body: str,
    definitions: dict[str, tuple[int, str]],
    *,
    include_nested_negation: bool = True,
) -> list[str]:
    """Return direct boolean composition over temporal helper predicates."""

    compositions: list[str] = []
    seen: set[str] = set()
    compact_body = " ".join(strip_static_outer_parentheses(body).split())
    for part in temporal_direct_boolean_parts(body):
        compact_part = " ".join(strip_static_outer_parentheses(part).split())
        if compact_part in seen:
            continue
        seen.add(compact_part)
        literal_gated_negated_operand = (
            literal_gated_negated_zero_arity_helper_operand(
                compact_part,
                definitions,
            )
        )
        if (
            literal_gated_negated_operand is not None
            and helper_definition_is_temporal(
                literal_gated_negated_operand,
                definitions,
                set(),
            )
        ):
            compositions.append(f"negation {compact_part}")
            continue
        kind = tla_whole_body_boolean_composition_kind(compact_part)
        if kind is None:
            continue
        if (
            kind == "negation"
            and not include_nested_negation
            and compact_part != compact_body
        ):
            continue
        if not boolean_composition_references_temporal_helper(
            compact_part,
            definitions,
        ):
            continue
        compositions.append(f"{kind} {compact_part}")
    return compositions


def tla_unary_temporal_operand(expression: str) -> str | None:
    """Return the operand of a static unary temporal formula, if present."""

    operator_operand = tla_unary_temporal_operator_operand(expression)
    if operator_operand is None:
        return None
    return operator_operand[1]


def tla_unary_action_operand(expression: str) -> str | None:
    """Return the operand of an ENABLED/UNCHANGED action wrapper, if present."""

    operator_operand = tla_unary_action_operator_operand(expression)
    if operator_operand is None:
        return None
    return operator_operand[1]


def tla_unary_action_operator_operand(expression: str) -> tuple[str, str] | None:
    """Return the operator and operand of an ENABLED/UNCHANGED wrapper."""

    stripped = strip_static_outer_parentheses(expression).strip()
    match = re.match(r"^(ENABLED|UNCHANGED)\b", stripped)
    if match is None:
        return None
    operand = stripped[match.end() :].strip()
    if not operand:
        return None
    return match.group(1), strip_static_outer_parentheses(operand)


def tla_unary_set_operator_operand(expression: str) -> str | None:
    """Return the operand of a static unary set operator, if present."""

    operator_operand = tla_unary_set_operator_expression_operand(expression)
    if operator_operand is None:
        return None
    return operator_operand[1]


def tla_unary_set_operator_expression_operand(
    expression: str,
) -> tuple[str, str] | None:
    """Return the operator and operand of a static unary set operator."""

    stripped = strip_static_outer_parentheses(expression).strip()
    for operator in sorted(TLA_UNARY_SET_OPERATOR_IDENTIFIERS):
        match = re.match(rf"^{operator}\b", stripped)
        if match is None:
            continue
        operand = stripped[match.end() :].strip()
        if not operand:
            return None
        return operator, strip_static_outer_parentheses(operand)
    return None


def tla_unary_temporal_operator_operand(expression: str) -> tuple[str, str] | None:
    """Return the operator and operand of a static unary temporal formula."""

    stripped = strip_static_outer_parentheses(expression).strip()
    for operator in ("[]", "<>"):
        if not stripped.startswith(operator):
            continue
        operand = stripped[len(operator) :].strip()
        if not operand:
            return None
        return operator, strip_static_outer_parentheses(operand)
    return None


def tla_static_negation_operand(expression: str) -> str | None:
    """Return the operand of a static top-level negation, if present."""

    stripped = strip_static_outer_parentheses(expression).strip()
    if not stripped.startswith("~"):
        return None
    operand = stripped[1:].strip()
    if not operand:
        return None
    return strip_static_outer_parentheses(operand)


def parameterized_temporal_helper_calls(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return direct parameterized helper calls in temporal helper chains."""

    calls: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, call: str) -> None:
        message = (
            f"{root} reaches {call} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        calls.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for conjunct in temporal_direct_boolean_parts(body):
            compact_conjunct = " ".join(
                strip_static_outer_parentheses(conjunct).split()
            )
            if tla_direct_operator_call_name(compact_conjunct) is not None:
                record(root, chain, line, compact_conjunct)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return calls


def nonzero_arity_temporal_helper_references(
    temporal_operator: str,
    temporal_body: str,
    signatures: dict[str, tuple[int, int]],
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return non-zero-arity helpers reached by temporal side-conjunct chains."""

    nonzero_arity: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, arity: int) -> None:
        message = (
            f"{root} reaches {' -> '.join(chain)} at "
            f"{display_path(module_path)}:{line} with arity {arity}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        nonzero_arity.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        _, body = definition
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            signature = signatures.get(reference)
            if signature is not None and signature[1] != 0:
                record(root, chain + [reference], signature[0], signature[1])
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        signature = signatures.get(helper)
        if signature is not None and signature[1] != 0:
            record(helper, [helper], signature[0], signature[1])
            continue
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return nonzero_arity


def transitive_undefined_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return undefined helpers reached by temporal side-conjunct chains."""

    undefined: list[str] = []
    seen_messages: set[str] = set()
    declared_names = {
        *tla_constant_declarations(module_path),
        *(variable for _, variable in tla_variable_declaration_entries(module_path)),
    }
    parameter_names = tla_operator_parameter_names(module_path)

    def record(root: str, chain: list[str], line: int, reference: str) -> None:
        message = (
            f"{root} reaches {reference} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        undefined.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        local_bound = parameter_names.get(chain[-1], frozenset())
        for reference in temporal_undefined_helper_references(body):
            if reference in local_bound:
                continue
            if reference == temporal_operator:
                continue
            if reference in declared_names:
                continue
            if reference not in definitions and is_tla_helper_identifier(reference):
                record(root, chain, line, reference)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference in parameter_names.get(current, frozenset()):
                continue
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return undefined


def transitive_undefined_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return undefined helpers inside quantified temporal helper formulas."""

    undefined: list[str] = []
    seen_messages: set[str] = set()
    parameter_names = tla_operator_parameter_names(module_path)

    def record(root: str, chain: list[str], line: int, reference: str) -> None:
        message = (
            f"{root} reaches {reference} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        undefined.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        local_bound = parameter_names.get(chain[-1], frozenset())
        for formula in quantified_helper_formulas(body):
            for reference in undefined_static_helper_identifiers(
                formula,
                definitions,
                module_path,
                current=chain[-1],
                exactness_operator=temporal_operator,
                local_bound=local_bound,
            ):
                record(root, chain, line, reference)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference in parameter_names.get(current, frozenset()):
                continue
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return undefined


def static_wrapped_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return quantified formulas below static wrappers in temporal chains."""

    quantified: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        quantified.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in static_wrapped_quantified_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return quantified


def structured_operand_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return quantified formulas below structured operands in temporal chains."""

    quantified: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        quantified.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in structured_operand_quantified_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return quantified


def structured_operand_control_flow_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return control-flow formulas below structured temporal operands."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in structured_operand_control_flow_formulas(body, definitions):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return control_flow


def transitive_vacuous_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return vacuous quantified helpers below temporal side conjuncts."""

    vacuous: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        vacuous.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in vacuous_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return vacuous


def transitive_duplicate_bound_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return quantified temporal helpers with duplicate bindings."""

    duplicated: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        duplicated.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in duplicate_bound_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return duplicated


def transitive_unused_bound_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return quantified temporal helpers with unused bindings."""

    unused: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        unused.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in unused_bound_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return unused


def transitive_control_flow_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return quantified temporal helpers with control-flow-selected predicates."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in control_flow_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return control_flow


def transitive_negated_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return negated quantified temporal helpers below side conjuncts."""

    negated: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        negated.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in negated_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return negated


def transitive_existential_quantified_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
    temporal_line: int,
) -> list[str]:
    """Return existential quantified helpers below temporal side conjuncts."""

    existential: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, formula: str) -> None:
        message = (
            f"{root} reaches {formula} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        existential.append(message)

    def inspect_body(root: str, chain: list[str], line: int, body: str) -> None:
        for formula in existential_quantified_helper_formulas(body):
            record(root, chain, line, formula)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        inspect_body(root, chain, line, body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    inspect_body(temporal_operator, [temporal_operator], temporal_line, temporal_body)
    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return existential


def temporal_undefined_helper_references(body: str) -> list[str]:
    """Return missing-helper candidates from direct temporal helper positions."""

    references: set[str] = set()
    seen: set[str] = set()

    def is_compound_helper_identifier(identifier: str) -> bool:
        return (
            is_tla_helper_identifier(identifier)
            and (
                identifier.startswith("Temporal")
                or "Helper" in identifier
                or "Predicate" in identifier
                or identifier.endswith("Safety")
                or identifier.endswith("Envelope")
            )
        )

    def record_if_helper(expression: str, *, compound_operand: bool = False) -> bool:
        normalized = strip_static_outer_parentheses(" ".join(expression.split()))
        if (
            TLA_IDENTIFIER_RE.fullmatch(normalized)
            and is_tla_user_identifier(normalized)
            and (
                is_compound_helper_identifier(normalized)
                if compound_operand
                else is_tla_helper_identifier(normalized)
            )
        ):
            references.add(normalized)
            return True
        return False

    def collect(expression: str, *, compound_operand: bool = False) -> None:
        normalized = strip_static_outer_parentheses(" ".join(expression.split()))
        if not normalized or normalized in seen:
            return
        seen.add(normalized)
        if record_if_helper(normalized, compound_operand=compound_operand):
            return

        let_operand = tla_static_let_alias_operand(normalized)
        if let_operand is not None:
            collect(let_operand, compound_operand=compound_operand)
            return

        operand = tla_unary_temporal_operand(normalized)
        if operand is not None:
            collect(operand, compound_operand=compound_operand)
            return

        negated_operand = tla_static_negation_operand(normalized)
        if negated_operand is not None:
            collect(negated_operand, compound_operand=compound_operand)
            return

        conjuncts = tla_top_level_conjuncts(normalized)
        if len(conjuncts) > 1:
            for conjunct in conjuncts:
                collect(conjunct, compound_operand=compound_operand)
            return

        for part in tla_top_level_boolean_parts(normalized):
            compact_part = strip_static_outer_parentheses(" ".join(part.split()))
            if compact_part == normalized:
                continue
            if record_if_helper(compact_part, compound_operand=True):
                continue
            collect(part, compound_operand=True)

    collect(body)
    return sorted(references)


def transitive_duplicate_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return repeated named helper conjuncts below temporal side conjuncts."""

    duplicates: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, repeated: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} repeats {repeated}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        duplicates.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for repeated in (
            duplicate_zero_arity_conjunct_references(body)
            + duplicate_zero_arity_wrapped_conjunct_references(body)
        ):
            if repeated in definitions:
                record(root, current, chain, line, repeated)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return duplicates


def transitive_duplicate_boolean_operand_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return repeated named boolean operands below temporal side conjuncts."""

    duplicates: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, repeated: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} repeats {repeated}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        duplicates.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for repeated in duplicate_zero_arity_boolean_operand_references(body):
            if repeated in definitions:
                record(root, current, chain, line, repeated)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return duplicates


def transitive_contradictory_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return contradictory operands below temporal side conjuncts."""

    contradictory: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operand: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} pairs {operand} with ~{operand}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        contradictory.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operand in contradictory_zero_arity_conjunct_references(body):
            if operand in definitions:
                record(root, current, chain, line, operand)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return contradictory


def transitive_excluded_middle_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return excluded-middle operands below temporal side conjuncts."""

    excluded: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operand: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} pairs {operand} with ~{operand}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        excluded.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operand in excluded_middle_zero_arity_disjunct_references(body):
            if operand in definitions:
                record(root, current, chain, line, operand)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return excluded


def transitive_complementary_equivalence_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return complementary-equivalence operands below temporal side conjuncts."""

    complementary: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operand: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} pairs {operand} with "
            f"~{operand} under equivalence"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        complementary.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operand in complementary_equivalence_zero_arity_references(body):
            if operand in definitions:
                record(root, current, chain, line, operand)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return complementary


def transitive_hidden_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return hidden coverage identifiers below allowlisted temporal helpers."""

    hidden: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, identifier: str) -> None:
        message = (
            f"{root} reaches {identifier} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        hidden.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            is_hidden_coverage = (
                reference == "TypeInvariant"
                or reference in GENERIC_CORRECTNESS_CHECKS
                or reference.endswith("Exactness")
            )
            if is_hidden_coverage:
                record(root, chain, line, reference)
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        is_hidden_coverage = (
            helper == "TypeInvariant"
            or helper in GENERIC_CORRECTNESS_CHECKS
            or helper.endswith("Exactness")
        )
        if is_hidden_coverage:
            continue
        walk(helper, helper, [helper], set())
    return hidden


def transitive_control_flow_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return whole-body control-flow helpers below allowlisted temporal helpers."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operator: str, body: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is whole-body {operator} "
            f"expression {body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        whole_body_control = TLA_WHOLE_BODY_CONTROL_RE.match(compact_body)
        if (
            whole_body_control is not None
            and tla_control_flow_helper_selects_predicate(compact_body)
        ):
            record(
                root,
                current,
                chain,
                line,
                whole_body_control.group(1),
                compact_body,
            )
        for reference in temporal_helper_references(stripped_body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return control_flow


def transitive_nested_control_flow_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return nested control-flow helpers below allowlisted temporal helpers."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str, current: str, chain: list[str], line: int, operator: str, body: str
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} contains nested {operator} "
            f"expression {body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operator, control_body in nested_control_flow_helper_formulas(
            body,
            definitions,
        ):
            record(root, current, chain, line, operator, control_body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return control_flow


def unary_temporal_control_flow_temporal_helpers(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return control-flow formulas below unary-temporal temporal helpers."""

    control_flow: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        operator: str,
        body: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is unary-temporal "
            f"{operator} expression {body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        control_flow.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        for operator, control_body in unary_temporal_control_flow_formulas(body):
            record(root, current, chain, line, operator, control_body)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return control_flow


def transitive_unary_temporal_let_alias_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return unary-temporal LET aliases below temporal helper chains."""

    aliases: list[str] = []
    seen_messages: set[str] = set()

    def record(root: str, chain: list[str], line: int, body_aliases: list[str]) -> None:
        message = (
            f"{root} reaches {chain[-1]} through {' -> '.join(chain)} at "
            f"{display_path(module_path)}:{line} contains "
            f"{', '.join(body_aliases)}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        aliases.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        body_aliases = unary_temporal_let_alias_parts(body)
        if body_aliases:
            record(root, chain, line, body_aliases)
        for reference in temporal_helper_references(body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return aliases


def transitive_boolean_composition_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return whole-body boolean-composition over temporal helpers."""

    boolean_composition: list[str] = []
    seen_messages: set[str] = set()

    def record(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        kind: str,
        body: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} is whole-body {kind} "
            f"{body}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        boolean_composition.append(message)

    def record_nested(
        root: str,
        current: str,
        chain: list[str],
        line: int,
        composition: str,
    ) -> None:
        message = (
            f"{root} reaches {current} through {' -> '.join(chain)} "
            f"at {display_path(module_path)}:{line} contains "
            f"temporal-helper boolean composition {composition}"
        )
        if message in seen_messages:
            return
        seen_messages.add(message)
        boolean_composition.append(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        kind = tla_whole_body_boolean_composition_kind(stripped_body)
        if kind is not None and boolean_composition_references_temporal_helper(
            stripped_body,
            definitions,
        ):
            record(root, current, chain, line, kind, compact_body)
        elif kind is None:
            for composition in temporal_helper_boolean_composition_parts(
                stripped_body,
                definitions,
                include_nested_negation=False,
            ):
                record_nested(root, current, chain, line, composition)
        for reference in temporal_helper_references(stripped_body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return boolean_composition


def boolean_composition_references_temporal_helper(
    body: str,
    definitions: dict[str, tuple[int, str]],
) -> bool:
    """Return whether a boolean-composition body references temporal helpers."""

    return any(
        helper_definition_is_temporal(reference, definitions, set())
        for reference in temporal_helper_references(body)
    )


def helper_definition_is_temporal(
    helper: str,
    definitions: dict[str, tuple[int, str]],
    seen: set[str],
) -> bool:
    """Return whether a helper definition contains unary temporal structure."""

    if helper in seen:
        return False
    seen.add(helper)
    definition = definitions.get(helper)
    if definition is None:
        return False
    _, body = definition
    stripped_body = strip_static_outer_parentheses(body)
    if tla_unary_temporal_operand(stripped_body) is not None:
        return True
    return any(
        helper_definition_is_temporal(reference, definitions, seen.copy())
        for reference in temporal_helper_references(stripped_body)
        if reference != helper
    )


def transitive_vacuous_temporal_extra_conjuncts(
    temporal_operator: str,
    temporal_body: str,
    definitions: dict[str, tuple[int, str]],
    module_path: Path,
) -> list[str]:
    """Return literal or alias helpers below allowlisted temporal helpers."""

    vacuous: list[str] = []
    seen_messages: set[str] = set()

    def record(message: str) -> None:
        if message in seen_messages:
            return
        seen_messages.add(message)
        vacuous.append(message)

    def inspect_hidden_references(
        root: str,
        chain: list[str],
        body: str,
    ) -> None:
        for reference in hidden_static_structured_helper_references(body):
            if reference == chain[-1] or reference == temporal_operator:
                continue
            definition = definitions.get(reference)
            if definition is None:
                continue
            reference_line, reference_body = definition
            hidden_chain = chain + [reference]
            for message in vacuous_helper_leaf_messages(
                root,
                reference,
                hidden_chain,
                reference_line,
                reference_body,
                definitions,
                module_path,
                exactness=False,
            ):
                record(message)

    def walk(root: str, current: str, chain: list[str], seen: set[str]) -> None:
        if current in seen:
            return
        seen.add(current)
        definition = definitions.get(current)
        if definition is None:
            return
        line, body = definition
        stripped_body = strip_static_outer_parentheses(body)
        compact_body = " ".join(stripped_body.split())
        inspect_hidden_references(root, chain, stripped_body)
        literal_body = tla_static_temporal_boolean_literal(stripped_body)
        if literal_body is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is literal "
                f"{literal_body}"
            )
        static_if_literal = tla_static_if_boolean_literal(stripped_body)
        if static_if_literal is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is static IF literal "
                f"{static_if_literal}"
            )
        constant_relation = tla_static_constant_relation(stripped_body)
        if constant_relation is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is constant relation "
                f"{constant_relation}"
            )
        self_equality_body = tla_static_self_equality(stripped_body)
        if self_equality_body is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is self-equality "
                f"{self_equality_body}"
            )
        else:
            self_equality_parts = temporal_self_equality_parts(stripped_body)
            if self_equality_parts:
                record(
                    f"{root} reaches {current} through {' -> '.join(chain)} "
                    f"at {display_path(module_path)}:{line} contains "
                    f"self-equality {', '.join(self_equality_parts)}"
                )
        self_inequality_body = tla_static_self_inequality(stripped_body)
        if self_inequality_body is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} is self-inequality "
                f"{self_inequality_body}"
            )
        else:
            self_inequality_parts = temporal_self_inequality_parts(stripped_body)
            if self_inequality_parts:
                record(
                    f"{root} reaches {current} through {' -> '.join(chain)} "
                    f"at {display_path(module_path)}:{line} contains "
                    f"self-inequality {', '.join(self_inequality_parts)}"
                )
        body_identifiers = tla_static_non_string_identifiers(compact_body)
        if (
            len(body_identifiers) == 1
            and compact_body in body_identifiers
        ):
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} aliases "
                f"{compact_body}"
            )
        literal_gated_alias = literal_gated_zero_arity_helper_alias(
            stripped_body,
            definitions,
        )
        if literal_gated_alias is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} aliases "
                f"{literal_gated_alias} through a literal-gated helper operand"
            )
        single_conjunct_alias = single_zero_arity_conjunct_alias(
            stripped_body,
            definitions,
        )
        if single_conjunct_alias is not None:
            record(
                f"{root} reaches {current} through {' -> '.join(chain)} "
                f"at {display_path(module_path)}:{line} aliases "
                f"{single_conjunct_alias} through a single helper conjunct"
            )
        for reference in temporal_helper_references(stripped_body):
            if reference == current or reference == temporal_operator:
                continue
            if reference not in definitions:
                continue
            walk(root, reference, chain + [reference], seen.copy())

    for helper in temporal_helper_references(temporal_body):
        if helper not in definitions:
            continue
        walk(helper, helper, [helper], set())
    return vacuous


def cfg_correctness_envelope_shape_errors(
    mode: str,
    module_path: Path,
    cfg_file: Path,
    runner_name: str,
) -> list[str]:
    if not cfg_file.name.endswith("_fast.cfg") or "_bug_" in cfg_file.name:
        return []

    references, parse_errors = cfg_operator_references(cfg_file)
    if parse_errors:
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    signatures = tla_operator_signatures(module_path)
    errors: list[str] = []
    legacy_without_exactness = False
    for line_number, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        if not operator.endswith("CorrectnessEnvelope"):
            continue
        definition = definitions.get(operator)
        if definition is None:
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                f"{line_number} references correctness envelope {operator}, "
                f"but {display_path(module_path)} has no static "
                "single-expression definition for it"
            )
            continue
        definition_line, body = definition
        identifiers = tla_static_non_string_identifiers(body)
        envelope_conjunct_references = set(tla_zero_arity_conjunct_references(body))
        duplicate_conjuncts = duplicate_zero_arity_conjunct_references(body)
        if duplicate_conjuncts:
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                f"{line_number} references correctness envelope {operator}, "
                f"but {display_path(module_path)}:{definition_line} repeats "
                f"correctness-envelope conjunct {', '.join(duplicate_conjuncts)}; "
                "remove duplicate conjuncts so every obligation is counted once"
            )
        for conjunct in tla_top_level_conjuncts(body):
            compact_conjunct = " ".join(
                strip_static_outer_parentheses(conjunct).split()
            )
            if (
                TLA_IDENTIFIER_RE.fullmatch(compact_conjunct)
                and is_tla_user_identifier(compact_conjunct)
            ):
                continue
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                f"{line_number} references correctness envelope {operator}, "
                f"but {display_path(module_path)}:{definition_line} contains "
                "direct non-named correctness-envelope conjunct "
                f"{compact_conjunct}; compose named zero-arity envelope "
                "predicates directly"
            )
        nonzero_arity_conjuncts = nonzero_arity_conjunct_references(
            body,
            signatures,
        )
        if nonzero_arity_conjuncts:
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                f"{line_number} references correctness envelope {operator}, "
                f"but {display_path(module_path)}:{definition_line} contains "
                "non-zero-arity correctness-envelope conjunct "
                f"{format_nonzero_arity_references(nonzero_arity_conjuncts, module_path)}; "
                "correctness envelopes must compose zero-arity predicates "
                "directly"
            )
        if "TypeInvariant" not in identifiers:
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                f"{line_number} references correctness envelope {operator}, "
                f"but {display_path(module_path)}:{definition_line} does not "
                "compose TypeInvariant"
            )
        elif "TypeInvariant" not in envelope_conjunct_references:
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                f"{line_number} references correctness envelope {operator}, "
                f"but {display_path(module_path)}:{definition_line} mentions "
                "TypeInvariant outside a top-level conjunct; compose "
                "TypeInvariant as a direct /\\ conjunct"
            )
        exactness_identifiers = sorted(
            identifier for identifier in identifiers if identifier.endswith("Exactness")
        )
        if exactness_identifiers:
            nested_exactness_identifiers = sorted(
                identifier
                for identifier in exactness_identifiers
                if identifier not in envelope_conjunct_references
            )
            if nested_exactness_identifiers:
                errors.append(
                    f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                    f"{line_number} references correctness envelope {operator}, "
                    f"but {display_path(module_path)}:{definition_line} mentions "
                    f"exactness {', '.join(nested_exactness_identifiers)} outside "
                    "top-level conjuncts; compose *Exactness operators as direct "
                    "/\\ conjuncts"
                )
            generic_identifiers = sorted(
                identifier
                for identifier in identifiers
                if identifier in GENERIC_CORRECTNESS_CHECKS
            )
            if generic_identifiers:
                errors.append(
                    f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                    f"{line_number} references correctness envelope {operator}, "
                    f"but {display_path(module_path)}:{definition_line} mentions "
                    f"generic {', '.join(generic_identifiers)}; compose concrete "
                    "exactness predicates directly"
                )
            extra_identifiers = sorted(
                identifier
                for identifier in identifiers
                if identifier != "TypeInvariant"
                and not identifier.endswith("Exactness")
                and identifier not in GENERIC_CORRECTNESS_CHECKS
            )
            allowed_extra_identifiers = TEMPORAL_CORRECTNESS_ENVELOPE_EXTRAS.get(
                (module_path.name, operator), set()
            )
            unexpected_extra_identifiers = sorted(
                identifier
                for identifier in extra_identifiers
                if identifier not in allowed_extra_identifiers
            )
            if unexpected_extra_identifiers:
                errors.append(
                    f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                    f"{line_number} references correctness envelope {operator}, "
                    f"but {display_path(module_path)}:{definition_line} mentions "
                    f"non-exactness {', '.join(unexpected_extra_identifiers)}; compose "
                    "semantic obligations through *Exactness operators"
                )
            missing_allowed_extra_identifiers = sorted(
                identifier
                for identifier in allowed_extra_identifiers
                if identifier not in extra_identifiers
            )
            if missing_allowed_extra_identifiers:
                errors.append(
                    f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                    f"{line_number} references correctness envelope {operator}, "
                    f"but its temporal allowlist is stale for "
                    f"{', '.join(missing_allowed_extra_identifiers)}"
                )
            nested_allowed_extra_identifiers = sorted(
                identifier
                for identifier in allowed_extra_identifiers
                if identifier in extra_identifiers
                and identifier not in envelope_conjunct_references
            )
            if nested_allowed_extra_identifiers:
                errors.append(
                    f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
                    f"{line_number} references correctness envelope {operator}, "
                    f"but {display_path(module_path)}:{definition_line} mentions "
                    f"allowlisted temporal {', '.join(nested_allowed_extra_identifiers)} "
                    "outside top-level conjuncts; keep temporal exceptions as "
                    "direct /\\ conjuncts"
                )
            for allowed_extra_identifier in sorted(
                identifier
                for identifier in allowed_extra_identifiers
                if identifier in extra_identifiers
            ):
                errors.extend(
                    temporal_extra_definition_shape_errors(
                        mode,
                        module_path,
                        cfg_file,
                        line_number,
                        runner_name,
                        operator,
                        allowed_extra_identifier,
                        definitions,
                    )
                )
            for exactness_operator in exactness_identifiers:
                errors.extend(
                    exactness_definition_shape_errors(
                        mode,
                        module_path,
                        cfg_file,
                        line_number,
                        runner_name,
                        exactness_operator,
                        definitions,
                        (
                            f"references correctness envelope {operator}, but "
                            "exactness conjunct"
                        ),
                    )
                )
            continue
        if mode in LEGACY_FAST_ENVELOPE_WITHOUT_EXACTNESS:
            legacy_without_exactness = True
            continue
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_file)}:{line_number} "
            f"references correctness envelope {operator}, but "
            f"{display_path(module_path)}:{definition_line} has no "
            "model-specific *Exactness conjunct"
        )

    if mode in LEGACY_FAST_ENVELOPE_WITHOUT_EXACTNESS and not legacy_without_exactness:
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_file)} is listed in "
            "LEGACY_FAST_ENVELOPE_WITHOUT_EXACTNESS, but its correctness "
            "envelope already includes a model-specific *Exactness conjunct"
        )
    return errors


def cfg_direct_exactness_shape_errors(
    mode: str,
    module_path: Path,
    cfg_file: Path,
    runner_name: str,
) -> list[str]:
    if not cfg_file.name.endswith("_fast.cfg") or "_bug_" in cfg_file.name:
        return []

    references, parse_errors = cfg_operator_references(cfg_file)
    if parse_errors:
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    errors: list[str] = []
    for line_number, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        if not operator.endswith("Exactness"):
            continue
        errors.extend(
            exactness_definition_shape_errors(
                mode,
                module_path,
                cfg_file,
                line_number,
                runner_name,
                operator,
                definitions,
                "references direct exactness check",
            )
        )
    return errors


def cfg_direct_exactness_envelope_pairing_errors(
    mode: str,
    module_path: Path,
    cfg_file: Path,
    runner_name: str,
) -> list[str]:
    if not cfg_file.name.endswith("_fast.cfg") or "_bug_" in cfg_file.name:
        return []

    references, parse_errors = cfg_operator_references(cfg_file)
    if parse_errors:
        return []

    definitions = tla_single_expression_operator_definitions(module_path)
    enveloped_exactness: set[str] = set()
    for _, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        if not operator.endswith("CorrectnessEnvelope"):
            continue
        definition = definitions.get(operator)
        if definition is None:
            continue
        enveloped_exactness.update(
            identifier
            for identifier in tla_static_non_string_identifiers(definition[1])
            if identifier.endswith("Exactness")
        )

    errors: list[str] = []
    for line_number, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES:
            continue
        if not operator.endswith("Exactness"):
            continue
        if operator in enveloped_exactness:
            continue
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_file)}:"
            f"{line_number} references direct exactness check {operator}, "
            "but no checked correctness envelope in that CFG composes it"
        )
    return errors


def cfg_trivial_check_operator_errors(
    mode: str,
    module_path: Path,
    cfg_path: Path,
    runner_name: str,
) -> list[str]:
    if not module_path.exists() or not cfg_path.exists():
        return []

    references, parse_errors = cfg_operator_references(cfg_path)
    if parse_errors:
        return []

    trivial_chains = tla_trivial_operator_chains(module_path)
    errors: list[str] = []
    for line_number, directive, operator in references:
        if directive not in CFG_CHECK_DIRECTIVES and directive != "CONSTRAINT":
            continue
        if directive in CFG_CHECK_DIRECTIVES and operator == "TypeInvariant":
            continue
        chain = trivial_chains.get(operator)
        if chain is None:
            continue

        definition_line = chain[0][1]
        value = chain[-1][2]
        if directive == "CONSTRAINT":
            reference_kind = f"{directive} operator {operator}"
        else:
            reference_kind = f"{directive} check {operator}"
        if len(chain) == 1 and value in {"TRUE", "FALSE"}:
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_path)}:{line_number} "
                f"references {reference_kind}, but "
                f"{display_path(module_path)}:{definition_line} defines it as "
                f"literal {value}"
            )
            continue
        if len(chain) == 1 and value == "TypeInvariant":
            errors.append(
                f"{mode}: {runner_name} cfg {display_path(cfg_path)}:{line_number} "
                f"references {reference_kind}, but "
                f"{display_path(module_path)}:{definition_line} aliases "
                "TypeInvariant directly"
            )
            continue

        chain_text = " -> ".join(
            f"{name}@{display_path(module_path)}:{chain_line}"
            for name, chain_line, _ in chain
        )
        if value in {"TRUE", "FALSE"}:
            terminal = f"literal {value}"
        else:
            terminal = value
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_path)}:{line_number} "
            f"references {reference_kind}, but {chain_text} "
            f"resolves to {terminal}"
        )
    return errors


def unreferenced_formal_file_errors(referenced_paths: set[Path]) -> list[str]:
    referenced_formal_paths = {
        path for path in referenced_paths if path.suffix in FORMAL_FILE_SUFFIXES
    }
    formal_inventory = set(formal_artifact_paths())
    return [
        f"{display_path(path)} is not referenced by any checked or documented "
        "Sumeragi formal mode"
        for path in sorted(formal_inventory - referenced_formal_paths)
    ]


def sorted_unique(values: list[str] | set[str]) -> list[str]:
    return sorted(set(values))


def duplicate_values(values: list[str]) -> list[str]:
    seen: set[str] = set()
    duplicates: set[str] = set()
    for value in values:
        if value in seen:
            duplicates.add(value)
        else:
            seen.add(value)
    return sorted(duplicates)


def format_items(values: list[str], limit: int = 80) -> str:
    if not values:
        return ""
    shown = values[:limit]
    suffix = ""
    if len(values) > limit:
        suffix = f"\n  ... and {len(values) - limit} more"
    return "\n".join(f"  - {value}" for value in shown) + suffix


def print_error_sections(errors: list[str]) -> None:
    print("Sumeragi formal coverage check failed:", file=sys.stderr)
    for section in errors:
        print(f"\n{section}", file=sys.stderr)


def required_command_errors(
    path: Path,
    commands: tuple[str, ...],
    owner: str,
) -> list[str]:
    text = read_text(path)
    return [
        f"{owner} {display_path(path)} is missing command: {command}"
        for command in commands
        if command not in text
    ]


def required_text_errors(
    path: Path,
    snippets: tuple[str, ...],
    owner: str,
) -> list[str]:
    text = read_text(path)
    return [
        f"{owner} {display_path(path)} is missing required text: {snippet}"
        for snippet in snippets
        if snippet not in text
    ]


def command_order_errors(
    path: Path,
    first: str,
    second: str,
    owner: str,
) -> list[str]:
    text = read_text(path)
    first_index = text.find(first)
    second_index = text.find(second)
    if first_index == -1 or second_index == -1 or first_index < second_index:
        return []
    return [
        f"{owner} {display_path(path)} must run {first!r} before {second!r}"
    ]


def regex_values(path: Path, pattern: re.Pattern[str]) -> list[str]:
    return pattern.findall(read_text(path))


def single_regex_value(
    path: Path,
    pattern: re.Pattern[str],
    label: str,
) -> tuple[str | None, list[str]]:
    values = regex_values(path, pattern)
    if len(values) == 1:
        return values[0], []
    return (
        None,
        [
            f"{label} {display_path(path)} declares Apalache version "
            f"{len(values)} times"
        ],
    )


def version_values_mismatch_errors(
    path: Path,
    pattern: re.Pattern[str],
    expected: str,
    label: str,
) -> list[str]:
    values = regex_values(path, pattern)
    if not values:
        return [
            f"{label} {display_path(path)} does not declare Apalache {expected}"
        ]
    return [
        f"{label} {display_path(path)} uses Apalache {value}, expected {expected}"
        for value in sorted_unique(values)
        if value != expected
    ]


def apalache_version_pin_errors() -> list[str]:
    runner_version, errors = single_regex_value(
        APALACHE_RUNNER, RUNNER_APALACHE_VERSION_RE, "Apalache runner"
    )
    if runner_version is None:
        return errors

    pinned_sources: tuple[tuple[Path, re.Pattern[str], str], ...] = (
        (TLC_RUNNER, RUNNER_APALACHE_VERSION_RE, "TLC runner"),
        (ROOT_DIR / "scripts" / "formal" / "install_apalache.sh", INSTALLER_APALACHE_VERSION_RE, "Apalache installer"),
        (PR_WORKFLOW, INSTALL_APALACHE_COMMAND_VERSION_RE, "PR workflow install command"),
        (PR_WORKFLOW, APALACHE_TOOLCHAIN_PATH_VERSION_RE, "PR workflow toolchain path"),
        (NIGHTLY_WORKFLOW, INSTALL_APALACHE_COMMAND_VERSION_RE, "nightly workflow install command"),
        (README, INSTALL_APALACHE_COMMAND_VERSION_RE, "formal README install command"),
        (README, APALACHE_TOOLCHAIN_PATH_VERSION_RE, "formal README toolchain path"),
        (README, APALACHE_DOCKER_IMAGE_VERSION_RE, "formal README Docker image"),
        (ROOT_DIR / "ci" / "README.md", INSTALL_APALACHE_COMMAND_VERSION_RE, "CI README install command"),
    )
    for path, pattern, label in pinned_sources:
        errors.extend(
            version_values_mismatch_errors(path, pattern, runner_version, label)
        )
    return errors


def expected_failure_semantics_errors(
    apalache_runner: Path = APALACHE_RUNNER,
    tlc_runner: Path = TLC_RUNNER,
) -> list[str]:
    errors: list[str] = []
    errors.extend(
        required_text_errors(
            apalache_runner,
            APALACHE_EXPECTED_FAILURE_SNIPPETS,
            "Apalache expected-failure path",
        )
    )
    errors.extend(
        required_text_errors(
            tlc_runner,
            TLC_EXPECTED_FAILURE_SNIPPETS,
            "TLC expected-failure path",
        )
    )
    return errors


def runner_invocation_errors(
    apalache_runner: Path = APALACHE_RUNNER,
    tlc_runner: Path = TLC_RUNNER,
) -> list[str]:
    errors: list[str] = []
    errors.extend(
        required_text_errors(
            apalache_runner,
            APALACHE_INVOCATION_SNIPPETS,
            "Apalache runner invocation",
        )
    )
    errors.extend(
        required_text_errors(
            tlc_runner,
            TLC_INVOCATION_SNIPPETS,
            "TLC runner invocation",
        )
    )
    return errors


def workflow_entrypoint_errors() -> list[str]:
    errors: list[str] = []
    errors.extend(
        required_command_errors(
            PR_WORKFLOW,
            (FORMAL_BASELINE_COMMAND,),
            "PR workflow",
        )
    )
    errors.extend(
        command_order_errors(
            PR_WORKFLOW,
            INSTALL_APALACHE_COMMAND_PREFIX,
            FORMAL_BASELINE_COMMAND,
            "PR workflow",
        )
    )
    errors.extend(
        required_command_errors(
            NIGHTLY_WORKFLOW,
            (FORMAL_BASELINE_COMMAND, FRONTIER_NIGHTLY_COMMAND),
            "nightly workflow",
        )
    )
    errors.extend(
        command_order_errors(
            NIGHTLY_WORKFLOW,
            INSTALL_APALACHE_COMMAND_PREFIX,
            FORMAL_BASELINE_COMMAND,
            "nightly workflow",
        )
    )
    errors.extend(
        command_order_errors(
            NIGHTLY_WORKFLOW,
            FORMAL_BASELINE_COMMAND,
            FRONTIER_NIGHTLY_COMMAND,
            "nightly workflow",
        )
    )
    errors.extend(
        required_command_errors(
            FAST_CI,
            (FORMAL_COVERAGE_COMMAND, FORMAL_EXPECTED_FAILURE_COMMAND),
            "formal baseline script",
        )
    )
    errors.extend(
        command_order_errors(
            FAST_CI,
            FORMAL_COVERAGE_COMMAND,
            APALACHE_COMMAND_PREFIX,
            "formal baseline script",
        )
    )
    return errors


def modes_without_expected_failure_marker(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    missing: list[str] = []
    for mode in sorted_unique(modes):
        case = matching_case(mode, cases)
        if case is not None and "1" not in EXPECT_FAILURE_ASSIGN_RE.findall(
            case.body
        ):
            missing.append(
                f"{mode}: {runner_name} runner case {case.label!r} "
                f"at line {case.line}"
            )
    return missing


def modes_with_unexpected_failure_marker(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    unexpected: list[str] = []
    for mode in sorted_unique(modes):
        case = matching_case(mode, cases)
        if case is not None and "1" in EXPECT_FAILURE_ASSIGN_RE.findall(case.body):
            unexpected.append(
                f"{mode}: {runner_name} runner case {case.label!r} "
                f"at line {case.line}"
            )
    return unexpected


def expected_failure_default_errors(
    path: Path,
    runner_name: str,
) -> list[str]:
    """Return errors for unsafe global expect_failure defaults."""
    lines = read_text(path).splitlines()
    starts = [index for index, line in enumerate(lines) if line == 'case "$mode" in']
    if len(starts) != 1:
        return []
    start = starts[0]
    try:
        end = next(
            index for index, line in enumerate(lines[start + 1 :], start + 1)
            if line == "esac"
        )
    except StopIteration:
        return []

    errors: list[str] = []
    values: list[tuple[int, str]] = []
    for index, line in enumerate(lines):
        if start <= index <= end:
            continue
        if not EXPECT_FAILURE_MUTATION_RE.match(line):
            continue
        match = EXPECT_FAILURE_ASSIGN_RE.match(line)
        line_number = index + 1
        if match is None:
            errors.append(
                f"{runner_name} runner {display_path(path)}:{line_number} has "
                f"malformed top-level expect_failure assignment: {line.strip()}"
            )
            continue
        values.append((line_number, match.group(1)))

    if len(values) != 1:
        errors.append(
            f"{runner_name} runner {display_path(path)} must declare exactly "
            f"one top-level expect_failure=0 default, found {len(values)}"
        )
    elif values[0][1] != "0":
        errors.append(
            f"{runner_name} runner {display_path(path)}:{values[0][0]} must "
            "set top-level expect_failure default to 0"
        )
    return errors


def expected_failure_assignment_errors(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    """Return malformed per-case expect_failure assignment errors."""
    errors: list[str] = []
    for mode in sorted_unique(modes):
        case = matching_case(mode, cases)
        if case is None:
            continue
        errors.extend(
            malformed_scalar_assignment_errors(
                mode,
                case,
                "expect_failure",
                EXPECT_FAILURE_ASSIGN_RE,
                f"{runner_name} runner",
            )
        )
        assignments = EXPECT_FAILURE_ASSIGN_RE.findall(case.body)
        if len(assignments) > 1:
            errors.append(
                f"{mode}: {runner_name} runner case {case.label!r} at line "
                f"{case.line} assigns expect_failure {len(assignments)} times"
            )
        elif assignments == ["0"]:
            errors.append(
                f"{mode}: {runner_name} runner case {case.label!r} at line "
                f"{case.line} sets expect_failure=0 inside a mode case; keep "
                "the default at top level"
            )
    return errors


def apalache_typecheck_default_errors(path: Path = APALACHE_RUNNER) -> list[str]:
    """Return errors for unsafe global typecheck_only defaults."""
    lines = read_text(path).splitlines()
    starts = [index for index, line in enumerate(lines) if line == 'case "$mode" in']
    if len(starts) != 1:
        return []
    start = starts[0]
    try:
        end = next(
            index for index, line in enumerate(lines[start + 1 :], start + 1)
            if line == "esac"
        )
    except StopIteration:
        return []

    errors: list[str] = []
    values: list[tuple[int, str]] = []
    for index, line in enumerate(lines):
        if start <= index <= end:
            continue
        if not TYPECHECK_ONLY_MUTATION_RE.match(line):
            continue
        match = TYPECHECK_ONLY_ASSIGN_RE.match(line)
        line_number = index + 1
        if match is None:
            errors.append(
                f"Apalache runner {display_path(path)}:{line_number} has "
                f"malformed top-level typecheck_only assignment: {line.strip()}"
            )
            continue
        values.append((line_number, match.group(1)))

    if len(values) != 1:
        errors.append(
            f"Apalache runner {display_path(path)} must declare exactly one "
            f"top-level typecheck_only=0 default, found {len(values)}"
        )
    elif values[0][1] != "0":
        errors.append(
            f"Apalache runner {display_path(path)}:{values[0][0]} must set "
            "top-level typecheck_only default to 0"
        )
    return errors


def apalache_typecheck_only_mode_errors(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    allowed_modes: set[str] = APALACHE_TYPECHECK_ONLY_MODES,
) -> list[str]:
    """Return errors for Apalache modes that weaken checks to typecheck-only."""
    errors: list[str] = []
    marked_modes: set[str] = set()
    for mode in sorted_unique(modes):
        case = matching_case(mode, cases)
        if case is None:
            continue
        errors.extend(
            malformed_scalar_assignment_errors(
                mode,
                case,
                "typecheck_only",
                TYPECHECK_ONLY_ASSIGN_RE,
                "Apalache runner",
            )
        )
        assignments = TYPECHECK_ONLY_ASSIGN_RE.findall(case.body)
        if len(assignments) > 1:
            errors.append(
                f"{mode}: Apalache runner case {case.label!r} at line "
                f"{case.line} assigns typecheck_only {len(assignments)} times"
            )
            continue
        if not assignments:
            continue
        value = assignments[0]
        if value == "0":
            errors.append(
                f"{mode}: Apalache runner case {case.label!r} at line "
                f"{case.line} sets typecheck_only=0 inside a mode case; keep "
                "the default at top level"
            )
            continue
        marked_modes.add(mode)
        if mode not in allowed_modes:
            errors.append(
                f"{mode}: Apalache runner case {case.label!r} at line "
                f"{case.line} sets typecheck_only=1 outside "
                "APALACHE_TYPECHECK_ONLY_MODES"
            )

    for mode in sorted_unique(allowed_modes - marked_modes):
        case = matching_case(mode, cases)
        if case is None:
            errors.append(
                f"{mode}: listed in APALACHE_TYPECHECK_ONLY_MODES but has no "
                "matching Apalache runner case"
            )
        else:
            errors.append(
                f"{mode}: listed in APALACHE_TYPECHECK_ONLY_MODES but "
                f"Apalache runner case {case.label!r} at line {case.line} "
                "does not set typecheck_only=1"
            )
    return errors


def apalache_length_table_errors(
    documented_lengths: dict[str, int],
    cases: dict[str, RunnerCase],
) -> list[str]:
    errors: list[str] = []
    for mode, documented_length in sorted(documented_lengths.items()):
        case = matching_case(mode, cases)
        if case is None:
            continue
        actual_length, length_errors = apalache_length_value(mode, case)
        if length_errors or actual_length is None:
            continue
        if actual_length != documented_length:
            errors.append(
                f"{mode}: README length {documented_length} differs from "
                f"Apalache runner length {actual_length}"
            )
    return errors


def cfg_file_for_mode(mode: str, case: RunnerCase) -> tuple[Path | None, list[str]]:
    files, errors = referenced_files(mode, case, required_variables=("cfg_file",))
    cfg_files = [path for path in files if path.suffix == ".cfg"]
    if len(cfg_files) != 1:
        errors.append(
            f"{mode}: runner case {case.label!r} at line {case.line} "
            f"resolves {len(cfg_files)} cfg files"
        )
        return None, errors
    return cfg_files[0], errors


def spec_file_for_mode(mode: str, case: RunnerCase) -> tuple[Path | None, list[str]]:
    files, errors = referenced_files(mode, case, required_variables=("spec_file",))
    spec_files = [path for path in files if path.suffix == ".tla"]
    if len(spec_files) != 1:
        errors.append(
            f"{mode}: runner case {case.label!r} at line {case.line} "
            f"resolves {len(spec_files)} TLA spec files"
        )
        return None, errors
    return spec_files[0], errors


def module_identity_errors(
    modes: list[str] | set[str],
    apalache_cases: dict[str, RunnerCase],
    tlc_cases: dict[str, RunnerCase],
) -> list[str]:
    errors: list[str] = []
    for mode in sorted_unique(modes):
        apalache_case = matching_case(mode, apalache_cases)
        tlc_case = matching_case(mode, tlc_cases)
        if apalache_case is None or tlc_case is None:
            continue

        apalache_spec, apalache_errors = spec_file_for_mode(mode, apalache_case)
        tlc_modules, tlc_errors = tlc_module_files(mode, tlc_case)
        if apalache_errors or tlc_errors:
            continue
        if apalache_spec is None or len(tlc_modules) != 1:
            continue
        tlc_module = tlc_modules[0]
        if apalache_spec != tlc_module:
            errors.append(
                f"{mode}: Apalache spec {display_path(apalache_spec)} differs "
                f"from TLC module {display_path(tlc_module)}"
            )
    return errors


def allowed_mutation_cfg_pair(mode: str, apalache_cfg: Path, tlc_cfg: Path) -> bool:
    if apalache_cfg == tlc_cfg:
        return True
    if not any(
        mode.startswith(prefix) for prefix in TLC_SPECIFIC_MUTATION_CFG_PREFIXES
    ):
        return False
    if "_bug_" not in apalache_cfg.name:
        return False
    expected_tlc_name = apalache_cfg.name.replace("_bug_", "_tlc_bug_", 1)
    return tlc_cfg == apalache_cfg.with_name(expected_tlc_name)


def mutation_mode_slug(mode: str) -> str:
    return mode.split("-bug-", 1)[1].replace("-", "_")


def mutation_cfg_name_errors(
    modes: list[str] | set[str],
    cases: dict[str, RunnerCase],
    runner_name: str,
) -> list[str]:
    errors: list[str] = []
    for mode in sorted_unique(modes):
        if "-bug-" not in mode:
            continue
        case = matching_case(mode, cases)
        if case is None:
            continue

        cfg_file, cfg_errors = cfg_file_for_mode(mode, case)
        if cfg_errors or cfg_file is None:
            continue

        slug = mutation_mode_slug(mode)
        expected_fragments = [f"_bug_{slug}"]
        if runner_name == "TLC" and any(
            mode.startswith(prefix) for prefix in TLC_SPECIFIC_MUTATION_CFG_PREFIXES
        ):
            expected_fragments.append(f"_tlc_bug_{slug}")
        if any(fragment in cfg_file.stem for fragment in expected_fragments):
            continue
        expected = " or ".join(expected_fragments)
        errors.append(
            f"{mode}: {runner_name} cfg {display_path(cfg_file)} does not "
            f"contain expected mutation fragment {expected}"
        )
    return errors


def mutation_cfg_equivalence_errors(
    modes: list[str] | set[str],
    apalache_cases: dict[str, RunnerCase],
    tlc_cases: dict[str, RunnerCase],
) -> list[str]:
    errors: list[str] = []
    for mode in sorted_unique(modes):
        apalache_case = matching_case(mode, apalache_cases)
        tlc_case = matching_case(mode, tlc_cases)
        if apalache_case is None or tlc_case is None:
            continue

        apalache_cfg, apalache_errors = cfg_file_for_mode(mode, apalache_case)
        tlc_cfg, tlc_errors = cfg_file_for_mode(mode, tlc_case)
        if apalache_errors or tlc_errors:
            continue
        if apalache_cfg is None or tlc_cfg is None:
            continue
        if not allowed_mutation_cfg_pair(mode, apalache_cfg, tlc_cfg):
            errors.append(
                f"{mode}: Apalache cfg {display_path(apalache_cfg)} differs "
                f"from TLC cfg {display_path(tlc_cfg)}"
            )
    return errors


def cfg_required_check_contract_errors(
    cfg_path: Path,
    required_checks: tuple[tuple[str, str], ...],
    coverage_label: str,
) -> list[str]:
    """Return errors when a CFG file omits or downgrades required proof checks."""

    if not cfg_path.exists():
        return [
            f"{display_path(cfg_path)} is missing required {coverage_label} checks"
        ]

    check_kinds, cfg_errors = cfg_check_operator_kinds(cfg_path)
    if cfg_errors:
        return [f"{display_path(cfg_path)}: {error}" for error in cfg_errors]

    errors: list[str] = []
    for operator, expected_kind in required_checks:
        kind = check_kinds.get(operator)
        if kind is None:
            errors.append(
                f"{display_path(cfg_path)} must check {expected_kind} {operator} "
                f"for {coverage_label}"
            )
        elif kind != expected_kind:
            errors.append(
                f"{display_path(cfg_path)} checks {kind} {operator}, "
                f"expected {expected_kind}"
            )
    return errors


def top_level_fast_cfg_check_errors(
    cfg_path: Path = SUMERAGI_FAST_CFG,
    required_checks: tuple[tuple[str, str], ...] = SUMERAGI_FAST_CFG_REQUIRED_CHECKS,
) -> list[str]:
    """Return errors if the root fast CFG stops checking its sentinel surface."""

    return cfg_required_check_contract_errors(
        cfg_path,
        required_checks,
        "top-level fast sentinel coverage",
    )


def projection_clean_cfg_errors(
    fast_cfg: Path = PROJECTION_FAST_CFG,
    progress_cfg: Path = PROJECTION_PROGRESS_CFG,
) -> list[str]:
    """Return errors if clean projection CFGs stop checking the proof surface."""

    return cfg_required_behavior_contract_errors(
        fast_cfg,
        CLEAN_SAFETY_CFG_REQUIRED_BEHAVIOR,
        "projection fast coverage",
    ) + cfg_required_constant_value_contract_errors(
        fast_cfg,
        "Bug",
        CLEAN_SAFETY_BUG_CONSTANT_VALUE,
        "projection fast coverage",
    ) + cfg_required_check_contract_errors(
        fast_cfg,
        PROJECTION_FAST_CFG_REQUIRED_CHECKS,
        "projection fast coverage",
    ) + cfg_required_check_deadlock_contract_errors(
        progress_cfg,
        "FALSE",
        "projection progress coverage",
    ) + cfg_required_constant_value_contract_errors(
        progress_cfg,
        "Bug",
        CLEAN_PROGRESS_BUG_CONSTANT_VALUE,
        "projection progress coverage",
    ) + cfg_required_behavior_contract_errors(
        progress_cfg,
        PROJECTION_PROGRESS_CFG_REQUIRED_BEHAVIOR,
        "projection progress coverage",
    ) + cfg_required_check_contract_errors(
        progress_cfg,
        PROJECTION_PROGRESS_CFG_REQUIRED_CHECKS,
        "projection progress coverage",
    )


def byzantine_top_cfg_errors(
    cfg_contracts: tuple[
        tuple[Path, tuple[tuple[str, str], ...], str],
        ...,
    ] = BYZANTINE_TOP_CFG_REQUIRED_CHECKS,
) -> list[str]:
    """Return errors if top-level Byzantine CFGs stop naming bridge checks."""

    errors: list[str] = []
    for cfg_path, required_checks, coverage_label in cfg_contracts:
        behavior_contract = BYZANTINE_TOP_CFG_REQUIRED_BEHAVIOR_BY_NAME.get(
            cfg_path.name
        )
        if behavior_contract is not None:
            required_behavior, behavior_label = behavior_contract
            errors.extend(
                cfg_required_behavior_contract_errors(
                    cfg_path,
                    required_behavior,
                    behavior_label,
                )
            )
        constant_contract = BYZANTINE_TOP_CFG_REQUIRED_CONSTANT_VALUES_BY_NAME.get(
            cfg_path.name
        )
        if constant_contract is not None:
            required_values, constant_label = constant_contract
            errors.extend(
                cfg_required_constant_values_contract_errors(
                    cfg_path,
                    required_values,
                    constant_label,
                )
            )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                required_checks,
                coverage_label,
            )
        )
    return errors


def byzantine_interleaving_clean_cfg_errors(
    fast_cfg: Path = BYZANTINE_INTERLEAVING_FAST_CFG,
    progress_cfg: Path = BYZANTINE_INTERLEAVING_PROGRESS_CFG,
) -> list[str]:
    """Return errors if clean source Byzantine CFGs drop proof checks."""

    return cfg_required_behavior_contract_errors(
        fast_cfg,
        CLEAN_SAFETY_CFG_REQUIRED_BEHAVIOR,
        "Byzantine interleaving fast coverage",
    ) + cfg_required_constant_value_contract_errors(
        fast_cfg,
        "Bug",
        CLEAN_SAFETY_BUG_CONSTANT_VALUE,
        "Byzantine interleaving fast coverage",
    ) + cfg_required_check_contract_errors(
        fast_cfg,
        BYZANTINE_INTERLEAVING_FAST_CFG_REQUIRED_CHECKS,
        "Byzantine interleaving fast coverage",
    ) + cfg_required_check_deadlock_contract_errors(
        progress_cfg,
        "FALSE",
        "Byzantine interleaving progress coverage",
    ) + cfg_required_constant_value_contract_errors(
        progress_cfg,
        "Bug",
        CLEAN_PROGRESS_BUG_CONSTANT_VALUE,
        "Byzantine interleaving progress coverage",
    ) + cfg_required_behavior_contract_errors(
        progress_cfg,
        BYZANTINE_INTERLEAVING_PROGRESS_CFG_REQUIRED_BEHAVIOR,
        "Byzantine interleaving progress coverage",
    ) + cfg_required_check_contract_errors(
        progress_cfg,
        BYZANTINE_INTERLEAVING_PROGRESS_CFG_REQUIRED_CHECKS,
        "Byzantine interleaving progress coverage",
    )


def direct_delivered_first_clean_cfg_errors(
    fast_cfg: Path = DIRECT_DELIVERED_FIRST_FAST_CFG,
    progress_cfg: Path = DIRECT_DELIVERED_FIRST_PROGRESS_CFG,
) -> list[str]:
    """Return errors if clean delivered-first CFGs drop proof checks."""

    return cfg_required_behavior_contract_errors(
        fast_cfg,
        CLEAN_SAFETY_CFG_REQUIRED_BEHAVIOR,
        "direct delivered-first fast coverage",
    ) + cfg_required_constant_value_contract_errors(
        fast_cfg,
        "Bug",
        CLEAN_SAFETY_BUG_CONSTANT_VALUE,
        "direct delivered-first fast coverage",
    ) + cfg_required_check_contract_errors(
        fast_cfg,
        DIRECT_DELIVERED_FIRST_FAST_CFG_REQUIRED_CHECKS,
        "direct delivered-first fast coverage",
    ) + cfg_required_check_deadlock_contract_errors(
        progress_cfg,
        "FALSE",
        "direct delivered-first progress coverage",
    ) + cfg_required_constant_value_contract_errors(
        progress_cfg,
        "Bug",
        CLEAN_PROGRESS_BUG_CONSTANT_VALUE,
        "direct delivered-first progress coverage",
    ) + cfg_required_behavior_contract_errors(
        progress_cfg,
        DIRECT_DELIVERED_FIRST_PROGRESS_CFG_REQUIRED_BEHAVIOR,
        "direct delivered-first progress coverage",
    ) + cfg_required_check_contract_errors(
        progress_cfg,
        DIRECT_DELIVERED_FIRST_PROGRESS_CFG_REQUIRED_CHECKS,
        "direct delivered-first progress coverage",
    )


def direct_vote_first_clean_cfg_errors(
    fast_cfg: Path = DIRECT_VOTE_FIRST_FAST_CFG,
    progress_cfg: Path = DIRECT_VOTE_FIRST_PROGRESS_CFG,
) -> list[str]:
    """Return errors if clean vote-first CFGs drop proof checks."""

    return cfg_required_behavior_contract_errors(
        fast_cfg,
        CLEAN_SAFETY_CFG_REQUIRED_BEHAVIOR,
        "direct vote-first fast coverage",
    ) + cfg_required_constant_value_contract_errors(
        fast_cfg,
        "Bug",
        CLEAN_SAFETY_BUG_CONSTANT_VALUE,
        "direct vote-first fast coverage",
    ) + cfg_required_check_contract_errors(
        fast_cfg,
        DIRECT_VOTE_FIRST_FAST_CFG_REQUIRED_CHECKS,
        "direct vote-first fast coverage",
    ) + cfg_required_check_deadlock_contract_errors(
        progress_cfg,
        "FALSE",
        "direct vote-first progress coverage",
    ) + cfg_required_constant_value_contract_errors(
        progress_cfg,
        "Bug",
        CLEAN_PROGRESS_BUG_CONSTANT_VALUE,
        "direct vote-first progress coverage",
    ) + cfg_required_behavior_contract_errors(
        progress_cfg,
        DIRECT_VOTE_FIRST_PROGRESS_CFG_REQUIRED_BEHAVIOR,
        "direct vote-first progress coverage",
    ) + cfg_required_check_contract_errors(
        progress_cfg,
        DIRECT_VOTE_FIRST_PROGRESS_CFG_REQUIRED_CHECKS,
        "direct vote-first progress coverage",
    )


def direct_interleaving_clean_cfg_errors(
    fast_cfg: Path = DIRECT_INTERLEAVING_FAST_CFG,
    progress_cfg: Path = DIRECT_INTERLEAVING_PROGRESS_CFG,
) -> list[str]:
    """Return errors if clean direct interleaving CFGs drop proof checks."""

    return cfg_required_behavior_contract_errors(
        fast_cfg,
        CLEAN_SAFETY_CFG_REQUIRED_BEHAVIOR,
        "direct interleaving fast coverage",
    ) + cfg_required_constant_value_contract_errors(
        fast_cfg,
        "Bug",
        CLEAN_SAFETY_BUG_CONSTANT_VALUE,
        "direct interleaving fast coverage",
    ) + cfg_required_check_contract_errors(
        fast_cfg,
        DIRECT_INTERLEAVING_FAST_CFG_REQUIRED_CHECKS,
        "direct interleaving fast coverage",
    ) + cfg_required_check_deadlock_contract_errors(
        progress_cfg,
        "FALSE",
        "direct interleaving progress coverage",
    ) + cfg_required_constant_value_contract_errors(
        progress_cfg,
        "Bug",
        CLEAN_PROGRESS_BUG_CONSTANT_VALUE,
        "direct interleaving progress coverage",
    ) + cfg_required_behavior_contract_errors(
        progress_cfg,
        DIRECT_INTERLEAVING_PROGRESS_CFG_REQUIRED_BEHAVIOR,
        "direct interleaving progress coverage",
    ) + cfg_required_check_contract_errors(
        progress_cfg,
        DIRECT_INTERLEAVING_PROGRESS_CFG_REQUIRED_CHECKS,
        "direct interleaving progress coverage",
    )


def source_safety_mutation_cfg_errors(
    cfg_contracts: tuple[
        tuple[str, tuple[tuple[str, str], ...], str],
        ...,
    ] = SOURCE_SAFETY_MUTATION_CFG_REQUIRED_CHECKS,
    spec_dir: Path = SPEC_DIR,
) -> list[str]:
    """Return errors if source safety mutation CFGs drop proof checks."""

    errors: list[str] = []
    for cfg_glob, required_checks, coverage_label in cfg_contracts:
        cfg_paths = sorted(spec_dir.glob(cfg_glob))
        if not cfg_paths:
            errors.append(f"no {coverage_label} cfgs matched {cfg_glob}")
            continue
        for cfg_path in cfg_paths:
            errors.extend(
                cfg_required_inferred_bug_suffix_constant_errors(
                    cfg_path,
                    coverage_label,
                )
            )
            errors.extend(
                cfg_required_behavior_contract_errors(
                    cfg_path,
                    SAFETY_MUTATION_CFG_REQUIRED_BEHAVIOR,
                    coverage_label,
                )
            )
            errors.extend(
                cfg_required_check_contract_errors(
                    cfg_path,
                    required_checks,
                    coverage_label,
                )
            )
    return errors


def direct_interleaving_progress_mutation_cfg_errors(
    cfg_paths: list[Path] | None = None,
) -> list[str]:
    """Return errors if direct interleaving progress mutation CFGs drop checks."""

    if cfg_paths is None:
        cfg_paths = sorted(SPEC_DIR.glob(DIRECT_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB))

    errors: list[str] = []
    if not cfg_paths:
        errors.append(
            "no direct interleaving progress mutation cfgs matched "
            f"{DIRECT_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB}"
        )
        return errors

    for cfg_path in cfg_paths:
        errors.extend(
            cfg_required_check_deadlock_contract_errors(
                cfg_path,
                "FALSE",
                "direct interleaving progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_bug_suffix_constant_errors(
                cfg_path,
                DIRECT_INTERLEAVING_PROGRESS_MUTATION_STEM_PREFIX,
                "direct interleaving progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                DIRECT_INTERLEAVING_PROGRESS_CFG_REQUIRED_BEHAVIOR,
                "direct interleaving progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                DIRECT_INTERLEAVING_PROGRESS_MUTATION_REQUIRED_CHECKS,
                "direct interleaving progress mutation coverage",
            )
        )
    return errors


def direct_delivered_first_progress_mutation_cfg_errors(
    cfg_paths: list[Path] | None = None,
) -> list[str]:
    """Return errors if delivered-first progress mutation CFGs drop checks."""

    if cfg_paths is None:
        cfg_paths = sorted(
            SPEC_DIR.glob(DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_CFG_GLOB)
        )

    errors: list[str] = []
    if not cfg_paths:
        errors.append(
            "no direct delivered-first progress mutation cfgs matched "
            f"{DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_CFG_GLOB}"
        )
        return errors

    for cfg_path in cfg_paths:
        errors.extend(
            cfg_required_check_deadlock_contract_errors(
                cfg_path,
                "FALSE",
                "direct delivered-first progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_bug_suffix_constant_errors(
                cfg_path,
                DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_STEM_PREFIX,
                "direct delivered-first progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                DIRECT_DELIVERED_FIRST_PROGRESS_CFG_REQUIRED_BEHAVIOR,
                "direct delivered-first progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_REQUIRED_CHECKS,
                "direct delivered-first progress mutation coverage",
            )
        )
    return errors


def direct_vote_first_progress_mutation_cfg_errors(
    cfg_paths: list[Path] | None = None,
) -> list[str]:
    """Return errors if vote-first progress mutation CFGs drop checks."""

    if cfg_paths is None:
        cfg_paths = sorted(SPEC_DIR.glob(DIRECT_VOTE_FIRST_PROGRESS_MUTATION_CFG_GLOB))

    errors: list[str] = []
    if not cfg_paths:
        errors.append(
            "no direct vote-first progress mutation cfgs matched "
            f"{DIRECT_VOTE_FIRST_PROGRESS_MUTATION_CFG_GLOB}"
        )
        return errors

    for cfg_path in cfg_paths:
        errors.extend(
            cfg_required_check_deadlock_contract_errors(
                cfg_path,
                "FALSE",
                "direct vote-first progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_bug_suffix_constant_errors(
                cfg_path,
                DIRECT_VOTE_FIRST_PROGRESS_MUTATION_STEM_PREFIX,
                "direct vote-first progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                DIRECT_VOTE_FIRST_PROGRESS_CFG_REQUIRED_BEHAVIOR,
                "direct vote-first progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                DIRECT_VOTE_FIRST_PROGRESS_MUTATION_REQUIRED_CHECKS,
                "direct vote-first progress mutation coverage",
            )
        )
    return errors


def byzantine_interleaving_progress_mutation_cfg_errors(
    cfg_paths: list[Path] | None = None,
) -> list[str]:
    """Return errors if source Byzantine progress mutation CFGs drop checks."""

    if cfg_paths is None:
        cfg_paths = sorted(
            SPEC_DIR.glob(BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB)
        )

    errors: list[str] = []
    if not cfg_paths:
        errors.append(
            "no Byzantine interleaving progress mutation cfgs matched "
            f"{BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB}"
        )
        return errors

    for cfg_path in cfg_paths:
        errors.extend(
            cfg_required_check_deadlock_contract_errors(
                cfg_path,
                "FALSE",
                "Byzantine interleaving progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_bug_suffix_constant_errors(
                cfg_path,
                BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_STEM_PREFIX,
                "Byzantine interleaving progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                BYZANTINE_INTERLEAVING_PROGRESS_CFG_REQUIRED_BEHAVIOR,
                "Byzantine interleaving progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_REQUIRED_CHECKS,
                "Byzantine interleaving progress mutation coverage",
            )
        )
    return errors


def projection_mutation_bridge_cfg_errors(
    cfg_paths: list[Path] | None = None,
) -> list[str]:
    """Return errors if projection mutation CFGs stop checking the full bridge."""

    if cfg_paths is None:
        cfg_paths = sorted(SPEC_DIR.glob(PROJECTION_MUTATION_CFG_GLOB))

    errors: list[str] = []
    if not cfg_paths:
        errors.append(
            f"no projection mutation cfgs matched {PROJECTION_MUTATION_CFG_GLOB}"
        )
        return errors

    for cfg_path in cfg_paths:
        errors.extend(
            cfg_required_inferred_bug_suffix_constant_errors(
                cfg_path,
                "projection bridge mutation coverage",
            )
        )
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                SAFETY_MUTATION_CFG_REQUIRED_BEHAVIOR,
                "projection bridge mutation coverage",
            )
        )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                PROJECTION_MUTATION_BRIDGE_REQUIRED_CHECKS,
                "projection bridge mutation coverage",
            )
        )
    return errors


def projection_progress_mutation_cfg_errors(
    cfg_paths: list[Path] | None = None,
) -> list[str]:
    """Return errors if projection progress mutation CFGs drop required checks."""

    if cfg_paths is None:
        cfg_paths = sorted(SPEC_DIR.glob(PROJECTION_PROGRESS_MUTATION_CFG_GLOB))

    errors: list[str] = []
    if not cfg_paths:
        errors.append(
            "no projection progress mutation cfgs matched "
            f"{PROJECTION_PROGRESS_MUTATION_CFG_GLOB}"
        )
        return errors

    for cfg_path in cfg_paths:
        errors.extend(
            cfg_required_check_deadlock_contract_errors(
                cfg_path,
                "FALSE",
                "projection progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_bug_suffix_constant_errors(
                cfg_path,
                PROJECTION_PROGRESS_MUTATION_STEM_PREFIX,
                "projection progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_behavior_contract_errors(
                cfg_path,
                PROJECTION_PROGRESS_CFG_REQUIRED_BEHAVIOR,
                "projection progress mutation coverage",
            )
        )
        errors.extend(
            cfg_required_check_contract_errors(
                cfg_path,
                PROJECTION_PROGRESS_MUTATION_REQUIRED_CHECKS,
                "projection progress mutation coverage",
            )
        )
    return errors


def mutation_cfg_suffixes(
    cfg_paths: list[Path],
    stem_prefix: str,
    family_label: str,
) -> tuple[set[str], list[str]]:
    """Return normalized mutation suffixes for a family of CFG files."""

    suffixes: set[str] = set()
    errors: list[str] = []
    if not cfg_paths:
        errors.append(f"no {family_label} mutation cfgs were found")
        return suffixes, errors

    for cfg_path in cfg_paths:
        stem = cfg_path.stem
        if not stem.startswith(stem_prefix):
            errors.append(
                f"{display_path(cfg_path)} does not match expected "
                f"{family_label} mutation prefix {stem_prefix}"
            )
            continue
        suffix = stem[len(stem_prefix) :]
        if not suffix:
            errors.append(
                f"{display_path(cfg_path)} has an empty {family_label} "
                "mutation suffix"
            )
            continue
        suffixes.add(suffix)
    return suffixes, errors


def mutation_suffix_set_difference_errors(
    actual_label: str,
    actual_suffixes: set[str],
    expected_label: str,
    expected_suffixes: set[str],
) -> list[str]:
    """Return errors when one mutation suffix family stops matching another."""

    errors: list[str] = []
    missing_suffixes = sorted_unique(expected_suffixes - actual_suffixes)
    if missing_suffixes:
        errors.append(
            f"{actual_label} mutation CFGs are missing {expected_label} "
            "mutation suffixes:\n"
            + format_items(missing_suffixes)
        )

    extra_suffixes = sorted_unique(actual_suffixes - expected_suffixes)
    if extra_suffixes:
        errors.append(
            f"{actual_label} mutation CFGs have suffixes absent from "
            f"{expected_label}:\n"
            + format_items(extra_suffixes)
        )
    return errors


def direct_mutation_family_alignment_errors(
    delivered_cfg_paths: list[Path] | None = None,
    delivered_progress_cfg_paths: list[Path] | None = None,
    vote_cfg_paths: list[Path] | None = None,
    vote_progress_cfg_paths: list[Path] | None = None,
    interleaving_cfg_paths: list[Path] | None = None,
    interleaving_progress_cfg_paths: list[Path] | None = None,
    delivered_progress_safety_only_mutations: frozenset[str] = (
        DIRECT_DELIVERED_FIRST_PROGRESS_SAFETY_ONLY_MUTATIONS
    ),
    vote_progress_safety_only_mutations: frozenset[str] = (
        DIRECT_VOTE_FIRST_PROGRESS_SAFETY_ONLY_MUTATIONS
    ),
    interleaving_progress_safety_only_mutations: frozenset[str] = (
        DIRECT_INTERLEAVING_PROGRESS_SAFETY_ONLY_MUTATIONS
    ),
) -> list[str]:
    """Return errors if direct safety/progress mutation families drift."""

    if delivered_cfg_paths is None:
        delivered_cfg_paths = sorted(
            SPEC_DIR.glob(DIRECT_DELIVERED_FIRST_MUTATION_CFG_GLOB)
        )
    if delivered_progress_cfg_paths is None:
        delivered_progress_cfg_paths = sorted(
            SPEC_DIR.glob(DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_CFG_GLOB)
        )
    if vote_cfg_paths is None:
        vote_cfg_paths = sorted(SPEC_DIR.glob(DIRECT_VOTE_FIRST_MUTATION_CFG_GLOB))
    if vote_progress_cfg_paths is None:
        vote_progress_cfg_paths = sorted(
            SPEC_DIR.glob(DIRECT_VOTE_FIRST_PROGRESS_MUTATION_CFG_GLOB)
        )
    if interleaving_cfg_paths is None:
        interleaving_cfg_paths = sorted(
            SPEC_DIR.glob(DIRECT_INTERLEAVING_MUTATION_CFG_GLOB)
        )
    if interleaving_progress_cfg_paths is None:
        interleaving_progress_cfg_paths = sorted(
            SPEC_DIR.glob(DIRECT_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB)
        )

    family_specs = (
        (
            "direct delivered-first",
            delivered_cfg_paths,
            DIRECT_DELIVERED_FIRST_MUTATION_STEM_PREFIX,
            delivered_progress_cfg_paths,
            DIRECT_DELIVERED_FIRST_PROGRESS_MUTATION_STEM_PREFIX,
            delivered_progress_safety_only_mutations,
        ),
        (
            "direct vote-first",
            vote_cfg_paths,
            DIRECT_VOTE_FIRST_MUTATION_STEM_PREFIX,
            vote_progress_cfg_paths,
            DIRECT_VOTE_FIRST_PROGRESS_MUTATION_STEM_PREFIX,
            vote_progress_safety_only_mutations,
        ),
        (
            "direct interleaving",
            interleaving_cfg_paths,
            DIRECT_INTERLEAVING_MUTATION_STEM_PREFIX,
            interleaving_progress_cfg_paths,
            DIRECT_INTERLEAVING_PROGRESS_MUTATION_STEM_PREFIX,
            interleaving_progress_safety_only_mutations,
        ),
    )

    errors: list[str] = []
    for (
        family_label,
        safety_cfg_paths,
        safety_stem_prefix,
        progress_cfg_paths,
        progress_stem_prefix,
        progress_safety_only_mutations,
    ) in family_specs:
        safety_suffixes, safety_errors = mutation_cfg_suffixes(
            safety_cfg_paths,
            safety_stem_prefix,
            f"{family_label} safety",
        )
        progress_suffixes, progress_errors = mutation_cfg_suffixes(
            progress_cfg_paths,
            progress_stem_prefix,
            f"{family_label} progress",
        )
        errors.extend(safety_errors)
        errors.extend(progress_errors)
        if safety_errors or progress_errors:
            continue

        stale_allowlist = sorted_unique(
            progress_safety_only_mutations - safety_suffixes
        )
        if stale_allowlist:
            errors.append(
                f"{family_label} progress safety-only mutation allowlist has "
                "stale suffixes absent from the safety family:\n"
                + format_items(stale_allowlist)
            )

        expected_progress = safety_suffixes - progress_safety_only_mutations
        errors.extend(
            mutation_suffix_set_difference_errors(
                f"{family_label} progress",
                progress_suffixes,
                f"{family_label} safety minus safety-only mutations",
                expected_progress,
            )
        )
    return errors


def byzantine_mutation_family_alignment_errors(
    interleaving_cfg_paths: list[Path] | None = None,
    interleaving_progress_cfg_paths: list[Path] | None = None,
    projection_cfg_paths: list[Path] | None = None,
    projection_progress_cfg_paths: list[Path] | None = None,
    progress_safety_only_mutations: frozenset[str] = (
        BYZANTINE_PROGRESS_SAFETY_ONLY_MUTATIONS
    ),
) -> list[str]:
    """Return errors if Byzantine source/projection mutation families drift."""

    if interleaving_cfg_paths is None:
        interleaving_cfg_paths = sorted(
            SPEC_DIR.glob(BYZANTINE_INTERLEAVING_MUTATION_CFG_GLOB)
        )
    if interleaving_progress_cfg_paths is None:
        interleaving_progress_cfg_paths = sorted(
            SPEC_DIR.glob(BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_CFG_GLOB)
        )
    if projection_cfg_paths is None:
        projection_cfg_paths = sorted(SPEC_DIR.glob(PROJECTION_MUTATION_CFG_GLOB))
    if projection_progress_cfg_paths is None:
        projection_progress_cfg_paths = sorted(
            SPEC_DIR.glob(PROJECTION_PROGRESS_MUTATION_CFG_GLOB)
        )

    family_specs = (
        (
            "source Byzantine interleaving safety",
            interleaving_cfg_paths,
            BYZANTINE_INTERLEAVING_MUTATION_STEM_PREFIX,
        ),
        (
            "source Byzantine interleaving progress",
            interleaving_progress_cfg_paths,
            BYZANTINE_INTERLEAVING_PROGRESS_MUTATION_STEM_PREFIX,
        ),
        (
            "projection safety",
            projection_cfg_paths,
            PROJECTION_MUTATION_STEM_PREFIX,
        ),
        (
            "projection progress",
            projection_progress_cfg_paths,
            PROJECTION_PROGRESS_MUTATION_STEM_PREFIX,
        ),
    )

    suffix_sets: dict[str, set[str]] = {}
    errors: list[str] = []
    for family_label, cfg_paths, stem_prefix in family_specs:
        suffixes, suffix_errors = mutation_cfg_suffixes(
            cfg_paths,
            stem_prefix,
            family_label,
        )
        suffix_sets[family_label] = suffixes
        errors.extend(suffix_errors)

    if errors:
        return errors

    source_safety = suffix_sets["source Byzantine interleaving safety"]
    source_progress = suffix_sets["source Byzantine interleaving progress"]
    projection_safety = suffix_sets["projection safety"]
    projection_progress = suffix_sets["projection progress"]

    errors.extend(
        mutation_suffix_set_difference_errors(
            "projection safety",
            projection_safety,
            "source Byzantine interleaving safety",
            source_safety,
        )
    )
    errors.extend(
        mutation_suffix_set_difference_errors(
            "projection progress",
            projection_progress,
            "source Byzantine interleaving progress",
            source_progress,
        )
    )

    stale_allowlist = sorted_unique(progress_safety_only_mutations - source_safety)
    if stale_allowlist:
        errors.append(
            "Byzantine progress safety-only mutation allowlist has stale "
            "suffixes absent from the source safety family:\n"
            + format_items(stale_allowlist)
        )

    for family_label, safety_suffixes, progress_suffixes in (
        (
            "source Byzantine interleaving",
            source_safety,
            source_progress,
        ),
        (
            "projection",
            projection_safety,
            projection_progress,
        ),
    ):
        expected_progress = safety_suffixes - progress_safety_only_mutations
        errors.extend(
            mutation_suffix_set_difference_errors(
                f"{family_label} progress",
                progress_suffixes,
                f"{family_label} safety minus safety-only mutations",
                expected_progress,
            )
        )
    return errors


def main() -> int:
    errors: list[str] = []

    conflict_marker_mismatches = conflict_marker_errors(
        (
            APALACHE_RUNNER,
            TLC_RUNNER,
            FAST_CI,
            EXPECTED_FAILURE_CI,
            PR_WORKFLOW,
            NIGHTLY_WORKFLOW,
            README,
        )
    )
    if conflict_marker_mismatches:
        print_error_sections(
            [
                "Sumeragi formal wiring files contain merge conflict markers:\n"
                + format_items(conflict_marker_mismatches)
            ]
        )
        return 1

    formal_artifact_conflict_marker_mismatches = conflict_marker_errors(
        formal_artifact_paths()
    )
    if formal_artifact_conflict_marker_mismatches:
        print_error_sections(
            [
                "Sumeragi formal TLA+/CFG files contain merge conflict markers:\n"
                + format_items(formal_artifact_conflict_marker_mismatches)
            ]
        )
        return 1

    runner_case_shape_mismatches = runner_case_shape_errors(
        APALACHE_RUNNER, "Apalache"
    )
    runner_case_shape_mismatches.extend(
        runner_case_shape_errors(TLC_RUNNER, "TLC")
    )
    if runner_case_shape_mismatches:
        print_error_sections(
            [
                "Sumeragi formal runner case blocks are malformed:\n"
                + format_items(runner_case_shape_mismatches)
            ]
        )
        return 1

    apalache_cases = parse_runner_cases(APALACHE_RUNNER)
    tlc_cases = parse_runner_cases(TLC_RUNNER)
    duplicate_apalache_case_labels = duplicate_values(
        runner_case_labels(APALACHE_RUNNER)
    )
    duplicate_tlc_case_labels = duplicate_values(runner_case_labels(TLC_RUNNER))
    shadowed_apalache_case_labels = runner_case_shadow_errors(
        apalache_cases, "Apalache"
    )
    shadowed_tlc_case_labels = runner_case_shadow_errors(tlc_cases, "TLC")
    apalache_version_mismatches = apalache_version_pin_errors()
    expected_failure_semantics_mismatches = expected_failure_semantics_errors()
    expected_failure_default_mismatches = expected_failure_default_errors(
        APALACHE_RUNNER, "Apalache"
    ) + expected_failure_default_errors(TLC_RUNNER, "TLC")
    runner_invocation_mismatches = runner_invocation_errors()
    top_level_cfg_behavior_mismatches = top_level_cfg_behavior_errors()
    top_level_cfg_deadlock_mismatches = top_level_cfg_deadlock_errors()
    top_level_cfg_constant_mismatches = top_level_cfg_constant_errors()
    top_level_fast_cfg_check_mismatches = top_level_fast_cfg_check_errors()
    top_level_cfg_check_parity_mismatches = top_level_cfg_check_parity_errors()
    property_root_reachability_mismatches = cfg_property_root_reachability_errors()
    state_invariant_root_reachability_mismatches = (
        cfg_state_invariant_root_reachability_errors()
    )
    state_invariant_root_cfg_coverage_mismatches = (
        state_invariant_root_cfg_coverage_errors()
    )
    temporal_property_root_cfg_coverage_mismatches = (
        temporal_property_root_cfg_coverage_errors()
    )
    consensus_core_root_contract_mismatches = (
        consensus_core_root_conjunct_contract_errors()
    )
    state_matches_envelope_contract_mismatches = (
        state_matches_envelope_conjunct_contract_errors()
    )
    committed_terminal_envelope_contract_mismatches = (
        committed_terminal_envelope_conjunct_contract_errors()
    )
    end_to_end_safety_envelope_contract_mismatches = (
        end_to_end_safety_envelope_conjunct_contract_errors()
    )
    post_finality_stability_envelope_contract_mismatches = (
        post_finality_stability_envelope_conjunct_contract_errors()
    )
    timeout_recovery_envelope_contract_mismatches = (
        timeout_recovery_envelope_conjunct_contract_errors()
    )
    finality_installation_envelope_contract_mismatches = (
        finality_installation_envelope_conjunct_contract_errors()
    )
    pre_commit_handoff_envelope_contract_mismatches = (
        pre_commit_handoff_envelope_conjunct_contract_errors()
    )
    commit_vote_handoff_envelope_contract_mismatches = (
        commit_vote_handoff_envelope_conjunct_contract_errors()
    )
    finalized_certificate_retention_envelope_contract_mismatches = (
        finalized_certificate_retention_envelope_conjunct_contract_errors()
    )
    rbc_delivered_finality_envelope_contract_mismatches = (
        rbc_delivered_finality_envelope_conjunct_contract_errors()
    )
    rbc_delivered_state_envelope_contract_mismatches = (
        rbc_delivered_state_envelope_conjunct_contract_errors()
    )
    rbc_delivered_pending_handoff_envelope_contract_mismatches = (
        rbc_delivered_pending_handoff_envelope_conjunct_contract_errors()
    )
    delivered_pending_complete_wait_state_envelope_contract_mismatches = (
        delivered_pending_complete_wait_state_envelope_conjunct_contract_errors()
    )
    rbc_delivery_entry_envelope_contract_mismatches = (
        rbc_delivery_entry_envelope_conjunct_contract_errors()
    )
    rbc_delivery_entry_continuation_envelope_contract_mismatches = (
        rbc_delivery_entry_continuation_envelope_conjunct_contract_errors()
    )
    rbc_lifecycle_envelope_contract_mismatches = (
        rbc_lifecycle_envelope_conjunct_contract_errors()
    )
    rbc_corruption_repair_envelope_contract_mismatches = (
        rbc_corruption_repair_envelope_conjunct_contract_errors()
    )
    rbc_chunk_ready_deliver_envelope_contract_mismatches = (
        rbc_chunk_ready_deliver_envelope_conjunct_contract_errors()
    )
    rbc_progress_mutation_envelope_contract_mismatches = (
        rbc_progress_mutation_envelope_conjunct_contract_errors()
    )
    rbc_progress_local_classification_envelope_contract_mismatches = (
        rbc_progress_local_classification_envelope_conjunct_contract_errors()
    )
    rbc_startup_boundary_envelope_contract_mismatches = (
        rbc_startup_boundary_envelope_conjunct_contract_errors()
    )
    rbc_progress_state_evidence_envelope_contract_mismatches = (
        rbc_progress_state_evidence_envelope_conjunct_contract_errors()
    )
    rbc_live_evidence_causality_envelope_contract_mismatches = (
        rbc_live_evidence_causality_envelope_conjunct_contract_errors()
    )
    byzantine_top_contract_mismatches = byzantine_top_conjunct_contract_errors()
    byzantine_top_cfg_mismatches = byzantine_top_cfg_errors()
    byzantine_top_corridor_contract_mismatches = (
        byzantine_top_corridor_contract_alignment_errors()
    )
    byzantine_top_projection_contract_mismatches = (
        byzantine_top_projection_contract_alignment_errors()
    )
    projection_bridge_interleaving_contract_mismatches = (
        projection_bridge_interleaving_contract_alignment_errors()
    )
    source_progress_safety_contract_mismatches = (
        source_progress_safety_contract_alignment_errors()
    )
    byzantine_interleaving_exactness_alignment_mismatches = (
        byzantine_interleaving_exactness_alignment_errors()
    )
    projection_bridge_core_source_alignment_mismatches = (
        projection_bridge_core_source_alignment_errors()
    )
    projected_commit_progress_contract_mismatches = (
        projected_commit_progress_contract_alignment_errors()
    )
    projected_commit_progress_spec_contract_mismatches = (
        projected_commit_progress_spec_contract_errors()
    )
    source_commit_progress_spec_contract_mismatches = (
        source_commit_progress_spec_contract_errors()
    )
    top_level_commit_spec_contract_mismatches = (
        top_level_commit_spec_contract_errors()
    )
    direct_delivered_first_contract_mismatches = (
        direct_delivered_first_gate_conjunct_contract_errors()
    )
    direct_delivered_first_clean_cfg_mismatches = (
        direct_delivered_first_clean_cfg_errors()
    )
    direct_vote_first_contract_mismatches = (
        direct_vote_first_gate_conjunct_contract_errors()
    )
    direct_vote_first_clean_cfg_mismatches = direct_vote_first_clean_cfg_errors()
    direct_interleaving_contract_mismatches = (
        direct_interleaving_gate_conjunct_contract_errors()
    )
    direct_interleaving_clean_cfg_mismatches = (
        direct_interleaving_clean_cfg_errors()
    )
    byzantine_interleaving_contract_mismatches = (
        byzantine_interleaving_gate_conjunct_contract_errors()
    )
    byzantine_interleaving_clean_cfg_mismatches = (
        byzantine_interleaving_clean_cfg_errors()
    )
    projection_gate_contract_mismatches = projection_gate_conjunct_contract_errors()
    projection_clean_cfg_mismatches = projection_clean_cfg_errors()
    consensus_core_root_cfg_check_contract_mismatches = (
        consensus_core_root_cfg_check_contract_errors()
    )
    temporal_shape_contract_mismatches = sumeragi_temporal_shape_contract_errors()
    apalache_typecheck_default_mismatches = apalache_typecheck_default_errors()
    workflow_entrypoint_mismatches = workflow_entrypoint_errors()
    command_shape_mismatches: list[str] = []
    for path in (FAST_CI, EXPECTED_FAILURE_CI, NIGHTLY_WORKFLOW, README):
        command_shape_mismatches.extend(
            command_shape_errors(path, APALACHE_COMMAND_PREFIX, "Apalache command")
        )
    command_shape_mismatches.extend(
        command_shape_errors(README, TLC_COMMAND_PREFIX, "TLC command")
    )
    fast_ci_modes = command_modes(FAST_CI, APALACHE_COMMAND_RE)
    expected_failure_modes = command_modes(EXPECTED_FAILURE_CI, APALACHE_COMMAND_RE)
    nightly_ci_modes = command_modes(NIGHTLY_WORKFLOW, APALACHE_COMMAND_RE)
    ci_modes = fast_ci_modes + expected_failure_modes + nightly_ci_modes
    readme_modes = command_modes(README, APALACHE_COMMAND_RE)
    readme_tlc_modes = command_modes(README, TLC_COMMAND_RE)
    readme_tlc_bug_modes = bug_modes(readme_tlc_modes)
    readme_fast_table_modes = documented_fast_table_modes(README)
    readme_apalache_length_rows = documented_apalache_length_rows(README)
    readme_apalache_length_shape_mismatches = apalache_length_table_shape_errors(
        README
    )
    readme_apalache_length_modes = [
        mode for mode, _ in readme_apalache_length_rows
    ]
    readme_apalache_lengths = dict(readme_apalache_length_rows)
    duplicate_fast_ci_modes = duplicate_values(fast_ci_modes)
    duplicate_expected_failure_ci_modes = duplicate_values(expected_failure_modes)
    duplicate_nightly_ci_modes = duplicate_values(nightly_ci_modes)
    duplicate_readme_apalache_commands = duplicate_values(readme_modes)
    duplicate_readme_apalache_length_modes = duplicate_values(
        readme_apalache_length_modes
    )
    overlapping_ci_modes = sorted_unique(
        set(fast_ci_modes) & set(expected_failure_modes)
    )
    readme_bug_modes = bug_modes(readme_modes)
    expected_failure_ci_set = set(expected_failure_modes)
    documented_bug_modes_missing_expected_failure_ci = sorted_unique(
        readme_bug_modes - expected_failure_ci_set
    )
    fast_ci_bug_modes = sorted_unique(bug_modes(fast_ci_modes))
    expected_failure_ci_non_bug_modes = sorted_unique(
        expected_failure_ci_set - bug_modes(expected_failure_modes)
    )

    all_documented_modes = set(readme_modes)
    all_checked_modes = set(ci_modes)
    all_modes_to_resolve = sorted_unique(all_checked_modes | all_documented_modes)

    unsupported_ci_modes: list[str] = []
    unsupported_readme_modes: list[str] = []
    missing_files: list[str] = []
    reference_errors: list[str] = []
    tlc_reference_errors: list[str] = []
    referenced_formal_files: set[Path] = set()

    for mode in all_modes_to_resolve:
        case = matching_case(mode, apalache_cases)
        if case is None:
            if mode in all_checked_modes:
                unsupported_ci_modes.append(mode)
            if mode in all_documented_modes:
                unsupported_readme_modes.append(mode)
            continue

        files, mode_reference_errors = referenced_files(mode, case)
        referenced_formal_files.update(
            path for path in files if path.suffix in FORMAL_FILE_SUFFIXES
        )
        reference_errors.extend(mode_reference_errors)
        reference_errors.extend(apalache_length_errors(mode, case))
        for spec_file in [path for path in files if path.suffix == ".tla"]:
            reachable_modules = tla_reachable_module_files(spec_file)
            referenced_formal_files.update(reachable_modules)
            reference_errors.extend(tla_module_validation_errors(mode, spec_file))
        reference_errors.extend(cfg_shape_errors(mode, files))
        spec_files = [path for path in files if path.suffix == ".tla"]
        cfg_files = [path for path in files if path.suffix == ".cfg"]
        for cfg_file in cfg_files:
            reference_errors.extend(
                cfg_duplicate_constant_binding_errors(mode, cfg_file)
            )
            reference_errors.extend(
                cfg_duplicate_operator_reference_errors(mode, cfg_file)
            )
            reference_errors.extend(
                cfg_semantic_check_errors(mode, cfg_file, "Apalache")
            )
            reference_errors.extend(
                cfg_fast_generic_check_errors(mode, cfg_file, "Apalache")
            )
        if len(spec_files) == 1:
            for cfg_file in cfg_files:
                reference_errors.extend(
                    cfg_module_ownership_errors(mode, spec_files[0], cfg_file)
                )
                reference_errors.extend(
                    cfg_operator_reference_errors(mode, spec_files[0], cfg_file)
                )
                reference_errors.extend(
                    cfg_trivial_check_operator_errors(
                        mode, spec_files[0], cfg_file, "Apalache"
                    )
                )
                reference_errors.extend(
                    cfg_correctness_envelope_shape_errors(
                        mode, spec_files[0], cfg_file, "Apalache"
                    )
                )
                reference_errors.extend(
                    cfg_direct_exactness_shape_errors(
                        mode, spec_files[0], cfg_file, "Apalache"
                    )
                )
                reference_errors.extend(
                    cfg_direct_exactness_envelope_pairing_errors(
                        mode, spec_files[0], cfg_file, "Apalache"
                    )
                )
                reference_errors.extend(
                    cfg_constant_binding_errors(mode, spec_files[0], cfg_file)
                )
        for path in files:
            if not path.exists():
                missing_files.append(f"{mode}: {path.relative_to(ROOT_DIR)}")

    expected_failure_without_marker = modes_without_expected_failure_marker(
        expected_failure_modes, apalache_cases, "Apalache"
    )
    baseline_with_expected_failure_marker = modes_with_unexpected_failure_marker(
        set(fast_ci_modes) | set(nightly_ci_modes), apalache_cases, "Apalache"
    )
    expected_failure_assignment_mismatches = expected_failure_assignment_errors(
        all_modes_to_resolve,
        apalache_cases,
        "Apalache",
    )

    missing_readme_commands = sorted_unique(all_checked_modes - all_documented_modes)
    exact_runner_modes = {label for label in apalache_cases if "*" not in label}
    pr_baseline_modes = {
        mode
        for mode in exact_runner_modes
        if mode in {"fast", "deep", "fork-npos"} or mode.endswith("-fast")
    }
    fast_ci_set = set(fast_ci_modes)
    missing_fast_ci_modes = sorted_unique(pr_baseline_modes - fast_ci_set)
    readme_apalache_length_set = set(readme_apalache_length_modes)
    missing_readme_apalache_length_modes = sorted_unique(
        pr_baseline_modes - readme_apalache_length_set
    )
    unsupported_readme_apalache_length_modes = sorted_unique(
        mode
        for mode in readme_apalache_length_set
        if matching_case(mode, apalache_cases) is None
    )
    apalache_length_mismatches = apalache_length_table_errors(
        readme_apalache_lengths, apalache_cases
    )
    missing_exact_runner_ci_modes = sorted_unique(
        exact_runner_modes - set(ci_modes)
    )
    unused_apalache_runner_cases = unused_runner_case_labels(
        all_modes_to_resolve, apalache_cases
    )
    readme_fast_table_set = set(readme_fast_table_modes)
    readme_tlc_set = set(readme_tlc_modes)
    pr_modes_with_tlc_runner = {
        mode
        for mode in pr_baseline_modes | APALACHE_ONLY_PR_MODES
        if matching_case(mode, tlc_cases) is not None
    }
    pr_tlc_cross_check_mismatches = pr_tlc_cross_check_errors(
        pr_baseline_modes,
        pr_modes_with_tlc_runner,
        readme_tlc_set,
    )
    apalache_only_readme_mismatches = required_text_errors(
        README,
        APALACHE_ONLY_PR_MODE_README_SNIPPETS,
        "Sumeragi formal README",
    )
    apalache_typecheck_only_mismatches = apalache_typecheck_only_mode_errors(
        set(all_modes_to_resolve) | exact_runner_modes,
        apalache_cases,
    )
    apalache_typecheck_only_readme_mismatches = required_text_errors(
        README,
        APALACHE_TYPECHECK_ONLY_README_SNIPPETS,
        "Sumeragi formal README",
    )
    formal_readme_guard_contract_mismatches = required_text_errors(
        README,
        FORMAL_README_GUARD_CONTRACT_SNIPPETS,
        "Sumeragi formal README",
    )
    tlc_modes_to_resolve = readme_fast_table_set | readme_tlc_set | readme_bug_modes
    missing_tlc_runner_modes = sorted_unique(
        mode
        for mode in readme_fast_table_set
        if mode not in APALACHE_ONLY_PR_MODES
        and matching_case(mode, tlc_cases) is None
    )
    missing_readme_tlc_commands = sorted_unique(
        readme_fast_table_set - readme_tlc_set - APALACHE_ONLY_PR_MODES
    )
    exact_tlc_fast_modes = exact_fast_runner_modes(tlc_cases)
    undocumented_tlc_runner_modes = sorted_unique(
        exact_tlc_fast_modes - readme_fast_table_set
    )
    documented_tlc_bug_modes = readme_bug_modes | readme_tlc_bug_modes
    documented_bug_modes_missing_tlc_runner = sorted_unique(
        mode
        for mode in documented_tlc_bug_modes
        if matching_case(mode, tlc_cases) is None
    )
    tlc_expected_failure_without_marker = modes_without_expected_failure_marker(
        documented_tlc_bug_modes, tlc_cases, "TLC"
    )
    tlc_non_bug_modes = tlc_modes_to_resolve - documented_tlc_bug_modes
    tlc_baseline_with_expected_failure_marker = modes_with_unexpected_failure_marker(
        tlc_non_bug_modes, tlc_cases, "TLC"
    )
    tlc_expected_failure_assignment_mismatches = expected_failure_assignment_errors(
        tlc_modes_to_resolve,
        tlc_cases,
        "TLC",
    )
    mutation_cfg_mismatches = mutation_cfg_equivalence_errors(
        readme_bug_modes, apalache_cases, tlc_cases
    )
    mutation_cfg_name_mismatches = mutation_cfg_name_errors(
        readme_bug_modes, apalache_cases, "Apalache"
    ) + mutation_cfg_name_errors(readme_bug_modes, tlc_cases, "TLC")
    source_safety_mutation_cfg_mismatches = source_safety_mutation_cfg_errors()
    projection_mutation_bridge_cfg_mismatches = (
        projection_mutation_bridge_cfg_errors()
    )
    direct_delivered_first_progress_mutation_cfg_mismatches = (
        direct_delivered_first_progress_mutation_cfg_errors()
    )
    direct_vote_first_progress_mutation_cfg_mismatches = (
        direct_vote_first_progress_mutation_cfg_errors()
    )
    direct_interleaving_progress_mutation_cfg_mismatches = (
        direct_interleaving_progress_mutation_cfg_errors()
    )
    projection_progress_mutation_cfg_mismatches = (
        projection_progress_mutation_cfg_errors()
    )
    byzantine_interleaving_progress_mutation_cfg_mismatches = (
        byzantine_interleaving_progress_mutation_cfg_errors()
    )
    direct_mutation_family_alignment_mismatches = (
        direct_mutation_family_alignment_errors()
    )
    byzantine_mutation_family_alignment_mismatches = (
        byzantine_mutation_family_alignment_errors()
    )
    module_identity_mismatches = module_identity_errors(
        tlc_modes_to_resolve, apalache_cases, tlc_cases
    )
    unused_tlc_runner_cases = unused_runner_case_labels(
        tlc_modes_to_resolve, tlc_cases
    )
    duplicate_readme_fast_table_modes = duplicate_values(readme_fast_table_modes)
    duplicate_readme_tlc_commands = duplicate_values(readme_tlc_modes)
    unsupported_readme_tlc_commands = sorted_unique(
        mode for mode in readme_tlc_set if matching_case(mode, tlc_cases) is None
    )
    missing_tlc_files: list[str] = []
    for mode in sorted_unique(tlc_modes_to_resolve):
        case = matching_case(mode, tlc_cases)
        if case is None:
            continue
        files, mode_reference_errors = referenced_files(
            mode, case, required_variables=("cfg_file",)
        )
        module_files, module_reference_errors = tlc_module_files(mode, case)
        files.extend(module_files)
        referenced_formal_files.update(
            path for path in files if path.suffix in FORMAL_FILE_SUFFIXES
        )
        tlc_reference_errors.extend(mode_reference_errors)
        tlc_reference_errors.extend(module_reference_errors)
        for module_file in module_files:
            reachable_modules = tla_reachable_module_files(module_file)
            referenced_formal_files.update(reachable_modules)
            tlc_reference_errors.extend(
                tla_module_validation_errors(mode, module_file)
            )
        tlc_reference_errors.extend(cfg_shape_errors(mode, files))
        for cfg_file in files:
            if cfg_file.suffix == ".cfg":
                tlc_reference_errors.extend(
                    cfg_duplicate_constant_binding_errors(mode, cfg_file)
                )
                tlc_reference_errors.extend(
                    cfg_duplicate_operator_reference_errors(mode, cfg_file)
                )
                tlc_reference_errors.extend(
                    cfg_semantic_check_errors(mode, cfg_file, "TLC")
                )
                tlc_reference_errors.extend(
                    cfg_fast_generic_check_errors(mode, cfg_file, "TLC")
                )
        if len(module_files) == 1:
            tlc_reference_errors.extend(
                tlc_runner_constraint_errors(mode, case, module_files[0])
            )
            for cfg_file in files:
                if cfg_file.suffix == ".cfg":
                    tlc_reference_errors.extend(
                        cfg_module_ownership_errors(mode, module_files[0], cfg_file)
                    )
                    tlc_reference_errors.extend(
                        cfg_operator_reference_errors(mode, module_files[0], cfg_file)
                    )
                    tlc_reference_errors.extend(
                        cfg_trivial_check_operator_errors(
                            mode, module_files[0], cfg_file, "TLC"
                        )
                    )
                    tlc_reference_errors.extend(
                        cfg_correctness_envelope_shape_errors(
                            mode, module_files[0], cfg_file, "TLC"
                        )
                    )
                    tlc_reference_errors.extend(
                        cfg_direct_exactness_shape_errors(
                            mode, module_files[0], cfg_file, "TLC"
                        )
                    )
                    tlc_reference_errors.extend(
                        cfg_direct_exactness_envelope_pairing_errors(
                            mode, module_files[0], cfg_file, "TLC"
                        )
                    )
                    tlc_reference_errors.extend(
                        cfg_constant_binding_errors(mode, module_files[0], cfg_file)
                    )
        for path in files:
            if not path.exists():
                missing_tlc_files.append(f"{mode}: {path.relative_to(ROOT_DIR)}")

    unreferenced_formal_files = unreferenced_formal_file_errors(
        referenced_formal_files
    )

    if unsupported_ci_modes:
        errors.append(
            "CI invokes Sumeragi formal modes unsupported by the runner:\n"
            + format_items(sorted_unique(unsupported_ci_modes))
        )
    if unsupported_readme_modes:
        errors.append(
            "README documents Sumeragi formal modes unsupported by the runner:\n"
            + format_items(sorted_unique(unsupported_readme_modes))
        )
    if missing_readme_commands:
        errors.append(
            "README is missing commands for CI-invoked Sumeragi formal modes:\n"
            + format_items(missing_readme_commands)
        )
    if missing_fast_ci_modes:
        errors.append(
            "PR CI is missing exact fast Sumeragi formal runner modes:\n"
            + format_items(missing_fast_ci_modes)
        )
    if readme_apalache_length_shape_mismatches:
        errors.append(
            "README Apalache length table is malformed:\n"
            + format_items(readme_apalache_length_shape_mismatches)
        )
    if missing_readme_apalache_length_modes:
        errors.append(
            "README Apalache length table is missing PR baseline modes:\n"
            + format_items(missing_readme_apalache_length_modes)
        )
    if unsupported_readme_apalache_length_modes:
        errors.append(
            "README Apalache length table documents modes unsupported by the runner:\n"
            + format_items(unsupported_readme_apalache_length_modes)
        )
    if apalache_length_mismatches:
        errors.append(
            "README Apalache length table disagrees with the runner:\n"
            + format_items(apalache_length_mismatches)
        )
    if missing_exact_runner_ci_modes:
        errors.append(
            "Exact Apalache runner modes are missing from formal CI:\n"
            + format_items(missing_exact_runner_ci_modes)
        )
    if unused_apalache_runner_cases:
        errors.append(
            "Apalache runner has case branches unused by CI or README modes:\n"
            + format_items(unused_apalache_runner_cases)
        )
    if duplicate_fast_ci_modes:
        errors.append(
            "PR CI has duplicate Sumeragi formal modes:\n"
            + format_items(duplicate_fast_ci_modes)
        )
    if duplicate_expected_failure_ci_modes:
        errors.append(
            "Expected-failure CI has duplicate Sumeragi formal modes:\n"
            + format_items(duplicate_expected_failure_ci_modes)
        )
    if duplicate_nightly_ci_modes:
        errors.append(
            "Scheduled/manual CI has duplicate Sumeragi formal modes:\n"
            + format_items(duplicate_nightly_ci_modes)
        )
    if overlapping_ci_modes:
        errors.append(
            "Sumeragi formal modes appear in both PR and expected-failure CI:\n"
            + format_items(overlapping_ci_modes)
        )
    if documented_bug_modes_missing_expected_failure_ci:
        errors.append(
            "README documents Sumeragi mutation modes missing from expected-failure CI:\n"
            + format_items(documented_bug_modes_missing_expected_failure_ci)
        )
    if fast_ci_bug_modes:
        errors.append(
            "PR CI includes Sumeragi mutation modes that belong in expected-failure CI:\n"
            + format_items(fast_ci_bug_modes)
        )
    if expected_failure_ci_non_bug_modes:
        errors.append(
            "Expected-failure CI includes non-mutation Sumeragi formal modes:\n"
            + format_items(expected_failure_ci_non_bug_modes)
        )
    if duplicate_readme_apalache_commands:
        errors.append(
            "README has duplicate Apalache commands for modes:\n"
            + format_items(duplicate_readme_apalache_commands)
        )
    if duplicate_readme_apalache_length_modes:
        errors.append(
            "README Apalache length table has duplicate modes:\n"
            + format_items(duplicate_readme_apalache_length_modes)
        )
    if expected_failure_without_marker:
        errors.append(
            "Expected-failure CI modes are not marked expect_failure=1 in the runner:\n"
            + format_items(expected_failure_without_marker)
        )
    if expected_failure_assignment_mismatches:
        errors.append(
            "Apalache runner expected-failure assignments are malformed:\n"
            + format_items(expected_failure_assignment_mismatches)
        )
    if baseline_with_expected_failure_marker:
        errors.append(
            "PR or scheduled/manual Sumeragi formal modes are marked "
            "expect_failure=1 in the runner:\n"
            + format_items(baseline_with_expected_failure_marker)
        )
    if duplicate_apalache_case_labels:
        errors.append(
            "Apalache runner has duplicate case labels:\n"
            + format_items(duplicate_apalache_case_labels)
        )
    if duplicate_tlc_case_labels:
        errors.append(
            "TLC runner has duplicate case labels:\n"
            + format_items(duplicate_tlc_case_labels)
        )
    if runner_case_shape_mismatches:
        errors.append(
            "Sumeragi formal runner case blocks are malformed:\n"
            + format_items(runner_case_shape_mismatches)
        )
    if shadowed_apalache_case_labels:
        errors.append(
            "Apalache runner has case labels shadowed by earlier wildcards:\n"
            + format_items(shadowed_apalache_case_labels)
        )
    if shadowed_tlc_case_labels:
        errors.append(
            "TLC runner has case labels shadowed by earlier wildcards:\n"
            + format_items(shadowed_tlc_case_labels)
        )
    if workflow_entrypoint_mismatches:
        errors.append(
            "Sumeragi formal workflow entrypoints are not wired to the guard:\n"
            + format_items(workflow_entrypoint_mismatches)
        )
    if command_shape_mismatches:
        errors.append(
            "Sumeragi formal command lines are malformed:\n"
            + format_items(command_shape_mismatches)
        )
    if apalache_version_mismatches:
        errors.append(
            "Sumeragi formal Apalache version pins disagree:\n"
            + format_items(apalache_version_mismatches)
        )
    if expected_failure_semantics_mismatches:
        errors.append(
            "Sumeragi formal expected-failure runner semantics are weak:\n"
            + format_items(expected_failure_semantics_mismatches)
        )
    if expected_failure_default_mismatches:
        errors.append(
            "Sumeragi formal expected-failure defaults are miswired:\n"
            + format_items(expected_failure_default_mismatches)
        )
    if runner_invocation_mismatches:
        errors.append(
            "Sumeragi formal runner invocations do not bind selected proof inputs:\n"
            + format_items(runner_invocation_mismatches)
        )
    if top_level_cfg_behavior_mismatches:
        errors.append(
            "Sumeragi top-level CFG behavior bindings drifted:\n"
            + format_items(top_level_cfg_behavior_mismatches)
        )
    if top_level_cfg_deadlock_mismatches:
        errors.append(
            "Sumeragi top-level CFG deadlock policy drifted:\n"
            + format_items(top_level_cfg_deadlock_mismatches)
        )
    if top_level_cfg_constant_mismatches:
        errors.append(
            "Sumeragi top-level CFG constants drifted:\n"
            + format_items(top_level_cfg_constant_mismatches)
        )
    if top_level_fast_cfg_check_mismatches:
        errors.append(
            "Sumeragi fast CFG sentinel checks drifted:\n"
            + format_items(top_level_fast_cfg_check_mismatches)
        )
    if top_level_cfg_check_parity_mismatches:
        errors.append(
            "Sumeragi top-level Apalache/TLC CFG proof checks diverge:\n"
            + format_items(top_level_cfg_check_parity_mismatches)
        )
    if property_root_reachability_mismatches:
        errors.append(
            "Sumeragi top-level proof property graph is disconnected:\n"
            + format_items(property_root_reachability_mismatches)
        )
    if state_invariant_root_reachability_mismatches:
        errors.append(
            "Sumeragi top-level state invariant graph is disconnected:\n"
            + format_items(state_invariant_root_reachability_mismatches)
        )
    if state_invariant_root_cfg_coverage_mismatches:
        errors.append(
            "Sumeragi state invariant root has untracked direct conjuncts:\n"
            + format_items(state_invariant_root_cfg_coverage_mismatches)
        )
    if temporal_property_root_cfg_coverage_mismatches:
        errors.append(
            "Sumeragi temporal property roots have untracked direct conjuncts:\n"
            + format_items(temporal_property_root_cfg_coverage_mismatches)
        )
    if consensus_core_root_contract_mismatches:
        errors.append(
            "Sumeragi consensus-core aggregate proof roots drifted:\n"
            + format_items(consensus_core_root_contract_mismatches)
        )
    if state_matches_envelope_contract_mismatches:
        errors.append(
            "Sumeragi state-safety proof envelope drifted:\n"
            + format_items(state_matches_envelope_contract_mismatches)
        )
    if committed_terminal_envelope_contract_mismatches:
        errors.append(
            "Sumeragi committed terminal-state proof envelope drifted:\n"
            + format_items(committed_terminal_envelope_contract_mismatches)
        )
    if end_to_end_safety_envelope_contract_mismatches:
        errors.append(
            "Sumeragi end-to-end safety proof envelope drifted:\n"
            + format_items(end_to_end_safety_envelope_contract_mismatches)
        )
    if post_finality_stability_envelope_contract_mismatches:
        errors.append(
            "Sumeragi post-finality stability proof envelope drifted:\n"
            + format_items(post_finality_stability_envelope_contract_mismatches)
        )
    if timeout_recovery_envelope_contract_mismatches:
        errors.append(
            "Sumeragi timeout-recovery proof envelope drifted:\n"
            + format_items(timeout_recovery_envelope_contract_mismatches)
        )
    if finality_installation_envelope_contract_mismatches:
        errors.append(
            "Sumeragi certified-commit installation proof envelope drifted:\n"
            + format_items(finality_installation_envelope_contract_mismatches)
        )
    if pre_commit_handoff_envelope_contract_mismatches:
        errors.append(
            "Sumeragi pre-commit handoff proof envelope drifted:\n"
            + format_items(pre_commit_handoff_envelope_contract_mismatches)
        )
    if commit_vote_handoff_envelope_contract_mismatches:
        errors.append(
            "Sumeragi commit-vote handoff proof envelope drifted:\n"
            + format_items(commit_vote_handoff_envelope_contract_mismatches)
        )
    if finalized_certificate_retention_envelope_contract_mismatches:
        errors.append(
            "Sumeragi finalized certificate retention proof envelope drifted:\n"
            + format_items(
                finalized_certificate_retention_envelope_contract_mismatches
            )
        )
    if rbc_delivered_finality_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC delivered-finality proof envelope drifted:\n"
            + format_items(rbc_delivered_finality_envelope_contract_mismatches)
        )
    if rbc_delivered_state_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC delivered-state lifecycle proof envelope drifted:\n"
            + format_items(rbc_delivered_state_envelope_contract_mismatches)
        )
    if rbc_delivered_pending_handoff_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC delivered-pending handoff proof envelope drifted:\n"
            + format_items(
                rbc_delivered_pending_handoff_envelope_contract_mismatches
            )
        )
    if delivered_pending_complete_wait_state_envelope_contract_mismatches:
        errors.append(
            "Sumeragi delivered-pending complete wait-state proof envelope "
            "drifted:\n"
            + format_items(
                delivered_pending_complete_wait_state_envelope_contract_mismatches
            )
        )
    if rbc_delivery_entry_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC delivery-entry outcome proof envelope drifted:\n"
            + format_items(rbc_delivery_entry_envelope_contract_mismatches)
        )
    if rbc_delivery_entry_continuation_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC delivery-entry continuation proof envelope drifted:\n"
            + format_items(rbc_delivery_entry_continuation_envelope_contract_mismatches)
        )
    if rbc_lifecycle_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC lifecycle proof envelope drifted:\n"
            + format_items(rbc_lifecycle_envelope_contract_mismatches)
        )
    if rbc_corruption_repair_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC corruption-repair proof envelope drifted:\n"
            + format_items(rbc_corruption_repair_envelope_contract_mismatches)
        )
    if rbc_chunk_ready_deliver_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC chunk/ready/deliver availability proof envelope "
            "drifted:\n"
            + format_items(rbc_chunk_ready_deliver_envelope_contract_mismatches)
        )
    if rbc_progress_mutation_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC progress-mutation proof envelope drifted:\n"
            + format_items(rbc_progress_mutation_envelope_contract_mismatches)
        )
    if rbc_progress_local_classification_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC progress local-classification proof envelope drifted:\n"
            + format_items(
                rbc_progress_local_classification_envelope_contract_mismatches
            )
        )
    if rbc_startup_boundary_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC startup-boundary proof envelope drifted:\n"
            + format_items(rbc_startup_boundary_envelope_contract_mismatches)
        )
    if rbc_progress_state_evidence_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC progress-state evidence proof envelope drifted:\n"
            + format_items(
                rbc_progress_state_evidence_envelope_contract_mismatches
            )
        )
    if rbc_live_evidence_causality_envelope_contract_mismatches:
        errors.append(
            "Sumeragi RBC live-evidence causality proof envelope drifted:\n"
            + format_items(
                rbc_live_evidence_causality_envelope_contract_mismatches
            )
        )
    if byzantine_top_contract_mismatches:
        errors.append(
            "Sumeragi Byzantine top-level proof aggregates drifted:\n"
            + format_items(byzantine_top_contract_mismatches)
        )
    if byzantine_top_cfg_mismatches:
        errors.append(
            "Sumeragi Byzantine top-level cfg behavior/checks drifted:\n"
            + format_items(byzantine_top_cfg_mismatches)
        )
    if byzantine_top_corridor_contract_mismatches:
        errors.append(
            "Sumeragi Byzantine top-level corridor contract alignment "
            "drifted:\n"
            + format_items(byzantine_top_corridor_contract_mismatches)
        )
    if byzantine_top_projection_contract_mismatches:
        errors.append(
            "Sumeragi Byzantine top/projection contract alignment drifted:\n"
            + format_items(byzantine_top_projection_contract_mismatches)
        )
    if projection_bridge_interleaving_contract_mismatches:
        errors.append(
            "Sumeragi projection bridge interleaving contract alignment "
            "drifted:\n"
            + format_items(projection_bridge_interleaving_contract_mismatches)
        )
    if source_progress_safety_contract_mismatches:
        errors.append(
            "Sumeragi source progress safety envelope contract alignment "
            "drifted:\n"
            + format_items(source_progress_safety_contract_mismatches)
        )
    if byzantine_interleaving_exactness_alignment_mismatches:
        errors.append(
            "Sumeragi Byzantine interleaving exactness alignment drifted:\n"
            + format_items(byzantine_interleaving_exactness_alignment_mismatches)
        )
    if projection_bridge_core_source_alignment_mismatches:
        errors.append(
            "Sumeragi projection bridge core/source alignment drifted:\n"
            + format_items(projection_bridge_core_source_alignment_mismatches)
        )
    if projected_commit_progress_contract_mismatches:
        errors.append(
            "Sumeragi projected commit progress safety contract alignment "
            "drifted:\n"
            + format_items(projected_commit_progress_contract_mismatches)
        )
    if projected_commit_progress_spec_contract_mismatches:
        errors.append(
            "Sumeragi projected commit progress spec/fairness contract "
            "drifted:\n"
            + format_items(projected_commit_progress_spec_contract_mismatches)
        )
    if source_commit_progress_spec_contract_mismatches:
        errors.append(
            "Sumeragi source commit progress spec/fairness contracts "
            "drifted:\n"
            + format_items(source_commit_progress_spec_contract_mismatches)
        )
    if top_level_commit_spec_contract_mismatches:
        errors.append(
            "Sumeragi top-level commit spec/fairness contracts drifted:\n"
            + format_items(top_level_commit_spec_contract_mismatches)
        )
    if direct_delivered_first_contract_mismatches:
        errors.append(
            "Sumeragi direct delivered-first progress safety aggregate "
            "drifted:\n"
            + format_items(direct_delivered_first_contract_mismatches)
        )
    if direct_delivered_first_clean_cfg_mismatches:
        errors.append(
            "Sumeragi direct delivered-first clean cfg behavior/checks drifted:\n"
            + format_items(direct_delivered_first_clean_cfg_mismatches)
        )
    if direct_vote_first_contract_mismatches:
        errors.append(
            "Sumeragi direct vote-first progress safety aggregate drifted:\n"
            + format_items(direct_vote_first_contract_mismatches)
        )
    if direct_vote_first_clean_cfg_mismatches:
        errors.append(
            "Sumeragi direct vote-first clean cfg behavior/checks drifted:\n"
            + format_items(direct_vote_first_clean_cfg_mismatches)
        )
    if direct_interleaving_contract_mismatches:
        errors.append(
            "Sumeragi direct interleaving progress safety aggregate "
            "drifted:\n"
            + format_items(direct_interleaving_contract_mismatches)
        )
    if direct_interleaving_clean_cfg_mismatches:
        errors.append(
            "Sumeragi direct interleaving clean cfg behavior/checks drifted:\n"
            + format_items(direct_interleaving_clean_cfg_mismatches)
        )
    if byzantine_interleaving_contract_mismatches:
        errors.append(
            "Sumeragi Byzantine interleaving progress safety aggregate "
            "drifted:\n"
            + format_items(byzantine_interleaving_contract_mismatches)
        )
    if byzantine_interleaving_clean_cfg_mismatches:
        errors.append(
            "Sumeragi Byzantine interleaving clean cfg behavior/checks drifted:\n"
            + format_items(byzantine_interleaving_clean_cfg_mismatches)
        )
    if projection_gate_contract_mismatches:
        errors.append(
            "Sumeragi Byzantine projection gate proof aggregates drifted:\n"
            + format_items(projection_gate_contract_mismatches)
        )
    if projection_clean_cfg_mismatches:
        errors.append(
            "Sumeragi Byzantine projection clean cfg behavior/checks drifted:\n"
            + format_items(projection_clean_cfg_mismatches)
        )
    if consensus_core_root_cfg_check_contract_mismatches:
        errors.append(
            "Sumeragi consensus-core root CFG check roles drifted:\n"
            + format_items(consensus_core_root_cfg_check_contract_mismatches)
        )
    if temporal_shape_contract_mismatches:
        errors.append(
            "Sumeragi temporal theorem shape contracts drifted:\n"
            + format_items(temporal_shape_contract_mismatches)
        )
    if apalache_typecheck_default_mismatches:
        errors.append(
            "Sumeragi Apalache typecheck-only default is miswired:\n"
            + format_items(apalache_typecheck_default_mismatches)
        )
    errors.extend(pr_tlc_cross_check_mismatches)
    if apalache_only_readme_mismatches:
        errors.append(
            "Sumeragi formal README is missing Apalache-only PR mode documentation:\n"
            + format_items(apalache_only_readme_mismatches)
        )
    if apalache_typecheck_only_mismatches:
        errors.append(
            "Sumeragi Apalache typecheck-only modes are miswired:\n"
            + format_items(apalache_typecheck_only_mismatches)
        )
    if apalache_typecheck_only_readme_mismatches:
        errors.append(
            "Sumeragi formal README is missing Apalache typecheck-only "
            "documentation:\n"
            + format_items(apalache_typecheck_only_readme_mismatches)
        )
    if formal_readme_guard_contract_mismatches:
        errors.append(
            "Sumeragi formal README is missing formal guard contract "
            "documentation:\n"
            + format_items(formal_readme_guard_contract_mismatches)
        )
    if missing_tlc_runner_modes:
        errors.append(
            "README fast-mode table documents modes unsupported by the TLC runner:\n"
            + format_items(missing_tlc_runner_modes)
        )
    if missing_readme_tlc_commands:
        errors.append(
            "README is missing TLC commands for documented fast modes:\n"
            + format_items(missing_readme_tlc_commands)
        )
    if undocumented_tlc_runner_modes:
        errors.append(
            "TLC runner has exact fast modes missing from the README fast-mode table:\n"
            + format_items(undocumented_tlc_runner_modes)
        )
    if documented_bug_modes_missing_tlc_runner:
        errors.append(
            "README documents Sumeragi mutation modes unsupported by the TLC runner:\n"
            + format_items(documented_bug_modes_missing_tlc_runner)
        )
    if tlc_expected_failure_without_marker:
        errors.append(
            "README mutation modes are not marked expect_failure=1 in the TLC runner:\n"
            + format_items(tlc_expected_failure_without_marker)
        )
    if tlc_expected_failure_assignment_mismatches:
        errors.append(
            "TLC runner expected-failure assignments are malformed:\n"
            + format_items(tlc_expected_failure_assignment_mismatches)
        )
    if tlc_baseline_with_expected_failure_marker:
        errors.append(
            "README non-mutation TLC modes are marked expect_failure=1 "
            "in the runner:\n"
            + format_items(tlc_baseline_with_expected_failure_marker)
        )
    if mutation_cfg_mismatches:
        errors.append(
            "README mutation modes resolve to different Apalache/TLC cfg files:\n"
            + format_items(mutation_cfg_mismatches)
        )
    if mutation_cfg_name_mismatches:
        errors.append(
            "README mutation modes resolve to cfg files without matching "
            "mutation-name fragments:\n"
            + format_items(mutation_cfg_name_mismatches)
        )
    if source_safety_mutation_cfg_mismatches:
        errors.append(
            "Sumeragi source safety mutation cfgs must check the fast "
            "type/exactness surface:\n"
            + format_items(source_safety_mutation_cfg_mismatches)
        )
    if projection_mutation_bridge_cfg_mismatches:
        errors.append(
            "Sumeragi projection mutation cfgs must check the full bridge:\n"
            + format_items(projection_mutation_bridge_cfg_mismatches)
        )
    if direct_delivered_first_progress_mutation_cfg_mismatches:
        errors.append(
            "Sumeragi direct delivered-first progress mutation cfgs must "
            "bind progress behavior and check the safety/progress surface:\n"
            + format_items(direct_delivered_first_progress_mutation_cfg_mismatches)
        )
    if direct_vote_first_progress_mutation_cfg_mismatches:
        errors.append(
            "Sumeragi direct vote-first progress mutation cfgs must bind "
            "progress behavior and check the safety/progress surface:\n"
            + format_items(direct_vote_first_progress_mutation_cfg_mismatches)
        )
    if direct_interleaving_progress_mutation_cfg_mismatches:
        errors.append(
            "Sumeragi direct interleaving progress mutation cfgs must "
            "bind progress behavior and check the safety/progress surface:\n"
            + format_items(direct_interleaving_progress_mutation_cfg_mismatches)
        )
    if byzantine_interleaving_progress_mutation_cfg_mismatches:
        errors.append(
            "Sumeragi Byzantine interleaving progress mutation cfgs must "
            "bind progress behavior and check the safety/progress surface:\n"
            + format_items(byzantine_interleaving_progress_mutation_cfg_mismatches)
        )
    if projection_progress_mutation_cfg_mismatches:
        errors.append(
            "Sumeragi projection progress mutation cfgs must bind progress "
            "behavior and check the safety/progress surface:\n"
            + format_items(projection_progress_mutation_cfg_mismatches)
        )
    if direct_mutation_family_alignment_mismatches:
        errors.append(
            "Sumeragi direct mutation family coverage drifted:\n"
            + format_items(direct_mutation_family_alignment_mismatches)
        )
    if byzantine_mutation_family_alignment_mismatches:
        errors.append(
            "Sumeragi Byzantine mutation family coverage drifted:\n"
            + format_items(byzantine_mutation_family_alignment_mismatches)
        )
    if module_identity_mismatches:
        errors.append(
            "TLC modes resolve to different TLA modules than Apalache:\n"
            + format_items(module_identity_mismatches)
        )
    if unused_tlc_runner_cases:
        errors.append(
            "TLC runner has case branches unused by README modes:\n"
            + format_items(unused_tlc_runner_cases)
        )
    if duplicate_readme_fast_table_modes:
        errors.append(
            "README fast-mode table has duplicate modes:\n"
            + format_items(duplicate_readme_fast_table_modes)
        )
    if duplicate_readme_tlc_commands:
        errors.append(
            "README has duplicate TLC commands for modes:\n"
            + format_items(duplicate_readme_tlc_commands)
        )
    if unsupported_readme_tlc_commands:
        errors.append(
            "README documents TLC commands unsupported by the TLC runner:\n"
            + format_items(unsupported_readme_tlc_commands)
        )
    if reference_errors:
        errors.append(
            "Could not resolve runner spec/config references:\n"
            + format_items(reference_errors)
        )
    if tlc_reference_errors:
        errors.append(
            "Could not resolve TLC runner config references:\n"
            + format_items(tlc_reference_errors)
        )
    if missing_files:
        errors.append(
            "Runner modes reference missing Sumeragi formal files:\n"
            + format_items(sorted_unique(missing_files))
        )
    if missing_tlc_files:
        errors.append(
            "TLC runner modes reference missing Sumeragi formal config files:\n"
            + format_items(sorted_unique(missing_tlc_files))
        )
    if unreferenced_formal_files:
        errors.append(
            "Sumeragi formal TLA+/CFG files are not reached by checked or "
            "documented modes:\n"
            + format_items(unreferenced_formal_files)
        )

    if errors:
        print_error_sections(errors)
        return 1

    print(
        "[formal] Sumeragi coverage wiring is consistent "
        f"({len(set(fast_ci_modes))} PR modes, "
        f"{len(set(expected_failure_modes))} expected-failure modes, "
        f"{len(set(nightly_ci_modes))} scheduled/manual modes, "
        f"{len(set(readme_modes))} documented modes, "
        f"{len(readme_fast_table_set)} TLC fast modes, "
        f"{len(documented_tlc_bug_modes)} TLC mutation modes)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

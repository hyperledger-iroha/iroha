---- MODULE SumeragiRbcHotRepairGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC hot-repair and proposal-blocking helpers.

This slice captures the helper contracts around:
- `rbc_rebroadcast_active_with_tip_and_session(...)`;
- `suppress_rbc_hot_repair(...)`;
- `allow_exact_frontier_recovered_rbc_chunk_repair(...)`;
- `rbc_rebroadcast_session_urgent_near_tip(...)`;
- `rbc_payload_backpressure_exempt_with_tip(...)`; and
- `rbc_blocks_proposal_with_tip(...)`.

The model keeps the helper boundary finite while pinning the observable
consensus decisions: only active pending/inflight/processing or tip-extending
sessions stay hot, delivered committed sessions retire, exact-frontier
disk-recovered chunk repair requires an armed body gap, payload backpressure is
exempt only for active urgent sessions that can still recover payload bytes,
and proposal blocking remains tied to non-aborted active pending work.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ActivePendingExtends == "active_pending_extends"
ActivePendingAborted == "active_pending_aborted"
ActivePendingWrongSlot == "active_pending_wrong_slot"
ActiveInflightExtends == "active_inflight_extends"
ActiveProcessingHash == "active_processing_hash"
ActiveSessionAbsent == "active_session_absent"
ActiveSessionInvalid == "active_session_invalid"
ActiveSessionMissingHeader == "active_session_missing_header"
ActiveSessionHeaderMismatch == "active_session_header_mismatch"
ActiveDeliveredAtTip == "active_delivered_at_tip"
ActiveDeliveredBehindTip == "active_delivered_behind_tip"
ActiveSessionExtendsTip == "active_session_extends_tip"
ActiveSessionMatchesTip == "active_session_matches_tip"
ActiveSessionOffTip == "active_session_off_tip"

ActiveCases == {
  ActivePendingExtends,
  ActivePendingAborted,
  ActivePendingWrongSlot,
  ActiveInflightExtends,
  ActiveProcessingHash,
  ActiveSessionAbsent,
  ActiveSessionInvalid,
  ActiveSessionMissingHeader,
  ActiveSessionHeaderMismatch,
  ActiveDeliveredAtTip,
  ActiveDeliveredBehindTip,
  ActiveSessionExtendsTip,
  ActiveSessionMatchesTip,
  ActiveSessionOffTip
}

SpecActive(c) ==
  c \in {
    ActivePendingExtends,
    ActiveInflightExtends,
    ActiveProcessingHash,
    ActiveSessionExtendsTip,
    ActiveSessionMatchesTip
  }

ImplementationActive(c) ==
  CASE Bug = "active_accept_aborted_pending"
       /\ c = ActivePendingAborted -> TRUE
    [] Bug = "active_ignore_pending"
       /\ c = ActivePendingExtends -> FALSE
    [] Bug = "active_processing_requires_session"
       /\ c = ActiveProcessingHash -> FALSE
    [] Bug = "active_accept_invalid_session"
       /\ c = ActiveSessionInvalid -> TRUE
    [] Bug = "active_accept_missing_header"
       /\ c = ActiveSessionMissingHeader -> TRUE
    [] Bug = "active_accept_header_mismatch"
       /\ c = ActiveSessionHeaderMismatch -> TRUE
    [] Bug = "active_keep_delivered_at_tip"
       /\ c = ActiveDeliveredAtTip -> TRUE
    [] Bug = "active_keep_delivered_behind_tip"
       /\ c = ActiveDeliveredBehindTip -> TRUE
    [] Bug = "active_reject_extends_tip"
       /\ c = ActiveSessionExtendsTip -> FALSE
    [] Bug = "active_reject_matches_tip"
       /\ c = ActiveSessionMatchesTip -> FALSE
    [] OTHER -> SpecActive(c)

SuppressDeliveredCommitted == "suppress_delivered_committed"
SuppressLocalCandidate == "suppress_local_candidate"
SuppressLocalPayload == "suppress_local_payload"
SuppressNoLocalNear == "suppress_no_local_near"
SuppressNoLocalFar == "suppress_no_local_far"
SuppressDeliveredCommittedLocalCandidate ==
  "suppress_delivered_committed_local_candidate"

SuppressCases == {
  SuppressDeliveredCommitted,
  SuppressLocalCandidate,
  SuppressLocalPayload,
  SuppressNoLocalNear,
  SuppressNoLocalFar,
  SuppressDeliveredCommittedLocalCandidate
}

SpecSuppress(c) ==
  c \in {
    SuppressDeliveredCommitted,
    SuppressNoLocalNear,
    SuppressNoLocalFar,
    SuppressDeliveredCommittedLocalCandidate
  }

ImplementationSuppress(c) ==
  CASE Bug = "suppress_delivered_committed_not_suppressed"
       /\ c \in {SuppressDeliveredCommitted,
                 SuppressDeliveredCommittedLocalCandidate} -> FALSE
    [] Bug = "suppress_local_candidate"
       /\ c = SuppressLocalCandidate -> TRUE
    [] Bug = "suppress_local_payload"
       /\ c = SuppressLocalPayload -> TRUE
    [] Bug = "suppress_no_local_near_allows"
       /\ c = SuppressNoLocalNear -> FALSE
    [] OTHER -> SpecSuppress(c)

ExactRecoveredFrontier == "exact_recovered_frontier"
ExactNotRecovered == "exact_not_recovered"
ExactNoPayloadRecovery == "exact_no_payload_recovery"
ExactWrongHeight == "exact_wrong_height"
ExactSlotWrongHash == "exact_slot_wrong_hash"
ExactSlotWrongView == "exact_slot_wrong_view"
ExactSlotNotArmed == "exact_slot_not_armed"
ExactSlotBodyPresent == "exact_slot_body_present"

ExactCases == {
  ExactRecoveredFrontier,
  ExactNotRecovered,
  ExactNoPayloadRecovery,
  ExactWrongHeight,
  ExactSlotWrongHash,
  ExactSlotWrongView,
  ExactSlotNotArmed,
  ExactSlotBodyPresent
}

SpecExactRepair(c) ==
  c = ExactRecoveredFrontier

ImplementationExactRepair(c) ==
  CASE Bug = "exact_accept_not_recovered"
       /\ c = ExactNotRecovered -> TRUE
    [] Bug = "exact_accept_no_payload_recovery"
       /\ c = ExactNoPayloadRecovery -> TRUE
    [] Bug = "exact_accept_wrong_height"
       /\ c = ExactWrongHeight -> TRUE
    [] Bug = "exact_accept_wrong_slot"
       /\ c \in {ExactSlotWrongHash, ExactSlotWrongView} -> TRUE
    [] Bug = "exact_accept_not_armed"
       /\ c = ExactSlotNotArmed -> TRUE
    [] Bug = "exact_accept_body_present"
       /\ c = ExactSlotBodyPresent -> TRUE
    [] Bug = "exact_reject_valid"
       /\ c = ExactRecoveredFrontier -> FALSE
    [] OTHER -> SpecExactRepair(c)

UrgentPendingNear == "urgent_pending_near"
UrgentPendingFar == "urgent_pending_far"
UrgentDeliveredNear == "urgent_delivered_near"
UrgentMissingChunksNear == "urgent_missing_chunks_near"
UrgentNonAuthoritativeNear == "urgent_non_authoritative_near"
UrgentAuthoritativeNear == "urgent_authoritative_near"
UrgentMissingChunksFar == "urgent_missing_chunks_far"

UrgentCases == {
  UrgentPendingNear,
  UrgentPendingFar,
  UrgentDeliveredNear,
  UrgentMissingChunksNear,
  UrgentNonAuthoritativeNear,
  UrgentAuthoritativeNear,
  UrgentMissingChunksFar
}

SpecUrgent(c) ==
  c \in {
    UrgentPendingNear,
    UrgentMissingChunksNear,
    UrgentNonAuthoritativeNear
  }

ImplementationUrgent(c) ==
  CASE Bug = "urgent_accept_far_pending"
       /\ c = UrgentPendingFar -> TRUE
    [] Bug = "urgent_accept_delivered"
       /\ c = UrgentDeliveredNear -> TRUE
    [] Bug = "urgent_accept_authoritative"
       /\ c = UrgentAuthoritativeNear -> TRUE
    [] Bug = "urgent_accept_far_missing"
       /\ c = UrgentMissingChunksFar -> TRUE
    [] Bug = "urgent_reject_pending"
       /\ c = UrgentPendingNear -> FALSE
    [] Bug = "urgent_reject_missing_chunks"
       /\ c = UrgentMissingChunksNear -> FALSE
    [] Bug = "urgent_reject_non_authoritative"
       /\ c = UrgentNonAuthoritativeNear -> FALSE
    [] OTHER -> SpecUrgent(c)

ExemptNoSession == "exempt_no_session"
ExemptInvalid == "exempt_invalid"
ExemptDelivered == "exempt_delivered"
ExemptInactive == "exempt_inactive"
ExemptNotUrgent == "exempt_not_urgent"
ExemptNoPayloadRecovery == "exempt_no_payload_recovery"
ExemptAuthoritativeNonPending == "exempt_authoritative_non_pending"
ExemptMissingChunks == "exempt_missing_chunks"
ExemptPendingAuthoritativeMissing == "exempt_pending_authoritative_missing"
ExemptReadyBelowQuorumNoMissing == "exempt_ready_below_quorum_no_missing"

ExemptCases == {
  ExemptNoSession,
  ExemptInvalid,
  ExemptDelivered,
  ExemptInactive,
  ExemptNotUrgent,
  ExemptNoPayloadRecovery,
  ExemptAuthoritativeNonPending,
  ExemptMissingChunks,
  ExemptPendingAuthoritativeMissing,
  ExemptReadyBelowQuorumNoMissing
}

SpecExempt(c) ==
  c \in {ExemptMissingChunks, ExemptPendingAuthoritativeMissing}

ImplementationExempt(c) ==
  CASE Bug = "exempt_accept_invalid"
       /\ c = ExemptInvalid -> TRUE
    [] Bug = "exempt_accept_delivered"
       /\ c = ExemptDelivered -> TRUE
    [] Bug = "exempt_accept_inactive"
       /\ c = ExemptInactive -> TRUE
    [] Bug = "exempt_accept_not_urgent"
       /\ c = ExemptNotUrgent -> TRUE
    [] Bug = "exempt_accept_no_recovery"
       /\ c = ExemptNoPayloadRecovery -> TRUE
    [] Bug = "exempt_accept_authoritative_non_pending"
       /\ c = ExemptAuthoritativeNonPending -> TRUE
    [] Bug = "exempt_accept_ready_without_missing"
       /\ c = ExemptReadyBelowQuorumNoMissing -> TRUE
    [] Bug = "exempt_reject_missing_chunks"
       /\ c = ExemptMissingChunks -> FALSE
    [] Bug = "exempt_reject_pending_authoritative"
       /\ c = ExemptPendingAuthoritativeMissing -> FALSE
    [] OTHER -> SpecExempt(c)

ProposalPendingActive == "proposal_pending_active"
ProposalPendingAborted == "proposal_pending_aborted"
ProposalPendingWrongSlot == "proposal_pending_wrong_slot"
ProposalPendingInactiveTip == "proposal_pending_inactive_tip"
ProposalInflightActive == "proposal_inflight_active"
ProposalProcessingPendingActive == "proposal_processing_pending_active"
ProposalProcessingNoPending == "proposal_processing_no_pending"
ProposalNoSource == "proposal_no_source"

ProposalCases == {
  ProposalPendingActive,
  ProposalPendingAborted,
  ProposalPendingWrongSlot,
  ProposalPendingInactiveTip,
  ProposalInflightActive,
  ProposalProcessingPendingActive,
  ProposalProcessingNoPending,
  ProposalNoSource
}

SpecBlocksProposal(c) ==
  c \in {
    ProposalPendingActive,
    ProposalInflightActive,
    ProposalProcessingPendingActive
  }

ImplementationBlocksProposal(c) ==
  CASE Bug = "proposal_accept_aborted"
       /\ c = ProposalPendingAborted -> TRUE
    [] Bug = "proposal_accept_wrong_slot"
       /\ c = ProposalPendingWrongSlot -> TRUE
    [] Bug = "proposal_accept_inactive_tip"
       /\ c = ProposalPendingInactiveTip -> TRUE
    [] Bug = "proposal_processing_without_pending"
       /\ c = ProposalProcessingNoPending -> TRUE
    [] Bug = "proposal_reject_pending_active"
       /\ c = ProposalPendingActive -> FALSE
    [] Bug = "proposal_reject_inflight_active"
       /\ c = ProposalInflightActive -> FALSE
    [] Bug = "proposal_reject_processing_active"
       /\ c = ProposalProcessingPendingActive -> FALSE
    [] OTHER -> SpecBlocksProposal(c)

Bugs == {
  "none",
  "active_accept_aborted_pending",
  "active_ignore_pending",
  "active_processing_requires_session",
  "active_accept_invalid_session",
  "active_accept_missing_header",
  "active_accept_header_mismatch",
  "active_keep_delivered_at_tip",
  "active_keep_delivered_behind_tip",
  "active_reject_extends_tip",
  "active_reject_matches_tip",
  "suppress_delivered_committed_not_suppressed",
  "suppress_local_candidate",
  "suppress_local_payload",
  "suppress_no_local_near_allows",
  "exact_accept_not_recovered",
  "exact_accept_no_payload_recovery",
  "exact_accept_wrong_height",
  "exact_accept_wrong_slot",
  "exact_accept_not_armed",
  "exact_accept_body_present",
  "exact_reject_valid",
  "urgent_accept_far_pending",
  "urgent_accept_delivered",
  "urgent_accept_authoritative",
  "urgent_accept_far_missing",
  "urgent_reject_pending",
  "urgent_reject_missing_chunks",
  "urgent_reject_non_authoritative",
  "exempt_accept_invalid",
  "exempt_accept_delivered",
  "exempt_accept_inactive",
  "exempt_accept_not_urgent",
  "exempt_accept_no_recovery",
  "exempt_accept_authoritative_non_pending",
  "exempt_accept_ready_without_missing",
  "exempt_reject_missing_chunks",
  "exempt_reject_pending_authoritative",
  "proposal_accept_aborted",
  "proposal_accept_wrong_slot",
  "proposal_accept_inactive_tip",
  "proposal_processing_without_pending",
  "proposal_reject_pending_active",
  "proposal_reject_inflight_active",
  "proposal_reject_processing_active"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 44
     /\ checked' = checked + 1
  \/ /\ checked = 44
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..44
  /\ \A c \in ActiveCases:
       /\ SpecActive(c) \in BOOLEAN
       /\ ImplementationActive(c) \in BOOLEAN
  /\ \A c \in SuppressCases:
       /\ SpecSuppress(c) \in BOOLEAN
       /\ ImplementationSuppress(c) \in BOOLEAN
  /\ \A c \in ExactCases:
       /\ SpecExactRepair(c) \in BOOLEAN
       /\ ImplementationExactRepair(c) \in BOOLEAN
  /\ \A c \in UrgentCases:
       /\ SpecUrgent(c) \in BOOLEAN
       /\ ImplementationUrgent(c) \in BOOLEAN
  /\ \A c \in ExemptCases:
       /\ SpecExempt(c) \in BOOLEAN
       /\ ImplementationExempt(c) \in BOOLEAN
  /\ \A c \in ProposalCases:
       /\ SpecBlocksProposal(c) \in BOOLEAN
       /\ ImplementationBlocksProposal(c) \in BOOLEAN

SafetyFast ==
  /\ \A c \in ActiveCases:
       ImplementationActive(c) = SpecActive(c)
  /\ \A c \in SuppressCases:
       ImplementationSuppress(c) = SpecSuppress(c)
  /\ \A c \in ExactCases:
       ImplementationExactRepair(c) = SpecExactRepair(c)
  /\ \A c \in UrgentCases:
       ImplementationUrgent(c) = SpecUrgent(c)
  /\ \A c \in ExemptCases:
       ImplementationExempt(c) = SpecExempt(c)
  /\ \A c \in ProposalCases:
       ImplementationBlocksProposal(c) = SpecBlocksProposal(c)

AllActiveCasesMatchSpec ==
  \A c \in ActiveCases:
    ImplementationActive(c) = SpecActive(c)

AllSuppressCasesMatchSpec ==
  \A c \in SuppressCases:
    ImplementationSuppress(c) = SpecSuppress(c)

AllExactRepairCasesMatchSpec ==
  \A c \in ExactCases:
    ImplementationExactRepair(c) = SpecExactRepair(c)

AllUrgentCasesMatchSpec ==
  \A c \in UrgentCases:
    ImplementationUrgent(c) = SpecUrgent(c)

AllExemptCasesMatchSpec ==
  \A c \in ExemptCases:
    ImplementationExempt(c) = SpecExempt(c)

AllProposalCasesMatchSpec ==
  \A c \in ProposalCases:
    ImplementationBlocksProposal(c) = SpecBlocksProposal(c)

ActivePositiveAnchors ==
  /\ ImplementationActive(ActivePendingExtends)
  /\ ImplementationActive(ActiveInflightExtends)
  /\ ImplementationActive(ActiveProcessingHash)
  /\ ImplementationActive(ActiveSessionExtendsTip)
  /\ ImplementationActive(ActiveSessionMatchesTip)

ActiveRejectAnchors ==
  /\ ~ImplementationActive(ActivePendingAborted)
  /\ ~ImplementationActive(ActivePendingWrongSlot)
  /\ ~ImplementationActive(ActiveSessionAbsent)
  /\ ~ImplementationActive(ActiveSessionInvalid)
  /\ ~ImplementationActive(ActiveSessionMissingHeader)
  /\ ~ImplementationActive(ActiveSessionHeaderMismatch)
  /\ ~ImplementationActive(ActiveDeliveredAtTip)
  /\ ~ImplementationActive(ActiveDeliveredBehindTip)
  /\ ~ImplementationActive(ActiveSessionOffTip)

SuppressAnchors ==
  /\ ImplementationSuppress(SuppressDeliveredCommitted)
  /\ ImplementationSuppress(SuppressNoLocalNear)
  /\ ImplementationSuppress(SuppressNoLocalFar)
  /\ ImplementationSuppress(SuppressDeliveredCommittedLocalCandidate)
  /\ ~ImplementationSuppress(SuppressLocalCandidate)
  /\ ~ImplementationSuppress(SuppressLocalPayload)

ExactRepairAnchors ==
  /\ ImplementationExactRepair(ExactRecoveredFrontier)
  /\ ~ImplementationExactRepair(ExactNotRecovered)
  /\ ~ImplementationExactRepair(ExactNoPayloadRecovery)
  /\ ~ImplementationExactRepair(ExactWrongHeight)
  /\ ~ImplementationExactRepair(ExactSlotWrongHash)
  /\ ~ImplementationExactRepair(ExactSlotWrongView)
  /\ ~ImplementationExactRepair(ExactSlotNotArmed)
  /\ ~ImplementationExactRepair(ExactSlotBodyPresent)

UrgentAnchors ==
  /\ ImplementationUrgent(UrgentPendingNear)
  /\ ImplementationUrgent(UrgentMissingChunksNear)
  /\ ImplementationUrgent(UrgentNonAuthoritativeNear)
  /\ ~ImplementationUrgent(UrgentPendingFar)
  /\ ~ImplementationUrgent(UrgentDeliveredNear)
  /\ ~ImplementationUrgent(UrgentAuthoritativeNear)
  /\ ~ImplementationUrgent(UrgentMissingChunksFar)

ExemptAnchors ==
  /\ ImplementationExempt(ExemptMissingChunks)
  /\ ImplementationExempt(ExemptPendingAuthoritativeMissing)
  /\ ~ImplementationExempt(ExemptNoSession)
  /\ ~ImplementationExempt(ExemptInvalid)
  /\ ~ImplementationExempt(ExemptDelivered)
  /\ ~ImplementationExempt(ExemptInactive)
  /\ ~ImplementationExempt(ExemptNotUrgent)
  /\ ~ImplementationExempt(ExemptNoPayloadRecovery)
  /\ ~ImplementationExempt(ExemptAuthoritativeNonPending)
  /\ ~ImplementationExempt(ExemptReadyBelowQuorumNoMissing)

ProposalBlockingAnchors ==
  /\ ImplementationBlocksProposal(ProposalPendingActive)
  /\ ImplementationBlocksProposal(ProposalInflightActive)
  /\ ImplementationBlocksProposal(ProposalProcessingPendingActive)
  /\ ~ImplementationBlocksProposal(ProposalPendingAborted)
  /\ ~ImplementationBlocksProposal(ProposalPendingWrongSlot)
  /\ ~ImplementationBlocksProposal(ProposalPendingInactiveTip)
  /\ ~ImplementationBlocksProposal(ProposalProcessingNoPending)
  /\ ~ImplementationBlocksProposal(ProposalNoSource)

SafetyAnchors ==
  /\ AllActiveCasesMatchSpec
  /\ AllSuppressCasesMatchSpec
  /\ AllExactRepairCasesMatchSpec
  /\ AllUrgentCasesMatchSpec
  /\ AllExemptCasesMatchSpec
  /\ AllProposalCasesMatchSpec
  /\ ActivePositiveAnchors
  /\ ActiveRejectAnchors
  /\ SuppressAnchors
  /\ ExactRepairAnchors
  /\ UrgentAnchors
  /\ ExemptAnchors
  /\ ProposalBlockingAnchors

====

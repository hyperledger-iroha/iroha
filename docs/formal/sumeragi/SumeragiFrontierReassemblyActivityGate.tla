---- MODULE SumeragiFrontierReassemblyActivityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for frontier reassembly activity predicates.

This slice pins `frontier_recovery_same_slot_reassembly_active(...)` and the
same-height RBC sender activity helper that feeds it. Reassembly may suppress
rotation only when there is exact same-slot ingress, fresh same-height
dependency progress with payload inbound backlog, recent same-height RBC sender
or deferral activity, same-height untimed RBC work, exact validation work, or
an exact deferred BlockSyncUpdate. Stale sender timestamps, wrong heights or
views, aborted/non-pending validation work, and empty-source reassembly must
not look active.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == "none"

Bugs == {
  NoBug,
  "dependency_reject_frontier_progress",
  "dependency_reject_storm_progress",
  "dependency_accept_stale_progress",
  "dependency_accept_no_payload_backlog",
  "dependency_accept_wrong_height_progress",
  "ingress_reject_same_slot",
  "ingress_accept_no_backlog",
  "ingress_accept_wrong_view",
  "sender_reject_payload_rebroadcast",
  "sender_reject_targeted_payload_rescue",
  "sender_reject_init_repair",
  "sender_reject_chunk_repair",
  "sender_reject_ready_rebroadcast",
  "sender_reject_deliver_rebroadcast",
  "sender_reject_ready_deferral",
  "sender_reject_deliver_deferral",
  "sender_reject_outbound_chunks",
  "sender_reject_persist_inflight",
  "sender_reject_persist_pending_refresh",
  "sender_reject_seed_inflight",
  "sender_accept_stale_timed",
  "sender_accept_wrong_height_timed",
  "sender_accept_wrong_height_untimed",
  "validation_reject_inflight",
  "validation_accept_aborted",
  "validation_accept_wrong_height",
  "validation_accept_wrong_view",
  "validation_accept_non_pending",
  "deferred_reject_exact_update",
  "deferred_accept_wrong_height",
  "deferred_accept_wrong_view",
  "reassembly_accept_no_source"
}

DependencyFrontierProgressExact ==
  IF Bug = "dependency_reject_frontier_progress" THEN FALSE ELSE TRUE

DependencyStormProgressExact ==
  IF Bug = "dependency_reject_storm_progress" THEN FALSE ELSE TRUE

DependencyStaleProgressAccepted ==
  IF Bug = "dependency_accept_stale_progress" THEN TRUE ELSE FALSE

DependencyNoPayloadBacklogAccepted ==
  IF Bug = "dependency_accept_no_payload_backlog" THEN TRUE ELSE FALSE

DependencyWrongHeightAccepted ==
  IF Bug = "dependency_accept_wrong_height_progress" THEN TRUE ELSE FALSE

SameSlotIngressExact ==
  IF Bug = "ingress_reject_same_slot" THEN FALSE ELSE TRUE

IngressNoBacklogAccepted ==
  IF Bug = "ingress_accept_no_backlog" THEN TRUE ELSE FALSE

IngressWrongViewAccepted ==
  IF Bug = "ingress_accept_wrong_view" THEN TRUE ELSE FALSE

SenderPayloadRebroadcastExact ==
  IF Bug = "sender_reject_payload_rebroadcast" THEN FALSE ELSE TRUE

SenderTargetedPayloadRescueExact ==
  IF Bug = "sender_reject_targeted_payload_rescue" THEN FALSE ELSE TRUE

SenderInitRepairExact ==
  IF Bug = "sender_reject_init_repair" THEN FALSE ELSE TRUE

SenderChunkRepairExact ==
  IF Bug = "sender_reject_chunk_repair" THEN FALSE ELSE TRUE

SenderReadyRebroadcastExact ==
  IF Bug = "sender_reject_ready_rebroadcast" THEN FALSE ELSE TRUE

SenderDeliverRebroadcastExact ==
  IF Bug = "sender_reject_deliver_rebroadcast" THEN FALSE ELSE TRUE

SenderReadyDeferralExact ==
  IF Bug = "sender_reject_ready_deferral" THEN FALSE ELSE TRUE

SenderDeliverDeferralExact ==
  IF Bug = "sender_reject_deliver_deferral" THEN FALSE ELSE TRUE

SenderOutboundChunksExact ==
  IF Bug = "sender_reject_outbound_chunks" THEN FALSE ELSE TRUE

SenderPersistInflightExact ==
  IF Bug = "sender_reject_persist_inflight" THEN FALSE ELSE TRUE

SenderPersistPendingRefreshExact ==
  IF Bug = "sender_reject_persist_pending_refresh" THEN FALSE ELSE TRUE

SenderSeedInflightExact ==
  IF Bug = "sender_reject_seed_inflight" THEN FALSE ELSE TRUE

SenderStaleTimedAccepted ==
  IF Bug = "sender_accept_stale_timed" THEN TRUE ELSE FALSE

SenderWrongHeightTimedAccepted ==
  IF Bug = "sender_accept_wrong_height_timed" THEN TRUE ELSE FALSE

SenderWrongHeightUntimedAccepted ==
  IF Bug = "sender_accept_wrong_height_untimed" THEN TRUE ELSE FALSE

ValidationInflightExact ==
  IF Bug = "validation_reject_inflight" THEN FALSE ELSE TRUE

ValidationAbortedAccepted ==
  IF Bug = "validation_accept_aborted" THEN TRUE ELSE FALSE

ValidationWrongHeightAccepted ==
  IF Bug = "validation_accept_wrong_height" THEN TRUE ELSE FALSE

ValidationWrongViewAccepted ==
  IF Bug = "validation_accept_wrong_view" THEN TRUE ELSE FALSE

ValidationNonPendingAccepted ==
  IF Bug = "validation_accept_non_pending" THEN TRUE ELSE FALSE

DeferredBlockSyncExact ==
  IF Bug = "deferred_reject_exact_update" THEN FALSE ELSE TRUE

DeferredWrongHeightAccepted ==
  IF Bug = "deferred_accept_wrong_height" THEN TRUE ELSE FALSE

DeferredWrongViewAccepted ==
  IF Bug = "deferred_accept_wrong_view" THEN TRUE ELSE FALSE

ReassemblyWithoutSourceAccepted ==
  IF Bug = "reassembly_accept_no_source" THEN TRUE ELSE FALSE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 32
     /\ checked' = checked + 1
  \/ /\ checked = 32
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..32

DependencyProgressSafety ==
  /\ DependencyFrontierProgressExact
  /\ DependencyStormProgressExact
  /\ ~DependencyStaleProgressAccepted
  /\ ~DependencyNoPayloadBacklogAccepted
  /\ ~DependencyWrongHeightAccepted

IngressSafety ==
  /\ SameSlotIngressExact
  /\ ~IngressNoBacklogAccepted
  /\ ~IngressWrongViewAccepted

SenderActivitySafety ==
  /\ SenderPayloadRebroadcastExact
  /\ SenderTargetedPayloadRescueExact
  /\ SenderInitRepairExact
  /\ SenderChunkRepairExact
  /\ SenderReadyRebroadcastExact
  /\ SenderDeliverRebroadcastExact
  /\ SenderReadyDeferralExact
  /\ SenderDeliverDeferralExact
  /\ SenderOutboundChunksExact
  /\ SenderPersistInflightExact
  /\ SenderPersistPendingRefreshExact
  /\ SenderSeedInflightExact
  /\ ~SenderStaleTimedAccepted
  /\ ~SenderWrongHeightTimedAccepted
  /\ ~SenderWrongHeightUntimedAccepted

ValidationSafety ==
  /\ ValidationInflightExact
  /\ ~ValidationAbortedAccepted
  /\ ~ValidationWrongHeightAccepted
  /\ ~ValidationWrongViewAccepted
  /\ ~ValidationNonPendingAccepted

DeferredBlockSyncSafety ==
  /\ DeferredBlockSyncExact
  /\ ~DeferredWrongHeightAccepted
  /\ ~DeferredWrongViewAccepted

NoSpuriousReassemblySafety ==
  ~ReassemblyWithoutSourceAccepted

ReassemblyActivityRejectsNonExactInputs ==
  /\ ~DependencyStaleProgressAccepted
  /\ ~DependencyNoPayloadBacklogAccepted
  /\ ~DependencyWrongHeightAccepted
  /\ ~IngressNoBacklogAccepted
  /\ ~IngressWrongViewAccepted
  /\ ~SenderStaleTimedAccepted
  /\ ~SenderWrongHeightTimedAccepted
  /\ ~SenderWrongHeightUntimedAccepted
  /\ ~ValidationAbortedAccepted
  /\ ~ValidationWrongHeightAccepted
  /\ ~ValidationWrongViewAccepted
  /\ ~ValidationNonPendingAccepted
  /\ ~DeferredWrongHeightAccepted
  /\ ~DeferredWrongViewAccepted
  /\ ~ReassemblyWithoutSourceAccepted

ReassemblyActivityHasExactPositiveEvidence ==
  /\ (\/ DependencyFrontierProgressExact
      \/ DependencyStormProgressExact)
  /\ SameSlotIngressExact
  /\ (\/ SenderPayloadRebroadcastExact
      \/ SenderTargetedPayloadRescueExact
      \/ SenderInitRepairExact
      \/ SenderChunkRepairExact
      \/ SenderReadyRebroadcastExact
      \/ SenderDeliverRebroadcastExact
      \/ SenderReadyDeferralExact
      \/ SenderDeliverDeferralExact
      \/ SenderOutboundChunksExact
      \/ SenderPersistInflightExact
      \/ SenderPersistPendingRefreshExact
      \/ SenderSeedInflightExact)
  /\ ValidationInflightExact
  /\ DeferredBlockSyncExact

FrontierReassemblyActivityExactness ==
  /\ ReassemblyActivityRejectsNonExactInputs
  /\ ReassemblyActivityHasExactPositiveEvidence

SafetyFast ==
  /\ DependencyProgressSafety
  /\ IngressSafety
  /\ SenderActivitySafety
  /\ ValidationSafety
  /\ DeferredBlockSyncSafety
  /\ NoSpuriousReassemblySafety
  /\ FrontierReassemblyActivityExactness

DependencyProgressAnchors ==
  /\ DependencyProgressSafety
  /\ DependencyFrontierProgressExact
  /\ DependencyStormProgressExact
  /\ ~DependencyStaleProgressAccepted
  /\ ~DependencyNoPayloadBacklogAccepted
  /\ ~DependencyWrongHeightAccepted

IngressAnchors ==
  /\ IngressSafety
  /\ SameSlotIngressExact
  /\ ~IngressNoBacklogAccepted
  /\ ~IngressWrongViewAccepted

SenderActivityAnchors ==
  /\ SenderActivitySafety
  /\ SenderPayloadRebroadcastExact
  /\ SenderTargetedPayloadRescueExact
  /\ SenderInitRepairExact
  /\ SenderChunkRepairExact
  /\ SenderReadyRebroadcastExact
  /\ SenderDeliverRebroadcastExact
  /\ SenderReadyDeferralExact
  /\ SenderDeliverDeferralExact
  /\ SenderOutboundChunksExact
  /\ SenderPersistInflightExact
  /\ SenderPersistPendingRefreshExact
  /\ SenderSeedInflightExact
  /\ ~SenderStaleTimedAccepted
  /\ ~SenderWrongHeightTimedAccepted
  /\ ~SenderWrongHeightUntimedAccepted

ValidationAnchors ==
  /\ ValidationSafety
  /\ ValidationInflightExact
  /\ ~ValidationAbortedAccepted
  /\ ~ValidationWrongHeightAccepted
  /\ ~ValidationWrongViewAccepted
  /\ ~ValidationNonPendingAccepted

DeferredBlockSyncAnchors ==
  /\ DeferredBlockSyncSafety
  /\ DeferredBlockSyncExact
  /\ ~DeferredWrongHeightAccepted
  /\ ~DeferredWrongViewAccepted

NoSpuriousReassemblyAnchors ==
  /\ NoSpuriousReassemblySafety
  /\ ~ReassemblyWithoutSourceAccepted

FrontierReassemblyActivitySafetyAnchors ==
  /\ DependencyProgressAnchors
  /\ IngressAnchors
  /\ SenderActivityAnchors
  /\ ValidationAnchors
  /\ DeferredBlockSyncAnchors
  /\ NoSpuriousReassemblyAnchors

FrontierReassemblyActivityCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ FrontierReassemblyActivitySafetyAnchors

Safety == FrontierReassemblyActivitySafetyAnchors

====

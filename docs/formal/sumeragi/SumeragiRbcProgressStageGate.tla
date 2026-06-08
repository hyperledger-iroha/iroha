---- MODULE SumeragiRbcProgressStageGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC progress-stage synchronization.

This slice captures the monotone `RbcProgressStage` helper family:
- `advance_progress_stage(...)` only moves to a strictly later stage;
- `sync_progress_observations(...)` cascades complete payload, authoritative
  payload, local READY, and DELIVERED observations in stage order; and
- `sync_rbc_progress_stage_with_roster(...)` derives a READY quorum requirement
  only for authoritative non-empty rosters, while that quorum alone does not
  advance progress today.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CollectingChunks == "CollectingChunks"
AuthoritativePayload == "AuthoritativePayload"
LocalReadySent == "LocalReadySent"
Delivered == "Delivered"

Stages == {
  CollectingChunks,
  AuthoritativePayload,
  LocalReadySent,
  Delivered
}

StageRank(stage) ==
  CASE stage = CollectingChunks -> 0
    [] stage = AuthoritativePayload -> 1
    [] stage = LocalReadySent -> 2
    [] stage = Delivered -> 3

AdvanceStage(current, target) ==
  IF StageRank(target) > StageRank(current) THEN target ELSE current

AdvanceCollectingToAuthoritative == "advance_collecting_to_authoritative"
AdvanceCollectingToReady == "advance_collecting_to_ready"
AdvanceCollectingToDelivered == "advance_collecting_to_delivered"
AdvanceReadyToAuthoritative == "advance_ready_to_authoritative"
AdvanceDeliveredToReady == "advance_delivered_to_ready"
AdvanceAuthoritativeEqual == "advance_authoritative_equal"
AdvanceAuthoritativeToDelivered == "advance_authoritative_to_delivered"

AdvanceCases == {
  AdvanceCollectingToAuthoritative,
  AdvanceCollectingToReady,
  AdvanceCollectingToDelivered,
  AdvanceReadyToAuthoritative,
  AdvanceDeliveredToReady,
  AdvanceAuthoritativeEqual,
  AdvanceAuthoritativeToDelivered
}

AdvanceStart(c) ==
  CASE c \in {
         AdvanceCollectingToAuthoritative,
         AdvanceCollectingToReady,
         AdvanceCollectingToDelivered
       } -> CollectingChunks
    [] c = AdvanceReadyToAuthoritative -> LocalReadySent
    [] c = AdvanceDeliveredToReady -> Delivered
    [] c \in {AdvanceAuthoritativeEqual, AdvanceAuthoritativeToDelivered} ->
         AuthoritativePayload

AdvanceTarget(c) ==
  CASE c = AdvanceCollectingToAuthoritative -> AuthoritativePayload
    [] c = AdvanceCollectingToReady -> LocalReadySent
    [] c = AdvanceCollectingToDelivered -> Delivered
    [] c = AdvanceReadyToAuthoritative -> AuthoritativePayload
    [] c = AdvanceDeliveredToReady -> LocalReadySent
    [] c = AdvanceAuthoritativeEqual -> AuthoritativePayload
    [] c = AdvanceAuthoritativeToDelivered -> Delivered

SpecAdvanceStage(c) ==
  AdvanceStage(AdvanceStart(c), AdvanceTarget(c))

SpecAdvanceProgressed(c) ==
  SpecAdvanceStage(c) /= AdvanceStart(c)

ImplementationAdvanceStage(c) ==
  CASE Bug = "advance_allows_regress"
       /\ c = AdvanceReadyToAuthoritative -> AuthoritativePayload
    [] Bug = "advance_skip_authoritative"
       /\ c = AdvanceCollectingToAuthoritative -> CollectingChunks
    [] Bug = "advance_skip_local_ready"
       /\ c = AdvanceCollectingToReady -> AuthoritativePayload
    [] Bug = "advance_skip_delivered"
       /\ c = AdvanceCollectingToDelivered -> LocalReadySent
    [] OTHER -> SpecAdvanceStage(c)

ImplementationAdvanceProgressed(c) ==
  CASE Bug = "advance_equal_reports_progress"
       /\ c = AdvanceAuthoritativeEqual -> TRUE
    [] OTHER -> ImplementationAdvanceStage(c) /= AdvanceStart(c)

SyncNone == "sync_none"
SyncCompletePayloadCase == "sync_complete_payload"
SyncAuthoritativePayload == "sync_authoritative_payload"
SyncAuthoritativePayloadWithQuorum == "sync_authoritative_payload_with_quorum"
SyncLocalReady == "sync_local_ready"
SyncDeliveredCase == "sync_delivered"
SyncAllObservationsDelivered == "sync_all_observations_delivered"
SyncReadyQuorumOnly == "sync_ready_quorum_only"
SyncReadyFromAuthoritative == "sync_ready_from_authoritative"
SyncDeliveredFromLocalReady == "sync_delivered_from_local_ready"
SyncDeliveredNoObservation == "sync_delivered_no_observation"
SyncAuthoritativeFromLocalReady == "sync_authoritative_from_local_ready"

SyncCases == {
  SyncNone,
  SyncCompletePayloadCase,
  SyncAuthoritativePayload,
  SyncAuthoritativePayloadWithQuorum,
  SyncLocalReady,
  SyncDeliveredCase,
  SyncAllObservationsDelivered,
  SyncReadyQuorumOnly,
  SyncReadyFromAuthoritative,
  SyncDeliveredFromLocalReady,
  SyncDeliveredNoObservation,
  SyncAuthoritativeFromLocalReady
}

SyncStart(c) ==
  CASE c \in {SyncDeliveredFromLocalReady, SyncAuthoritativeFromLocalReady} ->
         LocalReadySent
    [] c = SyncDeliveredNoObservation -> Delivered
    [] c = SyncReadyFromAuthoritative -> AuthoritativePayload
    [] OTHER -> CollectingChunks

SyncHasCompletePayload(c) ==
  c \in {SyncCompletePayloadCase, SyncAllObservationsDelivered}

SyncHasAuthoritativePayload(c) ==
  c \in {
    SyncAuthoritativePayload,
    SyncAuthoritativePayloadWithQuorum,
    SyncAllObservationsDelivered,
    SyncAuthoritativeFromLocalReady
  }

SyncHasLocalReady(c) ==
  c \in {SyncLocalReady, SyncAllObservationsDelivered, SyncReadyFromAuthoritative}

SyncHasDelivered(c) ==
  c \in {
    SyncDeliveredCase,
    SyncAllObservationsDelivered,
    SyncDeliveredFromLocalReady
  }

SyncReadyQuorumRequired(c) ==
  c \in {SyncReadyQuorumOnly, SyncAuthoritativePayloadWithQuorum}

SyncAfterComplete(c) ==
  IF SyncHasCompletePayload(c)
  THEN AdvanceStage(SyncStart(c), AuthoritativePayload)
  ELSE SyncStart(c)

SyncAfterAuthoritative(c) ==
  IF SyncHasAuthoritativePayload(c)
  THEN AdvanceStage(SyncAfterComplete(c), AuthoritativePayload)
  ELSE SyncAfterComplete(c)

SyncAfterReady(c) ==
  IF SyncHasLocalReady(c)
  THEN AdvanceStage(SyncAfterAuthoritative(c), LocalReadySent)
  ELSE SyncAfterAuthoritative(c)

SpecSyncStage(c) ==
  IF SyncHasDelivered(c)
  THEN AdvanceStage(SyncAfterReady(c), Delivered)
  ELSE SyncAfterReady(c)

SpecSyncProgressed(c) ==
  SpecSyncStage(c) /= SyncStart(c)

ImplementationSyncStage(c) ==
  CASE Bug = "sync_ready_quorum_advances"
       /\ c = SyncReadyQuorumOnly -> LocalReadySent
    [] Bug = "sync_skip_complete_payload"
       /\ c = SyncCompletePayloadCase -> CollectingChunks
    [] Bug = "sync_skip_authoritative_payload"
       /\ c = SyncAuthoritativePayload -> CollectingChunks
    [] Bug = "sync_skip_local_ready"
       /\ c = SyncLocalReady -> AuthoritativePayload
    [] Bug = "sync_skip_delivered"
       /\ c = SyncDeliveredCase -> LocalReadySent
    [] Bug = "sync_stop_after_authoritative"
       /\ c = SyncAllObservationsDelivered -> AuthoritativePayload
    [] Bug = "sync_regresses_delivered"
       /\ c = SyncDeliveredNoObservation -> LocalReadySent
    [] Bug = "sync_authoritative_regresses_ready"
       /\ c = SyncAuthoritativeFromLocalReady -> AuthoritativePayload
    [] Bug = "sync_quorum_blocks_payload"
       /\ c = SyncAuthoritativePayloadWithQuorum -> CollectingChunks
    [] OTHER -> SpecSyncStage(c)

ImplementationSyncProgressed(c) ==
  ImplementationSyncStage(c) /= SyncStart(c)

Derived == "Derived"
InitRoster == "Init"

RosterSources == {Derived, InitRoster}

RosterAuthoritativeNonempty == "roster_authoritative_nonempty"
RosterInitNonempty == "roster_init_nonempty"
RosterAuthoritativeEmpty == "roster_authoritative_empty"
RosterAuthoritativePayloadEmpty == "roster_authoritative_payload_empty"
RosterInitPayload == "roster_init_payload"
RosterDerivedReadySent == "roster_derived_ready_sent"
RosterDerivedDelivered == "roster_derived_delivered"

RosterCases == {
  RosterAuthoritativeNonempty,
  RosterInitNonempty,
  RosterAuthoritativeEmpty,
  RosterAuthoritativePayloadEmpty,
  RosterInitPayload,
  RosterDerivedReadySent,
  RosterDerivedDelivered
}

RosterSource(c) ==
  CASE c \in {RosterInitNonempty, RosterInitPayload} -> InitRoster
    [] OTHER -> Derived

RosterSize(c) ==
  CASE c \in {RosterAuthoritativeEmpty, RosterAuthoritativePayloadEmpty} -> 0
    [] OTHER -> 4

SpecRosterQuorumComputed(c) ==
  RosterSource(c) = Derived /\ RosterSize(c) > 0

RosterStart(c) ==
  CollectingChunks

RosterHasCompletePayload(c) ==
  FALSE

RosterHasAuthoritativePayload(c) ==
  c \in {RosterAuthoritativePayloadEmpty, RosterInitPayload}

RosterHasLocalReady(c) ==
  c = RosterDerivedReadySent

RosterHasDelivered(c) ==
  c = RosterDerivedDelivered

RosterAfterComplete(c) ==
  IF RosterHasCompletePayload(c)
  THEN AdvanceStage(RosterStart(c), AuthoritativePayload)
  ELSE RosterStart(c)

RosterAfterAuthoritative(c) ==
  IF RosterHasAuthoritativePayload(c)
  THEN AdvanceStage(RosterAfterComplete(c), AuthoritativePayload)
  ELSE RosterAfterComplete(c)

RosterAfterReady(c) ==
  IF RosterHasLocalReady(c)
  THEN AdvanceStage(RosterAfterAuthoritative(c), LocalReadySent)
  ELSE RosterAfterAuthoritative(c)

SpecRosterStage(c) ==
  IF RosterHasDelivered(c)
  THEN AdvanceStage(RosterAfterReady(c), Delivered)
  ELSE RosterAfterReady(c)

SpecRosterProgressed(c) ==
  SpecRosterStage(c) /= RosterStart(c)

ImplementationRosterQuorumComputed(c) ==
  CASE Bug = "roster_non_authoritative_computes_quorum"
       /\ c = RosterInitNonempty -> TRUE
    [] Bug = "roster_empty_authoritative_computes_quorum"
       /\ c = RosterAuthoritativeEmpty -> TRUE
    [] Bug = "roster_authoritative_nonempty_skips_quorum"
       /\ c = RosterAuthoritativeNonempty -> FALSE
    [] OTHER -> SpecRosterQuorumComputed(c)

ImplementationRosterStage(c) ==
  CASE Bug = "roster_quorum_advances"
       /\ c = RosterAuthoritativeNonempty -> LocalReadySent
    [] Bug = "roster_non_authoritative_blocks_payload"
       /\ c = RosterInitPayload -> CollectingChunks
    [] Bug = "roster_empty_blocks_payload"
       /\ c = RosterAuthoritativePayloadEmpty -> CollectingChunks
    [] OTHER -> SpecRosterStage(c)

ImplementationRosterProgressed(c) ==
  ImplementationRosterStage(c) /= RosterStart(c)

Bugs == {
  "none",
  "advance_allows_regress",
  "advance_equal_reports_progress",
  "advance_skip_authoritative",
  "advance_skip_local_ready",
  "advance_skip_delivered",
  "sync_ready_quorum_advances",
  "sync_skip_complete_payload",
  "sync_skip_authoritative_payload",
  "sync_skip_local_ready",
  "sync_skip_delivered",
  "sync_stop_after_authoritative",
  "sync_regresses_delivered",
  "sync_authoritative_regresses_ready",
  "sync_quorum_blocks_payload",
  "roster_non_authoritative_computes_quorum",
  "roster_empty_authoritative_computes_quorum",
  "roster_authoritative_nonempty_skips_quorum",
  "roster_quorum_advances",
  "roster_non_authoritative_blocks_payload",
  "roster_empty_blocks_payload"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 20
     /\ checked' = checked + 1
  \/ /\ checked = 20
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..20
  /\ \A stage \in Stages: StageRank(stage) \in 0..3
  /\ \A c \in AdvanceCases:
       /\ AdvanceStart(c) \in Stages
       /\ AdvanceTarget(c) \in Stages
       /\ SpecAdvanceStage(c) \in Stages
       /\ SpecAdvanceProgressed(c) \in BOOLEAN
       /\ ImplementationAdvanceStage(c) \in Stages
       /\ ImplementationAdvanceProgressed(c) \in BOOLEAN
  /\ \A c \in SyncCases:
       /\ SyncStart(c) \in Stages
       /\ SyncHasCompletePayload(c) \in BOOLEAN
       /\ SyncHasAuthoritativePayload(c) \in BOOLEAN
       /\ SyncHasLocalReady(c) \in BOOLEAN
       /\ SyncHasDelivered(c) \in BOOLEAN
       /\ SyncReadyQuorumRequired(c) \in BOOLEAN
       /\ SpecSyncStage(c) \in Stages
       /\ SpecSyncProgressed(c) \in BOOLEAN
       /\ ImplementationSyncStage(c) \in Stages
       /\ ImplementationSyncProgressed(c) \in BOOLEAN
  /\ \A c \in RosterCases:
       /\ RosterSource(c) \in RosterSources
       /\ RosterSize(c) \in 0..4
       /\ SpecRosterQuorumComputed(c) \in BOOLEAN
       /\ SpecRosterStage(c) \in Stages
       /\ SpecRosterProgressed(c) \in BOOLEAN
       /\ ImplementationRosterQuorumComputed(c) \in BOOLEAN
       /\ ImplementationRosterStage(c) \in Stages
       /\ ImplementationRosterProgressed(c) \in BOOLEAN

RbcProgressStageMatchesSpec ==
  /\ \A c \in AdvanceCases:
       /\ ImplementationAdvanceStage(c) = SpecAdvanceStage(c)
       /\ ImplementationAdvanceProgressed(c) = SpecAdvanceProgressed(c)
  /\ \A c \in SyncCases:
       /\ ImplementationSyncStage(c) = SpecSyncStage(c)
       /\ ImplementationSyncProgressed(c) = SpecSyncProgressed(c)
  /\ \A c \in RosterCases:
       /\ ImplementationRosterQuorumComputed(c) = SpecRosterQuorumComputed(c)
       /\ ImplementationRosterStage(c) = SpecRosterStage(c)
       /\ ImplementationRosterProgressed(c) = SpecRosterProgressed(c)

SafetyFast ==
  RbcProgressStageMatchesSpec

AllAdvanceCasesMatchSpec ==
  \A c \in AdvanceCases:
    /\ ImplementationAdvanceStage(c) = SpecAdvanceStage(c)
    /\ ImplementationAdvanceProgressed(c) = SpecAdvanceProgressed(c)

AllSyncCasesMatchSpec ==
  \A c \in SyncCases:
    /\ ImplementationSyncStage(c) = SpecSyncStage(c)
    /\ ImplementationSyncProgressed(c) = SpecSyncProgressed(c)

AllRosterCasesMatchSpec ==
  \A c \in RosterCases:
    /\ ImplementationRosterQuorumComputed(c) = SpecRosterQuorumComputed(c)
    /\ ImplementationRosterStage(c) = SpecRosterStage(c)
    /\ ImplementationRosterProgressed(c) = SpecRosterProgressed(c)

StageOrderAnchors ==
  /\ StageRank(CollectingChunks) = 0
  /\ StageRank(AuthoritativePayload) = 1
  /\ StageRank(LocalReadySent) = 2
  /\ StageRank(Delivered) = 3
  /\ AdvanceStage(LocalReadySent, AuthoritativePayload) = LocalReadySent
  /\ AdvanceStage(Delivered, LocalReadySent) = Delivered

AdvanceForwardAnchors ==
  /\ ImplementationAdvanceStage(AdvanceCollectingToAuthoritative) =
       AuthoritativePayload
  /\ ImplementationAdvanceProgressed(AdvanceCollectingToAuthoritative)
  /\ ImplementationAdvanceStage(AdvanceCollectingToReady) = LocalReadySent
  /\ ImplementationAdvanceProgressed(AdvanceCollectingToReady)
  /\ ImplementationAdvanceStage(AdvanceCollectingToDelivered) = Delivered
  /\ ImplementationAdvanceProgressed(AdvanceCollectingToDelivered)
  /\ ImplementationAdvanceStage(AdvanceAuthoritativeToDelivered) = Delivered
  /\ ImplementationAdvanceProgressed(AdvanceAuthoritativeToDelivered)

AdvanceNoRegressionAnchors ==
  /\ ImplementationAdvanceStage(AdvanceReadyToAuthoritative) = LocalReadySent
  /\ ~ImplementationAdvanceProgressed(AdvanceReadyToAuthoritative)
  /\ ImplementationAdvanceStage(AdvanceDeliveredToReady) = Delivered
  /\ ~ImplementationAdvanceProgressed(AdvanceDeliveredToReady)
  /\ ImplementationAdvanceStage(AdvanceAuthoritativeEqual) =
       AuthoritativePayload
  /\ ~ImplementationAdvanceProgressed(AdvanceAuthoritativeEqual)

SyncObservationAnchors ==
  /\ ImplementationSyncStage(SyncNone) = CollectingChunks
  /\ ~ImplementationSyncProgressed(SyncNone)
  /\ ImplementationSyncStage(SyncCompletePayloadCase) = AuthoritativePayload
  /\ ImplementationSyncProgressed(SyncCompletePayloadCase)
  /\ ImplementationSyncStage(SyncAuthoritativePayload) = AuthoritativePayload
  /\ ImplementationSyncProgressed(SyncAuthoritativePayload)
  /\ ImplementationSyncStage(SyncLocalReady) = LocalReadySent
  /\ ImplementationSyncProgressed(SyncLocalReady)
  /\ ImplementationSyncStage(SyncDeliveredCase) = Delivered
  /\ ImplementationSyncProgressed(SyncDeliveredCase)
  /\ ImplementationSyncStage(SyncAllObservationsDelivered) = Delivered
  /\ ImplementationSyncProgressed(SyncAllObservationsDelivered)

SyncNoRegressionAnchors ==
  /\ ImplementationSyncStage(SyncReadyQuorumOnly) = CollectingChunks
  /\ ~ImplementationSyncProgressed(SyncReadyQuorumOnly)
  /\ ImplementationSyncStage(SyncReadyFromAuthoritative) = LocalReadySent
  /\ ImplementationSyncProgressed(SyncReadyFromAuthoritative)
  /\ ImplementationSyncStage(SyncDeliveredFromLocalReady) = Delivered
  /\ ImplementationSyncProgressed(SyncDeliveredFromLocalReady)
  /\ ImplementationSyncStage(SyncDeliveredNoObservation) = Delivered
  /\ ~ImplementationSyncProgressed(SyncDeliveredNoObservation)
  /\ ImplementationSyncStage(SyncAuthoritativeFromLocalReady) =
       LocalReadySent
  /\ ~ImplementationSyncProgressed(SyncAuthoritativeFromLocalReady)
  /\ ImplementationSyncStage(SyncAuthoritativePayloadWithQuorum) =
       AuthoritativePayload
  /\ ImplementationSyncProgressed(SyncAuthoritativePayloadWithQuorum)

RosterQuorumAnchors ==
  /\ ImplementationRosterQuorumComputed(RosterAuthoritativeNonempty)
  /\ ~ImplementationRosterQuorumComputed(RosterInitNonempty)
  /\ ~ImplementationRosterQuorumComputed(RosterAuthoritativeEmpty)
  /\ ~ImplementationRosterQuorumComputed(RosterAuthoritativePayloadEmpty)

RosterProgressAnchors ==
  /\ ImplementationRosterStage(RosterAuthoritativeNonempty) = CollectingChunks
  /\ ~ImplementationRosterProgressed(RosterAuthoritativeNonempty)
  /\ ImplementationRosterStage(RosterAuthoritativePayloadEmpty) =
       AuthoritativePayload
  /\ ImplementationRosterProgressed(RosterAuthoritativePayloadEmpty)
  /\ ImplementationRosterStage(RosterInitPayload) = AuthoritativePayload
  /\ ImplementationRosterProgressed(RosterInitPayload)
  /\ ImplementationRosterStage(RosterDerivedReadySent) = LocalReadySent
  /\ ImplementationRosterProgressed(RosterDerivedReadySent)
  /\ ImplementationRosterStage(RosterDerivedDelivered) = Delivered
  /\ ImplementationRosterProgressed(RosterDerivedDelivered)

SafetyAnchors ==
  /\ AllAdvanceCasesMatchSpec
  /\ AllSyncCasesMatchSpec
  /\ AllRosterCasesMatchSpec
  /\ StageOrderAnchors
  /\ AdvanceForwardAnchors
  /\ AdvanceNoRegressionAnchors
  /\ SyncObservationAnchors
  /\ SyncNoRegressionAnchors
  /\ RosterQuorumAnchors
  /\ RosterProgressAnchors

====

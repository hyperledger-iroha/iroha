---- MODULE SumeragiNativeAmxJournalReplay ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for native AMX queue-plan journal replay.

This slice models the queue/Sumeragi restart boundary formed by
`QueuePlanJournal`, `QueuePlanJournalRecordV1`, `QueuePlanJournalFrameV1`, and
`RoutingPlan::NativeAmx`. Pending transactions are persisted with the full
routing plan, entrypoint, gossip payload, enqueue timestamp, transaction hash,
and routing-plan digest. Replay reconstructs live records by
`(signed_transaction_hash, plan_digest)`, so a tombstone for one digest cannot
delete a re-admitted transaction with the same compatibility hash and a new
native AMX plan. Opening the journal repairs incomplete tail frames while
preserving the last complete native AMX record.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

EmptyJournal == 1
NativePutReplay == 2
SinglePutReplay == 3
ParticipantOrder == 4
ParticipantDedup == 5
DigestPreserved == 6
GossipPayloadPreserved == 7
EntrypointPreserved == 8
RemoveExactDigest == 9
RemoveOtherDigest == 10
ReadmitSameHashNewDigest == 11
DuplicateSameKeyLastWins == 12
UnsupportedVersionIgnored == 13
CompactionKeepsLive == 14
CompactionDropsRemoved == 15
TornPayloadTailPreservesPrior == 16
TornLengthTailPreservesPrior == 17

Candidates == 1..17

NoBug == 0
DropNativePlanBug == 1
CollapseNativeToSingleBug == 2
SinglePlanAsNativeBug == 3
DropParticipantsBug == 4
ReorderParticipantsBug == 5
KeepDuplicateParticipantBug == 6
RecomputeDigestWrongBug == 7
DropGossipPayloadBug == 8
DropEntrypointBug == 9
RemoveByHashOnlyBug == 10
IgnoreExactRemoveBug == 11
ReplayUnsupportedVersionBug == 12
FirstPutWinsBug == 13
CompactionDropsLiveBug == 14
CompactionKeepsRemovedBug == 15
KeepTornTailBug == 16
DropPriorOnTailRepairBug == 17

Bugs == 0..17

BugDropNativePlan == Bug = DropNativePlanBug
BugCollapseNativeToSingle == Bug = CollapseNativeToSingleBug
BugSinglePlanAsNative == Bug = SinglePlanAsNativeBug
BugDropParticipants == Bug = DropParticipantsBug
BugReorderParticipants == Bug = ReorderParticipantsBug
BugKeepDuplicateParticipant == Bug = KeepDuplicateParticipantBug
BugRecomputeDigestWrong == Bug = RecomputeDigestWrongBug
BugDropGossipPayload == Bug = DropGossipPayloadBug
BugDropEntrypoint == Bug = DropEntrypointBug
BugRemoveByHashOnly == Bug = RemoveByHashOnlyBug
BugIgnoreExactRemove == Bug = IgnoreExactRemoveBug
BugReplayUnsupportedVersion == Bug = ReplayUnsupportedVersionBug
BugFirstPutWins == Bug = FirstPutWinsBug
BugCompactionDropsLive == Bug = CompactionDropsLiveBug
BugCompactionKeepsRemoved == Bug = CompactionKeepsRemovedBug
BugKeepTornTail == Bug = KeepTornTailBug
BugDropPriorOnTailRepair == Bug = DropPriorOnTailRepairBug

SpecLiveRecord(candidate) ==
  candidate \in {
    NativePutReplay,
    SinglePutReplay,
    ParticipantOrder,
    ParticipantDedup,
    DigestPreserved,
    GossipPayloadPreserved,
    EntrypointPreserved,
    RemoveOtherDigest,
    ReadmitSameHashNewDigest,
    DuplicateSameKeyLastWins,
    CompactionKeepsLive,
    TornPayloadTailPreservesPrior,
    TornLengthTailPreservesPrior
  }

SpecNativePlan(candidate) ==
  candidate \in {
    NativePutReplay,
    ParticipantOrder,
    ParticipantDedup,
    DigestPreserved,
    GossipPayloadPreserved,
    EntrypointPreserved,
    RemoveOtherDigest,
    ReadmitSameHashNewDigest,
    DuplicateSameKeyLastWins,
    CompactionKeepsLive,
    TornPayloadTailPreservesPrior,
    TornLengthTailPreservesPrior
  }

TailRepairCandidate(candidate) ==
  candidate \in {TornPayloadTailPreservesPrior, TornLengthTailPreservesPrior}

ImplementationLiveRecord(candidate) ==
  IF SpecLiveRecord(candidate)
  THEN
    /\ ~(SpecNativePlan(candidate) /\ BugDropNativePlan)
    /\ ~(candidate \in {RemoveOtherDigest, ReadmitSameHashNewDigest} /\ BugRemoveByHashOnly)
    /\ ~(candidate = CompactionKeepsLive /\ BugCompactionDropsLive)
    /\ ~(TailRepairCandidate(candidate) /\ BugDropPriorOnTailRepair)
  ELSE
    \/ /\ candidate = RemoveExactDigest
       /\ BugIgnoreExactRemove
    \/ /\ candidate = UnsupportedVersionIgnored
       /\ BugReplayUnsupportedVersion
    \/ /\ candidate = CompactionDropsRemoved
       /\ BugCompactionKeepsRemoved

ImplementationPlanIsNative(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ IF SpecNativePlan(candidate)
     THEN ~BugCollapseNativeToSingle
     ELSE BugSinglePlanAsNative

ImplementationParticipantSetComplete(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ SpecNativePlan(candidate)
  /\ ~BugDropParticipants

ImplementationParticipantOrderCanonical(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ SpecNativePlan(candidate)
  /\ ~BugReorderParticipants

ImplementationParticipantsDeduped(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ SpecNativePlan(candidate)
  /\ ~BugKeepDuplicateParticipant

ImplementationDigestPreserved(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ ~BugRecomputeDigestWrong

ImplementationGossipPayloadPreserved(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ ~BugDropGossipPayload

ImplementationEntrypointPreserved(candidate) ==
  /\ ImplementationLiveRecord(candidate)
  /\ ~BugDropEntrypoint

ImplementationLatestPutWins(candidate) ==
  /\ candidate = DuplicateSameKeyLastWins
  /\ ImplementationLiveRecord(candidate)
  /\ ~BugFirstPutWins

ImplementationTailRepaired(candidate) ==
  /\ TailRepairCandidate(candidate)
  /\ ImplementationLiveRecord(candidate)
  /\ ~BugKeepTornTail
  /\ ~BugDropPriorOnTailRepair

ImplementationCompactionEquivalent(candidate) ==
  /\ candidate \in {CompactionKeepsLive, CompactionDropsRemoved}
  /\ ~BugCompactionDropsLive
  /\ ~BugCompactionKeepsRemoved

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

ReplayMatchesSpec ==
  \A candidate \in tried:
    ImplementationLiveRecord(candidate) <=> SpecLiveRecord(candidate)

NativePlansStayNative ==
  \A candidate \in tried:
    (SpecNativePlan(candidate) /\ ImplementationLiveRecord(candidate))
      => ImplementationPlanIsNative(candidate)

SinglePlansStaySingle ==
  SinglePutReplay \in tried =>
    /\ ImplementationLiveRecord(SinglePutReplay)
    /\ ~ImplementationPlanIsNative(SinglePutReplay)

NativeParticipantsPreserved ==
  \A candidate \in tried:
    (SpecNativePlan(candidate) /\ ImplementationLiveRecord(candidate)) =>
      /\ ImplementationParticipantSetComplete(candidate)
      /\ ImplementationParticipantOrderCanonical(candidate)
      /\ ImplementationParticipantsDeduped(candidate)

RecordDigestPreserved ==
  \A candidate \in tried:
    ImplementationLiveRecord(candidate) => ImplementationDigestPreserved(candidate)

RecordPayloadPreserved ==
  \A candidate \in tried:
    ImplementationLiveRecord(candidate) =>
      /\ ImplementationGossipPayloadPreserved(candidate)
      /\ ImplementationEntrypointPreserved(candidate)

ExactDigestTombstonesAreScoped ==
  /\ (RemoveExactDigest \in tried => ~ImplementationLiveRecord(RemoveExactDigest))
  /\ (RemoveOtherDigest \in tried => ImplementationLiveRecord(RemoveOtherDigest))

SameHashNewDigestSurvives ==
  ReadmitSameHashNewDigest \in tried =>
    ImplementationLiveRecord(ReadmitSameHashNewDigest)

UnsupportedVersionsAreIgnored ==
  UnsupportedVersionIgnored \in tried =>
    ~ImplementationLiveRecord(UnsupportedVersionIgnored)

DuplicateSameKeyUsesLastPut ==
  DuplicateSameKeyLastWins \in tried =>
    ImplementationLatestPutWins(DuplicateSameKeyLastWins)

CompactionReplayEquivalent ==
  \A candidate \in tried:
    candidate \in {CompactionKeepsLive, CompactionDropsRemoved} =>
      ImplementationCompactionEquivalent(candidate)

TailRepairKeepsCompletePrefix ==
  \A candidate \in tried:
    TailRepairCandidate(candidate) => ImplementationTailRepaired(candidate)

Safety ==
  /\ TypeInvariant
  /\ ReplayMatchesSpec
  /\ NativePlansStayNative
  /\ SinglePlansStaySingle
  /\ NativeParticipantsPreserved
  /\ RecordDigestPreserved
  /\ RecordPayloadPreserved
  /\ ExactDigestTombstonesAreScoped
  /\ SameHashNewDigestSurvives
  /\ UnsupportedVersionsAreIgnored
  /\ DuplicateSameKeyUsesLastPut
  /\ CompactionReplayEquivalent
  /\ TailRepairKeepsCompletePrefix

====

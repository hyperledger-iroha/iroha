---- MODULE SumeragiFrontierBodyGapPayloadDrainUrgentGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `frontier_body_gap_payload_drain_urgent()`.

The helper raises payload-drain urgency only for an exact normal frontier slot
that is waiting for a body or commit QC, has no body yet, has vote-backed
quorum evidence, and has relevant payload/block backlog. Vote queue backlog is
not sufficient for this helper.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSlot == "no_slot"
NonNormalMode == "non_normal_mode"
ExactFetchDisarmed == "exact_fetch_disarmed"
BodyPresent == "body_present"
WrongPhase == "wrong_phase"
NoEvidenceWithBacklog == "no_evidence_with_backlog"
EvidenceNoPayloadBacklog == "evidence_no_payload_backlog"
EvidenceVoteBacklogOnly == "evidence_vote_backlog_only"
AwaitBodyVotesRbcChunk == "await_body_votes_rbc_chunk"
AwaitCommitQcCommitQcPayload == "await_commit_qc_commit_qc_payload"
AwaitBodySlotEvidenceBlock == "await_body_slot_evidence_block"
AwaitCommitQcAllEvidenceAllBacklog == "await_commit_qc_all_evidence_all_backlog"

Cases == {
  NoSlot,
  NonNormalMode,
  ExactFetchDisarmed,
  BodyPresent,
  WrongPhase,
  NoEvidenceWithBacklog,
  EvidenceNoPayloadBacklog,
  EvidenceVoteBacklogOnly,
  AwaitBodyVotesRbcChunk,
  AwaitCommitQcCommitQcPayload,
  AwaitBodySlotEvidenceBlock,
  AwaitCommitQcAllEvidenceAllBacklog
}

SlotPresentCases == Cases \ {NoSlot}
NormalModeCases == Cases \ {NonNormalMode}
ExactFetchArmedCases == Cases \ {ExactFetchDisarmed}
BodyAbsentCases == Cases \ {BodyPresent}
AllowedPhaseCases == Cases \ {WrongPhase}

VotesObservedCases == {
  AwaitBodyVotesRbcChunk,
  AwaitCommitQcAllEvidenceAllBacklog
}

CommitQcObservedCases == {
  AwaitCommitQcCommitQcPayload,
  AwaitCommitQcAllEvidenceAllBacklog
}

SlotVoteBackedEvidenceCases == {
  AwaitBodySlotEvidenceBlock,
  AwaitCommitQcAllEvidenceAllBacklog,
  EvidenceNoPayloadBacklog,
  EvidenceVoteBacklogOnly
}

RbcChunkBacklogCases == {
  AwaitBodyVotesRbcChunk,
  AwaitCommitQcAllEvidenceAllBacklog,
  NoEvidenceWithBacklog
}

PayloadBacklogCases == {
  AwaitCommitQcCommitQcPayload,
  AwaitCommitQcAllEvidenceAllBacklog
}

BlockBacklogCases == {
  AwaitBodySlotEvidenceBlock,
  AwaitCommitQcAllEvidenceAllBacklog
}

VoteBacklogOnlyCases == {EvidenceVoteBacklogOnly}

HasEvidence(c) ==
  c \in VotesObservedCases
    \/ c \in CommitQcObservedCases
    \/ c \in SlotVoteBackedEvidenceCases

HasRelevantBacklog(c) ==
  c \in RbcChunkBacklogCases
    \/ c \in PayloadBacklogCases
    \/ c \in BlockBacklogCases

SlotShapeOk(c) ==
  c \in SlotPresentCases
    /\ c \in NormalModeCases
    /\ c \in ExactFetchArmedCases
    /\ c \in BodyAbsentCases
    /\ c \in AllowedPhaseCases

SpecUrgent(c) ==
  SlotShapeOk(c) /\ HasEvidence(c) /\ HasRelevantBacklog(c)

ReturnUrgent == 1
ReturnNotUrgent == 2
CheckSlot == 3
RejectNoSlot == 4
CheckMode == 5
RejectNonNormalMode == 6
CheckExactFetch == 7
RejectDisarmed == 8
CheckBodyPresent == 9
RejectBodyPresent == 10
CheckPhase == 11
RejectWrongPhase == 12
CheckEvidence == 13
RejectNoEvidence == 14
EvidenceVotesObserved == 15
EvidenceCommitQcObserved == 16
EvidenceSlotVoteBacked == 17
CheckBacklog == 18
RejectNoBacklog == 19
BacklogRbcChunk == 20
BacklogPayload == 21
BacklogBlock == 22
RejectVoteBacklogOnly == 23

ActionUniverse == 1..23

EvidenceActions(c) ==
  (IF c \in VotesObservedCases THEN {EvidenceVotesObserved} ELSE {})
    \cup (IF c \in CommitQcObservedCases
          THEN {EvidenceCommitQcObserved}
          ELSE {})
    \cup (IF c \in SlotVoteBackedEvidenceCases
          THEN {EvidenceSlotVoteBacked}
          ELSE {})

BacklogActions(c) ==
  (IF c \in RbcChunkBacklogCases THEN {BacklogRbcChunk} ELSE {})
    \cup (IF c \in PayloadBacklogCases THEN {BacklogPayload} ELSE {})
    \cup (IF c \in BlockBacklogCases THEN {BacklogBlock} ELSE {})

SpecActions(c) ==
  {CheckSlot}
    \cup (IF SpecUrgent(c) THEN {ReturnUrgent} ELSE {ReturnNotUrgent})
    \cup (IF c \notin SlotPresentCases THEN {RejectNoSlot} ELSE {CheckMode})
    \cup (IF c \in SlotPresentCases /\ c \notin NormalModeCases
          THEN {RejectNonNormalMode}
          ELSE {})
    \cup (IF c \in SlotPresentCases /\ c \in NormalModeCases
          THEN {CheckExactFetch}
          ELSE {})
    \cup (IF c \in SlotPresentCases /\ c \in NormalModeCases
              /\ c \notin ExactFetchArmedCases
          THEN {RejectDisarmed}
          ELSE {})
    \cup (IF c \in SlotPresentCases /\ c \in NormalModeCases
              /\ c \in ExactFetchArmedCases
          THEN {CheckBodyPresent}
          ELSE {})
    \cup (IF c \in SlotPresentCases /\ c \in NormalModeCases
              /\ c \in ExactFetchArmedCases /\ c \notin BodyAbsentCases
          THEN {RejectBodyPresent}
          ELSE {})
    \cup (IF c \in SlotPresentCases /\ c \in NormalModeCases
              /\ c \in ExactFetchArmedCases /\ c \in BodyAbsentCases
          THEN {CheckPhase}
          ELSE {})
    \cup (IF c \in SlotPresentCases /\ c \in NormalModeCases
              /\ c \in ExactFetchArmedCases /\ c \in BodyAbsentCases
              /\ c \notin AllowedPhaseCases
          THEN {RejectWrongPhase}
          ELSE {})
    \cup (IF SlotShapeOk(c) THEN {CheckEvidence} ELSE {})
    \cup (IF SlotShapeOk(c) /\ ~HasEvidence(c)
          THEN {RejectNoEvidence}
          ELSE {})
    \cup (IF SlotShapeOk(c) /\ HasEvidence(c)
          THEN EvidenceActions(c) \cup {CheckBacklog}
          ELSE {})
    \cup (IF SlotShapeOk(c) /\ HasEvidence(c) /\ ~HasRelevantBacklog(c)
          THEN {RejectNoBacklog}
          ELSE {})
    \cup (IF SlotShapeOk(c) /\ HasEvidence(c) /\ c \in VoteBacklogOnlyCases
          THEN {RejectVoteBacklogOnly}
          ELSE {})
    \cup (IF SlotShapeOk(c) /\ HasEvidence(c) /\ HasRelevantBacklog(c)
          THEN BacklogActions(c)
          ELSE {})

ImplementationUrgent(c) ==
  CASE Bug = "accept_no_slot"
       /\ c = NoSlot ->
      TRUE
    [] Bug = "accept_non_normal_mode"
       /\ c = NonNormalMode ->
      TRUE
    [] Bug = "accept_disarmed"
       /\ c = ExactFetchDisarmed ->
      TRUE
    [] Bug = "accept_body_present"
       /\ c = BodyPresent ->
      TRUE
    [] Bug = "accept_wrong_phase"
       /\ c = WrongPhase ->
      TRUE
    [] Bug = "reject_votes_evidence"
       /\ c = AwaitBodyVotesRbcChunk ->
      FALSE
    [] Bug = "reject_commit_qc_evidence"
       /\ c = AwaitCommitQcCommitQcPayload ->
      FALSE
    [] Bug = "reject_slot_evidence"
       /\ c = AwaitBodySlotEvidenceBlock ->
      FALSE
    [] Bug = "accept_no_evidence"
       /\ c = NoEvidenceWithBacklog ->
      TRUE
    [] Bug = "accept_no_backlog"
       /\ c = EvidenceNoPayloadBacklog ->
      TRUE
    [] Bug = "accept_vote_backlog_only"
       /\ c = EvidenceVoteBacklogOnly ->
      TRUE
    [] Bug = "ignore_rbc_chunk_backlog"
       /\ c = AwaitBodyVotesRbcChunk ->
      FALSE
    [] Bug = "ignore_payload_backlog"
       /\ c = AwaitCommitQcCommitQcPayload ->
      FALSE
    [] Bug = "ignore_block_backlog"
       /\ c = AwaitBodySlotEvidenceBlock ->
      FALSE
    [] Bug = "require_all_backlogs"
       /\ c \in {AwaitBodyVotesRbcChunk, AwaitCommitQcCommitQcPayload,
                 AwaitBodySlotEvidenceBlock} ->
      FALSE
    [] Bug = "require_all_evidence"
       /\ c \in {AwaitBodyVotesRbcChunk, AwaitCommitQcCommitQcPayload,
                 AwaitBodySlotEvidenceBlock} ->
      FALSE
    [] OTHER -> SpecUrgent(c)

ImplementationActions(c) ==
  (SpecActions(c) \ {ReturnUrgent, ReturnNotUrgent})
    \cup (IF ImplementationUrgent(c)
          THEN {ReturnUrgent}
          ELSE {ReturnNotUrgent})

Bugs == {
  "none",
  "accept_no_slot",
  "accept_non_normal_mode",
  "accept_disarmed",
  "accept_body_present",
  "accept_wrong_phase",
  "reject_votes_evidence",
  "reject_commit_qc_evidence",
  "reject_slot_evidence",
  "accept_no_evidence",
  "accept_no_backlog",
  "accept_vote_backlog_only",
  "ignore_rbc_chunk_backlog",
  "ignore_payload_backlog",
  "ignore_block_backlog",
  "require_all_backlogs",
  "require_all_evidence"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecUrgent(c) \in BOOLEAN
       /\ ImplementationUrgent(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationUrgent(c) = SpecUrgent(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SlotShapeRequired ==
  /\ ImplementationUrgent(NoSlot) = FALSE
  /\ RejectNoSlot \in ImplementationActions(NoSlot)
  /\ ~(CheckMode \in ImplementationActions(NoSlot))
  /\ ImplementationUrgent(NonNormalMode) = FALSE
  /\ RejectNonNormalMode \in ImplementationActions(NonNormalMode)
  /\ ~(CheckExactFetch \in ImplementationActions(NonNormalMode))
  /\ ImplementationUrgent(ExactFetchDisarmed) = FALSE
  /\ RejectDisarmed \in ImplementationActions(ExactFetchDisarmed)
  /\ ImplementationUrgent(BodyPresent) = FALSE
  /\ RejectBodyPresent \in ImplementationActions(BodyPresent)
  /\ ImplementationUrgent(WrongPhase) = FALSE
  /\ RejectWrongPhase \in ImplementationActions(WrongPhase)
  /\ ~(CheckEvidence \in ImplementationActions(WrongPhase))

EachEvidenceSourceCanDriveUrgency ==
  /\ ImplementationUrgent(AwaitBodyVotesRbcChunk) = TRUE
  /\ EvidenceVotesObserved \in ImplementationActions(AwaitBodyVotesRbcChunk)
  /\ ImplementationUrgent(AwaitCommitQcCommitQcPayload) = TRUE
  /\ EvidenceCommitQcObserved
       \in ImplementationActions(AwaitCommitQcCommitQcPayload)
  /\ ImplementationUrgent(AwaitBodySlotEvidenceBlock) = TRUE
  /\ EvidenceSlotVoteBacked
       \in ImplementationActions(AwaitBodySlotEvidenceBlock)
  /\ ImplementationUrgent(AwaitCommitQcAllEvidenceAllBacklog) = TRUE

EvidenceRequiredBeforeBacklog ==
  /\ ImplementationUrgent(NoEvidenceWithBacklog) = FALSE
  /\ RejectNoEvidence \in ImplementationActions(NoEvidenceWithBacklog)
  /\ ~(CheckBacklog \in ImplementationActions(NoEvidenceWithBacklog))

RelevantBacklogRequired ==
  /\ ImplementationUrgent(EvidenceNoPayloadBacklog) = FALSE
  /\ RejectNoBacklog \in ImplementationActions(EvidenceNoPayloadBacklog)
  /\ ImplementationUrgent(EvidenceVoteBacklogOnly) = FALSE
  /\ RejectVoteBacklogOnly \in ImplementationActions(EvidenceVoteBacklogOnly)
  /\ ~(BacklogRbcChunk \in ImplementationActions(EvidenceVoteBacklogOnly))
  /\ ~(BacklogPayload \in ImplementationActions(EvidenceVoteBacklogOnly))
  /\ ~(BacklogBlock \in ImplementationActions(EvidenceVoteBacklogOnly))

AnyPayloadBacklogCanDriveUrgency ==
  /\ ImplementationUrgent(AwaitBodyVotesRbcChunk) = TRUE
  /\ BacklogRbcChunk \in ImplementationActions(AwaitBodyVotesRbcChunk)
  /\ ImplementationUrgent(AwaitCommitQcCommitQcPayload) = TRUE
  /\ BacklogPayload \in ImplementationActions(AwaitCommitQcCommitQcPayload)
  /\ ImplementationUrgent(AwaitBodySlotEvidenceBlock) = TRUE
  /\ BacklogBlock \in ImplementationActions(AwaitBodySlotEvidenceBlock)
  /\ ImplementationUrgent(AwaitCommitQcAllEvidenceAllBacklog) = TRUE

FrontierBodyGapPayloadDrainUrgentCoreSafety ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ SlotShapeRequired
  /\ EachEvidenceSourceCanDriveUrgency
  /\ EvidenceRequiredBeforeBacklog
  /\ RelevantBacklogRequired
  /\ AnyPayloadBacklogCanDriveUrgency

NoBugInvariant == FrontierBodyGapPayloadDrainUrgentCoreSafety

SafetyFast == FrontierBodyGapPayloadDrainUrgentCoreSafety

FrontierBodyGapPayloadDrainUrgentExactness ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ SlotShapeRequired
  /\ EachEvidenceSourceCanDriveUrgency
  /\ EvidenceRequiredBeforeBacklog
  /\ RelevantBacklogRequired
  /\ AnyPayloadBacklogCanDriveUrgency
FrontierBodyGapPayloadDrainUrgentCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FrontierBodyGapPayloadDrainUrgentExactness

====

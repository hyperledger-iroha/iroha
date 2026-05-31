---- MODULE SumeragiIsolatedVoteBackedHandoffGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for
`maybe_handoff_isolated_vote_backed_frontier_to_anchor(...)`.

The helper is a narrow recovery gate for a one-vote next-height frontier owner.
It may seed quorum-timeout frontier recovery, replay the body-available
frontier-slot event, validate the resulting slot, and request a committed-anchor
range pull. This model pins the local safety contract:
- resilience must be enabled,
- the block must have exactly one vote and still be below commit quorum,
- the height must be exactly the committed-height successor,
- cached commit QC suppresses the handoff,
- recovery/body-available side effects happen only after admission,
- the resulting frontier slot must still match height/view/hash, have a body,
  lack commit QC, and retain vote-backed owner state, and
- a true return requires the anchor range-pull request to succeed with the
  `frontier_stall_reset_fallback` reason.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DisabledResilience == "DisabledResilience"
ZeroVotes == "ZeroVotes"
MultipleVotes == "MultipleVotes"
AtQuorum == "AtQuorum"
StaleHeight == "StaleHeight"
FutureHeight == "FutureHeight"
CachedCommitQc == "CachedCommitQc"
HappyPath == "HappyPath"
NoSlotAfterSeed == "NoSlotAfterSeed"
WrongSlotHeight == "WrongSlotHeight"
WrongSlotView == "WrongSlotView"
WrongSlotHash == "WrongSlotHash"
MissingBody == "MissingBody"
CommitQcObserved == "CommitQcObserved"
NoVoteBackedOwner == "NoVoteBackedOwner"
RangePullRejected == "RangePullRejected"

Cases == {
  DisabledResilience,
  ZeroVotes,
  MultipleVotes,
  AtQuorum,
  StaleHeight,
  FutureHeight,
  CachedCommitQc,
  HappyPath,
  NoSlotAfterSeed,
  WrongSlotHeight,
  WrongSlotView,
  WrongSlotHash,
  MissingBody,
  CommitQcObserved,
  NoVoteBackedOwner,
  RangePullRejected
}

BoolToInt(b) == IF b THEN 1 ELSE 0

MinVotes(c) == 3

ResilienceEnabled(c) ==
  c /= DisabledResilience

VoteCount(c) ==
  CASE c = ZeroVotes -> 0
    [] c = MultipleVotes -> 2
    [] c = AtQuorum -> 3
    [] OTHER -> 1

CommittedHeight(c) == 10

Height(c) ==
  CASE c = StaleHeight -> 10
    [] c = FutureHeight -> 12
    [] OTHER -> 11

NextHeight(c) ==
  Height(c) = CommittedHeight(c) + 1

CachedQc(c) ==
  c = CachedCommitQc

SpecAdmission(c) ==
  /\ ResilienceEnabled(c)
  /\ VoteCount(c) = 1
  /\ VoteCount(c) < MinVotes(c)
  /\ NextHeight(c)
  /\ ~CachedQc(c)

SpecSeedsRecovery(c) ==
  SpecAdmission(c)

SpecBodyEvent(c) ==
  SpecAdmission(c)

SlotPresent(c) ==
  c /= NoSlotAfterSeed

SlotHeightMatches(c) ==
  c /= WrongSlotHeight

SlotViewMatches(c) ==
  c /= WrongSlotView

SlotHashMatches(c) ==
  c /= WrongSlotHash

SlotBodyPresent(c) ==
  c /= MissingBody

SlotCommitQcObserved(c) ==
  c = CommitQcObserved

VoteBackedOwnerState(c) ==
  c /= NoVoteBackedOwner

SpecSlotValid(c) ==
  /\ SlotPresent(c)
  /\ SlotHeightMatches(c)
  /\ SlotViewMatches(c)
  /\ SlotHashMatches(c)
  /\ SlotBodyPresent(c)
  /\ ~SlotCommitQcObserved(c)
  /\ VoteBackedOwnerState(c)

RangePullSucceeds(c) ==
  c /= RangePullRejected

SpecRequestsAnchor(c) ==
  /\ SpecAdmission(c)
  /\ SpecSlotValid(c)

SpecReasonOk(c) ==
  TRUE

SpecAction(c) ==
  /\ SpecRequestsAnchor(c)
  /\ RangePullSucceeds(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<BoolToInt(SpecSeedsRecovery(c)), BoolToInt(SpecBodyEvent(c)),
    BoolToInt(SpecRequestsAnchor(c)), BoolToInt(SpecAction(c)),
    BoolToInt(SpecReasonOk(c))>>

ActualAdmission(c) ==
  CASE Bug = "disabled_allows" /\ c = DisabledResilience -> TRUE
    [] Bug = "zero_votes_allow" /\ c = ZeroVotes -> TRUE
    [] Bug = "multiple_votes_allow" /\ c = MultipleVotes -> TRUE
    [] Bug = "quorum_votes_allow" /\ c = AtQuorum -> TRUE
    [] Bug = "stale_height_allow" /\ c = StaleHeight -> TRUE
    [] Bug = "future_height_allow" /\ c = FutureHeight -> TRUE
    [] Bug = "cached_qc_allow" /\ c = CachedCommitQc -> TRUE
    [] OTHER -> SpecAdmission(c)

ActualSeedsRecovery(c) ==
  CASE Bug = "seed_skipped" /\ c = HappyPath -> FALSE
    [] OTHER -> ActualAdmission(c)

ActualBodyEvent(c) ==
  CASE Bug = "body_event_skipped" /\ c = HappyPath -> FALSE
    [] OTHER -> ActualAdmission(c)

ActualSlotValid(c) ==
  CASE Bug = "no_slot_allows" /\ c = NoSlotAfterSeed -> TRUE
    [] Bug = "wrong_height_allows" /\ c = WrongSlotHeight -> TRUE
    [] Bug = "wrong_view_allows" /\ c = WrongSlotView -> TRUE
    [] Bug = "wrong_hash_allows" /\ c = WrongSlotHash -> TRUE
    [] Bug = "missing_body_allows" /\ c = MissingBody -> TRUE
    [] Bug = "commit_qc_observed_allows" /\ c = CommitQcObserved -> TRUE
    [] Bug = "no_vote_owner_allows" /\ c = NoVoteBackedOwner -> TRUE
    [] OTHER -> SpecSlotValid(c)

ActualRequestsAnchor(c) ==
  CASE Bug = "request_skipped" /\ c = HappyPath -> FALSE
    [] OTHER ->
       /\ ActualAdmission(c)
       /\ ActualSlotValid(c)

ActualReasonOk(c) ==
  CASE Bug = "wrong_reason" /\ c = HappyPath -> FALSE
    [] OTHER -> TRUE

ActualAction(c) ==
  CASE Bug = "request_failure_returns_true" /\ c = RangePullRejected -> TRUE
    [] OTHER ->
       /\ ActualRequestsAnchor(c)
       /\ RangePullSucceeds(c)
       /\ ActualReasonOk(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<BoolToInt(ActualSeedsRecovery(c)), BoolToInt(ActualBodyEvent(c)),
    BoolToInt(ActualRequestsAnchor(c)), BoolToInt(ActualAction(c)),
    BoolToInt(ActualReasonOk(c))>>

BugSet == {
  "none",
  "disabled_allows",
  "zero_votes_allow",
  "multiple_votes_allow",
  "quorum_votes_allow",
  "stale_height_allow",
  "future_height_allow",
  "cached_qc_allow",
  "seed_skipped",
  "body_event_skipped",
  "no_slot_allows",
  "wrong_height_allows",
  "wrong_view_allows",
  "wrong_hash_allows",
  "missing_body_allows",
  "commit_qc_observed_allows",
  "no_vote_owner_allows",
  "request_skipped",
  "request_failure_returns_true",
  "wrong_reason"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0

SelectionExact ==
  \A c \in Cases:
    ActualOutput(c) = SpecOutput(c)

AdmissionStable ==
  /\ SpecOutput(DisabledResilience)[4] = 0
  /\ SpecOutput(ZeroVotes)[4] = 0
  /\ SpecOutput(MultipleVotes)[4] = 0
  /\ SpecOutput(AtQuorum)[4] = 0
  /\ SpecOutput(StaleHeight)[4] = 0
  /\ SpecOutput(FutureHeight)[4] = 0
  /\ SpecOutput(CachedCommitQc)[4] = 0
  /\ SpecOutput(HappyPath)[1] = 1
  /\ SpecOutput(HappyPath)[2] = 1
  /\ SpecOutput(HappyPath)[3] = 1
  /\ SpecOutput(HappyPath)[4] = 1

SlotValidationStable ==
  /\ SpecOutput(NoSlotAfterSeed)[4] = 0
  /\ SpecOutput(WrongSlotHeight)[4] = 0
  /\ SpecOutput(WrongSlotView)[4] = 0
  /\ SpecOutput(WrongSlotHash)[4] = 0
  /\ SpecOutput(MissingBody)[4] = 0
  /\ SpecOutput(CommitQcObserved)[4] = 0
  /\ SpecOutput(NoVoteBackedOwner)[4] = 0

RangePullStable ==
  /\ SpecOutput(RangePullRejected)[3] = 1
  /\ SpecOutput(RangePullRejected)[4] = 0
  /\ SpecOutput(HappyPath)[5] = 1

SafetyFast ==
  /\ SelectionExact
  /\ AdmissionStable
  /\ SlotValidationStable
  /\ RangePullStable

Safety ==
  SafetyFast

=============================================================================

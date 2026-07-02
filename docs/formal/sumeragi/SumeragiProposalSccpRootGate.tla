---- MODULE SumeragiProposalSccpRootGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for SCCP proposal commitment-root construction in
`main_loop/propose.rs`.

This slice pins the merge-sensitive proposal path:
- candidate SCCP messages are considered only when Nexus is enabled, the
  proposal route is active at the proposal height, and the message has not
  already been recorded;
- final committed SCCP messages are filtered by signed entrypoint availability,
  candidate execution success, and any prior regular transaction effects, with
  ordered preflight used as the bounded helper-test abstraction;
- execution-derived SCCP roots must be stable across the probe/fixed-point pass;
- the signed block root is the execution-derived stable root exactly when a
  committable SCCP message remains after those gates.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "nexus_disabled_candidate",
  "inactive_route_candidate",
  "already_recorded_candidate",
  "active_signed_ok",
  "unsigned_candidate",
  "candidate_preflight_reject",
  "regular_before_candidate_ok",
  "regular_before_candidate_fails",
  "candidate_no_message",
  "stable_root_ok",
  "unstable_root"
}

Bugs == {
  "none",
  "ignore_nexus_disabled",
  "ignore_inactive_route",
  "ignore_recorded_filter",
  "include_unsigned_candidate",
  "include_preflight_reject",
  "skip_ordered_preflight",
  "skip_stable_root_check",
  "root_from_raw_candidates",
  "root_omits_committable"
}

NexusEnabled(c) ==
  c # "nexus_disabled_candidate"

ActiveRoute(c) ==
  c # "inactive_route_candidate"

AlreadyRecorded(c) ==
  c = "already_recorded_candidate"

CandidateMessage(c) ==
  c # "candidate_no_message"

SignedEntrypoint(c) ==
  c # "unsigned_candidate"

CandidatePreflightOk(c) ==
  c # "candidate_preflight_reject"

HasPriorRegular(c) ==
  c \in {"regular_before_candidate_ok", "regular_before_candidate_fails"}

PriorRegularPreflightOk(c) ==
  c # "regular_before_candidate_fails"

OrderedPreflightGate(c) ==
  ~HasPriorRegular(c) \/ PriorRegularPreflightOk(c)

SpecMayRecord(c) ==
  /\ NexusEnabled(c)
  /\ ActiveRoute(c)
  /\ ~AlreadyRecorded(c)
  /\ CandidateMessage(c)

SpecCommittable(c) ==
  /\ SpecMayRecord(c)
  /\ SignedEntrypoint(c)
  /\ CandidatePreflightOk(c)
  /\ OrderedPreflightGate(c)

ExecutionInitialRoot(c) ==
  IF c = "unstable_root" THEN 1 ELSE 2

ExecutionStableRoot(c) ==
  IF c = "unstable_root" THEN 3 ELSE 2

SpecExecutionRootStable(c) ==
  ExecutionInitialRoot(c) = ExecutionStableRoot(c)

SpecProposalSucceeds(c) ==
  ~SpecMayRecord(c) \/ SpecExecutionRootStable(c)

SpecFinalRootPresent(c) ==
  /\ SpecProposalSucceeds(c)
  /\ SpecCommittable(c)

ActualMayRecord(c) ==
  CASE Bug = "ignore_nexus_disabled" ->
       /\ ActiveRoute(c)
       /\ ~AlreadyRecorded(c)
       /\ CandidateMessage(c)
    [] Bug = "ignore_inactive_route" ->
       /\ NexusEnabled(c)
       /\ ~AlreadyRecorded(c)
       /\ CandidateMessage(c)
    [] Bug = "ignore_recorded_filter" ->
       /\ NexusEnabled(c)
       /\ ActiveRoute(c)
       /\ CandidateMessage(c)
    [] OTHER -> SpecMayRecord(c)

ActualCommittable(c) ==
  CASE Bug = "include_unsigned_candidate" ->
       /\ ActualMayRecord(c)
       /\ CandidatePreflightOk(c)
       /\ OrderedPreflightGate(c)
    [] Bug = "include_preflight_reject" ->
       /\ ActualMayRecord(c)
       /\ SignedEntrypoint(c)
       /\ OrderedPreflightGate(c)
    [] Bug = "skip_ordered_preflight" ->
       /\ ActualMayRecord(c)
       /\ SignedEntrypoint(c)
       /\ CandidatePreflightOk(c)
    [] OTHER ->
       /\ ActualMayRecord(c)
       /\ SignedEntrypoint(c)
       /\ CandidatePreflightOk(c)
       /\ OrderedPreflightGate(c)

ActualExecutionRootStable(c) ==
  CASE Bug = "skip_stable_root_check" -> TRUE
    [] OTHER -> SpecExecutionRootStable(c)

ActualProposalSucceeds(c) ==
  ~ActualMayRecord(c) \/ ActualExecutionRootStable(c)

ActualFinalRootPresent(c) ==
  CASE Bug = "root_from_raw_candidates" ->
       /\ ActualProposalSucceeds(c)
       /\ ActualMayRecord(c)
    [] Bug = "root_omits_committable" -> FALSE
    [] OTHER ->
       /\ ActualProposalSucceeds(c)
       /\ ActualCommittable(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ ActualMayRecord(c) \in BOOLEAN
       /\ ActualCommittable(c) \in BOOLEAN
       /\ ActualProposalSucceeds(c) \in BOOLEAN
       /\ ActualFinalRootPresent(c) \in BOOLEAN

MayRecordMatchesSpec ==
  \A c \in Cases:
    ActualMayRecord(c) = SpecMayRecord(c)

CommittableMatchesSpec ==
  \A c \in Cases:
    ActualCommittable(c) = SpecCommittable(c)

StableRootGateMatchesSpec ==
  \A c \in Cases:
    ActualProposalSucceeds(c) = SpecProposalSucceeds(c)

FinalRootMatchesSpec ==
  \A c \in Cases:
    ActualFinalRootPresent(c) = SpecFinalRootPresent(c)

DisabledInactiveAndRecordedCandidatesFiltered ==
  /\ ~ActualMayRecord("nexus_disabled_candidate")
  /\ ~ActualMayRecord("inactive_route_candidate")
  /\ ~ActualMayRecord("already_recorded_candidate")

UnsignedOrFailedCandidateFiltered ==
  /\ ~ActualCommittable("unsigned_candidate")
  /\ ~ActualCommittable("candidate_preflight_reject")

OrderedPreflightDependencyRespected ==
  /\ ActualCommittable("regular_before_candidate_ok")
  /\ ~ActualCommittable("regular_before_candidate_fails")

UnstableExecutionRootStopsProposal ==
  ~ActualProposalSucceeds("unstable_root")

FinalRootRequiresCommittable ==
  \A c \in Cases:
    ActualFinalRootPresent(c) => ActualCommittable(c)

CommittableProducesFinalRootOnSuccess ==
  \A c \in Cases:
    (ActualProposalSucceeds(c) /\ ActualCommittable(c)) =>
      ActualFinalRootPresent(c)

NoCandidateMessageHasNoRoot ==
  /\ ~ActualMayRecord("candidate_no_message")
  /\ ~ActualFinalRootPresent("candidate_no_message")

ProposalSccpRootExactness ==
  /\ MayRecordMatchesSpec
  /\ CommittableMatchesSpec
  /\ StableRootGateMatchesSpec
  /\ FinalRootMatchesSpec
  /\ DisabledInactiveAndRecordedCandidatesFiltered
  /\ UnsignedOrFailedCandidateFiltered
  /\ OrderedPreflightDependencyRespected
  /\ UnstableExecutionRootStopsProposal
  /\ FinalRootRequiresCommittable
  /\ CommittableProducesFinalRootOnSuccess
  /\ NoCandidateMessageHasNoRoot

Safety ==
  ProposalSccpRootExactness

ProposalSccpRootCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProposalSccpRootExactness

=============================================================================
====

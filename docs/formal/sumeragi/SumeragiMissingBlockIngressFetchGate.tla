---- MODULE SumeragiMissingBlockIngressFetchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for missing-block ingress fetch gating.

This slice captures `missing_block_ingress_fetch_gate(...)`. The helper gates
network fetch emission for exact-frontier missing bodies while an authoritative
body may still arrive through consensus ingress. The contract is:

- locally authoritative payloads and non-frontier heights bypass the gate
  without recording a new missing-block request;
- an initial exact-frontier missing body inside the ingress grace window records
  the request and returns `Hold`;
- the same initial request at or after the grace boundary returns `Fetch` with a
  one-shot `force_retry_now` flag, so the follow-up fetch planner bypasses its
  retry window exactly once;
- existing requests never receive the one-shot force flag and are not held by
  the ingress grace gate.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PayloadAvailable == "payload_available"
NonFrontierHeight == "non_frontier_height"
InitialWithinGrace == "initial_within_grace"
InitialAtGraceBoundary == "initial_at_grace_boundary"
InitialAfterGrace == "initial_after_grace"
ExistingWithinGrace == "existing_within_grace"
ExistingAfterGrace == "existing_after_grace"

Cases == {
  PayloadAvailable,
  NonFrontierHeight,
  InitialWithinGrace,
  InitialAtGraceBoundary,
  InitialAfterGrace,
  ExistingWithinGrace,
  ExistingAfterGrace
}

ReturnHold == 1
ReturnFetch == 2
ForceRetryNow == 3
NoForceRetry == 4
ObserveRequest == 5
NoObserveRequest == 6
AuthoritativePayloadBypass == 7
NonFrontierBypass == 8
InitialAttempt == 9
ExistingAttempt == 10
WithinGrace == 11
GraceElapsed == 12
RequestAttemptsZero == 13
NoNetworkFetchWhileHeld == 14

ActionUniverse == 1..14

FetchNoObserveActions ==
  {ReturnFetch, NoForceRetry, NoObserveRequest}

SpecActions(c) ==
  CASE c = PayloadAvailable ->
      FetchNoObserveActions \cup {AuthoritativePayloadBypass}
    [] c = NonFrontierHeight ->
      FetchNoObserveActions \cup {NonFrontierBypass}
    [] c = InitialWithinGrace ->
      {ReturnHold, ObserveRequest, InitialAttempt, WithinGrace,
       RequestAttemptsZero, NoNetworkFetchWhileHeld}
    [] c = InitialAtGraceBoundary ->
      {ReturnFetch, ForceRetryNow, ObserveRequest, InitialAttempt,
       GraceElapsed, RequestAttemptsZero}
    [] c = InitialAfterGrace ->
      {ReturnFetch, ForceRetryNow, ObserveRequest, InitialAttempt,
       GraceElapsed, RequestAttemptsZero}
    [] c = ExistingWithinGrace ->
      {ReturnFetch, NoForceRetry, ObserveRequest, ExistingAttempt,
       WithinGrace}
    [] c = ExistingAfterGrace ->
      {ReturnFetch, NoForceRetry, ObserveRequest, ExistingAttempt,
       GraceElapsed}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "hold_payload_available"
       /\ c = PayloadAvailable ->
      (spec \ {ReturnFetch, NoForceRetry, NoObserveRequest}) \cup
        {ReturnHold, ObserveRequest, NoNetworkFetchWhileHeld}
    [] Bug = "observe_payload_available"
       /\ c = PayloadAvailable ->
      (spec \ {NoObserveRequest}) \cup {ObserveRequest}
    [] Bug = "hold_non_frontier"
       /\ c = NonFrontierHeight ->
      (spec \ {ReturnFetch, NoForceRetry, NoObserveRequest}) \cup
        {ReturnHold, ObserveRequest, NoNetworkFetchWhileHeld}
    [] Bug = "observe_non_frontier"
       /\ c = NonFrontierHeight ->
      (spec \ {NoObserveRequest}) \cup {ObserveRequest}
    [] Bug = "fetch_initial_within_grace"
       /\ c = InitialWithinGrace ->
      (spec \ {ReturnHold, NoNetworkFetchWhileHeld}) \cup
        {ReturnFetch, NoForceRetry}
    [] Bug = "skip_initial_observe"
       /\ c = InitialWithinGrace ->
      spec \ {ObserveRequest, RequestAttemptsZero}
    [] Bug = "force_while_held"
       /\ c = InitialWithinGrace ->
      spec \cup {ForceRetryNow}
    [] Bug = "hold_at_grace_boundary"
       /\ c = InitialAtGraceBoundary ->
      (spec \ {ReturnFetch, ForceRetryNow}) \cup
        {ReturnHold, NoNetworkFetchWhileHeld}
    [] Bug = "skip_force_after_grace"
       /\ c = InitialAfterGrace ->
      (spec \ {ForceRetryNow}) \cup {NoForceRetry}
    [] Bug = "force_existing_within_grace"
       /\ c = ExistingWithinGrace ->
      (spec \ {NoForceRetry}) \cup {ForceRetryNow}
    [] Bug = "hold_existing_within_grace"
       /\ c = ExistingWithinGrace ->
      (spec \ {ReturnFetch, NoForceRetry}) \cup
        {ReturnHold, NoNetworkFetchWhileHeld}
    [] Bug = "force_existing_after_grace"
       /\ c = ExistingAfterGrace ->
      (spec \ {NoForceRetry}) \cup {ForceRetryNow}
    [] OTHER -> spec

Bugs == {
  "none",
  "hold_payload_available",
  "observe_payload_available",
  "hold_non_frontier",
  "observe_non_frontier",
  "fetch_initial_within_grace",
  "skip_initial_observe",
  "force_while_held",
  "hold_at_grace_boundary",
  "skip_force_after_grace",
  "force_existing_within_grace",
  "hold_existing_within_grace",
  "force_existing_after_grace"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

BypassCasesDoNotHoldOrMutateRequests ==
  \A c \in {PayloadAvailable, NonFrontierHeight}:
    /\ ReturnFetch \in ImplementationActions(c)
    /\ NoForceRetry \in ImplementationActions(c)
    /\ NoObserveRequest \in ImplementationActions(c)
    /\ ~(ReturnHold \in ImplementationActions(c))
    /\ ~(ObserveRequest \in ImplementationActions(c))

InitialFrontierWithinGraceHoldsAndRecords ==
  /\ ReturnHold \in ImplementationActions(InitialWithinGrace)
  /\ ObserveRequest \in ImplementationActions(InitialWithinGrace)
  /\ RequestAttemptsZero \in ImplementationActions(InitialWithinGrace)
  /\ NoNetworkFetchWhileHeld \in ImplementationActions(InitialWithinGrace)
  /\ ~(ReturnFetch \in ImplementationActions(InitialWithinGrace))
  /\ ~(ForceRetryNow \in ImplementationActions(InitialWithinGrace))

InitialFrontierAtOrAfterGraceForcesOnce ==
  \A c \in {InitialAtGraceBoundary, InitialAfterGrace}:
    /\ ReturnFetch \in ImplementationActions(c)
    /\ ForceRetryNow \in ImplementationActions(c)
    /\ ObserveRequest \in ImplementationActions(c)
    /\ RequestAttemptsZero \in ImplementationActions(c)
    /\ ~(ReturnHold \in ImplementationActions(c))

ExistingRequestsNeverForceOrHold ==
  \A c \in {ExistingWithinGrace, ExistingAfterGrace}:
    /\ ReturnFetch \in ImplementationActions(c)
    /\ NoForceRetry \in ImplementationActions(c)
    /\ ObserveRequest \in ImplementationActions(c)
    /\ ~(ForceRetryNow \in ImplementationActions(c))
    /\ ~(ReturnHold \in ImplementationActions(c))

HoldNeverFetchesOrForces ==
  \A c \in Cases:
    ReturnHold \in ImplementationActions(c) =>
      /\ NoNetworkFetchWhileHeld \in ImplementationActions(c)
      /\ ~(ReturnFetch \in ImplementationActions(c))
      /\ ~(ForceRetryNow \in ImplementationActions(c))

ForceOnlyForInitialElapsedGrace ==
  \A c \in Cases:
    ForceRetryNow \in ImplementationActions(c) =>
      /\ c \in {InitialAtGraceBoundary, InitialAfterGrace}
      /\ InitialAttempt \in ImplementationActions(c)
      /\ GraceElapsed \in ImplementationActions(c)

MissingBlockIngressFetchCoreSafety ==
  /\ ActionsMatchSpec
  /\ BypassCasesDoNotHoldOrMutateRequests
  /\ InitialFrontierWithinGraceHoldsAndRecords
  /\ InitialFrontierAtOrAfterGraceForcesOnce
  /\ ExistingRequestsNeverForceOrHold
  /\ HoldNeverFetchesOrForces
  /\ ForceOnlyForInitialElapsedGrace

MissingBlockIngressFetchExactness ==
  /\ ActionsMatchSpec
  /\ BypassCasesDoNotHoldOrMutateRequests
  /\ InitialFrontierWithinGraceHoldsAndRecords
  /\ InitialFrontierAtOrAfterGraceForcesOnce
  /\ ExistingRequestsNeverForceOrHold
  /\ HoldNeverFetchesOrForces
  /\ ForceOnlyForInitialElapsedGrace
MissingBlockIngressFetchCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MissingBlockIngressFetchExactness

NoBugInvariant == MissingBlockIngressFetchCoreSafety

SafetyFast == MissingBlockIngressFetchCoreSafety

====

---- MODULE SumeragiStaleRbcSessionPruneGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the stale RBC session branch of
`prune_stale_view_state(height, min_view)`.

The helper purges only same-height RBC sessions whose view is below the new
round. When DA is disabled, stale sessions are purged immediately. When DA is
enabled, invalid stale sessions are purged immediately, and delivered stale
sessions are purged only after Kura has the exact block payload for the session
block hash. Valid undelivered DA sessions, and delivered sessions without the
exact payload, remain available for RBC convergence after the view change.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

WrongHeight == "wrong_height"
FreshEqualView == "fresh_equal_view"
FreshFutureView == "fresh_future_view"
StaleNoDaUndelivered == "stale_no_da_undelivered"
StaleNoDaDelivered == "stale_no_da_delivered"
StaleDaInvalidNoPayload == "stale_da_invalid_no_payload"
StaleDaInvalidExactPayload == "stale_da_invalid_exact_payload"
StaleDaDeliveredExactPayload == "stale_da_delivered_exact_payload"
StaleDaDeliveredNoPayload == "stale_da_delivered_no_payload"
StaleDaDeliveredOtherPayloadOnly == "stale_da_delivered_other_payload_only"
StaleDaUndeliveredNoPayload == "stale_da_undelivered_no_payload"
StaleDaUndeliveredExactPayload == "stale_da_undelivered_exact_payload"

Cases == {
  WrongHeight,
  FreshEqualView,
  FreshFutureView,
  StaleNoDaUndelivered,
  StaleNoDaDelivered,
  StaleDaInvalidNoPayload,
  StaleDaInvalidExactPayload,
  StaleDaDeliveredExactPayload,
  StaleDaDeliveredNoPayload,
  StaleDaDeliveredOtherPayloadOnly,
  StaleDaUndeliveredNoPayload,
  StaleDaUndeliveredExactPayload
}

StaleSameHeightCases ==
  Cases \ {WrongHeight, FreshEqualView, FreshFutureView}

NoDaCases == {
  StaleNoDaUndelivered,
  StaleNoDaDelivered
}

DaEnabledCases == StaleSameHeightCases \ NoDaCases

DaInvalidCases == {
  StaleDaInvalidNoPayload,
  StaleDaInvalidExactPayload
}

DaValidCases == DaEnabledCases \ DaInvalidCases

DaDeliveredCases == {
  StaleDaDeliveredExactPayload,
  StaleDaDeliveredNoPayload,
  StaleDaDeliveredOtherPayloadOnly
}

DaUndeliveredCases == {
  StaleDaUndeliveredNoPayload,
  StaleDaUndeliveredExactPayload
}

SpecRemove(c) ==
  c \in NoDaCases
    \/ c \in DaInvalidCases
    \/ c = StaleDaDeliveredExactPayload

SessionPresent == 1
SessionAbsent == 2
RemovalCountIncremented == 3
RemovalCountUnchanged == 4
DaGateChecked == 5
InvalidChecked == 6
DeliveredChecked == 7
ExactPayloadChecked == 8
PurgeCalled == 9

ActionUniverse == 1..9

KeepActions ==
  {SessionPresent, RemovalCountUnchanged}

RemoveActions ==
  {SessionAbsent, RemovalCountIncremented, PurgeCalled}

SpecActions(c) ==
  (IF SpecRemove(c) THEN RemoveActions ELSE KeepActions)
    \cup (IF c \in StaleSameHeightCases THEN {DaGateChecked} ELSE {})
    \cup (IF c \in DaEnabledCases THEN {InvalidChecked} ELSE {})
    \cup (IF c \in DaValidCases THEN {DeliveredChecked} ELSE {})
    \cup (IF c \in DaDeliveredCases THEN {ExactPayloadChecked} ELSE {})

RemoveWrongHeightActions ==
  RemoveActions

RemoveFreshActions ==
  RemoveActions \cup {DaGateChecked}

KeepNoDaActions ==
  KeepActions \cup {DaGateChecked}

RemoveNoDaActions ==
  RemoveActions \cup {DaGateChecked}

KeepDaInvalidActions ==
  KeepActions \cup {DaGateChecked, InvalidChecked}

RemoveDaInvalidWithPayloadCheckActions ==
  RemoveActions \cup {DaGateChecked, InvalidChecked, ExactPayloadChecked}

KeepDaDeliveredActions ==
  KeepActions \cup {DaGateChecked, InvalidChecked, DeliveredChecked,
                    ExactPayloadChecked}

RemoveDaDeliveredActions ==
  RemoveActions \cup {DaGateChecked, InvalidChecked, DeliveredChecked,
                      ExactPayloadChecked}

RemoveDaDeliveredWithoutExactCheckActions ==
  RemoveActions \cup {DaGateChecked, InvalidChecked, DeliveredChecked}

RemoveDaUndeliveredActions ==
  RemoveActions \cup {DaGateChecked, InvalidChecked, DeliveredChecked}

RemoveDaUndeliveredPayloadActions ==
  RemoveActions \cup {DaGateChecked, InvalidChecked, DeliveredChecked,
                      ExactPayloadChecked}

RemoveWithoutPurgeActions ==
  {SessionAbsent, RemovalCountIncremented, DaGateChecked, InvalidChecked,
   DeliveredChecked, ExactPayloadChecked}

RemoveWithoutCountActions ==
  {SessionAbsent, RemovalCountUnchanged, PurgeCalled, DaGateChecked,
   InvalidChecked, DeliveredChecked, ExactPayloadChecked}

ImplementationActions(c) ==
  CASE Bug = "prune_wrong_height"
       /\ c = WrongHeight ->
      RemoveWrongHeightActions
    [] Bug = "prune_equal_view"
       /\ c = FreshEqualView ->
      RemoveFreshActions
    [] Bug = "prune_future_view"
       /\ c = FreshFutureView ->
      RemoveFreshActions
    [] Bug = "keep_no_da"
       /\ c \in NoDaCases ->
      KeepNoDaActions
    [] Bug = "require_delivered_for_no_da"
       /\ c = StaleNoDaUndelivered ->
      KeepNoDaActions
    [] Bug = "keep_da_invalid"
       /\ c \in DaInvalidCases ->
      KeepDaInvalidActions
    [] Bug = "require_payload_for_invalid"
       /\ c = StaleDaInvalidNoPayload ->
      KeepDaInvalidActions
    [] Bug = "require_payload_for_invalid"
       /\ c = StaleDaInvalidExactPayload ->
      RemoveDaInvalidWithPayloadCheckActions
    [] Bug = "keep_da_delivered_payload"
       /\ c = StaleDaDeliveredExactPayload ->
      KeepDaDeliveredActions
    [] Bug = "drop_da_undelivered"
       /\ c = StaleDaUndeliveredNoPayload ->
      RemoveDaUndeliveredActions
    [] Bug = "drop_da_undelivered_payload"
       /\ c = StaleDaUndeliveredExactPayload ->
      RemoveDaUndeliveredPayloadActions
    [] Bug = "drop_da_delivered_missing_payload"
       /\ c = StaleDaDeliveredNoPayload ->
      RemoveDaDeliveredActions
    [] Bug = "drop_da_delivered_other_payload"
       /\ c = StaleDaDeliveredOtherPayloadOnly ->
      RemoveDaDeliveredActions
    [] Bug = "skip_exact_payload_check"
       /\ c = StaleDaDeliveredNoPayload ->
      RemoveDaDeliveredWithoutExactCheckActions
    [] Bug = "skip_purge_state"
       /\ c = StaleDaDeliveredExactPayload ->
      RemoveWithoutPurgeActions
    [] Bug = "skip_removal_count"
       /\ c = StaleDaDeliveredExactPayload ->
      RemoveWithoutCountActions
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "prune_wrong_height",
  "prune_equal_view",
  "prune_future_view",
  "keep_no_da",
  "require_delivered_for_no_da",
  "keep_da_invalid",
  "require_payload_for_invalid",
  "keep_da_delivered_payload",
  "drop_da_undelivered",
  "drop_da_undelivered_payload",
  "drop_da_delivered_missing_payload",
  "drop_da_delivered_other_payload",
  "skip_exact_payload_check",
  "skip_purge_state",
  "skip_removal_count"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecRemove(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

OnlyStaleSameHeightSessionsPurged ==
  /\ SessionPresent \in ImplementationActions(WrongHeight)
  /\ SessionPresent \in ImplementationActions(FreshEqualView)
  /\ SessionPresent \in ImplementationActions(FreshFutureView)
  /\ SessionAbsent \in ImplementationActions(StaleNoDaUndelivered)

DaDisabledStaleSessionsAlwaysPurged ==
  /\ SessionAbsent \in ImplementationActions(StaleNoDaUndelivered)
  /\ SessionAbsent \in ImplementationActions(StaleNoDaDelivered)

DaInvalidSessionsPurgeBeforeDeliveryOrPayloadChecks ==
  /\ SessionAbsent \in ImplementationActions(StaleDaInvalidNoPayload)
  /\ SessionAbsent \in ImplementationActions(StaleDaInvalidExactPayload)
  /\ ~(DeliveredChecked \in ImplementationActions(StaleDaInvalidNoPayload))
  /\ ~(ExactPayloadChecked \in ImplementationActions(StaleDaInvalidExactPayload))

DaDeliveredSessionsRequireExactKuraPayload ==
  /\ SessionAbsent \in ImplementationActions(StaleDaDeliveredExactPayload)
  /\ SessionPresent \in ImplementationActions(StaleDaDeliveredNoPayload)
  /\ SessionPresent \in ImplementationActions(StaleDaDeliveredOtherPayloadOnly)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaDeliveredNoPayload)

DaUndeliveredSessionsRemainUntilDelivery ==
  /\ SessionPresent \in ImplementationActions(StaleDaUndeliveredNoPayload)
  /\ SessionPresent \in ImplementationActions(StaleDaUndeliveredExactPayload)

PurgeStateCalledForEveryRemoval ==
  \A c \in Cases:
    (SessionAbsent \in ImplementationActions(c))
      <=> (PurgeCalled \in ImplementationActions(c))

RemovalCounterMatchesRemoval ==
  \A c \in Cases:
    (SessionAbsent \in ImplementationActions(c))
      <=> (RemovalCountIncremented \in ImplementationActions(c))

NoSessionKeptWithSideEffects ==
  \A c \in Cases:
    (SessionPresent \in ImplementationActions(c))
      => /\ ~(RemovalCountIncremented \in ImplementationActions(c))
         /\ ~(PurgeCalled \in ImplementationActions(c))

NonStaleRetentionAnchors ==
  /\ ImplementationActions(WrongHeight) = KeepActions
  /\ ImplementationActions(FreshEqualView) = KeepActions
  /\ ImplementationActions(FreshFutureView) = KeepActions

NoDaPurgeAnchors ==
  /\ SessionAbsent \in ImplementationActions(StaleNoDaUndelivered)
  /\ SessionAbsent \in ImplementationActions(StaleNoDaDelivered)
  /\ PurgeCalled \in ImplementationActions(StaleNoDaUndelivered)
  /\ PurgeCalled \in ImplementationActions(StaleNoDaDelivered)

DaInvalidPurgeAnchors ==
  /\ SessionAbsent \in ImplementationActions(StaleDaInvalidNoPayload)
  /\ SessionAbsent \in ImplementationActions(StaleDaInvalidExactPayload)
  /\ InvalidChecked \in ImplementationActions(StaleDaInvalidNoPayload)
  /\ InvalidChecked \in ImplementationActions(StaleDaInvalidExactPayload)
  /\ ~(DeliveredChecked \in ImplementationActions(StaleDaInvalidNoPayload))
  /\ ~(DeliveredChecked \in ImplementationActions(StaleDaInvalidExactPayload))
  /\ ~(ExactPayloadChecked \in ImplementationActions(StaleDaInvalidNoPayload))
  /\ ~(ExactPayloadChecked \in ImplementationActions(StaleDaInvalidExactPayload))

DeliveredPayloadAnchors ==
  /\ SessionAbsent \in ImplementationActions(StaleDaDeliveredExactPayload)
  /\ SessionPresent \in ImplementationActions(StaleDaDeliveredNoPayload)
  /\ SessionPresent \in ImplementationActions(StaleDaDeliveredOtherPayloadOnly)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaDeliveredExactPayload)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaDeliveredNoPayload)
  /\ ExactPayloadChecked \in
       ImplementationActions(StaleDaDeliveredOtherPayloadOnly)

UndeliveredRetentionAnchors ==
  /\ SessionPresent \in ImplementationActions(StaleDaUndeliveredNoPayload)
  /\ SessionPresent \in ImplementationActions(StaleDaUndeliveredExactPayload)
  /\ DeliveredChecked \in ImplementationActions(StaleDaUndeliveredNoPayload)
  /\ DeliveredChecked \in
       ImplementationActions(StaleDaUndeliveredExactPayload)
  /\ ~(ExactPayloadChecked \in
       ImplementationActions(StaleDaUndeliveredNoPayload))
  /\ ~(ExactPayloadChecked \in
       ImplementationActions(StaleDaUndeliveredExactPayload))

RemovalSideEffectAnchors ==
  /\ RemovalCountIncremented \in ImplementationActions(StaleNoDaUndelivered)
  /\ RemovalCountIncremented \in ImplementationActions(StaleDaInvalidNoPayload)
  /\ RemovalCountIncremented \in
       ImplementationActions(StaleDaDeliveredExactPayload)
  /\ RemovalCountUnchanged \in
       ImplementationActions(StaleDaDeliveredNoPayload)
  /\ RemovalCountUnchanged \in
       ImplementationActions(StaleDaUndeliveredExactPayload)
  /\ PurgeCalled \in ImplementationActions(StaleDaDeliveredExactPayload)
  /\ ~(PurgeCalled \in ImplementationActions(StaleDaDeliveredNoPayload))
  /\ ~(PurgeCalled \in
       ImplementationActions(StaleDaUndeliveredExactPayload))

StaleRbcSessionPruneCoreSafety ==
  /\ ActionsMatchSpec
  /\ OnlyStaleSameHeightSessionsPurged
  /\ DaDisabledStaleSessionsAlwaysPurged
  /\ DaInvalidSessionsPurgeBeforeDeliveryOrPayloadChecks
  /\ DaDeliveredSessionsRequireExactKuraPayload
  /\ DaUndeliveredSessionsRemainUntilDelivery
  /\ PurgeStateCalledForEveryRemoval
  /\ RemovalCounterMatchesRemoval
  /\ NoSessionKeptWithSideEffects
  /\ NonStaleRetentionAnchors
  /\ NoDaPurgeAnchors
  /\ DaInvalidPurgeAnchors
  /\ DeliveredPayloadAnchors
  /\ UndeliveredRetentionAnchors
  /\ RemovalSideEffectAnchors

NoBugInvariant == StaleRbcSessionPruneCoreSafety

SafetyFast == StaleRbcSessionPruneCoreSafety

StaleRbcSessionPruneCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ StaleRbcSessionPruneCoreSafety

====

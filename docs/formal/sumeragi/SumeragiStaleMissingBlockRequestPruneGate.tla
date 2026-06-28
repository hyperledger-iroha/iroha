---- MODULE SumeragiStaleMissingBlockRequestPruneGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the missing-block request branch of
`prune_stale_view_state(height, min_view)`.

The helper removes only stale same-height missing-block requests. When DA is
disabled, stale requests can always be removed. When DA is enabled, a stale
request is removed only if the exact requested hash already has authoritative
payload progress or a Kura-local payload; unresolved DA payloads keep their
request so availability can still converge after the view change.
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
StaleNoDa == "stale_no_da"
StaleDaAuthoritativePayload == "stale_da_authoritative_payload"
StaleDaKuraPayload == "stale_da_kura_payload"
StaleDaBothPayloads == "stale_da_both_payloads"
StaleDaNoPayload == "stale_da_no_payload"
StaleDaOtherPayloadOnly == "stale_da_other_payload_only"

Cases == {
  WrongHeight,
  FreshEqualView,
  FreshFutureView,
  StaleNoDa,
  StaleDaAuthoritativePayload,
  StaleDaKuraPayload,
  StaleDaBothPayloads,
  StaleDaNoPayload,
  StaleDaOtherPayloadOnly
}

StaleSameHeightCases == {
  StaleNoDa,
  StaleDaAuthoritativePayload,
  StaleDaKuraPayload,
  StaleDaBothPayloads,
  StaleDaNoPayload,
  StaleDaOtherPayloadOnly
}

DaPayloadAvailableCases == {
  StaleDaAuthoritativePayload,
  StaleDaKuraPayload,
  StaleDaBothPayloads
}

SpecRemove(c) ==
  c = StaleNoDa \/ c \in DaPayloadAvailableCases

RequestPresent == 1
RequestAbsent == 2
RemovalCountIncremented == 3
RemovalCountUnchanged == 4
ExactPayloadChecked == 5
DaGateChecked == 6

ActionUniverse == 1..6

KeepActions ==
  {RequestPresent, RemovalCountUnchanged}

RemoveActions ==
  {RequestAbsent, RemovalCountIncremented}

SpecActions(c) ==
  (IF SpecRemove(c) THEN RemoveActions ELSE KeepActions)
    \cup (IF c \in StaleSameHeightCases THEN {DaGateChecked} ELSE {})
    \cup (IF c \in StaleSameHeightCases \ {StaleNoDa}
        THEN {ExactPayloadChecked}
        ELSE {})

RemoveWithoutCountActions ==
  {RequestAbsent, RemovalCountUnchanged, DaGateChecked, ExactPayloadChecked}

RemoveWrongHeightActions ==
  {RequestAbsent, RemovalCountIncremented}

RemoveFreshActions ==
  {RequestAbsent, RemovalCountIncremented, DaGateChecked}

KeepNoDaActions ==
  {RequestPresent, RemovalCountUnchanged, DaGateChecked}

KeepPayloadActions ==
  {RequestPresent, RemovalCountUnchanged, DaGateChecked, ExactPayloadChecked}

RemoveDaNoPayloadActions ==
  {RequestAbsent, RemovalCountIncremented, DaGateChecked, ExactPayloadChecked}

RemoveDaNoExactPayloadCheckActions ==
  {RequestAbsent, RemovalCountIncremented, DaGateChecked}

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
       /\ c = StaleNoDa ->
      KeepNoDaActions
    [] Bug = "keep_authoritative_payload"
       /\ c = StaleDaAuthoritativePayload ->
      KeepPayloadActions
    [] Bug = "keep_kura_payload"
       /\ c = StaleDaKuraPayload ->
      KeepPayloadActions
    [] Bug = "require_both_payload_sources"
       /\ c \in {StaleDaAuthoritativePayload, StaleDaKuraPayload} ->
      KeepPayloadActions
    [] Bug = "drop_da_no_payload"
       /\ c = StaleDaNoPayload ->
      RemoveDaNoPayloadActions
    [] Bug = "drop_da_other_payload"
       /\ c = StaleDaOtherPayloadOnly ->
      RemoveDaNoPayloadActions
    [] Bug = "skip_exact_payload_check"
       /\ c = StaleDaNoPayload ->
      RemoveDaNoExactPayloadCheckActions
    [] Bug = "skip_removal_count"
       /\ c = StaleDaKuraPayload ->
      RemoveWithoutCountActions
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "prune_wrong_height",
  "prune_equal_view",
  "prune_future_view",
  "keep_no_da",
  "keep_authoritative_payload",
  "keep_kura_payload",
  "require_both_payload_sources",
  "drop_da_no_payload",
  "drop_da_other_payload",
  "skip_exact_payload_check",
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

OnlyStaleSameHeightRequestsRemoved ==
  /\ RequestPresent \in ImplementationActions(WrongHeight)
  /\ RequestPresent \in ImplementationActions(FreshEqualView)
  /\ RequestPresent \in ImplementationActions(FreshFutureView)
  /\ RequestAbsent \in ImplementationActions(StaleNoDa)

DaModeRequiresExactPayloadBeforeRemoval ==
  /\ RequestAbsent \in ImplementationActions(StaleDaAuthoritativePayload)
  /\ RequestAbsent \in ImplementationActions(StaleDaKuraPayload)
  /\ RequestAbsent \in ImplementationActions(StaleDaBothPayloads)
  /\ RequestPresent \in ImplementationActions(StaleDaNoPayload)
  /\ RequestPresent \in ImplementationActions(StaleDaOtherPayloadOnly)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaNoPayload)

EitherPayloadSourceIsSufficient ==
  /\ RequestAbsent \in ImplementationActions(StaleDaAuthoritativePayload)
  /\ RequestAbsent \in ImplementationActions(StaleDaKuraPayload)

RemovalCounterMatchesRemoval ==
  \A c \in Cases:
    (RequestAbsent \in ImplementationActions(c))
      <=> (RemovalCountIncremented \in ImplementationActions(c))

NoRequestKeptWithIncrement ==
  \A c \in Cases:
    (RequestPresent \in ImplementationActions(c))
      => ~(RemovalCountIncremented \in ImplementationActions(c))

NonStaleRetentionAnchors ==
  /\ ImplementationActions(WrongHeight) = KeepActions
  /\ ImplementationActions(FreshEqualView) = KeepActions
  /\ ImplementationActions(FreshFutureView) = KeepActions

StaleRemovalSourceAnchors ==
  /\ RequestAbsent \in ImplementationActions(StaleNoDa)
  /\ RequestAbsent \in ImplementationActions(StaleDaAuthoritativePayload)
  /\ RequestAbsent \in ImplementationActions(StaleDaKuraPayload)
  /\ RequestAbsent \in ImplementationActions(StaleDaBothPayloads)

DaUnresolvedRetentionAnchors ==
  /\ RequestPresent \in ImplementationActions(StaleDaNoPayload)
  /\ RequestPresent \in ImplementationActions(StaleDaOtherPayloadOnly)
  /\ ~(RequestAbsent \in ImplementationActions(StaleDaNoPayload))
  /\ ~(RequestAbsent \in ImplementationActions(StaleDaOtherPayloadOnly))

ExactPayloadCheckAnchors ==
  /\ ~(ExactPayloadChecked \in ImplementationActions(StaleNoDa))
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaAuthoritativePayload)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaKuraPayload)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaBothPayloads)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaNoPayload)
  /\ ExactPayloadChecked \in ImplementationActions(StaleDaOtherPayloadOnly)

RemovalCounterAnchors ==
  /\ RemovalCountIncremented \in ImplementationActions(StaleNoDa)
  /\ RemovalCountIncremented \in ImplementationActions(StaleDaAuthoritativePayload)
  /\ RemovalCountIncremented \in ImplementationActions(StaleDaKuraPayload)
  /\ RemovalCountIncremented \in ImplementationActions(StaleDaBothPayloads)
  /\ RemovalCountUnchanged \in ImplementationActions(WrongHeight)
  /\ RemovalCountUnchanged \in ImplementationActions(FreshEqualView)
  /\ RemovalCountUnchanged \in ImplementationActions(FreshFutureView)
  /\ RemovalCountUnchanged \in ImplementationActions(StaleDaNoPayload)
  /\ RemovalCountUnchanged \in ImplementationActions(StaleDaOtherPayloadOnly)

StaleMissingBlockRequestPruneCoreSafety ==
  /\ ActionsMatchSpec
  /\ OnlyStaleSameHeightRequestsRemoved
  /\ DaModeRequiresExactPayloadBeforeRemoval
  /\ EitherPayloadSourceIsSufficient
  /\ RemovalCounterMatchesRemoval
  /\ NoRequestKeptWithIncrement
  /\ NonStaleRetentionAnchors
  /\ StaleRemovalSourceAnchors
  /\ DaUnresolvedRetentionAnchors
  /\ ExactPayloadCheckAnchors
  /\ RemovalCounterAnchors

NoBugInvariant == StaleMissingBlockRequestPruneCoreSafety

SafetyFast == StaleMissingBlockRequestPruneCoreSafety

StaleMissingBlockRequestPruneExactness ==
  /\ StaleMissingBlockRequestPruneCoreSafety

StaleMissingBlockRequestPruneCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ StaleMissingBlockRequestPruneExactness

====

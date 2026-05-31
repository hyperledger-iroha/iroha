---- MODULE SumeragiLocalPeerRemovedStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the local-peer removed status helper.

This slice captures `set_local_removed_from_world(...)` and
`local_peer_removed()`. It pins the helper contract used by admission and
catch-up paths: the flag starts false, setting removed stores true, setting
present stores false, the getter projects the stored flag exactly, later writes
overwrite earlier writes, repeated writes are idempotent, and getter reads do
not mutate the stored state.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialPresent == 1
SetRemovedStoresTrue == 2
GetterProjectsRemoved == 3
SetPresentStoresFalse == 4
GetterProjectsPresent == 5
PresentToRemovedOverwrite == 6
RemovedToPresentOverwrite == 7
RepeatedRemovedIdempotent == 8
RepeatedPresentIdempotent == 9
GetterSideEffectFree == 10

Candidates == 1..10

InitialFalse == 1
RemovedStoredTrue == 2
GetterRemovedTrue == 3
PresentStoredFalse == 4
GetterPresentFalse == 5
PresentToRemovedLatest == 6
RemovedToPresentLatest == 7
RepeatedRemovedStable == 8
RepeatedPresentStable == 9
GetterRemovedDoesNotClear == 10
GetterPresentDoesNotSet == 11

Actions == 1..11

SpecActions(candidate) ==
  CASE candidate = InitialPresent ->
      {InitialFalse}
    [] candidate = SetRemovedStoresTrue ->
      {RemovedStoredTrue}
    [] candidate = GetterProjectsRemoved ->
      {GetterRemovedTrue}
    [] candidate = SetPresentStoresFalse ->
      {PresentStoredFalse}
    [] candidate = GetterProjectsPresent ->
      {GetterPresentFalse}
    [] candidate = PresentToRemovedOverwrite ->
      {PresentToRemovedLatest}
    [] candidate = RemovedToPresentOverwrite ->
      {RemovedToPresentLatest}
    [] candidate = RepeatedRemovedIdempotent ->
      {RepeatedRemovedStable}
    [] candidate = RepeatedPresentIdempotent ->
      {RepeatedPresentStable}
    [] candidate = GetterSideEffectFree ->
      {GetterRemovedDoesNotClear, GetterPresentDoesNotSet}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialPresent /\ Bug = "initial_removed_true" ->
      spec \ {InitialFalse}
    [] candidate = SetRemovedStoresTrue /\ Bug = "set_removed_not_stored" ->
      spec \ {RemovedStoredTrue}
    [] candidate = GetterProjectsRemoved /\ Bug = "getter_removed_returns_false" ->
      spec \ {GetterRemovedTrue}
    [] candidate = SetPresentStoresFalse /\ Bug = "set_present_not_stored" ->
      spec \ {PresentStoredFalse}
    [] candidate = GetterProjectsPresent /\ Bug = "getter_present_returns_true" ->
      spec \ {GetterPresentFalse}
    [] candidate = PresentToRemovedOverwrite /\
          Bug = "present_to_removed_overwrite_ignored" ->
      spec \ {PresentToRemovedLatest}
    [] candidate = RemovedToPresentOverwrite /\
          Bug = "removed_to_present_overwrite_ignored" ->
      spec \ {RemovedToPresentLatest}
    [] candidate = RepeatedRemovedIdempotent /\
          Bug = "repeated_removed_toggles" ->
      spec \ {RepeatedRemovedStable}
    [] candidate = RepeatedPresentIdempotent /\
          Bug = "repeated_present_toggles" ->
      spec \ {RepeatedPresentStable}
    [] candidate = GetterSideEffectFree /\
          Bug = "getter_removed_clears_flag" ->
      spec \ {GetterRemovedDoesNotClear}
    [] candidate = GetterSideEffectFree /\
          Bug = "getter_present_sets_flag" ->
      spec \ {GetterPresentDoesNotSet}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 10
     /\ checked' = checked + 1
  \/ /\ checked = 10
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "initial_removed_true",
       "set_removed_not_stored",
       "getter_removed_returns_false",
       "set_present_not_stored",
       "getter_present_returns_true",
       "present_to_removed_overwrite_ignored",
       "removed_to_present_overwrite_ignored",
       "repeated_removed_toggles",
       "repeated_present_toggles",
       "getter_removed_clears_flag",
       "getter_present_sets_flag"
     }
  /\ checked \in 0..10
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllLocalPeerRemovedActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

StorageWriteActionsMatchSpec ==
  \A candidate \in {
    InitialPresent,
    SetRemovedStoresTrue,
    SetPresentStoresFalse,
    PresentToRemovedOverwrite,
    RemovedToPresentOverwrite,
    RepeatedRemovedIdempotent,
    RepeatedPresentIdempotent
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

GetterActionsMatchSpec ==
  \A candidate \in {
    GetterProjectsRemoved,
    GetterProjectsPresent,
    GetterSideEffectFree
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

InitialPresentAnchor ==
  InitialFalse \in ImplementationActions(InitialPresent)

SetAndGetterAnchors ==
  /\ RemovedStoredTrue \in ImplementationActions(SetRemovedStoresTrue)
  /\ GetterRemovedTrue \in ImplementationActions(GetterProjectsRemoved)
  /\ PresentStoredFalse \in ImplementationActions(SetPresentStoresFalse)
  /\ GetterPresentFalse \in ImplementationActions(GetterProjectsPresent)

OverwriteAnchors ==
  /\ PresentToRemovedLatest \in ImplementationActions(PresentToRemovedOverwrite)
  /\ RemovedToPresentLatest \in ImplementationActions(RemovedToPresentOverwrite)

IdempotenceAnchors ==
  /\ RepeatedRemovedStable \in ImplementationActions(RepeatedRemovedIdempotent)
  /\ RepeatedPresentStable \in ImplementationActions(RepeatedPresentIdempotent)

GetterSideEffectFreeAnchors ==
  /\ GetterRemovedDoesNotClear \in ImplementationActions(GetterSideEffectFree)
  /\ GetterPresentDoesNotSet \in ImplementationActions(GetterSideEffectFree)

LocalPeerRemovedStatusSafetyAnchors ==
  /\ AllLocalPeerRemovedActionsMatchSpec
  /\ StorageWriteActionsMatchSpec
  /\ GetterActionsMatchSpec
  /\ InitialPresentAnchor
  /\ SetAndGetterAnchors
  /\ OverwriteAnchors
  /\ IdempotenceAnchors
  /\ GetterSideEffectFreeAnchors

BugInitialRemovedTrue ==
  ImplementationActions(InitialPresent) = SpecActions(InitialPresent)

BugSetRemovedNotStored ==
  ImplementationActions(SetRemovedStoresTrue) =
    SpecActions(SetRemovedStoresTrue)

BugGetterRemovedReturnsFalse ==
  ImplementationActions(GetterProjectsRemoved) =
    SpecActions(GetterProjectsRemoved)

BugSetPresentNotStored ==
  ImplementationActions(SetPresentStoresFalse) =
    SpecActions(SetPresentStoresFalse)

BugGetterPresentReturnsTrue ==
  ImplementationActions(GetterProjectsPresent) =
    SpecActions(GetterProjectsPresent)

BugPresentToRemovedOverwriteIgnored ==
  ImplementationActions(PresentToRemovedOverwrite) =
    SpecActions(PresentToRemovedOverwrite)

BugRemovedToPresentOverwriteIgnored ==
  ImplementationActions(RemovedToPresentOverwrite) =
    SpecActions(RemovedToPresentOverwrite)

BugRepeatedRemovedToggles ==
  ImplementationActions(RepeatedRemovedIdempotent) =
    SpecActions(RepeatedRemovedIdempotent)

BugRepeatedPresentToggles ==
  ImplementationActions(RepeatedPresentIdempotent) =
    SpecActions(RepeatedPresentIdempotent)

BugGetterRemovedClearsFlag ==
  ImplementationActions(GetterSideEffectFree) =
    SpecActions(GetterSideEffectFree)

BugGetterPresentSetsFlag ==
  ImplementationActions(GetterSideEffectFree) =
    SpecActions(GetterSideEffectFree)

====

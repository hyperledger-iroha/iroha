---- MODULE SumeragiVrfLocalStateGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `VrfLocalState` and `VrfActor`.

This slice pins the local VRF emission state used by the consensus actor:
`ensure_epoch(...)` resets material only when the epoch changes, `state_mut(...)`
creates or refreshes state only for permissioned/NPoS modes, `reset(...)` clears
the local state, and `note_commit(...)` / `note_reveal(...)` mutate state only
after the same supported-mode gate.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Str;
  candidate,
  \* @type: Set(Int);
  actions

\* @type: <<Str, Set(Int)>>;
vars == <<candidate, actions>>

NoneCase == "none"

NewState == "new_state"
EnsureSameEpoch == "ensure_same_epoch"
EnsureEpochReset == "ensure_epoch_reset"
StateMutPermissionedCreates == "state_mut_permissioned_creates"
StateMutNposResets == "state_mut_npos_resets"
StateMutUnsupportedAbsent == "state_mut_unsupported_absent"
StateMutUnsupportedExisting == "state_mut_unsupported_existing"
ActorReset == "actor_reset"
NoteCommitAllowedNewEpoch == "note_commit_allowed_new_epoch"
NoteCommitAllowedSameEpoch == "note_commit_allowed_same_epoch"
NoteCommitUnsupported == "note_commit_unsupported"
NoteRevealAllowedNewEpoch == "note_reveal_allowed_new_epoch"
NoteRevealAllowedSameEpoch == "note_reveal_allowed_same_epoch"
NoteRevealUnsupported == "note_reveal_unsupported"

Cases == {
  NewState,
  EnsureSameEpoch,
  EnsureEpochReset,
  StateMutPermissionedCreates,
  StateMutNposResets,
  StateMutUnsupportedAbsent,
  StateMutUnsupportedExisting,
  ActorReset,
  NoteCommitAllowedNewEpoch,
  NoteCommitAllowedSameEpoch,
  NoteCommitUnsupported,
  NoteRevealAllowedNewEpoch,
  NoteRevealAllowedSameEpoch,
  NoteRevealUnsupported
}

AfterPresent == 1
AfterAbsent == 2
AfterEpochRequested == 3
AfterEpochOld == 4
AfterRevealZero == 5
AfterRevealOld == 6
AfterRevealInput == 7
AfterCommitmentZero == 8
AfterCommitmentOld == 9
AfterCommitmentInput == 10
AfterCommitSentTrue == 11
AfterCommitSentFalse == 12
AfterRevealSentTrue == 13
AfterRevealSentFalse == 14
ReturnSome == 15
ReturnNone == 16

Actions == 1..16

FreshStateActions ==
  {AfterPresent, AfterEpochRequested, AfterRevealZero, AfterCommitmentZero,
   AfterCommitSentFalse, AfterRevealSentFalse}

ExistingOldActions ==
  {AfterPresent, AfterEpochOld, AfterRevealOld, AfterCommitmentOld,
   AfterCommitSentTrue, AfterRevealSentTrue}

SameEpochOldActions ==
  {AfterPresent, AfterEpochRequested, AfterRevealOld, AfterCommitmentOld,
   AfterCommitSentTrue, AfterRevealSentTrue}

SpecActions(c) ==
  CASE c = NewState ->
      FreshStateActions
    [] c = EnsureSameEpoch ->
      SameEpochOldActions
    [] c = EnsureEpochReset ->
      FreshStateActions
    [] c = StateMutPermissionedCreates ->
      FreshStateActions \cup {ReturnSome}
    [] c = StateMutNposResets ->
      FreshStateActions \cup {ReturnSome}
    [] c = StateMutUnsupportedAbsent ->
      {AfterAbsent, ReturnNone}
    [] c = StateMutUnsupportedExisting ->
      ExistingOldActions \cup {ReturnNone}
    [] c = ActorReset ->
      {AfterAbsent}
    [] c = NoteCommitAllowedNewEpoch ->
      {AfterPresent, AfterEpochRequested, AfterRevealZero,
       AfterCommitmentInput, AfterCommitSentTrue, AfterRevealSentFalse}
    [] c = NoteCommitAllowedSameEpoch ->
      {AfterPresent, AfterEpochRequested, AfterRevealOld,
       AfterCommitmentInput, AfterCommitSentTrue, AfterRevealSentTrue}
    [] c = NoteCommitUnsupported ->
      ExistingOldActions
    [] c = NoteRevealAllowedNewEpoch ->
      {AfterPresent, AfterEpochRequested, AfterRevealInput,
       AfterCommitmentZero, AfterCommitSentFalse, AfterRevealSentTrue}
    [] c = NoteRevealAllowedSameEpoch ->
      {AfterPresent, AfterEpochRequested, AfterRevealInput,
       AfterCommitmentOld, AfterCommitSentTrue, AfterRevealSentTrue}
    [] c = NoteRevealUnsupported ->
      ExistingOldActions
    [] OTHER ->
      {}

EpochResetCases ==
  {EnsureEpochReset, StateMutNposResets, NoteCommitAllowedNewEpoch,
   NoteRevealAllowedNewEpoch}

SameEpochCases ==
  {EnsureSameEpoch, NoteCommitAllowedSameEpoch, NoteRevealAllowedSameEpoch}

UnsupportedExistingCases ==
  {StateMutUnsupportedExisting, NoteCommitUnsupported, NoteRevealUnsupported}

ActualActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "unsupported_mode_creates_state"
       /\ c = StateMutUnsupportedAbsent ->
      FreshStateActions \cup {ReturnSome}
    [] Bug = "unsupported_mode_mutates_existing"
       /\ c \in UnsupportedExistingCases ->
      FreshStateActions \cup
        (IF c = StateMutUnsupportedExisting THEN {ReturnSome} ELSE {})
    [] Bug = "allowed_mode_returns_none"
       /\ c \in {StateMutPermissionedCreates, StateMutNposResets} ->
      {AfterAbsent, ReturnNone}
    [] Bug = "epoch_switch_keeps_material"
       /\ c \in EpochResetCases ->
      (spec \ {AfterRevealZero, AfterCommitmentZero,
               AfterCommitSentFalse, AfterRevealSentFalse}) \cup
      {AfterRevealOld, AfterCommitmentOld,
       AfterCommitSentTrue, AfterRevealSentTrue}
    [] Bug = "same_epoch_resets_material"
       /\ c \in SameEpochCases ->
      (spec \ {AfterRevealOld, AfterCommitmentOld,
               AfterCommitSentTrue, AfterRevealSentTrue}) \cup
      {AfterRevealZero, AfterCommitmentZero,
       AfterCommitSentFalse, AfterRevealSentFalse}
    [] Bug = "note_commit_skips_commitment"
       /\ c \in {NoteCommitAllowedNewEpoch, NoteCommitAllowedSameEpoch} ->
      (spec \ {AfterCommitmentInput}) \cup {AfterCommitmentOld}
    [] Bug = "note_commit_skips_sent_flag"
       /\ c \in {NoteCommitAllowedNewEpoch, NoteCommitAllowedSameEpoch} ->
      (spec \ {AfterCommitSentTrue}) \cup {AfterCommitSentFalse}
    [] Bug = "note_commit_clears_reveal_same_epoch"
       /\ c = NoteCommitAllowedSameEpoch ->
      (spec \ {AfterRevealOld, AfterRevealSentTrue}) \cup
      {AfterRevealZero, AfterRevealSentFalse}
    [] Bug = "note_reveal_skips_reveal"
       /\ c \in {NoteRevealAllowedNewEpoch, NoteRevealAllowedSameEpoch} ->
      (spec \ {AfterRevealInput}) \cup {AfterRevealOld}
    [] Bug = "note_reveal_skips_sent_flag"
       /\ c \in {NoteRevealAllowedNewEpoch, NoteRevealAllowedSameEpoch} ->
      (spec \ {AfterRevealSentTrue}) \cup {AfterRevealSentFalse}
    [] Bug = "note_reveal_clears_commit_same_epoch"
       /\ c = NoteRevealAllowedSameEpoch ->
      (spec \ {AfterCommitmentOld, AfterCommitSentTrue}) \cup
      {AfterCommitmentZero, AfterCommitSentFalse}
    [] Bug = "reset_keeps_local"
       /\ c = ActorReset ->
      ExistingOldActions
    [] OTHER ->
      spec

Init ==
  /\ candidate = NoneCase
  /\ actions = {}

Apply ==
  /\ candidate = NoneCase
  /\ candidate' \in Cases
  /\ actions' = ActualActions(candidate')

Stable ==
  UNCHANGED vars

Next ==
  \/ Apply
  \/ Stable

TypeInvariant ==
  /\ candidate \in Cases \cup {NoneCase}
  /\ actions \subseteq Actions

Safety ==
  candidate = NoneCase \/ actions = SpecActions(candidate)

=============================================================================

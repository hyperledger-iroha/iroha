---- MODULE SumeragiProposalLivenessGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the proposal-liveness state helper used by the
missing-QC recovery path.

The model captures `step_proposal_liveness_state(...)`,
`ProposalLivenessSlot::new(...)`, the slot-creation behavior of
`ensure_proposal_liveness_slot(...)`, and the observable state update performed
by `mark_proposal_liveness_state(...)`. In particular, dependency-recovery
state must not downgrade back to awaiting-proposal unless the caller explicitly
requests `Normal`, and creating or replacing a slot must reset its per-view
bookkeeping.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSlot == 0
SameSlot == 1
DifferentSlot == 2

Normal == 0
AwaitingProposal == 1
RecoveryAcquireDependencies == 2

ResultStep == 1
ResultSlot == 2

NoField == -1
ArgHeight == 10
ArgView == 3

StepCases == {
  "step_normal_normal",
  "step_normal_awaiting",
  "step_normal_recovery",
  "step_awaiting_normal",
  "step_awaiting_awaiting",
  "step_awaiting_recovery",
  "step_recovery_normal",
  "step_recovery_awaiting",
  "step_recovery_recovery"
}

NewCases == {"new_slot"}

EnsureCases == {
  "ensure_absent_creates",
  "ensure_same_preserves",
  "ensure_diff_replaces"
}

MarkCases == {
  "mark_absent_normal",
  "mark_absent_recovery",
  "mark_same_recovery_to_normal",
  "mark_same_recovery_to_awaiting",
  "mark_same_awaiting_to_recovery",
  "mark_diff_replaces_then_recovery"
}

Cases == StepCases \cup NewCases \cup EnsureCases \cup MarkCases

States == {Normal, AwaitingProposal, RecoveryAcquireDependencies}

StepCurrent(c) ==
  CASE c \in {"step_normal_normal", "step_normal_awaiting",
              "step_normal_recovery"} -> Normal
    [] c \in {"step_awaiting_normal", "step_awaiting_awaiting",
              "step_awaiting_recovery"} -> AwaitingProposal
    [] c \in {"step_recovery_normal", "step_recovery_awaiting",
              "step_recovery_recovery"} -> RecoveryAcquireDependencies
    [] OTHER -> Normal

RequestedState(c) ==
  CASE c \in {"step_normal_normal", "step_awaiting_normal",
              "step_recovery_normal", "mark_absent_normal",
              "mark_same_recovery_to_normal"} -> Normal
    [] c \in {"step_normal_awaiting", "step_awaiting_awaiting",
              "step_recovery_awaiting", "mark_same_recovery_to_awaiting"} ->
       AwaitingProposal
    [] OTHER -> RecoveryAcquireDependencies

SpecStep(current, requested) ==
  CASE requested = Normal -> Normal
    [] requested = RecoveryAcquireDependencies -> RecoveryAcquireDependencies
    [] current = RecoveryAcquireDependencies -> RecoveryAcquireDependencies
    [] OTHER -> AwaitingProposal

ActualStep(current, requested) ==
  CASE Bug = "step_recovery_downgrades_to_awaiting"
       /\ current = RecoveryAcquireDependencies
       /\ requested = AwaitingProposal -> AwaitingProposal
    [] Bug = "step_awaiting_recovery_ignored"
       /\ current = AwaitingProposal
       /\ requested = RecoveryAcquireDependencies -> AwaitingProposal
    [] Bug = "step_normal_recovery_ignored"
       /\ current = Normal
       /\ requested = RecoveryAcquireDependencies -> Normal
    [] Bug = "step_normal_normal_enters_awaiting"
       /\ current = Normal
       /\ requested = Normal -> AwaitingProposal
    [] OTHER -> SpecStep(current, requested)

\* @type: (Int, Int, Int, Int, Int, Int) => <<Int, Int, Int, Int, Int, Int>>;
Slot(height, view, state, attempts, rotation_deferred, reacquire_exhausted) ==
  <<height, view, state, attempts, rotation_deferred, reacquire_exhausted>>

NoSlotValue == Slot(NoField, NoField, NoField, NoField, NoField, NoField)

NewSlot ==
  Slot(ArgHeight, ArgView, AwaitingProposal, 0, 0, 0)

ActualNewSlot ==
  CASE Bug = "new_slot_wrong_height" ->
       Slot(ArgHeight + 1, ArgView, AwaitingProposal, 0, 0, 0)
    [] Bug = "new_slot_normal_state" ->
       Slot(ArgHeight, ArgView, Normal, 0, 0, 0)
    [] Bug = "new_slot_nonzero_attempts" ->
       Slot(ArgHeight, ArgView, AwaitingProposal, 1, 0, 0)
    [] Bug = "new_slot_sets_rotation_flag" ->
       Slot(ArgHeight, ArgView, AwaitingProposal, 0, 1, 0)
    [] OTHER -> NewSlot

InputKind(c) ==
  CASE c \in {"ensure_absent_creates", "mark_absent_normal",
              "mark_absent_recovery"} -> NoSlot
    [] c \in {"ensure_diff_replaces",
              "mark_diff_replaces_then_recovery"} -> DifferentSlot
    [] OTHER -> SameSlot

InputSlot(c) ==
  CASE InputKind(c) = NoSlot -> NoSlotValue
    [] c = "mark_same_awaiting_to_recovery" ->
       Slot(ArgHeight, ArgView, AwaitingProposal, 1, 0, 0)
    [] InputKind(c) = SameSlot ->
       Slot(ArgHeight, ArgView, RecoveryAcquireDependencies, 2, 1, 1)
    [] OTHER ->
       Slot(ArgHeight - 1, ArgView - 1, RecoveryAcquireDependencies, 2, 1, 0)

SpecEnsure(c) ==
  IF InputKind(c) = SameSlot
  THEN InputSlot(c)
  ELSE NewSlot

ActualEnsure(c) ==
  CASE Bug = "ensure_absent_missing_slot"
       /\ InputKind(c) = NoSlot -> NoSlotValue
    [] Bug = "ensure_same_replaces_existing"
       /\ InputKind(c) = SameSlot -> ActualNewSlot
    [] Bug = "ensure_diff_preserves_old_slot"
       /\ InputKind(c) = DifferentSlot -> InputSlot(c)
    [] OTHER -> SpecEnsure(c)

\* @type: (<<Int, Int, Int, Int, Int, Int>>, Int) => <<Int, Int, Int, Int, Int, Int>>;
WithState(slot, state) ==
  Slot(slot[1], slot[2], state, slot[4], slot[5], slot[6])

SpecMarked(c) ==
  LET ensured == SpecEnsure(c) IN
    WithState(ensured, SpecStep(ensured[3], RequestedState(c)))

ActualMarked(c) ==
  LET ensured == ActualEnsure(c) IN
    CASE Bug = "mark_absent_skips_ensure"
         /\ InputKind(c) = NoSlot -> NoSlotValue
      [] Bug = "mark_same_skips_step"
         /\ c = "mark_same_recovery_to_normal" -> ensured
      [] Bug = "mark_diff_updates_old_slot"
         /\ InputKind(c) = DifferentSlot ->
         WithState(InputSlot(c), ActualStep(InputSlot(c)[3], RequestedState(c)))
      [] Bug = "mark_recovery_downgrades"
         /\ c = "mark_same_recovery_to_awaiting" ->
         WithState(ensured, AwaitingProposal)
      [] OTHER -> WithState(ensured, ActualStep(ensured[3], RequestedState(c)))

\* @type: Int => <<Int, Int, Int, Int, Int, Int, Int>>;
StepResult(state) ==
  <<ResultStep, state, NoField, NoField, NoField, NoField, NoField>>

\* @type: <<Int, Int, Int, Int, Int, Int>> => <<Int, Int, Int, Int, Int, Int, Int>>;
SlotResult(slot) ==
  <<ResultSlot, slot[1], slot[2], slot[3], slot[4], slot[5], slot[6]>>

SpecResult(c) ==
  CASE c \in StepCases ->
       StepResult(SpecStep(StepCurrent(c), RequestedState(c)))
    [] c \in NewCases -> SlotResult(NewSlot)
    [] c \in EnsureCases -> SlotResult(SpecEnsure(c))
    [] OTHER -> SlotResult(SpecMarked(c))

ActualResult(c) ==
  CASE c \in StepCases ->
       StepResult(ActualStep(StepCurrent(c), RequestedState(c)))
    [] c \in NewCases -> SlotResult(ActualNewSlot)
    [] c \in EnsureCases -> SlotResult(ActualEnsure(c))
    [] OTHER -> SlotResult(ActualMarked(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "step_recovery_downgrades_to_awaiting",
       "step_awaiting_recovery_ignored",
       "step_normal_recovery_ignored",
       "step_normal_normal_enters_awaiting",
       "new_slot_wrong_height",
       "new_slot_normal_state",
       "new_slot_nonzero_attempts",
       "new_slot_sets_rotation_flag",
       "ensure_absent_missing_slot",
       "ensure_same_replaces_existing",
       "ensure_diff_preserves_old_slot",
       "mark_absent_skips_ensure",
       "mark_same_skips_step",
       "mark_diff_updates_old_slot",
       "mark_recovery_downgrades"
     }
  /\ checked = 0

StepTransitionsMatchSpec ==
  \A c \in StepCases:
    ActualResult(c) = SpecResult(c)

NewSlotMatchesSpec ==
  \A c \in NewCases:
    ActualResult(c) = SpecResult(c)

EnsureSlotMatchesSpec ==
  \A c \in EnsureCases:
    ActualResult(c) = SpecResult(c)

MarkSlotMatchesSpec ==
  \A c \in MarkCases:
    ActualResult(c) = SpecResult(c)

RecoveryStepAnchors ==
  /\ SpecStep(RecoveryAcquireDependencies, AwaitingProposal)
       = RecoveryAcquireDependencies
  /\ SpecStep(RecoveryAcquireDependencies, Normal) = Normal
  /\ SpecStep(AwaitingProposal, RecoveryAcquireDependencies)
       = RecoveryAcquireDependencies
  /\ SpecStep(Normal, RecoveryAcquireDependencies)
       = RecoveryAcquireDependencies
  /\ SpecStep(Normal, Normal) = Normal

NewSlotResetAnchors ==
  /\ NewSlot[1] = ArgHeight
  /\ NewSlot[2] = ArgView
  /\ NewSlot[3] = AwaitingProposal
  /\ NewSlot[4] = 0
  /\ NewSlot[5] = 0
  /\ NewSlot[6] = 0

EnsureSlotAnchors ==
  /\ SpecEnsure("ensure_absent_creates") = NewSlot
  /\ SpecEnsure("ensure_same_preserves") = InputSlot("ensure_same_preserves")
  /\ SpecEnsure("ensure_diff_replaces") = NewSlot

MarkSlotAnchors ==
  /\ SpecMarked("mark_absent_normal") = WithState(NewSlot, Normal)
  /\ SpecMarked("mark_absent_recovery")
       = WithState(NewSlot, RecoveryAcquireDependencies)
  /\ SpecMarked("mark_same_recovery_to_normal")
       = WithState(InputSlot("mark_same_recovery_to_normal"), Normal)
  /\ SpecMarked("mark_same_recovery_to_awaiting")
       = WithState(InputSlot("mark_same_recovery_to_awaiting"),
                   RecoveryAcquireDependencies)
  /\ SpecMarked("mark_same_awaiting_to_recovery")
       = WithState(InputSlot("mark_same_awaiting_to_recovery"),
                   RecoveryAcquireDependencies)
  /\ SpecMarked("mark_diff_replaces_then_recovery")
       = WithState(NewSlot, RecoveryAcquireDependencies)

ProposalLivenessTransitionExact ==
  /\ StepTransitionsMatchSpec
  /\ RecoveryStepAnchors

ProposalLivenessNewSlotExact ==
  /\ NewSlotMatchesSpec
  /\ NewSlotResetAnchors

ProposalLivenessEnsureExact ==
  /\ EnsureSlotMatchesSpec
  /\ EnsureSlotAnchors

ProposalLivenessMarkExact ==
  /\ MarkSlotMatchesSpec
  /\ MarkSlotAnchors

ProposalLivenessExactness ==
  /\ ProposalLivenessTransitionExact
  /\ ProposalLivenessNewSlotExact
  /\ ProposalLivenessEnsureExact
  /\ ProposalLivenessMarkExact

SafetyFast ==
  ProposalLivenessExactness

====

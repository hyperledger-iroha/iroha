---- MODULE SumeragiEffectiveModeGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for effective consensus-mode selection.

This slice captures `effective_consensus_mode_for_height_from_world(...)` and
`staged_mode_info(...)`. It abstracts Permissioned/NPoS and staged parameter
presence into finite cases while preserving the observable contracts: missing
next-mode or activation parameters do not activate a mode, activation height is
inclusive, pre-activation heights use the fallback mode until a local runtime
flip has already moved fallback to the staged target, and staged status
projection reports the next-mode tag independently from activation-height
presence while preserving the activation height if it is configured.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoNextNoActivation == "no_next_no_activation"
NoNextWithActivation == "no_next_with_activation"
NextNposNoActivation == "next_npos_no_activation"
NextPermNoActivation == "next_perm_no_activation"
BeforePermToNposOldFallback == "before_perm_to_npos_old_fallback"
AtPermToNpos == "at_perm_to_npos"
AfterPermToNpos == "after_perm_to_npos"
BeforePermToNposAfterFlip == "before_perm_to_npos_after_flip"
BeforeNposToPermOldFallback == "before_npos_to_perm_old_fallback"
AtNposToPerm == "at_npos_to_perm"
AfterNposToPerm == "after_npos_to_perm"
BeforeNposToPermAfterFlip == "before_npos_to_perm_after_flip"

Cases == {
  NoNextNoActivation,
  NoNextWithActivation,
  NextNposNoActivation,
  NextPermNoActivation,
  BeforePermToNposOldFallback,
  AtPermToNpos,
  AfterPermToNpos,
  BeforePermToNposAfterFlip,
  BeforeNposToPermOldFallback,
  AtNposToPerm,
  AfterNposToPerm,
  BeforeNposToPermAfterFlip
}

EffectivePerm == 1
EffectiveNpos == 2
StagedNone == 3
StagedPerm == 4
StagedNpos == 5
ActivationNone == 6
ActivationSome == 7

Actions == 1..7

SpecActions(c) ==
  CASE c = NoNextNoActivation ->
      {EffectivePerm, StagedNone, ActivationNone}
    [] c = NoNextWithActivation ->
      {EffectivePerm, StagedNone, ActivationSome}
    [] c = NextNposNoActivation ->
      {EffectivePerm, StagedNpos, ActivationNone}
    [] c = NextPermNoActivation ->
      {EffectiveNpos, StagedPerm, ActivationNone}
    [] c = BeforePermToNposOldFallback ->
      {EffectivePerm, StagedNpos, ActivationSome}
    [] c = AtPermToNpos ->
      {EffectiveNpos, StagedNpos, ActivationSome}
    [] c = AfterPermToNpos ->
      {EffectiveNpos, StagedNpos, ActivationSome}
    [] c = BeforePermToNposAfterFlip ->
      {EffectivePerm, StagedNpos, ActivationSome}
    [] c = BeforeNposToPermOldFallback ->
      {EffectiveNpos, StagedPerm, ActivationSome}
    [] c = AtNposToPerm ->
      {EffectivePerm, StagedPerm, ActivationSome}
    [] c = AfterNposToPerm ->
      {EffectivePerm, StagedPerm, ActivationSome}
    [] c = BeforeNposToPermAfterFlip ->
      {EffectiveNpos, StagedPerm, ActivationSome}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "no_next_ignores_fallback"
       /\ c \in {NoNextNoActivation, NoNextWithActivation} ->
      (spec \ {EffectivePerm}) \cup {EffectiveNpos}
    [] Bug = "no_next_drops_activation_status"
       /\ c = NoNextWithActivation ->
      (spec \ {ActivationSome}) \cup {ActivationNone}
    [] Bug = "next_without_activation_switches"
       /\ c = NextNposNoActivation ->
      (spec \ {EffectivePerm}) \cup {EffectiveNpos}
    [] Bug = "next_without_activation_loses_tag"
       /\ c \in {NextNposNoActivation, NextPermNoActivation} ->
      (spec \ {StagedNpos, StagedPerm}) \cup {StagedNone}
    [] Bug = "before_activation_switches_early"
       /\ c = BeforePermToNposOldFallback ->
      (spec \ {EffectivePerm}) \cup {EffectiveNpos}
    [] Bug = "activation_boundary_strict"
       /\ c \in {AtPermToNpos, AtNposToPerm} ->
      (spec \ {EffectiveNpos, EffectivePerm}) \cup
        IF c = AtPermToNpos THEN {EffectivePerm} ELSE {EffectiveNpos}
    [] Bug = "after_activation_uses_fallback"
       /\ c = AfterPermToNpos ->
      (spec \ {EffectiveNpos}) \cup {EffectivePerm}
    [] Bug = "after_activation_perm_target_uses_fallback"
       /\ c = AfterNposToPerm ->
      (spec \ {EffectivePerm}) \cup {EffectiveNpos}
    [] Bug = "pre_activation_after_flip_uses_fallback"
       /\ c = BeforePermToNposAfterFlip ->
      (spec \ {EffectivePerm}) \cup {EffectiveNpos}
    [] Bug = "pre_activation_after_flip_uses_next"
       /\ c = BeforeNposToPermAfterFlip ->
      (spec \ {EffectiveNpos}) \cup {EffectivePerm}
    [] Bug = "staged_npos_tag_as_permissioned"
       /\ c \in {NextNposNoActivation, BeforePermToNposOldFallback,
                 AtPermToNpos, AfterPermToNpos, BeforePermToNposAfterFlip} ->
      (spec \ {StagedNpos}) \cup {StagedPerm}
    [] Bug = "staged_permissioned_tag_as_npos"
       /\ c \in {NextPermNoActivation, BeforeNposToPermOldFallback,
                 AtNposToPerm, AfterNposToPerm, BeforeNposToPermAfterFlip} ->
      (spec \ {StagedPerm}) \cup {StagedNpos}
    [] Bug = "staged_requires_activation"
       /\ c \in {NextNposNoActivation, NextPermNoActivation} ->
      (spec \ {StagedNpos, StagedPerm}) \cup {StagedNone}
    [] Bug = "staged_drops_activation_height"
       /\ c \in {BeforePermToNposOldFallback, AtPermToNpos,
                 BeforeNposToPermOldFallback, AtNposToPerm} ->
      (spec \ {ActivationSome}) \cup {ActivationNone}
    [] Bug = "fallback_old_npos_switches_early"
       /\ c = BeforeNposToPermOldFallback ->
      (spec \ {EffectiveNpos}) \cup {EffectivePerm}
    [] OTHER -> spec

Bugs == {
  "none",
  "no_next_ignores_fallback",
  "no_next_drops_activation_status",
  "next_without_activation_switches",
  "next_without_activation_loses_tag",
  "before_activation_switches_early",
  "activation_boundary_strict",
  "after_activation_uses_fallback",
  "after_activation_perm_target_uses_fallback",
  "pre_activation_after_flip_uses_fallback",
  "pre_activation_after_flip_uses_next",
  "staged_npos_tag_as_permissioned",
  "staged_permissioned_tag_as_npos",
  "staged_requires_activation",
  "staged_drops_activation_height",
  "fallback_old_npos_switches_early"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

NoBugInvariant ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SafetyFast == NoBugInvariant

BugNoNextIgnoresFallback == NoBugInvariant
BugNoNextDropsActivationStatus == NoBugInvariant
BugNextWithoutActivationSwitches == NoBugInvariant
BugNextWithoutActivationLosesTag == NoBugInvariant
BugBeforeActivationSwitchesEarly == NoBugInvariant
BugActivationBoundaryStrict == NoBugInvariant
BugAfterActivationUsesFallback == NoBugInvariant
BugAfterActivationPermTargetUsesFallback == NoBugInvariant
BugPreActivationAfterFlipUsesFallback == NoBugInvariant
BugPreActivationAfterFlipUsesNext == NoBugInvariant
BugStagedNposTagAsPermissioned == NoBugInvariant
BugStagedPermissionedTagAsNpos == NoBugInvariant
BugStagedRequiresActivation == NoBugInvariant
BugStagedDropsActivationHeight == NoBugInvariant
BugFallbackOldNposSwitchesEarly == NoBugInvariant

====

---- MODULE SumeragiValidatorSetTransition ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi validator-set transition safety.

This model focuses on one scheduled validator-set change. It checks that:
- the old validator set finalizes the boundary block before activation,
- the new validator set cannot certify blocks before activation,
- old-set certificates stop before the activation height,
- a height cannot be committed by multiple validator-set certificates, and
- mixed-set certificates are never accepted.

The model intentionally abstracts signatures into certificate events. The
quorum math and signer-set fork safety are covered by SumeragiForkSafety.tla;
this model covers the epoch/activation gate that selects which validator set is
authoritative for a height.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxHeight,
  \* @type: Int;
  ActivationHeight,
  \* @type: Bool;
  BugDisableActivationFinalityGate,
  \* @type: Bool;
  BugAllowPrematureNewSetCertificate,
  \* @type: Bool;
  BugAllowMixedSetCertificate

VARIABLES
  \* @type: Int;
  height,
  \* @type: Str;
  activeSet,
  \* @type: Bool;
  staged,
  \* @type: Bool;
  activated,
  \* @type: Set(Int);
  committedOld,
  \* @type: Set(Int);
  committedNew,
  \* @type: Set(Int);
  committedMixed

vars == <<
  height,
  activeSet,
  staged,
  activated,
  committedOld,
  committedNew,
  committedMixed
>>

ValidatorSets == {"Old", "New"}
Heights == 1..MaxHeight
BoundaryHeight == ActivationHeight - 1

CommittedHeights ==
  committedOld \cup committedNew \cup committedMixed

HeightCommitted(h) == h \in CommittedHeights

TypeInvariant ==
  /\ MaxHeight \in Nat
  /\ MaxHeight >= 2
  /\ ActivationHeight \in 2..MaxHeight
  /\ BugDisableActivationFinalityGate \in BOOLEAN
  /\ BugAllowPrematureNewSetCertificate \in BOOLEAN
  /\ BugAllowMixedSetCertificate \in BOOLEAN
  /\ height \in Heights
  /\ activeSet \in ValidatorSets
  /\ staged \in BOOLEAN
  /\ activated \in BOOLEAN
  /\ committedOld \subseteq Heights
  /\ committedNew \subseteq Heights
  /\ committedMixed \subseteq Heights

Init ==
  /\ height = 1
  /\ activeSet = "Old"
  /\ staged = FALSE
  /\ activated = FALSE
  /\ committedOld = {}
  /\ committedNew = {}
  /\ committedMixed = {}

StageReconfiguration ==
  /\ ~staged
  /\ (BoundaryHeight \in committedOld \/ BugDisableActivationFinalityGate)
  /\ staged' = TRUE
  /\ UNCHANGED <<
      height,
      activeSet,
      activated,
      committedOld,
      committedNew,
      committedMixed
     >>

ActivateValidatorSet ==
  /\ staged
  /\ ~activated
  /\ ((height >= ActivationHeight /\ BoundaryHeight \in committedOld)
      \/ BugDisableActivationFinalityGate)
  /\ activeSet' = "New"
  /\ activated' = TRUE
  /\ UNCHANGED <<
      height,
      staged,
      committedOld,
      committedNew,
      committedMixed
     >>

OldSetCommit ==
  /\ ~HeightCommitted(height)
  /\ activeSet = "Old"
  /\ height < ActivationHeight
  /\ committedOld' = committedOld \cup {height}
  /\ UNCHANGED <<
      height,
      activeSet,
      staged,
      activated,
      committedNew,
      committedMixed
     >>

NewSetCommit ==
  /\ ~HeightCommitted(height)
  /\ (activeSet = "New" \/ BugAllowPrematureNewSetCertificate)
  /\ (height >= ActivationHeight \/ BugAllowPrematureNewSetCertificate)
  /\ committedNew' = committedNew \cup {height}
  /\ UNCHANGED <<
      height,
      activeSet,
      staged,
      activated,
      committedOld,
      committedMixed
     >>

MixedSetCommit ==
  /\ BugAllowMixedSetCertificate
  /\ ~HeightCommitted(height)
  /\ committedMixed' = committedMixed \cup {height}
  /\ UNCHANGED <<
      height,
      activeSet,
      staged,
      activated,
      committedOld,
      committedNew
     >>

AdvanceHeight ==
  /\ height < MaxHeight
  /\ HeightCommitted(height)
  /\ height' = height + 1
  /\ UNCHANGED <<
      activeSet,
      staged,
      activated,
      committedOld,
      committedNew,
      committedMixed
     >>

Next ==
  \/ StageReconfiguration
  \/ ActivateValidatorSet
  \/ OldSetCommit
  \/ NewSetCommit
  \/ MixedSetCommit
  \/ AdvanceHeight

ActivationRequiresOldBoundaryFinality ==
  activated => BoundaryHeight \in committedOld

NewCertificatesStartAtActivationHeight ==
  committedNew \subseteq ActivationHeight..MaxHeight

NewCertificatesRequireActivation ==
  committedNew # {} => activated

OldCertificatesStopBeforeActivationHeight ==
  committedOld \subseteq 1..BoundaryHeight

NoMixedValidatorSetCertificates ==
  committedMixed = {}

NoHeightCommittedByMultipleValidatorSets ==
  /\ committedOld \cap committedNew = {}
  /\ committedOld \cap committedMixed = {}
  /\ committedNew \cap committedMixed = {}

====

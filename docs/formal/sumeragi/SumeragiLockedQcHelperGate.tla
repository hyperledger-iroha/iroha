---- MODULE SumeragiLockedQcHelperGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi locked-QC helper rules.

This slice captures `ensure_locked_qc_allows(...)`,
`qc_extends_locked_with_lookup(...)`, `qc_satisfies_locked_with_lookup(...)`,
`qc_extends_locked_if_present(...)`, `qc_satisfies_locked_if_present(...)`,
and `realign_locked_to_committed_if_extends(...)`. It abstracts block hashes
and parent lookups to finite cases while preserving the observable contracts:
missing locks allow progress, height regressions and same-height hash
divergence fail closed, extension walks require explicit parent evidence,
newer-view QCs satisfy the lock before missing-payload checks, missing locked
payloads fail same-view checks, and committed-QC realignment occurs only when
the highest QC extends the committed QC.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

EnsureNoLockAllows == "ensure_no_lock_allows"
EnsureHigherHeightAllows == "ensure_higher_height_allows"
EnsureSameHashAllows == "ensure_same_hash_allows"
EnsureHeightRegressionRejects == "ensure_height_regression_rejects"
EnsureSameHeightHashMismatchRejects == "ensure_same_height_hash_mismatch_rejects"
ExtendsHeightRegressionFalse == "extends_height_regression_false"
ExtendsSameHashTrue == "extends_same_hash_true"
ExtendsDirectParentTrue == "extends_direct_parent_true"
ExtendsGrandparentTrue == "extends_grandparent_true"
ExtendsMissingParentFalse == "extends_missing_parent_false"
ExtendsDivergentParentFalse == "extends_divergent_parent_false"
SatisfiesNewerViewBypass == "satisfies_newer_view_bypass"
SatisfiesSameViewExtending == "satisfies_same_view_extending"
SatisfiesSameViewNonExtending == "satisfies_same_view_non_extending"
PresentExtendsNoLock == "present_extends_no_lock"
PresentExtendsMissingLockFalse == "present_extends_missing_lock_false"
PresentExtendsKnownLockTrue == "present_extends_known_lock_true"
PresentSatisfiesNoLock == "present_satisfies_no_lock"
PresentSatisfiesNewerViewSkipsPresence == "present_satisfies_newer_view_skips_presence"
PresentSatisfiesMissingLockSameViewFalse ==
  "present_satisfies_missing_lock_same_view_false"
PresentSatisfiesKnownLockSameViewTrue ==
  "present_satisfies_known_lock_same_view_true"
RealignNoCommittedKeepsLock == "realign_no_committed_keeps_lock"
RealignCommittedExtendedReturnsCommitted ==
  "realign_committed_extended_returns_committed"
RealignCommittedNotExtendedKeepsLock ==
  "realign_committed_not_extended_keeps_lock"
RealignNoExistingLockCommittedExtendedReturnsCommitted ==
  "realign_no_existing_lock_committed_extended_returns_committed"

Cases == {
  EnsureNoLockAllows,
  EnsureHigherHeightAllows,
  EnsureSameHashAllows,
  EnsureHeightRegressionRejects,
  EnsureSameHeightHashMismatchRejects,
  ExtendsHeightRegressionFalse,
  ExtendsSameHashTrue,
  ExtendsDirectParentTrue,
  ExtendsGrandparentTrue,
  ExtendsMissingParentFalse,
  ExtendsDivergentParentFalse,
  SatisfiesNewerViewBypass,
  SatisfiesSameViewExtending,
  SatisfiesSameViewNonExtending,
  PresentExtendsNoLock,
  PresentExtendsMissingLockFalse,
  PresentExtendsKnownLockTrue,
  PresentSatisfiesNoLock,
  PresentSatisfiesNewerViewSkipsPresence,
  PresentSatisfiesMissingLockSameViewFalse,
  PresentSatisfiesKnownLockSameViewTrue,
  RealignNoCommittedKeepsLock,
  RealignCommittedExtendedReturnsCommitted,
  RealignCommittedNotExtendedKeepsLock,
  RealignNoExistingLockCommittedExtendedReturnsCommitted
}

NoLockedQc == 1
LockedQcPresent == 2
EnsureAllow == 3
RejectHeightRegressed == 4
RejectHashMismatch == 5
ExtendsTrue == 6
ExtendsFalse == 7
SatisfiesTrue == 8
SatisfiesFalse == 9
ParentLookup == 10
MissingParentRejected == 11
DivergentParentRejected == 12
NewerViewBypass == 13
PresenceChecked == 14
PresenceSkipped == 15
MissingLockedRejected == 16
ReturnExistingLock == 17
ReturnCommittedLock == 18
ReturnNone == 19
NoCommittedQc == 20
SameHeightHashChecked == 21
CommittedExtensionChecked == 22

Actions == 1..22

SpecActions(c) ==
  CASE c = EnsureNoLockAllows ->
      {NoLockedQc, EnsureAllow}
    [] c = EnsureHigherHeightAllows ->
      {LockedQcPresent, EnsureAllow}
    [] c = EnsureSameHashAllows ->
      {LockedQcPresent, SameHeightHashChecked, EnsureAllow}
    [] c = EnsureHeightRegressionRejects ->
      {LockedQcPresent, RejectHeightRegressed}
    [] c = EnsureSameHeightHashMismatchRejects ->
      {LockedQcPresent, SameHeightHashChecked, RejectHashMismatch}
    [] c = ExtendsHeightRegressionFalse ->
      {LockedQcPresent, ExtendsFalse}
    [] c = ExtendsSameHashTrue ->
      {LockedQcPresent, ExtendsTrue}
    [] c = ExtendsDirectParentTrue ->
      {LockedQcPresent, ParentLookup, ExtendsTrue}
    [] c = ExtendsGrandparentTrue ->
      {LockedQcPresent, ParentLookup, ExtendsTrue}
    [] c = ExtendsMissingParentFalse ->
      {LockedQcPresent, ParentLookup, MissingParentRejected, ExtendsFalse}
    [] c = ExtendsDivergentParentFalse ->
      {LockedQcPresent, ParentLookup, DivergentParentRejected, ExtendsFalse}
    [] c = SatisfiesNewerViewBypass ->
      {LockedQcPresent, NewerViewBypass, SatisfiesTrue}
    [] c = SatisfiesSameViewExtending ->
      {LockedQcPresent, ParentLookup, ExtendsTrue, SatisfiesTrue}
    [] c = SatisfiesSameViewNonExtending ->
      {LockedQcPresent, ParentLookup, ExtendsFalse, SatisfiesFalse}
    [] c = PresentExtendsNoLock ->
      {NoLockedQc, ExtendsTrue}
    [] c = PresentExtendsMissingLockFalse ->
      {LockedQcPresent, PresenceChecked, MissingLockedRejected, ExtendsFalse}
    [] c = PresentExtendsKnownLockTrue ->
      {LockedQcPresent, PresenceChecked, ParentLookup, ExtendsTrue}
    [] c = PresentSatisfiesNoLock ->
      {NoLockedQc, SatisfiesTrue}
    [] c = PresentSatisfiesNewerViewSkipsPresence ->
      {LockedQcPresent, NewerViewBypass, PresenceSkipped, SatisfiesTrue}
    [] c = PresentSatisfiesMissingLockSameViewFalse ->
      {LockedQcPresent, PresenceChecked, MissingLockedRejected, SatisfiesFalse}
    [] c = PresentSatisfiesKnownLockSameViewTrue ->
      {LockedQcPresent, PresenceChecked, ParentLookup, ExtendsTrue, SatisfiesTrue}
    [] c = RealignNoCommittedKeepsLock ->
      {LockedQcPresent, NoCommittedQc, ReturnExistingLock}
    [] c = RealignCommittedExtendedReturnsCommitted ->
      {LockedQcPresent, CommittedExtensionChecked, ParentLookup, ExtendsTrue,
       ReturnCommittedLock}
    [] c = RealignCommittedNotExtendedKeepsLock ->
      {LockedQcPresent, CommittedExtensionChecked, ParentLookup, ExtendsFalse,
       ReturnExistingLock}
    [] c = RealignNoExistingLockCommittedExtendedReturnsCommitted ->
      {NoLockedQc, CommittedExtensionChecked, ParentLookup, ExtendsTrue,
       ReturnCommittedLock}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "ensure_allows_no_lock_rejected"
       /\ c = EnsureNoLockAllows ->
      (spec \ {EnsureAllow}) \cup {RejectHeightRegressed}
    [] Bug = "ensure_misses_height_regression"
       /\ c = EnsureHeightRegressionRejects ->
      (spec \ {RejectHeightRegressed}) \cup {EnsureAllow}
    [] Bug = "ensure_misses_same_height_hash_conflict"
       /\ c = EnsureSameHeightHashMismatchRejects ->
      (spec \ {RejectHashMismatch}) \cup {EnsureAllow}
    [] Bug = "ensure_rejects_matching_same_height"
       /\ c = EnsureSameHashAllows ->
      (spec \ {EnsureAllow}) \cup {RejectHashMismatch}
    [] Bug = "extends_allows_height_regression"
       /\ c = ExtendsHeightRegressionFalse ->
      (spec \ {ExtendsFalse}) \cup {ExtendsTrue}
    [] Bug = "extends_skips_same_hash"
       /\ c = ExtendsSameHashTrue ->
      (spec \ {ExtendsTrue}) \cup {ExtendsFalse}
    [] Bug = "extends_treats_missing_parent_as_true"
       /\ c = ExtendsMissingParentFalse ->
      (spec \ {MissingParentRejected, ExtendsFalse}) \cup {ExtendsTrue}
    [] Bug = "extends_treats_divergent_parent_as_true"
       /\ c = ExtendsDivergentParentFalse ->
      (spec \ {DivergentParentRejected, ExtendsFalse}) \cup {ExtendsTrue}
    [] Bug = "satisfies_requires_extension_for_newer_view"
       /\ c = SatisfiesNewerViewBypass ->
      (spec \ {NewerViewBypass, SatisfiesTrue}) \cup
        {ParentLookup, ExtendsFalse, SatisfiesFalse}
    [] Bug = "present_extends_ignores_missing_locked_payload"
       /\ c = PresentExtendsMissingLockFalse ->
      (spec \ {MissingLockedRejected, ExtendsFalse}) \cup {ExtendsTrue}
    [] Bug = "present_satisfies_checks_presence_before_newer_view"
       /\ c = PresentSatisfiesNewerViewSkipsPresence ->
      (spec \ {NewerViewBypass, PresenceSkipped, SatisfiesTrue}) \cup
        {PresenceChecked, MissingLockedRejected, SatisfiesFalse}
    [] Bug = "realign_without_committed_clears_lock"
       /\ c = RealignNoCommittedKeepsLock ->
      (spec \ {ReturnExistingLock}) \cup {ReturnNone}
    [] Bug = "realign_skips_committed_extension"
       /\ c = RealignCommittedExtendedReturnsCommitted ->
      (spec \ {ReturnCommittedLock}) \cup {ReturnExistingLock}
    [] Bug = "realign_accepts_nonextending_committed"
       /\ c = RealignCommittedNotExtendedKeepsLock ->
      (spec \ {ReturnExistingLock}) \cup {ReturnCommittedLock}
    [] Bug = "realign_requires_existing_lock"
       /\ c = RealignNoExistingLockCommittedExtendedReturnsCommitted ->
      (spec \ {ReturnCommittedLock}) \cup {ReturnNone}
    [] OTHER -> spec

Bugs == {
  "none",
  "ensure_allows_no_lock_rejected",
  "ensure_misses_height_regression",
  "ensure_misses_same_height_hash_conflict",
  "ensure_rejects_matching_same_height",
  "extends_allows_height_regression",
  "extends_skips_same_hash",
  "extends_treats_missing_parent_as_true",
  "extends_treats_divergent_parent_as_true",
  "satisfies_requires_extension_for_newer_view",
  "present_extends_ignores_missing_locked_payload",
  "present_satisfies_checks_presence_before_newer_view",
  "realign_without_committed_clears_lock",
  "realign_skips_committed_extension",
  "realign_accepts_nonextending_committed",
  "realign_requires_existing_lock"
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

LockedQcHelperCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

LockedQcHelperExactness ==
  LockedQcHelperCoreSafety

LockedQcHelperCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ LockedQcHelperExactness

NoBugInvariant == LockedQcHelperExactness

SafetyFast == LockedQcHelperExactness

BugEnsureAllowsNoLockRejected == NoBugInvariant
BugEnsureMissesHeightRegression == NoBugInvariant
BugEnsureMissesSameHeightHashConflict == NoBugInvariant
BugEnsureRejectsMatchingSameHeight == NoBugInvariant
BugExtendsAllowsHeightRegression == NoBugInvariant
BugExtendsSkipsSameHash == NoBugInvariant
BugExtendsTreatsMissingParentAsTrue == NoBugInvariant
BugExtendsTreatsDivergentParentAsTrue == NoBugInvariant
BugSatisfiesRequiresExtensionForNewerView == NoBugInvariant
BugPresentExtendsIgnoresMissingLockedPayload == NoBugInvariant
BugPresentSatisfiesChecksPresenceBeforeNewerView == NoBugInvariant
BugRealignWithoutCommittedClearsLock == NoBugInvariant
BugRealignSkipsCommittedExtension == NoBugInvariant
BugRealignAcceptsNonextendingCommitted == NoBugInvariant
BugRealignRequiresExistingLock == NoBugInvariant

====

---- MODULE SumeragiValidationRedriveLabelGate ----

EXTENDS FiniteSets, Integers

(***************************************************************************
A bounded abstract model for `VNextValidationRedriveReason::label()`.

Validation redrive reasons are emitted through the stale vNext validation
warning path. The model uses stable integer label codes to stand in for the
exact Rust string literals; comments next to each code show the corresponding
observable label. Each reason must map to one stable, distinct label so
operators and status consumers can distinguish orphaned queued work, orphaned
running work, stalled running work, and backpressured work without depending on
debug formatting.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == 0
BugOrphanedQueuedWrong == 1
BugOrphanedRunningWrong == 2
BugStalledRunningWrong == 3
BugBackpressuredWrong == 4
BugBackpressuredCollides == 5

WrongLabel == 0

OrphanedQueuedLabel == 10 \* "orphaned_queued"
OrphanedRunningLabel == 11 \* "orphaned_running"
StalledRunningLabel == 12 \* "stalled_running"
BackpressuredLabel == 13 \* "backpressured"

SpecOrphanedQueuedLabel == OrphanedQueuedLabel
SpecOrphanedRunningLabel == OrphanedRunningLabel
SpecStalledRunningLabel == StalledRunningLabel
SpecBackpressuredLabel == BackpressuredLabel

ActualOrphanedQueuedLabel ==
  IF Bug = BugOrphanedQueuedWrong
  THEN WrongLabel
  ELSE OrphanedQueuedLabel

ActualOrphanedRunningLabel ==
  IF Bug = BugOrphanedRunningWrong
  THEN WrongLabel
  ELSE OrphanedRunningLabel

ActualStalledRunningLabel ==
  IF Bug = BugStalledRunningWrong
  THEN WrongLabel
  ELSE StalledRunningLabel

ActualBackpressuredLabel ==
  IF Bug = BugBackpressuredWrong
  THEN WrongLabel
  ELSE IF Bug = BugBackpressuredCollides
  THEN StalledRunningLabel
  ELSE BackpressuredLabel

\* @type: <<Int, Int, Int, Int>>;
SpecLabels ==
  <<SpecOrphanedQueuedLabel,
    SpecOrphanedRunningLabel,
    SpecStalledRunningLabel,
    SpecBackpressuredLabel>>

\* @type: <<Int, Int, Int, Int>>;
ActualLabels ==
  <<ActualOrphanedQueuedLabel,
    ActualOrphanedRunningLabel,
    ActualStalledRunningLabel,
    ActualBackpressuredLabel>>

\* @type: Set(Int);
SpecLabelSet ==
  {SpecOrphanedQueuedLabel,
   SpecOrphanedRunningLabel,
   SpecStalledRunningLabel,
   SpecBackpressuredLabel}

\* @type: Set(Int);
ActualLabelSet ==
  {ActualOrphanedQueuedLabel,
   ActualOrphanedRunningLabel,
   ActualStalledRunningLabel,
   ActualBackpressuredLabel}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

ValidationRedriveLabelsMatchSpec ==
  ActualLabels = SpecLabels

LabelsDistinct ==
  Cardinality(ActualLabelSet) = 4

LabelsNonzero ==
  /\ ActualOrphanedQueuedLabel # WrongLabel
  /\ ActualOrphanedRunningLabel # WrongLabel
  /\ ActualStalledRunningLabel # WrongLabel
  /\ ActualBackpressuredLabel # WrongLabel

LabelSetStable ==
  ActualLabelSet = SpecLabelSet

ValidationRedriveLabelExactness ==
  /\ ValidationRedriveLabelsMatchSpec
  /\ LabelsDistinct
  /\ LabelsNonzero
  /\ LabelSetStable

ValidationRedriveLabelCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ValidationRedriveLabelExactness

SafetyFast ==
  ValidationRedriveLabelExactness

BugOrphanedQueuedLabelWrong ==
  ActualOrphanedQueuedLabel = SpecOrphanedQueuedLabel

BugOrphanedRunningLabelWrong ==
  ActualOrphanedRunningLabel = SpecOrphanedRunningLabel

BugStalledRunningLabelWrong ==
  ActualStalledRunningLabel = SpecStalledRunningLabel

BugBackpressuredLabelWrong ==
  ActualBackpressuredLabel = SpecBackpressuredLabel

BugBackpressuredLabelCollides ==
  ActualBackpressuredLabel = SpecBackpressuredLabel

====

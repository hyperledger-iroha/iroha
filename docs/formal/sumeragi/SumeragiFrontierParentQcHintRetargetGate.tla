---- MODULE SumeragiFrontierParentQcHintRetargetGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for contiguous-frontier missing-parent retargeting.

This slice pins `should_retarget_contiguous_frontier_parent_from_qc_hint(...)`
and the deferred-QC payload-hint branch in `request_missing_parent(...)`.
The helper allows retargeting immediately while frontier stall mode is active
for the exact `(frontier_height, local_height)` pair. Otherwise it allows
retargeting only when canonical frontier reanchor work was previously emitted
and dependency progress has not advanced since that emit. The request branch
then rewrites the target parent only for exact-frontier parents with a present,
different QC payload hint.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ExpectedSource == "expected_parent"
QcHintSource == "qc_hint"

Sources == {ExpectedSource, QcHintSource}

ExpectedHash == 1
HintHash == 2

Hashes == {ExpectedHash, HintHash}

StallActiveRetarget == "StallActiveRetarget"
StallWrongFrontierRejected == "StallWrongFrontierRejected"
UnchangedProgressRetarget == "UnchangedProgressRetarget"
EqualProgressRetarget == "EqualProgressRetarget"
ClearedProgressRetarget == "ClearedProgressRetarget"
CreatedProgressNoRetarget == "CreatedProgressNoRetarget"
AdvancedProgressNoRetarget == "AdvancedProgressNoRetarget"
NoPreviousEmitNoRetarget == "NoPreviousEmitNoRetarget"
AbsentGateNoRetarget == "AbsentGateNoRetarget"
NoHintNoRetarget == "NoHintNoRetarget"
SameHashNoRetarget == "SameHashNoRetarget"
NonFrontierParentNoRetarget == "NonFrontierParentNoRetarget"

Cases == {
  StallActiveRetarget,
  StallWrongFrontierRejected,
  UnchangedProgressRetarget,
  EqualProgressRetarget,
  ClearedProgressRetarget,
  CreatedProgressNoRetarget,
  AdvancedProgressNoRetarget,
  NoPreviousEmitNoRetarget,
  AbsentGateNoRetarget,
  NoHintNoRetarget,
  SameHashNoRetarget,
  NonFrontierParentNoRetarget
}

ParentIsFrontier(c) ==
  c /= NonFrontierParentNoRetarget

HintPresent(c) ==
  c /= NoHintNoRetarget

HintDiffers(c) ==
  c /= SameHashNoRetarget

SpecGate(c) ==
  c \in {
    StallActiveRetarget,
    UnchangedProgressRetarget,
    EqualProgressRetarget,
    ClearedProgressRetarget,
    NoHintNoRetarget,
    SameHashNoRetarget,
    NonFrontierParentNoRetarget
  }

ActualGate(c) ==
  CASE Bug = "stall_ignored"
       /\ c = StallActiveRetarget -> FALSE
    [] Bug = "stall_wrong_frontier_allowed"
       /\ c = StallWrongFrontierRejected -> TRUE
    [] Bug = "unchanged_progress_rejected"
       /\ c = UnchangedProgressRetarget -> FALSE
    [] Bug = "cleared_progress_rejected"
       /\ c = ClearedProgressRetarget -> FALSE
    [] Bug = "created_progress_allowed"
       /\ c = CreatedProgressNoRetarget -> TRUE
    [] Bug = "advanced_progress_allowed"
       /\ c = AdvancedProgressNoRetarget -> TRUE
    [] Bug = "no_previous_emit_allowed"
       /\ c = NoPreviousEmitNoRetarget -> TRUE
    [] Bug = "absent_gate_allowed"
       /\ c = AbsentGateNoRetarget -> TRUE
    [] OTHER -> SpecGate(c)

SpecRetarget(c) ==
  /\ ParentIsFrontier(c)
  /\ HintPresent(c)
  /\ HintDiffers(c)
  /\ SpecGate(c)

ActualRetarget(c) ==
  CASE Bug = "wrong_parent_height_allowed"
       /\ c = NonFrontierParentNoRetarget -> TRUE
    [] Bug = "no_hint_retargets"
       /\ c = NoHintNoRetarget -> TRUE
    [] Bug = "same_hash_retargets"
       /\ c = SameHashNoRetarget -> TRUE
    [] OTHER ->
       /\ ParentIsFrontier(c)
       /\ HintPresent(c)
       /\ HintDiffers(c)
       /\ ActualGate(c)

SpecSource(c) ==
  IF SpecRetarget(c) THEN QcHintSource ELSE ExpectedSource

ActualSource(c) ==
  CASE Bug = "target_not_rewritten"
       /\ c = StallActiveRetarget -> ExpectedSource
    [] ActualRetarget(c) -> QcHintSource
    [] OTHER -> ExpectedSource

SpecTarget(c) ==
  IF SpecRetarget(c) THEN HintHash ELSE ExpectedHash

ActualTarget(c) ==
  CASE Bug = "target_not_rewritten"
       /\ c = StallActiveRetarget -> ExpectedHash
    [] ActualRetarget(c) -> HintHash
    [] OTHER -> ExpectedHash

BugSet == {
  "none",
  "stall_ignored",
  "stall_wrong_frontier_allowed",
  "unchanged_progress_rejected",
  "cleared_progress_rejected",
  "created_progress_allowed",
  "advanced_progress_allowed",
  "no_previous_emit_allowed",
  "absent_gate_allowed",
  "wrong_parent_height_allowed",
  "no_hint_retargets",
  "same_hash_retargets",
  "target_not_rewritten"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 12
     /\ checked' = checked + 1
  \/ /\ checked = 12
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..12
  /\ \A c \in Cases: ActualGate(c) \in BOOLEAN
  /\ \A c \in Cases: ActualRetarget(c) \in BOOLEAN
  /\ \A c \in Cases: ActualSource(c) \in Sources
  /\ \A c \in Cases: ActualTarget(c) \in Hashes

GateExact ==
  \A c \in Cases:
    ActualGate(c) = SpecGate(c)

RetargetExact ==
  \A c \in Cases:
    ActualRetarget(c) = SpecRetarget(c)

RewriteExact ==
  \A c \in Cases:
    /\ ActualSource(c) = SpecSource(c)
    /\ ActualTarget(c) = SpecTarget(c)

StallBypassStable ==
  /\ ActualGate(StallActiveRetarget)
  /\ ~ActualGate(StallWrongFrontierRejected)
  /\ ActualRetarget(StallActiveRetarget)
  /\ ~ActualRetarget(StallWrongFrontierRejected)

ProgressGateStable ==
  /\ ActualRetarget(UnchangedProgressRetarget)
  /\ ActualRetarget(EqualProgressRetarget)
  /\ ActualRetarget(ClearedProgressRetarget)
  /\ ~ActualRetarget(CreatedProgressNoRetarget)
  /\ ~ActualRetarget(AdvancedProgressNoRetarget)
  /\ ~ActualRetarget(NoPreviousEmitNoRetarget)
  /\ ~ActualRetarget(AbsentGateNoRetarget)

RequestBranchStable ==
  /\ ~ActualRetarget(NoHintNoRetarget)
  /\ ~ActualRetarget(SameHashNoRetarget)
  /\ ~ActualRetarget(NonFrontierParentNoRetarget)

RewriteStable ==
  /\ ActualSource(StallActiveRetarget) = QcHintSource
  /\ ActualTarget(StallActiveRetarget) = HintHash
  /\ ActualSource(UnchangedProgressRetarget) = QcHintSource
  /\ ActualTarget(UnchangedProgressRetarget) = HintHash
  /\ ActualSource(NoPreviousEmitNoRetarget) = ExpectedSource
  /\ ActualTarget(NoPreviousEmitNoRetarget) = ExpectedHash

ParentQcHintRetargetHasExactPositiveEvidence ==
  /\ ActualGate(StallActiveRetarget)
  /\ ActualRetarget(StallActiveRetarget)
  /\ ActualRetarget(UnchangedProgressRetarget)
  /\ ActualRetarget(EqualProgressRetarget)
  /\ ActualRetarget(ClearedProgressRetarget)

ParentQcHintRetargetRejectsIneligibleInputs ==
  /\ ~ActualGate(StallWrongFrontierRejected)
  /\ ~ActualRetarget(StallWrongFrontierRejected)
  /\ ~ActualRetarget(CreatedProgressNoRetarget)
  /\ ~ActualRetarget(AdvancedProgressNoRetarget)
  /\ ~ActualRetarget(NoPreviousEmitNoRetarget)
  /\ ~ActualRetarget(AbsentGateNoRetarget)
  /\ ~ActualRetarget(NoHintNoRetarget)
  /\ ~ActualRetarget(SameHashNoRetarget)
  /\ ~ActualRetarget(NonFrontierParentNoRetarget)

ParentQcHintRetargetPreservesRewriteTargets ==
  /\ ActualSource(StallActiveRetarget) = QcHintSource
  /\ ActualTarget(StallActiveRetarget) = HintHash
  /\ ActualSource(UnchangedProgressRetarget) = QcHintSource
  /\ ActualTarget(UnchangedProgressRetarget) = HintHash
  /\ ActualSource(EqualProgressRetarget) = QcHintSource
  /\ ActualTarget(EqualProgressRetarget) = HintHash
  /\ ActualSource(ClearedProgressRetarget) = QcHintSource
  /\ ActualTarget(ClearedProgressRetarget) = HintHash
  /\ ActualSource(StallWrongFrontierRejected) = ExpectedSource
  /\ ActualTarget(StallWrongFrontierRejected) = ExpectedHash
  /\ ActualSource(NoPreviousEmitNoRetarget) = ExpectedSource
  /\ ActualTarget(NoPreviousEmitNoRetarget) = ExpectedHash
  /\ ActualSource(NoHintNoRetarget) = ExpectedSource
  /\ ActualTarget(NoHintNoRetarget) = ExpectedHash
  /\ ActualSource(SameHashNoRetarget) = ExpectedSource
  /\ ActualTarget(SameHashNoRetarget) = ExpectedHash
  /\ ActualSource(NonFrontierParentNoRetarget) = ExpectedSource
  /\ ActualTarget(NonFrontierParentNoRetarget) = ExpectedHash

FrontierParentQcHintRetargetExactness ==
  /\ GateExact
  /\ RetargetExact
  /\ RewriteExact
  /\ StallBypassStable
  /\ ProgressGateStable
  /\ RequestBranchStable
  /\ RewriteStable
  /\ ParentQcHintRetargetHasExactPositiveEvidence
  /\ ParentQcHintRetargetRejectsIneligibleInputs
  /\ ParentQcHintRetargetPreservesRewriteTargets

SafetyFast == FrontierParentQcHintRetargetExactness

GateAnchors ==
  /\ GateExact
  /\ \A c \in Cases: ActualGate(c) = SpecGate(c)

RetargetAnchors ==
  /\ RetargetExact
  /\ \A c \in Cases: ActualRetarget(c) = SpecRetarget(c)

RewriteAnchors ==
  /\ RewriteExact
  /\ \A c \in Cases:
       /\ ActualSource(c) = SpecSource(c)
       /\ ActualTarget(c) = SpecTarget(c)

StallBypassAnchors ==
  /\ StallBypassStable
  /\ ActualGate(StallActiveRetarget)
  /\ ~ActualGate(StallWrongFrontierRejected)
  /\ ActualRetarget(StallActiveRetarget)
  /\ ~ActualRetarget(StallWrongFrontierRejected)

ProgressGateAnchors ==
  /\ ProgressGateStable
  /\ ActualRetarget(UnchangedProgressRetarget)
  /\ ActualRetarget(EqualProgressRetarget)
  /\ ActualRetarget(ClearedProgressRetarget)
  /\ ~ActualRetarget(CreatedProgressNoRetarget)
  /\ ~ActualRetarget(AdvancedProgressNoRetarget)
  /\ ~ActualRetarget(NoPreviousEmitNoRetarget)
  /\ ~ActualRetarget(AbsentGateNoRetarget)

RequestBranchAnchors ==
  /\ RequestBranchStable
  /\ ~ActualRetarget(NoHintNoRetarget)
  /\ ~ActualRetarget(SameHashNoRetarget)
  /\ ~ActualRetarget(NonFrontierParentNoRetarget)

RewriteStableAnchors ==
  /\ RewriteStable
  /\ ActualSource(StallActiveRetarget) = QcHintSource
  /\ ActualTarget(StallActiveRetarget) = HintHash
  /\ ActualSource(UnchangedProgressRetarget) = QcHintSource
  /\ ActualTarget(UnchangedProgressRetarget) = HintHash
  /\ ActualSource(NoPreviousEmitNoRetarget) = ExpectedSource
  /\ ActualTarget(NoPreviousEmitNoRetarget) = ExpectedHash

FrontierParentQcHintRetargetSafetyAnchors ==
  /\ GateAnchors
  /\ RetargetAnchors
  /\ RewriteAnchors
  /\ StallBypassAnchors
  /\ ProgressGateAnchors
  /\ RequestBranchAnchors
  /\ RewriteStableAnchors

FrontierParentQcHintRetargetCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FrontierParentQcHintRetargetExactness

Safety == FrontierParentQcHintRetargetSafetyAnchors

====

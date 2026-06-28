---- MODULE SumeragiEnginePrepareQcGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for the pure Sumeragi engine prepare-QC gate.

This slice models `ConsensusEngine::on_certificate(...)` dispatching a current
view prepare certificate into `on_prepare_qc(...)`. A prepare QC may emit one
local commit vote only when the certificate is for the current height, epoch,
validator set, view, and quorum policy; the height is not already committed;
the engine has not already emitted a commit vote for that round; and the engine
is not waiting for missing payload finality.

The accepted prepare QC must also update the locked QC and highest QC to the
prepare certificate. The model enumerates the finite guard cases that matter
for this engine boundary; `SpecMaySign` is the reference contract, and the
implementation transition records whether each candidate certificate signs,
ignores, locks, and records highest-QC state.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSignWrongContext,
  \* @type: Bool;
  BugSignWrongQuorumPolicy,
  \* @type: Bool;
  BugSignStaleView,
  \* @type: Bool;
  BugSignCommittedHeight,
  \* @type: Bool;
  BugSignReplayPrepare,
  \* @type: Bool;
  BugSignConflictingPrepare,
  \* @type: Bool;
  BugSignPendingFinality,
  \* @type: Bool;
  BugRejectSafePrepare,
  \* @type: Bool;
  BugSkipLockRecord

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Set(Str);
  signed,
  \* @type: Set(Str);
  ignored,
  \* @type: Set(Str);
  locked,
  \* @type: Set(Str);
  highest

\* @type: <<Set(Str), Set(Str), Set(Str), Set(Str), Set(Str)>>;
vars == <<tried, signed, ignored, locked, highest>>

Candidates == {
  "safePrepareQc",
  "wrongHeight",
  "wrongEpoch",
  "wrongValidatorSet",
  "wrongQuorumPolicy",
  "staleView",
  "committedHeight",
  "replaySamePrepareQc",
  "conflictingPrepareQc",
  "pendingFinality"
}

SpecMaySign(candidate) ==
  candidate = "safePrepareQc"

BugAllowsUnsafe(candidate) ==
  \/ /\ candidate \in {"wrongHeight", "wrongEpoch", "wrongValidatorSet"}
     /\ BugSignWrongContext
  \/ /\ candidate = "wrongQuorumPolicy"
     /\ BugSignWrongQuorumPolicy
  \/ /\ candidate = "staleView"
     /\ BugSignStaleView
  \/ /\ candidate = "committedHeight"
     /\ BugSignCommittedHeight
  \/ /\ candidate = "replaySamePrepareQc"
     /\ BugSignReplayPrepare
  \/ /\ candidate = "conflictingPrepareQc"
     /\ BugSignConflictingPrepare
  \/ /\ candidate = "pendingFinality"
     /\ BugSignPendingFinality

ImplementationSigns(candidate) ==
  IF SpecMaySign(candidate)
  THEN ~BugRejectSafePrepare
  ELSE BugAllowsUnsafe(candidate)

ImplementationRecordsLock(candidate) ==
  ImplementationSigns(candidate) /\ ~BugSkipLockRecord

TypeInvariant ==
  /\ BugSignWrongContext \in BOOLEAN
  /\ BugSignWrongQuorumPolicy \in BOOLEAN
  /\ BugSignStaleView \in BOOLEAN
  /\ BugSignCommittedHeight \in BOOLEAN
  /\ BugSignReplayPrepare \in BOOLEAN
  /\ BugSignConflictingPrepare \in BOOLEAN
  /\ BugSignPendingFinality \in BOOLEAN
  /\ BugRejectSafePrepare \in BOOLEAN
  /\ BugSkipLockRecord \in BOOLEAN
  /\ tried \subseteq Candidates
  /\ signed \subseteq Candidates
  /\ ignored \subseteq Candidates
  /\ locked \subseteq Candidates
  /\ highest \subseteq Candidates
  /\ signed \cap ignored = {}
  /\ signed \cup ignored = tried

Init ==
  /\ tried = {}
  /\ signed = {}
  /\ ignored = {}
  /\ locked = {}
  /\ highest = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ IF ImplementationSigns(candidate)
     THEN
       /\ signed' = signed \cup {candidate}
       /\ ignored' = ignored
     ELSE
       /\ signed' = signed
       /\ ignored' = ignored \cup {candidate}
  /\ IF ImplementationRecordsLock(candidate)
     THEN
       /\ locked' = locked \cup {candidate}
       /\ highest' = highest \cup {candidate}
     ELSE
       /\ locked' = locked
       /\ highest' = highest

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

SignedMatchesSpec ==
  signed \subseteq {candidate \in Candidates : SpecMaySign(candidate)}

IgnoredMatchesSpec ==
  ignored \subseteq {candidate \in Candidates : ~SpecMaySign(candidate)}

SafePrepareQcsSign ==
  \A candidate \in tried:
    SpecMaySign(candidate) => candidate \in signed

UnsafePrepareQcsAreIgnored ==
  \A candidate \in tried:
    ~SpecMaySign(candidate) => candidate \in ignored

WrongContextNeverSigns ==
  /\ "wrongHeight" \notin signed
  /\ "wrongEpoch" \notin signed
  /\ "wrongValidatorSet" \notin signed

WrongQuorumPolicyNeverSigns ==
  "wrongQuorumPolicy" \notin signed

StaleViewNeverSigns ==
  "staleView" \notin signed

CommittedHeightNeverSigns ==
  "committedHeight" \notin signed

ReplayPrepareNeverSigns ==
  "replaySamePrepareQc" \notin signed

ConflictingPrepareNeverSigns ==
  "conflictingPrepareQc" \notin signed

PendingFinalityNeverSigns ==
  "pendingFinality" \notin signed

SignedPrepareRecordsLock ==
  signed \subseteq locked

SignedPrepareRecordsHighest ==
  signed \subseteq highest

IgnoredPrepareDoesNotMutateLock ==
  ignored \cap (locked \cup highest) = {}

LockAndHighestFollowSigned ==
  locked \cup highest \subseteq signed

EnginePrepareQcExactness ==
  /\ SignedMatchesSpec
  /\ IgnoredMatchesSpec
  /\ SafePrepareQcsSign
  /\ UnsafePrepareQcsAreIgnored
  /\ WrongContextNeverSigns
  /\ WrongQuorumPolicyNeverSigns
  /\ StaleViewNeverSigns
  /\ CommittedHeightNeverSigns
  /\ ReplayPrepareNeverSigns
  /\ ConflictingPrepareNeverSigns
  /\ PendingFinalityNeverSigns
  /\ SignedPrepareRecordsLock
  /\ SignedPrepareRecordsHighest
  /\ IgnoredPrepareDoesNotMutateLock
  /\ LockAndHighestFollowSigned

EnginePrepareQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EnginePrepareQcExactness

SafetyFast == EnginePrepareQcExactness

====

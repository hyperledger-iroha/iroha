---- MODULE SumeragiV2AdequateLeaderPeriodicPrefixMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite mutation kernel for the adequate-leader periodic scheduler prefix.

The frozen timeout owns shared ordinal four.  Periodic ordinal three already
exists behind an older Candidate when the selected occurrence is frozen.  The
repair snapshots that latent predecessor, drains the Candidate, retires the
same periodic identity, and allocates any later clock episode at the shared
high-watermark (five), where it cannot precede the timeout.  Only then may the
ordinary finite Candidate/Wire owner episode start.

The hidden-prefix mutation snapshots only the currently executable periodic
owner.  It therefore misses ordinal three while the Candidate is ahead and
starts the finite owner episode with an unserviced pre-timeout predecessor.
The replenishment mutation services ordinal three but recreates that same
ordinal forever.  It is a liveness counterexample: replacement itself is not
progress.

These bounded pairs are mutation evidence for the repaired ownership cut, not
a deductive proof of the asynchronous production specification.
***************************************************************************)

TimeoutOrdinal == 4

Phases ==
  {"CandidatePrefix", "PeriodicPrefix", "Drained",
   "FreshLater", "FiniteOwner", "Done"}

VARIABLES
  phase,
  retransmitOrdinal,
  nextOrdinal,
  candidateAhead,
  frozenSnapshot,
  retiredOrdinals,
  replacementEpoch,
  targetDone

mutationVars ==
  <<phase, retransmitOrdinal, nextOrdinal, candidateAhead,
    frozenSnapshot, retiredOrdinals, replacementEpoch, targetDone>>

PeriodicPredecessorOrdinals ==
  IF /\ retransmitOrdinal # 0
     /\ retransmitOrdinal < TimeoutOrdinal
  THEN {retransmitOrdinal}
  ELSE {}

PeriodicRuntimeReady ==
  /\ PeriodicPredecessorOrdinals # {}
  /\ ~candidateAhead

FrozenSnapshotRetired ==
  frozenSnapshot \subseteq retiredOrdinals

FixedInit ==
  /\ phase = "CandidatePrefix"
  /\ retransmitOrdinal = 3
  /\ nextOrdinal = 5
  /\ candidateAhead
  /\ frozenSnapshot = {3}
  /\ retiredOrdinals = {}
  /\ ~replacementEpoch
  /\ ~targetDone

HiddenPrefixBugInit ==
  /\ phase = "CandidatePrefix"
  /\ retransmitOrdinal = 3
  /\ nextOrdinal = 5
  /\ candidateAhead
  \* Mutation: snapshot only the executable periodic owner.  The older
  \* Candidate hides the already-owned ordinal three.
  /\ frozenSnapshot = {}
  /\ retiredOrdinals = {}
  /\ ~replacementEpoch
  /\ ~targetDone

ReplenishmentBugInit ==
  /\ phase = "PeriodicPrefix"
  /\ retransmitOrdinal = 3
  /\ nextOrdinal = 5
  /\ ~candidateAhead
  /\ frozenSnapshot = {3}
  /\ retiredOrdinals = {}
  /\ ~replacementEpoch
  /\ ~targetDone

DrainOlderCandidate ==
  /\ phase = "CandidatePrefix"
  /\ candidateAhead
  /\ phase' = "PeriodicPrefix"
  /\ ~candidateAhead'
  /\ UNCHANGED
       <<retransmitOrdinal, nextOrdinal, frozenSnapshot,
         retiredOrdinals, replacementEpoch, targetDone>>

ServiceFrozenPeriodicIdentity ==
  /\ phase = "PeriodicPrefix"
  /\ PeriodicRuntimeReady
  /\ retransmitOrdinal \in frozenSnapshot
  /\ phase' = "Drained"
  /\ retransmitOrdinal' = 0
  /\ retiredOrdinals' = retiredOrdinals \cup {retransmitOrdinal}
  /\ UNCHANGED
       <<nextOrdinal, candidateAhead, frozenSnapshot,
         replacementEpoch, targetDone>>

AcquireFreshPeriodicAtSharedHighWatermark ==
  /\ phase = "Drained"
  /\ retransmitOrdinal = 0
  /\ FrozenSnapshotRetired
  /\ phase' = "FreshLater"
  /\ retransmitOrdinal' = nextOrdinal
  /\ nextOrdinal' = nextOrdinal + 1
  /\ UNCHANGED
       <<candidateAhead, frozenSnapshot, retiredOrdinals,
         replacementEpoch, targetDone>>

StartFiniteOwnerEpisode ==
  /\ phase = "FreshLater"
  /\ FrozenSnapshotRetired
  /\ PeriodicPredecessorOrdinals = {}
  /\ phase' = "FiniteOwner"
  /\ UNCHANGED
       <<retransmitOrdinal, nextOrdinal, candidateAhead, frozenSnapshot,
         retiredOrdinals, replacementEpoch, targetDone>>

ServiceTargetOccurrence ==
  /\ phase = "FiniteOwner"
  /\ phase' = "Done"
  /\ targetDone'
  /\ UNCHANGED
       <<retransmitOrdinal, nextOrdinal, candidateAhead, frozenSnapshot,
         retiredOrdinals, replacementEpoch>>

StartFiniteOwnerEpisodeWithHiddenPeriodicPrefix ==
  /\ phase = "CandidatePrefix"
  /\ frozenSnapshot = {}
  /\ phase' = "FiniteOwner"
  /\ UNCHANGED
       <<retransmitOrdinal, nextOrdinal, candidateAhead, frozenSnapshot,
         retiredOrdinals, replacementEpoch, targetDone>>

ReplaceRetiredPeriodicAtSameOrdinal ==
  /\ phase = "PeriodicPrefix"
  /\ PeriodicRuntimeReady
  /\ retransmitOrdinal = 3
  /\ phase' = "PeriodicPrefix"
  /\ retransmitOrdinal' = 3
  /\ retiredOrdinals' = retiredOrdinals \cup {3}
  /\ replacementEpoch' = ~replacementEpoch
  /\ UNCHANGED
       <<nextOrdinal, candidateAhead, frozenSnapshot, targetDone>>

FixedNext ==
  \/ DrainOlderCandidate
  \/ ServiceFrozenPeriodicIdentity
  \/ AcquireFreshPeriodicAtSharedHighWatermark
  \/ StartFiniteOwnerEpisode
  \/ ServiceTargetOccurrence

HiddenPrefixBugNext ==
  StartFiniteOwnerEpisodeWithHiddenPeriodicPrefix

ReplenishmentBugNext ==
  ReplaceRetiredPeriodicAtSameOrdinal

FixedSpec ==
  /\ FixedInit
  /\ [][FixedNext]_mutationVars
  /\ WF_mutationVars(FixedNext)

HiddenPrefixBugSpec ==
  /\ HiddenPrefixBugInit
  /\ [][HiddenPrefixBugNext]_mutationVars
  /\ WF_mutationVars(HiddenPrefixBugNext)

ReplenishmentBugSpec ==
  /\ ReplenishmentBugInit
  /\ [][ReplenishmentBugNext]_mutationVars
  /\ WF_mutationVars(ReplenishmentBugNext)

MutationTypeInvariant ==
  /\ phase \in Phases
  /\ retransmitOrdinal \in Nat
  /\ nextOrdinal \in Nat \ {0}
  /\ candidateAhead \in BOOLEAN
  /\ frozenSnapshot \subseteq 1..TimeoutOrdinal
  /\ retiredOrdinals \subseteq 1..TimeoutOrdinal
  /\ replacementEpoch \in BOOLEAN
  /\ targetDone \in BOOLEAN

SharedHighWatermarkStaysAheadOfLivePeriodicOwner ==
  retransmitOrdinal # 0 => retransmitOrdinal < nextOrdinal

FrozenPeriodicSnapshotCannotReplenish ==
  PeriodicPredecessorOrdinals \subseteq frozenSnapshot

RetiredPeriodicIdentityCannotResurrect ==
  retiredOrdinals \cap PeriodicPredecessorOrdinals = {}

FiniteOwnerEpisodeStartsAfterPeriodicPrefixDrains ==
  phase \in {"FiniteOwner", "Done"}
    => /\ FrozenSnapshotRetired
       /\ PeriodicPredecessorOrdinals = {}

TargetEventuallyDone ==
  <>targetDone

=============================================================================

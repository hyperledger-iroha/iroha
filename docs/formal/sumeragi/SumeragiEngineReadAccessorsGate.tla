---- MODULE SumeragiEngineReadAccessorsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pure-engine read-only accessors.

This slice models the public `ConsensusEngine::state()` and
`ConsensusEngine::committed_at(height)` accessors. Accessors must expose the
exact current engine snapshot and exact committed-block lookup for the queried
height without mutating consensus state or emitting adapter commands.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugStateWrongRound,
  \* @type: Bool;
  BugStateWrongPhase,
  \* @type: Bool;
  BugStateWrongLock,
  \* @type: Bool;
  BugStateWrongHighest,
  \* @type: Bool;
  BugStateWrongPending,
  \* @type: Bool;
  BugStateWrongQuorum,
  \* @type: Bool;
  BugCommittedAbsentSome,
  \* @type: Bool;
  BugCommittedWrongHeight,
  \* @type: Bool;
  BugCommittedWrongBlock,
  \* @type: Bool;
  BugCommittedForMissingHeight,
  \* @type: Bool;
  BugStateAccessorMutatesState,
  \* @type: Bool;
  BugCommittedAccessorMutatesMap,
  \* @type: Bool;
  BugAccessorEmitsOutput

VARIABLES
  \* @type: Set(Str);
  tried,
  \* @type: Str;
  last_case,
  \* @type: Str;
  round_read,
  \* @type: Str;
  phase_read,
  \* @type: Str;
  lock_read,
  \* @type: Str;
  highest_read,
  \* @type: Str;
  pending_read,
  \* @type: Str;
  quorum_read,
  \* @type: Str;
  committed_read,
  \* @type: Str;
  post_round,
  \* @type: Str;
  post_phase,
  \* @type: Str;
  post_lock,
  \* @type: Str;
  post_highest,
  \* @type: Str;
  post_pending,
  \* @type: Str;
  post_quorum,
  \* @type: Str;
  post_height_one,
  \* @type: Str;
  post_height_two,
  \* @type: Set(Str);
  outputs

vars == <<tried, last_case, round_read, phase_read, lock_read, highest_read,
  pending_read, quorum_read, committed_read, post_round, post_phase,
  post_lock, post_highest, post_pending, post_quorum, post_height_one,
  post_height_two, outputs>>

Cases == {
  "state_full",
  "state_empty",
  "committed_exact_h1",
  "committed_exact_h2",
  "committed_missing_h1",
  "committed_missing_h2"
}

Rounds == {"none", "round0", "round1", "round2", "wrong_round"}
Phases == {"none", "Proposal", "Prepare", "Commit", "PendingFinality", "wrong_phase"}
Locks == {"none", "lock_a", "wrong_lock"}
HighestQcs == {"none", "highest_a", "highest_b", "wrong_highest"}
PendingSubjects == {"none", "pending_a", "wrong_pending"}
Quorums == {"none", "quorum4", "quorum7", "wrong_quorum"}
Blocks == {"none", "block_a", "block_b", "block_c", "wrong_block"}
OutputDomain == {"spurious_output"}

SpecRound(c) ==
  CASE c = "state_empty" -> "round0"
    [] c = "committed_exact_h2" -> "round2"
    [] OTHER -> "round1"

SpecPhase(c) ==
  CASE c = "state_full" -> "PendingFinality"
    [] c = "state_empty" -> "Proposal"
    [] c = "committed_exact_h1" -> "Commit"
    [] c = "committed_exact_h2" -> "Prepare"
    [] OTHER -> "Proposal"

SpecLock(c) ==
  CASE c \in {"state_full", "committed_exact_h1"} -> "lock_a"
    [] OTHER -> "none"

SpecHighest(c) ==
  CASE c = "state_full" -> "highest_b"
    [] c \in {"committed_exact_h1", "committed_exact_h2"} -> "highest_a"
    [] OTHER -> "none"

SpecPending(c) ==
  CASE c = "state_full" -> "pending_a"
    [] OTHER -> "none"

SpecQuorum(c) ==
  CASE c = "state_empty" -> "quorum7"
    [] OTHER -> "quorum4"

QueryHeight(c) ==
  CASE c \in {"committed_exact_h2", "committed_missing_h2"} -> 2
    [] OTHER -> 1

OtherHeight(c) ==
  IF QueryHeight(c) = 1 THEN 2 ELSE 1

PreCommitAt(c, height) ==
  CASE /\ c \in {"state_full", "committed_exact_h1", "committed_exact_h2", "committed_missing_h2"}
       /\ height = 1 -> "block_a"
    [] /\ c \in {"committed_exact_h1", "committed_exact_h2", "committed_missing_h1"}
       /\ height = 2 -> "block_b"
    [] OTHER -> "none"

SpecCommittedAt(c) ==
  PreCommitAt(c, QueryHeight(c))

OtherRound(round) ==
  IF round = "round0" THEN "round1" ELSE "wrong_round"

OtherPhase(phase) ==
  IF phase = "Proposal" THEN "Commit" ELSE "wrong_phase"

OtherLock(lock) ==
  IF lock = "none" THEN "wrong_lock" ELSE "none"

OtherHighest(highest) ==
  IF highest = "none" THEN "wrong_highest" ELSE "none"

OtherPending(pending) ==
  IF pending = "none" THEN "wrong_pending" ELSE "none"

OtherQuorum(quorum) ==
  IF quorum = "quorum4" THEN "quorum7" ELSE "wrong_quorum"

ImplementationRound(c) ==
  IF BugStateWrongRound THEN OtherRound(SpecRound(c)) ELSE SpecRound(c)

ImplementationPhase(c) ==
  IF BugStateWrongPhase THEN OtherPhase(SpecPhase(c)) ELSE SpecPhase(c)

ImplementationLock(c) ==
  IF BugStateWrongLock THEN OtherLock(SpecLock(c)) ELSE SpecLock(c)

ImplementationHighest(c) ==
  IF BugStateWrongHighest THEN OtherHighest(SpecHighest(c)) ELSE SpecHighest(c)

ImplementationPending(c) ==
  IF BugStateWrongPending THEN OtherPending(SpecPending(c)) ELSE SpecPending(c)

ImplementationQuorum(c) ==
  IF BugStateWrongQuorum THEN OtherQuorum(SpecQuorum(c)) ELSE SpecQuorum(c)

ImplementationCommittedAt(c) ==
  IF BugCommittedAbsentSome /\ SpecCommittedAt(c) # "none"
  THEN "none"
  ELSE IF BugCommittedWrongHeight
  THEN PreCommitAt(c, OtherHeight(c))
  ELSE IF BugCommittedWrongBlock /\ SpecCommittedAt(c) # "none"
  THEN "wrong_block"
  ELSE IF BugCommittedForMissingHeight /\ SpecCommittedAt(c) = "none"
  THEN "wrong_block"
  ELSE SpecCommittedAt(c)

PostRound(c) ==
  IF BugStateAccessorMutatesState THEN OtherRound(SpecRound(c)) ELSE SpecRound(c)

PostPhase(c) ==
  IF BugStateAccessorMutatesState THEN OtherPhase(SpecPhase(c)) ELSE SpecPhase(c)

PostHeightOne(c) ==
  IF BugCommittedAccessorMutatesMap THEN "none" ELSE PreCommitAt(c, 1)

PostHeightTwo(c) ==
  IF BugCommittedAccessorMutatesMap THEN "block_c" ELSE PreCommitAt(c, 2)

ImplementationOutputs ==
  IF BugAccessorEmitsOutput THEN {"spurious_output"} ELSE {}

TypeInvariant ==
  /\ BugStateWrongRound \in BOOLEAN
  /\ BugStateWrongPhase \in BOOLEAN
  /\ BugStateWrongLock \in BOOLEAN
  /\ BugStateWrongHighest \in BOOLEAN
  /\ BugStateWrongPending \in BOOLEAN
  /\ BugStateWrongQuorum \in BOOLEAN
  /\ BugCommittedAbsentSome \in BOOLEAN
  /\ BugCommittedWrongHeight \in BOOLEAN
  /\ BugCommittedWrongBlock \in BOOLEAN
  /\ BugCommittedForMissingHeight \in BOOLEAN
  /\ BugStateAccessorMutatesState \in BOOLEAN
  /\ BugCommittedAccessorMutatesMap \in BOOLEAN
  /\ BugAccessorEmitsOutput \in BOOLEAN
  /\ tried \subseteq Cases
  /\ last_case \in Cases \union {"none"}
  /\ round_read \in Rounds
  /\ phase_read \in Phases
  /\ lock_read \in Locks
  /\ highest_read \in HighestQcs
  /\ pending_read \in PendingSubjects
  /\ quorum_read \in Quorums
  /\ committed_read \in Blocks
  /\ post_round \in Rounds
  /\ post_phase \in Phases
  /\ post_lock \in Locks
  /\ post_highest \in HighestQcs
  /\ post_pending \in PendingSubjects
  /\ post_quorum \in Quorums
  /\ post_height_one \in Blocks
  /\ post_height_two \in Blocks
  /\ outputs \subseteq OutputDomain

Init ==
  /\ tried = {}
  /\ last_case = "none"
  /\ round_read = "none"
  /\ phase_read = "none"
  /\ lock_read = "none"
  /\ highest_read = "none"
  /\ pending_read = "none"
  /\ quorum_read = "none"
  /\ committed_read = "none"
  /\ post_round = "none"
  /\ post_phase = "none"
  /\ post_lock = "none"
  /\ post_highest = "none"
  /\ post_pending = "none"
  /\ post_quorum = "none"
  /\ post_height_one = "none"
  /\ post_height_two = "none"
  /\ outputs = {}

ReadAccessors(c) ==
  /\ c \in Cases
  /\ tried' = tried \union {c}
  /\ last_case' = c
  /\ round_read' = ImplementationRound(c)
  /\ phase_read' = ImplementationPhase(c)
  /\ lock_read' = ImplementationLock(c)
  /\ highest_read' = ImplementationHighest(c)
  /\ pending_read' = ImplementationPending(c)
  /\ quorum_read' = ImplementationQuorum(c)
  /\ committed_read' = ImplementationCommittedAt(c)
  /\ post_round' = PostRound(c)
  /\ post_phase' = PostPhase(c)
  /\ post_lock' = SpecLock(c)
  /\ post_highest' = SpecHighest(c)
  /\ post_pending' = SpecPending(c)
  /\ post_quorum' = SpecQuorum(c)
  /\ post_height_one' = PostHeightOne(c)
  /\ post_height_two' = PostHeightTwo(c)
  /\ outputs' = ImplementationOutputs

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: ReadAccessors(c)
  \/ Stable

StateSnapshotMatchesFields ==
  last_case = "none" \/
    /\ round_read = SpecRound(last_case)
    /\ phase_read = SpecPhase(last_case)
    /\ lock_read = SpecLock(last_case)
    /\ highest_read = SpecHighest(last_case)
    /\ pending_read = SpecPending(last_case)
    /\ quorum_read = SpecQuorum(last_case)

CommittedAtMatchesQueriedHeight ==
  last_case = "none" \/ committed_read = SpecCommittedAt(last_case)

AccessorsDoNotMutateState ==
  last_case = "none" \/
    /\ post_round = SpecRound(last_case)
    /\ post_phase = SpecPhase(last_case)
    /\ post_lock = SpecLock(last_case)
    /\ post_highest = SpecHighest(last_case)
    /\ post_pending = SpecPending(last_case)
    /\ post_quorum = SpecQuorum(last_case)
    /\ post_height_one = PreCommitAt(last_case, 1)
    /\ post_height_two = PreCommitAt(last_case, 2)

AccessorsEmitNoOutputs ==
  outputs = {}

AllTriedCasesRemainModeled ==
  tried \subseteq Cases

EngineReadAccessorsCoreSafety ==
  /\ StateSnapshotMatchesFields
  /\ CommittedAtMatchesQueriedHeight
  /\ AccessorsDoNotMutateState
  /\ AccessorsEmitNoOutputs
  /\ AllTriedCasesRemainModeled

EngineReadAccessorsExactness ==
  /\ StateSnapshotMatchesFields
  /\ CommittedAtMatchesQueriedHeight
  /\ AccessorsDoNotMutateState
  /\ AccessorsEmitNoOutputs
  /\ AllTriedCasesRemainModeled

EngineReadAccessorsCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineReadAccessorsExactness

Safety == EngineReadAccessorsExactness

====

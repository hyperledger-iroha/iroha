---- MODULE SumeragiRbcDeliverQuorum ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the Sumeragi RBC deliver-quorum gate.

The live implementation derives the RBC deliver threshold from the deduplicated
validator topology via `Topology::min_votes_for_commit()`, except for the
developer-only debug path that forces the threshold to one. READY messages are
counted by distinct sender, so duplicate READY observations cannot inflate the
deliver decision.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxPeers,
  \* @type: Int;
  MaxReadyEvents,
  \* @type: Bool;
  BugDuplicateReadyCount,
  \* @type: Bool;
  BugUnderQuorumDeliver,
  \* @type: Bool;
  BugWrongCommitFormula,
  \* @type: Bool;
  BugForceOneIgnored

VARIABLES
  \* @type: Int;
  uniquePeers,
  \* @type: Int;
  rawReadyEvents,
  \* @type: Int;
  distinctReady,
  \* @type: Bool;
  forceOne,
  \* @type: Int;
  required,
  \* @type: Int;
  readyCount,
  \* @type: Bool;
  delivered

vars == <<
  uniquePeers,
  rawReadyEvents,
  distinctReady,
  forceOne,
  required,
  readyCount,
  delivered
>>

RawReadyValues == 0..MaxReadyEvents

CommitQuorumSpec(n) ==
  IF n <= 3 THEN n ELSE (n * 2) \div 3 + 1

CommitQuorumPolicy(n) ==
  IF BugWrongCommitFormula
  THEN IF n <= 3 THEN n ELSE (n * 2) \div 3
  ELSE CommitQuorumSpec(n)

RequiredSpec(n, force) ==
  IF force THEN 1 ELSE CommitQuorumSpec(n)

RequiredPolicy(n, force) ==
  IF BugForceOneIgnored
  THEN CommitQuorumPolicy(n)
  ELSE IF force THEN 1 ELSE CommitQuorumPolicy(n)

ReadyCountPolicy(raw, distinct) ==
  IF BugDuplicateReadyCount THEN raw ELSE distinct

DeliverPolicy(raw, distinct, n, force) ==
  LET r == RequiredPolicy(n, force)
      c == ReadyCountPolicy(raw, distinct)
  IN IF BugUnderQuorumDeliver THEN c + 1 >= r ELSE c >= r

TypeInvariant ==
  /\ MaxPeers \in Nat
  /\ MaxPeers >= 4
  /\ MaxReadyEvents \in Nat
  /\ MaxReadyEvents >= MaxPeers
  /\ BugDuplicateReadyCount \in BOOLEAN
  /\ BugUnderQuorumDeliver \in BOOLEAN
  /\ BugWrongCommitFormula \in BOOLEAN
  /\ BugForceOneIgnored \in BOOLEAN
  /\ uniquePeers \in 1..MaxPeers
  /\ rawReadyEvents \in RawReadyValues
  /\ distinctReady \in 0..uniquePeers
  /\ rawReadyEvents >= distinctReady
  /\ forceOne \in BOOLEAN
  /\ required \in 1..MaxPeers
  /\ readyCount \in RawReadyValues
  /\ delivered \in BOOLEAN

Init ==
  /\ uniquePeers = 1
  /\ rawReadyEvents = 0
  /\ distinctReady = 0
  /\ forceOne = FALSE
  /\ required = 1
  /\ readyCount = 0
  /\ delivered = FALSE

Evaluate(n, raw, distinct, force) ==
  /\ raw >= distinct
  /\ uniquePeers' = n
  /\ rawReadyEvents' = raw
  /\ distinctReady' = distinct
  /\ forceOne' = force
  /\ required' = RequiredPolicy(n, force)
  /\ readyCount' = ReadyCountPolicy(raw, distinct)
  /\ delivered' = DeliverPolicy(raw, distinct, n, force)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E n \in 1..MaxPeers:
       \E raw \in RawReadyValues:
         \E distinct \in 0..n:
           \E force \in BOOLEAN:
             Evaluate(n, raw, distinct, force)
  \/ Stable

RequiredMatchesSpec ==
  required = RequiredSpec(uniquePeers, forceOne)

DefaultQuorumMatchesDedupedTopology ==
  ~forceOne => required = CommitQuorumSpec(uniquePeers)

ForceOneUsesSingleReadyThreshold ==
  forceOne => required = 1

ReadyCountUsesDistinctSenders ==
  readyCount = distinctReady

NoDeliverBeforeDistinctReadyQuorum ==
  distinctReady < required => ~delivered

DuplicateReadyDoesNotInflateDeliver ==
  /\ rawReadyEvents > distinctReady
  /\ distinctReady < required
  => ~delivered

DeliveredMatchesSpec ==
  delivered = (distinctReady >= RequiredSpec(uniquePeers, forceOne))

====

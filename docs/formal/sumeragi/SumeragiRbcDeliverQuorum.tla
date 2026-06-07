---- MODULE SumeragiRbcDeliverQuorum ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the Sumeragi RBC deliver-quorum gate.

The live implementation derives inbound RBC DELIVER acceptance from the
deduplicated validator topology via `Topology::min_votes_for_commit()`. The
developer-only debug path can force local DELIVER emission to threshold one, but
it must not lower receiver-side acceptance of external DELIVER frames. READY
messages are counted by distinct sender, so duplicate READY observations cannot
inflate the deliver decision.
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
  BugForceOneIgnored,
  \* @type: Bool;
  BugInboundForceOneAccepted

VARIABLES
  \* @type: Int;
  uniquePeers,
  \* @type: Int;
  rawReadyEvents,
  \* @type: Int;
  distinctReady,
  \* @type: Bool;
  forceOne,
  \* @type: Bool;
  localEmission,
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
  localEmission,
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

RequiredSpec(n, force, local) ==
  IF local /\ force THEN 1 ELSE CommitQuorumSpec(n)

RequiredPolicy(n, force, local) ==
  IF local
  THEN
    IF BugForceOneIgnored
    THEN CommitQuorumPolicy(n)
    ELSE IF force THEN 1 ELSE CommitQuorumPolicy(n)
  ELSE
    IF BugInboundForceOneAccepted /\ force
    THEN 1
    ELSE CommitQuorumPolicy(n)

ReadyCountPolicy(raw, distinct) ==
  IF BugDuplicateReadyCount THEN raw ELSE distinct

DeliverPolicy(raw, distinct, n, force, local) ==
  LET r == RequiredPolicy(n, force, local)
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
  /\ BugInboundForceOneAccepted \in BOOLEAN
  /\ uniquePeers \in 1..MaxPeers
  /\ rawReadyEvents \in RawReadyValues
  /\ distinctReady \in 0..uniquePeers
  /\ rawReadyEvents >= distinctReady
  /\ forceOne \in BOOLEAN
  /\ localEmission \in BOOLEAN
  /\ required \in 1..MaxPeers
  /\ readyCount \in RawReadyValues
  /\ delivered \in BOOLEAN

Init ==
  /\ uniquePeers = 1
  /\ rawReadyEvents = 0
  /\ distinctReady = 0
  /\ forceOne = FALSE
  /\ localEmission = FALSE
  /\ required = 1
  /\ readyCount = 0
  /\ delivered = FALSE

Evaluate(n, raw, distinct, force, local) ==
  /\ raw >= distinct
  /\ uniquePeers' = n
  /\ rawReadyEvents' = raw
  /\ distinctReady' = distinct
  /\ forceOne' = force
  /\ localEmission' = local
  /\ required' = RequiredPolicy(n, force, local)
  /\ readyCount' = ReadyCountPolicy(raw, distinct)
  /\ delivered' = DeliverPolicy(raw, distinct, n, force, local)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E n \in 1..MaxPeers:
       \E raw \in RawReadyValues:
           \E distinct \in 0..n:
             \E force \in BOOLEAN:
               \E local \in BOOLEAN:
                 Evaluate(n, raw, distinct, force, local)
  \/ Stable

RequiredMatchesSpec ==
  required = RequiredSpec(uniquePeers, forceOne, localEmission)

DefaultQuorumMatchesDedupedTopology ==
  ~(forceOne /\ localEmission) => required = CommitQuorumSpec(uniquePeers)

ForceOneLocalEmissionUsesSingleReadyThreshold ==
  forceOne /\ localEmission => required = 1

ForceOneInboundAcceptanceUsesProtocolThreshold ==
  forceOne /\ ~localEmission => required = CommitQuorumSpec(uniquePeers)

ReadyCountUsesDistinctSenders ==
  readyCount = distinctReady

NoDeliverBeforeDistinctReadyQuorum ==
  distinctReady < required => ~delivered

DuplicateReadyDoesNotInflateDeliver ==
  /\ rawReadyEvents > distinctReady
  /\ distinctReady < required
  => ~delivered

DeliveredMatchesSpec ==
  delivered = (distinctReady >= RequiredSpec(uniquePeers, forceOne, localEmission))

====

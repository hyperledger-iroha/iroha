---- MODULE SumeragiV2ChainEpoch ----
EXTENDS SumeragiV2Reconfiguration

(***************************************************************************
Per-validator application histories and lagging epoch transitions.

The core agreement theorem gives a single write-once decided subject at each
canonical height.  This module models the next layer exactly: each validator
stores its own applied height and therefore its own history/context, while the
certified chain may advance after the responsive dual quorum has applied.
Honest validators outside `Responsive` can remain at an old height and epoch.
No transition rewrites their state or installs a context for them globally.
***************************************************************************)

VARIABLES
  certifiedHeight,
  decidedAt,
  nodeHeight,
  nodeContext

DecisionSlots == 1..MaxHeight
DecisionMapSet == [DecisionSlots -> SubjectOrNone]

HistoryThrough(blockHeight) ==
  [index \in 1..blockHeight |-> decidedAt[index]]

AllLineages ==
  UNION {LineagesAt(blockHeight): blockHeight \in Heights}

ChainEpochVars == <<certifiedHeight, decidedAt, nodeHeight, nodeContext>>
ChainEpochAllVars == <<vars, certifiedHeight, decidedAt,
                       nodeHeight, nodeContext>>

ChainEpochTypeInvariant ==
  /\ certifiedHeight \in Heights
  /\ decidedAt \in DecisionMapSet
  /\ nodeHeight \in [ValidatorIds -> Heights]
  /\ nodeContext \in [ValidatorIds -> ContextRecords]

CertifiedPrefixValid ==
  \A index \in 1..certifiedHeight: decidedAt[index] \in ValidSubjects

NodesDoNotOutrunCertificates ==
  \A node \in ValidatorIds: nodeHeight[node] <= certifiedHeight

ContextsMatchLocalHistories ==
  \A node \in ValidatorIds:
    nodeContext[node]
      = ContextRecord(nodeHeight[node], HistoryThrough(nodeHeight[node]))

HistoryPrefixComparable ==
  \A left, right \in ValidatorIds:
    \A index \in 1..nodeHeight[left]:
      index <= nodeHeight[right]
        => HistoryThrough(nodeHeight[left])[index]
             = HistoryThrough(nodeHeight[right])[index]

PerNodeFrozenEpoch ==
  \A node \in ValidatorIds:
    /\ nodeContext[node].height = nodeHeight[node]
    /\ nodeContext[node].epoch = ExpectedEpoch(nodeHeight[node])
    /\ nodeContext[node].roster
         = RosterSequence(ExpectedEpoch(nodeHeight[node]))
    /\ nodeContext[node].powers
         = EpochPowers[ExpectedEpoch(nodeHeight[node]) + 1]

PerNodeParentFinality ==
  \A node \in ValidatorIds:
    nodeHeight[node] > 0
      => /\ nodeContext[node].parent
               = decidedAt[nodeHeight[node]]
         /\ nodeContext[node].parentContextKey
               = ContextKey(nodeHeight[node] - 1,
                            HistoryThrough(nodeHeight[node] - 1))
         /\ nodeContext[node].parentFinality
               = [contextKey |-> nodeContext[node].parentContextKey,
                  height |-> nodeHeight[node] - 1,
                  phase |-> "Commit",
                  subject |-> decidedAt[nodeHeight[node]]]

CanApplyCertifiedLineage(node, lineage) ==
  /\ lineage = HistoryThrough(nodeHeight[node])
  /\ nodeHeight[node] < certifiedHeight

ForeignLineageRejected ==
  \A node \in ValidatorIds, lineage \in AllLineages:
    lineage # HistoryThrough(nodeHeight[node])
      => ~CanApplyCertifiedLineage(node, lineage)

ChainEpochInvariant ==
  /\ ChainEpochTypeInvariant
  /\ CertifiedPrefixValid
  /\ NodesDoNotOutrunCertificates
  /\ ContextsMatchLocalHistories
  /\ HistoryPrefixComparable
  /\ PerNodeFrozenEpoch
  /\ PerNodeParentFinality
  /\ ForeignLineageRejected

ChainEpochInit ==
  /\ Init
  /\ certifiedHeight = 0
  /\ decidedAt = [index \in DecisionSlots |-> NoSubject]
  /\ nodeHeight = [node \in ValidatorIds |-> 0]
  /\ nodeContext =
       [node \in ValidatorIds |-> ContextRecord(0, <<>>)]

ResponsiveAppliedCertifiedPrefix ==
  \A node \in Responsive: nodeHeight[node] = certifiedHeight

CertifyNextSubject(subject) ==
  LET nextHeight == certifiedHeight + 1
  IN /\ certifiedHeight < MaxHeight
     /\ ResponsiveAppliedCertifiedPrefix
     /\ subject \in ValidSubjects
     /\ decidedAt[nextHeight] = NoSubject
     /\ decidedAt' = [decidedAt EXCEPT ![nextHeight] = subject]
     /\ certifiedHeight' = nextHeight
     /\ UNCHANGED <<nodeHeight, nodeContext>>
     /\ UNCHANGED vars

ApplyCertifiedNext(node) ==
  LET nextHeight == nodeHeight[node] + 1
      nextLineage == HistoryThrough(nextHeight)
  IN /\ node \in Honest
     /\ nodeHeight[node] < certifiedHeight
     /\ decidedAt[nextHeight] \in ValidSubjects
     /\ nodeHeight' = [nodeHeight EXCEPT ![node] = nextHeight]
     /\ nodeContext' =
          [nodeContext EXCEPT ![node] = ContextRecord(nextHeight, nextLineage)]
     /\ UNCHANGED <<certifiedHeight, decidedAt>>
     /\ UNCHANGED vars

ChainEpochNext ==
  \/ \E subject \in ValidSubjects: CertifyNextSubject(subject)
  \/ \E node \in Honest: ApplyCertifiedNext(node)

ChainEpochSpec ==
  ChainEpochInit /\ [][ChainEpochNext]_ChainEpochAllVars

=============================================================================

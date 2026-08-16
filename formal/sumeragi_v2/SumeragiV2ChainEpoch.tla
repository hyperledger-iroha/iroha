---- MODULE SumeragiV2ChainEpoch ----
EXTENDS SumeragiV2Core

(***************************************************************************
Per-validator finalized histories and lagging epoch transitions.

This is a receipt-driven abstraction of the layer above one-height consensus.
A canonical slot is certified only from a durable decision carrying a valid
CommitQC for the preceding consensus height.  Certification does not wait for
all responsive validators to apply.  Each honest validator advances its own
history only after its independent durable application receipt for the exact
certified finality identity.  A lagging validator is never advanced by another
validator's receipt.

The durable evidence variables are abstract receipt logs.  The asynchronous
refinement maps them to observations that can be appended only after the
corresponding production `decisions` or `applied` entry exists.
***************************************************************************)

VARIABLES
  certifiedHeight,
  decidedAt,
  nodeHeight,
  nodeContext,
  durableDecisionEvidence,
  durableApplicationEvidence

DecisionSlots == 1..MaxHeight
DecisionMapSet == [DecisionSlots -> SubjectOrNone]
DecisionEvidenceSet == [node: ValidatorIds, qc: QcRecordSet]

HistoryThrough(blockHeight) ==
  [index \in 1..blockHeight |-> decidedAt[index]]

AllLineages ==
  UNION {LineagesAt(blockHeight): blockHeight \in Heights}

ChainEpochVars ==
  <<certifiedHeight, decidedAt, nodeHeight, nodeContext,
    durableDecisionEvidence, durableApplicationEvidence>>

HistoricalCommitCertificate(qc) ==
  /\ qc \in QcRecordSet
  /\ qc.context \in ContextRecords
  /\ qc.height = qc.context.height
  /\ qc.context.epoch \in Epochs
  /\ qc.phase = "Commit"
  /\ qc.subject \in ValidSubjects
  /\ ExactCertificateQuorum(qc.context.epoch, qc.signers)

DurableCommitDecision(decision) ==
  /\ decision \in DecisionEvidenceSet
  /\ HistoricalCommitCertificate(decision.qc)

\* This construction is extensionally equal to the DurableCommitDecision
\* guard shared by all four receipt actions.  Building the well-formed Commit
\* certificates directly avoids making TLC traverse every malformed product
\* member of QcRecordSet at each simulated transition.
CandidateHistoricalCommitCertificateSet ==
  {QC(qcContext, roundView, "Commit", subject, signers):
    qcContext \in ContextRecords,
    roundView \in Views,
    subject \in ValidSubjects,
    signers \in SUBSET ValidatorIds}

HistoricalCommitCertificateSet ==
  {qc \in CandidateHistoricalCommitCertificateSet:
    ExactCertificateQuorum(qc.context.epoch, qc.signers)}

CandidateDurableDecisionEvidenceSet ==
  {[node |-> node, qc |-> qc]:
    node \in ValidatorIds, qc \in HistoricalCommitCertificateSet}

DurableDecisionEvidenceSet ==
  {decision \in CandidateDurableDecisionEvidenceSet:
    decision \in DecisionEvidenceSet}

CommitFinalityIdentity(qc) ==
  [contextKey |-> qc.context.contextKey,
   height |-> qc.height,
   phase |-> qc.phase,
   subject |-> qc.subject]

CanonicalCommitForSlot(qc, index) ==
  /\ index \in DecisionSlots
  /\ qc.context
       = ContextRecord(index - 1, HistoryThrough(index - 1))
  /\ qc.height = index - 1
  /\ qc.phase = "Commit"
  /\ qc.subject = decidedAt[index]

DecisionBacksCertifiedSlot(decision) ==
  \E index \in 1..certifiedHeight:
    CanonicalCommitForSlot(decision.qc, index)

ReceiptOutsideChainHorizon(receipt) ==
  receipt.qc.context.height + 1 > MaxHeight

ApplicationHasRecordedDecision(application) ==
  application \in durableDecisionEvidence

ChainEpochTypeInvariant ==
  /\ certifiedHeight \in Heights
  /\ decidedAt \in DecisionMapSet
  /\ nodeHeight \in [ValidatorIds -> Heights]
  /\ nodeContext \in [ValidatorIds -> ContextRecords]
  /\ durableDecisionEvidence \subseteq DecisionEvidenceSet
  /\ durableApplicationEvidence \subseteq DecisionEvidenceSet

DurableDecisionEvidenceSound ==
  \A decision \in durableDecisionEvidence:
    /\ DurableCommitDecision(decision)
    /\ \/ DecisionBacksCertifiedSlot(decision)
       \/ ReceiptOutsideChainHorizon(decision)

DurableApplicationEvidenceSound ==
  \A application \in durableApplicationEvidence:
    /\ DurableCommitDecision(application)
    /\ ApplicationHasRecordedDecision(application)
    /\ \/ DecisionBacksCertifiedSlot(application)
       \/ ReceiptOutsideChainHorizon(application)

CertifiedPrefixBacked ==
  \A index \in 1..certifiedHeight:
    /\ decidedAt[index] \in ValidSubjects
    /\ \E decision \in durableDecisionEvidence:
         CanonicalCommitForSlot(decision.qc, index)

(***************************************************************************
An applied height is not merely a counter.  Every slot in a validator's local
prefix is backed by that validator's own durable application receipt carrying
the canonical CommitQC for the slot.  This is the receipt-level bridge from
prefix length to actual finality evidence.
***************************************************************************)
NodeAppliedPrefixBacked ==
  \A node \in ValidatorIds:
    \A index \in 1..nodeHeight[node]:
      \E application \in durableApplicationEvidence:
        /\ application.node = node
        /\ CanonicalCommitForSlot(application.qc, index)

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
               = ParentContextKey(nodeHeight[node],
                                  HistoryThrough(nodeHeight[node]))
         /\ nodeContext[node].parentFinality
               = ParentFinalityIdentity(nodeHeight[node],
                                        HistoryThrough(nodeHeight[node]))

CanApplyCertifiedLineage(node, lineage) ==
  /\ lineage = HistoryThrough(nodeHeight[node])
  /\ nodeHeight[node] < certifiedHeight

ForeignLineageRejected ==
  \A node \in ValidatorIds, lineage \in AllLineages:
    lineage # HistoryThrough(nodeHeight[node])
      => ~CanApplyCertifiedLineage(node, lineage)

(***************************************************************************
Certificate admission is local-context exact.  A lagging validator continues
to authenticate and serve historical evidence, but only a certificate for its
own active HeightContext may enter that validator's active reducer.
***************************************************************************)
CanAdmitNodeCertificate(node, qc) ==
  /\ node \in ValidatorIds
  /\ HistoricalCommitCertificate(qc)
  /\ qc.context = nodeContext[node]
  /\ qc.height = nodeHeight[node]

ForeignContextCertificateRejected ==
  \A node \in ValidatorIds, qc \in QcRecordSet:
    qc.context # nodeContext[node]
      => ~CanAdmitNodeCertificate(node, qc)

(***************************************************************************
The small inductive kernel.  The remaining public chain/epoch properties are
derived from this kernel in SumeragiV2ChainEpochProofs.
***************************************************************************)
ChainEpochInvariant ==
  /\ ModelConfiguration
  /\ ChainEpochTypeInvariant
  /\ DurableDecisionEvidenceSound
  /\ DurableApplicationEvidenceSound
  /\ CertifiedPrefixBacked
  /\ NodeAppliedPrefixBacked
  /\ NodesDoNotOutrunCertificates
  /\ ContextsMatchLocalHistories

ChainEpochSafety ==
  /\ ChainEpochInvariant
  /\ HistoryPrefixComparable
  /\ NodeAppliedPrefixBacked
  /\ PerNodeFrozenEpoch
  /\ PerNodeParentFinality
  /\ ForeignLineageRejected
  /\ ForeignContextCertificateRejected

ChainEpochInit ==
  /\ ModelConfiguration
  /\ certifiedHeight = 0
  /\ decidedAt = [index \in DecisionSlots |-> NoSubject]
  /\ nodeHeight = [node \in ValidatorIds |-> 0]
  /\ nodeContext =
       [node \in ValidatorIds |-> ContextRecord(0, <<>>)]
  /\ durableDecisionEvidence = {}
  /\ durableApplicationEvidence = {}

(***************************************************************************
A single durable CommitQC decision is sufficient to certify its canonical
slot.  There is deliberately no all-node or responsive-quorum apply barrier.
***************************************************************************)
RecordCertifiedNext(decision) ==
  LET nextHeight == certifiedHeight + 1
  IN /\ DurableCommitDecision(decision)
     /\ certifiedHeight < MaxHeight
     /\ decision.qc.context
          = ContextRecord(certifiedHeight,
                          HistoryThrough(certifiedHeight))
     /\ decidedAt[nextHeight] = NoSubject
     /\ certifiedHeight' = nextHeight
     /\ decidedAt' =
          [decidedAt EXCEPT ![nextHeight] = decision.qc.subject]
     /\ durableDecisionEvidence' =
          durableDecisionEvidence \cup {decision}
     /\ UNCHANGED <<nodeHeight, nodeContext,
                    durableApplicationEvidence>>

(***************************************************************************
Further durable decisions for an already certified canonical slot are retained
without certifying that slot twice.  A receipt at the bounded model's terminal
context is retained as total evidence but cannot name a representable successor
slot.  This action is what lets the concrete refinement mirror every durable
decision synchronously rather than selecting a convenient subset.
***************************************************************************)
RecordKnownDecision(decision) ==
  /\ DurableCommitDecision(decision)
  /\ decision \notin durableDecisionEvidence
  /\ \/ DecisionBacksCertifiedSlot(decision)
     \/ ReceiptOutsideChainHorizon(decision)
  /\ durableDecisionEvidence' =
       durableDecisionEvidence \cup {decision}
  /\ UNCHANGED <<certifiedHeight, decidedAt, nodeHeight, nodeContext,
                 durableApplicationEvidence>>

(***************************************************************************
Application is local.  The receipt must be for the node being advanced and
must itself be the exact recorded durable decision artifact for the canonical
next slot.  Consequently its context, subject, view, and signer set are the
same finality evidence that the concrete node durably applied.
***************************************************************************)
RecordAppliedNext(application) ==
  LET node == application.node
      nextHeight == nodeHeight[node] + 1
      nextLineage == HistoryThrough(nextHeight)
  IN /\ application \in DecisionEvidenceSet
     /\ node \in Honest
     /\ DurableCommitDecision(application)
     /\ nodeHeight[node] < certifiedHeight
     /\ CanonicalCommitForSlot(application.qc, nextHeight)
     /\ ApplicationHasRecordedDecision(application)
     /\ durableApplicationEvidence' =
          durableApplicationEvidence \cup {application}
     /\ nodeHeight' = [nodeHeight EXCEPT ![node] = nextHeight]
     /\ nodeContext' =
          [nodeContext EXCEPT
             ![node] = ContextRecord(nextHeight, nextLineage)]
     /\ UNCHANGED <<certifiedHeight, decidedAt,
                    durableDecisionEvidence>>

(***************************************************************************
Duplicate, Byzantine, and terminal-horizon application receipts remain in the
total evidence projection without extending an honest validator's contiguous
applied prefix.  An honest receipt for its exact next canonical slot cannot use
this branch and must take RecordAppliedNext.
***************************************************************************)
RecordKnownApplication(application) ==
  LET node == application.node
      slot == application.qc.context.height + 1
  IN /\ DurableCommitDecision(application)
     /\ ApplicationHasRecordedDecision(application)
     /\ application \notin durableApplicationEvidence
     /\ \/ DecisionBacksCertifiedSlot(application)
        \/ ReceiptOutsideChainHorizon(application)
     /\ \/ ReceiptOutsideChainHorizon(application)
        \/ node \notin Honest
        \/ slot <= nodeHeight[node]
     /\ durableApplicationEvidence' =
          durableApplicationEvidence \cup {application}
     /\ UNCHANGED <<certifiedHeight, decidedAt, nodeHeight, nodeContext,
                    durableDecisionEvidence>>

ChainEpochNext ==
  \/ \E decision \in DecisionEvidenceSet:
       RecordCertifiedNext(decision)
  \/ \E decision \in DecisionEvidenceSet:
       RecordKnownDecision(decision)
  \/ \E application \in DecisionEvidenceSet:
       RecordAppliedNext(application)
  \/ \E application \in DecisionEvidenceSet:
       RecordKnownApplication(application)

ChainEpochSpec ==
  ChainEpochInit /\ [][ChainEpochNext]_ChainEpochVars

(***************************************************************************
TLC checks every VARIABLE visible through EXTENDS, even when the deductive
ChainEpochSpec deliberately projects only ChainEpochVars.  This bounded-check
harness therefore initializes the inherited Core state and freezes it while
the receipt-driven chain projection advances.  Keeping the harness separate
preserves the exact deductive specification proved by ChainEpochProofs while
preventing a simulation from silently starting with unspecified Core state.
***************************************************************************)
ChainEpochTlcVars == <<vars, ChainEpochVars>>

ChainEpochTlcInit == Init /\ ChainEpochInit

ChainEpochTlcReceiptNext ==
  \/ \E decision \in DurableDecisionEvidenceSet:
       RecordCertifiedNext(decision)
  \/ \E decision \in DurableDecisionEvidenceSet:
       RecordKnownDecision(decision)
  \/ \E application \in DurableDecisionEvidenceSet:
       RecordAppliedNext(application)
  \/ \E application \in DurableDecisionEvidenceSet:
       RecordKnownApplication(application)

ChainEpochTlcNext == ChainEpochTlcReceiptNext /\ UNCHANGED vars

ChainEpochTlcSpec ==
  ChainEpochTlcInit /\ [][ChainEpochTlcNext]_ChainEpochTlcVars

ChainEpochTlcInvariant == TypeInvariant /\ ChainEpochInvariant

=============================================================================

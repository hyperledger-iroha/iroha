---- MODULE SumeragiV2CommitImportProvenanceMutation ----
EXTENDS Naturals, Sequences, TLC

(***************************************************************************
Bounded regressions for exact historical Commit-import provenance.

Production constructs a causal root from the authenticated CommitQC or
CommitCertificateResponse occurrence, retains that root on BeginDecision and
PersistDecision successors, and checks the exact projection immediately
before FIFO or deferred execution.  These two mutation pairs isolate both
boundaries:

  * the repaired executor classifies a forged import as rejected, while the
    mutation executes it; and
  * the repaired causal constructor retains the parent's root, while the
    mutation replaces it on the BeginDecision child.

The model does not treat a changed causal root as a second admissible request.
It checks the production fail-closed boundary directly.
***************************************************************************)

CommitQc ==
  [context |-> "context-7", phase |-> "Commit", view |-> 3,
   subject |-> "body-11"]

DirectEvidence ==
  [kind |-> "CommitQC", qc |-> CommitQc,
   requestAuthorized |-> FALSE]

ResponseEvidence ==
  [kind |-> "CommitCertificateResponse", qc |-> CommitQc,
   requestAuthorized |-> TRUE]

CommitImportEvidenceSet == {DirectEvidence, ResponseEvidence}

CanonicalCommitImportOrigin(evidence) ==
  [sourceKind |-> evidence.kind,
   context |-> evidence.qc.context,
   view |-> evidence.qc.view,
   subject |-> evidence.qc.subject]

ForeignCommitImportOrigin ==
  [sourceKind |-> "Forged",
   context |-> CommitQc.context,
   view |-> CommitQc.view,
   subject |-> CommitQc.subject]

CommitImportOriginSet ==
  {CanonicalCommitImportOrigin(DirectEvidence),
   CanonicalCommitImportOrigin(ResponseEvidence),
   ForeignCommitImportOrigin}

CommitImportCandidate(kind, evidence, causalOrigin) ==
  [kind |-> kind, evidence |-> evidence,
   context |-> evidence.qc.context,
   view |-> evidence.qc.view,
   subject |-> evidence.qc.subject,
   causalOrigin |-> causalOrigin]

CommitImportKindSet == {"DeliverQC", "BeginDecision", "PersistDecision"}

CommitImportCandidateSet ==
  {CommitImportCandidate(kind, evidence, causalOrigin):
     kind \in CommitImportKindSet,
     evidence \in CommitImportEvidenceSet,
     causalOrigin \in CommitImportOriginSet}

SequenceSet(sequence) ==
  {sequence[index]: index \in 1..Len(sequence)}

CommitImportExecutionNeedsLineage(candidate) ==
  candidate.kind \in CommitImportKindSet

CommitImportCandidateLineage(candidate) ==
  /\ candidate \in CommitImportCandidateSet
  /\ candidate.evidence.qc.phase = "Commit"
  /\ candidate.context = candidate.evidence.qc.context
  /\ candidate.view = candidate.evidence.qc.view
  /\ candidate.subject = candidate.evidence.qc.subject
  /\ candidate.causalOrigin =
       CanonicalCommitImportOrigin(candidate.evidence)
  /\ (candidate.evidence.kind = "CommitCertificateResponse"
        => candidate.evidence.requestAuthorized)

CommitImportExecutionProvenance(candidate) ==
  IF CommitImportExecutionNeedsLineage(candidate)
  THEN CommitImportCandidateLineage(candidate)
  ELSE TRUE

CanonicalDirectBegin ==
  CommitImportCandidate(
    "BeginDecision", DirectEvidence,
    CanonicalCommitImportOrigin(DirectEvidence))

ForgedDirectBegin ==
  CommitImportCandidate(
    "BeginDecision", DirectEvidence, ForeignCommitImportOrigin)

CanonicalResponsePersist ==
  CommitImportCandidate(
    "PersistDecision", ResponseEvidence,
    CanonicalCommitImportOrigin(ResponseEvidence))

ForgedResponsePersist ==
  CommitImportCandidate(
    "PersistDecision", ResponseEvidence, ForeignCommitImportOrigin)

ExecutionInventory ==
  <<CanonicalDirectBegin, ForgedDirectBegin,
    CanonicalResponsePersist, ForgedResponsePersist>>

CanonicalDirectDelivery ==
  CommitImportCandidate(
    "DeliverQC", DirectEvidence,
    CanonicalCommitImportOrigin(DirectEvidence))

CanonicalResponseDelivery ==
  CommitImportCandidate(
    "DeliverQC", ResponseEvidence,
    CanonicalCommitImportOrigin(ResponseEvidence))

SuccessorInventory ==
  <<CanonicalDirectDelivery, CanonicalResponseDelivery>>

CommitImportSuccessor(parent, causalOrigin) ==
  CommitImportCandidate("BeginDecision", parent.evidence, causalOrigin)

VARIABLES pending, executed, rejected, lastTransition

MutationVars == <<pending, executed, rejected, lastTransition>>

MutationTransitionNames ==
  {"Initial", "FixedExecute", "FixedReject", "BugExecute",
   "FixedSuccessor", "BugSuccessor"}

MutationTypeInvariant ==
  /\ pending \in Seq(CommitImportCandidateSet)
  /\ executed \in Seq(CommitImportCandidateSet)
  /\ rejected \in Seq(CommitImportCandidateSet)
  /\ lastTransition \in MutationTransitionNames

ExecutionInit ==
  /\ pending = ExecutionInventory
  /\ executed = <<>>
  /\ rejected = <<>>
  /\ lastTransition = "Initial"

FixedExecutionStep ==
  LET candidate == Head(pending)
      admissible == CommitImportExecutionProvenance(candidate)
  IN /\ pending # <<>>
     /\ pending' = Tail(pending)
     /\ executed' =
          IF admissible THEN Append(executed, candidate) ELSE executed
     /\ rejected' =
          IF admissible THEN rejected ELSE Append(rejected, candidate)
     /\ lastTransition' =
          IF admissible THEN "FixedExecute" ELSE "FixedReject"

BugExecutionStep ==
  LET candidate == Head(pending)
  IN /\ pending # <<>>
     /\ pending' = Tail(pending)
     /\ executed' = Append(executed, candidate)
     /\ rejected' = rejected
     /\ lastTransition' = "BugExecute"

FixedExecutionSpec ==
  /\ ExecutionInit
  /\ [][FixedExecutionStep]_MutationVars
  /\ WF_MutationVars(FixedExecutionStep)

BugExecutionSpec ==
  /\ ExecutionInit
  /\ [][BugExecutionStep]_MutationVars
  /\ WF_MutationVars(BugExecutionStep)

ExecutedImportsHaveExactLineage ==
  \A candidate \in SequenceSet(executed):
    CommitImportExecutionProvenance(candidate)

RejectedImportsHaveNoExactLineage ==
  \A candidate \in SequenceSet(rejected):
    ~CommitImportExecutionProvenance(candidate)

EventuallyEveryImportIsClassified == <>(pending = <<>>)

SuccessorInit ==
  /\ pending = SuccessorInventory
  /\ executed = <<>>
  /\ rejected = <<>>
  /\ lastTransition = "Initial"

FixedSuccessorStep ==
  LET parent == Head(pending)
      successor == CommitImportSuccessor(parent, parent.causalOrigin)
  IN /\ pending # <<>>
     /\ pending' = Tail(pending)
     /\ executed' = Append(executed, successor)
     /\ rejected' = rejected
     /\ lastTransition' = "FixedSuccessor"

BugSuccessorStep ==
  LET parent == Head(pending)
      successor ==
        CommitImportSuccessor(parent, ForeignCommitImportOrigin)
  IN /\ pending # <<>>
     /\ pending' = Tail(pending)
     /\ executed' = Append(executed, successor)
     /\ rejected' = rejected
     /\ lastTransition' = "BugSuccessor"

FixedSuccessorSpec ==
  /\ SuccessorInit
  /\ [][FixedSuccessorStep]_MutationVars
  /\ WF_MutationVars(FixedSuccessorStep)

BugSuccessorSpec ==
  /\ SuccessorInit
  /\ [][BugSuccessorStep]_MutationVars
  /\ WF_MutationVars(BugSuccessorStep)

ProducedSuccessorsRetainExactLineage ==
  \A candidate \in SequenceSet(executed):
    CommitImportCandidateLineage(candidate)

EventuallyEveryParentProducesSuccessor == <>(pending = <<>>)

=============================================================================

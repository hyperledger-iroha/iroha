---- MODULE SumeragiV2CertificateRefRecoveryMutation ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
Bounded mutation witness for historical BeginLockCommit recovery ownership.

Two independently valid Prepare QCs have one full production CertificateRef:
context, height, view, phase, and subject are identical, while their signer
sets differ.  The scheduled BeginLockCommit candidate carries PrepareQcA.
Execution is allowed to select coordinate-equivalent PrepareQcB and persists
that exact QC in the LockCommit request.

The fixed witness owns recovery by the full CertificateRef.  The mutant uses
whole-QC equality and therefore loses the source-A witness at the A-to-B
handoff.  StableFieldVariants are a compact negative control: changing any
stable reference field must not alias the source QC.

This small model isolates the identity quotient needed by the open-kernel
preservation proof.  It is mutation evidence, not a proof of the full
asynchronous specification.
***************************************************************************)

CONSTANT Mode

Modes == {"FullCertificateRef", "ExactQcEquality"}

ASSUME Mode \in Modes

Node == "validator-0"
RecoveryContext ==
  [chain |-> "sora",
   height |-> 7,
   epoch |-> 2,
   lineage |-> "parent-a"]
OtherContext ==
  [chain |-> "sora",
   height |-> 7,
   epoch |-> 2,
   lineage |-> "parent-b"]
LockedView == 3
LockedSubject == "locked-block-7"
OtherSubject == "other-block-7"
Validators ==
  {"validator-0", "validator-1", "validator-2", "validator-3"}
PrepareSignersA ==
  {"validator-0", "validator-1", "validator-2"}
PrepareSignersB ==
  {"validator-0", "validator-1", "validator-3"}

QC(qcContext, qcHeight, qcView, qcPhase, qcSubject, qcSigners) ==
  [context |-> qcContext,
   height |-> qcHeight,
   view |-> qcView,
   phase |-> qcPhase,
   subject |-> qcSubject,
   signers |-> qcSigners]

PrepareQcA ==
  QC(RecoveryContext, RecoveryContext.height, LockedView, "Prepare",
     LockedSubject, PrepareSignersA)

PrepareQcB ==
  QC(RecoveryContext, RecoveryContext.height, LockedView, "Prepare",
     LockedSubject, PrepareSignersB)

ValidPrepareQc(qc) ==
  /\ qc.context = RecoveryContext
  /\ qc.height = qc.context.height
  /\ qc.view = LockedView
  /\ qc.phase = "Prepare"
  /\ qc.subject = LockedSubject
  /\ qc.signers \subseteq Validators
  /\ Cardinality(qc.signers) = 3

(***************************************************************************
Match the stable identity in production `CertificateRef`.  `height` remains
explicit even though the bounded context record also contains it, because
production stores the context id and Round height as separate fields.
***************************************************************************)
SameCertificateRef(left, right) ==
  /\ left.context = right.context
  /\ left.height = right.height
  /\ left.view = right.view
  /\ left.phase = right.phase
  /\ left.subject = right.subject

StableFieldVariants ==
  {QC(OtherContext, RecoveryContext.height, LockedView, "Prepare",
      LockedSubject, PrepareSignersA),
   QC(RecoveryContext, RecoveryContext.height + 1, LockedView, "Prepare",
      LockedSubject, PrepareSignersA),
   QC(RecoveryContext, RecoveryContext.height, LockedView + 1, "Prepare",
      LockedSubject, PrepareSignersA),
   QC(RecoveryContext, RecoveryContext.height, LockedView, "Commit",
      LockedSubject, PrepareSignersA),
   QC(RecoveryContext, RecoveryContext.height, LockedView, "Prepare",
      OtherSubject, PrepareSignersA)}

BeginLockCandidateA ==
  [class |-> "Completion",
   kind |-> "BeginLockCommit",
   node |-> Node,
   height |-> PrepareQcA.height,
   view |-> PrepareQcA.view,
   subject |-> PrepareQcA.subject,
   evidence |-> PrepareQcA]

LockCommitRequest(qc) ==
  [node |-> Node,
   kind |-> "LockCommit",
   qc |-> qc]

VARIABLES stage,
          candidateScheduled,
          pendingLockCommit,
          executedFrom,
          pendingOwner

vars ==
  <<stage, candidateScheduled, pendingLockCommit, executedFrom, pendingOwner>>

Init ==
  /\ stage = "BeginLockCandidateA"
  /\ candidateScheduled = TRUE
  /\ pendingLockCommit = {}
  /\ executedFrom = "None"
  /\ pendingOwner = "None"

BeginLockUsingEquivalentQcB ==
  /\ stage = "BeginLockCandidateA"
  /\ candidateScheduled
  /\ BeginLockCandidateA.evidence = PrepareQcA
  /\ PrepareQcA # PrepareQcB
  /\ SameCertificateRef(PrepareQcA, PrepareQcB)
  /\ stage' = "PendingLockCommitB"
  /\ candidateScheduled' = FALSE
  /\ pendingLockCommit' = {LockCommitRequest(PrepareQcB)}
  /\ executedFrom' = "PrepareQcA"
  /\ pendingOwner' = "PrepareQcB"

Quiescent ==
  /\ stage = "PendingLockCommitB"
  /\ UNCHANGED vars

Next ==
  \/ BeginLockUsingEquivalentQcB
  \/ Quiescent

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeInvariant ==
  /\ stage \in {"BeginLockCandidateA", "PendingLockCommitB"}
  /\ candidateScheduled \in BOOLEAN
  /\ pendingLockCommit \subseteq
       {LockCommitRequest(PrepareQcA), LockCommitRequest(PrepareQcB)}
  /\ executedFrom \in {"None", "PrepareQcA"}
  /\ pendingOwner \in {"None", "PrepareQcB"}

DistinctValidPrepareQcFixture ==
  /\ ValidPrepareQc(PrepareQcA)
  /\ ValidPrepareQc(PrepareQcB)
  /\ PrepareQcA # PrepareQcB
  /\ PrepareQcA.signers # PrepareQcB.signers
  /\ SameCertificateRef(PrepareQcA, PrepareQcB)

StableReferenceFieldsCannotAlias ==
  \A variant \in StableFieldVariants:
    ~SameCertificateRef(PrepareQcA, variant)

ExactHandoffShape ==
  /\ (stage = "BeginLockCandidateA"
        => /\ candidateScheduled
           /\ pendingLockCommit = {}
           /\ executedFrom = "None"
           /\ pendingOwner = "None")
  /\ (stage = "PendingLockCommitB"
        => /\ ~candidateScheduled
           /\ pendingLockCommit = {LockCommitRequest(PrepareQcB)}
           /\ executedFrom = "PrepareQcA"
           /\ pendingOwner = "PrepareQcB")

PendingRecoveryReferenceMatches(request, qc) ==
  IF Mode = "FullCertificateRef"
  THEN SameCertificateRef(request.qc, qc)
  ELSE request.qc = qc

HistoricalLockedCommitRecoveryWitness(qc) ==
  \/ /\ candidateScheduled
     /\ BeginLockCandidateA.node = Node
     /\ BeginLockCandidateA.kind = "BeginLockCommit"
     /\ SameCertificateRef(BeginLockCandidateA.evidence, qc)
  \/ \E request \in pendingLockCommit:
       /\ request.node = Node
       /\ PendingRecoveryReferenceMatches(request, qc)

HistoricalLockedCommitRecoveryProgress ==
  HistoricalLockedCommitRecoveryWitness(PrepareQcA)

=============================================================================

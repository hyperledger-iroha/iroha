------------- MODULE SumeragiV2Revision4CertifiedFenceReservation -------------
\* Bounded authoritative revision-4 scheduler kernel for the certificate which
\* retires an exact retained Serve or leader-wire fence. The model explores
\* certificate-last and certificate-first arrival orders, two ordinary
\* Progress owners, the complete Completion reserve, and several distinct
\* certificate roots sharing one retained physical credit.

EXTENDS FiniteSets, Naturals, Sequences, TLC

CONSTANTS CertifiedFenceEscapeEnabled, RetainedCertifiedCreditEnabled

BarrierKinds == {"Serve", "LeaderWire"}
CertifiedKinds ==
    {"TimeoutCertificate", "CommitQC", "CommitCertificateResponse"}
IneligibleKinds == {"PrepareQC", "TimeoutVote"}
IngressKinds == CertifiedKinds \cup IneligibleKinds
CommandClasses == {"Normal", "Progress", "Completion"}

Stages == {"Ingress", "Runtime", "TrustedTail", "Handled"}
RuntimeCapacity == 4
NormalLimit == 1
ProgressLimit == 2
OrdinaryCompletionLimit == 3
CompletionReserve == OrdinaryCompletionLimit - ProgressLimit
OrdinaryRuntimePrefix == <<"Progress", "Progress", "Completion">>

OwnerFor(barrierKind) ==
    [barrierKind |-> barrierKind,
     identity |-> IF barrierKind = "Serve" THEN "serve-17" ELSE "wire-23",
     context |-> "revision-4",
     height |-> 7,
     view |-> 1]

OwnerIdentities == {OwnerFor(barrierKind) : barrierKind \in BarrierKinds}

OfferContext == "revision-4"
OfferHeight == 7
OfferView == 2

VARIABLES
    ownerIdentity,
    ownerSnapshot,
    ownerRetained,
    offeredKind,
    authenticated,
    stage,
    runtimeQueue,
    pendingProgress,
    pendingCompletion,
    pendingCertified,
    escapePhase,
    unpublishedBodyAvailable,
    conflictingProposalQueued,
    installedTC,
    decided

vars ==
    <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
      authenticated, stage, runtimeQueue, pendingProgress,
      pendingCompletion, pendingCertified, escapePhase,
      unpublishedBodyAvailable, conflictingProposalQueued,
      installedTC, decided>>

CertifiedFenceEscapeKind(kind) == kind \in CertifiedKinds

QueueClassCount(queue, commandClass) ==
  Cardinality(
    {index \in 1..Len(queue): queue[index] = commandClass})

QueueCertifiedCount(queue) ==
  Cardinality(
    {index \in 1..Len(queue):
       CertifiedFenceEscapeKind(queue[index])})

QueueCertifiedCredit(queue) ==
  IF QueueCertifiedCount(queue) = 0 THEN 0 ELSE 1

QueueNoncompletionCount(queue) ==
  Cardinality(
    {index \in 1..Len(queue): queue[index] # "Completion"})

ExternalOwnerCount ==
  (IF unpublishedBodyAvailable THEN 1 ELSE 0)
    + (IF conflictingProposalQueued THEN 1 ELSE 0)

OwnedRuntimeDepth(queue) == Len(queue) + ExternalOwnerCount

OwnedClassCount(queue, commandClass) ==
  IF commandClass = "Progress"
  THEN QueueClassCount(queue, "Progress") + QueueCertifiedCount(queue)
  ELSE QueueClassCount(queue, commandClass)
         + IF commandClass = "Completion" /\ unpublishedBodyAvailable
           THEN 1
           ELSE IF commandClass = "Normal" /\ conflictingProposalQueued
                THEN 1
                ELSE 0

OwnedNoncompletionCount(queue) ==
  OwnedClassCount(queue, "Normal") + OwnedClassCount(queue, "Progress")

CertifiedCreditIn(queue, incomingCertified) ==
  IF incomingCertified
       \/ (RetainedCertifiedCreditEnabled /\ QueueCertifiedCount(queue) > 0)
  THEN 1
  ELSE 0

CanAppendClass(queue, commandKind, commandClass, incomingCertified) ==
  LET credit == CertifiedCreditIn(queue, incomingCertified)
      normalAfter ==
        OwnedClassCount(queue, "Normal")
          + IF commandClass = "Normal" THEN 1 ELSE 0
      noncompletionAfter ==
        OwnedNoncompletionCount(queue)
          + IF commandClass = "Completion" THEN 0 ELSE 1
  IN /\ commandKind \in CommandClasses \cup CertifiedKinds
     /\ commandClass \in CommandClasses
     /\ OwnedRuntimeDepth(queue) < RuntimeCapacity
     /\ OwnedRuntimeDepth(queue) + 1
          <= OrdinaryCompletionLimit + credit
     /\ normalAfter <= NormalLimit
     /\ noncompletionAfter <= ProgressLimit + credit

TypeOK ==
    /\ ownerIdentity \in OwnerIdentities
    /\ ownerSnapshot \in OwnerIdentities
    /\ ownerRetained \in BOOLEAN
    /\ offeredKind \in IngressKinds
    /\ authenticated \in BOOLEAN
    /\ stage \in Stages
    /\ runtimeQueue \in Seq(CommandClasses \cup CertifiedKinds)
    /\ pendingProgress \in 0..2
    /\ pendingCompletion \in 0..1
    /\ pendingCertified \subseteq CertifiedKinds
    /\ escapePhase \in {"Fresh", "Charged", "Spent"}
    /\ unpublishedBodyAvailable \in BOOLEAN
    /\ conflictingProposalQueued \in BOOLEAN
    /\ ~(unpublishedBodyAvailable /\ conflictingProposalQueued)
    /\ installedTC \in BOOLEAN
    /\ decided \in BOOLEAN

Init ==
    /\ ownerIdentity \in OwnerIdentities
    /\ ownerSnapshot = ownerIdentity
    /\ ownerRetained = TRUE
    /\ offeredKind \in IngressKinds
    /\ authenticated \in BOOLEAN
    /\ stage = "Ingress"
    /\ \/ /\ runtimeQueue = OrdinaryRuntimePrefix
              /\ pendingProgress = 0
              /\ pendingCompletion = 0
              /\ unpublishedBodyAvailable = FALSE
              /\ conflictingProposalQueued = FALSE
       \/ /\ runtimeQueue = <<>>
              /\ pendingProgress = 2
              /\ pendingCompletion = 1
              /\ unpublishedBodyAvailable = FALSE
              /\ conflictingProposalQueued = FALSE
       \/ /\ runtimeQueue = <<"Progress">>
              /\ pendingProgress = 1
              /\ pendingCompletion = 0
              /\ unpublishedBodyAvailable = FALSE
              /\ conflictingProposalQueued = TRUE
    /\ pendingCertified =
         IF CertifiedFenceEscapeKind(offeredKind)
         THEN CertifiedKinds \ {offeredKind}
         ELSE {}
    /\ installedTC = FALSE
    /\ decided = FALSE
    /\ escapePhase = "Fresh"

OfferAdvancesRetainedOwner ==
    /\ OfferContext = ownerIdentity.context
    /\ OfferHeight = ownerIdentity.height
    /\ OfferView >= ownerIdentity.view

CanUseCertifiedFinalSlot ==
    /\ CertifiedFenceEscapeEnabled
    /\ OwnedRuntimeDepth(runtimeQueue) = RuntimeCapacity - 1
    /\ OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity
    /\ CanAppendClass(runtimeQueue, offeredKind, "Progress", TRUE)

CanUseCertifiedEarlySlot ==
    /\ CertifiedFenceEscapeEnabled
    /\ OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity - 1
    /\ CanAppendClass(runtimeQueue, offeredKind, "Progress", TRUE)

AdmitCertifiedEscape ==
    /\ stage = "Ingress"
    /\ ownerRetained
    /\ authenticated
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ escapePhase = "Fresh"
    /\ OfferAdvancesRetainedOwner
    /\ CanUseCertifiedFinalSlot
    /\ stage' = "Runtime"
    /\ runtimeQueue' = Append(runtimeQueue, offeredKind)
    /\ escapePhase' = "Charged"
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, pendingProgress, pendingCompletion,
                    pendingCertified, unpublishedBodyAvailable,
                    conflictingProposalQueued, installedTC, decided>>

AdmitCertifiedEscapeEarly ==
    /\ stage = "Ingress"
    /\ ownerRetained
    /\ authenticated
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ escapePhase = "Fresh"
    /\ OfferAdvancesRetainedOwner
    /\ CanUseCertifiedEarlySlot
    /\ stage' = "Runtime"
    /\ runtimeQueue' = Append(runtimeQueue, offeredKind)
    /\ escapePhase' = "Charged"
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, pendingProgress, pendingCompletion,
                    pendingCertified, unpublishedBodyAvailable,
                    conflictingProposalQueued, installedTC, decided>>

AdmitOrdinaryProgress ==
    /\ pendingProgress > 0
    /\ CanAppendClass(runtimeQueue, "Progress", "Progress", FALSE)
    /\ runtimeQueue' = Append(runtimeQueue, "Progress")
    /\ pendingProgress' = pendingProgress - 1
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, stage, pendingCompletion, pendingCertified,
                    escapePhase, unpublishedBodyAvailable,
                    conflictingProposalQueued, installedTC, decided>>

AdmitOrdinaryCompletion ==
    /\ pendingCompletion > 0
    /\ CanAppendClass(runtimeQueue, "Completion", "Completion", FALSE)
    /\ runtimeQueue' = Append(runtimeQueue, "Completion")
    /\ pendingCompletion' = pendingCompletion - 1
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, stage, pendingProgress, pendingCertified,
                    escapePhase, unpublishedBodyAvailable,
                    conflictingProposalQueued, installedTC, decided>>

AdmitAdditionalCertified(kind) ==
    /\ stage = "Runtime"
    /\ authenticated
    /\ escapePhase = "Fresh"
    /\ kind \in pendingCertified
    /\ CanAppendClass(runtimeQueue, kind, "Progress", TRUE)
    /\ runtimeQueue' = Append(runtimeQueue, kind)
    /\ pendingCertified' = pendingCertified \ {kind}
    /\ escapePhase' = "Charged"
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, stage, pendingProgress, pendingCompletion,
                    unpublishedBodyAvailable, conflictingProposalQueued,
                    installedTC, decided>>

ReserveUnpublishedBodyAvailable ==
    /\ conflictingProposalQueued
    /\ ~unpublishedBodyAvailable
    /\ unpublishedBodyAvailable' = TRUE
    /\ conflictingProposalQueued' = FALSE
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, stage, runtimeQueue, pendingProgress,
                    pendingCompletion, pendingCertified, escapePhase,
                    installedTC, decided>>

CertifiedQueueIndices ==
  {index \in 1..Len(runtimeQueue): runtimeQueue[index] = offeredKind}

FirstCertifiedQueueIndex ==
  CHOOSE index \in CertifiedQueueIndices:
    \A other \in CertifiedQueueIndices: index <= other

RemoveAt(sequence, index) ==
  (IF index = 1 THEN <<>> ELSE SubSeq(sequence, 1, index - 1))
    \o (IF index = Len(sequence)
        THEN <<>>
        ELSE SubSeq(sequence, index + 1, Len(sequence)))

DispatchCertifiedEscape ==
    /\ stage = "Runtime"
    /\ ownerRetained
    /\ CertifiedFenceEscapeEnabled
    /\ OwnedRuntimeDepth(runtimeQueue) = RuntimeCapacity
    /\ CertifiedQueueIndices # {}
    /\ FirstCertifiedQueueIndex = Len(runtimeQueue)
    /\ runtimeQueue[FirstCertifiedQueueIndex] = offeredKind
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ stage' = "TrustedTail"
    /\ runtimeQueue' = SubSeq(runtimeQueue, 1, Len(runtimeQueue) - 1)
    /\ escapePhase' =
         IF QueueCertifiedCount(runtimeQueue') = 0
              /\ ~CanAppendClass(
                    runtimeQueue', "Completion", "Completion", FALSE)
         THEN "Spent"
         ELSE escapePhase
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, pendingProgress, pendingCompletion,
                    pendingCertified, unpublishedBodyAvailable,
                    conflictingProposalQueued, installedTC, decided>>

DispatchEarlyCertifiedEscape ==
    /\ stage = "Runtime"
    /\ ownerRetained
    /\ CertifiedFenceEscapeEnabled
    /\ CertifiedQueueIndices # {}
    /\ FirstCertifiedQueueIndex # Len(runtimeQueue)
    /\ runtimeQueue[FirstCertifiedQueueIndex] = offeredKind
    /\ stage' = "TrustedTail"
    /\ runtimeQueue' = RemoveAt(runtimeQueue, FirstCertifiedQueueIndex)
    /\ escapePhase' =
         IF QueueCertifiedCount(runtimeQueue') = 0
              /\ ~CanAppendClass(
                    runtimeQueue', "Completion", "Completion", FALSE)
         THEN "Spent"
         ELSE escapePhase
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, pendingProgress, pendingCompletion,
                    pendingCertified, unpublishedBodyAvailable,
                    conflictingProposalQueued, installedTC, decided>>

RunCertifiedTrustedTail ==
    /\ stage = "TrustedTail"
    /\ ownerRetained
    /\ CertifiedFenceEscapeEnabled
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ stage' = "Handled"
    /\ ownerRetained' = FALSE
    /\ installedTC' = (offeredKind = "TimeoutCertificate")
    /\ decided' = (offeredKind \in {"CommitQC", "CommitCertificateResponse"})
    /\ escapePhase' = "Fresh"
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, offeredKind,
                    authenticated, runtimeQueue, pendingProgress,
                    pendingCompletion, pendingCertified,
                    unpublishedBodyAvailable, conflictingProposalQueued>>

Next ==
    \/ AdmitCertifiedEscape
    \/ AdmitCertifiedEscapeEarly
    \/ AdmitOrdinaryProgress
    \/ AdmitOrdinaryCompletion
    \/ \E kind \in CertifiedKinds: AdmitAdditionalCertified(kind)
    \/ ReserveUnpublishedBodyAvailable
    \/ DispatchCertifiedEscape
    \/ DispatchEarlyCertifiedEscape
    \/ RunCertifiedTrustedTail

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(AdmitCertifiedEscape)
    /\ WF_vars(AdmitCertifiedEscapeEarly)
    /\ WF_vars(AdmitOrdinaryProgress)
    /\ WF_vars(AdmitOrdinaryCompletion)
    /\ WF_vars(ReserveUnpublishedBodyAvailable)
    /\ \A kind \in CertifiedKinds:
         WF_vars(AdmitAdditionalCertified(kind))
    /\ WF_vars(DispatchCertifiedEscape)
    /\ WF_vars(DispatchEarlyCertifiedEscape)
    /\ WF_vars(RunCertifiedTrustedTail)

OwnerIdentityNeverReplaced == ownerIdentity = ownerSnapshot

OwnerRetainedAcrossEscape ==
    stage \in {"Runtime", "TrustedTail"} => ownerRetained

NoOrdinaryRuntimeDisplacement ==
    LET credit == QueueCertifiedCredit(runtimeQueue)
    IN /\ OwnedClassCount(runtimeQueue, "Normal") <= NormalLimit
       /\ OwnedNoncompletionCount(runtimeQueue) - credit <= ProgressLimit
    /\ OwnedClassCount(runtimeQueue, "Completion") <= CompletionReserve

ReservedSlotOnlyCertified ==
    OwnedRuntimeDepth(runtimeQueue) = RuntimeCapacity =>
        /\ authenticated
        /\ QueueCertifiedCount(runtimeQueue) >= 1
        /\ CertifiedFenceEscapeKind(offeredKind)

SingleCertifiedCredit ==
  /\ QueueCertifiedCredit(runtimeQueue) \in {0, 1}
  /\ (QueueCertifiedCount(runtimeQueue) > 0
        <=> QueueCertifiedCredit(runtimeQueue) = 1)

OrdinaryCapacityGeometry ==
  LET credit == QueueCertifiedCredit(runtimeQueue)
  IN /\ CompletionReserve = 1
     /\ OwnedRuntimeDepth(runtimeQueue) <= RuntimeCapacity
     /\ OwnedRuntimeDepth(runtimeQueue) - credit <= OrdinaryCompletionLimit
     /\ OwnedClassCount(runtimeQueue, "Normal") <= NormalLimit
     /\ OwnedNoncompletionCount(runtimeQueue) - credit <= ProgressLimit

CertifiedFirstCompletionCorridor ==
  /\ stage = "Runtime"
  /\ QueueCertifiedCount(runtimeQueue) >= 1
  /\ pendingCompletion = 1
  /\ OwnedNoncompletionCount(runtimeQueue) - 1 = ProgressLimit
  /\ OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity
  => CanAppendClass(runtimeQueue, "Completion", "Completion", FALSE)

CertifiedFirstProgressCorridor ==
  /\ stage = "Runtime"
  /\ QueueCertifiedCount(runtimeQueue) >= 1
  /\ pendingProgress > 0
  /\ OwnedNoncompletionCount(runtimeQueue) - 1 < ProgressLimit
  /\ OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity
  => CanAppendClass(runtimeQueue, "Progress", "Progress", FALSE)

PrepareQcCannotUseEscape ==
    offeredKind = "PrepareQC" => stage = "Ingress"

RawTimeoutVoteCannotUseEscape ==
    offeredKind = "TimeoutVote" => stage = "Ingress"

AuthenticationRequiredForEscape ==
    stage \in {"Runtime", "TrustedTail", "Handled"} => authenticated

HandledOutcomeExact ==
    stage = "Handled" =>
        IF offeredKind = "TimeoutCertificate"
        THEN installedTC /\ ~decided
        ELSE ~installedTC /\ decided

CertifiedEscapeEpisodeIsOneShot ==
  /\ escapePhase \in {"Fresh", "Charged", "Spent"}
  /\ (escapePhase \in {"Charged", "Spent"}
        => /\ ~ENABLED AdmitCertifiedEscape
           /\ ~ENABLED AdmitCertifiedEscapeEarly
           /\ ~ENABLED (\E kind \in CertifiedKinds:
                          AdmitAdditionalCertified(kind)))
  /\ (stage = "Handled" => /\ ~ownerRetained
                             /\ escapePhase = "Fresh")

UnpublishedBodyAvailableOwnsOrdinaryCompletion ==
  /\ ~(unpublishedBodyAvailable /\ conflictingProposalQueued)
  /\ (unpublishedBodyAvailable
        => OwnedClassCount(runtimeQueue, "Completion") >= 1)
  /\ (conflictingProposalQueued
        => OwnedClassCount(runtimeQueue, "Normal") >= 1)

THEOREM BodyAvailableReservationAtomicallyReplacesConflict ==
  ReserveUnpublishedBodyAvailable
    => /\ unpublishedBodyAvailable'
       /\ ~conflictingProposalQueued'
       /\ OwnedRuntimeDepth(runtimeQueue') = OwnedRuntimeDepth(runtimeQueue)
BY DEF ReserveUnpublishedBodyAvailable,
       OwnedRuntimeDepth, ExternalOwnerCount

CertifiedEscapeEventuallyHandled ==
    (authenticated /\ CertifiedFenceEscapeKind(offeredKind) /\ ownerRetained)
        ~> (stage = "Handled")

=============================================================================

------------- MODULE SumeragiV2Revision4CertifiedFenceReservation -------------
\* Bounded authoritative revision-4 scheduler kernel for the certificate which
\* retires an exact retained Serve or leader-wire fence.  The ordinary runtime
\* prefix is saturated.  Only an authenticated TC, direct CommitQC, or
\* CommitCertificateResponse carrying a CommitQC may consume the final physical
\* slot, and only the typed pacemaker corridor may dispatch that root and its
\* trusted causal tail while the retained owner remains immutable.

EXTENDS FiniteSets, Naturals, Sequences, TLC

CONSTANT CertifiedFenceEscapeEnabled

BarrierKinds == {"Serve", "LeaderWire"}
CertifiedKinds ==
    {"TimeoutCertificate", "CommitQC", "CommitCertificateResponse"}
IneligibleKinds == {"PrepareQC", "TimeoutVote"}
IngressKinds == CertifiedKinds \cup IneligibleKinds

Stages == {"Ingress", "Runtime", "TrustedTail", "Handled"}
RuntimeCapacity == 3
OrdinaryRuntimePrefix == <<"Normal", "Completion">>

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
    installedTC,
    decided

vars ==
    <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
      authenticated, stage, runtimeQueue, installedTC, decided>>

TypeOK ==
    /\ ownerIdentity \in OwnerIdentities
    /\ ownerSnapshot \in OwnerIdentities
    /\ ownerRetained \in BOOLEAN
    /\ offeredKind \in IngressKinds
    /\ authenticated \in BOOLEAN
    /\ stage \in Stages
    /\ runtimeQueue \in Seq(IngressKinds \cup {"Normal", "Completion"})
    /\ installedTC \in BOOLEAN
    /\ decided \in BOOLEAN

Init ==
    /\ ownerIdentity \in OwnerIdentities
    /\ ownerSnapshot = ownerIdentity
    /\ ownerRetained = TRUE
    /\ offeredKind \in IngressKinds
    /\ authenticated \in BOOLEAN
    /\ stage = "Ingress"
    /\ runtimeQueue = OrdinaryRuntimePrefix
    /\ installedTC = FALSE
    /\ decided = FALSE

CertifiedFenceEscapeKind(kind) == kind \in CertifiedKinds

OfferAdvancesRetainedOwner ==
    /\ OfferContext = ownerIdentity.context
    /\ OfferHeight = ownerIdentity.height
    /\ OfferView >= ownerIdentity.view

CanUseCertifiedFinalSlot ==
    /\ CertifiedFenceEscapeEnabled
    /\ Len(runtimeQueue) = RuntimeCapacity - 1
    /\ Len(runtimeQueue) < RuntimeCapacity

\* The ingress barrier remains owned.  Admission consumes only the final
\* certified slot and does not replace, retire, or mutate that exact owner.
AdmitCertifiedEscape ==
    /\ stage = "Ingress"
    /\ ownerRetained
    /\ authenticated
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ OfferAdvancesRetainedOwner
    /\ CanUseCertifiedFinalSlot
    /\ stage' = "Runtime"
    /\ runtimeQueue' = Append(runtimeQueue, offeredKind)
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, installedTC, decided>>

\* The typed pacemaker selects the exact certified root even though it was
\* admitted after the retained barrier.  Ordinary queue entries stay put.
DispatchCertifiedEscape ==
    /\ stage = "Runtime"
    /\ ownerRetained
    /\ CertifiedFenceEscapeEnabled
    /\ Len(runtimeQueue) = RuntimeCapacity
    /\ runtimeQueue[RuntimeCapacity] = offeredKind
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ stage' = "TrustedTail"
    /\ runtimeQueue' = SubSeq(runtimeQueue, 1, RuntimeCapacity - 1)
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, installedTC, decided>>

\* Trusted Completion/Progress descendants inherit the certified Progress
\* root.  This abstract step is deliberately unavailable to ordinary roots.
RunCertifiedTrustedTail ==
    /\ stage = "TrustedTail"
    /\ ownerRetained
    /\ CertifiedFenceEscapeEnabled
    /\ CertifiedFenceEscapeKind(offeredKind)
    /\ stage' = "Handled"
    /\ installedTC' = (offeredKind = "TimeoutCertificate")
    /\ decided' = (offeredKind \in {"CommitQC", "CommitCertificateResponse"})
    /\ UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained, offeredKind,
                    authenticated, runtimeQueue>>

Next ==
    \/ AdmitCertifiedEscape
    \/ DispatchCertifiedEscape
    \/ RunCertifiedTrustedTail

Spec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(AdmitCertifiedEscape)
    /\ WF_vars(DispatchCertifiedEscape)
    /\ WF_vars(RunCertifiedTrustedTail)

OwnerIdentityNeverReplaced == ownerIdentity = ownerSnapshot

OwnerRetainedAcrossEscape ==
    stage \in {"Runtime", "TrustedTail", "Handled"} => ownerRetained

NoOrdinaryRuntimeDisplacement ==
    /\ Len(runtimeQueue) \in {(RuntimeCapacity - 1), RuntimeCapacity}
    /\ SubSeq(runtimeQueue, 1, RuntimeCapacity - 1) = OrdinaryRuntimePrefix

ReservedSlotOnlyCertified ==
    Len(runtimeQueue) = RuntimeCapacity =>
        /\ authenticated
        /\ CertifiedFenceEscapeKind(runtimeQueue[RuntimeCapacity])
        /\ runtimeQueue[RuntimeCapacity] = offeredKind

AtMostOneCertifiedRuntimeOwner ==
    Cardinality(
      {index \in 1..Len(runtimeQueue) :
         runtimeQueue[index] \in CertifiedKinds}) <= 1

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

CertifiedEscapeEventuallyHandled ==
    (authenticated /\ CertifiedFenceEscapeKind(offeredKind) /\ ownerRetained)
        ~> (stage = "Handled")

=============================================================================

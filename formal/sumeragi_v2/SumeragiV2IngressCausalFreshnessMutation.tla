---- MODULE SumeragiV2IngressCausalFreshnessMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Bounded regression model for response ingress freshness across causal owners.

The initial state places exact authenticated candidates for both response
paths in the causal carrier while their matching wire responses wait in
ingress.  A CertifiedResponse drains into tracked completion work; a
CommitCertificateResponse drains into the serialized Progress queue.  The
repaired predicate checks every scheduler carrier and coalesces both
duplicates.  The historical mutation checks only queued/deferred/tracked
in-flight work, misses both causal owners, and creates both second owners.

This model intentionally keeps the response and candidate records immutable
and exact.  The only switch is RequireSchedulerWideFreshness; capacity,
authentication, request matching, and causal ownership are already satisfied.
***************************************************************************)

CONSTANT RequireSchedulerWideFreshness

ASSUME RequireSchedulerWideFreshness \in BOOLEAN

Node == "ValidatorA"
Signer == "ValidatorB"
Context == [height |-> 7, epoch |-> 2]

CertifiedResponse ==
  [kind |-> "CertifiedResponse",
   source |-> Signer,
   envelope |->
     [recipient |-> Node,
      height |-> Context.height,
      view |-> 4,
      subject |-> "subject-7",
      bodyHash |-> "body-7"]]

CommitCertificateResponse ==
  [kind |-> "CommitCertificateResponse",
   source |-> Signer,
   envelope |->
     [recipient |-> Node,
      qc |->
        [context |-> Context,
         phase |-> "Commit",
         view |-> 4,
         subject |-> "subject-7",
         signers |-> {Signer}]]]

ExactResponses == {CertifiedResponse, CommitCertificateResponse}

ResponseCandidate(response) ==
  IF response.kind = "CertifiedResponse"
  THEN [class |-> "Completion",
        kind |-> "FetchCertifiedBody",
        node |-> response.envelope.recipient,
        height |-> response.envelope.height,
        view |-> response.envelope.view,
        subject |-> response.envelope.subject,
        source |-> response.source,
        response |-> response]
  ELSE [class |-> "Progress",
        kind |-> "DeliverQC",
        node |-> response.envelope.recipient,
        height |-> response.envelope.qc.context.height,
        view |-> response.envelope.qc.view,
        subject |-> response.envelope.qc.subject,
        source |-> response.source,
        response |-> response]

CandidateUniverse == {ResponseCandidate(response): response \in ExactResponses}

VARIABLES phase,
          ingressLane,
          causalCarrier,
          queuedCarrier,
          deferredCarrier,
          trackedCarrier,
          coalesced,
          trackedDuplicateCreated,
          queuedDuplicateCreated

vars ==
  <<phase, ingressLane, causalCarrier, queuedCarrier, deferredCarrier,
    trackedCarrier, coalesced, trackedDuplicateCreated,
    queuedDuplicateCreated>>

(***************************************************************************
CandidateInFlight mirrors the old admission guard: causal ownership is
absent.  CandidateScheduled is the repaired scheduler-wide inventory.
***************************************************************************)
CandidateInFlight(candidate) ==
  candidate \in queuedCarrier \cup deferredCarrier \cup trackedCarrier

CandidateScheduled(candidate) ==
  candidate \in causalCarrier \cup queuedCarrier \cup deferredCarrier
    \cup trackedCarrier

IngressCandidateFresh(candidate) ==
  IF RequireSchedulerWideFreshness
  THEN ~CandidateScheduled(candidate)
  ELSE ~CandidateInFlight(candidate)

TypeInvariant ==
  /\ phase \in {"IngressReady", "Coalesced", "Admitted"}
  /\ ingressLane \in Seq(ExactResponses)
  /\ causalCarrier \subseteq CandidateUniverse
  /\ queuedCarrier \subseteq CandidateUniverse
  /\ deferredCarrier \subseteq CandidateUniverse
  /\ trackedCarrier \subseteq CandidateUniverse
  /\ coalesced \in BOOLEAN
  /\ trackedDuplicateCreated \in BOOLEAN
  /\ queuedDuplicateCreated \in BOOLEAN

PairwiseSingleOwnership ==
  /\ causalCarrier \cap queuedCarrier = {}
  /\ causalCarrier \cap deferredCarrier = {}
  /\ causalCarrier \cap trackedCarrier = {}
  /\ queuedCarrier \cap deferredCarrier = {}
  /\ queuedCarrier \cap trackedCarrier = {}
  /\ deferredCarrier \cap trackedCarrier = {}

SchedulerWideDuplicateCoalesced ==
  /\ RequireSchedulerWideFreshness
  /\ phase # "IngressReady"
  => /\ phase = "Coalesced"
     /\ coalesced
     /\ causalCarrier = CandidateUniverse
     /\ queuedCarrier = {}
     /\ deferredCarrier = {}
     /\ trackedCarrier = {}
     /\ ~trackedDuplicateCreated
     /\ ~queuedDuplicateCreated

IngressOccurrenceConsumedExactlyOnce ==
  phase # "IngressReady" => ingressLane = <<>>

Init ==
  /\ phase = "IngressReady"
  /\ ingressLane = <<CertifiedResponse, CommitCertificateResponse>>
  /\ causalCarrier = CandidateUniverse
  /\ queuedCarrier = {}
  /\ deferredCarrier = {}
  /\ trackedCarrier = {}
  /\ coalesced = FALSE
  /\ trackedDuplicateCreated = FALSE
  /\ queuedDuplicateCreated = FALSE

(***************************************************************************
An ingress attempt consumes the wire occurrence.  A non-fresh exact response
is coalesced into its causal owner.  The mutation deems it fresh and creates
the response-specific second scheduler owner while retaining the causal one.
***************************************************************************)
DrainIngress ==
  LET certifiedCandidate == ResponseCandidate(CertifiedResponse)
      commitCandidate == ResponseCandidate(CommitCertificateResponse)
      certifiedFresh == IngressCandidateFresh(certifiedCandidate)
      commitFresh == IngressCandidateFresh(commitCandidate)
  IN /\ phase = "IngressReady"
     /\ ingressLane = <<CertifiedResponse, CommitCertificateResponse>>
     /\ ingressLane' = <<>>
     /\ UNCHANGED <<causalCarrier, deferredCarrier>>
     /\ phase' = IF certifiedFresh \/ commitFresh
                  THEN "Admitted"
                  ELSE "Coalesced"
     /\ trackedCarrier' =
          IF certifiedFresh
          THEN trackedCarrier \cup {certifiedCandidate}
          ELSE trackedCarrier
     /\ queuedCarrier' =
          IF commitFresh
          THEN queuedCarrier \cup {commitCandidate}
          ELSE queuedCarrier
     /\ coalesced' = ~(certifiedFresh \/ commitFresh)
     /\ trackedDuplicateCreated' =
          certifiedFresh /\ certifiedCandidate \in causalCarrier
     /\ queuedDuplicateCreated' =
          commitFresh /\ commitCandidate \in causalCarrier

Next == DrainIngress

====

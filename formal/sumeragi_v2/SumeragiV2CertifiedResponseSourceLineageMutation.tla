---- MODULE SumeragiV2CertifiedResponseSourceLineageMutation ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
Bounded mutation witness for the certified-response Decision handoff.

An honest certified response is a transport completion.  Its outer `source`
is therefore the aggregate untrusted relay/resource hop.  The response
separately retains the exact signed-request hash, authenticated current
archive server, archive signature owner, and explicit frozen-QC signer
citation.  The fixed ownership predicate follows the request hash and cited
responder.  The mutant incorrectly treats the outer relay source as the
CommitQC signer and consequently loses the Decision recovery owner when the
response candidate is scheduled.  The legacy fixed Mode string is retained
for the frozen runner configuration; it now selects this explicit projection.

This small model isolates the source projection used by
`DecisionCertifiedFetchOwned`.  It is bounded mutation evidence, not a proof
of the complete asynchronous ingress relation.
***************************************************************************)

CONSTANT Mode

Modes == {"EmbeddedCitedSignerSurrogate", "OuterTransportSource"}

ASSUME Mode \in Modes

Node == "validator-0"
RemoteSigner == "validator-1"
OtherSigner == "validator-2"
RotatedArchive == "archive-current-9"
AsyncUntrustedSource == "untrusted-transport-hop"
Validators == {Node, RemoteSigner, OtherSigner, "validator-3"}

RecoveryContext ==
  [chain |-> "sora",
   height |-> 9,
   epoch |-> 2,
   lineage |-> "parent-8"]
RecoveryView == 4
RecoverySubject == "decided-block-9"

CommitQc ==
  [context |-> RecoveryContext,
   height |-> RecoveryContext.height,
   view |-> RecoveryView,
   phase |-> "Commit",
   subject |-> RecoverySubject,
   signers |-> {Node, RemoteSigner, OtherSigner}]

ExactRequestPreimage ==
  [round |-> [height |-> RecoveryContext.height, view |-> RecoveryView],
   subject |-> RecoverySubject,
   certificate |-> CommitQc,
   requester |-> Node]

ExactRequestSignature ==
  [signer |-> Node, preimage |-> ExactRequestPreimage]

ExactRequestHash ==
  [exactSignedRequest |->
    [preimage |-> ExactRequestPreimage,
     signature |-> ExactRequestSignature]]

CertifiedRequest ==
  [kind |-> "CertifiedRequest",
   source |-> Node,
   envelope |->
     [recipient |-> RotatedArchive,
      height |-> RecoveryContext.height,
      view |-> RecoveryView,
      subject |-> RecoverySubject,
      requester |-> Node,
      certificate |-> CommitQc,
      requestHash |-> ExactRequestHash]]

CertifiedResponse ==
  [kind |-> "CertifiedResponse",
   source |-> AsyncUntrustedSource,
   envelope |->
     [recipient |-> Node,
      height |-> RecoveryContext.height,
      view |-> RecoveryView,
      subject |-> RecoverySubject,
      requestHash |-> ExactRequestHash,
      archiveServer |-> RotatedArchive,
      citedResponder |-> RemoteSigner,
      signatureOwner |-> RotatedArchive]]

VARIABLES stage, candidateScheduled

vars == <<stage, candidateScheduled>>

Init ==
  /\ stage = "IngressResponse"
  /\ candidateScheduled = FALSE

ScheduleCertifiedResponse ==
  /\ stage = "IngressResponse"
  /\ ~candidateScheduled
  /\ stage' = "FetchCertifiedBody"
  /\ candidateScheduled' = TRUE

Quiescent ==
  /\ stage = "FetchCertifiedBody"
  /\ UNCHANGED vars

Next ==
  \/ ScheduleCertifiedResponse
  \/ Quiescent

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeInvariant ==
  /\ stage \in {"IngressResponse", "FetchCertifiedBody"}
  /\ candidateScheduled \in BOOLEAN

HonestCertifiedResponseShape ==
  /\ CommitQc.signers \subseteq Validators
  /\ Node \in CommitQc.signers
  /\ RemoteSigner \in CommitQc.signers \ {Node}
  /\ AsyncUntrustedSource \notin Validators
  /\ RotatedArchive \notin CommitQc.signers
  /\ CertifiedResponse.source = AsyncUntrustedSource
  /\ CertifiedRequest.source = Node
  /\ CertifiedRequest.envelope.recipient = RotatedArchive
  /\ CertifiedRequest.envelope.certificate = CommitQc
  /\ CertifiedResponse.envelope.requestHash =
       CertifiedRequest.envelope.requestHash
  /\ CertifiedResponse.envelope.archiveServer = RotatedArchive
  /\ CertifiedResponse.envelope.signatureOwner = RotatedArchive

ExplicitCitedResponderOwnsResponse ==
  /\ CertifiedResponse.kind = "CertifiedResponse"
  /\ CertifiedResponse.envelope.recipient = Node
  /\ CertifiedResponse.envelope.height = CommitQc.context.height
  /\ CertifiedResponse.envelope.view = CommitQc.view
  /\ CertifiedResponse.envelope.subject = CommitQc.subject
  /\ CertifiedResponse.envelope.requestHash = ExactRequestHash
  /\ CertifiedResponse.envelope.signatureOwner =
       CertifiedResponse.envelope.archiveServer
  /\ CertifiedResponse.envelope.citedResponder \in CommitQc.signers
  /\ candidateScheduled

OuterTransportSourceOwnsResponse ==
  /\ CertifiedResponse.kind = "CertifiedResponse"
  /\ CertifiedResponse.source \in CommitQc.signers
  /\ CertifiedResponse.envelope.recipient = Node
  /\ CertifiedResponse.envelope.height = CommitQc.context.height
  /\ CertifiedResponse.envelope.view = CommitQc.view
  /\ CertifiedResponse.envelope.subject = CommitQc.subject
  /\ candidateScheduled

DecisionCertifiedFetchOwned ==
  IF Mode = "EmbeddedCitedSignerSurrogate"
  THEN ExplicitCitedResponderOwnsResponse
  ELSE OuterTransportSourceOwnsResponse

DecisionRecoveryOwnerRetained ==
  stage = "IngressResponse" \/ DecisionCertifiedFetchOwned

=============================================================================

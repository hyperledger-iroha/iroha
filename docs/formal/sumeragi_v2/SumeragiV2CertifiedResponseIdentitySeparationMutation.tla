---- MODULE SumeragiV2CertifiedResponseIdentitySeparationMutation ----
EXTENDS Naturals

(***************************************************************************
Bounded mutation witness for certified-response authority identities.

Production keeps these values distinct:

  * the relay/resource hop charged by fair ingress;
  * the original route target selected for the exact live request;
  * the authenticated current archive server which signs the response;
  * the exact signed-request hash resolved in the outstanding table; and
  * the frozen-QC signer index cited by the response.

The exact request was originally routed to a distinct current voter.  The
honest response is instead signed by a rotated, zero-power archive server
outside the current voting roster and arrives through an unrelated relay.
The fixed predicate authenticates the response under that archive server,
checks the exact request hash and coordinates, and checks only the cited
responder against the frozen QC.  Routing is a liveness choice and grants no
response authority.

One mutant conflates the archive server with the cited responder.  A second
mutant incorrectly requires the archive server to equal the request's old
route target.  Both reject the valid non-target recovery response.  Exact
request-hash, coordinate, cited-signer, and signature-owner negative controls
are rejected without consuming or substituting the outstanding request.
This small model is mutation evidence for the identity projection.  It is not
a proof of the complete asynchronous ingress or body pipeline.
***************************************************************************)

CONSTANT Mode

Modes ==
  {"SeparatedIdentities",
   "ArchiveServerMustBeQcSigner",
   "ArchiveServerMustMatchRouteTarget"}

ASSUME Mode \in Modes

Requester == "validator-0"
FrozenSigner == "validator-1"
OtherFrozenSigner == "validator-2"
OriginalRouteTarget == "validator-route-current-3"
RotatedArchive == "archive-current-9"
UntrustedRelay == "relay-untrusted-4"

CurrentVotingRoster ==
  {Requester, FrozenSigner, OtherFrozenSigner, OriginalRouteTarget}

CurrentVotingPower(peer) ==
  IF peer \in CurrentVotingRoster THEN 1 ELSE 0

RecoveryContext ==
  [chain |-> "sora",
   height |-> 12,
   epoch |-> 3,
   lineage |-> "parent-11"]
RecoveryView == 5
RecoverySubject == "decided-block-12"

CommitQc ==
  [context |-> RecoveryContext,
   view |-> RecoveryView,
   phase |-> "Commit",
   subject |-> RecoverySubject,
   signers |-> {Requester, FrozenSigner, OtherFrozenSigner}]

ExactRequestPreimage ==
  [round |-> [height |-> RecoveryContext.height, view |-> RecoveryView],
   subject |-> RecoverySubject,
   certificate |-> CommitQc,
   requester |-> Requester]

ExactRequestSignature ==
  [signer |-> Requester, preimage |-> ExactRequestPreimage]

ExactRequestHash ==
  [exactSignedRequest |->
    [preimage |-> ExactRequestPreimage,
     signature |-> ExactRequestSignature]]

CertifiedRequest ==
  [kind |-> "CertifiedRequest",
   requester |-> Requester,
   requestHash |-> ExactRequestHash,
   certificate |-> CommitQc,
   context |-> RecoveryContext,
   view |-> RecoveryView,
   subject |-> RecoverySubject,
   routeTarget |-> OriginalRouteTarget]

CertifiedResponse(signatureOwner) ==
  [kind |-> "CertifiedResponse",
   via |-> UntrustedRelay,
   archiveServer |-> RotatedArchive,
   requestHash |-> ExactRequestHash,
   recipient |-> Requester,
   context |-> RecoveryContext,
   view |-> RecoveryView,
   subject |-> RecoverySubject,
   citedResponder |-> FrozenSigner,
   signatureOwner |-> signatureOwner]

HonestResponse == CertifiedResponse(RotatedArchive)
RelaySignedResponse == CertifiedResponse(UntrustedRelay)
WrongRequestPreimage ==
  [ExactRequestPreimage EXCEPT !.subject = "different-decided-block-12"]
WrongRequestSignature ==
  [signer |-> Requester, preimage |-> WrongRequestPreimage]
WrongRequestHash ==
  [exactSignedRequest |->
    [preimage |-> WrongRequestPreimage,
     signature |-> WrongRequestSignature]]
RequestHashMismatchResponse ==
  [HonestResponse EXCEPT !.requestHash = WrongRequestHash]
CoordinateMismatchResponse ==
  [HonestResponse EXCEPT !.view = RecoveryView + 1]
CitedSignerMismatchResponse ==
  [HonestResponse EXCEPT !.citedResponder = OriginalRouteTarget]

ExactResponseCoordinates(response) ==
  /\ response.requestHash = CertifiedRequest.requestHash
  /\ response.recipient = CertifiedRequest.requester
  /\ response.context = CertifiedRequest.context
  /\ response.view = CertifiedRequest.view
  /\ response.subject = CertifiedRequest.subject

ArchiveSignatureAuthenticated(response) ==
  response.signatureOwner = response.archiveServer

SeparatedResponseAuthorized(response) ==
  /\ response.kind = "CertifiedResponse"
  /\ ExactResponseCoordinates(response)
  /\ ArchiveSignatureAuthenticated(response)
  /\ response.citedResponder \in CertifiedRequest.certificate.signers

ConflatedResponseAuthorized(response) ==
  /\ SeparatedResponseAuthorized(response)
  /\ response.archiveServer \in CommitQc.signers
  /\ response.archiveServer = response.citedResponder

RouteBoundResponseAuthorized(response) ==
  /\ SeparatedResponseAuthorized(response)
  /\ response.archiveServer = CertifiedRequest.routeTarget

ResponseAuthorized(response) ==
  CASE Mode = "SeparatedIdentities" ->
         SeparatedResponseAuthorized(response)
    [] Mode = "ArchiveServerMustBeQcSigner" ->
         ConflatedResponseAuthorized(response)
    [] OTHER ->
         RouteBoundResponseAuthorized(response)

NoOutstandingRequest == [kind |-> "NoOutstandingRequest"]

AttemptKinds ==
  {"None",
   "Honest",
   "RequestHashMismatch",
   "CoordinateMismatch",
   "CitedSignerMismatch",
   "SignatureOwnerMismatch"}

VARIABLES stage,
          outstandingRequest,
          requestLive,
          candidateScheduled,
          responseRejected,
          lastAttempt

vars ==
  <<stage,
    outstandingRequest,
    requestLive,
    candidateScheduled,
    responseRejected,
    lastAttempt>>

Init ==
  /\ stage = "ResponseReady"
  /\ outstandingRequest = CertifiedRequest
  /\ requestLive = TRUE
  /\ candidateScheduled = FALSE
  /\ responseRejected = FALSE
  /\ lastAttempt = "None"

ProcessResponse(response, attemptKind) ==
  /\ stage = "ResponseReady"
  /\ requestLive
  /\ stage' = "Processed"
  /\ lastAttempt' = attemptKind
  /\ IF ResponseAuthorized(response)
     THEN /\ outstandingRequest' = NoOutstandingRequest
          /\ requestLive' = FALSE
          /\ candidateScheduled' = TRUE
          /\ responseRejected' = FALSE
     ELSE /\ outstandingRequest' = outstandingRequest
          /\ requestLive' = TRUE
          /\ candidateScheduled' = FALSE
          /\ responseRejected' = TRUE

ProcessHonestResponse ==
  ProcessResponse(HonestResponse, "Honest")

ProcessRequestHashMismatch ==
  ProcessResponse(RequestHashMismatchResponse, "RequestHashMismatch")

ProcessCoordinateMismatch ==
  ProcessResponse(CoordinateMismatchResponse, "CoordinateMismatch")

ProcessCitedSignerMismatch ==
  ProcessResponse(CitedSignerMismatchResponse, "CitedSignerMismatch")

ProcessSignatureOwnerMismatch ==
  ProcessResponse(RelaySignedResponse, "SignatureOwnerMismatch")

Quiescent ==
  /\ stage = "Processed"
  /\ UNCHANGED vars

Next ==
  \/ ProcessHonestResponse
  \/ ProcessRequestHashMismatch
  \/ ProcessCoordinateMismatch
  \/ ProcessCitedSignerMismatch
  \/ ProcessSignatureOwnerMismatch
  \/ Quiescent

Spec ==
  /\ Init
  /\ [][Next]_vars

TypeInvariant ==
  /\ stage \in {"ResponseReady", "Processed"}
  /\ outstandingRequest \in {CertifiedRequest, NoOutstandingRequest}
  /\ requestLive \in BOOLEAN
  /\ candidateScheduled \in BOOLEAN
  /\ responseRejected \in BOOLEAN
  /\ lastAttempt \in AttemptKinds
  /\ requestLive = (outstandingRequest = CertifiedRequest)

AuthorityIdentitiesAreDistinct ==
  /\ UntrustedRelay # RotatedArchive
  /\ UntrustedRelay # FrozenSigner
  /\ UntrustedRelay # OriginalRouteTarget
  /\ RotatedArchive # FrozenSigner
  /\ RotatedArchive # OriginalRouteTarget
  /\ FrozenSigner # OriginalRouteTarget
  /\ RotatedArchive \notin CommitQc.signers
  /\ UntrustedRelay \notin CommitQc.signers
  /\ FrozenSigner \in CommitQc.signers \ {Requester}
  /\ CertifiedRequest.routeTarget = OriginalRouteTarget
  /\ OriginalRouteTarget \in CurrentVotingRoster
  /\ RotatedArchive \notin CurrentVotingRoster
  /\ CurrentVotingPower(RotatedArchive) = 0

HonestResponseShape ==
  /\ ExactResponseCoordinates(HonestResponse)
  /\ HonestResponse.via = UntrustedRelay
  /\ HonestResponse.archiveServer = RotatedArchive
  /\ HonestResponse.citedResponder = FrozenSigner
  /\ ArchiveSignatureAuthenticated(HonestResponse)
  /\ HonestResponse.archiveServer # CertifiedRequest.routeTarget

ExactNegativeControlsRejected ==
  /\ ~SeparatedResponseAuthorized(RequestHashMismatchResponse)
  /\ ~SeparatedResponseAuthorized(CoordinateMismatchResponse)
  /\ ~SeparatedResponseAuthorized(CitedSignerMismatchResponse)
  /\ ~SeparatedResponseAuthorized(RelaySignedResponse)

ExactRequestRecoveryOwnerRetained ==
  stage = "ResponseReady"
  \/ outstandingRequest = CertifiedRequest
  \/ candidateScheduled

RejectedResponseRetainsExactRequest ==
  responseRejected =>
    /\ outstandingRequest = CertifiedRequest
    /\ requestLive
    /\ ~candidateScheduled

ValidRotatedArchiveResponseAccepted ==
  stage = "ResponseReady"
  \/ lastAttempt # "Honest"
  \/ candidateScheduled

=============================================================================

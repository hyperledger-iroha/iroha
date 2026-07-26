---- MODULE SumeragiV2CertifiedResponseRegistrationMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Bounded mutation model for exact certified-response registration.

One exact signed certified-body request, including its full CommitQC, is
fanned out to current archive servers.  Physical routes are not part of its
hash, so all fan-out occurrences share one exact-hash registration class.  The
repaired authorization requires that exact class to remain outstanding when a
response drains.  The mutation omits that guard, allowing either a second
fan-out response after the first response retired the request or a delayed
response after restart replay reset removed volatile request ownership.

Commit-certificate discovery has the same route-alias shape under its own
request identity: every addressed archive server is one transport alias for
the requester/height registration.  The CommitFanout scenario makes that
shape explicit.  Its repair retires every matching alias when the first valid
Commit-certificate response is accepted, so the second valid response is
unmatched.  Its route-only mutant retires just the embedded request alias and
therefore leaves the second route incorrectly live.

The Historical scenario supplies a positive catch-up trace: exact CommitQC
discovery installs a durable Decision, the target registers a certified-body
request, an applied historical server responds, and the target applies.  It
therefore checks that the repair rejects only unsolicited/replayed responses,
not the intended historical recovery corridor.
***************************************************************************)

CONSTANTS RequireMatchingCertifiedRequest,
          RetireAllCommitRouteAliases,
          Scenario

ASSUME RequireMatchingCertifiedRequest \in BOOLEAN
ASSUME RetireAllCommitRouteAliases \in BOOLEAN
ASSUME Scenario \in {"Duplicate", "Restart", "Historical", "CommitFanout"}

Requester == "Requester"
ArchiveA == "ArchiveA"
ArchiveB == "ArchiveB"
ArchiveServers == {ArchiveA, ArchiveB}
FrozenSigner == "FrozenSigner"
OtherFrozenSigner == "OtherFrozenSigner"
Relay == "Relay"
CommitCertificate == "CommitQC-7"

CertifiedQc ==
  [height |-> 7,
   view |-> 4,
   phase |-> "Commit",
   subject |-> "subject-7",
   signers |-> {Requester, FrozenSigner, OtherFrozenSigner}]

CertifiedRequestPreimage ==
  [round |-> [height |-> 7, view |-> 4],
   subject |-> "subject-7",
   certificate |-> CertifiedQc,
   requester |-> Requester]

CertifiedRequestSignature ==
  [signer |-> Requester, preimage |-> CertifiedRequestPreimage]

CertifiedRequestHash ==
  [exactSignedRequest |->
    [preimage |-> CertifiedRequestPreimage,
     signature |-> CertifiedRequestSignature]]

CertifiedRequest(route) ==
  [kind |-> "CertifiedRequest",
   source |-> Requester,
   requester |-> Requester,
   recipient |-> route,
   height |-> 7,
   view |-> 4,
   subject |-> "subject-7",
   certificate |-> CertifiedQc,
   requestHash |-> CertifiedRequestHash]

CertifiedResponse(archiveServer) ==
  [kind |-> "CertifiedResponse",
   source |-> Relay,
   archiveServer |-> archiveServer,
   signatureOwner |-> archiveServer,
   citedResponder |-> FrozenSigner,
   requestHash |-> CertifiedRequestHash,
   recipient |-> Requester,
   height |-> 7,
   view |-> 4,
   subject |-> "subject-7"]

CertifiedRequestOutbox ==
  {CertifiedRequest(server): server \in ArchiveServers}

CertifiedResponses ==
  {CertifiedResponse(server): server \in ArchiveServers}

CommitCertificateRequest(server) ==
  [kind |-> "CommitCertificateRequest",
   source |-> Requester,
   recipient |-> server,
   height |-> 7]

CommitCertificateResponse(server) ==
  [kind |-> "CommitCertificateResponse",
   source |-> server,
   recipient |-> Requester,
   request |-> CommitCertificateRequest(server),
   certificate |-> CommitCertificate]

CommitCertificateRequestOutbox ==
  {CommitCertificateRequest(server): server \in ArchiveServers}

CommitCertificateResponses ==
  {CommitCertificateResponse(server): server \in ArchiveServers}

RequestOutbox ==
  CertifiedRequestOutbox \cup CommitCertificateRequestOutbox

ResponseItems ==
  CertifiedResponses \cup CommitCertificateResponses

VARIABLES phase,
          generation,
          decisionInstalled,
          commitRequestOutstanding,
          commitResponseAvailable,
          activeRequests,
          responseQueue,
          requestRegistrations,
          acceptedResponses,
          droppedResponses,
          completionOwned,
          acceptedWithoutOutstanding,
          applied

vars ==
  <<phase,
    generation,
    decisionInstalled,
    commitRequestOutstanding,
    commitResponseAvailable,
    activeRequests,
    responseQueue,
    requestRegistrations,
    acceptedResponses,
    droppedResponses,
    completionOwned,
    acceptedWithoutOutstanding,
    applied>>

Phases ==
  {"Start", "DuplicateRequest", "DuplicateResponses", "DuplicateSecond",
   "CommitFanoutRequest", "CommitFanoutResponses", "CommitFanoutSecond",
   "RestartRunning", "RestartResponseInFlight", "RestartRequired",
   "ReplayRequired", "RecoveredFetchPending", "RestartReissuePending",
   "RestartRequest", "RestartFreshResponse", "RestartApply",
   "HistoricalCommitRequest", "HistoricalCommitResponse",
   "HistoricalDecision", "HistoricalRequest", "HistoricalResponse",
   "HistoricalApply", "Done"}

MatchingCertifiedRequests(response) ==
  {request \in activeRequests:
     /\ request.kind = "CertifiedRequest"
     /\ request.requestHash = response.requestHash}

CertifiedResponseAuthorized(response) ==
  /\ response.kind = "CertifiedResponse"
  /\ decisionInstalled
  /\ response.archiveServer \in ArchiveServers
  /\ response.signatureOwner = response.archiveServer
  /\ response.citedResponder \in CertifiedQc.signers
  /\ IF RequireMatchingCertifiedRequest
     THEN MatchingCertifiedRequests(response) # {}
     ELSE TRUE

CommitCertificateRequestRegistrationIdentity(request) ==
  [requester |-> request.source,
   height |-> request.height]

MatchingCommitCertificateRequests(response) ==
  {request \in activeRequests:
     /\ request.kind = "CommitCertificateRequest"
     /\ CommitCertificateRequestRegistrationIdentity(request)
          = CommitCertificateRequestRegistrationIdentity(response.request)}

CommitCertificateResponseAuthorized(response) ==
  /\ response.kind = "CommitCertificateResponse"
  /\ response.source \in ArchiveServers
  /\ response.recipient = Requester
  /\ response.request \in activeRequests
  /\ response.certificate = CommitCertificate
  /\ MatchingCommitCertificateRequests(response) # {}

TypeInvariant ==
  /\ phase \in Phases
  /\ generation \in Nat
  /\ decisionInstalled \in BOOLEAN
  /\ commitRequestOutstanding \in BOOLEAN
  /\ commitResponseAvailable \in BOOLEAN
  /\ activeRequests \subseteq RequestOutbox
  /\ responseQueue \in Seq(ResponseItems)
  /\ requestRegistrations \in Nat
  /\ acceptedResponses \in Nat
  /\ droppedResponses \in Nat
  /\ completionOwned \in BOOLEAN
  /\ acceptedWithoutOutstanding \in BOOLEAN
  /\ applied \in BOOLEAN

ExactRequestFanout ==
  \/ activeRequests = {}
  \/ activeRequests = CertifiedRequestOutbox
  \/ activeRequests = CommitCertificateRequestOutbox

AcceptedOnlyWhileOutstanding ==
  ~acceptedWithoutOutstanding

AcceptedResponsesBoundedByRegistrations ==
  acceptedResponses <= requestRegistrations

CompletionHasDecisionAuthority ==
  completionOwned => decisionInstalled

ApplicationHasExactCompletion ==
  applied => /\ decisionInstalled
             /\ completionOwned
             /\ activeRequests = {}

CommitFanoutFirstAcceptedResponseRetiresAllRouteAliases ==
  (Scenario = "CommitFanout" /\ phase = "CommitFanoutSecond")
    => /\ acceptedResponses = 1
       /\ decisionInstalled
       /\ activeRequests = {}

CommitFanoutSecondResponseIsUnmatched ==
  (Scenario = "CommitFanout" /\ phase = "CommitFanoutSecond")
    => /\ responseQueue = <<CommitCertificateResponse(ArchiveB)>>
       /\ MatchingCommitCertificateRequests(Head(responseQueue)) = {}

Init ==
  /\ phase = "Start"
  /\ generation = 0
  /\ decisionInstalled = (Scenario \in {"Duplicate", "Restart"})
  /\ commitRequestOutstanding = FALSE
  /\ commitResponseAvailable = FALSE
  /\ activeRequests = {}
  /\ responseQueue = <<>>
  /\ requestRegistrations = 0
  /\ acceptedResponses = 0
  /\ droppedResponses = 0
  /\ completionOwned = FALSE
  /\ acceptedWithoutOutstanding = FALSE
  /\ applied = FALSE

RegisterCertifiedRequests(nextPhase) ==
  /\ decisionInstalled
  /\ activeRequests = {}
  /\ responseQueue = <<>>
  /\ phase' = nextPhase
  /\ activeRequests' = CertifiedRequestOutbox
  /\ requestRegistrations' = requestRegistrations + 1
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  responseQueue, acceptedResponses, droppedResponses,
                  completionOwned, acceptedWithoutOutstanding, applied>>

ServeCertifiedResponses(responses, nextPhase) ==
  /\ activeRequests = CertifiedRequestOutbox
  /\ responseQueue = <<>>
  /\ responses \in Seq(CertifiedResponses)
  /\ responses # <<>>
  /\ phase' = nextPhase
  /\ responseQueue' = responses
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  activeRequests, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding, applied>>

DrainCertifiedResponse(nextPhase) ==
  LET response == Head(responseQueue)
      matching == MatchingCertifiedRequests(response)
      authorized == CertifiedResponseAuthorized(response)
  IN /\ responseQueue # <<>>
     /\ phase' = nextPhase
     /\ responseQueue' = Tail(responseQueue)
     /\ IF authorized
        THEN /\ acceptedResponses' = acceptedResponses + 1
             /\ droppedResponses' = droppedResponses
             /\ completionOwned' = TRUE
             /\ acceptedWithoutOutstanding' =
                  (acceptedWithoutOutstanding \/ (matching = {}))
             /\ activeRequests' = activeRequests \ matching
        ELSE /\ acceptedResponses' = acceptedResponses
             /\ droppedResponses' = droppedResponses + 1
             /\ completionOwned' = completionOwned
             /\ acceptedWithoutOutstanding' =
                  acceptedWithoutOutstanding
             /\ activeRequests' = activeRequests
     /\ UNCHANGED <<generation, decisionInstalled,
                     commitRequestOutstanding, commitResponseAvailable,
                     requestRegistrations, applied>>

DrainCommitCertificateResponse(nextPhase) ==
  LET response == Head(responseQueue)
      matching == MatchingCommitCertificateRequests(response)
      authorized == CommitCertificateResponseAuthorized(response)
      retired ==
        IF RetireAllCommitRouteAliases THEN matching ELSE {response.request}
  IN /\ responseQueue # <<>>
     /\ phase' = nextPhase
     /\ responseQueue' = Tail(responseQueue)
     /\ IF authorized
        THEN /\ acceptedResponses' = acceptedResponses + 1
             /\ droppedResponses' = droppedResponses
             /\ decisionInstalled' = TRUE
             /\ acceptedWithoutOutstanding' =
                  (acceptedWithoutOutstanding \/ (matching = {}))
             /\ activeRequests' = activeRequests \ retired
        ELSE /\ acceptedResponses' = acceptedResponses
             /\ droppedResponses' = droppedResponses + 1
             /\ decisionInstalled' = decisionInstalled
             /\ acceptedWithoutOutstanding' = acceptedWithoutOutstanding
             /\ activeRequests' = activeRequests
     /\ UNCHANGED <<generation, commitRequestOutstanding,
                     commitResponseAvailable, requestRegistrations,
                     completionOwned, applied>>

(***************************************************************************
Duplicate fan-out response scenario.
***************************************************************************)

OpenDuplicateRequest ==
  /\ Scenario = "Duplicate"
  /\ phase = "Start"
  /\ RegisterCertifiedRequests("DuplicateRequest")

ServeDuplicateResponses ==
  /\ Scenario = "Duplicate"
  /\ phase = "DuplicateRequest"
  /\ ServeCertifiedResponses(
       <<CertifiedResponse(ArchiveA), CertifiedResponse(ArchiveB)>>,
       "DuplicateResponses")

DrainDuplicateFirst ==
  /\ Scenario = "Duplicate"
  /\ phase = "DuplicateResponses"
  /\ DrainCertifiedResponse("DuplicateSecond")

DrainDuplicateSecond ==
  /\ Scenario = "Duplicate"
  /\ phase = "DuplicateSecond"
  /\ DrainCertifiedResponse("Done")

(***************************************************************************
Commit-certificate route-alias fan-out scenario.  Both transport requests
share one requester/height registration.  The first response is accepted and
must retire both aliases; the second response is then valid but unmatched and
is dropped.
***************************************************************************)

OpenCommitCertificateFanout ==
  /\ Scenario = "CommitFanout"
  /\ phase = "Start"
  /\ ~decisionInstalled
  /\ activeRequests = {}
  /\ responseQueue = <<>>
  /\ phase' = "CommitFanoutRequest"
  /\ activeRequests' = CommitCertificateRequestOutbox
  /\ requestRegistrations' = requestRegistrations + 1
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  responseQueue, acceptedResponses, droppedResponses,
                  completionOwned, acceptedWithoutOutstanding, applied>>

ServeCommitCertificateFanout ==
  /\ Scenario = "CommitFanout"
  /\ phase = "CommitFanoutRequest"
  /\ activeRequests = CommitCertificateRequestOutbox
  /\ responseQueue = <<>>
  /\ phase' = "CommitFanoutResponses"
  /\ responseQueue' =
       <<CommitCertificateResponse(ArchiveA),
         CommitCertificateResponse(ArchiveB)>>
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  activeRequests, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding, applied>>

DrainCommitCertificateFanoutFirst ==
  /\ Scenario = "CommitFanout"
  /\ phase = "CommitFanoutResponses"
  /\ DrainCommitCertificateResponse("CommitFanoutSecond")

DrainCommitCertificateFanoutSecond ==
  /\ Scenario = "CommitFanout"
  /\ phase = "CommitFanoutSecond"
  /\ DrainCommitCertificateResponse("Done")

(***************************************************************************
Crash/restart scenario.  Crash and authenticated restart preserve scheduler
state.  Replay reset clears the request while retaining the delayed transport
response; after quarantine ends, the delayed response can drain before the
replayed FetchBody frontier republishes its request.
***************************************************************************)

OpenRestartRequest ==
  /\ Scenario = "Restart"
  /\ phase = "Start"
  /\ RegisterCertifiedRequests("RestartRunning")

EmitRestartDelayedResponse ==
  /\ Scenario = "Restart"
  /\ phase = "RestartRunning"
  /\ ServeCertifiedResponses(
       <<CertifiedResponse(ArchiveA)>>, "RestartResponseInFlight")

CrashRestartRequester ==
  /\ Scenario = "Restart"
  /\ phase = "RestartResponseInFlight"
  /\ phase' = "RestartRequired"
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  activeRequests, responseQueue, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding, applied>>

AuthenticatedRestart ==
  /\ Scenario = "Restart"
  /\ phase = "RestartRequired"
  /\ phase' = "ReplayRequired"
  /\ generation' = generation + 1
  /\ UNCHANGED <<decisionInstalled, commitRequestOutstanding,
                  commitResponseAvailable, activeRequests, responseQueue,
                  requestRegistrations, acceptedResponses, droppedResponses,
                  completionOwned, acceptedWithoutOutstanding, applied>>

ReplayReset ==
  /\ Scenario = "Restart"
  /\ phase = "ReplayRequired"
  /\ activeRequests = CertifiedRequestOutbox
  /\ phase' = "RecoveredFetchPending"
  /\ activeRequests' = {}
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  responseQueue, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding, applied>>

DrainRestartDelayedResponse ==
  /\ Scenario = "Restart"
  /\ phase = "RecoveredFetchPending"
  /\ DrainCertifiedResponse("RestartReissuePending")

RegisterRestartReplayRequest ==
  /\ Scenario = "Restart"
  /\ phase = "RestartReissuePending"
  /\ RegisterCertifiedRequests("RestartRequest")

ServeRestartFreshResponse ==
  /\ Scenario = "Restart"
  /\ phase = "RestartRequest"
  /\ ServeCertifiedResponses(
       <<CertifiedResponse(ArchiveB)>>, "RestartFreshResponse")

DrainRestartFreshResponse ==
  /\ Scenario = "Restart"
  /\ phase = "RestartFreshResponse"
  /\ DrainCertifiedResponse("RestartApply")

ApplyRestartDecision ==
  /\ Scenario = "Restart"
  /\ phase = "RestartApply"
  /\ completionOwned
  /\ activeRequests = {}
  /\ phase' = "Done"
  /\ applied' = TRUE
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  activeRequests, responseQueue, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding>>

(***************************************************************************
Positive historical catch-up scenario.
***************************************************************************)

OpenHistoricalRecovery ==
  /\ Scenario = "Historical"
  /\ phase = "Start"
  /\ phase' = "HistoricalCommitRequest"
  /\ commitRequestOutstanding' = TRUE
  /\ UNCHANGED <<generation, decisionInstalled, commitResponseAvailable,
                  activeRequests, responseQueue, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding, applied>>

ServeHistoricalCommitCertificate ==
  /\ Scenario = "Historical"
  /\ phase = "HistoricalCommitRequest"
  /\ commitRequestOutstanding
  /\ phase' = "HistoricalCommitResponse"
  /\ commitResponseAvailable' = TRUE
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, activeRequests, responseQueue,
                  requestRegistrations, acceptedResponses, droppedResponses,
                  completionOwned, acceptedWithoutOutstanding, applied>>

ImportHistoricalCommitCertificate ==
  /\ Scenario = "Historical"
  /\ phase = "HistoricalCommitResponse"
  /\ commitRequestOutstanding
  /\ commitResponseAvailable
  /\ phase' = "HistoricalDecision"
  /\ decisionInstalled' = TRUE
  /\ commitRequestOutstanding' = FALSE
  /\ commitResponseAvailable' = FALSE
  /\ UNCHANGED <<generation, activeRequests, responseQueue,
                  requestRegistrations, acceptedResponses, droppedResponses,
                  completionOwned, acceptedWithoutOutstanding, applied>>

RegisterHistoricalCertifiedRequest ==
  /\ Scenario = "Historical"
  /\ phase = "HistoricalDecision"
  /\ RegisterCertifiedRequests("HistoricalRequest")

ServeHistoricalCertifiedResponse ==
  /\ Scenario = "Historical"
  /\ phase = "HistoricalRequest"
  /\ ServeCertifiedResponses(
       <<CertifiedResponse(ArchiveA)>>, "HistoricalResponse")

DrainHistoricalCertifiedResponse ==
  /\ Scenario = "Historical"
  /\ phase = "HistoricalResponse"
  /\ DrainCertifiedResponse("HistoricalApply")

ApplyHistoricalDecision ==
  /\ Scenario = "Historical"
  /\ phase = "HistoricalApply"
  /\ completionOwned
  /\ activeRequests = {}
  /\ phase' = "Done"
  /\ applied' = TRUE
  /\ UNCHANGED <<generation, decisionInstalled,
                  commitRequestOutstanding, commitResponseAvailable,
                  activeRequests, responseQueue, requestRegistrations,
                  acceptedResponses, droppedResponses, completionOwned,
                  acceptedWithoutOutstanding>>

Progress ==
  \/ OpenDuplicateRequest
  \/ ServeDuplicateResponses
  \/ DrainDuplicateFirst
  \/ DrainDuplicateSecond
  \/ OpenCommitCertificateFanout
  \/ ServeCommitCertificateFanout
  \/ DrainCommitCertificateFanoutFirst
  \/ DrainCommitCertificateFanoutSecond
  \/ OpenRestartRequest
  \/ EmitRestartDelayedResponse
  \/ CrashRestartRequester
  \/ AuthenticatedRestart
  \/ ReplayReset
  \/ DrainRestartDelayedResponse
  \/ RegisterRestartReplayRequest
  \/ ServeRestartFreshResponse
  \/ DrainRestartFreshResponse
  \/ ApplyRestartDecision
  \/ OpenHistoricalRecovery
  \/ ServeHistoricalCommitCertificate
  \/ ImportHistoricalCommitCertificate
  \/ RegisterHistoricalCertifiedRequest
  \/ ServeHistoricalCertifiedResponse
  \/ DrainHistoricalCertifiedResponse
  \/ ApplyHistoricalDecision

Next == Progress

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Progress)

ScenarioCompletes == <>(phase = "Done")

====

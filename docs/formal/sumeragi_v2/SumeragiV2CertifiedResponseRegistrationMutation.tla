---- MODULE SumeragiV2CertifiedResponseRegistrationMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Bounded mutation model for exact certified-response registration.

One signed certified-body request is fanned out to every CommitQC signer, but
all fan-out occurrences share one logical request registration.  The repaired
authorization requires that logical registration to remain outstanding when a
response drains.  The mutation omits that guard, allowing either a second
fan-out response after the first response retired the request or a delayed
response after restart replay reset removed volatile request ownership.

The Historical scenario supplies a positive catch-up trace: exact CommitQC
discovery installs a durable Decision, the target registers a certified-body
request, an applied historical server responds, and the target applies.  It
therefore checks that the repair rejects only unsolicited/replayed responses,
not the intended historical recovery corridor.
***************************************************************************)

CONSTANTS RequireMatchingCertifiedRequest, Scenario

ASSUME RequireMatchingCertifiedRequest \in BOOLEAN
ASSUME Scenario \in {"Duplicate", "Restart", "Historical"}

Requester == "Requester"
SignerA == "SignerA"
SignerB == "SignerB"
CertifiedSigners == {SignerA, SignerB}

CertifiedRequest(recipient) ==
  [kind |-> "CertifiedRequest",
   source |-> Requester,
   recipient |-> recipient,
   height |-> 7,
   view |-> 4,
   subject |-> "subject-7"]

CertifiedResponse(source) ==
  [kind |-> "CertifiedResponse",
   source |-> source,
   recipient |-> Requester,
   height |-> 7,
   view |-> 4,
   subject |-> "subject-7"]

CertifiedRequestOutbox ==
  {CertifiedRequest(signer): signer \in CertifiedSigners}

CertifiedResponses ==
  {CertifiedResponse(signer): signer \in CertifiedSigners}

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
   "RestartRunning", "RestartResponseInFlight", "RestartRequired",
   "ReplayRequired", "RecoveredFetchPending", "RestartReissuePending",
   "RestartRequest", "RestartFreshResponse", "RestartApply",
   "HistoricalCommitRequest", "HistoricalCommitResponse",
   "HistoricalDecision", "HistoricalRequest", "HistoricalResponse",
   "HistoricalApply", "Done"}

MatchingCertifiedRequests(response) ==
  {request \in activeRequests:
     /\ request.kind = "CertifiedRequest"
     /\ request.source = response.recipient
     /\ request.height = response.height
     /\ request.view = response.view
     /\ request.subject = response.subject}

CertifiedResponseAuthorized(response) ==
  /\ response.kind = "CertifiedResponse"
  /\ decisionInstalled
  /\ response.source \in CertifiedSigners
  /\ IF RequireMatchingCertifiedRequest
     THEN MatchingCertifiedRequests(response) # {}
     ELSE TRUE

TypeInvariant ==
  /\ phase \in Phases
  /\ generation \in Nat
  /\ decisionInstalled \in BOOLEAN
  /\ commitRequestOutstanding \in BOOLEAN
  /\ commitResponseAvailable \in BOOLEAN
  /\ activeRequests \subseteq CertifiedRequestOutbox
  /\ responseQueue \in Seq(CertifiedResponses)
  /\ requestRegistrations \in Nat
  /\ acceptedResponses \in Nat
  /\ droppedResponses \in Nat
  /\ completionOwned \in BOOLEAN
  /\ acceptedWithoutOutstanding \in BOOLEAN
  /\ applied \in BOOLEAN

ExactRequestFanout ==
  activeRequests = {} \/ activeRequests = CertifiedRequestOutbox

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

Init ==
  /\ phase = "Start"
  /\ generation = 0
  /\ decisionInstalled = (Scenario # "Historical")
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
       <<CertifiedResponse(SignerA), CertifiedResponse(SignerB)>>,
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
       <<CertifiedResponse(SignerA)>>, "RestartResponseInFlight")

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
       <<CertifiedResponse(SignerB)>>, "RestartFreshResponse")

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
       <<CertifiedResponse(SignerA)>>, "HistoricalResponse")

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

---- MODULE SumeragiV2DecisionRecoveryLifecycleMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Bounded mutation model for one exact durable Decision recovery lifecycle.

The model deliberately separates three identities which production keeps
separate: the durable Commit Decision, one generation-free logical certified
request registration, and its recipient-specific wire fan-out.  A responsive
crash and authenticated restart preserve the logical registration while the
executor generation advances.  Replay retires that old registration and
installs exactly one current-generation FetchBody candidate.

Each non-Fixed mode changes one seam.  The corresponding configuration checks
the named invariant which rejects that mutation.
***************************************************************************)

CONSTANT Mutation

Mutations ==
  {"Fixed", "DuplicateDecision", "GenerationInRegistration",
   "RecipientSplitRegistration", "RetainRegistrationOnReplay",
   "DropFetchBody", "StaleExecutorGeneration", "PrepareAuthority",
   "NonSingletonReplay"}

ASSUME Mutation \in Mutations

RecoveringNode == "ValidatorA"
SignerA == "SignerA"
SignerB == "SignerB"
CertifiedRecipients == {SignerA, SignerB}

CurrentContext == [height |-> 7, epoch |-> 2]

Certificate(phase, view) ==
  [context |-> CurrentContext,
   phase |-> phase,
   view |-> view,
   subject |-> "subject-7"]

CommitQC == Certificate("Commit", 4)
DuplicateCommitQC == Certificate("Commit", 5)
PrepareQC == Certificate("Prepare", 4)
NoQC == Certificate("None", 0)

Decision(qc) == [node |-> RecoveringNode, qc |-> qc]

CertifiedRequest(recipient) ==
  [kind |-> "CertifiedRequest",
   source |-> RecoveringNode,
   recipient |-> recipient,
   height |-> CurrentContext.height,
   view |-> CommitQC.view,
   subject |-> CommitQC.subject]

CertifiedRequestFanout ==
  {CertifiedRequest(recipient): recipient \in CertifiedRecipients}

ExactLogicalRegistration ==
  [source |-> RecoveringNode,
   height |-> CurrentContext.height,
   view |-> CommitQC.view,
   subject |-> CommitQC.subject]

GenerationScopedLogicalRegistration(currentGeneration) ==
  [source |-> RecoveringNode,
   height |-> CurrentContext.height,
   view |-> CommitQC.view,
   subject |-> CommitQC.subject,
   generation |-> currentGeneration]

GenerationScopedLogicalRegistrations ==
  {GenerationScopedLogicalRegistration(value): value \in 0..1}

RecipientScopedLogicalRegistration(recipient) ==
  [source |-> RecoveringNode,
   height |-> CurrentContext.height,
   view |-> CommitQC.view,
   subject |-> CommitQC.subject,
   recipient |-> recipient]

RecipientSplitLogicalRegistrations ==
  {RecipientScopedLogicalRegistration(recipient):
     recipient \in CertifiedRecipients}

NoAuthority == [node |-> "NoNode", qc |-> NoQC]

DecisionAuthority(qc) == [node |-> RecoveringNode, qc |-> qc]

FetchBodyCandidate(consumerGeneration, qc) ==
  [class |-> "Completion",
   kind |-> "FetchBody",
   node |-> RecoveringNode,
   consumerContext |-> CurrentContext,
   consumerView |-> qc.view,
   consumerGeneration |-> consumerGeneration,
   evidence |-> qc]

Phases ==
  {"PersistReady", "RegisterReady", "Running", "RestartRequired",
   "ReplayRequired", "Recovered", "Done"}

VARIABLES phase,
          up,
          generation,
          decisions,
          logicalRegistrations,
          wireRequests,
          crashRegistrations,
          authority,
          executorGeneration,
          replayQueue,
          completed

vars ==
  <<phase, up, generation, decisions, logicalRegistrations, wireRequests,
    crashRegistrations, authority, executorGeneration, replayQueue, completed>>

Init ==
  /\ phase = "PersistReady"
  /\ up = TRUE
  /\ generation = 0
  /\ decisions = {}
  /\ logicalRegistrations = {}
  /\ wireRequests = {}
  /\ crashRegistrations = {}
  /\ authority = NoAuthority
  /\ executorGeneration = 0
  /\ replayQueue = <<>>
  /\ completed = FALSE

PersistDurableDecision ==
  /\ phase = "PersistReady"
  /\ phase' = "RegisterReady"
  /\ decisions' =
       IF Mutation = "DuplicateDecision"
       THEN {Decision(CommitQC), Decision(DuplicateCommitQC)}
       ELSE {Decision(CommitQC)}
  /\ UNCHANGED <<up, generation, logicalRegistrations, wireRequests,
                  crashRegistrations, authority, executorGeneration,
                  replayQueue, completed>>

RegisterCertifiedRequest ==
  /\ phase = "RegisterReady"
  /\ phase' = "Running"
  /\ logicalRegistrations' =
       CASE Mutation = "GenerationInRegistration" ->
              {GenerationScopedLogicalRegistration(generation)}
         [] Mutation = "RecipientSplitRegistration" ->
              RecipientSplitLogicalRegistrations
         [] OTHER -> {ExactLogicalRegistration}
  /\ wireRequests' = CertifiedRequestFanout
  /\ UNCHANGED <<up, generation, decisions, crashRegistrations,
                  authority, executorGeneration, replayQueue, completed>>

ResponsiveCrash ==
  /\ phase = "Running"
  /\ up
  /\ Decision(CommitQC) \in decisions
  /\ phase' = "RestartRequired"
  /\ up' = FALSE
  /\ crashRegistrations' = logicalRegistrations
  /\ authority' =
       IF Mutation = "PrepareAuthority"
       THEN DecisionAuthority(PrepareQC)
       ELSE DecisionAuthority(CommitQC)
  /\ executorGeneration' = generation
  /\ UNCHANGED <<generation, decisions, logicalRegistrations, wireRequests,
                  replayQueue, completed>>

AuthenticatedRestart ==
  /\ phase = "RestartRequired"
  /\ ~up
  /\ phase' = "ReplayRequired"
  /\ up' = TRUE
  /\ generation' = generation + 1
  /\ authority' = authority
  /\ executorGeneration' =
       IF Mutation = "StaleExecutorGeneration"
       THEN executorGeneration
       ELSE generation + 1
  /\ UNCHANGED <<decisions, logicalRegistrations, wireRequests,
                  crashRegistrations, replayQueue, completed>>

ReplayExactDecision ==
  /\ phase = "ReplayRequired"
  /\ up
  /\ phase' = "Recovered"
  /\ logicalRegistrations' =
       IF Mutation = "RetainRegistrationOnReplay"
       THEN logicalRegistrations
       ELSE {}
  /\ wireRequests' = {}
  /\ authority' = NoAuthority
  /\ replayQueue' =
       CASE Mutation = "DropFetchBody" -> <<>>
         [] Mutation = "NonSingletonReplay" ->
              <<FetchBodyCandidate(generation, CommitQC),
                FetchBodyCandidate(generation, CommitQC)>>
         [] OTHER -> <<FetchBodyCandidate(generation, CommitQC)>>
  /\ UNCHANGED <<up, generation, decisions, crashRegistrations,
                  executorGeneration, completed>>

CompleteFetchBody ==
  /\ phase = "Recovered"
  /\ replayQueue # <<>>
  /\ Head(replayQueue) = FetchBodyCandidate(generation, CommitQC)
  /\ phase' = "Done"
  /\ replayQueue' = Tail(replayQueue)
  /\ completed' = TRUE
  /\ UNCHANGED <<up, generation, decisions, logicalRegistrations,
                  wireRequests, crashRegistrations, authority,
                  executorGeneration>>

Next ==
  \/ PersistDurableDecision
  \/ RegisterCertifiedRequest
  \/ ResponsiveCrash
  \/ AuthenticatedRestart
  \/ ReplayExactDecision
  \/ CompleteFetchBody

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(PersistDurableDecision)
  /\ WF_vars(RegisterCertifiedRequest)
  /\ WF_vars(ResponsiveCrash)
  /\ WF_vars(AuthenticatedRestart)
  /\ WF_vars(ReplayExactDecision)
  /\ WF_vars(CompleteFetchBody)

TypeInvariant ==
  /\ phase \in Phases
  /\ up \in BOOLEAN
  /\ generation \in 0..1
  /\ decisions \subseteq
       {Decision(CommitQC), Decision(DuplicateCommitQC)}
  /\ logicalRegistrations \subseteq
       ({ExactLogicalRegistration}
         \cup GenerationScopedLogicalRegistrations
         \cup RecipientSplitLogicalRegistrations)
  /\ wireRequests \subseteq CertifiedRequestFanout
  /\ crashRegistrations \subseteq
       ({ExactLogicalRegistration}
         \cup GenerationScopedLogicalRegistrations
         \cup RecipientSplitLogicalRegistrations)
  /\ authority \in
       ({NoAuthority}
         \cup {DecisionAuthority(qc): qc \in {CommitQC, PrepareQC}})
  /\ executorGeneration \in 0..1
  /\ replayQueue \in
       Seq({FetchBodyCandidate(value, CommitQC): value \in 0..1})
  /\ Len(replayQueue) <= 2
  /\ completed \in BOOLEAN

DecisionsUniqueByNodeContext ==
  \A left, right \in decisions:
    /\ left.node = right.node
    /\ left.qc.context = right.qc.context
    => left = right

LogicalRegistrationGenerationFree ==
  logicalRegistrations \cap GenerationScopedLogicalRegistrations = {}

RecipientFanoutSharesOneLogicalRegistration ==
  logicalRegistrations = {}
    \/ logicalRegistrations = {ExactLogicalRegistration}

CrashRestartPreservesLogicalRegistration ==
  phase \in {"RestartRequired", "ReplayRequired"} =>
    /\ logicalRegistrations = crashRegistrations
    /\ wireRequests = CertifiedRequestFanout

ReplayClearsLogicalRegistration ==
  phase \in {"Recovered", "Done"} =>
    /\ logicalRegistrations = {}
    /\ wireRequests = {}

RecoveryAuthorityIsExactCommitDecision ==
  phase \in {"RestartRequired", "ReplayRequired"} =>
    /\ authority.node = RecoveringNode
    /\ authority.qc = CommitQC
    /\ authority.qc.phase = "Commit"
    /\ Decision(authority.qc) \in decisions

RecoveryExecutorUsesCurrentGeneration ==
  phase \in {"RestartRequired", "ReplayRequired"} =>
    executorGeneration = generation

RecoveredHasExactFetchBody ==
  phase = "Recovered" =>
    /\ replayQueue # <<>>
    /\ Head(replayQueue) = FetchBodyCandidate(generation, CommitQC)

RecoveredReplayIsExactSingleton ==
  phase = "Recovered" =>
    replayQueue = <<FetchBodyCandidate(generation, CommitQC)>>

LifecycleCompletes == <>completed

====

------------------------- MODULE SumeragiV2Revision4 -------------------------
\* Compact executable model of the authoritative Sumeragi v2 revision-4
\* committee, routing, safety, and conditional post-GST progress contract.
\* Signatures, storage, execution, and Reed-Solomon shards are represented by
\* authenticated/durable completion steps.

EXTENDS FiniteSets, Integers, Naturals, Sequences, TLC

CONSTANTS Validators, Faulty, Bodies, NoBody, BaseOrder, LocalValidator

N == Cardinality(Validators)
F == (N - 1) \div 3
Q == 2 * F + 1
Honest == Validators \ Faulty
Views == 0..(N - 1)

At(position, currentView) ==
    BaseOrder[1 + ((position - 1 + currentView) % N)]

SetA(currentView) == {At(position, currentView) : position \in 1..Q}
SetB(currentView) == Validators \ SetA(currentView)
Leader(currentView) == At(1, currentView)
ProxyTail(currentView) == At(Q, currentView)
UsableView(currentView) ==
    /\ Leader(currentView) \in Honest
    /\ ProxyTail(currentView) \in Honest

\* A full rotation must contain a view whose leader and proxy tail are honest.
\* This follows from n = 3f + 1 and |Faulty| <= f; keeping the finite premise
\* explicit makes the bounded post-GST model fail closed if its geometry is
\* changed independently of the paper argument.
ConstantOK ==
    /\ N >= 4
    /\ N <= 31
    /\ N = 3 * F + 1
    /\ Cardinality(Faulty) <= F
    /\ Bodies /= {}
    /\ NoBody \notin Bodies
    /\ Len(BaseOrder) = N
    /\ {BaseOrder[i] : i \in 1..N} = Validators
    /\ Cardinality({BaseOrder[i] : i \in 1..N}) = N
    /\ LocalValidator \in Honest
    /\ \E candidateView \in Views : UsableView(candidateView)

VoteCount(votes, body) ==
    Cardinality({validator \in Validators : <<validator, body>> \in votes})

VoteSigners(votes) ==
    {validator \in Validators :
        \E body \in Bodies : <<validator, body>> \in votes}

RouteSources(routes) ==
    {validator \in Validators :
        \E target \in Validators : <<validator, target>> \in routes}

VARIABLES
    view,
    proposal,
    manifestTargets,
    bodyTargets,
    fullBody,
    prepareVotes,
    prepareVoteRoutes,
    commitVotes,
    commitVoteRoutes,
    preparedBody,
    lockedBody,
    honestCommittedBody,
    fallback,
    timeoutVotes,
    timeoutVoteRoutes,
    decisions,
    applied,
    finalizedOutputDebt,
    successorActive

vars ==
    <<view, proposal, manifestTargets, bodyTargets, fullBody, prepareVotes,
      prepareVoteRoutes, commitVotes, commitVoteRoutes, preparedBody,
      lockedBody, honestCommittedBody, fallback, timeoutVotes,
      timeoutVoteRoutes, decisions, applied, finalizedOutputDebt,
      successorActive>>

TypeOK ==
    /\ view \in Views
    /\ proposal \in Bodies \cup {NoBody}
    /\ manifestTargets \subseteq Validators
    /\ bodyTargets \subseteq Validators
    /\ fullBody \subseteq Validators \X Bodies
    /\ prepareVotes \subseteq Validators \X Bodies
    /\ prepareVoteRoutes \subseteq Validators \X Validators
    /\ commitVotes \subseteq Validators \X Bodies
    /\ commitVoteRoutes \subseteq Validators \X Validators
    /\ preparedBody \in Bodies \cup {NoBody}
    /\ lockedBody \in Bodies \cup {NoBody}
    /\ honestCommittedBody \in [Honest -> Bodies \cup {NoBody}]
    /\ fallback \in BOOLEAN
    /\ timeoutVotes \subseteq Validators
    /\ timeoutVoteRoutes \subseteq Validators \X Validators
    /\ decisions \subseteq Bodies
    /\ applied \in BOOLEAN
    /\ finalizedOutputDebt \in BOOLEAN
    /\ successorActive \in BOOLEAN

Init ==
    /\ ConstantOK
    /\ view = 0
    /\ proposal = NoBody
    /\ manifestTargets = {}
    /\ bodyTargets = {}
    /\ fullBody = {}
    /\ prepareVotes = {}
    /\ prepareVoteRoutes = {}
    /\ commitVotes = {}
    /\ commitVoteRoutes = {}
    /\ preparedBody = NoBody
    /\ lockedBody = NoBody
    /\ honestCommittedBody = [validator \in Honest |-> NoBody]
    /\ fallback = FALSE
    /\ timeoutVotes = {}
    /\ timeoutVoteRoutes = {}
    /\ decisions = {}
    /\ applied = FALSE
    /\ finalizedOutputDebt = FALSE
    /\ successorActive = FALSE

\* Proposal control is committee-wide. The first body/chunk occurrence targets
\* exactly Set A; a same-view retransmission expands it in EnterFallback.
Propose(body) ==
    /\ body \in Bodies
    /\ proposal = NoBody
    /\ decisions = {}
    /\ lockedBody = NoBody \/ body = lockedBody
    /\ proposal' = body
    /\ manifestTargets' = Validators
    /\ bodyTargets' = SetA(view)
    /\ UNCHANGED <<view, fullBody, prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, timeoutVotes,
                   timeoutVoteRoutes, decisions, applied, finalizedOutputDebt,
                   successorActive>>

\* This step represents successful RS16 reconstruction, hash checking, durable
\* storage, and deterministic validation of the complete canonical body.
CompleteFullBody(validator) ==
    /\ validator \in bodyTargets
    /\ proposal \in Bodies
    /\ <<validator, proposal>> \notin fullBody
    /\ fullBody' = fullBody \cup {<<validator, proposal>>}
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, prepareVotes,
                   prepareVoteRoutes, commitVotes, commitVoteRoutes,
                   preparedBody, lockedBody, honestCommittedBody, fallback,
                   timeoutVotes, timeoutVoteRoutes, decisions, applied,
                   finalizedOutputDebt, successorActive>>

EligibleVoter(validator) == validator \in bodyTargets

\* No Prepare vote exists before the signer owns the full validated body. The
\* logical route records that the current proxy tail is the only phase-vote
\* collector; local delivery is abstracted as the same logical route.
PrepareVote(validator) ==
    /\ validator \in Validators
    /\ EligibleVoter(validator)
    /\ proposal \in Bodies
    /\ <<validator, proposal>> \in fullBody
    /\ ~\E body \in Bodies : <<validator, body>> \in prepareVotes
    /\ (validator \in Honest =>
          (lockedBody = NoBody \/ proposal = lockedBody))
    /\ prepareVotes' = prepareVotes \cup {<<validator, proposal>>}
    /\ prepareVoteRoutes' =
          prepareVoteRoutes \cup {<<validator, ProxyTail(view)>>}
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   commitVotes, commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, timeoutVotes,
                   timeoutVoteRoutes, decisions, applied, finalizedOutputDebt,
                   successorActive>>

\* A faulty proxy tail may withhold this action. Conditional post-GST fairness
\* is applied only to the honest-tail wrapper below.
FormPrepareQC(body) ==
    /\ body = proposal
    /\ body \in Bodies
    /\ preparedBody = NoBody
    /\ VoteCount(prepareVotes, body) >= Q
    /\ preparedBody' = body
    /\ lockedBody' = body
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, honestCommittedBody, fallback,
                   timeoutVotes, timeoutVoteRoutes, decisions, applied,
                   finalizedOutputDebt, successorActive>>

CommitVote(validator) ==
    /\ validator \in Validators
    /\ EligibleVoter(validator)
    /\ preparedBody \in Bodies
    /\ <<validator, preparedBody>> \in fullBody
    /\ ~\E body \in Bodies : <<validator, body>> \in commitVotes
    /\ (validator \in Honest =>
          honestCommittedBody[validator] \in {NoBody, preparedBody})
    /\ commitVotes' = commitVotes \cup {<<validator, preparedBody>>}
    /\ commitVoteRoutes' =
          commitVoteRoutes \cup {<<validator, ProxyTail(view)>>}
    /\ honestCommittedBody' =
          IF validator \in Honest
          THEN [honestCommittedBody EXCEPT ![validator] = preparedBody]
          ELSE honestCommittedBody
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, preparedBody, lockedBody,
                   fallback, timeoutVotes, timeoutVoteRoutes, decisions,
                   applied, finalizedOutputDebt, successorActive>>

Decide(body) ==
    /\ body \in Bodies
    /\ body \notin decisions
    /\ VoteCount(commitVotes, body) >= Q
    /\ decisions' = decisions \cup {body}
    /\ finalizedOutputDebt' = TRUE
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, timeoutVotes,
                   timeoutVoteRoutes, applied, successorActive>>

\* The first periodic proposal retransmission expands body/chunk service from
\* Set A to the whole committee while retaining proposal, view, lock, and Q.
EnterFallback ==
    /\ ~fallback
    /\ proposal \in Bodies
    /\ decisions = {}
    /\ fallback' = TRUE
    /\ bodyTargets' = Validators
    /\ UNCHANGED <<view, proposal, manifestTargets, fullBody, prepareVotes,
                   prepareVoteRoutes, commitVotes, commitVoteRoutes,
                   preparedBody, lockedBody, honestCommittedBody, timeoutVotes,
                   timeoutVoteRoutes, decisions, applied, finalizedOutputDebt,
                   successorActive>>

\* Timeout votes use committee-wide control fanout and therefore never depend
\* on the sole proxy-tail phase-vote route.
SendTimeout(validator) ==
    /\ validator \in Validators
    /\ validator \notin timeoutVotes
    /\ decisions = {}
    /\ timeoutVotes' = timeoutVotes \cup {validator}
    /\ timeoutVoteRoutes' =
          timeoutVoteRoutes
            \cup {<<validator, target>> : target \in Validators}
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, decisions, applied,
                   finalizedOutputDebt, successorActive>>

ChangeView ==
    /\ view < N - 1
    /\ Cardinality(timeoutVotes) >= Q
    /\ decisions = {}
    /\ view' = view + 1
    /\ proposal' = NoBody
    /\ manifestTargets' = {}
    /\ bodyTargets' = {}
    /\ prepareVotes' = {}
    /\ prepareVoteRoutes' = {}
    /\ commitVotes' = {}
    /\ commitVoteRoutes' = {}
    /\ preparedBody' = NoBody
    /\ fallback' = FALSE
    /\ timeoutVotes' = {}
    /\ timeoutVoteRoutes' = {}
    /\ UNCHANGED <<fullBody, lockedBody, honestCommittedBody, decisions,
                   applied, finalizedOutputDebt, successorActive>>

\* A responsive validator that did not participate in the deciding fast path
\* can recover the exact certified body from honest QC signers before apply.
RecoverDecidedFullBody(validator, body) ==
    /\ validator \in Honest
    /\ body \in decisions
    /\ <<validator, body>> \notin fullBody
    /\ fullBody' = fullBody \cup {<<validator, body>>}
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, prepareVotes,
                   prepareVoteRoutes, commitVotes, commitVoteRoutes,
                   preparedBody, lockedBody, honestCommittedBody, fallback,
                   timeoutVotes, timeoutVoteRoutes, decisions, applied,
                   finalizedOutputDebt, successorActive>>

ApplyDecision(body) ==
    /\ body \in decisions
    /\ <<LocalValidator, body>> \in fullBody
    /\ ~applied
    /\ applied' = TRUE
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, timeoutVotes,
                   timeoutVoteRoutes, decisions, finalizedOutputDebt,
                   successorActive>>

\* Retryable old-height output repair is deliberately absent from this guard.
ActivateSuccessor ==
    /\ applied
    /\ ~successorActive
    /\ successorActive' = TRUE
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, timeoutVotes,
                   timeoutVoteRoutes, decisions, applied, finalizedOutputDebt>>

RepairFinalizedOutput ==
    /\ finalizedOutputDebt
    /\ finalizedOutputDebt' = FALSE
    /\ UNCHANGED <<view, proposal, manifestTargets, bodyTargets, fullBody,
                   prepareVotes, prepareVoteRoutes, commitVotes,
                   commitVoteRoutes, preparedBody, lockedBody,
                   honestCommittedBody, fallback, timeoutVotes,
                   timeoutVoteRoutes, decisions, applied, successorActive>>

ProgressNext ==
    \/ \E body \in Bodies : Propose(body)
    \/ \E validator \in Validators : CompleteFullBody(validator)
    \/ \E validator \in Validators : PrepareVote(validator)
    \/ \E body \in Bodies : FormPrepareQC(body)
    \/ \E validator \in Validators : CommitVote(validator)
    \/ \E body \in Bodies : Decide(body)
    \/ EnterFallback
    \/ \E validator \in Honest, body \in Bodies :
          RecoverDecidedFullBody(validator, body)
    \/ \E body \in Bodies : ApplyDecision(body)
    \/ ActivateSuccessor
    \/ RepairFinalizedOutput

Next ==
    \/ ProgressNext
    \/ \E validator \in Validators : SendTimeout(validator)
    \/ ChangeView

Spec == Init /\ [][Next]_vars

\* After GST, deadlines exceed the finite service bound in a usable view, so
\* responsive validators do not certify departure while both critical roles
\* are honest. Other views retain the ordinary timeout/view-change path.
PostGSTSendTimeout(validator) ==
    /\ ~UsableView(view)
    /\ SendTimeout(validator)

PostGSTNext ==
    \/ ProgressNext
    \/ \E validator \in Validators : PostGSTSendTimeout(validator)
    \/ ChangeView

HonestLeaderProposes ==
    /\ Leader(view) \in Honest
    /\ \E body \in Bodies : Propose(body)

HonestBodyService ==
    \E validator \in Honest : CompleteFullBody(validator)

HonestPrepareService ==
    \E validator \in Honest : PrepareVote(validator)

HonestTailPrepareQCService ==
    /\ ProxyTail(view) \in Honest
    /\ \E body \in Bodies : FormPrepareQC(body)

HonestCommitService ==
    \E validator \in Honest : CommitVote(validator)

HonestTailDecisionService ==
    /\ ProxyTail(view) \in Honest
    /\ \E body \in Bodies : Decide(body)

HonestTimeoutService ==
    \E validator \in Honest : PostGSTSendTimeout(validator)

LocalDecisionBodyRecovery ==
    \E body \in Bodies : RecoverDecidedFullBody(LocalValidator, body)

LocalDecisionApplication ==
    \E body \in Bodies : ApplyDecision(body)

\* These weak-fairness clauses are the compact model's explicit partial-
\* synchrony assumptions: honest leader/timer/network/body/vote/QC/application
\* services and the serialized successor activation runner are eventually
\* scheduled whenever they remain enabled. Output repair is intentionally not
\* in the fairness set and therefore cannot be a hidden activation prerequisite.
PostGSTFairness ==
    /\ WF_vars(HonestLeaderProposes)
    /\ WF_vars(HonestBodyService)
    /\ WF_vars(EnterFallback)
    /\ WF_vars(HonestPrepareService)
    /\ WF_vars(HonestTailPrepareQCService)
    /\ WF_vars(HonestCommitService)
    /\ WF_vars(HonestTailDecisionService)
    /\ WF_vars(HonestTimeoutService)
    /\ WF_vars(ChangeView)
    /\ WF_vars(LocalDecisionBodyRecovery)
    /\ WF_vars(LocalDecisionApplication)
    /\ WF_vars(ActivateSuccessor)

PostGSTSpec ==
    /\ Init
    /\ [][PostGSTNext]_vars
    /\ PostGSTFairness

CommitteeGeometry ==
    /\ Cardinality(SetA(view)) = Q
    /\ Cardinality(SetB(view)) = F
    /\ SetA(view) \cap SetB(view) = {}
    /\ SetA(view) \cup SetB(view) = Validators
    /\ Leader(view) \in SetA(view)
    /\ ProxyTail(view) \in SetA(view)

ManifestCommitteeFanout ==
    /\ (proposal = NoBody => manifestTargets = {})
    /\ (proposal \in Bodies => manifestTargets = Validators)

FastPathAndFallbackBodyFanout ==
    /\ (proposal = NoBody => bodyTargets = {})
    /\ (proposal \in Bodies =>
          bodyTargets = IF fallback THEN Validators ELSE SetA(view))

PrepareVotesRouteToProxyTail ==
    /\ prepareVoteRoutes \subseteq Validators \X {ProxyTail(view)}
    /\ RouteSources(prepareVoteRoutes) = VoteSigners(prepareVotes)

CommitVotesRouteToProxyTail ==
    /\ commitVoteRoutes \subseteq Validators \X {ProxyTail(view)}
    /\ RouteSources(commitVoteRoutes) = VoteSigners(commitVotes)

TimeoutVotesBypassProxyTail ==
    /\ RouteSources(timeoutVoteRoutes) = timeoutVotes
    /\ \A validator \in timeoutVotes :
          {target \in Validators :
              <<validator, target>> \in timeoutVoteRoutes} = Validators

FullBodyBeforePrepare ==
    \A validator \in Validators, body \in Bodies :
        <<validator, body>> \in prepareVotes =>
            <<validator, body>> \in fullBody

FullBodyBeforeCommit ==
    \A validator \in Validators, body \in Bodies :
        <<validator, body>> \in commitVotes =>
            <<validator, body>> \in fullBody

DecisionAgreement == Cardinality(decisions) <= 1

\* Enabledness is a safety check. The temporal consequence is separately
\* checked under PostGSTSpec and its explicit weak-fairness assumptions.
NonblockingSuccessorActivation ==
    (applied /\ finalizedOutputDebt /\ ~successorActive) =>
        ENABLED ActivateSuccessor

ConditionalPostGSTProgress ==
    <>(/\ decisions /= {}
       /\ applied
       /\ successorActive)

FinalizedOutputDebtDoesNotBlockSuccessor ==
    (applied /\ finalizedOutputDebt /\ ~successorActive) ~> successorActive

=============================================================================

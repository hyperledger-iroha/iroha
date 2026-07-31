---- MODULE SumeragiV2OwnershipInvariantCheck ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
Small exhaustive counterexample search for scheduler ownership.  The state
constraint holds the logical clock at its initial value while retaining every
production outer AsyncNext branch, so TLC can enumerate all zero-clock
ownership transfers without conflating this finite check with deductive proof
evidence.
***************************************************************************)

SingleValidatorRosters == <<<<0>>>>
SingleValidatorPowers == <<<<1>>>>

OwnershipAllVars == <<AsyncAllVars, acquisitionVars>>

OwnershipBoundedInit ==
  /\ AsyncFiniteInit
  /\ AcquisitionInit

(***************************************************************************
The production evidence union has one powerset-valued branch.  The runtime
type invariant already proves every command evidence value structurally
typed, so this bounded TLC instance may select the TC branch without
materializing `TcRecordSet`.  `ownership_n1.cfg` overrides only the equivalent
finite-search definitions in this module; each override preserves the
production predicate while avoiding an intractable generic carrier.
***************************************************************************)
OwnershipInstallTcFromEvidence(command) ==
  IF AsyncTcRecordTyped(command.evidence)
  THEN command.evidence
  ELSE command.evidence.envelope.tc

(***************************************************************************
The same structural expansion is required at the two authenticated-evidence
unions and at control publication.  Enumerating `AsyncNetworkItems` first
materializes the powerset-valued TC branch even though the runtime supplies
one already-typed value or a tiny concrete outbox.  These operators are exact
finite-search expansions of those membership/subset tests.
***************************************************************************)
OwnershipInstallTcEvidenceMatches(command, tc) ==
  \/ command.evidence = tc
  \/ /\ AsyncItemTyped(command.evidence)
     /\ command.evidence.kind = "TimeoutCertificate"
     /\ command.evidence.envelope.tc = tc

OwnershipBeginLockCommandEvidenceMatches(command, qc) ==
  \/ command.evidence = qc
  \/ /\ AsyncItemTyped(command.evidence)
     /\ \/ /\ command.evidence.kind = "PrepareQC"
           /\ command.evidence.envelope.qc = qc
        \/ /\ command.evidence.kind = "CertifiedResponse"
           /\ CertifiedResponseCapabilityAuthorized(command.evidence)
           /\ \E request \in
                  MatchingSentCertifiedRequests(command.evidence):
                /\ FrozenCertifiedResponseBinding(
                     command.evidence, request)
                /\ request.envelope.certificate = qc

OwnershipControlItemsTyped(items) ==
  \A item \in items:
    /\ AsyncItemTyped(item)
    /\ item.kind \in AsyncControlKinds

OwnershipPublishControlItems(items) ==
  /\ OwnershipControlItemsTyped(items)
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, items)
  /\ asyncSentItems' = asyncSentItems \cup items
  /\ asyncTransport' = asyncTransport \cup PacketsForItems(items)
  /\ UNCHANGED <<asyncActiveRequests, asyncCertifiedResponseClaim>>

OwnershipPublishControlAndEphemeralItems(controlItems, ephemeralItems) ==
  /\ OwnershipControlItemsTyped(controlItems)
  /\ asyncRetainedControl' =
       RememberedControl(asyncRetainedControl, controlItems)
  /\ asyncSentItems' =
       asyncSentItems \cup controlItems \cup ephemeralItems
  /\ asyncTransport' =
       asyncTransport
         \cup PacketsForItems(controlItems \cup ephemeralItems)
  /\ UNCHANGED <<asyncActiveRequests, asyncCertifiedResponseClaim>>

OwnershipExecuteSignProposalReady(command) ==
  /\ command.kind = "SignProposal"
  /\ \E request \in signProposals:
       LET controlItems == ProposalOutbox(request)
       IN /\ CommandMatches(command, request.node, request.proposal.view,
                             request.proposal.subject)
          /\ CompleteProposalSignatureReady(request)
          /\ OwnershipControlItemsTyped(controlItems)

OwnershipExecuteSignVoteReady(command) ==
  /\ command.kind = "SignVote"
  /\ \E request \in signVotes:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.subject)
       /\ CompleteVoteSignatureReady(request)
       /\ OwnershipControlItemsTyped(VoteOutbox(request))

OwnershipExecuteFormPrepareQCReady(command) ==
  LET signers == VoteSignersAt(command.node, command.view, "Prepare",
                               command.subject)
      qc == QC(context, command.view, "Prepare", command.subject, signers)
      items == QcOutbox(command.node, qc)
  IN /\ command.kind = "FormPrepareQC"
     /\ FormPrepareQCReady(command.node, command.view, command.subject)
     /\ OwnershipControlItemsTyped(items)

OwnershipExecuteSignTimeoutReady(command) ==
  /\ command.kind = "SignTimeout"
  /\ \E request \in signTimeouts:
       /\ CommandMatches(command, request.node, request.vote.view,
                         request.vote.highSubject)
       /\ CompleteTimeoutSignatureReady(request)
       /\ OwnershipControlItemsTyped(TimeoutOutbox(request))

(***************************************************************************
The claim invariant already requires every claim to have an authenticated
sent occurrence.  Its separate universe-membership conjunct can therefore
range over the finite sent history without changing the accepted states.
Using the same sent occurrence for authentication is equivalent to the
generic existential because the canonical claim projection omits only its
transport source.
***************************************************************************)
OwnershipAsyncCertifiedResponseClaimValues ==
  {AsyncCertifiedResponseCanonicalWireIdentity(item):
     item \in {candidate \in asyncSentItems:
       /\ AsyncItemTyped(candidate)
       /\ candidate.kind = "CertifiedResponse"}}

OwnershipCertifiedResponseClaimProjectionAuthenticated(projection) ==
  \E item \in asyncSentItems:
    /\ AsyncItemTyped(item)
    /\ item.kind = "CertifiedResponse"
    /\ AsyncCertifiedResponseCanonicalWireIdentity(item) = projection
    /\ CertifiedResponseAuthenticatedOccurrence(item)
    /\ item.envelope.archiveServer \in AsyncArchiveServerIds
    /\ MatchingCertifiedRequests(item) # {}
    /\ \E request \in MatchingCertifiedRequests(item):
         FrozenCertifiedResponseBinding(item, request)

OwnershipAsyncDeferredHandoffTyped(handoff) ==
  \/ handoff = NoAsyncDeferredHandoff
  \/ /\ DOMAIN handoff = {"active", "candidate", "identity"}
     /\ handoff.active = TRUE
     /\ AsyncCandidateTyped(handoff.candidate)
     /\ handoff.identity =
          ExactAsyncCandidateIdentity(handoff.candidate)

OwnershipAsyncDeferredTopologyTypeInvariant ==
  /\ DOMAIN asyncDeferredCompletionQueues = ValidatorIds
  /\ DOMAIN asyncDeferredProgressQueues = ValidatorIds
  /\ DOMAIN asyncDeferredNormalQueues = ValidatorIds
  /\ DOMAIN asyncDeferredHandoffs = ValidatorIds
  /\ \A node \in ValidatorIds:
       OwnershipAsyncDeferredHandoffTyped(
         asyncDeferredHandoffs[node])
  /\ asyncNextDeferredClass \in
       [ValidatorIds -> AsyncCommandClasses]
  /\ asyncDeferredDrainOwed \in [ValidatorIds -> BOOLEAN]

(***************************************************************************
The generic definition quantifies over `AsyncCandidateSet` before constraining
all candidate fields to one exact Fetch. That Cartesian record carrier is too
large for TLC once a historical lock appears. Constructing the unique
candidate and checking its structural type is logically equivalent and keeps
the bounded ownership search executable.
***************************************************************************)
OwnershipHistoricalLockRestartExactCurrentFetchOwner(authority) ==
  \E qc \in prepareQCs:
    LET candidate ==
          AsyncCandidateWithIdentity(
            "Completion", "FetchBody", authority.node,
            authority.context.height, authority.view, authority.subject,
            NoAsyncItem, authority.context, nodeView[authority.node],
            generation[authority.node], qc, authority.subject,
            authority.subject, authority.subject)
    IN /\ AsyncCandidateTyped(candidate)
       /\ HistoricalLockRestartAuthoritySourceKernel(
            authority, qc, context, nodeView, lockRank, lockSubject,
            installedTCs, commitIntents, decisions)
       /\ HistoricalLockRestartExactCurrentFetchKernel(
            authority, qc, candidate, context, nodeView, generation,
            asyncCommandQueues, asyncDeferredCompletionQueues,
            asyncDeferredProgressQueues, asyncDeferredNormalQueues,
            asyncCausalQueues, asyncOutstandingWork)

OwnershipHistoricalLockRestartExactCurrentFetchOwnerAfter(authority) ==
  \E qc \in prepareQCs':
    LET candidate ==
          AsyncCandidateWithIdentity(
            "Completion", "FetchBody", authority.node,
            authority.context.height, authority.view, authority.subject,
            NoAsyncItem, authority.context, nodeView'[authority.node],
            generation'[authority.node], qc, authority.subject,
            authority.subject, authority.subject)
    IN /\ AsyncCandidateTyped(candidate)
       /\ HistoricalLockRestartAuthoritySourceKernel(
            authority, qc, context', nodeView', lockRank', lockSubject',
            installedTCs', commitIntents', decisions')
       /\ HistoricalLockRestartExactCurrentFetchKernel(
            authority, qc, candidate, context', nodeView', generation',
            asyncCommandQueues', asyncDeferredCompletionQueues',
            asyncDeferredProgressQueues', asyncDeferredNormalQueues',
            asyncCausalQueues', asyncOutstandingWork')

(***************************************************************************
The production relation keeps Byzantine signer membership inside each Core
action. TLC enumerates the much larger proposal/QC domains before reaching
that guard, even when the N=1 ownership model has no Byzantine validator.
This equivalent finite-search projection moves the same signer guard to the
outer quantifier. It retains every fault branch while avoiding enumeration of
impossible Byzantine payloads in the all-honest boundary configuration.
***************************************************************************)
OwnershipFaultStep ==
  \/ \E packet \in asyncTransport: PreGstLosePacket(packet)
  \/ \E node \in ValidatorIds: PreGstCrash(node)
  \/ \E source \in AsyncIngressSources, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineNoise(source, recipient, nonce)
  \/ \E kind \in IngressTransportCompletionKinds,
       recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectUntrustedTransportCompletion(kind, recipient, nonce)
  \/ \E kind \in {"NormalJunk", "ProgressJunk"},
       source \in ValidatorIds, recipient \in ValidatorIds,
       nonce \in 0..(AsyncIngressCapacity - 1):
       InjectAuthenticatedJunk(kind, source, recipient, nonce)
  \/ \E source \in ValidatorIds, recipient \in ValidatorIds,
       qc \in commitQCs, nonce \in 0..(AsyncIngressCapacity - 1):
       InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
  \/ \E signer \in Byzantine(CurrentEpoch) \cap up,
       roundView \in Views, subject \in Subjects,
       timeoutCertificate \in TimeoutCertificateOptionSet,
       highestPrepare \in PrepareQcOptionSet:
       AsyncByzantineProposal(signer, roundView, subject,
                              timeoutCertificate, highestPrepare)
  \/ \E signer \in Byzantine(CurrentEpoch) \cap up,
       roundView \in Views, phase \in Phases, subject \in Subjects:
       AsyncByzantineVote(signer, roundView, phase, subject)
  \/ \E signer \in Byzantine(CurrentEpoch) \cap up,
       roundView \in Views, highestPrepare \in PrepareQcOptionSet:
       AsyncByzantineTimeout(signer, roundView, highestPrepare)

(***************************************************************************
`AsyncNext` conjoins the exact outer action with `[Next]_vars`.  Every outer
branch already executes a proved Core action or Core stutter; re-evaluating
the whole generic Core relation is semantically redundant and forces TLC to
materialize unrelated Cartesian carriers.  This bounded projection removes
only that redundant search conjunct.  All production outer branches, restart
authority transitions, and height/context frames remain exact.
***************************************************************************)
OwnershipAsyncNext ==
  /\ (AsyncNonCrashStep
        \/ (\E node \in ValidatorIds: PreGstCrash(node))
        \/ (\E node \in ValidatorIds: PreGstResponsiveCrash(node))
        \/ PreGstResponsiveRestart
        \/ PreGstResponsiveReplay)
  /\ AsyncHistoricalLockRestartAuthorityTransition
  /\ UNCHANGED <<height, context>>

OwnershipBoundedNext ==
  /\ OwnershipAsyncNext
  /\ UNCHANGED acquisitionVars

OwnershipDebugStutter ==
  UNCHANGED OwnershipAllVars

OwnershipBoundedSpec ==
  OwnershipBoundedInit /\ [][OwnershipBoundedNext]_OwnershipAllVars

OwnershipInitialClock == asyncNow = 0

=============================================================================

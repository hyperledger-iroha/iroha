---- MODULE SumeragiV2AdequateLeaderRetainedProducerClosureProofs ----
EXTENDS SumeragiV2AdequateLeaderServiceClosureProofs,
        SumeragiV2AsyncFiniteProducerEpisodes

(***************************************************************************
Retained ingress-producer ownership for one frozen adequate-leader corridor.

The older adequate-leader episode counts candidate and leader-wire owners.
This module adds the source-qualified ingress occurrence which precedes those
owners.  Its identity is the immutable semantic request together with the
authenticated transport source.  Mutable packet positions, generations, and
runner locations are deliberately absent.  A same-source retry is therefore
the same identity, while an alternate authenticated source consumes one
member of the configured finite source carrier.

`asyncProducerConsumedEpisodes` is the monotone ingress-retirement journal.
For exact Serve requests, `asyncServeAttempts` is the stronger lifecycle
memory: it retains the source key after drain and reaches `Complete` instead
of deleting the old logical request.  Neither memory is a fairness premise or
a progress endpoint.  A first distinct episode decreases the ingress-leading
composite rank; exact retransmission is a non-descent stutter.
***************************************************************************)

AdequateLeaderFrozenTargetProducerItems(
    target, leaderContext, leader, leaderView, subject) ==
  {item \in
     AdequateLeaderFrozenLifecycleDeliveryItems(
       target, leaderContext, leader, leaderView):
     /\ DeliveryView(item) = leaderView
     /\ DeliverySubject(item) = subject}

AdequateLeaderFrozenTargetProducerRequests(
    target, leaderContext, leader, leaderView, subject) ==
  {AsyncProducerIngressRequest(item):
     item \in AdequateLeaderFrozenTargetProducerItems(
                target, leaderContext, leader, leaderView, subject)}

AdequateLeaderRetainedProducerRequestOwner(request) ==
  IF request \in AsyncServeLogicalRequestIdentities
  THEN request.owner
  ELSE request.envelope.recipient

AdequateLeaderRetainedProducerRequestPhase(request) ==
  IF request \in AsyncServeLogicalRequestIdentities
  THEN request.request.kind
  ELSE request.kind

AdequateLeaderFrozenProducerOwnerIdentity(
    request, authenticatedSource,
    target, leaderContext, leader, leaderView, subject) ==
  [target |-> target,
   context |-> leaderContext,
   leader |-> leader,
   view |-> leaderView,
   subject |-> subject,
   phase |-> AdequateLeaderRetainedProducerRequestPhase(request),
   authority |->
     AdequateLeaderCorridorAuthorityReceipt(
       target, leaderContext, leader, leaderView),
   owner |-> AdequateLeaderRetainedProducerRequestOwner(request),
   kind |-> "IngressProducer",
   payload |->
     [request |-> request,
      authenticatedSource |-> authenticatedSource]]

AdequateLeaderRetainedFrozenProducerOwnerUniverse(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenProducerOwnerIdentity(
     request, authenticatedSource,
     target, leaderContext, leader, leaderView, subject):
     request \in AdequateLeaderFrozenTargetProducerRequests(
                   target, leaderContext, leader, leaderView, subject),
     authenticatedSource \in AsyncIngressSources}

AdequateLeaderRetainedFrozenOwnerUniverse(
    target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderFrozenOwnerUniverse(
    target, leaderContext, leader, leaderView, subject)
    \cup
  AdequateLeaderRetainedFrozenProducerOwnerUniverse(
    target, leaderContext, leader, leaderView, subject)

THEOREM AdequateLeaderFrozenProducerOwnerCarriesAuthorityReceipt ==
  \A owner, target, leaderContext, leader, leaderView, subject:
    owner \in
      AdequateLeaderRetainedFrozenProducerOwnerUniverse(
        target, leaderContext, leader, leaderView, subject)
      => /\ owner.target = target
         /\ owner.context = leaderContext
         /\ owner.leader = leader
         /\ owner.view = leaderView
         /\ owner.subject = subject
         /\ owner.kind = "IngressProducer"
         /\ owner.authority =
              AdequateLeaderCorridorAuthorityReceipt(
                target, leaderContext, leader, leaderView)
BY Isa
   DEF AdequateLeaderRetainedFrozenProducerOwnerUniverse,
       AdequateLeaderFrozenProducerOwnerIdentity

THEOREM AdequateLeaderRetainedFrozenProducerOwnerUniverseIsPrimeInvariant ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderRetainedFrozenProducerOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)'
      = AdequateLeaderRetainedFrozenProducerOwnerUniverse(
          target, leaderContext, leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderRetainedFrozenProducerOwnerUniverse,
       AdequateLeaderFrozenTargetProducerRequests,
       AdequateLeaderFrozenTargetProducerItems,
       AdequateLeaderFrozenProducerOwnerIdentity,
       AdequateLeaderRetainedProducerRequestOwner,
       AdequateLeaderRetainedProducerRequestPhase,
       AdequateLeaderFrozenLifecycleDeliveryItems,
       AdequateLeaderFrozenLifecycleNodes,
       AdequateLeaderFrozenNetworkItemCarrier,
       AdequateLeaderCorridorAuthorityReceipt

THEOREM AdequateLeaderRetainedFrozenOwnerUniverseIsPrimeInvariant ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderRetainedFrozenOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)'
      = AdequateLeaderRetainedFrozenOwnerUniverse(
          target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant,
   AdequateLeaderRetainedFrozenProducerOwnerUniverseIsPrimeInvariant, Isa
   DEF AdequateLeaderRetainedFrozenOwnerUniverse

THEOREM AdequateLeaderFrozenTargetProducerRequestsAreFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => IsFiniteSet(
         AdequateLeaderFrozenTargetProducerRequests(
           target, leaderContext, leader, leaderView, subject))
BY FS_Interval, FS_Image, FS_Union, FS_Subset, FS_Product, IsaT(900)
   DEF AdequateLeaderFrozenTargetProducerRequests,
       AdequateLeaderFrozenTargetProducerItems,
       AdequateLeaderFrozenLifecycleDeliveryItems,
       AdequateLeaderFrozenLifecycleNodes,
       AdequateLeaderFrozenNetworkItemCarrier,
       AdequateLeaderFrozenBodyEnvelopeCarrier,
       AdequateLeaderFrozenCertifiedRequestItemCarrier,
       AdequateLeaderFrozenCertifiedRequestHashCarrier,
       AdequateLeaderFrozenCommitRequestItemCarrier,
       AdequateLeaderFrozenQcRecordCarrier,
       AdequateLeaderFrozenTcRecordCarrier,
       AdequateLeaderFrozenProposalRecordCarrier,
       AdequateLeaderFrozenVoteRecordCarrier,
       AdequateLeaderFrozenTimeoutVoteRecordCarrier,
       AsyncProducerIngressRequest

THEOREM AdequateLeaderRetainedFrozenProducerOwnerUniverseIsFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => IsFiniteSet(
         AdequateLeaderRetainedFrozenProducerOwnerUniverse(
           target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderFrozenTargetProducerRequestsAreFinite,
   FS_Image, FS_Product, IsaT(300)
   DEF AdequateLeaderRetainedFrozenProducerOwnerUniverse,
       AsyncIngressSources, AsyncAuthenticatedDeliverySources,
       AsyncArchiveServerIds, ValidatorIds

THEOREM AdequateLeaderRetainedFrozenOwnerUniverseIsFinite ==
  \A target, leaderContext, leader, leaderView, subject:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    => IsFiniteSet(
         AdequateLeaderRetainedFrozenOwnerUniverse(
           target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderFrozenOwnerUniverseIsFinite,
   AdequateLeaderRetainedFrozenProducerOwnerUniverseIsFinite,
   FS_Union, Isa
   DEF AdequateLeaderRetainedFrozenOwnerUniverse

THEOREM AdequateLeaderRetainedProducerRetryIdentityIsStable ==
  \A leftRequest, rightRequest, leftSource, rightSource,
     target, leaderContext, leader, leaderView, subject:
    /\ leftRequest = rightRequest
    /\ leftSource = rightSource
    => AdequateLeaderFrozenProducerOwnerIdentity(
         leftRequest, leftSource,
         target, leaderContext, leader, leaderView, subject)
         = AdequateLeaderFrozenProducerOwnerIdentity(
             rightRequest, rightSource,
             target, leaderContext, leader, leaderView, subject)
BY Isa

(***************************************************************************
State projections.

The packet arm contains only not-yet-retired ingress episodes.  The attempt
arm contains the exact route-local Serve lifecycle after atomic admission.
The journal arm remains after every volatile carrier drains.  Thus a logical
request cannot regain its old ingress stage merely by retransmission.
***************************************************************************)

AdequateLeaderTargetProducerJournalEpisodes(
    target, leaderContext, leader, leaderView, subject) ==
  {episode \in asyncProducerConsumedEpisodes:
     /\ episode.request
          \in AdequateLeaderFrozenTargetProducerRequests(
               target, leaderContext, leader, leaderView, subject)
     /\ episode.authenticatedSource \in AsyncIngressSources}

AdequateLeaderTargetProducerPacketEpisodes(
    target, leaderContext, leader, leaderView, subject) ==
  {AsyncProducerIngressEpisode(
     packet.item, packet.authenticatedSource):
     packet \in
       {candidate \in asyncTransport:
          /\ AsyncProducerIngressRequest(candidate.item)
               \in AdequateLeaderFrozenTargetProducerRequests(
                    target, leaderContext, leader, leaderView, subject)
          /\ candidate.authenticatedSource \in AsyncIngressSources
          /\ AsyncProducerIngressEpisode(
               candidate.item, candidate.authenticatedSource)
               \notin asyncProducerConsumedEpisodes}}

AdequateLeaderTargetProducerPacketEpisodesFor(
    request, target, leaderContext, leader, leaderView, subject) ==
  {episode \in
     AdequateLeaderTargetProducerPacketEpisodes(
       target, leaderContext, leader, leaderView, subject):
     episode.request = request}

AdequateLeaderTargetProducerActiveAttemptEpisodes(
    target, leaderContext, leader, leaderView, subject) ==
  {AsyncProducerEpisode(
     attempt.key.identity,
     attempt.key.source,
     AsyncProducerIngressStage):
     attempt \in
       {candidate \in asyncServeAttempts:
          /\ candidate.key.identity
               \in AdequateLeaderFrozenTargetProducerRequests(
                    target, leaderContext, leader, leaderView, subject)
          /\ candidate.key.source \in AsyncIngressSources
          /\ candidate.stage # "Complete"}}

AdequateLeaderTargetProducerCompleteAttemptEpisodes(
    target, leaderContext, leader, leaderView, subject) ==
  {AsyncProducerEpisode(
     attempt.key.identity,
     attempt.key.source,
     AsyncProducerIngressStage):
     attempt \in
       {candidate \in asyncServeAttempts:
          /\ candidate.key.identity
               \in AdequateLeaderFrozenTargetProducerRequests(
                    target, leaderContext, leader, leaderView, subject)
          /\ candidate.key.source \in AsyncIngressSources
          /\ candidate.stage = "Complete"}}

AdequateLeaderTargetActiveRetainedProducerEpisodes(
    target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderTargetProducerPacketEpisodes(
    target, leaderContext, leader, leaderView, subject)
    \cup
  AdequateLeaderTargetProducerActiveAttemptEpisodes(
    target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetRetainedProducerEpisodes(
    target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderTargetActiveRetainedProducerEpisodes(
    target, leaderContext, leader, leaderView, subject)
    \cup
  AdequateLeaderTargetProducerJournalEpisodes(
    target, leaderContext, leader, leaderView, subject)
    \cup
  AdequateLeaderTargetProducerCompleteAttemptEpisodes(
    target, leaderContext, leader, leaderView, subject)

AdequateLeaderTargetRetainedProducerOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenProducerOwnerIdentity(
     episode.request, episode.authenticatedSource,
     target, leaderContext, leader, leaderView, subject):
     episode \in AdequateLeaderTargetRetainedProducerEpisodes(
                   target, leaderContext, leader, leaderView, subject)}

AdequateLeaderTargetActiveRetainedProducerOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenProducerOwnerIdentity(
     episode.request, episode.authenticatedSource,
     target, leaderContext, leader, leaderView, subject):
     episode \in AdequateLeaderTargetActiveRetainedProducerEpisodes(
                   target, leaderContext, leader, leaderView, subject)}

AdequateLeaderTargetRetainedProducerTransportOwner(
    target, leaderContext, leader, leaderView, subject) ==
  AdequateLeaderTargetActiveRetainedProducerOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) # {}

THEOREM AdequateLeaderRetainedProducerOwnersStayInsideFrozenUniverse ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderTargetRetainedProducerOwnerIdentitySet(
      target, leaderContext, leader, leaderView, subject)
      \subseteq
    AdequateLeaderRetainedFrozenProducerOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)
BY Isa
   DEF AdequateLeaderTargetRetainedProducerOwnerIdentitySet,
       AdequateLeaderTargetRetainedProducerEpisodes,
       AdequateLeaderTargetActiveRetainedProducerEpisodes,
       AdequateLeaderTargetProducerJournalEpisodes,
       AdequateLeaderTargetProducerPacketEpisodes,
       AdequateLeaderTargetProducerActiveAttemptEpisodes,
       AdequateLeaderTargetProducerCompleteAttemptEpisodes,
       AdequateLeaderRetainedFrozenProducerOwnerUniverse,
       AsyncProducerIngressEpisode,
       AsyncProducerEpisode

THEOREM AdequateLeaderActiveRetainedProducerOwnerIsFrozen ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderTargetRetainedProducerTransportOwner(
      target, leaderContext, leader, leaderView, subject)
      => /\ AdequateLeaderTargetActiveRetainedProducerOwnerIdentitySet(
               target, leaderContext, leader, leaderView, subject) # {}
         /\ AdequateLeaderTargetActiveRetainedProducerOwnerIdentitySet(
               target, leaderContext, leader, leaderView, subject)
              \subseteq
            AdequateLeaderRetainedFrozenProducerOwnerUniverse(
              target, leaderContext, leader, leaderView, subject)
BY AdequateLeaderRetainedProducerOwnersStayInsideFrozenUniverse, Isa
   DEF AdequateLeaderTargetRetainedProducerTransportOwner,
       AdequateLeaderTargetActiveRetainedProducerOwnerIdentitySet,
       AdequateLeaderTargetRetainedProducerOwnerIdentitySet,
       AdequateLeaderTargetRetainedProducerEpisodes,
       AdequateLeaderTargetActiveRetainedProducerEpisodes

THEOREM AdequateLeaderActiveRetainedProducerExposesNamedAuthorityOwner ==
  \A target, leaderContext, leader, leaderView, subject:
    AdequateLeaderTargetRetainedProducerTransportOwner(
      target, leaderContext, leader, leaderView, subject)
      => \E producerOwner \in
           AdequateLeaderRetainedFrozenProducerOwnerUniverse(
             target, leaderContext, leader, leaderView, subject):
           /\ producerOwner \in
                AdequateLeaderTargetActiveRetainedProducerOwnerIdentitySet(
                  target, leaderContext, leader, leaderView, subject)
           /\ producerOwner.authority =
                AdequateLeaderCorridorAuthorityReceipt(
                  target, leaderContext, leader, leaderView)
BY AdequateLeaderActiveRetainedProducerOwnerIsFrozen,
   AdequateLeaderFrozenProducerOwnerCarriesAuthorityReceipt, Isa

AdequateLeaderRetainedProducerTombstoneMemory(
    request, authenticatedSource) ==
  /\ request \in AsyncServeLogicalRequestIdentities
  /\ authenticatedSource \in AsyncAuthenticatedDeliverySources
  /\ AsyncServeSourceAttemptOwned(
       request.owner, request, authenticatedSource)

AdequateLeaderRetainedProducerCompleteTombstoneMemory(
    request, authenticatedSource) ==
  /\ AdequateLeaderRetainedProducerTombstoneMemory(
       request, authenticatedSource)
  /\ (AsyncServeSourceAttemptRecord(
        request.owner, request, authenticatedSource)).stage = "Complete"

THEOREM AdequateLeaderRetainedServeTombstoneBlocksLifecycleRecreation ==
  \A request \in AsyncServeLogicalRequestIdentities,
     authenticatedSource \in AsyncAuthenticatedDeliverySources,
     candidate \in AsyncCandidateSet:
    /\ AdequateLeaderRetainedProducerTombstoneMemory(
         request, authenticatedSource)
    /\ candidate.kind = "AcceptCertifiedRequest"
    /\ AsyncServeLogicalRequestIdentity(
         request.owner, candidate.item) = request
    => /\ ~ReserveExactServeCapacityVia(
                request.owner, candidate, authenticatedSource)
       /\ ~AdvanceExactServeCapacityVia(
                request.owner, candidate, authenticatedSource)
BY RetainedServeAttemptCannotReserveOrAdvanceExactLifecycle, Isa
   DEF AdequateLeaderRetainedProducerTombstoneMemory,
       AsyncServeLifecycleKnown,
       AsyncServeSourceAttemptOwned,
       AsyncServeSourceAttemptRecordsForSource,
       AsyncServeSourceAttemptRecords

THEOREM AdequateLeaderConsumedProducerEpisodesAreMonotone ==
  \A target, leaderContext, leader, leaderView, subject:
    AsyncProducerJournalMonotoneStep
      => AdequateLeaderTargetProducerJournalEpisodes(
           target, leaderContext, leader, leaderView, subject)
           \subseteq
         AdequateLeaderTargetProducerJournalEpisodes(
           target, leaderContext, leader, leaderView, subject)'
BY AdequateLeaderRetainedFrozenProducerOwnerUniverseIsPrimeInvariant, Isa
   DEF AdequateLeaderTargetProducerJournalEpisodes,
       AsyncProducerJournalMonotoneStep

THEOREM AdequateLeaderConsumedProducerEpisodeCannotReenterRemainingRank ==
  \A request \in AsyncProducerIngressRequests,
     episode \in AsyncProducerIngressEpisodeUniverseFor(request):
    episode \in asyncProducerConsumedEpisodes
      => episode \notin AsyncProducerRemainingIngressEpisodesFor(request)
BY Isa
   DEF AsyncProducerRemainingIngressEpisodesFor,
       AsyncProducerConsumedIngressEpisodesFor

(***************************************************************************
Ingress-leading rank bridge.

The candidate tail is the existing exact occurrence budget.  Its configured
radix is unchanged.  First distinct admission strictly lowers the leading
component even if the bounded tail changes; an exact retransmission preserves
the whole rank only under an explicit tail frame.  The latter theorem is a
non-progress statement.
***************************************************************************)

AdequateLeaderRetainedProducerCompositeRank(
    request, node, cutoffOrdinal) ==
  AsyncFiniteIngressCandidateCompositeRankFor(
    request, node, cutoffOrdinal)

THEOREM AdequateLeaderFirstDistinctProducerEpisodeStrictlyDecreasesCompositeRank ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncConfiguration
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerFirstDistinctIngressEpisodeStepFor(request)
    /\ AsyncFiniteCandidateEpisodeTailTypeInvariant
    /\ AsyncFiniteCandidateEpisodeTailTypeInvariant'
    => AdequateLeaderRetainedProducerCompositeRank(
         request, node, cutoffOrdinal)'
         < AdequateLeaderRetainedProducerCompositeRank(
             request, node, cutoffOrdinal)
BY AsyncFiniteFirstDistinctTargetIngressDominatesCandidateEpisodeTail
   DEF AdequateLeaderRetainedProducerCompositeRank

THEOREM AdequateLeaderExactProducerRetransmissionIsCompositeRankStutter ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerExactRetransmissionEpisodeStepFor(request)
    /\ AsyncFiniteCandidateEpisodeTailRank(node, cutoffOrdinal)'
         = AsyncFiniteCandidateEpisodeTailRank(node, cutoffOrdinal)
    => AdequateLeaderRetainedProducerCompositeRank(
         request, node, cutoffOrdinal)'
         = AdequateLeaderRetainedProducerCompositeRank(
             request, node, cutoffOrdinal)
BY AsyncFiniteExactTargetRetransmissionPreservesCompositeUnderTailFrame
   DEF AdequateLeaderRetainedProducerCompositeRank

AdequateLeaderTargetProducerJournalOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject) ==
  {AdequateLeaderFrozenProducerOwnerIdentity(
     episode.request, episode.authenticatedSource,
     target, leaderContext, leader, leaderView, subject):
     episode \in AdequateLeaderTargetProducerJournalEpisodes(
                   target, leaderContext, leader, leaderView, subject)}

THEOREM AdequateLeaderFirstDistinctProducerEpisodeExposesFrozenOwner ==
  \A request, target, leaderContext, leader, leaderView, subject:
    /\ request \in AdequateLeaderFrozenTargetProducerRequests(
         target, leaderContext, leader, leaderView, subject)
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerFirstDistinctIngressEpisodeStepFor(request)
    => (AdequateLeaderTargetProducerJournalOwnerIdentitySet(
          target, leaderContext, leader, leaderView, subject)'
          \ AdequateLeaderTargetProducerJournalOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject)) # {}
BY AdequateLeaderRetainedFrozenProducerOwnerUniverseIsPrimeInvariant,
   IsaT(300)
   DEF AdequateLeaderTargetProducerJournalOwnerIdentitySet,
       AdequateLeaderTargetProducerJournalEpisodes,
       AdequateLeaderFrozenProducerOwnerIdentity,
       AsyncProducerProjectionStep,
       AsyncProducerFirstDistinctIngressEpisodeStepFor,
       AsyncProducerAdmittedIngressEpisodesFor,
       AsyncProducerConsumedIngressEpisodesFor,
       AsyncProducerIngressEpisodeUniverseFor

THEOREM AdequateLeaderExactProducerRetransmissionDoesNotDiscoverOwner ==
  \A request, target, leaderContext, leader, leaderView, subject:
    /\ request \in AdequateLeaderFrozenTargetProducerRequests(
         target, leaderContext, leader, leaderView, subject)
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerExactRetransmissionEpisodeStepFor(request)
    => {identity \in
          AdequateLeaderTargetProducerJournalOwnerIdentitySet(
            target, leaderContext, leader, leaderView, subject)':
          identity.payload.request = request}
       = {identity \in
            AdequateLeaderTargetProducerJournalOwnerIdentitySet(
              target, leaderContext, leader, leaderView, subject):
            identity.payload.request = request}
BY AdequateLeaderRetainedFrozenProducerOwnerUniverseIsPrimeInvariant,
   IsaT(300)
   DEF AdequateLeaderTargetProducerJournalOwnerIdentitySet,
       AdequateLeaderTargetProducerJournalEpisodes,
       AdequateLeaderFrozenProducerOwnerIdentity,
       AsyncProducerProjectionStep,
       AsyncProducerExactRetransmissionEpisodeStepFor,
       AsyncProducerAdmittedIngressEpisodesFor,
       AsyncProducerConsumedIngressEpisodesFor,
       AsyncProducerIngressEpisodeUniverseFor

(***************************************************************************
Finite/coalesced non-descent producer episode.

`known` is immutable episode history, and its rank is the complement inside
the frozen request/source owner universe.  Discovering an alternate source
strictly consumes that complement.  Same-source retransmission discovers no
identity.  This budget only closes a proofless non-descent episode; it is not
itself Decision, occurrence-rank descent, or scheduler progress.
***************************************************************************)

AdequateLeaderRetainedProducerKnownOwnerSet(
    target, leaderContext, leader, leaderView, subject, known) ==
  /\ IsFiniteSet(known)
  /\ known \subseteq
       AdequateLeaderRetainedFrozenProducerOwnerUniverse(
         target, leaderContext, leader, leaderView, subject)

AdequateLeaderRetainedProducerDiscoveredOwnerSet(
    target, leaderContext, leader, leaderView, subject, known) ==
  AdequateLeaderTargetProducerJournalOwnerIdentitySet(
    target, leaderContext, leader, leaderView, subject)
    \ known

AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
    target, leaderContext, leader, leaderView, subject, known) ==
  Cardinality(
    AdequateLeaderRetainedFrozenProducerOwnerUniverse(
      target, leaderContext, leader, leaderView, subject)
      \ known)

AdequateLeaderRetainedProducerNonDescentEpisodeResidual(
    target, leaderContext, leader, leaderView, subject, known) ==
  /\ AdequateLeaderRetainedProducerKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ AdequateLeaderRetainedProducerDiscoveredOwnerSet(
       target, leaderContext, leader, leaderView, subject, known) # {}

THEOREM AdequateLeaderRetainedProducerNonDescentEpisodeBudgetIsFiniteAndCoalesced ==
  \A target, leaderContext, leader, leaderView, subject, known:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    /\ AdequateLeaderRetainedProducerKnownOwnerSet(
         target, leaderContext, leader, leaderView, subject, known)
    => /\ AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
             target, leaderContext, leader, leaderView, subject, known) \in Nat
       /\ AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
             target, leaderContext, leader, leaderView, subject, known)
            <= Cardinality(
                 AdequateLeaderRetainedFrozenProducerOwnerUniverse(
                   target, leaderContext, leader, leaderView, subject))
BY AdequateLeaderRetainedFrozenProducerOwnerUniverseIsFinite,
   FS_Subset, FS_CardinalityType, IsaT(180)
   DEF AdequateLeaderRetainedProducerKnownOwnerSet,
       AdequateLeaderRetainedProducerNonDescentEpisodeBudget

AdequateLeaderRetainedProducerKnownAdvanceGoal(
    target, leaderContext, leader, leaderView,
    subject, known, budget) ==
  \E discovered,
     known2 \in
       SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
         target, leaderContext, leader, leaderView, subject),
     budget2 \in Nat:
    /\ discovered =
         AdequateLeaderRetainedProducerDiscoveredOwnerSet(
           target, leaderContext, leader, leaderView, subject, known)
    /\ discovered # {}
    /\ known2 = known \cup discovered
    /\ budget2 =
         AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known2)
    /\ budget2 < budget

THEOREM AdequateLeaderRetainedProducerDiscoveryStrictlyConsumesBudget ==
  \A target, leaderContext, leader, leaderView,
     subject, known, budget:
    /\ target \in ValidatorIds
    /\ leaderContext \in ContextRecords
    /\ leader \in ValidatorIds
    /\ leaderView \in Nat
    /\ subject \in Subjects
    /\ AdequateLeaderRetainedProducerNonDescentEpisodeResidual(
         target, leaderContext, leader, leaderView, subject, known)
    /\ budget =
         AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known)
    => AdequateLeaderRetainedProducerKnownAdvanceGoal(
         target, leaderContext, leader, leaderView,
         subject, known, budget)
BY AdequateLeaderRetainedFrozenProducerOwnerUniverseIsFinite,
   AdequateLeaderRetainedProducerOwnersStayInsideFrozenUniverse,
   FS_Union, FS_Subset, FS_CardinalityType, IsaT(300)
   DEF AdequateLeaderRetainedProducerNonDescentEpisodeResidual,
       AdequateLeaderRetainedProducerKnownAdvanceGoal,
       AdequateLeaderRetainedProducerDiscoveredOwnerSet,
       AdequateLeaderRetainedProducerNonDescentEpisodeBudget,
       AdequateLeaderRetainedProducerKnownOwnerSet,
       AdequateLeaderTargetProducerJournalOwnerIdentitySet,
       AdequateLeaderTargetRetainedProducerOwnerIdentitySet

(***************************************************************************
Temporal composition interface.

The ranked source is an actual not-yet-journaled packet for the exact request.
The post-admission nonterminal `asyncServeAttempts` arm is deliberately not a
second ingress-rank source: atomic admission has already consumed that source
episode, and the inherited Serve/candidate ordinal continues through the
existing occurrence-service corridor.  The retained attempt remains an
explicit owner and tombstone above, but asking it to decrease the already
consumed ingress coordinate would manufacture progress.

At a fixed ingress/candidate composite rank in the exact frozen corridor, the
required concrete packet service provider must reach exact-target Decision,
lower that rank, or expose an actually journaled new frozen producer identity.
The theorem below eliminates only the finite identity-replenishment episode.
It intentionally does not assert the step property, add fairness, or turn
replenishment into progress.
***************************************************************************)

AdequateLeaderRetainedProducerEpisodeAtRank(
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget) ==
  /\ AdequateLeaderFrozenTargetCorridor(
       target, leaderContext, leader, leaderView)
  /\ request \in AdequateLeaderFrozenTargetProducerRequests(
       target, leaderContext, leader, leaderView, subject)
  /\ node = AdequateLeaderRetainedProducerRequestOwner(request)
  /\ node \in {target, leader}
  /\ AdequateLeaderTargetProducerPacketEpisodesFor(
       request, target, leaderContext, leader, leaderView, subject) # {}
  /\ cutoffOrdinal \in Nat
  /\ sourceRank =
       AdequateLeaderRetainedProducerCompositeRank(
         request, node, cutoffOrdinal)
  /\ AdequateLeaderRetainedProducerKnownOwnerSet(
       target, leaderContext, leader, leaderView, subject, known)
  /\ AdequateLeaderTargetProducerJournalOwnerIdentitySet(
       target, leaderContext, leader, leaderView, subject)
       \subseteq known
  /\ budget =
       AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
         target, leaderContext, leader, leaderView, subject, known)

THEOREM AdequateLeaderRetainedProducerRankSourceIsUnconsumedPacket ==
  \A request, node, cutoffOrdinal, sourceRank,
     target, leaderContext, leader, leaderView,
     subject, known, budget:
    AdequateLeaderRetainedProducerEpisodeAtRank(
      request, node, cutoffOrdinal, sourceRank,
      target, leaderContext, leader, leaderView,
      subject, known, budget)
      => \E episode \in AsyncProducerIngressEpisodeUniverseFor(request):
           /\ episode \in
                AdequateLeaderTargetProducerPacketEpisodesFor(
                  request, target, leaderContext,
                  leader, leaderView, subject)
           /\ episode \notin asyncProducerConsumedEpisodes
BY Isa
   DEF AdequateLeaderRetainedProducerEpisodeAtRank,
       AdequateLeaderTargetProducerPacketEpisodesFor,
       AdequateLeaderTargetProducerPacketEpisodes,
       AsyncProducerIngressEpisodeUniverseFor,
       AsyncProducerIngressEpisode,
       AsyncProducerEpisode

AdequateLeaderRetainedProducerActualKnownAdvanceGoal(
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget) ==
  \E discovered,
     known2 \in
       SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
         target, leaderContext, leader, leaderView, subject),
     budget2 \in SetLessThan(budget, OpToRel(<, Nat), Nat):
    /\ discovered =
         AdequateLeaderRetainedProducerDiscoveredOwnerSet(
           target, leaderContext, leader, leaderView, subject, known)
    /\ discovered # {}
    /\ known2 = known \cup discovered
    /\ budget2 =
         AdequateLeaderRetainedProducerNonDescentEpisodeBudget(
           target, leaderContext, leader, leaderView, subject, known2)
    /\ AdequateLeaderRetainedProducerEpisodeAtRank(
         request, node, cutoffOrdinal, sourceRank,
         target, leaderContext, leader, leaderView,
         subject, known2, budget2)

AdequateLeaderRetainedProducerExactRankGoal(
    request, node, cutoffOrdinal, sourceRank, target) ==
  \/ NodeHasDecision(target)
  \/ AdequateLeaderRetainedProducerCompositeRank(
       request, node, cutoffOrdinal) < sourceRank

AdequateLeaderRetainedProducerRankOrKnownAdvanceGoal(
    request, node, cutoffOrdinal, sourceRank,
    target, leaderContext, leader, leaderView,
    subject, known, budget) ==
  \/ AdequateLeaderRetainedProducerExactRankGoal(
       request, node, cutoffOrdinal, sourceRank, target)
  \/ AdequateLeaderRetainedProducerActualKnownAdvanceGoal(
       request, node, cutoffOrdinal, sourceRank,
       target, leaderContext, leader, leaderView,
       subject, known, budget)

THEOREM AdequateLeaderExactProducerRetransmissionCannotAdvanceKnownBudget ==
  \A request, node, cutoffOrdinal, sourceRank,
     target, leaderContext, leader, leaderView,
     subject, known, budget:
    /\ AdequateLeaderRetainedProducerEpisodeAtRank(
         request, node, cutoffOrdinal, sourceRank,
         target, leaderContext, leader, leaderView,
         subject, known, budget)
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerExactRetransmissionEpisodeStep
    => ~(AdequateLeaderRetainedProducerActualKnownAdvanceGoal(
           request, node, cutoffOrdinal, sourceRank,
           target, leaderContext, leader, leaderView,
           subject, known, budget))'
BY AsyncProducerExactRetransmissionIsJournalStutter,
   AdequateLeaderRetainedFrozenProducerOwnerUniverseIsPrimeInvariant,
   IsaT(180)
   DEF AdequateLeaderRetainedProducerActualKnownAdvanceGoal,
       AdequateLeaderRetainedProducerEpisodeAtRank,
       AdequateLeaderRetainedProducerDiscoveredOwnerSet,
       AdequateLeaderTargetProducerJournalOwnerIdentitySet,
       AdequateLeaderTargetProducerJournalEpisodes,
       AsyncProducerVars

AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty(specification) ==
  specification
    => \A request \in AsyncProducerIngressRequests,
          node \in ValidatorIds,
          cutoffOrdinal \in Nat,
          sourceRank \in Nat,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          known \in
            SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat:
         AdequateLeaderRetainedProducerEpisodeAtRank(
           request, node, cutoffOrdinal, sourceRank,
           target, leaderContext, leader, leaderView,
           subject, known, budget)
           ~> AdequateLeaderRetainedProducerRankOrKnownAdvanceGoal(
                request, node, cutoffOrdinal, sourceRank,
                target, leaderContext, leader, leaderView,
                subject, known, budget)

AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty(
    specification) ==
  specification
    => \A request \in AsyncProducerIngressRequests,
          node \in ValidatorIds,
          cutoffOrdinal \in Nat,
          sourceRank \in Nat,
          target \in ValidatorIds,
          leaderContext \in ContextRecords,
          leader \in ValidatorIds,
          leaderView \in Views,
          subject \in Subjects,
          known \in
            SUBSET AdequateLeaderRetainedFrozenProducerOwnerUniverse(
              target, leaderContext, leader, leaderView, subject),
          budget \in Nat:
         AdequateLeaderRetainedProducerEpisodeAtRank(
           request, node, cutoffOrdinal, sourceRank,
           target, leaderContext, leader, leaderView,
           subject, known, budget)
           ~> AdequateLeaderRetainedProducerExactRankGoal(
                request, node, cutoffOrdinal, sourceRank, target)

THEOREM AdequateLeaderFiniteRetainedProducerBudgetClosesNonDescentEpisode ==
  \A specification:
    AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty(specification)
      => AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty(
           specification)
BY NatLessThanWellFounded, WellFoundedLeadsTo
   DEF AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty,
       AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty,
       AdequateLeaderRetainedProducerRankOrKnownAdvanceGoal,
       AdequateLeaderRetainedProducerActualKnownAdvanceGoal,
       AdequateLeaderRetainedProducerExactRankGoal,
       SetLessThan, OpToRel

=============================================================================

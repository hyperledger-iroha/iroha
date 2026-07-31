---- MODULE SumeragiV2AsyncFiniteProducerEpisodes ----
EXTENDS SumeragiV2AsyncCausalWorkBudgetProofs

(***************************************************************************
Async specialization of the finite producer-episode kernel.

This module is the source-qualified composition boundary for identities and
transitions already exposed by `SumeragiV2AsyncNetwork`:

  * the exact timeout vote carried unchanged through Begin, Persist, Sign,
    and Deliver; and
  * the canonical reply semantic identity, physical archive owner, and
    authenticated request source carried by packets, source-indexed ingress,
    retained attempts, semantic Serve jobs, and source-keyed outputs.

Base AsyncNetwork does not model route capability tenure, actor-global route
delivery ordinals, route admission tickets, per-source message/chunk cursors,
Hold/Release, rehydration, or source-specific route cancellation.  Named
refinement operators at the end specify the required kernel mapping for that
future composition, but they are not invented AsyncNetwork transitions and
are not included in `AsyncFiniteProducerNext`.
***************************************************************************)

CONSTANT AsyncFiniteProducerNaturalRankOrdering

(***************************************************************************
Stable semantic request and authenticated source identities.

The timeout semantic is the complete `TimeoutVote` record, including its
context and exact highest PrepareQC.  The reply semantic reuses
`AsyncReplySemanticIdentity`; the archive process remains the separate owner
coordinate, just as it does in `AsyncServeLogicalRequestIdentity`.
***************************************************************************)

AsyncFiniteTimeoutRequest(vote) ==
  [family |-> "Timeout", owner |-> vote.signer, semantic |-> vote]

AsyncFiniteReplyRequest(owner, semantic) ==
  [family |-> "Reply", owner |-> owner, semantic |-> semantic]

AsyncFiniteTimeoutRequests ==
  {AsyncFiniteTimeoutRequest(vote):
     vote \in TimeoutVoteRecordSet}

AsyncFiniteReplyRequests ==
  {AsyncFiniteReplyRequest(owner, semantic):
     owner \in ValidatorIds,
     semantic \in AsyncReplySemanticIdentities}

AsyncFiniteProducerRequests ==
  AsyncFiniteTimeoutRequests \cup AsyncFiniteReplyRequests

AsyncFiniteProducerSources == AsyncAuthenticatedDeliverySources

AsyncFiniteProducerObligation(request, source) ==
  [request |-> request, source |-> source]

AsyncFiniteProducerObligationSet ==
  [request: AsyncFiniteProducerRequests,
   source: AsyncFiniteProducerSources]

AsyncFiniteTimeoutObligation(vote) ==
  AsyncFiniteProducerObligation(
    AsyncFiniteTimeoutRequest(vote), vote.signer)

AsyncFiniteReplyObligation(owner, semantic, authenticatedSource) ==
  AsyncFiniteProducerObligation(
    AsyncFiniteReplyRequest(owner, semantic), authenticatedSource)

AsyncFiniteReplyObligationVia(item, authenticatedSource) ==
  AsyncFiniteReplyObligation(
    item.envelope.recipient,
    AsyncReplySemanticIdentity(
      item.kind, item.source, item.envelope),
    authenticatedSource)

AsyncFiniteReplyAttemptObligation(attempt) ==
  AsyncFiniteReplyObligation(
    attempt.key.owner, attempt.key.identity.request,
    attempt.key.source)

(***************************************************************************
Source-qualified physical keys.

The semantic `DeliveryCandidate(item)` and the single semantic Serve job stay
unchanged.  This ghost projection instead recovers the authenticated source
from the carrier which actually owns it: a packet's outer authentication, an
ingress lane coordinate, an `asyncServeAttempts` key, or a source-keyed
output.  Multiple carriers and exact same-source retries collapse to one key;
an alternate authenticated source produces a different key.
***************************************************************************)

AsyncFiniteReplyPhysicalSourceKey(
    owner, semantic, authenticatedSource) ==
  [owner |-> owner, semantic |-> semantic,
   source |-> authenticatedSource]

AsyncFiniteReplyPhysicalSourceKeySet ==
  [owner: ValidatorIds,
   semantic: AsyncReplySemanticIdentities,
   source: AsyncAuthenticatedDeliverySources]

AsyncFiniteReplyObligationForPhysicalKey(key) ==
  AsyncFiniteReplyObligation(
    key.owner, key.semantic, key.source)

AsyncFiniteReplyPhysicalKeyForObligation(obligation) ==
  AsyncFiniteReplyPhysicalSourceKey(
    obligation.request.owner,
    obligation.request.semantic,
    obligation.source)

AsyncFiniteReplyPacketCarriers ==
  {packet \in asyncTransport:
     /\ packet.item.kind \in AsyncReplyRequestKinds
     /\ packet.item.envelope.recipient \in ValidatorIds
     /\ packet.authenticatedSource \in AsyncAuthenticatedDeliverySources
     /\ AsyncReplySemanticIdentity(
          packet.item.kind,
          packet.item.source,
          packet.item.envelope)
          \in AsyncReplySemanticIdentities}

AsyncFiniteReplyPacketPhysicalKey(packet) ==
  AsyncFiniteReplyPhysicalSourceKey(
    packet.item.envelope.recipient,
    AsyncReplySemanticIdentity(
      packet.item.kind, packet.item.source, packet.item.envelope),
    packet.authenticatedSource)

AsyncFiniteReplyPacketPhysicalKeys ==
  {AsyncFiniteReplyPacketPhysicalKey(packet):
     packet \in AsyncFiniteReplyPacketCarriers}

AsyncFiniteReplyIngressCarriers ==
  UNION {
    {[
       owner |-> owner,
       source |-> source,
       item |-> asyncIngressLanes[owner][source][index]
     ]:
       index \in
         {candidate \in 1..Len(asyncIngressLanes[owner][source]):
            LET item == asyncIngressLanes[owner][source][candidate]
            IN /\ item.kind \in AsyncReplyRequestKinds
               /\ AsyncReplySemanticIdentity(
                    item.kind, item.source, item.envelope)
                    \in AsyncReplySemanticIdentities}}:
    owner \in ValidatorIds,
    source \in AsyncAuthenticatedDeliverySources}

AsyncFiniteReplyIngressPhysicalKey(carrier) ==
  AsyncFiniteReplyPhysicalSourceKey(
    carrier.owner,
    AsyncReplySemanticIdentity(
      carrier.item.kind,
      carrier.item.source,
      carrier.item.envelope),
    carrier.source)

AsyncFiniteReplyIngressPhysicalKeys ==
  {AsyncFiniteReplyIngressPhysicalKey(carrier):
     carrier \in AsyncFiniteReplyIngressCarriers}

AsyncFiniteReplyAttemptCarriers ==
  {attempt \in asyncServeAttempts:
     /\ attempt.key.owner \in ValidatorIds
     /\ attempt.key.identity.request
          \in AsyncReplySemanticIdentities
     /\ attempt.key.source \in AsyncAuthenticatedDeliverySources}

AsyncFiniteReplyAttemptPhysicalKey(attempt) ==
  AsyncFiniteReplyPhysicalSourceKey(
    attempt.key.owner,
    attempt.key.identity.request,
    attempt.key.source)

AsyncFiniteReplyAttemptPhysicalKeys ==
  {AsyncFiniteReplyAttemptPhysicalKey(attempt):
     attempt \in AsyncFiniteReplyAttemptCarriers}

(***************************************************************************
A semantic Serve candidate/job maps to the set of matching retained source
attempts.  The candidate is deliberately not widened with a route field.
***************************************************************************)

AsyncFiniteReplyCandidatePhysicalKeys(node, candidate) ==
  {AsyncFiniteReplyAttemptPhysicalKey(attempt):
     attempt \in
       {matching \in AsyncFiniteReplyAttemptCarriers:
          /\ candidate.item.kind \in AsyncReplyRequestKinds
          /\ matching.key.owner = node
          /\ matching.key.identity =
               AsyncServeLogicalRequestIdentity(
                 node, candidate.item)}}

AsyncFiniteReplyServeJobPhysicalKeys ==
  UNION {
    UNION {
      AsyncFiniteReplyCandidatePhysicalKeys(
        node, asyncIoQueues[node][index].candidate):
      index \in AsyncIoServeIndices(asyncIoQueues[node])}:
    node \in ValidatorIds}

(***************************************************************************
Base tombstone outputs are deliberately route-neutral network items.  Their
authenticated source is retained only by `asyncServeAttempts`; manufacturing
a source-keyed output record here would reintroduce the provenance defect.
The reply-route product owns the positive-output/ticket edge, so this base
specialization exposes no emitted ReplyComplete episode.
***************************************************************************)
AsyncFiniteReplyOutputPhysicalKeys == {}
AsyncFiniteReplyEmittedCompletionPhysicalKeys == {}
AsyncFiniteReplyEmittedCompletionObligations == {}

AsyncFiniteProducerSourceCompatible(obligation) ==
  IF obligation.request.family = "Timeout"
  THEN obligation.source = obligation.request.semantic.signer
  ELSE obligation.request.family = "Reply"

(***************************************************************************
Finite stage and cursor carriers.

Timeout cursors are exact validator recipients.  Reply cursors mirror the
route ownership model's two message positions and configured chunk positions;
the terminal cursor values are retained because they are observable route
states even though base AsyncNetwork cannot advance them.
***************************************************************************)

AsyncFiniteProducerStages ==
  {"Observe", "Begin", "Persist", "Sign", "Deliver",
   "ReplyObserve", "ReplyMessage", "ReplyChunk", "ReplyComplete"}

AsyncFiniteProducerCursor(kind, index) ==
  [kind |-> kind, index |-> index]

AsyncFiniteValidatorCursor(node) ==
  AsyncFiniteProducerCursor("Validator", node)

AsyncFiniteReplyMessageCursor(index) ==
  AsyncFiniteProducerCursor("ReplyMessage", index)

AsyncFiniteReplyChunkCursor(index) ==
  AsyncFiniteProducerCursor("ReplyChunk", index)

AsyncFiniteReplyMessagePositions == 0..2
AsyncFiniteReplyChunkPositions == 0..AsyncChunkCount

AsyncFiniteProducerCursors ==
  {AsyncFiniteValidatorCursor(node): node \in ValidatorIds}
  \cup
  {AsyncFiniteReplyMessageCursor(index):
     index \in AsyncFiniteReplyMessagePositions}
  \cup
  {AsyncFiniteReplyChunkCursor(index):
     index \in AsyncFiniteReplyChunkPositions}

AsyncFiniteProducerEpisode(obligation, stage, cursor) ==
  [obligation |-> obligation, stage |-> stage, cursor |-> cursor]

AsyncFiniteTimeoutRecipients(vote) ==
  VotingRoster(vote.context.epoch)

AsyncFiniteTimeoutEpisodeUniverse(obligation) ==
  LET sourceCursor == AsyncFiniteValidatorCursor(obligation.source)
      vote == obligation.request.semantic
  IN
    {AsyncFiniteProducerEpisode(obligation, "Observe", sourceCursor),
     AsyncFiniteProducerEpisode(obligation, "Begin", sourceCursor),
     AsyncFiniteProducerEpisode(obligation, "Persist", sourceCursor)}
    \cup
    {AsyncFiniteProducerEpisode(
       obligation, "Sign", AsyncFiniteValidatorCursor(recipient)):
       recipient \in AsyncFiniteTimeoutRecipients(vote)}
    \cup
    {AsyncFiniteProducerEpisode(
       obligation, "Deliver", AsyncFiniteValidatorCursor(recipient)):
       recipient \in AsyncFiniteTimeoutRecipients(vote)}

AsyncFiniteReplyEpisodeUniverse(obligation) ==
  LET sourceCursor == AsyncFiniteValidatorCursor(obligation.source)
  IN
    {AsyncFiniteProducerEpisode(
       obligation, "ReplyObserve", sourceCursor),
     AsyncFiniteProducerEpisode(
       obligation, "ReplyComplete", sourceCursor)}
    \cup
    {AsyncFiniteProducerEpisode(
       obligation, "ReplyMessage",
       AsyncFiniteReplyMessageCursor(index)):
       index \in AsyncFiniteReplyMessagePositions}
    \cup
    {AsyncFiniteProducerEpisode(
       obligation, "ReplyChunk",
       AsyncFiniteReplyChunkCursor(index)):
       index \in AsyncFiniteReplyChunkPositions}

AsyncFiniteProducerEpisodeUniverse(obligation) ==
  IF obligation.request.family = "Timeout"
  THEN AsyncFiniteTimeoutEpisodeUniverse(obligation)
  ELSE AsyncFiniteReplyEpisodeUniverse(obligation)

AsyncFiniteProducerInitialEpisodes(obligation) ==
  LET stage ==
        IF obligation.request.family = "Timeout"
        THEN "Observe"
        ELSE "ReplyObserve"
  IN
    \* The validator cursor labels the authenticated observation source.  It
    \* consumes no ReplyMessage/ReplyChunk episode; route delivery starts at
    \* the separate per-attempt <<messageCursor, chunkCursor>> = <<0, 0>>.
    {AsyncFiniteProducerEpisode(
       obligation, stage,
       AsyncFiniteValidatorCursor(obligation.source))}

AsyncFiniteProducerEpisodePredecessors(episode) ==
  LET obligation == episode.obligation
      sourceCursor == AsyncFiniteValidatorCursor(obligation.source)
      sameCursor == episode.cursor
  IN
    CASE episode.stage = "Begin" ->
           {AsyncFiniteProducerEpisode(
              obligation, "Observe", sourceCursor)}
      [] episode.stage = "Persist" ->
           {AsyncFiniteProducerEpisode(
              obligation, "Begin", sourceCursor)}
      [] episode.stage = "Sign" ->
           {AsyncFiniteProducerEpisode(
              obligation, "Persist", sourceCursor)}
      [] episode.stage = "Deliver" ->
           {AsyncFiniteProducerEpisode(
              obligation, "Sign", sameCursor)}
      [] episode.stage \in {"ReplyMessage", "ReplyChunk"} ->
           {AsyncFiniteProducerEpisode(
              obligation, "ReplyObserve", sourceCursor)}
      [] episode.stage = "ReplyComplete" ->
           {AsyncFiniteProducerEpisode(
              obligation, "ReplyObserve", sourceCursor)}
             \cup
           {AsyncFiniteProducerEpisode(
              obligation, "ReplyMessage",
              AsyncFiniteReplyMessageCursor(index)):
              index \in AsyncFiniteReplyMessagePositions}
             \cup
           {AsyncFiniteProducerEpisode(
              obligation, "ReplyChunk",
              AsyncFiniteReplyChunkCursor(index)):
              index \in AsyncFiniteReplyChunkPositions}
      [] OTHER -> {}

(***************************************************************************
Source-connected timeout observations.

`timeoutIntents` is the durable monotone anchor.  Pending WAL and signing
owners retain the exact same vote.  Signature completion is recognized only
when the local receipt and every exact authenticated outbox occurrence are
visible; crash teardown alone therefore cannot manufacture a Sign episode.
Deliver episodes are exact `TimeoutVoteAt(recipient, vote)` receipts.
***************************************************************************)

AsyncFiniteTrackedTimeoutVotes ==
  {request.vote: request \in pendingTimeout}
    \cup timeoutIntents
    \cup {request.vote: request \in signTimeouts}

AsyncFiniteTimeoutSentItem(vote, recipient) ==
  AsyncNetworkItem(
    "TimeoutVote", vote.signer, TimeoutEnvelope(recipient, vote))

AsyncFiniteTimeoutSignatureComplete(vote) ==
  /\ TimeoutVoteAt(vote.signer, vote) \in receivedTimeoutVotes
  /\ \A recipient
       \in AsyncFiniteTimeoutRecipients(vote) \ {vote.signer}:
       AsyncFiniteTimeoutSentItem(vote, recipient) \in asyncSentItems

AsyncFiniteTimeoutDeliveredRecipients(vote) ==
  {recipient \in AsyncFiniteTimeoutRecipients(vote):
     TimeoutVoteAt(recipient, vote) \in receivedTimeoutVotes}

AsyncFiniteTimeoutVisibleEpisodes(vote) ==
  LET obligation == AsyncFiniteTimeoutObligation(vote)
      sourceCursor == AsyncFiniteValidatorCursor(vote.signer)
  IN
    {AsyncFiniteProducerEpisode(
       obligation, "Observe", sourceCursor),
     AsyncFiniteProducerEpisode(
       obligation, "Begin", sourceCursor)}
    \cup
    IF vote \in timeoutIntents
    THEN
      {AsyncFiniteProducerEpisode(
         obligation, "Persist", sourceCursor)}
      \cup
      IF AsyncFiniteTimeoutSignatureComplete(vote)
      THEN
        {AsyncFiniteProducerEpisode(
           obligation, "Sign", AsyncFiniteValidatorCursor(recipient)):
           recipient \in AsyncFiniteTimeoutRecipients(vote)}
        \cup
        {AsyncFiniteProducerEpisode(
           obligation, "Deliver",
           AsyncFiniteValidatorCursor(recipient)):
           recipient \in AsyncFiniteTimeoutDeliveredRecipients(vote)}
      ELSE {}
    ELSE {}

(***************************************************************************
Source-connected reply observations available in base AsyncNetwork.

The physical carrier union remains a diagnostic projection.  Ownership is
derived from the base monotone consumed journal, not from volatile packet or
lane residence.  Only `ReplyObserve` is consumed at this boundary; the
positive output/ticket edge belongs to ReplyRoutes.  Exact retransmission is
therefore a journal stutter even after every volatile carrier disappears.
***************************************************************************)

AsyncFiniteObservedReplyAttempts ==
  AsyncFiniteReplyAttemptCarriers

AsyncFiniteReplyPhysicalSourceKeys ==
  AsyncFiniteReplyPacketPhysicalKeys
    \cup AsyncFiniteReplyIngressPhysicalKeys
    \cup AsyncFiniteReplyAttemptPhysicalKeys
    \cup AsyncFiniteReplyServeJobPhysicalKeys
    \cup AsyncFiniteReplyOutputPhysicalKeys

AsyncFiniteBaseReplyIngressEpisodes ==
  {episode \in asyncProducerConsumedEpisodes:
     /\ episode.request \in AsyncServeLogicalRequestIdentities
     /\ episode.authenticatedSource
          \in AsyncAuthenticatedDeliverySources}

AsyncFiniteReplyObligationForBaseEpisode(episode) ==
  AsyncFiniteReplyObligation(
    episode.request.owner,
    episode.request.request,
    episode.authenticatedSource)

AsyncFiniteObservedReplyObligations ==
  {AsyncFiniteReplyObligationForBaseEpisode(episode):
     episode \in AsyncFiniteBaseReplyIngressEpisodes}

AsyncFiniteProducerObservedObligations ==
  {AsyncFiniteTimeoutObligation(vote):
     vote \in AsyncFiniteTrackedTimeoutVotes}
    \cup AsyncFiniteObservedReplyObligations

AsyncFiniteProducerVisibleEpisodes ==
  UNION
    {AsyncFiniteTimeoutVisibleEpisodes(vote):
       vote \in AsyncFiniteTrackedTimeoutVotes}
  \cup
  {AsyncFiniteProducerEpisode(
     AsyncFiniteReplyObligationForBaseEpisode(episode),
     "ReplyObserve",
     AsyncFiniteValidatorCursor(episode.authenticatedSource)):
     episode \in AsyncFiniteBaseReplyIngressEpisodes}

(***************************************************************************
Opaque current Async rank snapshot.

The carrier stores one natural-valued scheduler, stage, and cursor projection
per stable obligation.  The current snapshot is source-connected where base
AsyncNetwork exposes state: scheduler debt uses the exact local queue owner
(the timeout signer or reply archive owner), timeout stage debt uses
WAL/intent/signature state, and timeout cursor debt counts exact missing
receipts.  Reply stage/cursor values deliberately stop at the observed-request
boundary pending the separate route composition.
***************************************************************************)

AsyncFiniteProducerRankStateSet ==
  [scheduler:
     [AsyncFiniteProducerObligationSet -> Nat],
   stage:
     [AsyncFiniteProducerObligationSet -> Nat],
   cursor:
     [AsyncFiniteProducerObligationSet -> Nat]]

AsyncFiniteProducerInitialRankState ==
  [scheduler |->
     [obligation \in AsyncFiniteProducerObligationSet |-> 0],
   stage |->
     [obligation \in AsyncFiniteProducerObligationSet |-> 0],
   cursor |->
     [obligation \in AsyncFiniteProducerObligationSet |-> 0]]

AsyncFiniteProducerSchedulerNode(obligation) ==
  IF obligation.request.family = "Timeout"
  THEN obligation.source
  ELSE obligation.request.owner

AsyncFiniteProducerSchedulerDebt(obligation) ==
  LET node == AsyncFiniteProducerSchedulerNode(obligation)
  IN Len(asyncCommandQueues[node])
       + Len(asyncCausalQueues[node])
       + AsyncIoQueueDepth(node)
       + Len(asyncDeferredCompletionQueues[node])
       + Len(asyncDeferredProgressQueues[node])
       + Len(asyncDeferredNormalQueues[node])

AsyncFiniteTimeoutStageDebt(vote) ==
  IF vote \notin timeoutIntents
  THEN 3
  ELSE IF ~AsyncFiniteTimeoutSignatureComplete(vote)
       THEN 2
       ELSE IF AsyncFiniteTimeoutDeliveredRecipients(vote)
                 # AsyncFiniteTimeoutRecipients(vote)
            THEN 1
            ELSE 0

AsyncFiniteProducerStageDebt(obligation) ==
  IF obligation.request.family = "Timeout"
  THEN AsyncFiniteTimeoutStageDebt(obligation.request.semantic)
  ELSE IF obligation \in AsyncFiniteObservedReplyObligations
       THEN 1
       ELSE 0

AsyncFiniteProducerCursorDebt(obligation) ==
  IF obligation.request.family = "Timeout"
  THEN Cardinality(
         AsyncFiniteTimeoutRecipients(obligation.request.semantic)
           \ AsyncFiniteTimeoutDeliveredRecipients(
               obligation.request.semantic))
  ELSE 0

VARIABLES
  asyncFiniteProducerKnownObligations,
  asyncFiniteProducerConsumedEpisodes,
  asyncFiniteProducerRankState

AsyncFiniteProducerVars ==
  <<asyncFiniteProducerKnownObligations,
    asyncFiniteProducerConsumedEpisodes,
    asyncFiniteProducerRankState>>

AsyncFiniteProducerCurrentRankState ==
  [scheduler |->
     [obligation \in AsyncFiniteProducerObligationSet |->
        IF obligation \in asyncFiniteProducerKnownObligations
        THEN AsyncFiniteProducerSchedulerDebt(obligation)
        ELSE 0],
   stage |->
     [obligation \in AsyncFiniteProducerObligationSet |->
        IF obligation \in asyncFiniteProducerKnownObligations
        THEN AsyncFiniteProducerStageDebt(obligation)
        ELSE 0],
   cursor |->
     [obligation \in AsyncFiniteProducerObligationSet |->
        IF obligation \in asyncFiniteProducerKnownObligations
        THEN AsyncFiniteProducerCursorDebt(obligation)
        ELSE 0]]

AsyncFiniteProducerSchedulerRank(rankState, obligation) ==
  rankState.scheduler[obligation]

AsyncFiniteProducerStageRank(rankState, obligation) ==
  rankState.stage[obligation]

AsyncFiniteProducerCursorRank(rankState, obligation) ==
  rankState.cursor[obligation]

(***************************************************************************
Explicit generic-kernel instantiation.
***************************************************************************)

AsyncFiniteProducerKernel ==
  INSTANCE SumeragiV2FiniteProducerEpisodes WITH
    ProducerRequests <- AsyncFiniteProducerRequests,
    ProducerSources <- AsyncFiniteProducerSources,
    ProducerSourceOrder <- AsyncReplySourceOrder,
    ProducerSourceCapacity <- AsyncReplySourceCapacity,
    ProducerStages <- AsyncFiniteProducerStages,
    ProducerCursors <- AsyncFiniteProducerCursors,
    ProducerEpisodeUniverse <- AsyncFiniteProducerEpisodeUniverse,
    ProducerInitialEpisodes <- AsyncFiniteProducerInitialEpisodes,
    ProducerEpisodePredecessors <-
      AsyncFiniteProducerEpisodePredecessors,
    ProducerRankStates <- AsyncFiniteProducerRankStateSet,
    ProducerInitialRankState <- AsyncFiniteProducerInitialRankState,
    ProducerSchedulerRankCarrier <- Nat,
    ProducerSchedulerRankOrdering <-
      AsyncFiniteProducerNaturalRankOrdering,
    ProducerSchedulerRank <- AsyncFiniteProducerSchedulerRank,
    ProducerStageRankCarrier <- Nat,
    ProducerStageRankOrdering <-
      AsyncFiniteProducerNaturalRankOrdering,
    ProducerStageRank <- AsyncFiniteProducerStageRank,
    ProducerCursorRankCarrier <- Nat,
    ProducerCursorRankOrdering <-
      AsyncFiniteProducerNaturalRankOrdering,
    ProducerCursorRank <- AsyncFiniteProducerCursorRank,
    fpKnownObligations <- asyncFiniteProducerKnownObligations,
    fpConsumedEpisodes <- asyncFiniteProducerConsumedEpisodes,
    fpRankState <- asyncFiniteProducerRankState

(***************************************************************************
Composition with the existing candidate-episode budget.

The ingress component is the finite source/request universe minus the base
consumed journal.  The tail is the existing exact remaining-candidate
occurrence budget at a frozen scheduler cutoff;
`AsyncCandidateProducerEpisodeBudget` is its already-proved bound and radix,
not a new producer limit or runtime knob.  A first distinct ingress episode
may decrease the leading component.  Exact retransmission preserves it and
is a separate proofless non-descent episode; it is never called progress.
***************************************************************************)
AsyncFiniteCandidateEpisodeRadix ==
  AsyncCandidateProducerEpisodeBudget + 1

AsyncFiniteCandidateEpisodeTailRank(node, cutoffOrdinal) ==
  AsyncCausalEpisodeExactCandidateOccurrenceBudget(
    node, cutoffOrdinal)

AsyncFiniteCandidateEpisodeTailTypeInvariant ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    AsyncFiniteCandidateEpisodeTailRank(node, cutoffOrdinal)
      \in 0..AsyncCandidateProducerEpisodeBudget

AsyncFiniteIngressCandidateCompositeRank(node, cutoffOrdinal) ==
  AsyncProducerRemainingIngressRank
    * AsyncFiniteCandidateEpisodeRadix
    + AsyncFiniteCandidateEpisodeTailRank(node, cutoffOrdinal)

AsyncFiniteIngressCandidateCompositeRankFor(
    request, node, cutoffOrdinal) ==
  AsyncProducerRemainingIngressRankFor(request)
    * AsyncFiniteCandidateEpisodeRadix
    + AsyncFiniteCandidateEpisodeTailRank(node, cutoffOrdinal)

THEOREM AsyncFiniteStrongTypeBoundsCandidateEpisodeTail ==
  AsyncStrongTypeInvariant
    => AsyncFiniteCandidateEpisodeTailTypeInvariant
BY AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode, Isa
   DEF AsyncFiniteCandidateEpisodeTailTypeInvariant,
       AsyncFiniteCandidateEpisodeTailRank

THEOREM AsyncFiniteFirstDistinctIngressDominatesCandidateEpisodeTail ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncConfiguration
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerFirstDistinctIngressEpisodeStep
    /\ AsyncFiniteCandidateEpisodeTailTypeInvariant
    /\ AsyncFiniteCandidateEpisodeTailTypeInvariant'
    => AsyncFiniteIngressCandidateCompositeRank(
         node, cutoffOrdinal)'
         < AsyncFiniteIngressCandidateCompositeRank(
             node, cutoffOrdinal)
BY AsyncProducerFirstDistinctEpisodeStrictlyConsumesFiniteRank, Isa
   DEF AsyncFiniteIngressCandidateCompositeRank,
       AsyncFiniteCandidateEpisodeRadix,
       AsyncFiniteCandidateEpisodeTailTypeInvariant

THEOREM AsyncFiniteFirstDistinctTargetIngressDominatesCandidateEpisodeTail ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncConfiguration
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerFirstDistinctIngressEpisodeStepFor(request)
    /\ AsyncFiniteCandidateEpisodeTailTypeInvariant
    /\ AsyncFiniteCandidateEpisodeTailTypeInvariant'
    => AsyncFiniteIngressCandidateCompositeRankFor(
         request, node, cutoffOrdinal)'
         < AsyncFiniteIngressCandidateCompositeRankFor(
             request, node, cutoffOrdinal)
BY AsyncProducerFirstDistinctEpisodeForStrictlyConsumesFiniteRank, Isa
   DEF AsyncFiniteIngressCandidateCompositeRankFor,
       AsyncFiniteCandidateEpisodeRadix,
       AsyncFiniteCandidateEpisodeTailTypeInvariant

THEOREM AsyncFiniteExactRetransmissionPreservesCompositeUnderTailFrame ==
  \A node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerExactRetransmissionEpisodeStep
    /\ AsyncFiniteCandidateEpisodeTailRank(
         node, cutoffOrdinal)' =
         AsyncFiniteCandidateEpisodeTailRank(
           node, cutoffOrdinal)
    => AsyncFiniteIngressCandidateCompositeRank(
         node, cutoffOrdinal)' =
         AsyncFiniteIngressCandidateCompositeRank(
           node, cutoffOrdinal)
BY AsyncProducerExactRetransmissionIsJournalStutter, Isa
   DEF AsyncFiniteIngressCandidateCompositeRank

THEOREM AsyncFiniteExactTargetRetransmissionPreservesCompositeUnderTailFrame ==
  \A request \in AsyncProducerIngressRequests,
     node \in ValidatorIds, cutoffOrdinal \in Nat:
    /\ AsyncProducerJournalClosed
    /\ AsyncProducerProjectionStep
    /\ AsyncProducerExactRetransmissionEpisodeStepFor(request)
    /\ AsyncFiniteCandidateEpisodeTailRank(
         node, cutoffOrdinal)' =
         AsyncFiniteCandidateEpisodeTailRank(
           node, cutoffOrdinal)
    => AsyncFiniteIngressCandidateCompositeRankFor(
         request, node, cutoffOrdinal)' =
         AsyncFiniteIngressCandidateCompositeRankFor(
           request, node, cutoffOrdinal)
BY AsyncProducerExactRetransmissionPreservesTargetIngressRank, Isa
   DEF AsyncFiniteIngressCandidateCompositeRankFor

(***************************************************************************
Coupled initialization, deterministic projection step, and spec skeleton.

Every Async step journals exactly the obligations and episodes visible in its
post-state.  The union equations make replay, retransmission, and duplicate
delivery journal stutters.  They also preserve consumed history across crash
teardown.  No route-only episode can appear because base AsyncNetwork exposes
no route cursor transition.
***************************************************************************)

AsyncFiniteProducerInit ==
  /\ AsyncInit
  /\ AsyncFiniteProducerNaturalRankOrdering =
       {pair \in Nat \X Nat: pair[1] < pair[2]}
  /\ AsyncFiniteProducerKernel!ProducerInit
  /\ asyncFiniteProducerRankState =
       AsyncFiniteProducerCurrentRankState

AsyncFiniteProducerObserveEpisodeDelta ==
  {episode \in AsyncFiniteProducerVisibleEpisodes':
     episode.stage \in {"Observe", "ReplyObserve"}}
    \ asyncFiniteProducerConsumedEpisodes

AsyncFiniteReplyFreshObservationObligationDelta ==
  AsyncFiniteObservedReplyObligations'
    \ asyncFiniteProducerKnownObligations

AsyncFiniteReplyTransferredObservationObligations ==
  AsyncFiniteObservedReplyObligations'
    \cap asyncFiniteProducerKnownObligations

AsyncFiniteReplyObservationEpisodeDelta ==
  {episode \in AsyncFiniteProducerVisibleEpisodes':
     episode.stage = "ReplyObserve"}
    \ asyncFiniteProducerConsumedEpisodes

AsyncFiniteTimeoutBeginEpisodeDelta ==
  {episode \in AsyncFiniteProducerVisibleEpisodes':
     episode.stage = "Begin"}
    \ asyncFiniteProducerConsumedEpisodes

AsyncFiniteTimeoutPersistEpisodeDelta ==
  {episode \in AsyncFiniteProducerVisibleEpisodes':
     episode.stage = "Persist"}
    \ asyncFiniteProducerConsumedEpisodes

AsyncFiniteTimeoutSignEpisodeDelta ==
  {episode \in AsyncFiniteProducerVisibleEpisodes':
     episode.stage = "Sign"}
    \ asyncFiniteProducerConsumedEpisodes

AsyncFiniteTimeoutDeliverEpisodeDelta ==
  {episode \in AsyncFiniteProducerVisibleEpisodes':
     episode.stage = "Deliver"}
    \ asyncFiniteProducerConsumedEpisodes

AsyncFiniteProducerDeterministicEpisodeDelta ==
  AsyncFiniteProducerObserveEpisodeDelta
    \cup AsyncFiniteTimeoutBeginEpisodeDelta
    \cup AsyncFiniteTimeoutPersistEpisodeDelta
    \cup AsyncFiniteTimeoutSignEpisodeDelta
    \cup AsyncFiniteTimeoutDeliverEpisodeDelta

AsyncFiniteProducerProjectionStep ==
  /\ asyncFiniteProducerKnownObligations' =
       asyncFiniteProducerKnownObligations
         \cup AsyncFiniteProducerObservedObligations'
  /\ asyncFiniteProducerConsumedEpisodes' =
       asyncFiniteProducerConsumedEpisodes
         \cup AsyncFiniteProducerDeterministicEpisodeDelta
  /\ asyncFiniteProducerRankState' =
       AsyncFiniteProducerCurrentRankState'

AsyncFiniteProducerNext ==
  /\ AsyncNext
  /\ AsyncFiniteProducerProjectionStep

AsyncFiniteProducerAllVars ==
  <<AsyncAllVars, AsyncFiniteProducerVars>>

AsyncFiniteProducerSpec ==
  /\ AsyncFiniteProducerInit
  /\ [][AsyncFiniteProducerNext]_AsyncFiniteProducerAllVars
  /\ AsyncFairness

(***************************************************************************
Source-connected safety and refinement predicates.  These are declarations,
not unproved theorem claims.
***************************************************************************)

AsyncFiniteProducerIdentityInvariant ==
  /\ asyncFiniteProducerKnownObligations
       \subseteq AsyncFiniteProducerObligationSet
  /\ \A obligation \in asyncFiniteProducerKnownObligations:
       AsyncFiniteProducerSourceCompatible(obligation)

AsyncFiniteProducerObservationInvariant ==
  /\ AsyncFiniteProducerObservedObligations
       \subseteq asyncFiniteProducerKnownObligations
  /\ AsyncFiniteProducerVisibleEpisodes
       \subseteq asyncFiniteProducerConsumedEpisodes

AsyncFiniteBaseReplyJournalProjectionInvariant ==
  \A episode \in AsyncFiniteBaseReplyIngressEpisodes:
    LET obligation ==
          AsyncFiniteReplyObligationForBaseEpisode(episode)
        observed ==
          AsyncFiniteProducerEpisode(
            obligation, "ReplyObserve",
            AsyncFiniteValidatorCursor(
              episode.authenticatedSource))
    IN /\ obligation \in asyncFiniteProducerKnownObligations
       /\ observed \in asyncFiniteProducerConsumedEpisodes

AsyncFiniteProducerRankSnapshotInvariant ==
  asyncFiniteProducerRankState =
    AsyncFiniteProducerCurrentRankState

AsyncFiniteProducerSafety ==
  /\ AsyncFiniteProducerKernel!ProducerSafetyInvariant
  /\ AsyncProducerTypeInvariant
  /\ AsyncFiniteProducerIdentityInvariant
  /\ AsyncFiniteProducerObservationInvariant
  /\ AsyncFiniteBaseReplyJournalProjectionInvariant
  /\ AsyncFiniteProducerRankSnapshotInvariant
  /\ AsyncFiniteCandidateEpisodeTailTypeInvariant

AsyncFiniteProducerStepSafety ==
  /\ AsyncFiniteProducerProjectionStep
  /\ AsyncFiniteProducerKernel!ProducerJournalStepInvariant

THEOREM AsyncFiniteProducerNextProjectsBothMonotoneJournals ==
  AsyncFiniteProducerNext
    => /\ AsyncProducerProjectionStep
       /\ AsyncFiniteProducerProjectionStep
       /\ AsyncProducerJournalMonotoneStep
BY Isa
   DEF AsyncFiniteProducerNext, AsyncNext,
       AsyncProducerProjectionStep,
       AsyncProducerJournalMonotoneStep

(***************************************************************************
Named route-refinement operators.

These operators define the exact ghost action a future
`SumeragiV2AsyncNetworkReplyRoutes` transition must imply.  They are not part
of `AsyncFiniteProducerNext`, because AsyncNetwork has no capability, tenure,
ticket, Hold/Release, rehydration, or per-source route cursor state.
***************************************************************************)

AsyncFiniteReplyRequestIdentity(owner, semantic) ==
  AsyncFiniteReplyRequest(owner, semantic)

AsyncFiniteReplyRouteObligation(owner, semantic, source) ==
  AsyncFiniteProducerObligation(
    AsyncFiniteReplyRequestIdentity(owner, semantic), source)

AsyncFiniteReplyObserveNewSourceRefinement(
    owner, semantic, source) ==
  LET request == AsyncFiniteReplyRequestIdentity(owner, semantic)
  IN
    /\ owner \in ValidatorIds
    /\ semantic \in AsyncReplySemanticIdentities
    /\ source \in AsyncAuthenticatedDeliverySources
    /\ AsyncFiniteProducerKernel!ObserveNewProducerSource(
         request, source, AsyncFiniteProducerCurrentRankState')

AsyncFiniteReplyRefreshSourceRefinement(
    owner, semantic, source) ==
  LET obligation ==
        AsyncFiniteReplyRouteObligation(owner, semantic, source)
  IN
    /\ owner \in ValidatorIds
    /\ semantic \in AsyncReplySemanticIdentities
    /\ source \in AsyncAuthenticatedDeliverySources
    /\ AsyncFiniteProducerKernel!TransferProducerObligation(
         obligation, AsyncFiniteProducerCurrentRankState')

AsyncFiniteReplyConsumeMessageRefinement(
    owner, semantic, source, messageCursor) ==
  LET obligation ==
        AsyncFiniteReplyRouteObligation(owner, semantic, source)
      episode ==
        AsyncFiniteProducerEpisode(
          obligation, "ReplyMessage",
          AsyncFiniteReplyMessageCursor(messageCursor))
  IN
    /\ owner \in ValidatorIds
    /\ semantic \in AsyncReplySemanticIdentities
    /\ source \in AsyncAuthenticatedDeliverySources
    /\ messageCursor \in AsyncFiniteReplyMessagePositions
    /\ AsyncFiniteProducerKernel!ConsumeProducerEpisodes(
         obligation, {episode}, AsyncFiniteProducerCurrentRankState')

AsyncFiniteReplyConsumeChunkRefinement(
    owner, semantic, source, chunkCursor) ==
  LET obligation ==
        AsyncFiniteReplyRouteObligation(owner, semantic, source)
      episode ==
        AsyncFiniteProducerEpisode(
          obligation, "ReplyChunk",
          AsyncFiniteReplyChunkCursor(chunkCursor))
  IN
    /\ owner \in ValidatorIds
    /\ semantic \in AsyncReplySemanticIdentities
    /\ source \in AsyncAuthenticatedDeliverySources
    /\ chunkCursor \in AsyncFiniteReplyChunkPositions
    /\ AsyncFiniteProducerKernel!ConsumeProducerEpisodes(
         obligation, {episode}, AsyncFiniteProducerCurrentRankState')

AsyncFiniteReplyCompleteSourceRefinement(
    owner, semantic, source) ==
  LET obligation ==
        AsyncFiniteReplyRouteObligation(owner, semantic, source)
  IN
    /\ owner \in ValidatorIds
    /\ semantic \in AsyncReplySemanticIdentities
    /\ source \in AsyncAuthenticatedDeliverySources
    /\ AsyncFiniteProducerKernel!CompleteProducerObligation(
         obligation, AsyncFiniteProducerCurrentRankState')

AsyncFiniteReplyCancelExactSourceRefinement(
    owner, semantic, source) ==
  LET obligation ==
        AsyncFiniteReplyRouteObligation(owner, semantic, source)
  IN
    /\ owner \in ValidatorIds
    /\ semantic \in AsyncReplySemanticIdentities
    /\ source \in AsyncAuthenticatedDeliverySources
    /\ AsyncFiniteProducerKernel!CompleteProducerObligation(
         obligation, AsyncFiniteProducerCurrentRankState')

(***************************************************************************
TODO(route composition): connect the six named refinement operators above to
the actual capability/tenure owner, checked delivery ordinal, source-bound
ticket, message/chunk cursor, Hold/Release, rehydration, and cancellation
actions in the separate reply-route composition.  Same-source refresh must
select Transfer without resetting either cursor; a newly authenticated source
must select ObserveNew and start its downstream cursor at item zero.  Exact
source cancellation must select CancelExactSource for only the authenticated
source obligation; it must not consume or reset a sibling obligation.

TODO(rank descent): replace the deliberately terminal reply stage/cursor
projection with the exact route attempt maps, then prove that each stable
incomplete timeout or reply obligation either completes or strictly descends
`AsyncFiniteProducerKernel!ProducerRankOrdering`.  This file claims only the
source-connected safety projection available from base AsyncNetwork.

ACTION CLASSIFICATION (proved): the source-frozen reply composition enumerates
the base Async branch and every real V2 route branch, and
`AsyncReplyFiniteProductionActionClassificationIsExact` proves that inventory
equivalent to `AsyncProductionNext`.  The proof does not infer route delivery
ordinals from `asyncNextServeAdmissionOrdinal`; it remains only the
process-local Serve FIFO ordinal.
***************************************************************************)

=============================================================================

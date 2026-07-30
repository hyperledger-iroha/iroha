---- MODULE SumeragiV2TimeoutDurability ----
EXTENDS SumeragiV2Proofs, FunctionTheorems

(***************************************************************************
Core-only timeout durability frontier.  This module deliberately excludes the
moving asynchronous transport layer: it proves the exact WAL -> durable intent
-> signature -> Core broadcast chain that the scheduler proof must service.
***************************************************************************)

DurableTimeoutVoteAt(node, roundView) ==
  \E vote \in timeoutIntents:
    /\ vote.signer = node
    /\ vote.context = context
    /\ vote.view = roundView

TimeoutRequestPending(node, roundView) ==
  \E request \in pendingTimeout:
    /\ request.node = node
    /\ request.vote.signer = node
    /\ request.vote.context = context
    /\ request.vote.view = roundView

TimeoutSignRequestPending(node, roundView) ==
  \E request \in signTimeouts:
    /\ request.node = node
    /\ request.vote.context = context
    /\ request.vote.view = roundView

TimeoutVotePublishedTo(node, roundView, recipient) ==
  \E envelope \in timeoutNetwork:
    /\ envelope.recipient = recipient
    /\ envelope.vote.signer = node
    /\ envelope.vote.context = context
    /\ envelope.vote.view = roundView

TimeoutSigningProvenanceInvariant ==
  \A request \in signTimeouts:
    request.vote.signer = request.node

THEOREM TimedOutNodeHasDurableTimeoutVote ==
  \A node \in ValidatorIds, roundView \in Views:
    NodeTimedOut(node, roundView)
      => DurableTimeoutVoteAt(node, roundView)
BY DEF NodeTimedOut, DurableTimeoutVoteAt

THEOREM PendingTimeoutRequestFieldsAreTyped ==
  \A request \in pendingTimeout:
    StrongInductiveInvariant
      => /\ request.node \in ValidatorIds
         /\ request.vote \in TimeoutVoteRecordSet
         /\ request.vote.signer = request.node
PROOF
  <1>1. ASSUME NEW request \in pendingTimeout,
                StrongInductiveInvariant
         PROVE /\ request.node \in ValidatorIds
               /\ request.vote \in TimeoutVoteRecordSet
               /\ request.vote.signer = request.node
    <2>1. /\ TypeInvariant
           /\ ReducerProvenanceInvariant
      BY <1>1 DEF StrongInductiveInvariant, Safety
    <2>2. request \in TimeoutWalSet
      BY <1>1, <2>1 DEF TypeInvariant
    <2>3. /\ request.node \in ValidatorIds
           /\ request.vote \in TimeoutVoteRecordSet
      BY <2>2, Isa DEF TimeoutWalSet
    <2>4. request.vote.signer = request.node
      BY <1>1, <2>1
         DEF ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM PendingTimeoutRequestEitherDurableOrCoreEnabled ==
  \A node \in ValidatorIds, roundView \in Views:
    TimeoutRequestPending(node, roundView)
      => \/ DurableTimeoutVoteAt(node, roundView)
         \/ \E request \in pendingTimeout:
              /\ request.node = node
              /\ request.vote.signer = node
              /\ request.vote.context = context
              /\ request.vote.view = roundView
              /\ ENABLED PersistTimeout(request)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW roundView \in Views,
                TimeoutRequestPending(node, roundView)
         PROVE \/ DurableTimeoutVoteAt(node, roundView)
               \/ \E request \in pendingTimeout:
                    /\ request.node = node
                    /\ request.vote.signer = node
                    /\ request.vote.context = context
                    /\ request.vote.view = roundView
                    /\ ENABLED PersistTimeout(request)
    <2>1. PICK request \in pendingTimeout:
             /\ request.node = node
             /\ request.vote.signer = node
             /\ request.vote.context = context
             /\ request.vote.view = roundView
      BY <1>1 DEF TimeoutRequestPending
    <2>2. CASE request.vote \in timeoutIntents
      BY <2>1, <2>2 DEF DurableTimeoutVoteAt
    <2>3. CASE request.vote \notin timeoutIntents
      <3>1. ENABLED PersistTimeout(request)
        BY <2>1, <2>3, ExpandENABLED, Isa
           DEF PersistTimeout, vars
      <3> QED BY <2>1, <3>1
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM PersistTimeoutMakesExactVoteDurableAndSignable ==
  \A request:
    (/\ StrongInductiveInvariant
     /\ PersistTimeout(request))
      => /\ DurableTimeoutVoteAt(
               request.node, request.vote.view)'
         /\ TimeoutSign(request.node, request.vote) \in signTimeouts'
PROOF
  <1>1. ASSUME NEW request,
                StrongInductiveInvariant,
                PersistTimeout(request)
         PROVE /\ DurableTimeoutVoteAt(
                      request.node, request.vote.view)'
               /\ TimeoutSign(request.node, request.vote)
                    \in signTimeouts'
    <2>1. request \in pendingTimeout
      BY <1>1 DEF PersistTimeout
    <2>2. /\ request.vote.signer = request.node
           /\ request.vote.context = context
      BY <1>1, <2>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized
    <2>3. /\ request.vote \in timeoutIntents'
           /\ TimeoutSign(request.node, request.vote)
                \in signTimeouts'
           /\ context' = context
      BY <1>1, Isa DEF PersistTimeout
    <2>4. DurableTimeoutVoteAt(
             request.node, request.vote.view)'
      BY <2>2, <2>3 DEF DurableTimeoutVoteAt
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM PersistTimeoutProducesExactPendingSignature ==
  \A request:
    (/\ request.vote.context = context
     /\ PersistTimeout(request))
    => TimeoutSignRequestPending(
         request.node, request.vote.view)'
BY SMT
   DEF PersistTimeout, TimeoutSignRequestPending, TimeoutSign

THEOREM PendingTimeoutSignatureIsAuthorized ==
  \A request \in signTimeouts:
    (/\ StrongInductiveInvariant
     /\ TimeoutSigningProvenanceInvariant)
      => /\ request.node \in ValidatorIds
         /\ request.vote \in timeoutIntents
         /\ request.vote.signer = request.node
PROOF
  <1>1. ASSUME NEW request \in signTimeouts,
                StrongInductiveInvariant,
                TimeoutSigningProvenanceInvariant
         PROVE /\ request.node \in ValidatorIds
               /\ request.vote \in timeoutIntents
               /\ request.vote.signer = request.node
    <2>1. /\ TypeInvariant
           /\ TimeoutSigningRequiresIntent
      BY <1>1 DEF StrongInductiveInvariant, Safety
    <2>2. request \in TimeoutSignSet
      BY <1>1, <2>1 DEF TypeInvariant
    <2>3. /\ request.node \in ValidatorIds
           /\ request.vote \in TimeoutVoteRecordSet
      BY <2>2, Isa DEF TimeoutSignSet
    <2>4. request.vote \in timeoutIntents
      BY <1>1, <2>1
         DEF TimeoutSigningRequiresIntent
    <2>5. request.vote.signer = request.node
      BY <1>1 DEF TimeoutSigningProvenanceInvariant
    <2> QED BY <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM PendingTimeoutSignatureIsCoreEnabled ==
  \A request \in signTimeouts:
    (/\ StrongInductiveInvariant
     /\ TimeoutSigningProvenanceInvariant)
      => ENABLED CompleteTimeoutSignature(request)
PROOF
  <1>1. ASSUME NEW request \in signTimeouts,
                StrongInductiveInvariant,
                TimeoutSigningProvenanceInvariant
         PROVE ENABLED CompleteTimeoutSignature(request)
    <2>1. /\ request.vote \in timeoutIntents
           /\ request.vote.signer = request.node
      BY <1>1, PendingTimeoutSignatureIsAuthorized
    <2> QED BY <1>1, <2>1, ExpandENABLED, Isa
         DEF CompleteTimeoutSignature, vars
  <1> QED BY <1>1

THEOREM CompleteTimeoutSignaturePublishesEveryVoter ==
  \A request:
    (/\ request.vote.context = context
     /\ CompleteTimeoutSignature(request))
      => \A recipient \in CurrentVoters:
           TimeoutVotePublishedTo(
             request.node, request.vote.view, recipient)'
PROOF
  <1>1. ASSUME NEW request,
                request.vote.context = context,
                CompleteTimeoutSignature(request)
         PROVE \A recipient \in CurrentVoters:
                 TimeoutVotePublishedTo(
                   request.node, request.vote.view, recipient)'
    <2>1. ASSUME NEW recipient \in CurrentVoters
           PROVE TimeoutVotePublishedTo(
                   request.node, request.vote.view, recipient)'
      <3>1. TimeoutEnvelope(recipient, request.vote)
               \in timeoutNetwork'
        BY <1>1, <2>1
           DEF CompleteTimeoutSignature, BroadcastTimeouts
      <3>2. /\ TimeoutEnvelope(recipient, request.vote).recipient
                     = recipient
             /\ TimeoutEnvelope(recipient, request.vote).vote.signer
                     = request.node
             /\ TimeoutEnvelope(recipient, request.vote).vote.context
                     = context'
             /\ TimeoutEnvelope(recipient, request.vote).vote.view
                     = request.vote.view
        BY <1>1, Isa DEF CompleteTimeoutSignature, TimeoutEnvelope
      <3> QED BY <3>1, <3>2 DEF TimeoutVotePublishedTo
    <2> QED BY <2>1
  <1> QED BY <1>1

=============================================================================

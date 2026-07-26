---- MODULE SumeragiV2ReplyRouteOwnershipProofs ----
EXTENDS SumeragiV2ReplyRouteOwnership, TLAPS

(***************************************************************************
Deductive boundary for the V2 merge-sidecar reply lifecycle.

The theorems in this module are action-local safety facts.  They establish
the canonical identity projection, lexicographic cancellation, exact
GenerationHint correlation, persistence ordering, atomic future-generation
rejection, and terminal-only generation installation directly from the
production model.

The full temporal induction and local-progress formulas are intentionally
left as named obligations below.  They are not THEOREMs and carry no proof
evidence.  In particular this module does not claim rotating-leader progress
or discharge any consensus liveness debt.
***************************************************************************)

THEOREM ReplyCanonicalRequestIdentityBindsEveryImmutableCoordinate ==
  \A serviceGeneration, streamEpoch, semanticSequence,
     semantic, requester, responder:
    LET identity ==
          ReplyCanonicalRequestIdentity(
            serviceGeneration, streamEpoch, semanticSequence,
            semantic, requester, responder)
    IN /\ identity.version = ReplyProtocolVersion
       /\ identity.serviceGeneration = serviceGeneration
       /\ identity.streamEpoch = streamEpoch
       /\ identity.semanticSequence = semanticSequence
       /\ identity.payload = semantic
       /\ identity.reference = ReplyCanonicalReference(semantic)
       /\ identity.requesterPeer = requester
       /\ identity.responderPeer = responder
BY DEF ReplyCanonicalRequestIdentity

THEOREM ReplyCanonicalRequestIdentityExcludesOnlyCumulativeCloseFloor ==
  \A serviceGeneration, streamEpoch, semanticSequence,
     semantic, requester, responder, leftClosedThrough,
     rightClosedThrough:
    ReplyCanonicalRequestIdentityWithCloseFloor(
      serviceGeneration, streamEpoch, semanticSequence,
      semantic, requester, responder, leftClosedThrough)
      =
    ReplyCanonicalRequestIdentityWithCloseFloor(
      serviceGeneration, streamEpoch, semanticSequence,
      semantic, requester, responder, rightClosedThrough)
BY DEF ReplyCanonicalRequestIdentityWithCloseFloor

THEOREM ReplyCanonicalIdentitySeparatesGenerationOrEpochSuccessors ==
  \A leftGeneration, rightGeneration,
     leftEpoch, rightEpoch, semanticSequence,
     semantic, requester, responder:
    \/ leftGeneration # rightGeneration
    \/ leftEpoch # rightEpoch
    =>
      ReplyCanonicalRequestIdentity(
        leftGeneration, leftEpoch, semanticSequence,
        semantic, requester, responder)
        #
      ReplyCanonicalRequestIdentity(
        rightGeneration, rightEpoch, semanticSequence,
        semantic, requester, responder)
BY SMT DEF ReplyCanonicalRequestIdentity

THEOREM ReplyClosedPrefixCoverageIsLexicographic ==
  \A candidateGeneration, candidateEpoch, candidateSequence,
     floorGeneration, floorEpoch, floorSequence:
    ReplyCoordinateAtOrBefore(
      ReplyOccurrenceCoordinate(
        candidateGeneration, candidateEpoch, candidateSequence),
      ReplyOccurrenceCoordinate(
        floorGeneration, floorEpoch, floorSequence))
    <=>
      \/ candidateGeneration < floorGeneration
      \/ /\ candidateGeneration = floorGeneration
         /\ \/ candidateEpoch < floorEpoch
            \/ /\ candidateEpoch = floorEpoch
               /\ candidateSequence <= floorSequence
BY DEF ReplyCoordinateAtOrBefore, ReplyOccurrenceCoordinate

THEOREM ReplyClosedPrefixUpdateCoalescesRequesterRow ==
  \A witness \in ReplyCloseWitnessSet:
    ReplyClosedPrefixUpdate(witness) =>
      rrClosedPrefix' =
        [rrClosedPrefix EXCEPT
           ![witness.requester] =
             [source \in ReplySources |->
                ReplyCloseCoordinate(witness)]]
BY DEF ReplyClosedPrefixUpdate

THEOREM ReplyCloseAndAcknowledgementBindGenerationEpochAndFloor ==
  \A witness \in ReplyCloseWitnessSet,
     acknowledgement \in ReplyCloseAcknowledgementSet:
    /\ ReplyCloseWitnessValid(witness)
    /\ ReplyCloseAcknowledgementValid(acknowledgement)
    /\ witness.requester = acknowledgement.requester
    /\ witness.responder = acknowledgement.responder
    /\ witness.closeIdentity = acknowledgement.closeIdentity
    =>
      /\ witness.serviceGeneration =
           acknowledgement.serviceGeneration
      /\ witness.streamEpoch = acknowledgement.streamEpoch
      /\ witness.closedThrough = acknowledgement.closedThrough
BY SMT
   DEF ReplyCloseWitnessValid,
       ReplyCloseAcknowledgementValid,
       ReplyCanonicalCloseIdentity

THEOREM ReplyGenerationHintAcceptanceIsAuthenticatedExactAndStrict ==
  \A hint \in ReplyGenerationHintSet:
    ReplyGenerationHintValid(hint) =>
      /\ hint.authenticatedResponder = hint.responder
      /\ hint.observedGeneration =
           rrServiceGeneration[hint.requester][hint.responder]
      /\ hint.currentGeneration = rrResponderGeneration[hint.responder]
      /\ hint.currentGeneration > hint.observedGeneration
      /\ hint.observedMessageHash # {}
      /\ ReplyGenerationHintExactTrigger(hint)
BY DEF ReplyGenerationHintValid,
       ReplyGenerationHintExactTrigger

THEOREM ReplyFutureGenerationRejectionIsFailAtomic ==
  ReplyFutureGenerationRejectIsAtomic
BY DEF ReplyFutureGenerationRejectIsAtomic,
       RejectFutureGenerationWithoutMutation

THEOREM ReplyOlderGenerationHintHasNoReplyRoute ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     observedMessageHash
       \in SUBSET (ReplyRequestIdentitySet \cup ReplyCloseIdentitySet):
    ReturnOlderGenerationHintWithoutRoute(
      requester, responder, observedMessageHash)
      => UNCHANGED ReplyRouteV2Vars
BY DEF ReturnOlderGenerationHintWithoutRoute

THEOREM ReplyCapacityRejectionIsFailAtomic ==
  ReplyCapacityRejectIsAtomic
BY DEF ReplyCapacityRejectIsAtomic,
       RejectRequesterEpochOverflowWithoutMutation,
       RejectNonTerminalResponderCompactionWithoutMutation,
       RejectResponderGenerationOverflow

THEOREM ReplyHintDiscardRequiresPersistedGenerationAndEpoch ==
  \A reset \in ReplyHintResetSet:
    DiscardPersistedHintPartialState(reset) =>
      /\ reset \in rrPendingHintResets
      /\ rrServiceGeneration[reset.requester][reset.responder] =
           reset.newGeneration
      /\ rrRequesterStreamEpoch
           [reset.requester][reset.responder] = reset.newEpoch
      /\ rrCloseStreamEpoch
           [reset.requester][reset.responder] = reset.newEpoch
BY DEF DiscardPersistedHintPartialState

THEOREM ReplyResponderGenerationPersistenceRequiresTerminalState ==
  \A source \in ReplySources:
    PersistTerminalResponderGeneration(source) =>
      /\ ReplyResponderStateTerminal(source)
      /\ rrDurableResponderGeneration' =
           [rrDurableResponderGeneration EXCEPT ![source] = @ + 1]
      /\ rrResponderGeneration' = rrResponderGeneration
BY DEF PersistTerminalResponderGeneration

THEOREM ReplyResponderGenerationInstallRequiresPersistedSuccessor ==
  \A source \in ReplySources:
    InstallPersistedResponderGeneration(source) =>
      /\ ReplyResponderStateTerminal(source)
      /\ rrDurableResponderGeneration[source] =
           rrResponderGeneration[source] + 1
      /\ rrResponderGeneration' =
           [rrResponderGeneration EXCEPT
              ![source] = rrDurableResponderGeneration[source]]
BY DEF InstallPersistedResponderGeneration

(***************************************************************************
Specified-unproved boundaries.  These predicates are suitable proof-ledger
targets, but this repair deliberately attaches no completion evidence.
***************************************************************************)
ReplyRouteV2InductiveSafetyObligation ==
  ReplyRouteV2Spec => []ReplyRouteV2SafetyInvariant

ReplyRouteV2SuccessorIsolationObligation ==
  ReplyRouteV2Spec =>
    /\ []ReplyStaleArtifactCannotAffectSuccessor
    /\ [][ReplyFutureGenerationRejectIsAtomic]_ReplyRouteV2Vars
    /\ [][ReplyHintPersistencePrecedesPartialDiscard]_ReplyRouteV2Vars

ReplyRouteV2LocalProgressObligation ==
  ReplyRouteV2Spec =>
    \A requester \in ReplyOwners, responder \in ReplySources:
      ReplyCloseWorkEventuallyTerminates(requester, responder)

ReplyRouteOwnershipModelObligation ==
  /\ ReplyRouteV2InductiveSafetyObligation
  /\ ReplyRouteV2SuccessorIsolationObligation
  /\ ReplyRouteV2LocalProgressObligation

=============================================================================

---- MODULE SumeragiV2ReplyRoutePipelineProofs ----
EXTENDS SumeragiV2ReplyRoutePipeline, TLAPS

(***************************************************************************
Action-local proof surface for V2 gate and writer-flush ownership.

The checked lemmas below bind each gate and flush receipt to the exact
generation/epoch/sequence request occurrence and show that stale receipts and
future-generation inputs are fail-atomic.  Full temporal induction remains a
specified-unproved obligation; no network or rotating-leader liveness result
is claimed here.
***************************************************************************)

THEOREM ReplyPipelinePayloadCarriesExactOccurrenceCoordinates ==
  \A item \in ReplyPipelineItemSet:
    /\ item.serviceGeneration =
         item.requestIdentity.serviceGeneration
    /\ item.streamEpoch = item.requestIdentity.streamEpoch
    /\ item.semanticSequence =
         item.requestIdentity.semanticSequence
    =>
      LET payload == ReplyPipelinePayloadForItem(item)
      IN /\ payload.requestIdentity = item.requestIdentity
         /\ payload.serviceGeneration = item.serviceGeneration
         /\ payload.streamEpoch = item.streamEpoch
         /\ payload.semanticSequence = item.semanticSequence
BY SMT DEF ReplyPipelinePayloadForItem, ReplyPipelinePayload

THEOREM ReplyRequestGateIdentityCarriesExactOccurrenceCoordinates ==
  \A item \in ReplyPipelineItemSet:
    /\ ReplyRequestGateIdentity(item).requestIdentity =
         item.requestIdentity
    /\ ReplyRequestGateIdentity(item).serviceGeneration =
         item.serviceGeneration
    /\ ReplyRequestGateIdentity(item).streamEpoch = item.streamEpoch
    /\ ReplyRequestGateIdentity(item).semanticSequence =
         item.semanticSequence
BY DEF ReplyRequestGateIdentity

THEOREM ReplyFlushIdentityCarriesExactOccurrenceCoordinates ==
  \A item \in ReplyPipelineItemSet:
    /\ ReplyFlushIdentity(item).requestIdentity =
         item.requestIdentity
    /\ ReplyFlushIdentity(item).serviceGeneration =
         item.serviceGeneration
    /\ ReplyFlushIdentity(item).streamEpoch = item.streamEpoch
    /\ ReplyFlushIdentity(item).semanticSequence =
         item.semanticSequence
    /\ ReplyFlushIdentity(item).gateIdentity =
         ReplyRequestGateIdentity(item)
BY DEF ReplyFlushIdentity

THEOREM ReplyExactTicketAuthorityRequiresCurrentOccurrence ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, item \in ReplyPipelineItemSet,
     attempt \in ReplyAttemptSet:
    ReplyPipelineExactTicketAuthority(
      owner, semantic, source, item, attempt)
      => ReplyFlushIdentityMatchesCurrentOccurrence(item)
BY DEF ReplyPipelineExactTicketAuthority

THEOREM ReplyStaleFlushReceiptRejectionIsFailAtomic ==
  \A item \in ReplyPipelineItemSet:
    RejectStaleFlushReceiptWithoutMutation(item)
      => UNCHANGED ReplyPipelineVars
BY DEF RejectStaleFlushReceiptWithoutMutation

THEOREM ReplyPipelineCapacityRejectionIsFailAtomic ==
  \A source \in ReplySources:
    RejectNonTerminalPipelineCompactionWithoutMutation(source)
      => UNCHANGED ReplyPipelineVars
BY DEF RejectNonTerminalPipelineCompactionWithoutMutation

THEOREM ReplyHintDiscardRemovesOnlyOldGenerationEpochItems ==
  \A reset \in ReplyHintResetSet:
    DiscardPersistedPipelinePartialState(reset) =>
      rpItems' = ReplyPipelineItemsAfterHintReset(reset)
BY DEF DiscardPersistedPipelinePartialState

THEOREM ReplyClosePruningUsesLexicographicOccurrenceCoverage ==
  \A witness \in ReplyCloseWitnessSet,
     item \in rpItems:
    /\ CloseSemanticPipelineRequest(witness)
    /\ item \notin rpItems'
    /\ item.owner = witness.requester
    =>
      ReplyCoordinateAtOrBefore(
        ReplyOccurrenceCoordinate(
          item.serviceGeneration,
          item.streamEpoch,
          item.semanticSequence),
        ReplyCloseCoordinate(witness))
BY SMT
   DEF CloseSemanticPipelineRequest,
       ReplyPipelineItemsAfterClosedPrefix

THEOREM ReplyPipelineRolloverRequiresEveryGateAndFlushTerminal ==
  \A source \in ReplySources:
    PersistTerminalPipelineResponderGeneration(source) =>
      /\ ReplyResponderStateTerminal(source)
      /\ \A item \in rpItems: item.source # source
      /\ \A attachment \in rpPendingAttachments:
           attachment.source # source
BY DEF PersistTerminalPipelineResponderGeneration,
       ReplyPipelineResponderTerminal

(***************************************************************************
Specified-unproved boundaries.  These are deliberately operators rather than
THEOREMs, so the proof ledger cannot mistake specification for completion.
***************************************************************************)
ReplyPipelineV2InductiveSafetyObligation ==
  ReplyPipelineSpec => []ReplyPipelineV2SafetyInvariant

ReplyPipelineV2SuccessorIsolationObligation ==
  ReplyPipelineSpec =>
    []ReplyStaleChunkAckOrFlushCannotAffectSuccessor

ReplyPipelineV2LocalProgressObligation ==
  ReplyPipelineSpec =>
    \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
      ReplyPendingAttachmentEventuallyConsumed(
        owner, semantic, source)

ReplyRoutePipelineModelObligation ==
  /\ ReplyPipelineV2InductiveSafetyObligation
  /\ ReplyPipelineV2SuccessorIsolationObligation
  /\ ReplyPipelineV2LocalProgressObligation

=============================================================================

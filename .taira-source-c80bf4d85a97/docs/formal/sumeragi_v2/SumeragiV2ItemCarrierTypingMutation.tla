---- MODULE SumeragiV2ItemCarrierTypingMutation ----
EXTENDS SumeragiV2AsyncNetwork

(***************************************************************************
Bounded semantic witness for the exact item/carrier typing boundary.

The historical structural predicate accepted a Proposal whose resource source
was any validator even when it differed from the signed proposal's proposer.
Such an item cannot occur in the canonical `AsyncNetworkItems` Proposal
family.  The fixed branch uses `AsyncItemTyped`; the mutation branch restores
only that missing source/proposer check.  Both branches execute over the same
full asynchronous state relation so scheduler typing remains in scope.
***************************************************************************)

CONSTANT LooseProposalSourceTyping

ItemCarrierTypingSpec == AsyncFiniteSpec

MutationProposalEnvelope ==
  CHOOSE envelope \in ProposalEnvelopeSet: TRUE

MutationAlternateProposalSource ==
  CHOOSE source \in
      ValidatorIds \ {MutationProposalEnvelope.proposal.proposer}:
    TRUE

MutationProposalItem ==
  AsyncNetworkItem(
    "Proposal", MutationAlternateProposalSource, MutationProposalEnvelope)

LooseProposalSourceItemTyped(item) ==
  /\ DOMAIN item = {"kind", "source", "envelope"}
  /\ item.kind = "Proposal"
  /\ item.source \in ValidatorIds
  /\ item.envelope.recipient \in ValidatorIds
  /\ item.envelope \in ProposalEnvelopeSet

CanonicalProposalItem(item) ==
  \E envelope \in ProposalEnvelopeSet:
    item = AsyncNetworkItem(
             "Proposal", envelope.proposal.proposer, envelope)

SelectedProposalItemTyped(item) ==
  IF LooseProposalSourceTyping
  THEN LooseProposalSourceItemTyped(item)
  ELSE AsyncItemTyped(item)

MutationWitnessIsLooseTyped ==
  LooseProposalSourceItemTyped(MutationProposalItem)

MutationWitnessIsOutsideCanonicalProposalCarrier ==
  ~CanonicalProposalItem(MutationProposalItem)

SelectedTypingImpliesCanonicalProposalCarrier ==
  SelectedProposalItemTyped(MutationProposalItem)
    => CanonicalProposalItem(MutationProposalItem)

=============================================================================

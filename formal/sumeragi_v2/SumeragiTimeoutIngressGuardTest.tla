---- MODULE SumeragiTimeoutIngressGuardTest ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

(***************************************************************************
Timeout wire-schema boundary checks for the reducer and asynchronous adapter.
The production boundary represented by DeliverTimeout must validate the full
decoded envelope before selected authorization fields are consumed.
***************************************************************************)

MalformedTimeoutEnvelope(recipient, vote) ==
  [recipient |-> recipient, vote |-> vote, extra |-> TRUE]

THEOREM SelectedViewCheckDoesNotEstablishEnvelopeSchema ==
  \A recipient \in ValidatorIds, vote \in TimeoutVoteRecordSet:
    LET envelope == MalformedTimeoutEnvelope(recipient, vote)
    IN /\ envelope.recipient = recipient
       /\ envelope.vote = vote
       /\ envelope.vote.view \in Views
       /\ envelope \notin TimeoutEnvelopeSet
PROOF
  <1>1. ASSUME NEW recipient \in ValidatorIds,
                NEW vote \in TimeoutVoteRecordSet
         PROVE LET envelope == MalformedTimeoutEnvelope(recipient, vote)
               IN /\ envelope.recipient = recipient
                  /\ envelope.vote = vote
                  /\ envelope.vote.view \in Views
                  /\ envelope \notin TimeoutEnvelopeSet
    <2>1. DOMAIN MalformedTimeoutEnvelope(recipient, vote) =
             {"recipient", "vote", "extra"}
      BY Isa DEF MalformedTimeoutEnvelope
    <2> QED BY <1>1, <2>1, Isa
       DEF MalformedTimeoutEnvelope, TimeoutEnvelopeSet,
           TimeoutVoteRecordSet
  <1> QED BY <1>1

THEOREM FullEnvelopeGuardTypesTimeoutVote ==
  \A envelope:
    envelope \in TimeoutEnvelopeSet
      => /\ envelope.recipient \in ValidatorIds
         /\ envelope.vote \in TimeoutVoteRecordSet
         /\ envelope.vote.view \in Views
BY Isa DEF TimeoutEnvelopeSet, TimeoutVoteRecordSet

THEOREM DeliverTimeoutRequiresCanonicalEnvelope ==
  \A envelope:
    DeliverTimeout(envelope) => envelope \in TimeoutEnvelopeSet
BY DEF DeliverTimeout

THEOREM AsyncSentTimeoutItemCarriesCanonicalEnvelope ==
  \A item:
    (/\ AsyncTransportHistoryTypeInvariant
     /\ item \in asyncSentItems
     /\ item.kind = "TimeoutVote")
      => item.envelope \in TimeoutEnvelopeSet
PROOF
  <1>1. ASSUME NEW item,
                AsyncTransportHistoryTypeInvariant,
                item \in asyncSentItems,
                item.kind = "TimeoutVote"
         PROVE item.envelope \in TimeoutEnvelopeSet
    <2>1. AsyncItemTyped(item)
      BY <1>1 DEF AsyncTransportHistoryTypeInvariant
    <2> QED BY <1>1, <2>1, Isa DEF AsyncItemTyped
  <1> QED BY <1>1

THEOREM ExecuteCoreTimeoutDeliveryCarriesCanonicalEnvelope ==
  \A command:
    (/\ AsyncTransportHistoryTypeInvariant
     /\ ExecuteCoreDelivery(command)
     /\ command.kind = "DeliverTimeout")
      => /\ command.item.kind = "TimeoutVote"
         /\ command.item.envelope \in TimeoutEnvelopeSet
         /\ DeliverTimeout(command.item.envelope)
PROOF
  <1>1. ASSUME NEW command,
                AsyncTransportHistoryTypeInvariant,
                ExecuteCoreDelivery(command),
                command.kind = "DeliverTimeout"
         PROVE /\ command.item.kind = "TimeoutVote"
               /\ command.item.envelope \in TimeoutEnvelopeSet
               /\ DeliverTimeout(command.item.envelope)
    <2>1. /\ command.item \in asyncSentItems
           /\ command.item.kind = "TimeoutVote"
           /\ DeliverTimeout(command.item.envelope)
      BY <1>1, Isa DEF ExecuteCoreDelivery
    <2>2. command.item.envelope \in TimeoutEnvelopeSet
      BY <1>1, <2>1, AsyncSentTimeoutItemCarriesCanonicalEnvelope
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM AsyncTypedTimeoutDeliveryRefinesCoreStep ==
  \A command:
    (/\ AsyncTransportHistoryTypeInvariant
     /\ ExecuteCoreDelivery(command)
     /\ command.kind = "DeliverTimeout")
      => [Next]_vars
PROOF
  <1>1. ASSUME NEW command,
                AsyncTransportHistoryTypeInvariant,
                ExecuteCoreDelivery(command),
                command.kind = "DeliverTimeout"
         PROVE [Next]_vars
    <2>1. DeliverTimeout(command.item.envelope)
      BY <1>1, ExecuteCoreTimeoutDeliveryCarriesCanonicalEnvelope
    <2>2. command.item.envelope \in timeoutNetwork
      BY <2>1 DEF DeliverTimeout
    <2>3. Next
      BY <2>1, <2>2 DEF Next
    <2> QED BY <2>3
  <1> QED BY <1>1

=============================================================================

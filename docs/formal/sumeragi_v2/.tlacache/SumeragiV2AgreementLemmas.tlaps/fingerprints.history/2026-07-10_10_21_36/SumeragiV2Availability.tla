---- MODULE SumeragiV2Availability ----
EXTENDS SumeragiV2Quorums

(***************************************************************************
Availability and external-validity vocabulary shared by the core protocol.

The production reducer receives a validation completion only after the body
adapter has reconstructed the exact subject, durably stored it, checked its
manifest/hash, and completed deterministic validation.  The model represents
that trusted adapter contract with separate Available, Durable, and Validated
records.  A Prepare intent may be persisted only after all three boundaries.
***************************************************************************)

NoSubject == "NoSubject"
Subjects == {"A", "B"}
SubjectOrNone == Subjects \cup {NoSubject}

BodyRecord(node, context, subject) ==
  [node |-> node, context |-> context, subject |-> subject]

ValidationRecord(node, context, view, generation, subject) ==
  [node |-> node, context |-> context, view |-> view,
   generation |-> generation, subject |-> subject]

BodyHeldBy(durableBodies, node, context, subject) ==
  BodyRecord(node, context, subject) \in durableBodies

BodyValidatedBy(validatedBodies, node, context, view, generation, subject) ==
  ValidationRecord(node, context, view, generation, subject)
    \in validatedBodies

PrepareSignerAvailability(durableBodies, validatedBodies, context,
                          view, generations, subject, signer) ==
  /\ BodyHeldBy(durableBodies, signer, context, subject)
  /\ BodyValidatedBy(validatedBodies, signer, context, view,
                     generations[signer], subject)

CertifiedBodyAvailable(epoch, signers, durableBodies, context, subject) ==
  \E signer \in signers \cap Honest:
    BodyHeldBy(durableBodies, signer, context, subject)

CertifiedBodyValid(epoch, signers, validSubjects, subject) ==
  /\ DualQuorum(epoch, signers)
  /\ subject \in validSubjects
  /\ (signers \cap Honest) # {}

THEOREM PrepareSignerAvailabilityIncludesDurability ==
  \A durableBodies, validatedBodies, context, view, generations,
     subject, signer:
    PrepareSignerAvailability(durableBodies, validatedBodies, context,
                              view, generations, subject, signer)
      => BodyHeldBy(durableBodies, signer, context, subject)
BY DEF PrepareSignerAvailability

=============================================================================

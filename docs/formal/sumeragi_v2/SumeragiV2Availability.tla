---- MODULE SumeragiV2Availability ----
EXTENDS SumeragiV2Quorums

(***************************************************************************
Availability and external-validity vocabulary shared by the core protocol.

The production reducer receives a validation completion only after the body
adapter has reconstructed the exact subject, durably stored it, checked its
manifest/hash, and completed deterministic validation.  The model represents
that trusted adapter contract with separate Available, Durable, and Validated
records.  Every adapter-owned record is keyed by the exact consensus view.
A separate view-independent retained record may authorize only the explicit
locked-body rebind action; it is never itself durable, validated, or applied
evidence.  A Prepare intent may be persisted only after all three exact-round
boundaries.
***************************************************************************)

NoSubject == "NoSubject"
Subjects == {"A", "B"}
SubjectOrNone == Subjects \cup {NoSubject}

BodyRecord(node, context, view, subject) ==
  [node |-> node, context |-> context, view |-> view,
   subject |-> subject]

RetainedLockedBodyRecord(node, context, subject) ==
  [node |-> node, context |-> context, subject |-> subject]

ValidationRecord(node, context, view, generation, subject) ==
  [node |-> node, context |-> context, view |-> view,
   generation |-> generation, subject |-> subject]

BodyHeldBy(durableBodies, node, context, view, subject) ==
  BodyRecord(node, context, view, subject) \in durableBodies

RetainedLockedBodyHeldBy(retainedLockedBodies, node, context, subject) ==
  RetainedLockedBodyRecord(node, context, subject)
    \in retainedLockedBodies

BodyValidatedBy(validatedBodies, node, context, view, generation, subject) ==
  ValidationRecord(node, context, view, generation, subject)
    \in validatedBodies

ValidatedBodiesSound(validatedBodies, validSubjects) ==
  \A validation \in validatedBodies:
    validation.subject \in validSubjects

PrepareSignerAvailability(durableBodies, validatedBodies, context,
                          view, generations, subject, signer) ==
  /\ BodyHeldBy(durableBodies, signer, context, view, subject)
  /\ BodyValidatedBy(validatedBodies, signer, context, view,
                     generations[signer], subject)

CertifiedBodyAvailable(epoch, signers, durableBodies, context, view,
                       subject) ==
  \E signer \in signers \cap Honest:
    BodyHeldBy(durableBodies, signer, context, view, subject)

CertifiedBodyValid(epoch, signers, validSubjects, subject) ==
  /\ DualQuorum(epoch, signers)
  /\ subject \in validSubjects
  /\ (signers \cap Honest) # {}

=============================================================================

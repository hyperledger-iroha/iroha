---- MODULE SumeragiV2CandidateIdentityMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Bounded mutation witness for scheduler-wide candidate coalescing.

The record and identity operators mirror the source fields in
SumeragiV2AsyncNetwork.  Exact admission coalesces only an identical immutable
identity.  The historical projection keeps only node and kind, reproducing a
semantic-duplicate filter which survives the consumer pool
it protected.  Each buggy configuration changes one omitted identity
dimension and checks the corresponding named invariant.
***************************************************************************)

CONSTANTS Mode, Difference

BaseConsumerContext == [height |-> 7, epoch |-> 2]
OtherConsumerContext == [height |-> 7, epoch |-> 3]
NoAsyncItem == "NoAsyncItem"

AsyncCandidateWithIdentity(
    commandClass, kind, node, blockHeight, roundView, subject, item,
    consumerContext, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity) ==
  [class |-> commandClass, kind |-> kind, node |-> node,
   height |-> blockHeight, view |-> roundView, subject |-> subject,
   item |-> item, consumerContext |-> consumerContext,
   consumerView |-> consumerView,
   consumerGeneration |-> consumerGeneration,
   evidence |-> evidence, bodyIdentity |-> bodyIdentity,
   manifestIdentity |-> manifestIdentity,
   commitmentIdentity |-> commitmentIdentity]

CandidateAt(consumerContext, consumerView, consumerGeneration, item,
            evidence, roundView, bodyIdentity, manifestIdentity,
            commitmentIdentity) ==
  AsyncCandidateWithIdentity(
    "Completion", "SignVote", 1, 7, roundView, "subject-a", item,
    consumerContext, consumerView, consumerGeneration, evidence,
    bodyIdentity, manifestIdentity, commitmentIdentity)

BaseCandidate ==
  CandidateAt(BaseConsumerContext, 4, 0, "payload-0", "evidence-0", 4,
              "body-0", "manifest-0", "commitment-0")

OfferedCandidate ==
  CASE Difference = "Identical" -> BaseCandidate
    [] Difference = "ConsumerContext" ->
         CandidateAt(OtherConsumerContext, 4, 0, "payload-0", "evidence-0",
                     4, "body-0", "manifest-0", "commitment-0")
    [] Difference = "ConsumerView" ->
         CandidateAt(BaseConsumerContext, 5, 0, "payload-0", "evidence-0",
                     4, "body-0", "manifest-0", "commitment-0")
    [] Difference = "Generation" ->
         CandidateAt(BaseConsumerContext, 4, 1, "payload-0", "evidence-0",
                     4, "body-0", "manifest-0", "commitment-0")
    [] Difference = "Payload" ->
         CandidateAt(BaseConsumerContext, 4, 0, "payload-1", "evidence-0",
                     4, "body-0", "manifest-0", "commitment-0")
    [] Difference = "Evidence" ->
         CandidateAt(BaseConsumerContext, 4, 0, "payload-0", "evidence-1",
                     4, "body-0", "manifest-0", "commitment-0")
    [] Difference = "Work" ->
         CandidateAt(BaseConsumerContext, 4, 0, "payload-0", "evidence-0",
                     5, "body-0", "manifest-0", "commitment-0")
    [] Difference = "Body" ->
         CandidateAt(BaseConsumerContext, 4, 0, "payload-0", "evidence-0",
                     4, "body-1", "manifest-0", "commitment-0")
    [] Difference = "Manifest" ->
         CandidateAt(BaseConsumerContext, 4, 0, "payload-0", "evidence-0",
                     4, "body-0", "manifest-1", "commitment-0")
    [] Difference = "Commitment" ->
         CandidateAt(BaseConsumerContext, 4, 0, "payload-0", "evidence-0",
                     4, "body-0", "manifest-0", "commitment-1")
    [] OTHER ->
         CandidateAt(OtherConsumerContext, 5, 1, "payload-1", "evidence-1",
                     5, "body-1", "manifest-1", "commitment-1")

AsyncConsumerEventTag(candidate) ==
  [context |-> candidate.consumerContext,
   height |-> candidate.consumerContext.height,
   node |-> candidate.node,
   view |-> candidate.consumerView,
   generation |-> candidate.consumerGeneration]

AsyncWorkIdentity(candidate) ==
  [class |-> candidate.class, kind |-> candidate.kind,
   node |-> candidate.node, height |-> candidate.height,
   view |-> candidate.view, subject |-> candidate.subject]

ExactAsyncCandidateIdentity(candidate) ==
  [consumer |-> AsyncConsumerEventTag(candidate),
   payload |-> candidate.item,
   evidence |-> candidate.evidence,
   work |-> AsyncWorkIdentity(candidate),
   body |-> candidate.bodyIdentity,
   manifest |-> candidate.manifestIdentity,
   commitment |-> candidate.commitmentIdentity]

ProjectedIdentity(candidate) ==
  [node |-> candidate.node,
   kind |-> candidate.kind]

VARIABLES resident, handled, coalesced

vars == <<resident, handled, coalesced>>

Init ==
  /\ resident = BaseCandidate
  /\ handled = FALSE
  /\ coalesced = FALSE

ExactAdmission ==
  LET identical ==
        ExactAsyncCandidateIdentity(resident)
          = ExactAsyncCandidateIdentity(OfferedCandidate)
  IN /\ ~handled
     /\ resident' = IF identical THEN resident ELSE OfferedCandidate
     /\ handled' = TRUE
     /\ coalesced' = identical

ProjectedAdmission ==
  LET identical ==
        ProjectedIdentity(resident) = ProjectedIdentity(OfferedCandidate)
  IN /\ ~handled
     /\ resident' = IF identical THEN resident ELSE OfferedCandidate
     /\ handled' = TRUE
     /\ coalesced' = identical

Done == handled /\ UNCHANGED vars

Next ==
  (IF Mode = "Exact" THEN ExactAdmission ELSE ProjectedAdmission) \/ Done

Spec == Init /\ [][Next]_vars

ExactCandidateAdmitted == handled => resident = OfferedCandidate

ExactIdenticalCandidateCoalesced ==
  handled => /\ coalesced
             /\ resident = BaseCandidate

ChangedConsumerContextNotCoalesced ==
  handled => resident.consumerContext = OfferedCandidate.consumerContext

ChangedConsumerViewNotCoalesced ==
  handled => resident.consumerView = OfferedCandidate.consumerView

StaleGenerationNotCoalesced ==
  handled =>
    resident.consumerGeneration = OfferedCandidate.consumerGeneration

ChangedPayloadNotCoalesced ==
  handled => resident.item = OfferedCandidate.item

ChangedEvidenceNotCoalesced ==
  handled => resident.evidence = OfferedCandidate.evidence

ChangedWorkNotCoalesced ==
  handled =>
    AsyncWorkIdentity(resident) = AsyncWorkIdentity(OfferedCandidate)

ChangedBodyNotCoalesced ==
  handled => resident.bodyIdentity = OfferedCandidate.bodyIdentity

ChangedManifestNotCoalesced ==
  handled => resident.manifestIdentity = OfferedCandidate.manifestIdentity

ChangedCommitmentNotCoalesced ==
  handled =>
    resident.commitmentIdentity = OfferedCandidate.commitmentIdentity

====

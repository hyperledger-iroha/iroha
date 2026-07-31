---- MODULE SumeragiV2AdequateLeaderCandidateTombstoneMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Finite mutation model for the missing adequate-leader candidate lifecycle.

Candidate A is serviced and tombstoned, same-height restart preserves that
record, and an equal-rank replacement B is serviced next.  A delayed producer
then retries A.  The repaired lifecycle coalesces the retry against A's
durable record, excluding the A -> B -> A lasso.  Tombstones remain bounded:
strict view advance reclaims only lower-view records and successor-height
rollover clears the predecessor-height table.

An independent D branch covers atomic restart reconstruction.  D has a
durable replay source and a matching volatile pre-crash scheduler owner.  The
fixed reset ignores the doomed volatile owner and rebuilds D; the mutation
consults it first, suppresses replay, and then clears the only owner.  After D
is genuinely serviced, the fixed path still suppresses its replay from the
retained tombstone.

The S branch covers the deliberately restart-scoped signature callback.
Durable signing intent survives while the old process callback and its
published-control cache do not.  Restart must therefore remove S's
process-local marker and reissue S.  Retaining that marker suppresses the only
new-generation callback and strands the durable signing owner.

The Q branch covers equivalent aggregate evidence.  Two valid CommitQCs name
the same context/view/phase/subject but the second carries a redundant signer
superset.  The repaired semantic projection maps them to one owner and the
second delivery coalesces with the first tombstone.  Retaining the raw signer
carrier manufactures a second same-semantic owner and can exhaust a capacity
derived from protocol geometry rather than the certificate powerset.

The E branch covers a nondispatchable queue candidate.  Removing that carrier
is terminal lifecycle retirement even though it is not semantic protocol
service: the repaired path installs E's tombstone before releasing the queue
owner.  The mutation drops E without a tombstone, allowing an exact delayed
retransmission to recreate the same logical occurrence.

The K and J branches cover a Chunk whose full tombstone is reclaimed after a
strict view advance or durable Decision.  The bounded replacement is the
monotone stage guard: a held/stale-view or post-Decision Chunk is terminally
retired before scheduler admission.  The mutation omits that guard and
recreates the old carrier after reclamation.

The logical identity mirrors
`AdequateLeaderFrozenCandidateOwnerIdentity`: target, context/height, adequate
leader, frozen view, subject, semantic phase, local owner, and immutable
payload.  Consumer generation is intentionally absent, so restart replay is
the same logical occurrence.  S is the narrow lifecycle exception: the
identity is unchanged, but its completion marker ends at process restart.
***************************************************************************)

CONSTANTS
  CoalesceServicedCandidate,
  RetireTerminalDiscard,
  RejectRetiredChunkStage,
  PreserveAcrossSameHeightRestart,
  ConsultVolatileOwnerDuringRestart,
  RetainSignedMarkerAcrossRestart,
  NormalizeAggregateEvidence,
  ReclaimAtStrictViewAdvance,
  ResetAtSuccessorHeight

ASSUME
  /\ CoalesceServicedCandidate \in BOOLEAN
  /\ RetireTerminalDiscard \in BOOLEAN
  /\ RejectRetiredChunkStage \in BOOLEAN
  /\ PreserveAcrossSameHeightRestart \in BOOLEAN
  /\ ConsultVolatileOwnerDuringRestart \in BOOLEAN
  /\ RetainSignedMarkerAcrossRestart \in BOOLEAN
  /\ NormalizeAggregateEvidence \in BOOLEAN
  /\ ReclaimAtStrictViewAdvance \in BOOLEAN
  /\ ResetAtSuccessorHeight \in BOOLEAN

CandidateIdentity(
    target, blockHeight, leader, roundView,
    subject, semanticPhase, owner, payload) ==
  [target |-> target,
   context |-> [height |-> blockHeight],
   leader |-> leader,
   view |-> roundView,
   subject |-> subject,
   phase |-> semanticPhase,
   owner |-> owner,
   kind |-> "Candidate",
   payload |-> payload]

CandidateA ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "Prepare/4", "target", "payload-A")

CandidateB ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "Prepare/4", "target", "payload-B")

CandidateC ==
  CandidateIdentity(
    "target", 0, "leader", 2,
    "subject", "Prepare/4", "target", "payload-C")

CandidateD ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "Prepare/4", "target", "durable-replay-D")

CandidateS ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "SignVote", "target", "durable-signature-S")

CandidateE ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "Nondispatchable", "target", "terminal-discard-E")

CandidateK ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "chunk-subject-K", "Chunk", "target", "held-chunk-K")

CandidateJ ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "chunk-subject-J", "Chunk", "target", "decided-chunk-J")

AggregateCertificateReference ==
  [context |-> [height |-> 0],
   height |-> 0,
   view |-> 1,
   phase |-> "Commit",
   subject |-> "subject"]

AggregateEvidenceA ==
  [reference |-> AggregateCertificateReference,
   signers |-> {"signer-1", "signer-2", "signer-3"}]

AggregateEvidenceSuperset ==
  [reference |-> AggregateCertificateReference,
   signers |->
     {"signer-1", "signer-2", "signer-3", "signer-4"}]

AggregateCandidatePayload(evidence) ==
  IF NormalizeAggregateEvidence
  THEN evidence.reference
  ELSE evidence

CandidateQa ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "DeliverQC", "target",
    AggregateCandidatePayload(AggregateEvidenceA))

CandidateQb ==
  CandidateIdentity(
    "target", 0, "leader", 1,
    "subject", "DeliverQC", "target",
    AggregateCandidatePayload(AggregateEvidenceSuperset))

CandidateCarrier ==
  {CandidateA, CandidateB, CandidateC, CandidateD, CandidateS, CandidateE,
   CandidateK, CandidateJ,
   CandidateQa, CandidateQb}

VARIABLES phase, height, view, liveCandidates, candidateTombstones

vars ==
  <<phase, height, view, liveCandidates, candidateTombstones>>

TypeInvariant ==
  /\ phase \in
       {"Fresh", "AActive", "AServiced", "Restarted",
        "BActive", "BServiced", "AReplayChecked",
        "ViewAdvanced", "CActive", "CServiced", "RolledOver",
        "DPreCrashScheduled", "DRebuilt", "DServiced",
        "DTombstoneReplayChecked", "SActive", "SServiced",
        "SReissued", "EActive", "EDiscarded", "EReplayChecked",
        "KActive", "KServiced", "KViewAdvanced", "KReplayChecked",
        "JActive", "JServiced", "JDecided", "JReplayChecked",
        "QaActive", "QaServiced", "QVariantChecked"}
  /\ height \in 0..1
  /\ view \in 1..2
  /\ liveCandidates \subseteq CandidateCarrier
  /\ candidateTombstones \subseteq CandidateCarrier

LiveCandidateIsNotTombstoned ==
  liveCandidates \cap candidateTombstones = {}

SameHeightRestartPreservesServicedA ==
  phase = "Restarted"
    => /\ height = 0
       /\ view = 1
       /\ CandidateA \in candidateTombstones
       /\ liveCandidates = {}

ServicedCandidateACannotResurrect ==
  phase \in
    {"AReplayChecked", "ViewAdvanced", "CActive", "CServiced", "RolledOver"}
    => CandidateA \notin liveCandidates

StrictViewAdvanceReclaimsOnlyOldView ==
  phase = "ViewAdvanced"
    => /\ height = 0
       /\ view = 2
       /\ candidateTombstones = {}

CandidateTombstoneTableRemainsEpisodeBounded ==
  Cardinality(candidateTombstones) <= 2

SuccessorHeightReclaimsPredecessorTombstones ==
  phase = "RolledOver"
    => /\ height = 1
       /\ candidateTombstones = {}
       /\ liveCandidates = {}

(***************************************************************************
Restart clears every volatile scheduler carrier in the same atomic action
which rebuilds the durable replay candidate.  The rebuilt sequence may consult
the durable tombstone, but it must not consult the pre-crash live set which is
about to be erased.  The paired post-service restart demonstrates that this
repair still suppresses a genuinely retired logical occurrence.
***************************************************************************)
UnservicedDurableCandidateRebuiltAfterRestart ==
  phase = "DRebuilt"
    => /\ CandidateD \in liveCandidates
       /\ CandidateD \notin candidateTombstones

GenuineTombstoneSuppressesRestartReplay ==
  phase = "DTombstoneReplayChecked"
    => /\ CandidateD \notin liveCandidates
       /\ CandidateD \in candidateTombstones

RestartScopedSignedCompletionIsReissued ==
  phase = "SReissued"
    => /\ CandidateS \in liveCandidates
       /\ CandidateS \notin candidateTombstones

TerminalDiscardCannotBeReadmitted ==
  phase = "EReplayChecked"
    => /\ CandidateE \notin liveCandidates
       /\ CandidateE \in candidateTombstones

RetiredChunkStageCannotReadmitAfterViewAdvance ==
  phase = "KReplayChecked"
    => CandidateK \notin liveCandidates

RetiredChunkStageCannotReadmitAfterDecision ==
  phase = "JReplayChecked"
    => CandidateJ \notin liveCandidates

EquivalentAggregateEvidenceCoalescesToOneIdentity ==
  phase = "QVariantChecked"
    => /\ CandidateQa = CandidateQb
       /\ liveCandidates = {}
       /\ candidateTombstones = {CandidateQa}

Init ==
  /\ phase = "Fresh"
  /\ height = 0
  /\ view = 1
  /\ liveCandidates = {}
  /\ candidateTombstones = {}

AdmitCandidateA ==
  /\ phase = "Fresh"
  /\ phase' = "AActive"
  /\ liveCandidates' = {CandidateA}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceCandidateA ==
  /\ phase = "AActive"
  /\ phase' = "AServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateA}
  /\ UNCHANGED <<height, view>>

RestartSameHeight ==
  /\ phase = "AServiced"
  /\ phase' = "Restarted"
  /\ liveCandidates' = {}
  /\ candidateTombstones' =
       IF PreserveAcrossSameHeightRestart
       THEN candidateTombstones
       ELSE {}
  /\ UNCHANGED <<height, view>>

AdmitEqualRankReplacementB ==
  /\ phase = "Restarted"
  /\ CandidateB \notin candidateTombstones
  /\ phase' = "BActive"
  /\ liveCandidates' = {CandidateB}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceCandidateB ==
  /\ phase = "BActive"
  /\ phase' = "BServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateB}
  /\ UNCHANGED <<height, view>>

RetryCandidateA ==
  /\ phase = "BServiced"
  /\ phase' = "AReplayChecked"
  /\ liveCandidates' =
       IF /\ CoalesceServicedCandidate
          /\ CandidateA \in candidateTombstones
       THEN {}
       ELSE {CandidateA}
  /\ UNCHANGED <<height, view, candidateTombstones>>

AdvanceStrictlyNewerView ==
  /\ phase = "AReplayChecked"
  /\ liveCandidates = {}
  /\ phase' = "ViewAdvanced"
  /\ view' = 2
  /\ candidateTombstones' =
       IF ReclaimAtStrictViewAdvance
       THEN {identity \in candidateTombstones:
               identity.context.height = height
                 /\ identity.view >= view'}
       ELSE candidateTombstones
  /\ UNCHANGED <<height, liveCandidates>>

AdmitCandidateC ==
  /\ phase = "ViewAdvanced"
  /\ CandidateC \notin candidateTombstones
  /\ phase' = "CActive"
  /\ liveCandidates' = {CandidateC}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceCandidateC ==
  /\ phase = "CActive"
  /\ phase' = "CServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateC}
  /\ UNCHANGED <<height, view>>

RolloverSuccessorHeight ==
  /\ phase = "CServiced"
  /\ phase' = "RolledOver"
  /\ height' = 1
  /\ view' = 1
  /\ liveCandidates' = {}
  /\ candidateTombstones' =
       IF ResetAtSuccessorHeight
       THEN {}
       ELSE candidateTombstones

ScheduleUnservicedDurableCandidateBeforeCrash ==
  /\ phase = "Fresh"
  /\ phase' = "DPreCrashScheduled"
  /\ liveCandidates' = {CandidateD}
  /\ UNCHANGED <<height, view, candidateTombstones>>

RestartAndRebuildUnservicedDurableCandidate ==
  /\ phase = "DPreCrashScheduled"
  /\ CandidateD \in liveCandidates
  /\ CandidateD \notin candidateTombstones
  /\ phase' = "DRebuilt"
  /\ liveCandidates' =
       IF /\ ConsultVolatileOwnerDuringRestart
          /\ CandidateD \in liveCandidates
       THEN {}
       ELSE IF CandidateD \in candidateTombstones
            THEN {}
            ELSE {CandidateD}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceRebuiltDurableCandidate ==
  /\ phase = "DRebuilt"
  /\ CandidateD \in liveCandidates
  /\ phase' = "DServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateD}
  /\ UNCHANGED <<height, view>>

RestartAfterDurableCandidateService ==
  /\ phase = "DServiced"
  /\ phase' = "DTombstoneReplayChecked"
  /\ liveCandidates' =
       IF CandidateD \in candidateTombstones
       THEN {}
       ELSE {CandidateD}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ScheduleSignatureCompletion ==
  /\ phase = "Fresh"
  /\ phase' = "SActive"
  /\ liveCandidates' = {CandidateS}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceSignatureCompletion ==
  /\ phase = "SActive"
  /\ phase' = "SServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateS}
  /\ UNCHANGED <<height, view>>

RestartAndReissueSignatureCompletion ==
  /\ phase = "SServiced"
  /\ CandidateS \in candidateTombstones
  /\ phase' = "SReissued"
  /\ candidateTombstones' =
       IF RetainSignedMarkerAcrossRestart
       THEN candidateTombstones
       ELSE candidateTombstones \ {CandidateS}
  /\ liveCandidates' =
       IF RetainSignedMarkerAcrossRestart
       THEN {}
       ELSE {CandidateS}
  /\ UNCHANGED <<height, view>>

AdmitNondispatchableCandidate ==
  /\ phase = "Fresh"
  /\ phase' = "EActive"
  /\ liveCandidates' = {CandidateE}
  /\ UNCHANGED <<height, view, candidateTombstones>>

DiscardNondispatchableCandidate ==
  /\ phase = "EActive"
  /\ phase' = "EDiscarded"
  /\ liveCandidates' = {}
  /\ candidateTombstones' =
       IF RetireTerminalDiscard
       THEN candidateTombstones \cup {CandidateE}
       ELSE candidateTombstones
  /\ UNCHANGED <<height, view>>

RetryDiscardedCandidate ==
  /\ phase = "EDiscarded"
  /\ phase' = "EReplayChecked"
  /\ liveCandidates' =
       IF CandidateE \in candidateTombstones
       THEN {}
       ELSE {CandidateE}
  /\ UNCHANGED <<height, view, candidateTombstones>>

AdmitChunkBeforeViewAdvance ==
  /\ phase = "Fresh"
  /\ phase' = "KActive"
  /\ liveCandidates' = {CandidateK}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceChunkBeforeViewAdvance ==
  /\ phase = "KActive"
  /\ phase' = "KServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateK}
  /\ UNCHANGED <<height, view>>

AdvanceViewAfterChunkService ==
  /\ phase = "KServiced"
  /\ phase' = "KViewAdvanced"
  /\ view' = 2
  /\ candidateTombstones' =
       IF ReclaimAtStrictViewAdvance
       THEN {identity \in candidateTombstones:
               identity.context.height = height
                 /\ identity.view >= view'}
       ELSE candidateTombstones
  /\ UNCHANGED <<height, liveCandidates>>

RetryRetiredChunkAfterViewAdvance ==
  /\ phase = "KViewAdvanced"
  /\ phase' = "KReplayChecked"
  /\ liveCandidates' =
       IF /\ RejectRetiredChunkStage
          /\ CandidateK.view < view
       THEN {}
       ELSE IF CandidateK \in candidateTombstones
            THEN {}
            ELSE {CandidateK}
  /\ UNCHANGED <<height, view, candidateTombstones>>

AdmitChunkBeforeDecision ==
  /\ phase = "Fresh"
  /\ phase' = "JActive"
  /\ liveCandidates' = {CandidateJ}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceChunkBeforeDecision ==
  /\ phase = "JActive"
  /\ phase' = "JServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' = candidateTombstones \cup {CandidateJ}
  /\ UNCHANGED <<height, view>>

DecideAfterChunkService ==
  /\ phase = "JServiced"
  /\ phase' = "JDecided"
  /\ candidateTombstones' = {}
  /\ UNCHANGED <<height, view, liveCandidates>>

RetryRetiredChunkAfterDecision ==
  /\ phase = "JDecided"
  /\ phase' = "JReplayChecked"
  /\ liveCandidates' =
       IF RejectRetiredChunkStage
       THEN {}
       ELSE IF CandidateJ \in candidateTombstones
            THEN {}
            ELSE {CandidateJ}
  /\ UNCHANGED <<height, view, candidateTombstones>>

AdmitAggregateCertificate ==
  /\ phase = "Fresh"
  /\ phase' = "QaActive"
  /\ liveCandidates' = {CandidateQa}
  /\ UNCHANGED <<height, view, candidateTombstones>>

ServiceAggregateCertificate ==
  /\ phase = "QaActive"
  /\ phase' = "QaServiced"
  /\ liveCandidates' = {}
  /\ candidateTombstones' =
       candidateTombstones \cup {CandidateQa}
  /\ UNCHANGED <<height, view>>

RetryEquivalentAggregateSuperset ==
  /\ phase = "QaServiced"
  /\ phase' = "QVariantChecked"
  /\ liveCandidates' =
       IF CandidateQb \in candidateTombstones
       THEN {}
       ELSE {CandidateQb}
  /\ UNCHANGED <<height, view, candidateTombstones>>

Next ==
  \/ AdmitCandidateA
  \/ ServiceCandidateA
  \/ RestartSameHeight
  \/ AdmitEqualRankReplacementB
  \/ ServiceCandidateB
  \/ RetryCandidateA
  \/ AdvanceStrictlyNewerView
  \/ AdmitCandidateC
  \/ ServiceCandidateC
  \/ RolloverSuccessorHeight
  \/ ScheduleUnservicedDurableCandidateBeforeCrash
  \/ RestartAndRebuildUnservicedDurableCandidate
  \/ ServiceRebuiltDurableCandidate
  \/ RestartAfterDurableCandidateService
  \/ ScheduleSignatureCompletion
  \/ ServiceSignatureCompletion
  \/ RestartAndReissueSignatureCompletion
  \/ AdmitNondispatchableCandidate
  \/ DiscardNondispatchableCandidate
  \/ RetryDiscardedCandidate
  \/ AdmitChunkBeforeViewAdvance
  \/ ServiceChunkBeforeViewAdvance
  \/ AdvanceViewAfterChunkService
  \/ RetryRetiredChunkAfterViewAdvance
  \/ AdmitChunkBeforeDecision
  \/ ServiceChunkBeforeDecision
  \/ DecideAfterChunkService
  \/ RetryRetiredChunkAfterDecision
  \/ AdmitAggregateCertificate
  \/ ServiceAggregateCertificate
  \/ RetryEquivalentAggregateSuperset

=============================================================================

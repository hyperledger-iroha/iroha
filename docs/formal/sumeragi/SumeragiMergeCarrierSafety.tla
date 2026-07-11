---- MODULE SumeragiMergeCarrierSafety ----
EXTENDS FiniteSets, Integers

(***************************************************************************
A bounded safety model for certified merge-candidate dissemination and the
compact global carrier that commits it.

The deterministic round leader may be Byzantine and may advertise two
different candidate bodies.  Honest validators authenticate the sender,
re-execute the exact body and durably lock the complete round context before
signing.  With four validators, one Byzantine validator and quorum three,
that lock prevents two distinct merge QCs for the same round.  A global block
then carries one exact QC-bound digest; a replica may apply the entry only
after the block is committed and the full hash-verified sidecar is available.

`Bug` selects expected-failure mutations.  `"none"` is the production model.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

Honest == {0, 1, 2}
Byzantine == {3}
Nodes == Honest \cup Byzantine
Leader == 3
Digests == {1, 2}
Quorum == 3

VARIABLES
  \* Candidate bodies accepted after transport authentication and re-execution.
  \* @type: Int -> Set(Int);
  accepted,
  \* Sticky evidence that any invalid sender/context/body was accepted.
  \* @type: Bool;
  badCandidateAccepted,
  \* Durable per-round signing guard; zero means unsigned.
  \* @type: Int -> Int;
  durableLock,
  \* Signers collected for each candidate digest.
  \* @type: Int -> Set(Int);
  signatures,
  \* Candidate digests with a formed merge QC.
  \* @type: Set(Int);
  qcs,
  \* Compact digest staged in the current global proposal; zero means absent.
  \* @type: Int;
  proposedCarrier,
  \* Compact digest committed by the canonical global block; zero means absent.
  \* @type: Int;
  committedCarrier,
  \* Sticky evidence that a carrier lacked its exact QC/round binding.
  \* @type: Bool;
  badCarrier,
  \* Fully assembled, hash-verified sidecars available to each replica.
  \* @type: Int -> Set(Int);
  sidecars,
  \* Sticky evidence that a corrupt or mismatched sidecar was admitted.
  \* @type: Bool;
  badSidecarAccepted,
  \* Applied merge digest per replica; zero means not applied.
  \* @type: Int -> Int;
  applied,
  \* Sticky evidence of application without the committed carrier and sidecar.
  \* @type: Bool;
  unsafeApply

vars ==
  <<accepted, badCandidateAccepted, durableLock, signatures, qcs,
    proposedCarrier, committedCarrier, badCarrier, sidecars,
    badSidecarAccepted, applied, unsafeApply>>

Init ==
  /\ accepted = [n \in Nodes |-> {}]
  /\ badCandidateAccepted = FALSE
  /\ durableLock = [n \in Nodes |-> 0]
  /\ signatures = [d \in Digests |-> {}]
  /\ qcs = {}
  /\ proposedCarrier = 0
  /\ committedCarrier = 0
  /\ badCarrier = FALSE
  /\ sidecars = [n \in Nodes |-> {}]
  /\ badSidecarAccepted = FALSE
  /\ applied = [n \in Nodes |-> 0]
  /\ unsafeApply = FALSE

AcceptCandidate(n, sender, digest, exactContext, validBody) ==
  /\ digest \notin accepted[n]
  /\ LET valid ==
           /\ sender = Leader
           /\ exactContext
           /\ validBody
         injected ==
           \/ (Bug = "accept_wrong_leader" /\ sender # Leader
               /\ exactContext /\ validBody)
           \/ (Bug = "accept_wrong_context" /\ sender = Leader
               /\ ~exactContext /\ validBody)
           \/ (Bug = "accept_invalid_body" /\ sender = Leader
               /\ exactContext /\ ~validBody)
     IN /\ valid \/ injected
        /\ badCandidateAccepted' = badCandidateAccepted \/ ~valid
  /\ accepted' = [accepted EXCEPT ![n] = @ \cup {digest}]
  /\ UNCHANGED <<durableLock, signatures, qcs, proposedCarrier,
                 committedCarrier, badCarrier, sidecars,
                 badSidecarAccepted, applied, unsafeApply>>

HonestSign(n, digest) ==
  /\ n \in Honest
  /\ digest \in accepted[n]
  /\ n \notin signatures[digest]
  /\ (durableLock[n] \in {0, digest}
      \/ (Bug = "double_sign" /\ durableLock[n] \in (Digests \ {digest})))
  /\ signatures' = [signatures EXCEPT ![digest] = @ \cup {n}]
  /\ durableLock' =
       IF Bug = "sign_without_lock" THEN durableLock
       ELSE IF durableLock[n] = 0
            THEN [durableLock EXCEPT ![n] = digest]
            ELSE durableLock
  /\ UNCHANGED <<accepted, badCandidateAccepted, qcs, proposedCarrier,
                 committedCarrier, badCarrier, sidecars,
                 badSidecarAccepted, applied, unsafeApply>>

ByzantineSign(digest) ==
  /\ Leader \notin signatures[digest]
  /\ signatures' = [signatures EXCEPT ![digest] = @ \cup {Leader}]
  /\ UNCHANGED <<accepted, badCandidateAccepted, durableLock, qcs,
                 proposedCarrier, committedCarrier, badCarrier, sidecars,
                 badSidecarAccepted, applied, unsafeApply>>

Restart(n) ==
  /\ durableLock[n] # 0
  /\ durableLock' =
       IF Bug = "forget_lock_on_restart"
       THEN [durableLock EXCEPT ![n] = 0]
       ELSE durableLock
  /\ UNCHANGED <<accepted, badCandidateAccepted, signatures, qcs,
                 proposedCarrier, committedCarrier, badCarrier, sidecars,
                 badSidecarAccepted, applied, unsafeApply>>

FormQc(digest) ==
  /\ digest \notin qcs
  /\ (Cardinality(signatures[digest]) >= Quorum
      \/ Bug = "qc_without_quorum")
  /\ qcs' = qcs \cup {digest}
  /\ UNCHANGED <<accepted, badCandidateAccepted, durableLock, signatures,
                 proposedCarrier, committedCarrier, badCarrier, sidecars,
                 badSidecarAccepted, applied, unsafeApply>>

StageCarrier(digest, exactContext) ==
  /\ proposedCarrier = 0
  /\ LET valid == digest \in qcs /\ exactContext
         injected ==
           \/ (Bug = "carrier_without_qc" /\ digest \notin qcs
               /\ exactContext)
           \/ (Bug = "carrier_wrong_context" /\ digest \in qcs
               /\ ~exactContext)
     IN /\ valid \/ injected
        /\ badCarrier' = badCarrier \/ ~valid
  /\ proposedCarrier' = digest
  /\ UNCHANGED <<accepted, badCandidateAccepted, durableLock, signatures,
                 qcs, committedCarrier, sidecars, badSidecarAccepted,
                 applied, unsafeApply>>

CommitCarrier ==
  /\ proposedCarrier \in Digests
  /\ committedCarrier = 0
  /\ committedCarrier' = proposedCarrier
  /\ UNCHANGED <<accepted, badCandidateAccepted, durableLock, signatures,
                 qcs, proposedCarrier, badCarrier, sidecars,
                 badSidecarAccepted, applied, unsafeApply>>

ReceiveSidecar(n, digest, exactHash) ==
  /\ digest \notin sidecars[n]
  /\ LET valid == proposedCarrier = digest /\ exactHash
         injected ==
           Bug = "accept_corrupt_sidecar"
           /\ proposedCarrier = digest
           /\ ~exactHash
     IN /\ valid \/ injected
        /\ badSidecarAccepted' = badSidecarAccepted \/ ~valid
  /\ sidecars' = [sidecars EXCEPT ![n] = @ \cup {digest}]
  /\ UNCHANGED <<accepted, badCandidateAccepted, durableLock, signatures,
                 qcs, proposedCarrier, committedCarrier, badCarrier,
                 applied, unsafeApply>>

ApplyCarrier(n, digest) ==
  /\ applied[n] = 0
  /\ LET valid ==
           /\ committedCarrier = digest
           /\ digest \in sidecars[n]
         injected ==
           \/ (Bug = "apply_before_commit" /\ proposedCarrier = digest
               /\ committedCarrier = 0 /\ digest \in sidecars[n])
           \/ (Bug = "apply_without_sidecar" /\ committedCarrier = digest
               /\ digest \notin sidecars[n])
           \/ (Bug = "apply_wrong_digest" /\ committedCarrier \in Digests
               /\ digest # committedCarrier)
     IN /\ valid \/ injected
        /\ unsafeApply' = unsafeApply \/ ~valid
  /\ applied' = [applied EXCEPT ![n] = digest]
  /\ UNCHANGED <<accepted, badCandidateAccepted, durableLock, signatures,
                 qcs, proposedCarrier, committedCarrier, badCarrier,
                 sidecars, badSidecarAccepted>>

Stable == UNCHANGED vars

Next ==
  \/ \E n \in Nodes, sender \in Nodes, digest \in Digests,
        exactContext \in BOOLEAN, validBody \in BOOLEAN:
       AcceptCandidate(n, sender, digest, exactContext, validBody)
  \/ \E n \in Honest, digest \in Digests: HonestSign(n, digest)
  \/ \E digest \in Digests: ByzantineSign(digest)
  \/ \E n \in Honest: Restart(n)
  \/ \E digest \in Digests: FormQc(digest)
  \/ \E digest \in Digests, exactContext \in BOOLEAN:
       StageCarrier(digest, exactContext)
  \/ CommitCarrier
  \/ \E n \in Nodes, digest \in Digests, exactHash \in BOOLEAN:
       ReceiveSidecar(n, digest, exactHash)
  \/ \E n \in Nodes, digest \in Digests: ApplyCarrier(n, digest)
  \/ Stable

TypeInvariant ==
  /\ accepted \in [Nodes -> SUBSET Digests]
  /\ badCandidateAccepted \in BOOLEAN
  /\ durableLock \in [Nodes -> ({0} \cup Digests)]
  /\ signatures \in [Digests -> SUBSET Nodes]
  /\ qcs \subseteq Digests
  /\ proposedCarrier \in ({0} \cup Digests)
  /\ committedCarrier \in ({0} \cup Digests)
  /\ badCarrier \in BOOLEAN
  /\ sidecars \in [Nodes -> SUBSET Digests]
  /\ badSidecarAccepted \in BOOLEAN
  /\ applied \in [Nodes -> ({0} \cup Digests)]
  /\ unsafeApply \in BOOLEAN

CandidateAdmissionExact == ~badCandidateAccepted

HonestSignaturesDurablyLocked ==
  \A digest \in Digests, n \in (signatures[digest] \cap Honest):
    durableLock[n] = digest

QuorumCertificatesWellFormed ==
  \A digest \in qcs: Cardinality(signatures[digest]) >= Quorum

AtMostOneMergeQc == Cardinality(qcs) <= 1

CarrierBindingExact ==
  /\ ~badCarrier
  /\ (proposedCarrier # 0 => proposedCarrier \in qcs)
  /\ (committedCarrier # 0 => committedCarrier = proposedCarrier)

SidecarAdmissionExact == ~badSidecarAccepted

ApplicationRequiresCommittedCarrierAndSidecar ==
  /\ ~unsafeApply
  /\ \A n \in Nodes:
       applied[n] # 0 =>
         /\ applied[n] = committedCarrier
         /\ applied[n] \in sidecars[n]

AppliedReplicasConverge ==
  \A left \in Nodes, right \in Nodes:
    (applied[left] # 0 /\ applied[right] # 0) =>
      applied[left] = applied[right]

MergeCarrierSafety ==
  /\ CandidateAdmissionExact
  /\ HonestSignaturesDurablyLocked
  /\ QuorumCertificatesWellFormed
  /\ AtMostOneMergeQc
  /\ CarrierBindingExact
  /\ SidecarAdmissionExact
  /\ ApplicationRequiresCommittedCarrierAndSidecar
  /\ AppliedReplicasConverge

MergeCarrierCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MergeCarrierSafety

SafetyFast == MergeCarrierCorrectnessEnvelope

Spec == Init /\ [][Next]_vars

====

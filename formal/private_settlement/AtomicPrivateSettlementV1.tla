---- MODULE AtomicPrivateSettlementV1 ----
EXTENDS Naturals

(***************************************************************************
Abstract safety and bounded-liveness model for AtomicPrivateSettlementV1.

Legs are represented by monotonically increasing certified counts. This is a
symmetry reduction: all legs execute the same state machine, canonical leg
order is checked by the implementation, and no transition depends on a
leg-local secret. The reduction makes LegCount = 255 practical while retaining
the global all-Prepare, all-Commit, and all-or-none application barriers.

"Fixed" is the production protocol. The other modes are deliberate mutations
used to demonstrate that the invariants detect partial application, Commit
before the all-Prepare barrier, and loss of a durable staged delta on restart.
***************************************************************************)

CONSTANTS
  LegCount,
  CommitteeSize,
  Quorum,
  AvailableValidators,
  AuditorAvailable,
  GlobalQuorumAvailable,
  InitialHeight,
  ExpiryHeight,
  MaxCrashes,
  AllowInvalid,
  AllowExpiry,
  Mode

Modes == {"Fixed", "PartialApply", "CommitBeforeAllPrepare", "DropStageOnCrash"}
Phases ==
  {"Collecting", "Audited", "Prepared", "CommitCertified",
   "Finalized", "Aborted", "Expired"}
TerminalPhases == {"Finalized", "Aborted", "Expired"}

VARIABLES
  phase,
  height,
  certifiedSidecars,
  auditedLegs,
  stagedLegs,
  prepareQcs,
  preparedBundleDigest,
  commitQcs,
  carrierDurable,
  globallyAppliedLegs,
  applyCount,
  receiptDurable,
  online,
  crashesRemaining,
  replayRejected,
  invalidRejected

vars ==
  <<phase, height, certifiedSidecars, auditedLegs, stagedLegs,
    prepareQcs, preparedBundleDigest, commitQcs, carrierDurable,
    globallyAppliedLegs, applyCount, receiptDurable, online,
    crashesRemaining, replayRejected, invalidRejected>>

Init ==
  /\ Mode \in Modes
  /\ LegCount \in 2..255
  /\ CommitteeSize = 4
  /\ Quorum = 3
  /\ AvailableValidators \in 0..CommitteeSize
  /\ InitialHeight \in Nat
  /\ ExpiryHeight > InitialHeight
  /\ MaxCrashes \in Nat
  /\ phase = "Collecting"
  /\ height = InitialHeight
  /\ certifiedSidecars = 0
  /\ auditedLegs = 0
  /\ stagedLegs = 0
  /\ prepareQcs = 0
  /\ preparedBundleDigest = FALSE
  /\ commitQcs = 0
  /\ carrierDurable = FALSE
  /\ globallyAppliedLegs = 0
  /\ applyCount = 0
  /\ receiptDurable = FALSE
  /\ online = TRUE
  /\ crashesRemaining = MaxCrashes
  /\ replayRejected = FALSE
  /\ invalidRejected = FALSE

UploadCertifiedSidecar ==
  /\ online
  /\ phase = "Collecting"
  /\ AvailableValidators >= Quorum
  /\ certifiedSidecars < LegCount
  /\ certifiedSidecars' = certifiedSidecars + 1
  /\ UNCHANGED <<phase, height, auditedLegs, stagedLegs, prepareQcs,
                  preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected, invalidRejected>>

AuditOneLeg ==
  /\ online
  /\ phase \in {"Collecting", "Audited"}
  /\ AuditorAvailable
  /\ certifiedSidecars = LegCount
  /\ auditedLegs < LegCount
  /\ auditedLegs' = auditedLegs + 1
  /\ phase' = IF auditedLegs + 1 = LegCount THEN "Audited" ELSE phase
  /\ UNCHANGED <<height, certifiedSidecars, stagedLegs, prepareQcs,
                  preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected, invalidRejected>>

DurablyStageOneLeg ==
  /\ online
  /\ phase = "Audited"
  /\ auditedLegs = LegCount
  /\ stagedLegs < LegCount
  /\ stagedLegs' = stagedLegs + 1
  /\ UNCHANGED <<phase, height, certifiedSidecars, auditedLegs, prepareQcs,
                  preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected, invalidRejected>>

CertifyOnePrepare ==
  /\ online
  /\ phase = "Audited"
  /\ AvailableValidators >= Quorum
  /\ prepareQcs < stagedLegs
  /\ prepareQcs' = prepareQcs + 1
  /\ phase' = IF prepareQcs + 1 = LegCount THEN "Prepared" ELSE phase
  /\ preparedBundleDigest' = (prepareQcs + 1 = LegCount)
  /\ UNCHANGED <<height, certifiedSidecars, auditedLegs, stagedLegs,
                  commitQcs, carrierDurable, globallyAppliedLegs, applyCount,
                  receiptDurable, online, crashesRemaining, replayRejected,
                  invalidRejected>>

CertifyOneCommit ==
  /\ online
  /\ AvailableValidators >= Quorum
  /\ commitQcs < LegCount
  /\ IF Mode = "CommitBeforeAllPrepare"
        THEN /\ phase \in {"Audited", "Prepared"}
             /\ prepareQcs > 0
        ELSE /\ phase = "Prepared"
             /\ prepareQcs = LegCount
             /\ preparedBundleDigest
  /\ commitQcs' = commitQcs + 1
  /\ phase' = IF commitQcs + 1 = LegCount THEN "CommitCertified" ELSE phase
  /\ UNCHANGED <<height, certifiedSidecars, auditedLegs, stagedLegs,
                  prepareQcs, preparedBundleDigest, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected, invalidRejected>>

PersistCarrier ==
  /\ online
  /\ phase = "CommitCertified"
  /\ commitQcs = LegCount
  /\ ~carrierDurable
  /\ carrierDurable' = TRUE
  /\ UNCHANGED <<phase, height, certifiedSidecars, auditedLegs, stagedLegs,
                  prepareQcs, preparedBundleDigest, commitQcs,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected, invalidRejected>>

ApplyGlobalTransaction ==
  /\ online
  /\ phase = "CommitCertified"
  /\ commitQcs = LegCount
  /\ carrierDurable
  /\ GlobalQuorumAvailable
  /\ applyCount = 0
  /\ globallyAppliedLegs' = IF Mode = "PartialApply" THEN 1 ELSE LegCount
  /\ applyCount' = 1
  /\ receiptDurable' = TRUE
  /\ phase' = "Finalized"
  /\ UNCHANGED <<height, certifiedSidecars, auditedLegs, stagedLegs,
                  prepareQcs, preparedBundleDigest, commitQcs, carrierDurable,
                  online, crashesRemaining, replayRejected, invalidRejected>>

RejectReplay ==
  /\ online
  /\ phase = "Finalized"
  /\ ~replayRejected
  /\ replayRejected' = TRUE
  /\ UNCHANGED <<phase, height, certifiedSidecars, auditedLegs, stagedLegs,
                  prepareQcs, preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, invalidRejected>>

RejectInvalidLeg ==
  /\ AllowInvalid
  /\ online
  /\ phase \in {"Collecting", "Audited"}
  /\ ~invalidRejected
  /\ invalidRejected' = TRUE
  /\ phase' = "Aborted"
  /\ UNCHANGED <<height, certifiedSidecars, auditedLegs, stagedLegs,
                  prepareQcs, preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected>>

ExpireBeforeFinality ==
  /\ AllowExpiry
  /\ online
  /\ phase \notin TerminalPhases
  /\ height <= ExpiryHeight
  /\ height' = ExpiryHeight + 1
  /\ phase' = "Expired"
  /\ UNCHANGED <<certifiedSidecars, auditedLegs, stagedLegs, prepareQcs,
                  preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable, online,
                  crashesRemaining, replayRejected, invalidRejected>>

Crash ==
  /\ online
  /\ phase \notin TerminalPhases
  /\ crashesRemaining > 0
  /\ online' = FALSE
  /\ crashesRemaining' = crashesRemaining - 1
  /\ stagedLegs' = IF Mode = "DropStageOnCrash" THEN 0 ELSE stagedLegs
  /\ UNCHANGED <<phase, height, certifiedSidecars, auditedLegs, prepareQcs,
                  preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable,
                  replayRejected, invalidRejected>>

Restart ==
  /\ ~online
  /\ online' = TRUE
  /\ UNCHANGED <<phase, height, certifiedSidecars, auditedLegs, stagedLegs,
                  prepareQcs, preparedBundleDigest, commitQcs, carrierDurable,
                  globallyAppliedLegs, applyCount, receiptDurable,
                  crashesRemaining, replayRejected, invalidRejected>>

TerminalIdle ==
  /\ phase \in TerminalPhases
  /\ UNCHANGED vars

Next ==
  \/ UploadCertifiedSidecar
  \/ AuditOneLeg
  \/ DurablyStageOneLeg
  \/ CertifyOnePrepare
  \/ CertifyOneCommit
  \/ PersistCarrier
  \/ ApplyGlobalTransaction
  \/ RejectReplay
  \/ RejectInvalidLeg
  \/ ExpireBeforeFinality
  \/ Crash
  \/ Restart
  \/ TerminalIdle

Spec == Init /\ [][Next]_vars

FairSpec ==
  Spec
  /\ WF_vars(UploadCertifiedSidecar)
  /\ WF_vars(AuditOneLeg)
  /\ WF_vars(DurablyStageOneLeg)
  /\ WF_vars(CertifyOnePrepare)
  /\ WF_vars(CertifyOneCommit)
  /\ WF_vars(PersistCarrier)
  /\ WF_vars(ApplyGlobalTransaction)
  /\ WF_vars(Restart)

TypeOK ==
  /\ phase \in Phases
  /\ height \in Nat
  /\ certifiedSidecars \in 0..LegCount
  /\ auditedLegs \in 0..LegCount
  /\ stagedLegs \in 0..LegCount
  /\ prepareQcs \in 0..LegCount
  /\ commitQcs \in 0..LegCount
  /\ globallyAppliedLegs \in 0..LegCount
  /\ applyCount \in 0..1
  /\ crashesRemaining \in 0..MaxCrashes
  /\ preparedBundleDigest \in BOOLEAN
  /\ carrierDurable \in BOOLEAN
  /\ receiptDurable \in BOOLEAN
  /\ online \in BOOLEAN
  /\ replayRejected \in BOOLEAN
  /\ invalidRejected \in BOOLEAN

APSExactCommitteeQuorum == CommitteeSize = 4 /\ Quorum = 3

APSFsyncBeforePrepare == prepareQcs <= stagedLegs

APSAllPrepareBarrier ==
  /\ (preparedBundleDigest <=> prepareQcs = LegCount)
  /\ (commitQcs > 0 => prepareQcs = LegCount /\ preparedBundleDigest)

APSAtomicVisibility ==
  globallyAppliedLegs = 0 \/ globallyAppliedLegs = LegCount

APSNoEarlyVisibility ==
  globallyAppliedLegs > 0 =>
    /\ phase = "Finalized"
    /\ commitQcs = LegCount
    /\ carrierDurable

APSIdempotentFinality == applyCount <= 1

APSTerminalFailureIsByteSilent ==
  phase \in {"Aborted", "Expired"} =>
    /\ globallyAppliedLegs = 0
    /\ applyCount = 0
    /\ ~receiptDurable

APSReceiptMatchesAtomicState ==
  receiptDurable <=>
    (phase = "Finalized" /\ globallyAppliedLegs = LegCount /\ applyCount = 1)

Safety ==
  /\ TypeOK
  /\ APSExactCommitteeQuorum
  /\ APSFsyncBeforePrepare
  /\ APSAllPrepareBarrier
  /\ APSAtomicVisibility
  /\ APSNoEarlyVisibility
  /\ APSIdempotentFinality
  /\ APSTerminalFailureIsByteSilent
  /\ APSReceiptMatchesAtomicState

BoundedLivenessAssumptions ==
  /\ Mode = "Fixed"
  /\ ~AllowInvalid
  /\ ~AllowExpiry
  /\ AuditorAvailable
  /\ GlobalQuorumAvailable
  /\ AvailableValidators >= Quorum
  /\ ExpiryHeight > InitialHeight + MaxCrashes

APSEventuallyFinalized ==
  BoundedLivenessAssumptions => <> (phase = "Finalized")

=============================================================================

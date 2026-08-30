---- MODULE AtomicPrivateSettlementV1CommitteeFaults ----
EXTENDS Naturals, FiniteSets, Sequences

(***************************************************************************
Committee-indexed refinement model for AtomicPrivateSettlementV1.

Unlike the count-symmetry model in AtomicPrivateSettlementV1.tla, this model
retains every ordered leg, its independent four-validator committee, its local
auditor, and its DA/Prepare/Commit channels.  It is intended for bounded TLC
runs at small N.  The protocol parameter remains valid for every LegCount in
2..255; the count-symmetry model is still used for an exhaustive 255-leg run.

The one possible faulty identity in each committee is stable.  That detail is
important: allowing a mobile Byzantine identity to change while retaining old
signatures would permit an attacker to accumulate a quorum over time, which is
outside the stated f = 1 fault assumption.  A faulty validator may be
unavailable or may equivocate, and may recover and fail again, but it cannot be
replaced by a second faulty identity in the same committee.

Network Hold, Drop, and Delay all block delivery until HealChannel.  Delivery
after healing represents authenticated retransmission.  Votes can only be
introduced by exact committee members.  An equivocator may sign both modeled
digests, but one equivocator cannot certify the non-bundle digest.

Crash actions snapshot every durable edge and then take the relevant committee
processor, coordinator, or global processor offline.  Restarts do not rebuild
durable values; the crash-floor invariants require those values to survive.
***************************************************************************)

CONSTANTS
  LegCount,
  CommitteeSize,
  Quorum,
  GlobalCommitteeSize,
  GlobalQuorum,
  GlobalAvailableValidators,
  InitialHeight,
  ExpiryHeight,
  MaxValidatorFaultsPerLeg,
  MaxAuditorFaultsPerLeg,
  MaxChannelFaultsPerLeg,
  MaxCommitteeCrashes,
  MaxCoordinatorCrashes,
  MaxGlobalCrashes,
  AllowAbort,
  AllowExpiry

Legs == 1..LegCount
Validators == 1..CommitteeSize
GlobalValidators == 1..GlobalCommitteeSize
AvailableGlobalValidators == 1..GlobalAvailableValidators

Phases ==
  {"Collecting", "Audited", "Prepared", "CommitCertified",
   "Finalized", "Aborted", "Expired"}
TerminalFailurePhases == {"Aborted", "Expired"}
ValidatorStates == {"Honest", "Unavailable", "Byzantine"}
Digests == {"Bundle", "Equivocated"}
Channels == {"DA", "Prepare", "Commit"}
NetworkModes == {"Deliver", "Hold", "Drop", "Delay"}
CommitteeBoundaries == {"Sidecar", "StagedDelta", "PrepareQC", "CommitQC"}
CoordinatorBoundaries == {"PrepareQC", "CommitQC", "KuraAppend"}
GlobalBoundaries == {"KuraAppend", "WSVApplication", "ReceiptPublication"}
PersistenceBoundaries ==
  CommitteeBoundaries \cup CoordinatorBoundaries \cup GlobalBoundaries

(* The ordinal is the abstract route.  Thus the function is both the ordered
   bundle catalogue and a uniqueness witness for participant dataspaces. *)
CanonicalOrder == [ordinal \in Legs |-> ordinal]

VARIABLE st

vars == <<st>>

EmptyVotes ==
  [leg \in Legs |-> [digest \in Digests |-> {}]]

Init ==
  /\ LegCount \in 2..255
  /\ CommitteeSize = 4
  /\ Quorum = 3
  /\ GlobalCommitteeSize = 4
  /\ GlobalQuorum = 3
  /\ GlobalAvailableValidators \in 0..GlobalCommitteeSize
  /\ InitialHeight \in Nat
  /\ ExpiryHeight > InitialHeight
  /\ MaxValidatorFaultsPerLeg \in Nat
  /\ MaxAuditorFaultsPerLeg \in Nat
  /\ MaxChannelFaultsPerLeg \in Nat
  /\ MaxCommitteeCrashes \in Nat
  /\ MaxCoordinatorCrashes \in Nat
  /\ MaxGlobalCrashes \in Nat
  /\ st = [
       phase |-> "Collecting",
       height |-> InitialHeight,
       validatorStatus |->
         [leg \in Legs |-> [validator \in Validators |-> "Honest"]],
       faultIdentity |-> [leg \in Legs |-> 0],
       auditorOnline |-> [leg \in Legs |-> TRUE],
       networkMode |->
         [channel \in Channels |-> [leg \in Legs |-> "Deliver"]],
       committeeOnline |-> [leg \in Legs |-> TRUE],
       coordinatorOnline |-> TRUE,
       globalOnline |-> TRUE,
       daVotes |-> [leg \in Legs |-> {}],
       sidecarDurable |-> {},
       auditApproved |-> {},
       stagedDurable |-> {},
       prepareVotes |-> EmptyVotes,
       prepareQc |-> {},
       bundleDigestDurable |-> FALSE,
       commitVotes |-> EmptyVotes,
       commitQc |-> {},
       carrierDurable |-> FALSE,
       globalVotes |-> {},
       appliedLegs |-> {},
       replayMarkerDurable |-> FALSE,
       applyCount |-> 0,
       receiptDurable |-> FALSE,
       replayRejected |-> FALSE,
       invalidRejected |-> FALSE,
       validatorFaultsRemaining |->
         [leg \in Legs |-> MaxValidatorFaultsPerLeg],
       auditorFaultsRemaining |->
         [leg \in Legs |-> MaxAuditorFaultsPerLeg],
       channelFaultsRemaining |->
         [leg \in Legs |-> MaxChannelFaultsPerLeg],
       committeeCrashesRemaining |-> MaxCommitteeCrashes,
       coordinatorCrashesRemaining |-> MaxCoordinatorCrashes,
       globalCrashesRemaining |-> MaxGlobalCrashes,
       crashedBoundaries |-> {},
       crashFloorSidecars |-> {},
       crashFloorStaged |-> {},
       crashFloorPrepareQc |-> {},
       crashFloorCommitQc |-> {},
       crashFloorCarrier |-> FALSE,
       crashFloorApplied |-> {},
       crashFloorReplayMarker |-> FALSE,
       crashFloorReceipt |-> FALSE
     ]

HonestValidators(leg) ==
  {validator \in Validators : st.validatorStatus[leg][validator] = "Honest"}

ByzantineValidators(leg) ==
  {validator \in Validators :
    st.validatorStatus[leg][validator] = "Byzantine"}

FaultyValidators(leg) ==
  {validator \in Validators :
    st.validatorStatus[leg][validator] # "Honest"}

ProtocolActive == st.phase \notin TerminalFailurePhases /\ st.phase # "Finalized"

(***************************************************************************
Authenticated DA and local audit.
***************************************************************************)

DeliverDaVotes(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Collecting"
  /\ st.committeeOnline[leg]
  /\ st.networkMode["DA"][leg] = "Deliver"
  /\ leg \notin st.sidecarDurable
  /\ ~(HonestValidators(leg) \subseteq st.daVotes[leg])
  /\ st' = [st EXCEPT
       !.daVotes[leg] = @ \cup HonestValidators(leg)
     ]

CertifySidecar(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Collecting"
  /\ st.committeeOnline[leg]
  /\ leg \notin st.sidecarDurable
  /\ Cardinality(st.daVotes[leg]) >= Quorum
  /\ st' = [st EXCEPT
       !.sidecarDurable = @ \cup {leg}
     ]

ApproveAudit(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Collecting"
  /\ leg \in st.sidecarDurable
  /\ st.auditorOnline[leg]
  /\ leg \notin st.auditApproved
  /\ st' = [st EXCEPT
       !.auditApproved = @ \cup {leg},
       !.phase =
         IF st.auditApproved \cup {leg} = Legs THEN "Audited" ELSE @
     ]

(***************************************************************************
Prepare and Commit.  Every QC is over the exact logical bundle digest and is
local to one leg's committee.  The implementation normalizes equivalent
three-of-four signature encodings before computing that digest; the model's
"bundle" value represents the shared certified statement, not a signer bitmap.
Durable staging precedes every Prepare vote and QC.
***************************************************************************)

DurablyStageDelta(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Audited"
  /\ st.committeeOnline[leg]
  /\ leg \in st.auditApproved
  /\ leg \notin st.stagedDurable
  /\ st' = [st EXCEPT
       !.stagedDurable = @ \cup {leg}
     ]

DeliverPrepareVotes(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Audited"
  /\ st.committeeOnline[leg]
  /\ st.networkMode["Prepare"][leg] = "Deliver"
  /\ leg \in st.stagedDurable
  /\ ~(HonestValidators(leg) \subseteq st.prepareVotes[leg]["Bundle"])
  /\ st' = [st EXCEPT
       !.prepareVotes[leg]["Bundle"] = @ \cup HonestValidators(leg)
     ]

EquivocatePrepare(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Audited"
  /\ st.committeeOnline[leg]
  /\ st.networkMode["Prepare"][leg] = "Deliver"
  /\ leg \in st.stagedDurable
  /\ ByzantineValidators(leg) # {}
  /\ ~(ByzantineValidators(leg) \subseteq
       st.prepareVotes[leg]["Equivocated"])
  /\ st' = [st EXCEPT
       !.prepareVotes[leg]["Bundle"] = @ \cup ByzantineValidators(leg),
       !.prepareVotes[leg]["Equivocated"] = @ \cup ByzantineValidators(leg)
     ]

CertifyPrepareQc(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Audited"
  /\ st.committeeOnline[leg]
  /\ leg \in st.stagedDurable
  /\ leg \notin st.prepareQc
  /\ Cardinality(st.prepareVotes[leg]["Bundle"]) >= Quorum
  /\ st' = [st EXCEPT
       !.prepareQc = @ \cup {leg}
     ]

SealCompleteBundleDigest ==
  /\ ProtocolActive
  /\ st.phase = "Audited"
  /\ st.coordinatorOnline
  /\ st.prepareQc = Legs
  /\ ~st.bundleDigestDurable
  /\ st' = [st EXCEPT
       !.bundleDigestDurable = TRUE,
       !.phase = "Prepared"
     ]

DeliverCommitVotes(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Prepared"
  /\ st.committeeOnline[leg]
  /\ st.networkMode["Commit"][leg] = "Deliver"
  /\ st.prepareQc = Legs
  /\ st.bundleDigestDurable
  /\ ~(HonestValidators(leg) \subseteq st.commitVotes[leg]["Bundle"])
  /\ st' = [st EXCEPT
       !.commitVotes[leg]["Bundle"] = @ \cup HonestValidators(leg)
     ]

EquivocateCommit(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Prepared"
  /\ st.committeeOnline[leg]
  /\ st.networkMode["Commit"][leg] = "Deliver"
  /\ st.prepareQc = Legs
  /\ st.bundleDigestDurable
  /\ ByzantineValidators(leg) # {}
  /\ ~(ByzantineValidators(leg) \subseteq
       st.commitVotes[leg]["Equivocated"])
  /\ st' = [st EXCEPT
       !.commitVotes[leg]["Bundle"] = @ \cup ByzantineValidators(leg),
       !.commitVotes[leg]["Equivocated"] = @ \cup ByzantineValidators(leg)
     ]

CertifyCommitQc(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Prepared"
  /\ st.committeeOnline[leg]
  /\ st.prepareQc = Legs
  /\ st.bundleDigestDurable
  /\ leg \notin st.commitQc
  /\ Cardinality(st.commitVotes[leg]["Bundle"]) >= Quorum
  /\ st' = [st EXCEPT
       !.commitQc = @ \cup {leg},
       !.phase =
         IF st.commitQc \cup {leg} = Legs THEN "CommitCertified" ELSE @
     ]

(***************************************************************************
The carrier, global quorum, WSV application, replay marker, and receipt.
Application and the replay marker are one transition, representing one global
StateTransaction.  Receipt publication is deliberately separate so a crash at
the WSV boundary is covered.
***************************************************************************)

PersistCarrierInKura ==
  /\ ProtocolActive
  /\ st.phase = "CommitCertified"
  /\ st.coordinatorOnline
  /\ st.globalOnline
  /\ st.commitQc = Legs
  /\ ~st.carrierDurable
  /\ st' = [st EXCEPT !.carrierDurable = TRUE]

DeliverGlobalVotes ==
  /\ ProtocolActive
  /\ st.phase = "CommitCertified"
  /\ st.globalOnline
  /\ st.carrierDurable
  /\ ~(AvailableGlobalValidators \subseteq st.globalVotes)
  /\ st' = [st EXCEPT
       !.globalVotes = @ \cup AvailableGlobalValidators
     ]

ApplyGlobalStateTransaction ==
  /\ ProtocolActive
  /\ st.phase = "CommitCertified"
  /\ st.globalOnline
  /\ st.carrierDurable
  /\ st.commitQc = Legs
  /\ Cardinality(st.globalVotes) >= GlobalQuorum
  /\ st.applyCount = 0
  /\ ~st.replayMarkerDurable
  /\ st' = [st EXCEPT
       !.appliedLegs = Legs,
       !.replayMarkerDurable = TRUE,
       !.applyCount = 1,
       !.phase = "Finalized"
     ]

PublishReceipt ==
  /\ st.phase = "Finalized"
  /\ st.globalOnline
  /\ st.appliedLegs = Legs
  /\ st.replayMarkerDurable
  /\ ~st.receiptDurable
  /\ st' = [st EXCEPT !.receiptDurable = TRUE]

RejectReplay ==
  /\ st.phase \in {"Finalized", "Expired"}
  /\ ~st.replayRejected
  /\ st' = [st EXCEPT !.replayRejected = TRUE]

AbortInvalidBundle ==
  /\ AllowAbort
  /\ ProtocolActive
  /\ st.phase \in {"Collecting", "Audited"}
  /\ ~st.invalidRejected
  /\ st' = [st EXCEPT
       !.phase = "Aborted",
       !.stagedDurable = {},
       !.invalidRejected = TRUE
     ]

ExpireBundle ==
  /\ AllowExpiry
  /\ ProtocolActive
  /\ st.height <= ExpiryHeight
  /\ st' = [st EXCEPT
       !.height = ExpiryHeight + 1,
       !.phase = "Expired",
       !.stagedDurable = {}
     ]

(***************************************************************************
Independent validator, auditor, and authenticated-channel faults.  Each leg
has its own finite budget, so one configured fault can be active in every
committee simultaneously without permitting a mobile validator identity.
Healing/restart actions are weakly fair in FairSpec.
***************************************************************************)

InjectValidatorFault(leg, validator, kind) ==
  /\ ProtocolActive
  /\ kind \in {"Unavailable", "Byzantine"}
  /\ st.validatorFaultsRemaining[leg] > 0
  /\ st.validatorStatus[leg][validator] = "Honest"
  /\ FaultyValidators(leg) = {}
  /\ st.faultIdentity[leg] \in {0, validator}
  /\ st' = [st EXCEPT
       !.validatorStatus[leg][validator] = kind,
       !.faultIdentity[leg] = validator,
       !.validatorFaultsRemaining[leg] = @ - 1
     ]

RestoreValidator(leg, validator) ==
  /\ st.validatorStatus[leg][validator] # "Honest"
  /\ st' = [st EXCEPT
       !.validatorStatus[leg][validator] = "Honest"
     ]

LoseAuditor(leg) ==
  /\ ProtocolActive
  /\ st.auditorFaultsRemaining[leg] > 0
  /\ st.auditorOnline[leg]
  /\ leg \notin st.auditApproved
  /\ st' = [st EXCEPT
       !.auditorOnline[leg] = FALSE,
       !.auditorFaultsRemaining[leg] = @ - 1
     ]

RestoreAuditor(leg) ==
  /\ ~st.auditorOnline[leg]
  /\ st' = [st EXCEPT !.auditorOnline[leg] = TRUE]

ImpairChannel(channel, leg, mode) ==
  /\ ProtocolActive
  /\ mode \in {"Hold", "Drop", "Delay"}
  /\ st.channelFaultsRemaining[leg] > 0
  /\ st.networkMode[channel][leg] = "Deliver"
  /\ st' = [st EXCEPT
       !.networkMode[channel][leg] = mode,
       !.channelFaultsRemaining[leg] = @ - 1
     ]

HealChannel(channel, leg) ==
  /\ st.networkMode[channel][leg] # "Deliver"
  /\ st' = [st EXCEPT !.networkMode[channel][leg] = "Deliver"]

(***************************************************************************
Crash/restart boundaries.  Every crash records a lower bound for all durable
objects.  Subsequent protocol work may add objects, but restart cannot remove
anything at or above that floor.  Abort/expiry are the only intentional staged
lock release, and the invariant treats that release separately.
***************************************************************************)

CommitteeBoundaryReached(leg, boundary) ==
  \/ /\ boundary = "Sidecar"
     /\ leg \in st.sidecarDurable
  \/ /\ boundary = "StagedDelta"
     /\ leg \in st.stagedDurable
  \/ /\ boundary = "PrepareQC"
     /\ leg \in st.prepareQc
  \/ /\ boundary = "CommitQC"
     /\ leg \in st.commitQc

CoordinatorBoundaryReached(boundary) ==
  \/ /\ boundary = "PrepareQC"
     /\ st.prepareQc # {}
  \/ /\ boundary = "CommitQC"
     /\ st.commitQc # {}
  \/ /\ boundary = "KuraAppend"
     /\ st.carrierDurable

GlobalBoundaryReached(boundary) ==
  \/ /\ boundary = "KuraAppend"
     /\ st.carrierDurable
  \/ /\ boundary = "WSVApplication"
     /\ st.appliedLegs = Legs
  \/ /\ boundary = "ReceiptPublication"
     /\ st.receiptDurable

CrashCommitteeAt(leg, boundary) ==
  /\ boundary \in CommitteeBoundaries
  /\ CommitteeBoundaryReached(leg, boundary)
  /\ st.committeeOnline[leg]
  /\ st.committeeCrashesRemaining > 0
  /\ st' = [st EXCEPT
       !.committeeOnline[leg] = FALSE,
       !.committeeCrashesRemaining = @ - 1,
       !.crashedBoundaries = @ \cup {boundary},
       !.crashFloorSidecars = @ \cup st.sidecarDurable,
       !.crashFloorStaged = @ \cup st.stagedDurable,
       !.crashFloorPrepareQc = @ \cup st.prepareQc,
       !.crashFloorCommitQc = @ \cup st.commitQc,
       !.crashFloorCarrier = @ \/ st.carrierDurable,
       !.crashFloorApplied = @ \cup st.appliedLegs,
       !.crashFloorReplayMarker = @ \/ st.replayMarkerDurable,
       !.crashFloorReceipt = @ \/ st.receiptDurable
     ]

RestartCommittee(leg) ==
  /\ ~st.committeeOnline[leg]
  /\ st' = [st EXCEPT !.committeeOnline[leg] = TRUE]

CrashCoordinatorAt(boundary) ==
  /\ boundary \in CoordinatorBoundaries
  /\ CoordinatorBoundaryReached(boundary)
  /\ st.coordinatorOnline
  /\ st.coordinatorCrashesRemaining > 0
  /\ st' = [st EXCEPT
       !.coordinatorOnline = FALSE,
       !.coordinatorCrashesRemaining = @ - 1,
       !.crashedBoundaries = @ \cup {boundary},
       !.crashFloorSidecars = @ \cup st.sidecarDurable,
       !.crashFloorStaged = @ \cup st.stagedDurable,
       !.crashFloorPrepareQc = @ \cup st.prepareQc,
       !.crashFloorCommitQc = @ \cup st.commitQc,
       !.crashFloorCarrier = @ \/ st.carrierDurable,
       !.crashFloorApplied = @ \cup st.appliedLegs,
       !.crashFloorReplayMarker = @ \/ st.replayMarkerDurable,
       !.crashFloorReceipt = @ \/ st.receiptDurable
     ]

RestartCoordinator ==
  /\ ~st.coordinatorOnline
  /\ st' = [st EXCEPT !.coordinatorOnline = TRUE]

CrashGlobalAt(boundary) ==
  /\ boundary \in GlobalBoundaries
  /\ GlobalBoundaryReached(boundary)
  /\ st.globalOnline
  /\ st.globalCrashesRemaining > 0
  /\ st' = [st EXCEPT
       !.globalOnline = FALSE,
       !.globalCrashesRemaining = @ - 1,
       !.crashedBoundaries = @ \cup {boundary},
       !.crashFloorSidecars = @ \cup st.sidecarDurable,
       !.crashFloorStaged = @ \cup st.stagedDurable,
       !.crashFloorPrepareQc = @ \cup st.prepareQc,
       !.crashFloorCommitQc = @ \cup st.commitQc,
       !.crashFloorCarrier = @ \/ st.carrierDurable,
       !.crashFloorApplied = @ \cup st.appliedLegs,
       !.crashFloorReplayMarker = @ \/ st.replayMarkerDurable,
       !.crashFloorReceipt = @ \/ st.receiptDurable
     ]

RestartGlobal ==
  /\ ~st.globalOnline
  /\ st' = [st EXCEPT !.globalOnline = TRUE]

ProtocolStep ==
  \/ \E leg \in Legs:
       DeliverDaVotes(leg)
       \/ CertifySidecar(leg)
       \/ ApproveAudit(leg)
       \/ DurablyStageDelta(leg)
       \/ DeliverPrepareVotes(leg)
       \/ EquivocatePrepare(leg)
       \/ CertifyPrepareQc(leg)
       \/ DeliverCommitVotes(leg)
       \/ EquivocateCommit(leg)
       \/ CertifyCommitQc(leg)
  \/ SealCompleteBundleDigest
  \/ PersistCarrierInKura
  \/ DeliverGlobalVotes
  \/ ApplyGlobalStateTransaction
  \/ PublishReceipt
  \/ RejectReplay

FaultStep ==
  \/ \E leg \in Legs, validator \in Validators,
        kind \in {"Unavailable", "Byzantine"}:
       InjectValidatorFault(leg, validator, kind)
  \/ \E leg \in Legs: LoseAuditor(leg)
  \/ \E channel \in Channels, leg \in Legs,
        mode \in {"Hold", "Drop", "Delay"}:
       ImpairChannel(channel, leg, mode)

RecoveryStep ==
  \/ \E leg \in Legs, validator \in Validators:
       RestoreValidator(leg, validator)
  \/ \E leg \in Legs: RestoreAuditor(leg)
  \/ \E channel \in Channels, leg \in Legs: HealChannel(channel, leg)
  \/ \E leg \in Legs: RestartCommittee(leg)
  \/ RestartCoordinator
  \/ RestartGlobal

CrashStep ==
  \/ \E leg \in Legs, boundary \in CommitteeBoundaries:
       CrashCommitteeAt(leg, boundary)
  \/ \E boundary \in CoordinatorBoundaries:
       CrashCoordinatorAt(boundary)
  \/ \E boundary \in GlobalBoundaries:
       CrashGlobalAt(boundary)

TerminalIdle ==
  /\ \/ st.phase \in TerminalFailurePhases
     \/ /\ st.phase = "Finalized"
        /\ st.receiptDurable
        /\ st.replayRejected
  /\ UNCHANGED st

Next ==
  \/ ProtocolStep
  \/ FaultStep
  \/ RecoveryStep
  \/ CrashStep
  \/ AbortInvalidBundle
  \/ ExpireBundle
  \/ TerminalIdle

Spec == Init /\ [][Next]_vars

FairSpec ==
  Spec
  /\ WF_vars(ProtocolStep)
  /\ WF_vars(RecoveryStep)

(***************************************************************************
Safety, durability, expiry, replay, and bounded-liveness properties.
***************************************************************************)

TypeOK ==
  /\ st.phase \in Phases
  /\ st.height \in Nat
  /\ st.validatorStatus \in [Legs -> [Validators -> ValidatorStates]]
  /\ st.faultIdentity \in [Legs -> 0..CommitteeSize]
  /\ st.auditorOnline \in [Legs -> BOOLEAN]
  /\ st.networkMode \in [Channels -> [Legs -> NetworkModes]]
  /\ st.committeeOnline \in [Legs -> BOOLEAN]
  /\ st.coordinatorOnline \in BOOLEAN
  /\ st.globalOnline \in BOOLEAN
  /\ st.daVotes \in [Legs -> SUBSET Validators]
  /\ st.sidecarDurable \subseteq Legs
  /\ st.auditApproved \subseteq Legs
  /\ st.stagedDurable \subseteq Legs
  /\ st.prepareVotes \in [Legs -> [Digests -> SUBSET Validators]]
  /\ st.prepareQc \subseteq Legs
  /\ st.bundleDigestDurable \in BOOLEAN
  /\ st.commitVotes \in [Legs -> [Digests -> SUBSET Validators]]
  /\ st.commitQc \subseteq Legs
  /\ st.carrierDurable \in BOOLEAN
  /\ st.globalVotes \subseteq GlobalValidators
  /\ st.appliedLegs \subseteq Legs
  /\ st.replayMarkerDurable \in BOOLEAN
  /\ st.applyCount \in 0..1
  /\ st.receiptDurable \in BOOLEAN
  /\ st.replayRejected \in BOOLEAN
  /\ st.invalidRejected \in BOOLEAN
  /\ st.validatorFaultsRemaining \in
       [Legs -> 0..MaxValidatorFaultsPerLeg]
  /\ st.auditorFaultsRemaining \in
       [Legs -> 0..MaxAuditorFaultsPerLeg]
  /\ st.channelFaultsRemaining \in
       [Legs -> 0..MaxChannelFaultsPerLeg]
  /\ st.committeeCrashesRemaining \in 0..MaxCommitteeCrashes
  /\ st.coordinatorCrashesRemaining \in 0..MaxCoordinatorCrashes
  /\ st.globalCrashesRemaining \in 0..MaxGlobalCrashes
  /\ st.crashedBoundaries \subseteq PersistenceBoundaries
  /\ st.crashFloorSidecars \subseteq Legs
  /\ st.crashFloorStaged \subseteq Legs
  /\ st.crashFloorPrepareQc \subseteq Legs
  /\ st.crashFloorCommitQc \subseteq Legs
  /\ st.crashFloorCarrier \in BOOLEAN
  /\ st.crashFloorApplied \subseteq Legs
  /\ st.crashFloorReplayMarker \in BOOLEAN
  /\ st.crashFloorReceipt \in BOOLEAN

APSOrderedUniqueLegs ==
  /\ Len(CanonicalOrder) = LegCount
  /\ DOMAIN CanonicalOrder = Legs
  /\ \A left, right \in Legs:
       (CanonicalOrder[left] = CanonicalOrder[right]) => left = right
  /\ \A left, right \in Legs:
       left < right => CanonicalOrder[left] < CanonicalOrder[right]

APSExactCommitteesAndFaultBound ==
  /\ CommitteeSize = 4
  /\ Quorum = 3
  /\ \A leg \in Legs: Cardinality(FaultyValidators(leg)) <= 1
  /\ \A leg \in Legs:
       \A validator \in FaultyValidators(leg):
         st.faultIdentity[leg] = validator

APSAuthenticatedCommitteeVotes ==
  /\ \A leg \in Legs:
       st.daVotes[leg] \subseteq Validators
  /\ \A leg \in Legs, digest \in Digests:
       /\ st.prepareVotes[leg][digest] \subseteq Validators
       /\ st.commitVotes[leg][digest] \subseteq Validators
  /\ \A leg \in Legs:
       st.prepareVotes[leg]["Equivocated"] \subseteq
         IF st.faultIdentity[leg] = 0
           THEN {}
           ELSE {st.faultIdentity[leg]}
  /\ \A leg \in Legs:
       st.commitVotes[leg]["Equivocated"] \subseteq
         IF st.faultIdentity[leg] = 0
           THEN {}
           ELSE {st.faultIdentity[leg]}

APSQuorumAuthenticity ==
  /\ \A leg \in st.sidecarDurable:
       Cardinality(st.daVotes[leg]) >= Quorum
  /\ \A leg \in st.prepareQc:
       /\ (st.phase \notin TerminalFailurePhases =>
            leg \in st.stagedDurable)
       /\ Cardinality(st.prepareVotes[leg]["Bundle"]) >= Quorum
  /\ \A leg \in st.commitQc:
       /\ st.prepareQc = Legs
       /\ st.bundleDigestDurable
       /\ Cardinality(st.commitVotes[leg]["Bundle"]) >= Quorum
  /\ \A leg \in Legs:
       Cardinality(st.prepareVotes[leg]["Equivocated"]) < Quorum
  /\ \A leg \in Legs:
       Cardinality(st.commitVotes[leg]["Equivocated"]) < Quorum

APSAllPrepareAndCommitBarriers ==
  /\ st.bundleDigestDurable => st.prepareQc = Legs
  /\ st.commitQc # {} =>
       st.bundleDigestDurable /\ st.prepareQc = Legs
  /\ st.phase \in {"Prepared", "CommitCertified", "Finalized"} =>
       st.bundleDigestDurable /\ st.prepareQc = Legs
  /\ st.phase \in {"CommitCertified", "Finalized"} =>
       st.commitQc = Legs

APSAtomicVisibility ==
  st.appliedLegs = {} \/ st.appliedLegs = Legs

APSNoEarlyVisibility ==
  st.appliedLegs # {} =>
    /\ st.phase = "Finalized"
    /\ st.commitQc = Legs
    /\ st.carrierDurable
    /\ Cardinality(st.globalVotes) >= GlobalQuorum
    /\ st.replayMarkerDurable
    /\ st.applyCount = 1

APSIdempotencyAndReplay ==
  /\ (st.replayMarkerDurable <=> st.appliedLegs = Legs)
  /\ (st.applyCount = 1 <=> st.appliedLegs = Legs)
  /\ st.replayRejected =>
       \/ /\ st.phase = "Finalized"
          /\ st.appliedLegs = Legs
          /\ st.applyCount = 1
       \/ /\ st.phase = "Expired"
          /\ st.appliedLegs = {}
          /\ st.applyCount = 0

APSTerminalFailureIsByteSilent ==
  st.phase \in TerminalFailurePhases =>
    /\ st.appliedLegs = {}
    /\ ~st.replayMarkerDurable
    /\ st.applyCount = 0
    /\ ~st.receiptDurable
    /\ st.stagedDurable = {}

APSReceiptFollowsAtomicState ==
  st.receiptDurable =>
    /\ st.phase = "Finalized"
    /\ st.appliedLegs = Legs
    /\ st.replayMarkerDurable
    /\ st.applyCount = 1

APSCrashDurability ==
  /\ st.crashFloorSidecars \subseteq st.sidecarDurable
  /\ (st.phase \notin TerminalFailurePhases =>
       st.crashFloorStaged \subseteq st.stagedDurable)
  /\ st.crashFloorPrepareQc \subseteq st.prepareQc
  /\ st.crashFloorCommitQc \subseteq st.commitQc
  /\ (st.crashFloorCarrier => st.carrierDurable)
  /\ st.crashFloorApplied \subseteq st.appliedLegs
  /\ (st.crashFloorReplayMarker => st.replayMarkerDurable)
  /\ (st.crashFloorReceipt => st.receiptDurable)

Safety ==
  /\ TypeOK
  /\ APSOrderedUniqueLegs
  /\ APSExactCommitteesAndFaultBound
  /\ APSAuthenticatedCommitteeVotes
  /\ APSQuorumAuthenticity
  /\ APSAllPrepareAndCommitBarriers
  /\ APSAtomicVisibility
  /\ APSNoEarlyVisibility
  /\ APSIdempotencyAndReplay
  /\ APSTerminalFailureIsByteSilent
  /\ APSReceiptFollowsAtomicState
  /\ APSCrashDurability

BoundedLivenessAssumptions ==
  /\ ~AllowAbort
  /\ ~AllowExpiry
  /\ GlobalAvailableValidators >= GlobalQuorum
  /\ ExpiryHeight > InitialHeight

APSEventuallyFinalizedAndPublished ==
  BoundedLivenessAssumptions =>
    <> (st.phase = "Finalized" /\ st.receiptDurable /\ st.replayRejected)

APSExpiryEventuallyRejectsReplay ==
  (st.phase = "Expired") ~> st.replayRejected

=============================================================================

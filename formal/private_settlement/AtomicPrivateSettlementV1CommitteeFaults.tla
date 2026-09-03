---- MODULE AtomicPrivateSettlementV1CommitteeFaults ----
EXTENDS Naturals, FiniteSets, Sequences

(***************************************************************************
Committee-indexed refinement model for AtomicPrivateSettlementV1.

Unlike the count-symmetry model in AtomicPrivateSettlementV1.tla, this model
retains every ordered leg, its independent four-validator committee, its local
auditor, and its DA/Prepare/Commit channels.  It is intended for bounded TLC
runs at small N.  The protocol parameter remains valid for every LegCount in
2..255; the count-symmetry model is still used for an exhaustive 255-leg run.

The one possible faulty identity in each committee is stable.  Because every
validator is exchangeable in this model and votes are introduced only as exact
sets, validator 1 is the canonical representative for that identity.  This is
an in-specification quotient, not TLC's SYMMETRY mechanism: the pinned TLC
version does not soundly support symmetry reduction for liveness checking.
The representative may be unavailable or may equivocate, and may recover and
fail again when the configured budget permits, but it cannot be replaced by a
second faulty identity in the same committee.

Network Hold, Drop, and Delay refine to the same abstract impairment: delivery
is blocked until HealChannel, after which authenticated retransmission occurs.
The real-process matrix retains and distinguishes the three concrete controller
modes.  This abstraction does not claim kind-specific timing, release, or retry
semantics.  Votes can only be introduced by exact committee members.  An
equivocator may sign both modeled digests, but one equivocator cannot certify
the non-bundle digest.

Crash actions take the relevant committee processor, coordinator, or global
processor offline.  APSDurabilityTemporal checks every transition and requires
all durable values to survive, except for the specified staged-lock release on
finalization, abort, or expiry.  The complete Prepare barrier is an exact
bundle identity replicated to every global validator before any Commit QC can
be certified.
Finalization, abort, and expiry release that live registration; each Commit QC
retains its binding to the same identity.  The two mutation constants are
FALSE in every positive configuration.
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
  AllowExpiry,
  DropDurableStageOnCommitteeCrash,
  CommitWithoutPrepareRegistration

Legs == 1..LegCount
Validators == 1..CommitteeSize
GlobalValidators == 1..GlobalCommitteeSize
AvailableGlobalValidators == 1..GlobalAvailableValidators

Phases ==
  {"Collecting", "Audited", "Prepared", "CommitCertified",
   "Finalized", "Aborted", "Expired"}
TerminalFailurePhases == {"Aborted", "Expired"}
TerminalPhases == {"Finalized", "Aborted", "Expired"}
ValidatorStates == {"Honest", "Unavailable", "Byzantine"}
Digests == {"Bundle", "Equivocated"}
Channels == {"DA", "Prepare", "Commit"}
NetworkModes == {"Deliver", "Impaired"}
CanonicalFaultValidator == 1
CommitteeBoundaries == {"Sidecar", "StagedDelta", "PrepareQC", "CommitQC"}
CoordinatorBoundaries ==
  {"PrepareQC", "PrepareRegistration", "CommitQC", "KuraAppend"}
GlobalBoundaries ==
  {"PrepareRegistration", "KuraAppend", "WSVApplication", "ReceiptPublication"}

(* The ordinal is the abstract route.  Thus the function is both the ordered
   bundle catalogue and a uniqueness witness for participant dataspaces. *)
CanonicalOrder == [ordinal \in Legs |-> ordinal]

NoBundleIdentity ==
  [digest |-> "None", orderedLegs |-> <<>>, prepareQc |-> {}]
CompleteBundleIdentity ==
  [digest |-> "Bundle", orderedLegs |-> CanonicalOrder, prepareQc |-> Legs]
BundleIdentities == {NoBundleIdentity, CompleteBundleIdentity}
EmptyPrepareRegistration ==
  [validator \in GlobalValidators |-> NoBundleIdentity]
CompletePrepareRegistration ==
  [validator \in GlobalValidators |-> CompleteBundleIdentity]

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
  /\ DropDurableStageOnCommitteeCrash \in BOOLEAN
  /\ CommitWithoutPrepareRegistration \in BOOLEAN
  /\ st = [
       phase |-> "Collecting",
       validatorStatus |->
         [leg \in Legs |-> [validator \in Validators |-> "Honest"]],
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
       prepareRegistration |-> EmptyPrepareRegistration,
       commitVotes |-> EmptyVotes,
       commitQc |-> {},
       commitBinding |-> [leg \in Legs |-> NoBundleIdentity],
       carrierDurable |-> FALSE,
       globalVotes |-> {},
       appliedLegs |-> {},
       replayMarkerDurable |-> FALSE,
       applyCount |-> 0,
       receiptDurable |-> FALSE,
       replayRejected |-> FALSE,
       validatorFaultsRemaining |->
         [leg \in Legs |-> MaxValidatorFaultsPerLeg],
       auditorFaultsRemaining |->
         [leg \in Legs |-> MaxAuditorFaultsPerLeg],
       channelFaultsRemaining |->
         [leg \in Legs |-> MaxChannelFaultsPerLeg],
       committeeCrashesRemaining |-> MaxCommitteeCrashes,
       coordinatorCrashesRemaining |-> MaxCoordinatorCrashes,
       globalCrashesRemaining |-> MaxGlobalCrashes
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

(* Faults are introduced at the first protocol-visible point where they can
   affect delivery or voting.  Injecting the same fault earlier is trace-
   equivalent because no deadline or wall-clock observation exists here; a
   fault after the corresponding certificate is future-inert. *)
ValidatorVotePending(leg) ==
  \/ /\ st.phase = "Collecting"
     /\ leg \notin st.sidecarDurable
  \/ /\ st.phase = "Audited"
     /\ leg \notin st.prepareQc
  \/ /\ st.phase = "Prepared"
     /\ leg \notin st.commitQc

ChannelDeliveryPending(channel, leg) ==
  \/ /\ channel = "DA"
     /\ st.phase = "Collecting"
     /\ leg \notin st.sidecarDurable
  \/ /\ channel = "Prepare"
     /\ st.phase = "Audited"
     /\ leg \notin st.prepareQc
  \/ /\ channel = "Commit"
     /\ st.phase = "Prepared"
     /\ leg \notin st.commitQc

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
       !.sidecarDurable = @ \cup {leg},
       !.daVotes[leg] = {},
       !.networkMode["DA"][leg] = "Deliver"
     ]

ApproveAudit(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Collecting"
  /\ leg \in st.sidecarDurable
  /\ st.auditorOnline[leg]
  /\ leg \notin st.auditApproved
  /\ st' = [st EXCEPT
       !.auditApproved = @ \cup {leg},
       !.auditorFaultsRemaining[leg] = 0,
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
  /\ leg \notin st.prepareQc
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
  /\ leg \notin st.prepareQc
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
       !.prepareQc = @ \cup {leg},
       !.prepareVotes[leg] = [digest \in Digests |-> {}],
       !.networkMode["Prepare"][leg] = "Deliver"
     ]

SealCompleteBundleDigest ==
  /\ ProtocolActive
  /\ st.phase = "Audited"
  /\ st.coordinatorOnline
  /\ st.prepareQc = Legs
  /\ ~st.bundleDigestDurable
  /\ st' = [st EXCEPT
       !.bundleDigestDurable = TRUE
     ]

(* This action abstracts finality of the Prepare-registration carrier.  The
   state effect is one atomic, globally replicated lock over the exact ordered
   bundle identity. *)
RegisterCompletePrepareBundle ==
  /\ ProtocolActive
  /\ ~CommitWithoutPrepareRegistration
  /\ st.phase = "Audited"
  /\ st.coordinatorOnline
  /\ st.globalOnline
  /\ st.prepareQc = Legs
  /\ st.bundleDigestDurable
  /\ Cardinality(AvailableGlobalValidators) >= GlobalQuorum
  /\ st.prepareRegistration = EmptyPrepareRegistration
  /\ st' = [st EXCEPT
       !.prepareRegistration = CompletePrepareRegistration,
       !.phase = "Prepared"
     ]

(* Deliberate negative control: open Commit while the complete Prepare bundle
   is absent from replicated global state. *)
OpenCommitWithoutPrepareRegistration ==
  /\ ProtocolActive
  /\ CommitWithoutPrepareRegistration
  /\ st.phase = "Audited"
  /\ st.coordinatorOnline
  /\ st.prepareQc = Legs
  /\ st.bundleDigestDurable
  /\ st.prepareRegistration = EmptyPrepareRegistration
  /\ st' = [st EXCEPT
       !.phase = "Prepared"
     ]

CommitRegistrationAvailable ==
  \/ st.prepareRegistration = CompletePrepareRegistration
  \/ /\ CommitWithoutPrepareRegistration
     /\ st.prepareRegistration = EmptyPrepareRegistration

DeliverCommitVotes(leg) ==
  /\ ProtocolActive
  /\ st.phase = "Prepared"
  /\ st.committeeOnline[leg]
  /\ st.networkMode["Commit"][leg] = "Deliver"
  /\ st.prepareQc = Legs
  /\ st.bundleDigestDurable
  /\ CommitRegistrationAvailable
  /\ leg \notin st.commitQc
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
  /\ CommitRegistrationAvailable
  /\ leg \notin st.commitQc
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
  /\ CommitRegistrationAvailable
  /\ leg \notin st.commitQc
  /\ Cardinality(st.commitVotes[leg]["Bundle"]) >= Quorum
  /\ st' = [st EXCEPT
       !.commitQc = @ \cup {leg},
       !.commitBinding[leg] =
         IF st.prepareRegistration = CompletePrepareRegistration
           THEN CompleteBundleIdentity
           ELSE NoBundleIdentity,
       !.commitVotes[leg] = [digest \in Digests |-> {}],
       !.validatorStatus[leg] =
         [validator \in Validators |-> "Honest"],
       !.validatorFaultsRemaining[leg] = 0,
       !.auditorOnline[leg] = TRUE,
       !.auditorFaultsRemaining[leg] = 0,
       !.networkMode["DA"][leg] = "Deliver",
       !.networkMode["Prepare"][leg] = "Deliver",
       !.networkMode["Commit"][leg] = "Deliver",
       !.channelFaultsRemaining[leg] = 0,
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
  /\ st.prepareRegistration = CompletePrepareRegistration
  /\ \A leg \in Legs: st.commitBinding[leg] = CompleteBundleIdentity
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
  /\ st.prepareRegistration = CompletePrepareRegistration
  /\ \A leg \in Legs: st.commitBinding[leg] = CompleteBundleIdentity
  /\ Cardinality(st.globalVotes) >= GlobalQuorum
  /\ st.applyCount = 0
  /\ ~st.replayMarkerDurable
  /\ st' = [st EXCEPT
       !.appliedLegs = Legs,
       !.replayMarkerDurable = TRUE,
       !.applyCount = 1,
       !.stagedDurable = {},
       !.prepareRegistration = EmptyPrepareRegistration,
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
  /\ st.phase \in TerminalPhases
  /\ ~st.replayRejected
  /\ st' = [st EXCEPT !.replayRejected = TRUE]

AbortInvalidBundle ==
  /\ AllowAbort
  /\ ProtocolActive
  /\ st.phase \in {"Collecting", "Audited", "Prepared"}
  /\ st' = [st EXCEPT
       !.phase = "Aborted",
       !.stagedDurable = {},
       !.prepareRegistration = EmptyPrepareRegistration
     ]

ExpireBundle ==
  /\ AllowExpiry
  /\ ProtocolActive
  /\ st' = [st EXCEPT
       !.phase = "Expired",
       !.stagedDurable = {},
       !.prepareRegistration = EmptyPrepareRegistration
     ]

(***************************************************************************
Independent validator, auditor, and authenticated-channel faults.  Each leg
has its own finite budget, so one configured fault can be active in every
committee simultaneously without permitting a mobile validator identity.
Healing/restart actions are weakly fair in FairSpec.
***************************************************************************)

InjectValidatorFault(leg, kind) ==
  /\ ProtocolActive
  /\ ValidatorVotePending(leg)
  /\ kind \in {"Unavailable", "Byzantine"}
  /\ st.validatorFaultsRemaining[leg] > 0
  /\ st.validatorStatus[leg][CanonicalFaultValidator] = "Honest"
  /\ FaultyValidators(leg) = {}
  /\ st' = [st EXCEPT
       !.validatorStatus[leg][CanonicalFaultValidator] = kind,
       !.validatorFaultsRemaining[leg] = @ - 1
     ]

RestoreValidator(leg) ==
  /\ st.validatorStatus[leg][CanonicalFaultValidator] # "Honest"
  /\ st' = [st EXCEPT
       !.validatorStatus[leg][CanonicalFaultValidator] = "Honest"
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

ImpairChannel(channel, leg) ==
  /\ ProtocolActive
  /\ ChannelDeliveryPending(channel, leg)
  /\ st.channelFaultsRemaining[leg] > 0
  /\ st.networkMode[channel][leg] = "Deliver"
  /\ st' = [st EXCEPT
       !.networkMode[channel][leg] = "Impaired",
       !.channelFaultsRemaining[leg] = @ - 1
     ]

HealChannel(channel, leg) ==
  /\ st.networkMode[channel][leg] # "Deliver"
  /\ st' = [st EXCEPT !.networkMode[channel][leg] = "Deliver"]

(***************************************************************************
Crash/restart boundaries.  Durable values are constrained by the temporal
action property below rather than copied into history-only floor fields.  The
mutation is restricted to the StagedDelta boundary so its negative control
isolates loss of a staged record without violating another safety invariant.
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
  \/ /\ boundary = "PrepareRegistration"
     /\ st.prepareRegistration = CompletePrepareRegistration
  \/ /\ boundary = "CommitQC"
     /\ st.commitQc # {}
  \/ /\ boundary = "KuraAppend"
     /\ st.carrierDurable

GlobalBoundaryReached(boundary) ==
  \/ /\ boundary = "PrepareRegistration"
     /\ st.prepareRegistration = CompletePrepareRegistration
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
  /\ (DropDurableStageOnCommitteeCrash => boundary = "StagedDelta")
  /\ st' = [st EXCEPT
       !.committeeOnline[leg] = FALSE,
       !.committeeCrashesRemaining = @ - 1,
       !.stagedDurable =
         IF DropDurableStageOnCommitteeCrash THEN @ \ {leg} ELSE @
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
       !.coordinatorCrashesRemaining = @ - 1
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
       !.globalCrashesRemaining = @ - 1
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
  \/ RegisterCompletePrepareBundle
  \/ OpenCommitWithoutPrepareRegistration
  \/ PersistCarrierInKura
  \/ DeliverGlobalVotes
  \/ ApplyGlobalStateTransaction
  \/ PublishReceipt
  \/ RejectReplay

FaultStep ==
  \/ \E leg \in Legs, kind \in {"Unavailable", "Byzantine"}:
       InjectValidatorFault(leg, kind)
  \/ \E leg \in Legs: LoseAuditor(leg)
  \/ \E channel \in Channels, leg \in Legs:
       ImpairChannel(channel, leg)

RecoveryStep ==
  \/ \E leg \in Legs: RestoreValidator(leg)
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
  /\ st.validatorStatus \in [Legs -> [Validators -> ValidatorStates]]
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
  /\ st.prepareRegistration \in [GlobalValidators -> BundleIdentities]
  /\ st.commitVotes \in [Legs -> [Digests -> SUBSET Validators]]
  /\ st.commitQc \subseteq Legs
  /\ st.commitBinding \in [Legs -> BundleIdentities]
  /\ st.carrierDurable \in BOOLEAN
  /\ st.globalVotes \subseteq GlobalValidators
  /\ st.appliedLegs \subseteq Legs
  /\ st.replayMarkerDurable \in BOOLEAN
  /\ st.applyCount \in 0..1
  /\ st.receiptDurable \in BOOLEAN
  /\ st.replayRejected \in BOOLEAN
  /\ st.validatorFaultsRemaining \in
       [Legs -> 0..MaxValidatorFaultsPerLeg]
  /\ st.auditorFaultsRemaining \in
       [Legs -> 0..MaxAuditorFaultsPerLeg]
  /\ st.channelFaultsRemaining \in
       [Legs -> 0..MaxChannelFaultsPerLeg]
  /\ st.committeeCrashesRemaining \in 0..MaxCommitteeCrashes
  /\ st.coordinatorCrashesRemaining \in 0..MaxCoordinatorCrashes
  /\ st.globalCrashesRemaining \in 0..MaxGlobalCrashes

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
  /\ CanonicalFaultValidator \in Validators
  /\ \A leg \in Legs:
       FaultyValidators(leg) \subseteq {CanonicalFaultValidator}
  /\ \A leg \in Legs: Cardinality(FaultyValidators(leg)) <= 1

APSAuthenticatedCommitteeVotes ==
  /\ \A leg \in Legs:
       st.daVotes[leg] \subseteq Validators
  /\ \A leg \in Legs, digest \in Digests:
       /\ st.prepareVotes[leg][digest] \subseteq Validators
       /\ st.commitVotes[leg][digest] \subseteq Validators
  /\ \A leg \in Legs:
       st.prepareVotes[leg]["Equivocated"] \subseteq
         {CanonicalFaultValidator}
  /\ \A leg \in Legs:
       st.commitVotes[leg]["Equivocated"] \subseteq
         {CanonicalFaultValidator}

APSQuorumAuthenticity ==
  /\ \A leg \in st.prepareQc:
       st.phase \notin TerminalPhases =>
         leg \in st.stagedDurable
  /\ \A leg \in st.commitQc:
       /\ st.prepareQc = Legs
       /\ st.bundleDigestDurable
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

APSPrepareRegistrationAndCommitBinding ==
  /\ st.prepareRegistration \in
       {EmptyPrepareRegistration, CompletePrepareRegistration}
  /\ st.prepareRegistration = CompletePrepareRegistration =>
       st.bundleDigestDurable /\ st.prepareQc = Legs
  /\ st.commitQc # {} /\ st.phase \notin TerminalPhases =>
       st.prepareRegistration = CompletePrepareRegistration
  /\ st.phase = "CommitCertified" =>
       st.prepareRegistration = CompletePrepareRegistration
  /\ st.phase \in TerminalPhases =>
       st.prepareRegistration = EmptyPrepareRegistration
  /\ \A leg \in Legs:
       IF leg \in st.commitQc
         THEN st.commitBinding[leg] = CompleteBundleIdentity
         ELSE st.commitBinding[leg] = NoBundleIdentity

APSAtomicVisibility ==
  st.appliedLegs = {} \/ st.appliedLegs = Legs

APSNoEarlyVisibility ==
  st.appliedLegs # {} =>
    /\ st.phase = "Finalized"
    /\ st.commitQc = Legs
    /\ \A leg \in Legs: st.commitBinding[leg] = CompleteBundleIdentity
    /\ st.carrierDurable
    /\ Cardinality(st.globalVotes) >= GlobalQuorum
    /\ st.replayMarkerDurable
    /\ st.applyCount = 1

APSAtMostOnceAndReplay ==
  /\ (st.replayMarkerDurable <=> st.appliedLegs = Legs)
  /\ (st.applyCount = 1 <=> st.appliedLegs = Legs)
  /\ st.replayRejected =>
       \/ /\ st.phase = "Finalized"
          /\ st.appliedLegs = Legs
          /\ st.applyCount = 1
       \/ /\ st.phase = "Expired"
          /\ st.appliedLegs = {}
          /\ st.applyCount = 0
       \/ /\ st.phase = "Aborted"
          /\ st.appliedLegs = {}
          /\ st.applyCount = 0

APSTerminalFailureIsByteSilent ==
  st.phase \in TerminalFailurePhases =>
    /\ st.appliedLegs = {}
    /\ ~st.replayMarkerDurable
    /\ st.applyCount = 0
    /\ ~st.receiptDurable
    /\ st.stagedDurable = {}
    /\ st.prepareRegistration = EmptyPrepareRegistration

APSReceiptFollowsAtomicState ==
  st.receiptDurable =>
    /\ st.phase = "Finalized"
    /\ st.appliedLegs = Legs
    /\ st.replayMarkerDurable
    /\ st.applyCount = 1
    /\ st.stagedDurable = {}
    /\ st.prepareRegistration = EmptyPrepareRegistration
    /\ \A leg \in Legs: st.commitBinding[leg] = CompleteBundleIdentity

Safety ==
  /\ TypeOK
  /\ APSOrderedUniqueLegs
  /\ APSExactCommitteesAndFaultBound
  /\ APSAuthenticatedCommitteeVotes
  /\ APSQuorumAuthenticity
  /\ APSAllPrepareAndCommitBarriers
  /\ APSPrepareRegistrationAndCommitBinding
  /\ APSAtomicVisibility
  /\ APSNoEarlyVisibility
  /\ APSAtMostOnceAndReplay
  /\ APSTerminalFailureIsByteSilent
  /\ APSReceiptFollowsAtomicState

(* Durability is a property of every transition, not only states following an
   explicit modeled crash. Finalization, abort, and expiry may release staged
   locks and the Prepare registration; every other durable edge is monotonic. *)
DurableStep ==
  /\ st.sidecarDurable \subseteq st'.sidecarDurable
  /\ (st'.phase \notin TerminalPhases =>
       st.stagedDurable \subseteq st'.stagedDurable)
  /\ st.prepareQc \subseteq st'.prepareQc
  /\ st.commitQc \subseteq st'.commitQc
  /\ \A leg \in st.commitQc:
       st'.commitBinding[leg] = st.commitBinding[leg]
  /\ (st.bundleDigestDurable => st'.bundleDigestDurable)
  /\ (st.prepareRegistration = CompletePrepareRegistration /\
       st'.phase \notin TerminalPhases =>
       st'.prepareRegistration = st.prepareRegistration)
  /\ (st.carrierDurable => st'.carrierDurable)
  /\ st.appliedLegs \subseteq st'.appliedLegs
  /\ (st.replayMarkerDurable => st'.replayMarkerDurable)
  /\ st.applyCount <= st'.applyCount
  /\ (st.receiptDurable => st'.receiptDurable)

APSDurabilityTemporal == [][DurableStep]_vars

(* Every modeled crash and restart preserves the exact replicated Prepare
   registration. Protocol terminal actions may clear it, but none also toggles
   one of these process-online markers. *)
PrepareRegistrationCrashRecoveryStep ==
  /\ \A leg \in Legs:
       (st.committeeOnline[leg] # st'.committeeOnline[leg]) =>
         st'.prepareRegistration = st.prepareRegistration
  /\ (st.coordinatorOnline # st'.coordinatorOnline) =>
       st'.prepareRegistration = st.prepareRegistration
  /\ (st.globalOnline # st'.globalOnline) =>
       st'.prepareRegistration = st.prepareRegistration

APSPrepareRegistrationCrashRecoveryTemporal ==
  [][PrepareRegistrationCrashRecoveryStep]_vars

(* Vote collections are discarded after their exact three-of-four certificate
   marker is created.  Check provenance on the transition that creates each
   marker so post-certificate history does not multiply otherwise equivalent
   states. *)
CertificateQuorumStep ==
  /\ \A leg \in Legs:
       (leg \notin st.sidecarDurable /\ leg \in st'.sidecarDurable) =>
         Cardinality(st.daVotes[leg]) >= Quorum
  /\ \A leg \in Legs:
       (leg \notin st.prepareQc /\ leg \in st'.prepareQc) =>
         /\ leg \in st.stagedDurable
         /\ Cardinality(st.prepareVotes[leg]["Bundle"]) >= Quorum
  /\ \A leg \in Legs:
       (leg \notin st.commitQc /\ leg \in st'.commitQc) =>
         /\ st.prepareQc = Legs
         /\ st.bundleDigestDurable
         /\ st.prepareRegistration = CompletePrepareRegistration
         /\ st'.commitBinding[leg] = CompleteBundleIdentity
         /\ Cardinality(st.commitVotes[leg]["Bundle"]) >= Quorum

APSCertificateQuorumTemporal == [][CertificateQuorumStep]_vars

BoundedLivenessAssumptions ==
  /\ ~AllowAbort
  /\ ~AllowExpiry
  /\ ~CommitWithoutPrepareRegistration
  /\ GlobalAvailableValidators >= GlobalQuorum
  /\ ExpiryHeight > InitialHeight

APSEventuallyFinalizedAndPublished ==
  BoundedLivenessAssumptions =>
    <> (st.phase = "Finalized" /\ st.receiptDurable /\ st.replayRejected)

APSExpiryEventuallyRejectsReplay ==
  (st.phase = "Expired") ~> st.replayRejected

=============================================================================

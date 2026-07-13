---- MODULE SumeragiNativeAmxReceiptValidation ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for native AMX receipt validation.

This slice models the execution-context admission boundary formed by
`validate_native_amx_receipt_against_plan(...)`,
`validate_native_amx_attestation_qc(...)`, the caller-side check that native
AMX routing plans must carry receipts while single-route plans must not, and
the participant-slot application boundary. A native AMX receipt is accepted
only when its header, participant-leg set, each participant-local lane height,
lane view, proposal hash, settlement commitment hash, prepare/commit QC body,
validator-set hash, signer bitmap, quorum, proof of possession state, and
aggregate BLS signature all match the transaction, routing plan, dataspace
catalog, and finalized coordinator block. A participant lane frontier may
advance only after the exact certified participant application is durable.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried,
  \* @type: Bool;
  participantApplicationDurable,
  \* @type: Int;
  participantFrontierHeight

\* @type: <<Set(Int), Bool, Int>>;
vars == <<tried, participantApplicationDurable, participantFrontierHeight>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

ValidNativeReceipt == 1
ValidSingleNoReceipt == 2
NativeMissingReceipt == 3
NativeUnsignedEntrypoint == 4
SingleUnexpectedReceipt == 5
UnsupportedVersion == 6
SourceMismatch == 7
CoordinatorMismatch == 8
HeightMismatch == 9
PlanDigestMismatch == 10
MissingParticipantLeg == 11
UnexpectedParticipantLeg == 12
DuplicateParticipantLeg == 13
QcSourceMismatch == 14
QcEntrypointMismatch == 15
QcPlanDigestMismatch == 16
QcWrongPhase == 17
QcCoordinatorMismatch == 18
QcParticipantMismatch == 19
QcHeightMismatch == 20
ValidatorHashVersionMismatch == 21
ValidatorSetHashMismatch == 22
UnknownParticipantDataspace == 23
ValidatorSetTooSmall == 24
SignerBitmapLengthMismatch == 25
SignerIndexOutOfBounds == 26
SignerNotBls == 27
SignerMissingPop == 28
QuorumNotMet == 29
MissingAggregateSignature == 30
InvalidAggregateSignature == 31
ValidDistinctParticipantSlots == 32
ParticipantHeightMismatch == 33
ParticipantViewMismatch == 34
ParticipantProposalHashMismatch == 35
ParticipantSettlementHashMismatch == 36
ParticipantProposalSlotConflict == 37
ParticipantSettlementSlotConflict == 38
PrepareCommitIdentityMismatch == 39

Candidates == 1..39

ParticipantPredecessorHeight == 6
ParticipantBlockHeight == 7
ParticipantBlockView == 0

CoordinatorSlot == [
  height |-> 11,
  view |-> 2,
  proposal |-> "coordinator-proposal",
  settlement |-> "coordinator-settlement"
]

ParticipantSlot == [
  height |-> ParticipantBlockHeight,
  view |-> ParticipantBlockView,
  proposal |-> "participant-proposal",
  settlement |-> "participant-settlement"
]

ReceiptParticipantSlot(candidate) == [
  height |-> IF candidate = ParticipantHeightMismatch
             THEN ParticipantBlockHeight + 1
             ELSE ParticipantSlot.height,
  view |-> IF candidate = ParticipantViewMismatch
           THEN ParticipantBlockView + 1
           ELSE ParticipantSlot.view,
  proposal |-> IF candidate \in {
                  ParticipantProposalHashMismatch,
                  ParticipantProposalSlotConflict
                }
              THEN "conflicting-participant-proposal"
              ELSE ParticipantSlot.proposal,
  settlement |-> IF candidate \in {
                    ParticipantSettlementHashMismatch,
                    ParticipantSettlementSlotConflict
                  }
                THEN "conflicting-participant-settlement"
                ELSE ParticipantSlot.settlement
]

PrepareParticipantIdentity == ParticipantSlot

CommitParticipantIdentity(candidate) ==
  IF candidate = PrepareCommitIdentityMismatch
  THEN [ParticipantSlot EXCEPT !.settlement = "commit-only-settlement"]
  ELSE ParticipantSlot

NoBug == 0
AcceptNativeMissingReceiptBug == 1
RejectValidSingleNoReceiptBug == 2
AcceptSingleUnexpectedReceiptBug == 3
AcceptUnsignedEntrypointBug == 4
AcceptUnsupportedVersionBug == 5
AcceptSourceMismatchBug == 6
AcceptCoordinatorMismatchBug == 7
AcceptHeightMismatchBug == 8
AcceptPlanDigestMismatchBug == 9
AcceptMissingParticipantLegBug == 10
AcceptUnexpectedParticipantLegBug == 11
AcceptDuplicateParticipantLegBug == 12
AcceptQcSourceMismatchBug == 13
AcceptQcEntrypointMismatchBug == 14
AcceptQcPlanDigestMismatchBug == 15
AcceptQcWrongPhaseBug == 16
AcceptQcCoordinatorMismatchBug == 17
AcceptQcParticipantMismatchBug == 18
AcceptQcHeightMismatchBug == 19
AcceptValidatorHashVersionMismatchBug == 20
AcceptValidatorSetHashMismatchBug == 21
AcceptUnknownParticipantDataspaceBug == 22
AcceptValidatorSetTooSmallBug == 23
AcceptSignerBitmapLengthMismatchBug == 24
AcceptSignerIndexOutOfBoundsBug == 25
AcceptSignerNotBlsBug == 26
AcceptSignerMissingPopBug == 27
AcceptQuorumNotMetBug == 28
AcceptMissingAggregateSignatureBug == 29
AcceptInvalidAggregateSignatureBug == 30
RejectValidNativeReceiptBug == 31
AcceptParticipantHeightMismatchBug == 32
AcceptParticipantViewMismatchBug == 33
AcceptParticipantProposalHashMismatchBug == 34
AcceptParticipantSettlementHashMismatchBug == 35
AcceptParticipantProposalSlotConflictBug == 36
AcceptParticipantSettlementSlotConflictBug == 37
AcceptPrepareCommitIdentityMismatchBug == 38
AdvanceFrontierBeforeApplicationBug == 39

Bugs == 0..39

BugAcceptNativeMissingReceipt == Bug = AcceptNativeMissingReceiptBug
BugRejectValidSingleNoReceipt == Bug = RejectValidSingleNoReceiptBug
BugAcceptSingleUnexpectedReceipt == Bug = AcceptSingleUnexpectedReceiptBug
BugAcceptUnsignedEntrypoint == Bug = AcceptUnsignedEntrypointBug
BugAcceptUnsupportedVersion == Bug = AcceptUnsupportedVersionBug
BugAcceptSourceMismatch == Bug = AcceptSourceMismatchBug
BugAcceptCoordinatorMismatch == Bug = AcceptCoordinatorMismatchBug
BugAcceptHeightMismatch == Bug = AcceptHeightMismatchBug
BugAcceptPlanDigestMismatch == Bug = AcceptPlanDigestMismatchBug
BugAcceptMissingParticipantLeg == Bug = AcceptMissingParticipantLegBug
BugAcceptUnexpectedParticipantLeg == Bug = AcceptUnexpectedParticipantLegBug
BugAcceptDuplicateParticipantLeg == Bug = AcceptDuplicateParticipantLegBug
BugAcceptQcSourceMismatch == Bug = AcceptQcSourceMismatchBug
BugAcceptQcEntrypointMismatch == Bug = AcceptQcEntrypointMismatchBug
BugAcceptQcPlanDigestMismatch == Bug = AcceptQcPlanDigestMismatchBug
BugAcceptQcWrongPhase == Bug = AcceptQcWrongPhaseBug
BugAcceptQcCoordinatorMismatch == Bug = AcceptQcCoordinatorMismatchBug
BugAcceptQcParticipantMismatch == Bug = AcceptQcParticipantMismatchBug
BugAcceptQcHeightMismatch == Bug = AcceptQcHeightMismatchBug
BugAcceptValidatorHashVersionMismatch == Bug = AcceptValidatorHashVersionMismatchBug
BugAcceptValidatorSetHashMismatch == Bug = AcceptValidatorSetHashMismatchBug
BugAcceptUnknownParticipantDataspace == Bug = AcceptUnknownParticipantDataspaceBug
BugAcceptValidatorSetTooSmall == Bug = AcceptValidatorSetTooSmallBug
BugAcceptSignerBitmapLengthMismatch == Bug = AcceptSignerBitmapLengthMismatchBug
BugAcceptSignerIndexOutOfBounds == Bug = AcceptSignerIndexOutOfBoundsBug
BugAcceptSignerNotBls == Bug = AcceptSignerNotBlsBug
BugAcceptSignerMissingPop == Bug = AcceptSignerMissingPopBug
BugAcceptQuorumNotMet == Bug = AcceptQuorumNotMetBug
BugAcceptMissingAggregateSignature == Bug = AcceptMissingAggregateSignatureBug
BugAcceptInvalidAggregateSignature == Bug = AcceptInvalidAggregateSignatureBug
BugRejectValidNativeReceipt == Bug = RejectValidNativeReceiptBug
BugAcceptParticipantHeightMismatch == Bug = AcceptParticipantHeightMismatchBug
BugAcceptParticipantViewMismatch == Bug = AcceptParticipantViewMismatchBug
BugAcceptParticipantProposalHashMismatch ==
  Bug = AcceptParticipantProposalHashMismatchBug
BugAcceptParticipantSettlementHashMismatch ==
  Bug = AcceptParticipantSettlementHashMismatchBug
BugAcceptParticipantProposalSlotConflict ==
  Bug = AcceptParticipantProposalSlotConflictBug
BugAcceptParticipantSettlementSlotConflict ==
  Bug = AcceptParticipantSettlementSlotConflictBug
BugAcceptPrepareCommitIdentityMismatch ==
  Bug = AcceptPrepareCommitIdentityMismatchBug
BugAdvanceFrontierBeforeApplication ==
  Bug = AdvanceFrontierBeforeApplicationBug

SpecAccepted(candidate) ==
  candidate \in {
    ValidNativeReceipt,
    ValidSingleNoReceipt,
    ValidDistinctParticipantSlots
  }

ImplementationAccepted(candidate) ==
  \/ /\ candidate = ValidNativeReceipt
     /\ ~BugRejectValidNativeReceipt
  \/ /\ candidate = ValidSingleNoReceipt
     /\ ~BugRejectValidSingleNoReceipt
  \/ /\ candidate = NativeMissingReceipt
     /\ BugAcceptNativeMissingReceipt
  \/ /\ candidate = NativeUnsignedEntrypoint
     /\ BugAcceptUnsignedEntrypoint
  \/ /\ candidate = SingleUnexpectedReceipt
     /\ BugAcceptSingleUnexpectedReceipt
  \/ /\ candidate = UnsupportedVersion
     /\ BugAcceptUnsupportedVersion
  \/ /\ candidate = SourceMismatch
     /\ BugAcceptSourceMismatch
  \/ /\ candidate = CoordinatorMismatch
     /\ BugAcceptCoordinatorMismatch
  \/ /\ candidate = HeightMismatch
     /\ BugAcceptHeightMismatch
  \/ /\ candidate = PlanDigestMismatch
     /\ BugAcceptPlanDigestMismatch
  \/ /\ candidate = MissingParticipantLeg
     /\ BugAcceptMissingParticipantLeg
  \/ /\ candidate = UnexpectedParticipantLeg
     /\ BugAcceptUnexpectedParticipantLeg
  \/ /\ candidate = DuplicateParticipantLeg
     /\ BugAcceptDuplicateParticipantLeg
  \/ /\ candidate = QcSourceMismatch
     /\ BugAcceptQcSourceMismatch
  \/ /\ candidate = QcEntrypointMismatch
     /\ BugAcceptQcEntrypointMismatch
  \/ /\ candidate = QcPlanDigestMismatch
     /\ BugAcceptQcPlanDigestMismatch
  \/ /\ candidate = QcWrongPhase
     /\ BugAcceptQcWrongPhase
  \/ /\ candidate = QcCoordinatorMismatch
     /\ BugAcceptQcCoordinatorMismatch
  \/ /\ candidate = QcParticipantMismatch
     /\ BugAcceptQcParticipantMismatch
  \/ /\ candidate = QcHeightMismatch
     /\ BugAcceptQcHeightMismatch
  \/ /\ candidate = ValidatorHashVersionMismatch
     /\ BugAcceptValidatorHashVersionMismatch
  \/ /\ candidate = ValidatorSetHashMismatch
     /\ BugAcceptValidatorSetHashMismatch
  \/ /\ candidate = UnknownParticipantDataspace
     /\ BugAcceptUnknownParticipantDataspace
  \/ /\ candidate = ValidatorSetTooSmall
     /\ BugAcceptValidatorSetTooSmall
  \/ /\ candidate = SignerBitmapLengthMismatch
     /\ BugAcceptSignerBitmapLengthMismatch
  \/ /\ candidate = SignerIndexOutOfBounds
     /\ BugAcceptSignerIndexOutOfBounds
  \/ /\ candidate = SignerNotBls
     /\ BugAcceptSignerNotBls
  \/ /\ candidate = SignerMissingPop
     /\ BugAcceptSignerMissingPop
  \/ /\ candidate = QuorumNotMet
     /\ BugAcceptQuorumNotMet
  \/ /\ candidate = MissingAggregateSignature
     /\ BugAcceptMissingAggregateSignature
  \/ /\ candidate = InvalidAggregateSignature
     /\ BugAcceptInvalidAggregateSignature
  \/ candidate = ValidDistinctParticipantSlots
  \/ /\ candidate = ParticipantHeightMismatch
     /\ BugAcceptParticipantHeightMismatch
  \/ /\ candidate = ParticipantViewMismatch
     /\ BugAcceptParticipantViewMismatch
  \/ /\ candidate = ParticipantProposalHashMismatch
     /\ BugAcceptParticipantProposalHashMismatch
  \/ /\ candidate = ParticipantSettlementHashMismatch
     /\ BugAcceptParticipantSettlementHashMismatch
  \/ /\ candidate = ParticipantProposalSlotConflict
     /\ BugAcceptParticipantProposalSlotConflict
  \/ /\ candidate = ParticipantSettlementSlotConflict
     /\ BugAcceptParticipantSettlementSlotConflict
  \/ /\ candidate = PrepareCommitIdentityMismatch
     /\ BugAcceptPrepareCommitIdentityMismatch

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates
  /\ participantApplicationDurable \in BOOLEAN
  /\ participantFrontierHeight \in {
       ParticipantPredecessorHeight,
       ParticipantBlockHeight
     }

Init ==
  /\ tried = {}
  /\ participantApplicationDurable = FALSE
  /\ participantFrontierHeight = ParticipantPredecessorHeight

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}
  /\ UNCHANGED <<participantApplicationDurable, participantFrontierHeight>>

PersistParticipantApplication ==
  /\ ~participantApplicationDurable
  /\ participantApplicationDurable' = TRUE
  /\ UNCHANGED <<tried, participantFrontierHeight>>

AdvanceParticipantFrontier ==
  /\ participantFrontierHeight = ParticipantPredecessorHeight
  /\ participantApplicationDurable \/ BugAdvanceFrontierBeforeApplication
  /\ participantFrontierHeight' = ParticipantBlockHeight
  /\ UNCHANGED <<tried, participantApplicationDurable>>

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ PersistParticipantApplication
  \/ AdvanceParticipantFrontier
  \/ Stable

ValidationMatchesSpec ==
  \A candidate \in tried:
    ImplementationAccepted(candidate) <=> SpecAccepted(candidate)

ValidReceiptsAreAccepted ==
  /\ ValidNativeReceipt \in tried => ImplementationAccepted(ValidNativeReceipt)
  /\ ValidSingleNoReceipt \in tried => ImplementationAccepted(ValidSingleNoReceipt)
  /\ ValidDistinctParticipantSlots \in tried =>
       ImplementationAccepted(ValidDistinctParticipantSlots)

NativeContextRequiresReceipt ==
  NativeMissingReceipt \in tried => ~ImplementationAccepted(NativeMissingReceipt)

SingleContextForbidsReceipt ==
  SingleUnexpectedReceipt \in tried => ~ImplementationAccepted(SingleUnexpectedReceipt)

SignedTransactionRequired ==
  NativeUnsignedEntrypoint \in tried => ~ImplementationAccepted(NativeUnsignedEntrypoint)

ReceiptHeaderMatchesContext ==
  \A candidate \in tried:
    candidate \in {
      UnsupportedVersion,
      SourceMismatch,
      CoordinatorMismatch,
      HeightMismatch,
      PlanDigestMismatch
    } => ~ImplementationAccepted(candidate)

ParticipantSetMatchesPlan ==
  \A candidate \in tried:
    candidate \in {
      MissingParticipantLeg,
      UnexpectedParticipantLeg,
      DuplicateParticipantLeg
    } => ~ImplementationAccepted(candidate)

ParticipantSlotFieldsMatchPlan ==
  \A candidate \in tried:
    candidate \in {
      ParticipantHeightMismatch,
      ParticipantViewMismatch,
      ParticipantProposalHashMismatch,
      ParticipantSettlementHashMismatch
    } =>
      /\ ReceiptParticipantSlot(candidate) # ParticipantSlot
      /\ ~ImplementationAccepted(candidate)

ParticipantSlotsRemainLaneLocal ==
  ValidDistinctParticipantSlots \in tried =>
    /\ ParticipantSlot # CoordinatorSlot
    /\ ParticipantSlot.height = ParticipantBlockHeight
    /\ ParticipantSlot.view = ParticipantBlockView
    /\ ImplementationAccepted(ValidDistinctParticipantSlots)

ParticipantSlotConflictsFailClosed ==
  \A candidate \in tried:
    candidate \in {
      ParticipantProposalSlotConflict,
      ParticipantSettlementSlotConflict
    } =>
      /\ ReceiptParticipantSlot(candidate).height = ParticipantSlot.height
      /\ ReceiptParticipantSlot(candidate).view = ParticipantSlot.view
      /\ ReceiptParticipantSlot(candidate) # ParticipantSlot
      /\ ~ImplementationAccepted(candidate)

PrepareCommitIdentityMatches ==
  PrepareCommitIdentityMismatch \in tried =>
    /\ CommitParticipantIdentity(PrepareCommitIdentityMismatch) #
         PrepareParticipantIdentity
    /\ ~ImplementationAccepted(PrepareCommitIdentityMismatch)

ParticipantApplicationPrecedesFrontier ==
  participantFrontierHeight = ParticipantBlockHeight =>
    participantApplicationDurable

QcBodyMatchesReceipt ==
  \A candidate \in tried:
    candidate \in {
      QcSourceMismatch,
      QcEntrypointMismatch,
      QcPlanDigestMismatch,
      QcWrongPhase,
      QcCoordinatorMismatch,
      QcParticipantMismatch,
      QcHeightMismatch
    } => ~ImplementationAccepted(candidate)

QcValidatorSetAuthenticated ==
  \A candidate \in tried:
    candidate \in {
      ValidatorHashVersionMismatch,
      ValidatorSetHashMismatch,
      UnknownParticipantDataspace,
      ValidatorSetTooSmall
    } => ~ImplementationAccepted(candidate)

QcSignerBitmapWellFormed ==
  \A candidate \in tried:
    candidate \in {
      SignerBitmapLengthMismatch,
      SignerIndexOutOfBounds,
      SignerNotBls,
      SignerMissingPop
    } => ~ImplementationAccepted(candidate)

QcQuorumAndSignatureRequired ==
  \A candidate \in tried:
    candidate \in {
      QuorumNotMet,
      MissingAggregateSignature,
      InvalidAggregateSignature
    } => ~ImplementationAccepted(candidate)

ReceiptPositiveCases == {
  ValidNativeReceipt,
  ValidSingleNoReceipt,
  ValidDistinctParticipantSlots
}

ReceiptContextCases == {
  NativeMissingReceipt,
  NativeUnsignedEntrypoint,
  SingleUnexpectedReceipt
}

ReceiptHeaderCases == {
  UnsupportedVersion,
  SourceMismatch,
  CoordinatorMismatch,
  HeightMismatch,
  PlanDigestMismatch
}

ReceiptParticipantCases == {
  MissingParticipantLeg,
  UnexpectedParticipantLeg,
  DuplicateParticipantLeg
}

ReceiptParticipantSlotCases == {
  ParticipantHeightMismatch,
  ParticipantViewMismatch,
  ParticipantProposalHashMismatch,
  ParticipantSettlementHashMismatch
}

ReceiptParticipantConflictCases == {
  ParticipantProposalSlotConflict,
  ParticipantSettlementSlotConflict
}

ReceiptParticipantPhaseCases == {
  PrepareCommitIdentityMismatch
}

ReceiptQcBodyCases == {
  QcSourceMismatch,
  QcEntrypointMismatch,
  QcPlanDigestMismatch,
  QcWrongPhase,
  QcCoordinatorMismatch,
  QcParticipantMismatch,
  QcHeightMismatch
}

ReceiptValidatorSetCases == {
  ValidatorHashVersionMismatch,
  ValidatorSetHashMismatch,
  UnknownParticipantDataspace,
  ValidatorSetTooSmall
}

ReceiptSignerBitmapCases == {
  SignerBitmapLengthMismatch,
  SignerIndexOutOfBounds,
  SignerNotBls,
  SignerMissingPop
}

ReceiptQuorumSignatureCases == {
  QuorumNotMet,
  MissingAggregateSignature,
  InvalidAggregateSignature
}

NativeAmxReceiptGroupedCases ==
  ReceiptPositiveCases \cup
  ReceiptContextCases \cup
  ReceiptHeaderCases \cup
  ReceiptParticipantCases \cup
  ReceiptParticipantSlotCases \cup
  ReceiptParticipantConflictCases \cup
  ReceiptParticipantPhaseCases \cup
  ReceiptQcBodyCases \cup
  ReceiptValidatorSetCases \cup
  ReceiptSignerBitmapCases \cup
  ReceiptQuorumSignatureCases

NativeAmxReceiptCaseGroupsComplete ==
  NativeAmxReceiptGroupedCases = Candidates

NativeAmxReceiptAcceptanceExact ==
  /\ ValidationMatchesSpec
  /\ ValidReceiptsAreAccepted

NativeAmxReceiptContextExact ==
  /\ NativeContextRequiresReceipt
  /\ SingleContextForbidsReceipt
  /\ SignedTransactionRequired

NativeAmxReceiptHeaderExact ==
  /\ ReceiptHeaderMatchesContext

NativeAmxReceiptParticipantExact ==
  /\ ParticipantSetMatchesPlan
  /\ ParticipantSlotFieldsMatchPlan
  /\ ParticipantSlotsRemainLaneLocal
  /\ ParticipantSlotConflictsFailClosed
  /\ PrepareCommitIdentityMatches

NativeAmxParticipantApplicationExact ==
  /\ ParticipantApplicationPrecedesFrontier

NativeAmxReceiptQcBodyExact ==
  /\ QcBodyMatchesReceipt

NativeAmxReceiptValidatorSetExact ==
  /\ QcValidatorSetAuthenticated

NativeAmxReceiptSignerBitmapExact ==
  /\ QcSignerBitmapWellFormed

NativeAmxReceiptQuorumSignatureExact ==
  /\ QcQuorumAndSignatureRequired

NativeAmxReceiptValidationExactness ==
  /\ NativeAmxReceiptCaseGroupsComplete
  /\ NativeAmxReceiptAcceptanceExact
  /\ NativeAmxReceiptContextExact
  /\ ReceiptHeaderMatchesContext
  /\ ParticipantSetMatchesPlan
  /\ ParticipantSlotFieldsMatchPlan
  /\ ParticipantSlotsRemainLaneLocal
  /\ ParticipantSlotConflictsFailClosed
  /\ PrepareCommitIdentityMatches
  /\ ParticipantApplicationPrecedesFrontier
  /\ QcBodyMatchesReceipt
  /\ QcValidatorSetAuthenticated
  /\ QcSignerBitmapWellFormed
  /\ QcQuorumAndSignatureRequired

NativeAmxReceiptValidationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NativeAmxReceiptValidationExactness

Safety ==
  NativeAmxReceiptValidationExactness

====

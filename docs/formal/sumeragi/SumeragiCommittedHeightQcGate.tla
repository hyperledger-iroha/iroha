---- MODULE SumeragiCommittedHeightQcGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for committed-height QC admission.

This slice models `Actor::qc_for_committed_height(...)` and the immediate
record-only side effects in QC handling. Future-height QCs continue through
normal processing. QCs whose subject matches an already committed block are
recorded in the cache only, with commit-specific roster/checkpoint recording
and NewView-specific replay work preserved. Unknown already committed heights
drop as stale, while divergent committed-height hashes drop as finality
conflicts. Divergent commit QCs are first validated against the incoming hash,
height, view, epoch, mode tag, chain, genesis-stub policy, and stake snapshot;
valid divergent commit QCs still drop and emit finality-conflict evidence.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FutureQc == "future_qc"
MatchingCommit == "matching_commit"
MatchingPrepare == "matching_prepare"
MatchingNewView == "matching_new_view"
UnknownCommittedBlock == "unknown_committed_block"
DivergentPrepare == "divergent_prepare"
DivergentNewView == "divergent_new_view"
DivergentCommitInvalid == "divergent_commit_invalid"
DivergentCommitValid == "divergent_commit_valid"
DivergentCommitGenesisValid == "divergent_commit_genesis_valid"
DivergentCommitNonGenesis == "divergent_commit_non_genesis"

Cases == {
  FutureQc,
  MatchingCommit,
  MatchingPrepare,
  MatchingNewView,
  UnknownCommittedBlock,
  DivergentPrepare,
  DivergentNewView,
  DivergentCommitInvalid,
  DivergentCommitValid,
  DivergentCommitGenesisValid,
  DivergentCommitNonGenesis
}

Continue == "continue"
RecordOnly == "record_only"
Drop == "drop"
Decisions == {Continue, RecordOnly, Drop}

NoneReason == "none"
CommittedReason == "committed"
CommitConflictReason == "commit_conflict"
WrongReason == "wrong_reason"
Reasons == {
  NoneReason,
  CommittedReason,
  CommitConflictReason,
  WrongReason
}

NoContext == "none"
IncomingHash == "incoming_hash"
CommittedHash == "committed_hash"
EpochForQcHeight == "epoch_for_qc_height"
CurrentEpoch == "current_epoch"
CallerModeTag == "caller_mode_tag"
DefaultModeTag == "default_mode_tag"
CallerChain == "caller_chain"
DefaultChain == "default_chain"
StakeSnapshotPreserved == "stake_snapshot_preserved"
StakeSnapshotDropped == "stake_snapshot_dropped"

ValidationSubjects == {NoContext, IncomingHash, CommittedHash}
ValidationEpochs == {NoContext, EpochForQcHeight, CurrentEpoch}
ValidationModes == {NoContext, CallerModeTag, DefaultModeTag}
ValidationChains == {NoContext, CallerChain, DefaultChain}
ValidationStakes == {
  NoContext,
  StakeSnapshotPreserved,
  StakeSnapshotDropped
}

NoEvidence == "none"
InvalidQcEvidence == "invalid_qc"
WrongEvidence == "wrong_evidence"
EvidenceKinds == {NoEvidence, InvalidQcEvidence, WrongEvidence}
FinalityReason == "commit_conflict_finality"
EvidenceReasons == {NoneReason, FinalityReason, WrongReason}

CommittedMatches(c) ==
  c \in {MatchingCommit, MatchingPrepare, MatchingNewView}

DivergentNonCommit(c) ==
  c \in {DivergentPrepare, DivergentNewView}

DivergentCommit(c) ==
  c \in {
    DivergentCommitInvalid,
    DivergentCommitValid,
    DivergentCommitGenesisValid,
    DivergentCommitNonGenesis
  }

DivergentValidCommit(c) ==
  c \in {
    DivergentCommitValid,
    DivergentCommitGenesisValid,
    DivergentCommitNonGenesis
  }

SpecDecision(c) ==
  CASE c = FutureQc -> Continue
    [] CommittedMatches(c) -> RecordOnly
    [] OTHER -> Drop

SpecDropReason(c) ==
  CASE c = UnknownCommittedBlock -> CommittedReason
    [] DivergentNonCommit(c) \/ DivergentCommit(c) -> CommitConflictReason
    [] OTHER -> NoneReason

SpecValidateCommit(c) ==
  DivergentCommit(c)

SpecValidationSubject(c) ==
  IF SpecValidateCommit(c) THEN IncomingHash ELSE NoContext

SpecValidationEpoch(c) ==
  IF SpecValidateCommit(c) THEN EpochForQcHeight ELSE NoContext

SpecValidationModeTag(c) ==
  IF SpecValidateCommit(c) THEN CallerModeTag ELSE NoContext

SpecValidationChain(c) ==
  IF SpecValidateCommit(c) THEN CallerChain ELSE NoContext

SpecValidationStake(c) ==
  IF SpecValidateCommit(c) THEN StakeSnapshotPreserved ELSE NoContext

SpecAllowGenesisStub(c) ==
  c = DivergentCommitGenesisValid

SpecValidationAccepts(c) ==
  DivergentValidCommit(c)

SpecEvidenceEmitted(c) ==
  DivergentValidCommit(c)

SpecEvidenceKind(c) ==
  IF SpecEvidenceEmitted(c) THEN InvalidQcEvidence ELSE NoEvidence

SpecEvidenceReason(c) ==
  IF SpecEvidenceEmitted(c) THEN FinalityReason ELSE NoneReason

SpecCacheInserted(c) ==
  SpecDecision(c) = RecordOnly

SpecCommitRosterRecorded(c) ==
  c = MatchingCommit

SpecMissingCommitRequestCleared(c) ==
  c = MatchingCommit

SpecNewViewReplay(c) ==
  c = MatchingNewView

ActualDecision(c) ==
  CASE Bug = "future_dropped"
       /\ c = FutureQc -> Drop
    [] Bug = "future_record_only"
       /\ c = FutureQc -> RecordOnly
    [] Bug = "matching_hash_dropped"
       /\ c = MatchingPrepare -> Drop
    [] Bug = "matching_hash_continues"
       /\ c = MatchingPrepare -> Continue
    [] Bug = "unknown_block_record_only"
       /\ c = UnknownCommittedBlock -> RecordOnly
    [] Bug = "divergent_prepare_record_only"
       /\ c = DivergentPrepare -> RecordOnly
    [] Bug = "divergent_commit_invalid_record_only"
       /\ c = DivergentCommitInvalid -> RecordOnly
    [] Bug = "divergent_commit_valid_record_only"
       /\ c = DivergentCommitValid -> RecordOnly
    [] OTHER -> SpecDecision(c)

ActualDropReason(c) ==
  CASE Bug = "unknown_block_wrong_reason"
       /\ c = UnknownCommittedBlock -> CommitConflictReason
    [] Bug = "divergent_prepare_wrong_reason"
       /\ c = DivergentPrepare -> CommittedReason
    [] OTHER -> SpecDropReason(c)

ActualValidateCommit(c) ==
  CASE Bug = "divergent_commit_valid_skips_validation"
       /\ c = DivergentCommitValid -> FALSE
    [] OTHER -> SpecValidateCommit(c)

ActualValidationSubject(c) ==
  CASE Bug = "validation_uses_wrong_subject"
       /\ c = DivergentCommitValid -> CommittedHash
    [] OTHER -> SpecValidationSubject(c)

ActualValidationEpoch(c) ==
  CASE Bug = "validation_uses_wrong_epoch"
       /\ c = DivergentCommitValid -> CurrentEpoch
    [] OTHER -> SpecValidationEpoch(c)

ActualValidationModeTag(c) ==
  CASE Bug = "validation_uses_wrong_mode"
       /\ c = DivergentCommitValid -> DefaultModeTag
    [] OTHER -> SpecValidationModeTag(c)

ActualValidationChain(c) ==
  CASE Bug = "validation_uses_wrong_chain"
       /\ c = DivergentCommitValid -> DefaultChain
    [] OTHER -> SpecValidationChain(c)

ActualValidationStake(c) ==
  CASE Bug = "validation_drops_stake"
       /\ c = DivergentCommitValid -> StakeSnapshotDropped
    [] OTHER -> SpecValidationStake(c)

ActualAllowGenesisStub(c) ==
  CASE Bug = "genesis_stub_not_allowed"
       /\ c = DivergentCommitGenesisValid -> FALSE
    [] Bug = "non_genesis_stub_allowed"
       /\ c = DivergentCommitNonGenesis -> TRUE
    [] OTHER -> SpecAllowGenesisStub(c)

ActualValidationAccepts(c) ==
  SpecValidationAccepts(c)

ActualEvidenceEmitted(c) ==
  CASE Bug = "divergent_commit_invalid_emits_evidence"
       /\ c = DivergentCommitInvalid -> TRUE
    [] Bug = "divergent_commit_valid_no_evidence"
       /\ c = DivergentCommitValid -> FALSE
    [] OTHER -> SpecEvidenceEmitted(c)

ActualEvidenceKind(c) ==
  CASE Bug = "divergent_commit_valid_wrong_evidence_kind"
       /\ c = DivergentCommitValid -> WrongEvidence
    [] OTHER -> SpecEvidenceKind(c)

ActualEvidenceReason(c) ==
  CASE Bug = "divergent_commit_valid_wrong_evidence_reason"
       /\ c = DivergentCommitValid -> WrongReason
    [] OTHER -> SpecEvidenceReason(c)

ActualCacheInserted(c) ==
  CASE Bug = "matching_hash_dropped"
       /\ c = MatchingPrepare -> FALSE
    [] OTHER -> SpecCacheInserted(c)

ActualCommitRosterRecorded(c) ==
  CASE Bug = "matching_commit_skips_roster_record"
       /\ c = MatchingCommit -> FALSE
    [] Bug = "matching_prepare_records_commit_roster"
       /\ c = MatchingPrepare -> TRUE
    [] OTHER -> SpecCommitRosterRecorded(c)

ActualMissingCommitRequestCleared(c) ==
  CASE Bug = "matching_commit_skips_missing_request_clear"
       /\ c = MatchingCommit -> FALSE
    [] OTHER -> SpecMissingCommitRequestCleared(c)

ActualNewViewReplay(c) ==
  CASE Bug = "matching_new_view_skips_replay"
       /\ c = MatchingNewView -> FALSE
    [] OTHER -> SpecNewViewReplay(c)

Bugs == {
  "none",
  "future_dropped",
  "future_record_only",
  "matching_hash_dropped",
  "matching_hash_continues",
  "matching_commit_skips_roster_record",
  "matching_commit_skips_missing_request_clear",
  "matching_new_view_skips_replay",
  "matching_prepare_records_commit_roster",
  "unknown_block_record_only",
  "unknown_block_wrong_reason",
  "divergent_prepare_record_only",
  "divergent_prepare_wrong_reason",
  "divergent_commit_invalid_record_only",
  "divergent_commit_invalid_emits_evidence",
  "divergent_commit_valid_record_only",
  "divergent_commit_valid_no_evidence",
  "divergent_commit_valid_wrong_evidence_reason",
  "divergent_commit_valid_wrong_evidence_kind",
  "divergent_commit_valid_skips_validation",
  "validation_uses_wrong_subject",
  "validation_uses_wrong_epoch",
  "validation_uses_wrong_mode",
  "validation_uses_wrong_chain",
  "validation_drops_stake",
  "genesis_stub_not_allowed",
  "non_genesis_stub_allowed"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecDecision(c) \in Decisions
       /\ ActualDecision(c) \in Decisions
       /\ SpecDropReason(c) \in Reasons
       /\ ActualDropReason(c) \in Reasons
       /\ SpecValidateCommit(c) \in BOOLEAN
       /\ ActualValidateCommit(c) \in BOOLEAN
       /\ SpecValidationSubject(c) \in ValidationSubjects
       /\ ActualValidationSubject(c) \in ValidationSubjects
       /\ SpecValidationEpoch(c) \in ValidationEpochs
       /\ ActualValidationEpoch(c) \in ValidationEpochs
       /\ SpecValidationModeTag(c) \in ValidationModes
       /\ ActualValidationModeTag(c) \in ValidationModes
       /\ SpecValidationChain(c) \in ValidationChains
       /\ ActualValidationChain(c) \in ValidationChains
       /\ SpecValidationStake(c) \in ValidationStakes
       /\ ActualValidationStake(c) \in ValidationStakes
       /\ SpecAllowGenesisStub(c) \in BOOLEAN
       /\ ActualAllowGenesisStub(c) \in BOOLEAN
       /\ SpecValidationAccepts(c) \in BOOLEAN
       /\ ActualValidationAccepts(c) \in BOOLEAN
       /\ SpecEvidenceEmitted(c) \in BOOLEAN
       /\ ActualEvidenceEmitted(c) \in BOOLEAN
       /\ SpecEvidenceKind(c) \in EvidenceKinds
       /\ ActualEvidenceKind(c) \in EvidenceKinds
       /\ SpecEvidenceReason(c) \in EvidenceReasons
       /\ ActualEvidenceReason(c) \in EvidenceReasons
       /\ SpecCacheInserted(c) \in BOOLEAN
       /\ ActualCacheInserted(c) \in BOOLEAN
       /\ SpecCommitRosterRecorded(c) \in BOOLEAN
       /\ ActualCommitRosterRecorded(c) \in BOOLEAN
       /\ SpecMissingCommitRequestCleared(c) \in BOOLEAN
       /\ ActualMissingCommitRequestCleared(c) \in BOOLEAN
       /\ SpecNewViewReplay(c) \in BOOLEAN
       /\ ActualNewViewReplay(c) \in BOOLEAN

DecisionMatchesSpec ==
  \A c \in Cases:
    ActualDecision(c) = SpecDecision(c)

DropReasonMatchesSpec ==
  \A c \in Cases:
    ActualDropReason(c) = SpecDropReason(c)

ValidationMatchesSpec ==
  \A c \in Cases:
    /\ ActualValidateCommit(c) = SpecValidateCommit(c)
    /\ ActualValidationAccepts(c) = SpecValidationAccepts(c)

ValidationContextMatchesSpec ==
  \A c \in Cases:
    /\ ActualValidationSubject(c) = SpecValidationSubject(c)
    /\ ActualValidationEpoch(c) = SpecValidationEpoch(c)
    /\ ActualValidationModeTag(c) = SpecValidationModeTag(c)
    /\ ActualValidationChain(c) = SpecValidationChain(c)
    /\ ActualValidationStake(c) = SpecValidationStake(c)
    /\ ActualAllowGenesisStub(c) = SpecAllowGenesisStub(c)

EvidenceMatchesSpec ==
  \A c \in Cases:
    /\ ActualEvidenceEmitted(c) = SpecEvidenceEmitted(c)
    /\ ActualEvidenceKind(c) = SpecEvidenceKind(c)
    /\ ActualEvidenceReason(c) = SpecEvidenceReason(c)

RecordOnlySideEffectsMatch ==
  \A c \in Cases:
    /\ ActualCacheInserted(c) = SpecCacheInserted(c)
    /\ ActualCommitRosterRecorded(c) = SpecCommitRosterRecorded(c)
    /\ ActualMissingCommitRequestCleared(c) =
         SpecMissingCommitRequestCleared(c)
    /\ ActualNewViewReplay(c) = SpecNewViewReplay(c)

NoBugInvariant ==
  /\ DecisionMatchesSpec
  /\ DropReasonMatchesSpec
  /\ ValidationMatchesSpec
  /\ ValidationContextMatchesSpec
  /\ EvidenceMatchesSpec
  /\ RecordOnlySideEffectsMatch

SafetyFast == NoBugInvariant

BugFutureDropped == NoBugInvariant
BugFutureRecordOnly == NoBugInvariant
BugMatchingHashDropped == NoBugInvariant
BugMatchingHashContinues == NoBugInvariant
BugMatchingCommitSkipsRosterRecord == NoBugInvariant
BugMatchingCommitSkipsMissingRequestClear == NoBugInvariant
BugMatchingNewViewSkipsReplay == NoBugInvariant
BugMatchingPrepareRecordsCommitRoster == NoBugInvariant
BugUnknownBlockRecordOnly == NoBugInvariant
BugUnknownBlockWrongReason == NoBugInvariant
BugDivergentPrepareRecordOnly == NoBugInvariant
BugDivergentPrepareWrongReason == NoBugInvariant
BugDivergentCommitInvalidRecordOnly == NoBugInvariant
BugDivergentCommitInvalidEmitsEvidence == NoBugInvariant
BugDivergentCommitValidRecordOnly == NoBugInvariant
BugDivergentCommitValidNoEvidence == NoBugInvariant
BugDivergentCommitValidWrongEvidenceReason == NoBugInvariant
BugDivergentCommitValidWrongEvidenceKind == NoBugInvariant
BugDivergentCommitValidSkipsValidation == NoBugInvariant
BugValidationUsesWrongSubject == NoBugInvariant
BugValidationUsesWrongEpoch == NoBugInvariant
BugValidationUsesWrongMode == NoBugInvariant
BugValidationUsesWrongChain == NoBugInvariant
BugValidationDropsStake == NoBugInvariant
BugGenesisStubNotAllowed == NoBugInvariant
BugNonGenesisStubAllowed == NoBugInvariant

====

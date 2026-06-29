---- MODULE SumeragiBlockSyncQcStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi block-sync QC status accounting.

This slice captures `inc_block_sync_drop_invalid_signatures()`,
`inc_block_sync_qc_replaced()`, `inc_block_sync_qc_derive_failed()`,
`inc_block_sync_locked_qc_prefilter_drop()`,
`inc_blocksync_qc_quarantine()`, `inc_blocksync_qc_revalidated()`,
`inc_blocksync_qc_final_drop(...)`, their `snapshot()` projection, and the
block-sync QC subset of the test-only `reset_block_sync_counters_for_tests()`
helper from `status.rs`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetEmpty == 1
InvalidSignaturesDrop == 2
QcReplaced == 3
QcDeriveFailed == 4
LockedPrefilterDrop == 5
QcQuarantine == 6
QcRevalidated == 7
QcFinalDrop == 8
FinalDropReasonOverwrites == 9
RepeatedQcReplacedAccumulates == 10
RepeatedFinalDropAccumulates == 11
SnapshotProjectsInvalidSignatures == 12
SnapshotProjectsQcReplaced == 13
SnapshotProjectsQcDeriveFailed == 14
SnapshotProjectsLockedPrefilter == 15
SnapshotProjectsQuarantine == 16
SnapshotProjectsRevalidated == 17
SnapshotProjectsFinalDrop == 18
SnapshotProjectsFinalReason == 19
ResetAfterRecordsClears == 20

Candidates == 1..20

ResetCounters == 1
ResetFinalReason == 2
IncrementInvalidSignatures == 3
IncrementQcReplaced == 4
IncrementQcDeriveFailed == 5
IncrementLockedPrefilter == 6
IncrementQcQuarantine == 7
IncrementQcRevalidated == 8
IncrementQcFinalDrop == 9
SetFinalDropReason == 10
LastReasonOverwrites == 11
SameCounterAccumulates == 12
SnapshotInvalidSignaturesMatches == 13
SnapshotQcReplacedMatches == 14
SnapshotQcDeriveFailedMatches == 15
SnapshotLockedPrefilterMatches == 16
SnapshotQuarantineMatches == 17
SnapshotRevalidatedMatches == 18
SnapshotFinalDropMatches == 19
SnapshotFinalReasonMatches == 20

Actions == 1..20

AllResetActions == {ResetCounters, ResetFinalReason}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = InvalidSignaturesDrop ->
      {IncrementInvalidSignatures}
    [] candidate = QcReplaced ->
      {IncrementQcReplaced}
    [] candidate = QcDeriveFailed ->
      {IncrementQcDeriveFailed}
    [] candidate = LockedPrefilterDrop ->
      {IncrementLockedPrefilter}
    [] candidate = QcQuarantine ->
      {IncrementQcQuarantine}
    [] candidate = QcRevalidated ->
      {IncrementQcRevalidated}
    [] candidate = QcFinalDrop ->
      {IncrementQcFinalDrop, SetFinalDropReason}
    [] candidate = FinalDropReasonOverwrites ->
      {SetFinalDropReason, LastReasonOverwrites,
       SnapshotFinalReasonMatches}
    [] candidate = RepeatedQcReplacedAccumulates ->
      {IncrementQcReplaced, SameCounterAccumulates,
       SnapshotQcReplacedMatches}
    [] candidate = RepeatedFinalDropAccumulates ->
      {IncrementQcFinalDrop, SameCounterAccumulates,
       SnapshotFinalDropMatches}
    [] candidate = SnapshotProjectsInvalidSignatures ->
      {SnapshotInvalidSignaturesMatches}
    [] candidate = SnapshotProjectsQcReplaced ->
      {SnapshotQcReplacedMatches}
    [] candidate = SnapshotProjectsQcDeriveFailed ->
      {SnapshotQcDeriveFailedMatches}
    [] candidate = SnapshotProjectsLockedPrefilter ->
      {SnapshotLockedPrefilterMatches}
    [] candidate = SnapshotProjectsQuarantine ->
      {SnapshotQuarantineMatches}
    [] candidate = SnapshotProjectsRevalidated ->
      {SnapshotRevalidatedMatches}
    [] candidate = SnapshotProjectsFinalDrop ->
      {SnapshotFinalDropMatches}
    [] candidate = SnapshotProjectsFinalReason ->
      {SnapshotFinalReasonMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_reason" ->
      spec \ {ResetFinalReason}
    [] candidate = InvalidSignaturesDrop /\
          Bug = "invalid_signatures_not_counted" ->
      spec \ {IncrementInvalidSignatures}
    [] candidate = QcReplaced /\ Bug = "qc_replaced_not_counted" ->
      spec \ {IncrementQcReplaced}
    [] candidate = QcReplaced /\
          Bug = "qc_replaced_counts_derive_failed" ->
      (spec \ {IncrementQcReplaced}) \cup {IncrementQcDeriveFailed}
    [] candidate = QcDeriveFailed /\
          Bug = "qc_derive_failed_not_counted" ->
      spec \ {IncrementQcDeriveFailed}
    [] candidate = LockedPrefilterDrop /\
          Bug = "locked_prefilter_not_counted" ->
      spec \ {IncrementLockedPrefilter}
    [] candidate = QcQuarantine /\
          Bug = "qc_quarantine_not_counted" ->
      spec \ {IncrementQcQuarantine}
    [] candidate = QcRevalidated /\
          Bug = "qc_revalidated_not_counted" ->
      spec \ {IncrementQcRevalidated}
    [] candidate = QcFinalDrop /\
          Bug = "final_drop_not_counted" ->
      spec \ {IncrementQcFinalDrop}
    [] candidate = QcFinalDrop /\
          Bug = "final_drop_reason_not_recorded" ->
      spec \ {SetFinalDropReason}
    [] candidate = FinalDropReasonOverwrites /\
          Bug = "final_drop_reason_not_overwritten" ->
      spec \ {SetFinalDropReason, LastReasonOverwrites,
              SnapshotFinalReasonMatches}
    [] candidate = RepeatedQcReplacedAccumulates /\
          Bug = "repeated_qc_replaced_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotQcReplacedMatches}
    [] candidate = RepeatedFinalDropAccumulates /\
          Bug = "repeated_final_drop_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotFinalDropMatches}
    [] candidate = SnapshotProjectsInvalidSignatures /\
          Bug = "snapshot_invalid_signatures_mismatch" ->
      spec \ {SnapshotInvalidSignaturesMatches}
    [] candidate = SnapshotProjectsQcReplaced /\
          Bug = "snapshot_qc_replaced_mismatch" ->
      spec \ {SnapshotQcReplacedMatches}
    [] candidate = SnapshotProjectsQcDeriveFailed /\
          Bug = "snapshot_qc_derive_failed_mismatch" ->
      spec \ {SnapshotQcDeriveFailedMatches}
    [] candidate = SnapshotProjectsLockedPrefilter /\
          Bug = "snapshot_locked_prefilter_mismatch" ->
      spec \ {SnapshotLockedPrefilterMatches}
    [] candidate = SnapshotProjectsQuarantine /\
          Bug = "snapshot_quarantine_mismatch" ->
      spec \ {SnapshotQuarantineMatches}
    [] candidate = SnapshotProjectsRevalidated /\
          Bug = "snapshot_revalidated_mismatch" ->
      spec \ {SnapshotRevalidatedMatches}
    [] candidate = SnapshotProjectsFinalDrop /\
          Bug = "snapshot_final_drop_mismatch" ->
      spec \ {SnapshotFinalDropMatches}
    [] candidate = SnapshotProjectsFinalReason /\
          Bug = "snapshot_final_reason_mismatch" ->
      spec \ {SnapshotFinalReasonMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_reason" ->
      spec \ {ResetFinalReason}
    [] OTHER -> spec

Init ==
  checked = 0

Advance ==
  /\ checked < 20
  /\ checked' = checked + 1

Stable ==
  /\ checked = 20
  /\ checked' = checked

Next ==
  Advance \/ Stable

TypeInvariant ==
  checked \in 0..20

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BlockSyncQcStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BlockSyncQcStatusExactness ==
  /\ BlockSyncQcStatusActionsMatchSpec

BlockSyncQcStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncQcStatusExactness

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsReason ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugInvalidSignaturesNotCounted ==
  ImplementationActions(InvalidSignaturesDrop) =
    SpecActions(InvalidSignaturesDrop)

BugQcReplacedNotCounted ==
  ImplementationActions(QcReplaced) = SpecActions(QcReplaced)

BugQcReplacedCountsDeriveFailed ==
  ImplementationActions(QcReplaced) = SpecActions(QcReplaced)

BugQcDeriveFailedNotCounted ==
  ImplementationActions(QcDeriveFailed) = SpecActions(QcDeriveFailed)

BugLockedPrefilterNotCounted ==
  ImplementationActions(LockedPrefilterDrop) =
    SpecActions(LockedPrefilterDrop)

BugQcQuarantineNotCounted ==
  ImplementationActions(QcQuarantine) = SpecActions(QcQuarantine)

BugQcRevalidatedNotCounted ==
  ImplementationActions(QcRevalidated) = SpecActions(QcRevalidated)

BugFinalDropNotCounted ==
  ImplementationActions(QcFinalDrop) = SpecActions(QcFinalDrop)

BugFinalDropReasonNotRecorded ==
  ImplementationActions(QcFinalDrop) = SpecActions(QcFinalDrop)

BugFinalDropReasonNotOverwritten ==
  ImplementationActions(FinalDropReasonOverwrites) =
    SpecActions(FinalDropReasonOverwrites)

BugRepeatedQcReplacedOverwritesCount ==
  ImplementationActions(RepeatedQcReplacedAccumulates) =
    SpecActions(RepeatedQcReplacedAccumulates)

BugRepeatedFinalDropOverwritesCount ==
  ImplementationActions(RepeatedFinalDropAccumulates) =
    SpecActions(RepeatedFinalDropAccumulates)

BugSnapshotInvalidSignaturesMismatch ==
  ImplementationActions(SnapshotProjectsInvalidSignatures) =
    SpecActions(SnapshotProjectsInvalidSignatures)

BugSnapshotQcReplacedMismatch ==
  ImplementationActions(SnapshotProjectsQcReplaced) =
    SpecActions(SnapshotProjectsQcReplaced)

BugSnapshotQcDeriveFailedMismatch ==
  ImplementationActions(SnapshotProjectsQcDeriveFailed) =
    SpecActions(SnapshotProjectsQcDeriveFailed)

BugSnapshotLockedPrefilterMismatch ==
  ImplementationActions(SnapshotProjectsLockedPrefilter) =
    SpecActions(SnapshotProjectsLockedPrefilter)

BugSnapshotQuarantineMismatch ==
  ImplementationActions(SnapshotProjectsQuarantine) =
    SpecActions(SnapshotProjectsQuarantine)

BugSnapshotRevalidatedMismatch ==
  ImplementationActions(SnapshotProjectsRevalidated) =
    SpecActions(SnapshotProjectsRevalidated)

BugSnapshotFinalDropMismatch ==
  ImplementationActions(SnapshotProjectsFinalDrop) =
    SpecActions(SnapshotProjectsFinalDrop)

BugSnapshotFinalReasonMismatch ==
  ImplementationActions(SnapshotProjectsFinalReason) =
    SpecActions(SnapshotProjectsFinalReason)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsReason ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====

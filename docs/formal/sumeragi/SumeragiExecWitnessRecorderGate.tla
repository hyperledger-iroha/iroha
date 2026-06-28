---- MODULE SumeragiExecWitnessRecorderGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the execution-witness recorder lifecycle.

This slice captures `sumeragi::witness`: `start_block`,
`drain_exec_witness`, `snapshot_exec_witness`, `ExecWitnessGuard::drop`,
recording read/write/delete witnesses, key encoding, and FASTPQ transcript
digest copying. The recorder is global and mutex-protected in Rust; this model
focuses on the deterministic single-recorder contract that commit roots rely on:

- block start activates a fresh recorder and clears stale state,
- records are ignored while inactive,
- reads keep the first pre-value for a key while writes keep the latest
  post-value,
- deletes retain any earlier read pre-value and record an empty post-value,
- drain returns BTreeMap-ordered witnesses, finalizes FASTPQ digests, emits no
  legacy FASTPQ batches, clears state, and deactivates capture,
- snapshots clone/finalize without clearing,
- guard drop clears unfinished capture,
- witness key tags and separators keep namespaces distinct, and
- FASTPQ digest copying only fills matching existing transcript slots.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

StartClearsState == 1
InactiveIgnoresRecords == 2
ReadFirstWins == 3
WriteLastWins == 4
DeleteRecordsPreAndEmpty == 5
DrainReturnsSorted == 6
DrainClearsAndDeactivates == 7
SnapshotDoesNotClear == 8
GuardDropClears == 9
MetadataKeyTags == 10
MetadataKeySeparator == 11
BalanceKeyTag == 12
AssetTotalKeyTag == 13
FastpqRecordActive == 14
FastpqRecordInactiveIgnored == 15
FastpqDrainFinalizes == 16
FastpqSnapshotFinalizesNoClear == 17
ApplyDigestExistingOnly == 18
ApplyDigestMatchesTranscript == 19
ApplyDigestDoesNotOverwrite == 20
ApplyDigestZipAligned == 21
StartAfterDrainFresh == 22

Cases == 1..22

StartSetsActive == 1
StartClearsReads == 2
StartClearsWrites == 3
StartClearsFastpq == 4
InactiveDropsRead == 5
InactiveDropsWrite == 6
InactiveDropsFastpq == 7
ReadEntryOrInsert == 8
ReadFirstValuePreserved == 9
WriteInsert == 10
WriteLastValuePreserved == 11
DeletePreservesExistingRead == 12
DeleteWritesEmptyPostValue == 13
DrainReadsSortedByKey == 14
DrainWritesSortedByKey == 15
DrainClearsReads == 16
DrainClearsWrites == 17
DrainClearsFastpq == 18
DrainDeactivates == 19
DrainFastpqBatchesEmpty == 20
SnapshotClonesRecords == 21
SnapshotKeepsState == 22
SnapshotKeepsActive == 23
SnapshotFinalizesFastpq == 24
GuardDropClearsAll == 25
MetadataTagsDistinct == 26
MetadataUsesUnitSeparator == 27
BalanceUsesB1Tag == 28
BalanceHasNoSeparator == 29
AssetTotalUsesB2Tag == 30
AssetTotalHasNoSeparator == 31
FastpqGroupedByBatch == 32
FastpqPreservesInsertionOrder == 33
FastpqDrainFinalizesDigest == 34
FastpqInactiveIgnored == 35
ApplyDigestExistingBatchOnly == 36
ApplyDigestRequiresSameTranscript == 37
ApplyDigestFillsOnlyMissing == 38
ApplyDigestKeepsExistingDigest == 39
ApplyDigestUsesZipPosition == 40

Actions == 1..40

SpecActions(candidate) ==
  CASE candidate = StartClearsState ->
      {StartSetsActive, StartClearsReads, StartClearsWrites, StartClearsFastpq}
    [] candidate = InactiveIgnoresRecords ->
      {InactiveDropsRead, InactiveDropsWrite, InactiveDropsFastpq}
    [] candidate = ReadFirstWins ->
      {ReadEntryOrInsert, ReadFirstValuePreserved}
    [] candidate = WriteLastWins ->
      {WriteInsert, WriteLastValuePreserved}
    [] candidate = DeleteRecordsPreAndEmpty ->
      {DeletePreservesExistingRead, DeleteWritesEmptyPostValue}
    [] candidate = DrainReturnsSorted ->
      {DrainReadsSortedByKey, DrainWritesSortedByKey}
    [] candidate = DrainClearsAndDeactivates ->
      {DrainClearsReads, DrainClearsWrites, DrainClearsFastpq,
       DrainDeactivates, DrainFastpqBatchesEmpty}
    [] candidate = SnapshotDoesNotClear ->
      {SnapshotClonesRecords, SnapshotKeepsState, SnapshotKeepsActive}
    [] candidate = GuardDropClears ->
      {GuardDropClearsAll}
    [] candidate = MetadataKeyTags ->
      {MetadataTagsDistinct}
    [] candidate = MetadataKeySeparator ->
      {MetadataUsesUnitSeparator}
    [] candidate = BalanceKeyTag ->
      {BalanceUsesB1Tag, BalanceHasNoSeparator}
    [] candidate = AssetTotalKeyTag ->
      {AssetTotalUsesB2Tag, AssetTotalHasNoSeparator}
    [] candidate = FastpqRecordActive ->
      {FastpqGroupedByBatch, FastpqPreservesInsertionOrder}
    [] candidate = FastpqRecordInactiveIgnored ->
      {FastpqInactiveIgnored}
    [] candidate = FastpqDrainFinalizes ->
      {FastpqDrainFinalizesDigest, DrainFastpqBatchesEmpty}
    [] candidate = FastpqSnapshotFinalizesNoClear ->
      {SnapshotFinalizesFastpq, SnapshotKeepsState}
    [] candidate = ApplyDigestExistingOnly ->
      {ApplyDigestExistingBatchOnly}
    [] candidate = ApplyDigestMatchesTranscript ->
      {ApplyDigestRequiresSameTranscript, ApplyDigestFillsOnlyMissing}
    [] candidate = ApplyDigestDoesNotOverwrite ->
      {ApplyDigestKeepsExistingDigest}
    [] candidate = ApplyDigestZipAligned ->
      {ApplyDigestUsesZipPosition}
    [] candidate = StartAfterDrainFresh ->
      {StartSetsActive, StartClearsReads, StartClearsWrites, StartClearsFastpq}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = StartClearsState /\ Bug = "start_not_active" ->
      spec \ {StartSetsActive}
    [] candidate \in {StartClearsState, StartAfterDrainFresh} /\
          Bug = "start_keeps_reads" ->
      spec \ {StartClearsReads}
    [] candidate \in {StartClearsState, StartAfterDrainFresh} /\
          Bug = "start_keeps_writes" ->
      spec \ {StartClearsWrites}
    [] candidate \in {StartClearsState, StartAfterDrainFresh} /\
          Bug = "start_keeps_fastpq" ->
      spec \ {StartClearsFastpq}
    [] candidate = InactiveIgnoresRecords /\ Bug = "inactive_records_read" ->
      spec \ {InactiveDropsRead}
    [] candidate = InactiveIgnoresRecords /\ Bug = "inactive_records_write" ->
      spec \ {InactiveDropsWrite}
    [] candidate \in {InactiveIgnoresRecords, FastpqRecordInactiveIgnored} /\
          Bug = "inactive_records_fastpq" ->
      spec \ {InactiveDropsFastpq, FastpqInactiveIgnored}
    [] candidate = ReadFirstWins /\ Bug = "read_overwrites_existing" ->
      spec \ {ReadFirstValuePreserved}
    [] candidate = ReadFirstWins /\ Bug = "read_skips_entry_insert" ->
      spec \ {ReadEntryOrInsert}
    [] candidate = WriteLastWins /\ Bug = "write_preserves_old_value" ->
      spec \ {WriteLastValuePreserved}
    [] candidate = WriteLastWins /\ Bug = "write_skips_insert" ->
      spec \ {WriteInsert}
    [] candidate = DeleteRecordsPreAndEmpty /\
          Bug = "delete_overwrites_existing_read" ->
      spec \ {DeletePreservesExistingRead}
    [] candidate = DeleteRecordsPreAndEmpty /\ Bug = "delete_writes_nonempty" ->
      spec \ {DeleteWritesEmptyPostValue}
    [] candidate = DrainReturnsSorted /\ Bug = "drain_reads_unsorted" ->
      spec \ {DrainReadsSortedByKey}
    [] candidate = DrainReturnsSorted /\ Bug = "drain_writes_unsorted" ->
      spec \ {DrainWritesSortedByKey}
    [] candidate = DrainClearsAndDeactivates /\ Bug = "drain_keeps_reads" ->
      spec \ {DrainClearsReads}
    [] candidate = DrainClearsAndDeactivates /\ Bug = "drain_keeps_writes" ->
      spec \ {DrainClearsWrites}
    [] candidate = DrainClearsAndDeactivates /\ Bug = "drain_keeps_fastpq" ->
      spec \ {DrainClearsFastpq}
    [] candidate = DrainClearsAndDeactivates /\ Bug = "drain_keeps_active" ->
      spec \ {DrainDeactivates}
    [] candidate \in {DrainClearsAndDeactivates, FastpqDrainFinalizes} /\
          Bug = "drain_emits_fastpq_batches" ->
      spec \ {DrainFastpqBatchesEmpty}
    [] candidate = SnapshotDoesNotClear /\ Bug = "snapshot_drops_records" ->
      spec \ {SnapshotClonesRecords}
    [] candidate \in {SnapshotDoesNotClear, FastpqSnapshotFinalizesNoClear} /\
          Bug = "snapshot_clears_state" ->
      spec \ {SnapshotKeepsState}
    [] candidate = SnapshotDoesNotClear /\ Bug = "snapshot_deactivates" ->
      spec \ {SnapshotKeepsActive}
    [] candidate = FastpqSnapshotFinalizesNoClear /\
          Bug = "snapshot_skips_fastpq_finalize" ->
      spec \ {SnapshotFinalizesFastpq}
    [] candidate = GuardDropClears /\ Bug = "guard_drop_keeps_capture" ->
      spec \ {GuardDropClearsAll}
    [] candidate = MetadataKeyTags /\ Bug = "metadata_tags_collide" ->
      spec \ {MetadataTagsDistinct}
    [] candidate = MetadataKeySeparator /\ Bug = "metadata_missing_separator" ->
      spec \ {MetadataUsesUnitSeparator}
    [] candidate = BalanceKeyTag /\ Bug = "balance_wrong_tag" ->
      spec \ {BalanceUsesB1Tag}
    [] candidate = BalanceKeyTag /\ Bug = "balance_adds_separator" ->
      spec \ {BalanceHasNoSeparator}
    [] candidate = AssetTotalKeyTag /\ Bug = "asset_total_wrong_tag" ->
      spec \ {AssetTotalUsesB2Tag}
    [] candidate = AssetTotalKeyTag /\ Bug = "asset_total_adds_separator" ->
      spec \ {AssetTotalHasNoSeparator}
    [] candidate = FastpqRecordActive /\ Bug = "fastpq_not_grouped" ->
      spec \ {FastpqGroupedByBatch}
    [] candidate = FastpqRecordActive /\ Bug = "fastpq_reorders_transcripts" ->
      spec \ {FastpqPreservesInsertionOrder}
    [] candidate = FastpqDrainFinalizes /\ Bug = "drain_skips_fastpq_finalize" ->
      spec \ {FastpqDrainFinalizesDigest}
    [] candidate = ApplyDigestExistingOnly /\
          Bug = "apply_creates_missing_batch" ->
      spec \ {ApplyDigestExistingBatchOnly}
    [] candidate = ApplyDigestMatchesTranscript /\ Bug = "apply_ignores_shape" ->
      spec \ {ApplyDigestRequiresSameTranscript}
    [] candidate = ApplyDigestMatchesTranscript /\
          Bug = "apply_does_not_fill_missing" ->
      spec \ {ApplyDigestFillsOnlyMissing}
    [] candidate = ApplyDigestDoesNotOverwrite /\
          Bug = "apply_overwrites_existing_digest" ->
      spec \ {ApplyDigestKeepsExistingDigest}
    [] candidate = ApplyDigestZipAligned /\ Bug = "apply_not_zip_aligned" ->
      spec \ {ApplyDigestUsesZipPosition}
    [] OTHER -> spec

Bugs == {
  "none",
  "start_not_active",
  "start_keeps_reads",
  "start_keeps_writes",
  "start_keeps_fastpq",
  "inactive_records_read",
  "inactive_records_write",
  "inactive_records_fastpq",
  "read_overwrites_existing",
  "read_skips_entry_insert",
  "write_preserves_old_value",
  "write_skips_insert",
  "delete_overwrites_existing_read",
  "delete_writes_nonempty",
  "drain_reads_unsorted",
  "drain_writes_unsorted",
  "drain_keeps_reads",
  "drain_keeps_writes",
  "drain_keeps_fastpq",
  "drain_keeps_active",
  "drain_emits_fastpq_batches",
  "snapshot_drops_records",
  "snapshot_clears_state",
  "snapshot_deactivates",
  "snapshot_skips_fastpq_finalize",
  "guard_drop_keeps_capture",
  "metadata_tags_collide",
  "metadata_missing_separator",
  "balance_wrong_tag",
  "balance_adds_separator",
  "asset_total_wrong_tag",
  "asset_total_adds_separator",
  "fastpq_not_grouped",
  "fastpq_reorders_transcripts",
  "drain_skips_fastpq_finalize",
  "apply_creates_missing_batch",
  "apply_ignores_shape",
  "apply_does_not_fill_missing",
  "apply_overwrites_existing_digest",
  "apply_not_zip_aligned"
}

Init ==
  checked = 0

Next ==
  /\ checked < 22
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..22
  /\ \A candidate \in Cases:
       /\ SpecActions(candidate) \subseteq Actions
       /\ ImplementationActions(candidate) \subseteq Actions

ActionsMatchSpec ==
  \A candidate \in Cases:
    ImplementationActions(candidate) = SpecActions(candidate)

ExecWitnessRecorderExactness ==
  /\ ActionsMatchSpec

ExecWitnessRecorderCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ExecWitnessRecorderExactness

Safety ==
  ExecWitnessRecorderExactness

====

---- MODULE SumeragiRestartReplayGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for restarted-peer replay and snapshot/Kura
consistency gates.

This slice models the restart-facing contracts in `try_read_snapshot_bundle`,
`ensure_state_is_backed_by_kura`, and the canonical WSV replay checkpoint
helpers. The model abstracts concrete hashes, JSON fields, and block bodies
into representative boundary cases while preserving the key safety obligations:
snapshot metadata and chain identity are verified before state is accepted,
snapshots cannot run ahead of Kura, required durable replay sections are
present, normal restart requires local block bodies while hard-fork bootstrap
uses only the durable hash journal, hash divergence is fail-closed except for
the latest-block revert path, snapshot writes require the state head to be
backed by Kura, and replay checkpoints ignore consensus sidecars while
preserving ledger WSV changes.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

RestoreDigestMismatch == 1
RestoreSignatureMismatch == 2
RestoreMerkleMismatch == 3
RestoreChainIdMismatch == 4
RestoreSnapshotAhead == 5
RestoreMissingOfflineKeys == 6
RestoreNormalMissingBlock == 7
RestoreHardForkMissingHash == 8
RestoreInteriorHashMismatch == 9
RestoreLatestHashMismatch == 10
RestoreHardForkHashMismatch == 11
RestoreLegacyManifestReplay == 12
RestoreLegacyManifestEmptyNoop == 13
RestoreCompleteSnapshot == 14
WriteZeroHeightAllowed == 15
WriteStateAhead == 16
WriteLatestHashMismatch == 17
WriteAlignedPublishes == 18
CanonicalCommitQcIgnored == 19
CanonicalConsensusEvidenceIgnored == 20
CanonicalVrfEpochIgnored == 21
CanonicalTopologyIgnored == 22
CanonicalMvCurrentOnly == 23
CanonicalSortsKeyPolicy == 24
CanonicalKeepsWsvMutation == 25
RestoreHardForkMatchingHash == 26
RestoreNormalHashOnlyNoBody == 27
RestoreManifestReplayFailure == 28

Candidates == 1..28

NoBug == 0
AcceptBadDigestBug == 1
AcceptBadSignatureBug == 2
AcceptBadMerkleBug == 3
AcceptWrongChainBug == 4
AcceptAheadSnapshotBug == 5
AcceptMissingOfflineKeysBug == 6
AcceptMissingNormalBlockBug == 7
AcceptMissingHardForkHashBug == 8
AcceptInteriorMismatchBug == 9
RejectLatestRevertBug == 10
AcceptHardForkMismatchBug == 11
SkipLegacyManifestReplayBug == 12
RejectEmptyLegacyManifestBug == 13
RejectCompleteSnapshotBug == 14
RejectZeroHeightWriteBug == 15
AcceptStateAheadWriteBug == 16
AcceptLatestHashMismatchWriteBug == 17
PublishWithoutAtomicTmpBug == 18
CanonicalKeepsCommitQcBug == 19
CanonicalKeepsConsensusEvidenceBug == 20
CanonicalKeepsVrfEpochBug == 21
CanonicalKeepsTopologyBug == 22
CanonicalKeepsMvHistoryBug == 23
CanonicalKeyPolicyOrderSensitiveBug == 24
CanonicalDropsWsvMutationBug == 25
HardForkRequiresBodyBug == 26
NormalAcceptsHashOnlyBug == 27
AcceptManifestReplayFailureBug == 28

Bugs == 0..28

Reject == 1
Accept == 2
VerifyDigest == 3
VerifySignature == 4
VerifyMerkle == 5
VerifyChainId == 6
CheckHeight == 7
RequireOfflineKeys == 8
RequireKuraBody == 9
RequireKuraHash == 10
CheckInteriorHash == 11
CheckLatestHash == 12
RevertLatest == 13
ReplaySpaceManifests == 14
NoopLegacyManifest == 15
CheckWriteBacked == 16
PublishSnapshot == 17
UseAtomicTmpFiles == 18
RedactCommitQc == 19
RedactConsensusEvidence == 20
RedactVrfEpoch == 21
RedactTopology == 22
NormalizeMvCell == 23
NormalizeKeyPolicy == 24
PreserveLedgerWsv == 25
UseHashJournal == 26

Actions == 1..26

SpecActions(candidate) ==
  CASE candidate = RestoreDigestMismatch -> {Reject, VerifyDigest}
    [] candidate = RestoreSignatureMismatch -> {Reject, VerifySignature}
    [] candidate = RestoreMerkleMismatch -> {Reject, VerifyMerkle}
    [] candidate = RestoreChainIdMismatch -> {Reject, VerifyChainId}
    [] candidate = RestoreSnapshotAhead -> {Reject, CheckHeight}
    [] candidate = RestoreMissingOfflineKeys -> {Reject, RequireOfflineKeys}
    [] candidate = RestoreNormalMissingBlock -> {Reject, RequireKuraBody}
    [] candidate = RestoreHardForkMissingHash ->
      {Reject, RequireKuraHash, UseHashJournal}
    [] candidate = RestoreInteriorHashMismatch ->
      {Reject, RequireKuraBody, CheckInteriorHash}
    [] candidate = RestoreLatestHashMismatch ->
      {Accept, RequireKuraBody, CheckLatestHash, RevertLatest}
    [] candidate = RestoreHardForkHashMismatch ->
      {Reject, RequireKuraHash, CheckInteriorHash, UseHashJournal}
    [] candidate = RestoreLegacyManifestReplay ->
      {Accept, RequireKuraBody, ReplaySpaceManifests}
    [] candidate = RestoreLegacyManifestEmptyNoop ->
      {Accept, NoopLegacyManifest}
    [] candidate = RestoreCompleteSnapshot ->
      {Accept, VerifyDigest, VerifySignature, VerifyMerkle, VerifyChainId,
       CheckHeight, RequireKuraBody, CheckInteriorHash, CheckLatestHash}
    [] candidate = WriteZeroHeightAllowed -> {Accept, CheckWriteBacked}
    [] candidate = WriteStateAhead -> {Reject, CheckWriteBacked}
    [] candidate = WriteLatestHashMismatch -> {Reject, CheckWriteBacked}
    [] candidate = WriteAlignedPublishes ->
      {Accept, CheckWriteBacked, PublishSnapshot, UseAtomicTmpFiles}
    [] candidate = CanonicalCommitQcIgnored -> {Accept, RedactCommitQc}
    [] candidate = CanonicalConsensusEvidenceIgnored ->
      {Accept, RedactConsensusEvidence}
    [] candidate = CanonicalVrfEpochIgnored -> {Accept, RedactVrfEpoch}
    [] candidate = CanonicalTopologyIgnored -> {Accept, RedactTopology}
    [] candidate = CanonicalMvCurrentOnly -> {Accept, NormalizeMvCell}
    [] candidate = CanonicalSortsKeyPolicy -> {Accept, NormalizeKeyPolicy}
    [] candidate = CanonicalKeepsWsvMutation -> {Accept, PreserveLedgerWsv}
    [] candidate = RestoreHardForkMatchingHash ->
      {Accept, RequireKuraHash, CheckInteriorHash, UseHashJournal}
    [] candidate = RestoreNormalHashOnlyNoBody -> {Reject, RequireKuraBody}
    [] candidate = RestoreManifestReplayFailure ->
      {Reject, RequireKuraBody, ReplaySpaceManifests}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = RestoreDigestMismatch /\ Bug = AcceptBadDigestBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreSignatureMismatch /\ Bug = AcceptBadSignatureBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreMerkleMismatch /\ Bug = AcceptBadMerkleBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreChainIdMismatch /\ Bug = AcceptWrongChainBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreSnapshotAhead /\ Bug = AcceptAheadSnapshotBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreMissingOfflineKeys /\
          Bug = AcceptMissingOfflineKeysBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreNormalMissingBlock /\
          Bug = AcceptMissingNormalBlockBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreHardForkMissingHash /\
          Bug = AcceptMissingHardForkHashBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreInteriorHashMismatch /\
          Bug = AcceptInteriorMismatchBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreLatestHashMismatch /\ Bug = RejectLatestRevertBug ->
      (spec \ {Accept, RevertLatest}) \cup {Reject}
    [] candidate = RestoreHardForkHashMismatch /\
          Bug = AcceptHardForkMismatchBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = RestoreLegacyManifestReplay /\
          Bug = SkipLegacyManifestReplayBug ->
      spec \ {ReplaySpaceManifests}
    [] candidate = RestoreLegacyManifestEmptyNoop /\
          Bug = RejectEmptyLegacyManifestBug ->
      (spec \ {Accept}) \cup {Reject}
    [] candidate = RestoreCompleteSnapshot /\ Bug = RejectCompleteSnapshotBug ->
      (spec \ {Accept}) \cup {Reject}
    [] candidate = WriteZeroHeightAllowed /\ Bug = RejectZeroHeightWriteBug ->
      (spec \ {Accept}) \cup {Reject}
    [] candidate = WriteStateAhead /\ Bug = AcceptStateAheadWriteBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = WriteLatestHashMismatch /\
          Bug = AcceptLatestHashMismatchWriteBug ->
      (spec \ {Reject}) \cup {Accept}
    [] candidate = WriteAlignedPublishes /\ Bug = PublishWithoutAtomicTmpBug ->
      spec \ {UseAtomicTmpFiles}
    [] candidate = CanonicalCommitQcIgnored /\
          Bug = CanonicalKeepsCommitQcBug ->
      spec \ {RedactCommitQc}
    [] candidate = CanonicalConsensusEvidenceIgnored /\
          Bug = CanonicalKeepsConsensusEvidenceBug ->
      spec \ {RedactConsensusEvidence}
    [] candidate = CanonicalVrfEpochIgnored /\
          Bug = CanonicalKeepsVrfEpochBug ->
      spec \ {RedactVrfEpoch}
    [] candidate = CanonicalTopologyIgnored /\
          Bug = CanonicalKeepsTopologyBug ->
      spec \ {RedactTopology}
    [] candidate = CanonicalMvCurrentOnly /\ Bug = CanonicalKeepsMvHistoryBug ->
      spec \ {NormalizeMvCell}
    [] candidate = CanonicalSortsKeyPolicy /\
          Bug = CanonicalKeyPolicyOrderSensitiveBug ->
      spec \ {NormalizeKeyPolicy}
    [] candidate = CanonicalKeepsWsvMutation /\
          Bug = CanonicalDropsWsvMutationBug ->
      spec \ {PreserveLedgerWsv}
    [] candidate = RestoreHardForkMatchingHash /\
          Bug = HardForkRequiresBodyBug ->
      (spec \ {Accept, RequireKuraHash, UseHashJournal}) \cup
        {Reject, RequireKuraBody}
    [] candidate = RestoreNormalHashOnlyNoBody /\
          Bug = NormalAcceptsHashOnlyBug ->
      (spec \ {Reject, RequireKuraBody}) \cup {Accept, RequireKuraHash}
    [] candidate = RestoreManifestReplayFailure /\
          Bug = AcceptManifestReplayFailureBug ->
      (spec \ {Reject}) \cup {Accept}
    [] OTHER -> spec

Init ==
  tried = {}

Next ==
  \E candidate \in Candidates \ tried:
    tried' = tried \cup {candidate}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried: ImplementationActions(candidate) \subseteq Actions

RestartReplaySnapshotValidationCases == {
  RestoreDigestMismatch,
  RestoreSignatureMismatch,
  RestoreMerkleMismatch,
  RestoreChainIdMismatch,
  RestoreSnapshotAhead,
  RestoreMissingOfflineKeys,
  RestoreCompleteSnapshot
}

RestartReplayKuraParityCases == {
  RestoreNormalMissingBlock,
  RestoreHardForkMissingHash,
  RestoreInteriorHashMismatch,
  RestoreLatestHashMismatch,
  RestoreHardForkHashMismatch,
  RestoreHardForkMatchingHash,
  RestoreNormalHashOnlyNoBody
}

RestartReplayLegacyManifestCases == {
  RestoreLegacyManifestReplay,
  RestoreLegacyManifestEmptyNoop,
  RestoreManifestReplayFailure
}

RestartReplayWriteBackCases == {
  WriteZeroHeightAllowed,
  WriteStateAhead,
  WriteLatestHashMismatch,
  WriteAlignedPublishes
}

RestartReplayCanonicalCheckpointCases == {
  CanonicalCommitQcIgnored,
  CanonicalConsensusEvidenceIgnored,
  CanonicalVrfEpochIgnored,
  CanonicalTopologyIgnored,
  CanonicalMvCurrentOnly,
  CanonicalSortsKeyPolicy,
  CanonicalKeepsWsvMutation
}

RestartReplayGroupedCases ==
  RestartReplaySnapshotValidationCases \cup
  RestartReplayKuraParityCases \cup
  RestartReplayLegacyManifestCases \cup
  RestartReplayWriteBackCases \cup
  RestartReplayCanonicalCheckpointCases

RestartReplayCaseGroupsComplete ==
  RestartReplayGroupedCases = Candidates

RestartReplayActionExactFor(cases) ==
  \A candidate \in tried:
    candidate \in cases =>
      ImplementationActions(candidate) = SpecActions(candidate)

RestartReplaySnapshotValidationExactness ==
  RestartReplayActionExactFor(RestartReplaySnapshotValidationCases)

RestartReplayKuraParityExactness ==
  RestartReplayActionExactFor(RestartReplayKuraParityCases)

RestartReplayLegacyManifestExactness ==
  RestartReplayActionExactFor(RestartReplayLegacyManifestCases)

RestartReplayWriteBackExactness ==
  RestartReplayActionExactFor(RestartReplayWriteBackCases)

RestartReplayCanonicalCheckpointExactness ==
  RestartReplayActionExactFor(RestartReplayCanonicalCheckpointCases)

RestartReplayExactness ==
  /\ RestartReplayCaseGroupsComplete
  /\ RestartReplaySnapshotValidationExactness
  /\ RestartReplayKuraParityExactness
  /\ RestartReplayLegacyManifestExactness
  /\ RestartReplayWriteBackExactness
  /\ RestartReplayCanonicalCheckpointExactness

Safety ==
  RestartReplayExactness

====

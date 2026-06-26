---- MODULE SumeragiPenaltyOffenderSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi evidence penalty attribution helpers.

This slice captures the deterministic helper contract in `penalties.rs` around
`canonicalize_index_for_view(...)`, `canonicalize_indices_for_view(...)`,
`npos_leader_index(...)`, `offender_indices(...)`,
`evidence_has_legitimate_empty_offenders(...)`, `censorship_anchor_height(...)`,
`evidence_epoch(...)`, `consensus_mode_for_evidence(...)`, and
`roster_for_evidence(...)`.

The model abstracts cryptographic NPoS shuffling as required bindings rather
than hashing bytes. It preserves the visible contract that permissioned view
rotation is modulo the active topology, NPoS attribution requires a VRF seed and
binds leader selection to height and view, invalid-QC bitmaps expand by set bits
across all bytes, censorship evidence anchors to the latest receipt height
capped by the recording height, evidence epochs come from the evidence subject
or anchored censorship height, consensus-mode lookup prefers subject height,
and roster recovery tries current, state snapshot, commit-QC, and checkpoint
sources while ignoring empty rosters.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PermissionedViewZero == "permissioned_view_zero"
PermissionedWrap == "permissioned_wrap"
CanonicalOutOfRange == "canonical_out_of_range"
CanonicalZeroTopology == "canonical_zero_topology"
CanonicalDedupSorted == "canonical_dedup_sorted"
DoubleVotePermissioned == "double_vote_permissioned"
InvalidProposalPermissioned == "invalid_proposal_permissioned"
InvalidQcBitmap == "invalid_qc_bitmap"
InvalidQcEmptyBitmap == "invalid_qc_empty_bitmap"
CensorshipPermissioned == "censorship_permissioned"
CensorshipNoReceipts == "censorship_no_receipts"
NposLeaderFirstCycle == "npos_leader_first_cycle"
NposLeaderWrap == "npos_leader_wrap"
NposMissingSeed == "npos_missing_seed"
NposCensorship == "npos_censorship"
EpochDoubleVote == "epoch_double_vote"
EpochInvalidProposal == "epoch_invalid_proposal"
EpochInvalidQc == "epoch_invalid_qc"
EpochCensorshipCapped == "epoch_censorship_capped"
EpochCensorshipNoReceipts == "epoch_censorship_no_receipts"
ModeSubjectHeight == "mode_subject_height"
ModeFallbackRecorded == "mode_fallback_recorded"
RosterNoRefsCommitTopology == "roster_no_refs_commit_topology"
RosterStateSnapshotFirst == "roster_state_snapshot_first"
RosterCommitCertFallback == "roster_commit_cert_fallback"
RosterCheckpointFallback == "roster_checkpoint_fallback"
RosterNoCandidates == "roster_no_candidates"

Cases == {
  PermissionedViewZero,
  PermissionedWrap,
  CanonicalOutOfRange,
  CanonicalZeroTopology,
  CanonicalDedupSorted,
  DoubleVotePermissioned,
  InvalidProposalPermissioned,
  InvalidQcBitmap,
  InvalidQcEmptyBitmap,
  CensorshipPermissioned,
  CensorshipNoReceipts,
  NposLeaderFirstCycle,
  NposLeaderWrap,
  NposMissingSeed,
  NposCensorship,
  EpochDoubleVote,
  EpochInvalidProposal,
  EpochInvalidQc,
  EpochCensorshipCapped,
  EpochCensorshipNoReceipts,
  ModeSubjectHeight,
  ModeFallbackRecorded,
  RosterNoRefsCommitTopology,
  RosterStateSnapshotFirst,
  RosterCommitCertFallback,
  RosterCheckpointFallback,
  RosterNoCandidates
}

TopologyNonEmpty == 1
RejectZeroTopology == 2
RejectOutOfRange == 3
PermissionedIdentityAtViewZero == 4
PermissionedModuloRotation == 5
WrongRotation == 6
DedupCanonicalIndices == 7
SortCanonicalIndices == 8
UseFirstDoubleVote == 9
UseSecondDoubleVote == 10
UseProposalHeader == 11
UseProposalQcRef == 12
ExpandBitmapBits == 13
ScanAllBitmapBytes == 14
CountBitmapBytes == 15
EmptyBitmapLegitimate == 16
CensorshipReceiptAnchor == 17
AnchorMaxReceiptHeight == 18
AnchorCappedToRecorded == 19
AnchorRecordedHeight == 20
CensorshipEmptyNoAnchor == 21
CensorshipLeaderSlotZero == 22
NposRequiresSeed == 23
NposUsesPrfPermutation == 24
NposSlotModuloTopology == 25
NposBindsHeight == 26
EpochFromVote == 27
EpochFromProposal == 28
EpochFromQc == 29
EpochFromCensorshipAnchor == 30
EpochEmptyCensorshipZero == 31
EpochFromRecorded == 32
ConsensusModeSubjectHeight == 33
ConsensusModeFallbackRecorded == 34
ConsensusModeZeroHeight == 35
RosterCurrentOnNoRefs == 36
RosterStateSnapshotFirstAction == 37
RosterCommitCertFallbackAction == 38
RosterCheckpointFallbackAction == 39
IgnoreEmptyRosters == 40
ReturnNoneWhenUnresolved == 41
ReturnEmptyRoster == 42
ReturnOffenders == 43
ReturnEmptyOffenders == 44

Actions == 1..44

PermissionedOffenderBase ==
  {TopologyNonEmpty, PermissionedModuloRotation, ReturnOffenders}

NposOffenderBase ==
  {TopologyNonEmpty, NposRequiresSeed, NposUsesPrfPermutation,
   NposSlotModuloTopology, NposBindsHeight, ReturnOffenders}

SpecActions(c) ==
  CASE c = PermissionedViewZero ->
      {TopologyNonEmpty, PermissionedIdentityAtViewZero, ReturnOffenders}
    [] c = PermissionedWrap ->
      PermissionedOffenderBase
    [] c = CanonicalOutOfRange ->
      {TopologyNonEmpty, RejectOutOfRange, ReturnEmptyOffenders}
    [] c = CanonicalZeroTopology ->
      {RejectZeroTopology, ReturnEmptyOffenders}
    [] c = CanonicalDedupSorted ->
      PermissionedOffenderBase \cup {DedupCanonicalIndices,
        SortCanonicalIndices}
    [] c = DoubleVotePermissioned ->
      PermissionedOffenderBase \cup {UseFirstDoubleVote}
    [] c = InvalidProposalPermissioned ->
      PermissionedOffenderBase \cup {UseProposalHeader}
    [] c = InvalidQcBitmap ->
      PermissionedOffenderBase \cup {ExpandBitmapBits, ScanAllBitmapBytes,
        DedupCanonicalIndices, SortCanonicalIndices}
    [] c = InvalidQcEmptyBitmap ->
      {ExpandBitmapBits, EmptyBitmapLegitimate, ReturnEmptyOffenders}
    [] c = CensorshipPermissioned ->
      {CensorshipReceiptAnchor, AnchorMaxReceiptHeight,
       AnchorCappedToRecorded, CensorshipLeaderSlotZero, TopologyNonEmpty,
       PermissionedIdentityAtViewZero, ReturnOffenders}
    [] c = CensorshipNoReceipts ->
      {CensorshipEmptyNoAnchor, ReturnEmptyOffenders}
    [] c \in {NposLeaderFirstCycle, NposLeaderWrap} ->
      NposOffenderBase
    [] c = NposMissingSeed ->
      {TopologyNonEmpty, NposRequiresSeed, ReturnEmptyOffenders}
    [] c = NposCensorship ->
      NposOffenderBase \cup {CensorshipReceiptAnchor,
        AnchorMaxReceiptHeight, AnchorCappedToRecorded,
        CensorshipLeaderSlotZero}
    [] c = EpochDoubleVote ->
      {EpochFromVote}
    [] c = EpochInvalidProposal ->
      {EpochFromProposal}
    [] c = EpochInvalidQc ->
      {EpochFromQc}
    [] c = EpochCensorshipCapped ->
      {CensorshipReceiptAnchor, AnchorMaxReceiptHeight,
       AnchorCappedToRecorded, EpochFromCensorshipAnchor}
    [] c = EpochCensorshipNoReceipts ->
      {CensorshipEmptyNoAnchor, EpochEmptyCensorshipZero}
    [] c = ModeSubjectHeight ->
      {ConsensusModeSubjectHeight}
    [] c = ModeFallbackRecorded ->
      {ConsensusModeFallbackRecorded}
    [] c = RosterNoRefsCommitTopology ->
      {RosterCurrentOnNoRefs, IgnoreEmptyRosters}
    [] c = RosterStateSnapshotFirst ->
      {RosterStateSnapshotFirstAction, IgnoreEmptyRosters}
    [] c = RosterCommitCertFallback ->
      {RosterCommitCertFallbackAction, IgnoreEmptyRosters}
    [] c = RosterCheckpointFallback ->
      {RosterCheckpointFallbackAction, IgnoreEmptyRosters}
    [] c = RosterNoCandidates ->
      {ReturnNoneWhenUnresolved}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "permissioned_rotation_ignored"
       /\ c = PermissionedWrap ->
      (spec \ {PermissionedModuloRotation}) \cup
        {PermissionedIdentityAtViewZero}
    [] Bug = "permissioned_rotation_off_by_one"
       /\ c = PermissionedWrap ->
      (spec \ {PermissionedModuloRotation}) \cup {WrongRotation}
    [] Bug = "view_zero_rotates"
       /\ c = PermissionedViewZero ->
      (spec \ {PermissionedIdentityAtViewZero}) \cup
        {PermissionedModuloRotation}
    [] Bug = "out_of_range_accepted"
       /\ c = CanonicalOutOfRange ->
      (spec \ {RejectOutOfRange, ReturnEmptyOffenders}) \cup
        {ReturnOffenders}
    [] Bug = "zero_topology_accepts"
       /\ c = CanonicalZeroTopology ->
      (spec \ {RejectZeroTopology, ReturnEmptyOffenders}) \cup
        {TopologyNonEmpty, ReturnOffenders}
    [] Bug = "duplicate_indices_kept"
       /\ c = CanonicalDedupSorted ->
      spec \ {DedupCanonicalIndices}
    [] Bug = "indices_unsorted"
       /\ c = CanonicalDedupSorted ->
      spec \ {SortCanonicalIndices}
    [] Bug = "double_vote_uses_second_vote"
       /\ c = DoubleVotePermissioned ->
      (spec \ {UseFirstDoubleVote}) \cup {UseSecondDoubleVote}
    [] Bug = "invalid_proposal_uses_qc_ref"
       /\ c = InvalidProposalPermissioned ->
      (spec \ {UseProposalHeader}) \cup {UseProposalQcRef}
    [] Bug = "invalid_qc_second_byte_ignored"
       /\ c = InvalidQcBitmap ->
      spec \ {ScanAllBitmapBytes}
    [] Bug = "invalid_qc_counts_bytes"
       /\ c = InvalidQcBitmap ->
      (spec \ {ExpandBitmapBits}) \cup {CountBitmapBytes}
    [] Bug = "empty_bitmap_not_legitimate"
       /\ c = InvalidQcEmptyBitmap ->
      spec \ {EmptyBitmapLegitimate}
    [] Bug = "empty_bitmap_blames_leader"
       /\ c = InvalidQcEmptyBitmap ->
      (spec \ {ReturnEmptyOffenders}) \cup {ReturnOffenders}
    [] Bug = "censorship_no_receipts_blames_leader"
       /\ c = CensorshipNoReceipts ->
      (spec \ {CensorshipEmptyNoAnchor, ReturnEmptyOffenders}) \cup
        {CensorshipLeaderSlotZero, ReturnOffenders}
    [] Bug = "censorship_anchor_uses_recorded"
       /\ c = CensorshipPermissioned ->
      (spec \ {AnchorMaxReceiptHeight, AnchorCappedToRecorded}) \cup
        {AnchorRecordedHeight}
    [] Bug = "censorship_anchor_not_capped"
       /\ c = EpochCensorshipCapped ->
      spec \ {AnchorCappedToRecorded}
    [] Bug = "censorship_uses_view_rotation"
       /\ c = CensorshipPermissioned ->
      (spec \ {PermissionedIdentityAtViewZero}) \cup
        {PermissionedModuloRotation}
    [] Bug = "npos_missing_seed_falls_back_permissioned"
       /\ c = NposMissingSeed ->
      (spec \ {NposRequiresSeed, ReturnEmptyOffenders}) \cup
        {PermissionedModuloRotation, ReturnOffenders}
    [] Bug = "npos_uses_permissioned_rotation"
       /\ c = NposLeaderFirstCycle ->
      (spec \ {NposUsesPrfPermutation}) \cup {PermissionedModuloRotation}
    [] Bug = "npos_slot_not_modulo"
       /\ c = NposLeaderWrap ->
      spec \ {NposSlotModuloTopology}
    [] Bug = "npos_height_ignored"
       /\ c = NposLeaderFirstCycle ->
      spec \ {NposBindsHeight}
    [] Bug = "epoch_double_uses_recorded"
       /\ c = EpochDoubleVote ->
      (spec \ {EpochFromVote}) \cup {EpochFromRecorded}
    [] Bug = "epoch_proposal_uses_recorded"
       /\ c = EpochInvalidProposal ->
      (spec \ {EpochFromProposal}) \cup {EpochFromRecorded}
    [] Bug = "epoch_qc_uses_recorded"
       /\ c = EpochInvalidQc ->
      (spec \ {EpochFromQc}) \cup {EpochFromRecorded}
    [] Bug = "epoch_censorship_empty_recorded"
       /\ c = EpochCensorshipNoReceipts ->
      (spec \ {EpochEmptyCensorshipZero}) \cup {EpochFromRecorded}
    [] Bug = "consensus_mode_uses_recorded_height"
       /\ c = ModeSubjectHeight ->
      (spec \ {ConsensusModeSubjectHeight}) \cup
        {ConsensusModeFallbackRecorded}
    [] Bug = "consensus_mode_missing_subject_uses_zero"
       /\ c = ModeFallbackRecorded ->
      (spec \ {ConsensusModeFallbackRecorded}) \cup {ConsensusModeZeroHeight}
    [] Bug = "roster_no_refs_returns_none"
       /\ c = RosterNoRefsCommitTopology ->
      (spec \ {RosterCurrentOnNoRefs}) \cup {ReturnNoneWhenUnresolved}
    [] Bug = "roster_skips_state_snapshot"
       /\ c = RosterStateSnapshotFirst ->
      (spec \ {RosterStateSnapshotFirstAction}) \cup
        {RosterCommitCertFallbackAction}
    [] Bug = "roster_skips_commit_cert"
       /\ c = RosterCommitCertFallback ->
      (spec \ {RosterCommitCertFallbackAction}) \cup
        {ReturnNoneWhenUnresolved}
    [] Bug = "roster_skips_checkpoint"
       /\ c = RosterCheckpointFallback ->
      (spec \ {RosterCheckpointFallbackAction}) \cup
        {ReturnNoneWhenUnresolved}
    [] Bug = "roster_accepts_empty_roster"
       /\ c = RosterNoRefsCommitTopology ->
      (spec \ {IgnoreEmptyRosters}) \cup {ReturnEmptyRoster}
    [] OTHER -> spec

Bugs == {
  "none",
  "permissioned_rotation_ignored",
  "permissioned_rotation_off_by_one",
  "view_zero_rotates",
  "out_of_range_accepted",
  "zero_topology_accepts",
  "duplicate_indices_kept",
  "indices_unsorted",
  "double_vote_uses_second_vote",
  "invalid_proposal_uses_qc_ref",
  "invalid_qc_second_byte_ignored",
  "invalid_qc_counts_bytes",
  "empty_bitmap_not_legitimate",
  "empty_bitmap_blames_leader",
  "censorship_no_receipts_blames_leader",
  "censorship_anchor_uses_recorded",
  "censorship_anchor_not_capped",
  "censorship_uses_view_rotation",
  "npos_missing_seed_falls_back_permissioned",
  "npos_uses_permissioned_rotation",
  "npos_slot_not_modulo",
  "npos_height_ignored",
  "epoch_double_uses_recorded",
  "epoch_proposal_uses_recorded",
  "epoch_qc_uses_recorded",
  "epoch_censorship_empty_recorded",
  "consensus_mode_uses_recorded_height",
  "consensus_mode_missing_subject_uses_zero",
  "roster_no_refs_returns_none",
  "roster_skips_state_snapshot",
  "roster_skips_commit_cert",
  "roster_skips_checkpoint",
  "roster_accepts_empty_roster"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

PenaltyOffenderSelectionCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == PenaltyOffenderSelectionCoreSafety

SafetyFast == PenaltyOffenderSelectionCoreSafety

CanonicalizationActionsMatchSpec ==
  \A c \in {
    PermissionedViewZero,
    PermissionedWrap,
    CanonicalOutOfRange,
    CanonicalZeroTopology,
    CanonicalDedupSorted
  }:
    ImplementationActions(c) = SpecActions(c)

EvidenceOffenderActionsMatchSpec ==
  \A c \in {
    DoubleVotePermissioned,
    InvalidProposalPermissioned,
    InvalidQcBitmap,
    InvalidQcEmptyBitmap,
    CensorshipPermissioned,
    CensorshipNoReceipts
  }:
    ImplementationActions(c) = SpecActions(c)

NposActionsMatchSpec ==
  \A c \in {
    NposLeaderFirstCycle,
    NposLeaderWrap,
    NposMissingSeed,
    NposCensorship
  }:
    ImplementationActions(c) = SpecActions(c)

EpochModeActionsMatchSpec ==
  \A c \in {
    EpochDoubleVote,
    EpochInvalidProposal,
    EpochInvalidQc,
    EpochCensorshipCapped,
    EpochCensorshipNoReceipts,
    ModeSubjectHeight,
    ModeFallbackRecorded
  }:
    ImplementationActions(c) = SpecActions(c)

RosterFallbackActionsMatchSpec ==
  \A c \in {
    RosterNoRefsCommitTopology,
    RosterStateSnapshotFirst,
    RosterCommitCertFallback,
    RosterCheckpointFallback,
    RosterNoCandidates
  }:
    ImplementationActions(c) = SpecActions(c)

CanonicalizationAnchors ==
  /\ PermissionedIdentityAtViewZero \in
       ImplementationActions(PermissionedViewZero)
  /\ PermissionedModuloRotation \in ImplementationActions(PermissionedWrap)
  /\ RejectOutOfRange \in ImplementationActions(CanonicalOutOfRange)
  /\ RejectZeroTopology \in ImplementationActions(CanonicalZeroTopology)
  /\ DedupCanonicalIndices \in ImplementationActions(CanonicalDedupSorted)
  /\ SortCanonicalIndices \in ImplementationActions(CanonicalDedupSorted)

EvidenceOffenderAnchors ==
  /\ UseFirstDoubleVote \in ImplementationActions(DoubleVotePermissioned)
  /\ UseProposalHeader \in ImplementationActions(InvalidProposalPermissioned)
  /\ ExpandBitmapBits \in ImplementationActions(InvalidQcBitmap)
  /\ ScanAllBitmapBytes \in ImplementationActions(InvalidQcBitmap)
  /\ EmptyBitmapLegitimate \in ImplementationActions(InvalidQcEmptyBitmap)
  /\ ReturnEmptyOffenders \in ImplementationActions(InvalidQcEmptyBitmap)

CensorshipAndNposAnchors ==
  /\ CensorshipReceiptAnchor \in ImplementationActions(CensorshipPermissioned)
  /\ AnchorMaxReceiptHeight \in ImplementationActions(CensorshipPermissioned)
  /\ AnchorCappedToRecorded \in ImplementationActions(CensorshipPermissioned)
  /\ CensorshipEmptyNoAnchor \in ImplementationActions(CensorshipNoReceipts)
  /\ NposRequiresSeed \in ImplementationActions(NposMissingSeed)
  /\ NposUsesPrfPermutation \in ImplementationActions(NposLeaderFirstCycle)
  /\ NposSlotModuloTopology \in ImplementationActions(NposLeaderWrap)
  /\ NposBindsHeight \in ImplementationActions(NposLeaderFirstCycle)

EpochModeAnchors ==
  /\ EpochFromVote \in ImplementationActions(EpochDoubleVote)
  /\ EpochFromProposal \in ImplementationActions(EpochInvalidProposal)
  /\ EpochFromQc \in ImplementationActions(EpochInvalidQc)
  /\ EpochFromCensorshipAnchor \in
       ImplementationActions(EpochCensorshipCapped)
  /\ EpochEmptyCensorshipZero \in
       ImplementationActions(EpochCensorshipNoReceipts)
  /\ ConsensusModeSubjectHeight \in ImplementationActions(ModeSubjectHeight)
  /\ ConsensusModeFallbackRecorded \in
       ImplementationActions(ModeFallbackRecorded)

RosterFallbackAnchors ==
  /\ RosterCurrentOnNoRefs \in ImplementationActions(RosterNoRefsCommitTopology)
  /\ RosterStateSnapshotFirstAction \in
       ImplementationActions(RosterStateSnapshotFirst)
  /\ RosterCommitCertFallbackAction \in
       ImplementationActions(RosterCommitCertFallback)
  /\ RosterCheckpointFallbackAction \in
       ImplementationActions(RosterCheckpointFallback)
  /\ IgnoreEmptyRosters \in ImplementationActions(RosterNoRefsCommitTopology)
  /\ ReturnNoneWhenUnresolved \in ImplementationActions(RosterNoCandidates)

PenaltyOffenderSelectionSafetyAnchors ==
  /\ CanonicalizationActionsMatchSpec
  /\ EvidenceOffenderActionsMatchSpec
  /\ NposActionsMatchSpec
  /\ EpochModeActionsMatchSpec
  /\ RosterFallbackActionsMatchSpec
  /\ CanonicalizationAnchors
  /\ EvidenceOffenderAnchors
  /\ CensorshipAndNposAnchors
  /\ EpochModeAnchors
  /\ RosterFallbackAnchors

PenaltyOffenderSelectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PenaltyOffenderSelectionCoreSafety
  /\ PenaltyOffenderSelectionSafetyAnchors

====

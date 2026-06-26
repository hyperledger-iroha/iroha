---- MODULE SumeragiViewChangeProofStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi view-change proof status accounting.

This slice captures `set_view_change_index(...)`,
`inc_view_change_proof_accepted()`, `inc_view_change_proof_stale()`,
`inc_view_change_proof_rejected()`, `inc_view_change_suggest()`,
`inc_view_change_install()`, the test-only
`reset_view_change_proof_counters_for_tests()` helper, and the corresponding
`snapshot()` projection fields. View-change cause labels are covered by
`SumeragiViewChangeCauseStatusGate`; this gate pins the independent proof,
suggest/install, and index status counters.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialZero == 1
SetIndex == 2
IndexOverwrite == 3
AcceptedProofRecord == 4
StaleProofRecord == 5
RejectedProofRecord == 6
SuggestRecord == 7
InstallRecord == 8
RepeatedAcceptedAccumulates == 9
ProofBucketsIndependent == 10
SuggestInstallIndependent == 11
SnapshotProjectsProofCounters == 12
SnapshotProjectsIndex == 13
ResetEmpty == 14
ResetAfterRecords == 15

Candidates == 1..15

InitialIndexZero == 1
InitialCountersZero == 2
IndexStored == 3
IndexOverwriteLatest == 4
AcceptedIncremented == 5
StaleIncremented == 6
RejectedIncremented == 7
SuggestIncremented == 8
InstallIncremented == 9
RepeatedAcceptedAccumulatesAction == 10
ProofBucketsIndependentAction == 11
SuggestInstallIndependentAction == 12
SnapshotAcceptedMatches == 13
SnapshotStaleMatches == 14
SnapshotRejectedMatches == 15
SnapshotSuggestMatches == 16
SnapshotInstallMatches == 17
SnapshotIndexMatches == 18
ResetProofCounters == 19
ResetSuggestInstallCounters == 20
ResetPreservesIndex == 21

Actions == 1..21

AllCounterActions ==
  {AcceptedIncremented, StaleIncremented, RejectedIncremented,
   SuggestIncremented, InstallIncremented}

SnapshotCounterActions ==
  {SnapshotAcceptedMatches, SnapshotStaleMatches, SnapshotRejectedMatches,
   SnapshotSuggestMatches, SnapshotInstallMatches}

ResetActions ==
  {ResetProofCounters, ResetSuggestInstallCounters, ResetPreservesIndex}

SpecActions(candidate) ==
  CASE candidate = InitialZero ->
      {InitialIndexZero, InitialCountersZero}
    [] candidate = SetIndex ->
      {IndexStored}
    [] candidate = IndexOverwrite ->
      {IndexStored, IndexOverwriteLatest}
    [] candidate = AcceptedProofRecord ->
      {AcceptedIncremented}
    [] candidate = StaleProofRecord ->
      {StaleIncremented}
    [] candidate = RejectedProofRecord ->
      {RejectedIncremented}
    [] candidate = SuggestRecord ->
      {SuggestIncremented}
    [] candidate = InstallRecord ->
      {InstallIncremented}
    [] candidate = RepeatedAcceptedAccumulates ->
      {AcceptedIncremented, RepeatedAcceptedAccumulatesAction}
    [] candidate = ProofBucketsIndependent ->
      {AcceptedIncremented, StaleIncremented, RejectedIncremented,
       ProofBucketsIndependentAction}
    [] candidate = SuggestInstallIndependent ->
      {SuggestIncremented, InstallIncremented,
       SuggestInstallIndependentAction}
    [] candidate = SnapshotProjectsProofCounters ->
      SnapshotCounterActions
    [] candidate = SnapshotProjectsIndex ->
      {SnapshotIndexMatches}
    [] candidate = ResetEmpty ->
      ResetActions
    [] candidate = ResetAfterRecords ->
      ResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialZero /\ Bug = "initial_index_nonzero" ->
      spec \ {InitialIndexZero}
    [] candidate = InitialZero /\ Bug = "initial_counters_nonzero" ->
      spec \ {InitialCountersZero}
    [] candidate \in {SetIndex, IndexOverwrite} /\
          Bug = "index_not_stored" ->
      spec \ {IndexStored}
    [] candidate = IndexOverwrite /\ Bug = "index_overwrite_ignored" ->
      spec \ {IndexOverwriteLatest}
    [] candidate \in {AcceptedProofRecord, RepeatedAcceptedAccumulates,
          ProofBucketsIndependent} /\ Bug = "accepted_not_counted" ->
      spec \ {AcceptedIncremented}
    [] candidate \in {StaleProofRecord, ProofBucketsIndependent} /\
          Bug = "stale_not_counted" ->
      spec \ {StaleIncremented}
    [] candidate \in {RejectedProofRecord, ProofBucketsIndependent} /\
          Bug = "rejected_not_counted" ->
      spec \ {RejectedIncremented}
    [] candidate \in {SuggestRecord, SuggestInstallIndependent} /\
          Bug = "suggest_not_counted" ->
      spec \ {SuggestIncremented}
    [] candidate \in {InstallRecord, SuggestInstallIndependent} /\
          Bug = "install_not_counted" ->
      spec \ {InstallIncremented}
    [] candidate = RepeatedAcceptedAccumulates /\
          Bug = "repeated_accepted_overwrites" ->
      spec \ {RepeatedAcceptedAccumulatesAction}
    [] candidate = ProofBucketsIndependent /\
          Bug = "proof_buckets_collide" ->
      spec \ {ProofBucketsIndependentAction}
    [] candidate = SuggestInstallIndependent /\
          Bug = "suggest_install_collide" ->
      spec \ {SuggestInstallIndependentAction}
    [] candidate = SnapshotProjectsProofCounters /\
          Bug = "snapshot_proof_mismatch" ->
      spec \ {SnapshotAcceptedMatches, SnapshotStaleMatches,
       SnapshotRejectedMatches}
    [] candidate = SnapshotProjectsProofCounters /\
          Bug = "snapshot_suggest_install_mismatch" ->
      spec \ {SnapshotSuggestMatches, SnapshotInstallMatches}
    [] candidate = SnapshotProjectsIndex /\ Bug = "snapshot_index_mismatch" ->
      spec \ {SnapshotIndexMatches}
    [] candidate \in {ResetEmpty, ResetAfterRecords} /\
          Bug = "reset_keeps_proof_counters" ->
      spec \ {ResetProofCounters}
    [] candidate \in {ResetEmpty, ResetAfterRecords} /\
          Bug = "reset_keeps_suggest_install" ->
      spec \ {ResetSuggestInstallCounters}
    [] candidate = ResetAfterRecords /\ Bug = "reset_clears_index" ->
      spec \ {ResetPreservesIndex}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 15
     /\ checked' = checked + 1
  \/ /\ checked = 15
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in {
       "none",
       "initial_index_nonzero",
       "initial_counters_nonzero",
       "index_not_stored",
       "index_overwrite_ignored",
       "accepted_not_counted",
       "stale_not_counted",
       "rejected_not_counted",
       "suggest_not_counted",
       "install_not_counted",
       "repeated_accepted_overwrites",
       "proof_buckets_collide",
       "suggest_install_collide",
       "snapshot_proof_mismatch",
       "snapshot_suggest_install_mismatch",
       "snapshot_index_mismatch",
       "reset_keeps_proof_counters",
       "reset_keeps_suggest_install",
       "reset_clears_index"
     }
  /\ checked \in 0..15
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugInitialIndexNonzero ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugInitialCountersNonzero ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugIndexNotStored ==
  ImplementationActions(SetIndex) = SpecActions(SetIndex)

BugIndexOverwriteIgnored ==
  ImplementationActions(IndexOverwrite) = SpecActions(IndexOverwrite)

BugAcceptedNotCounted ==
  ImplementationActions(AcceptedProofRecord) =
    SpecActions(AcceptedProofRecord)

BugStaleNotCounted ==
  ImplementationActions(StaleProofRecord) = SpecActions(StaleProofRecord)

BugRejectedNotCounted ==
  ImplementationActions(RejectedProofRecord) =
    SpecActions(RejectedProofRecord)

BugSuggestNotCounted ==
  ImplementationActions(SuggestRecord) = SpecActions(SuggestRecord)

BugInstallNotCounted ==
  ImplementationActions(InstallRecord) = SpecActions(InstallRecord)

BugRepeatedAcceptedOverwrites ==
  ImplementationActions(RepeatedAcceptedAccumulates) =
    SpecActions(RepeatedAcceptedAccumulates)

BugProofBucketsCollide ==
  ImplementationActions(ProofBucketsIndependent) =
    SpecActions(ProofBucketsIndependent)

BugSuggestInstallCollide ==
  ImplementationActions(SuggestInstallIndependent) =
    SpecActions(SuggestInstallIndependent)

BugSnapshotProofMismatch ==
  ImplementationActions(SnapshotProjectsProofCounters) =
    SpecActions(SnapshotProjectsProofCounters)

BugSnapshotSuggestInstallMismatch ==
  ImplementationActions(SnapshotProjectsProofCounters) =
    SpecActions(SnapshotProjectsProofCounters)

BugSnapshotIndexMismatch ==
  ImplementationActions(SnapshotProjectsIndex) =
    SpecActions(SnapshotProjectsIndex)

BugResetKeepsProofCounters ==
  ImplementationActions(ResetAfterRecords) = SpecActions(ResetAfterRecords)

BugResetKeepsSuggestInstall ==
  ImplementationActions(ResetAfterRecords) = SpecActions(ResetAfterRecords)

BugResetClearsIndex ==
  ImplementationActions(ResetAfterRecords) = SpecActions(ResetAfterRecords)

AllViewChangeProofCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

InitialStatusAnchors ==
  /\ InitialIndexZero \in ImplementationActions(InitialZero)
  /\ InitialCountersZero \in ImplementationActions(InitialZero)
  /\ AllCounterActions \cap ImplementationActions(InitialZero) = {}

IndexStorageAnchors ==
  /\ IndexStored \in ImplementationActions(SetIndex)
  /\ IndexStored \in ImplementationActions(IndexOverwrite)
  /\ IndexOverwriteLatest \in ImplementationActions(IndexOverwrite)

ProofCounterAnchors ==
  /\ AcceptedIncremented \in ImplementationActions(AcceptedProofRecord)
  /\ StaleIncremented \in ImplementationActions(StaleProofRecord)
  /\ RejectedIncremented \in ImplementationActions(RejectedProofRecord)
  /\ ~(StaleIncremented \in ImplementationActions(AcceptedProofRecord))
  /\ ~(RejectedIncremented \in ImplementationActions(StaleProofRecord))

SuggestInstallAnchors ==
  /\ SuggestIncremented \in ImplementationActions(SuggestRecord)
  /\ InstallIncremented \in ImplementationActions(InstallRecord)
  /\ ~(InstallIncremented \in ImplementationActions(SuggestRecord))
  /\ ~(SuggestIncremented \in ImplementationActions(InstallRecord))

AccumulationAnchors ==
  /\ RepeatedAcceptedAccumulatesAction \in
       ImplementationActions(RepeatedAcceptedAccumulates)
  /\ ProofBucketsIndependentAction \in
       ImplementationActions(ProofBucketsIndependent)
  /\ SuggestInstallIndependentAction \in
       ImplementationActions(SuggestInstallIndependent)
  /\ AllCounterActions \subseteq
       (ImplementationActions(ProofBucketsIndependent) \cup
        ImplementationActions(SuggestInstallIndependent))

SnapshotProjectionAnchors ==
  /\ SnapshotCounterActions \subseteq
       ImplementationActions(SnapshotProjectsProofCounters)
  /\ SnapshotIndexMatches \in ImplementationActions(SnapshotProjectsIndex)

ResetAnchors ==
  /\ ResetActions \subseteq ImplementationActions(ResetEmpty)
  /\ ResetActions \subseteq ImplementationActions(ResetAfterRecords)

SafetyAnchors ==
  /\ AllViewChangeProofCandidatesMatchSpec
  /\ InitialStatusAnchors
  /\ IndexStorageAnchors
  /\ ProofCounterAnchors
  /\ SuggestInstallAnchors
  /\ AccumulationAnchors
  /\ SnapshotProjectionAnchors
  /\ ResetAnchors

ViewChangeProofStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ Safety
  /\ SafetyAnchors

====

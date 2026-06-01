---- MODULE SumeragiFrontierRepairViewChangeGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for
`suppress_quorum_view_change_while_frontier_repair_active(...)`.

The helper suppresses only QuorumTimeout/StakeQuorumTimeout view changes at the
contiguous frontier. Committed-edge conflict ownership and passive committed
anchor catch-up suppress first without repair seeding or urgent body fetches.
Otherwise, a direct frontier-slot view advance request or an authoritative
payload lets the view change proceed. Only active exact-body repair, same-slot
missing-payload recovery, or same-slot reassembly activity may suppress the
view change; those repair suppressions seed quorum-timeout frontier recovery
when possible and emit an urgent frontier body fetch.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Str;
  checked

\* @type: <<Str>>;
vars == <<checked>>

QuorumCause == "quorum_cause"
StakeQuorumCause == "stake_quorum_cause"
MissingQcCause == "missing_qc_cause"
OtherCause == "other_cause"

NonFrontierHeight == "non_frontier_height"
CommittedEdgeSuppression == "committed_edge_suppression"
PassiveCatchupSuppression == "passive_catchup_suppression"
DirectAdvanceRequest == "direct_advance_request"
AuthoritativePayload == "authoritative_payload"
NoRepairActive == "no_repair_active"
ExactRepairActive == "exact_repair_active"
ExactRepairViewMismatch == "exact_repair_view_mismatch"
ExactRepairUnarmed == "exact_repair_unarmed"
ExactRepairBodyPresent == "exact_repair_body_present"
ExactRepairWrongMode == "exact_repair_wrong_mode"
MissingPayloadRecoveryActive == "missing_payload_recovery_active"
ReassemblyActive == "reassembly_active"
MultipleRepairSources == "multiple_repair_sources"
RepairExistingFrontierSlotSeed == "repair_existing_frontier_slot_seed"
RepairNoSlotSeedFromEvidence == "repair_no_slot_seed_from_evidence"
RepairExistingRecoveryNoSeed == "repair_existing_recovery_no_seed"
RepairNoEvidenceCreatesCatchup == "repair_no_evidence_creates_catchup"
CommitEdgePreemptsDirectAdvance == "commit_edge_preempts_direct_advance"
PassivePreemptsAuthoritative == "passive_preempts_authoritative"

CauseCases == {QuorumCause, StakeQuorumCause, MissingQcCause, OtherCause}

BranchCases == {
  NonFrontierHeight,
  CommittedEdgeSuppression,
  PassiveCatchupSuppression,
  DirectAdvanceRequest,
  AuthoritativePayload,
  NoRepairActive,
  ExactRepairActive,
  ExactRepairViewMismatch,
  ExactRepairUnarmed,
  ExactRepairBodyPresent,
  ExactRepairWrongMode,
  MissingPayloadRecoveryActive,
  ReassemblyActive,
  MultipleRepairSources,
  RepairExistingFrontierSlotSeed,
  RepairNoSlotSeedFromEvidence,
  RepairExistingRecoveryNoSeed,
  RepairNoEvidenceCreatesCatchup,
  CommitEdgePreemptsDirectAdvance,
  PassivePreemptsAuthoritative
}

Cases == CauseCases \cup BranchCases

CauseAccepted == 1
CauseIgnored == 2
HeightMatched == 3
HeightMismatched == 4
CommitEdgeChecked == 5
CommitEdgeSuppresses == 6
PassiveChecked == 7
PassiveSuppresses == 8
DirectAdvanceChecked == 9
DirectAdvanceAllows == 10
DirectAdvanceSkipped == 11
AuthoritativeChecked == 12
AuthoritativeAllows == 13
AuthoritativeSkipped == 14
RepairChecked == 15
ExactRepairSource == 16
MissingPayloadSource == 17
ReassemblySource == 18
NoRepairSource == 19
ExactViewMatched == 20
ExactViewMismatched == 21
ExactFetchArmed == 22
ExactFetchUnarmed == 23
ExactBodyMissing == 24
ExactBodyPresent == 25
ExactNormalMode == 26
ExactWrongMode == 27
SuppressViewChange == 28
AllowViewChange == 29
SeedRecovery == 30
NoSeedRecovery == 31
UrgentBodyFetch == 32
NoUrgentBodyFetch == 33
SeedSameSlot == 34
SeedFromEvidence == 35
SeedCreatesCatchup == 36
SeedExistingRecoveryBlocked == 37
SeedQuorumTimeoutCause == 38
ActiveViewMaxed == 39
NoProgressWindowsTwo == 40

ActionUniverse == 1..40

AcceptedCauses == {QuorumCause, StakeQuorumCause}

BaseRepairSuppression ==
  {CauseAccepted, HeightMatched, CommitEdgeChecked, PassiveChecked,
   DirectAdvanceChecked, AuthoritativeChecked, RepairChecked,
   SuppressViewChange, SeedRecovery, UrgentBodyFetch}

BaseAllowAfterChecks ==
  {CauseAccepted, HeightMatched, CommitEdgeChecked, PassiveChecked,
   DirectAdvanceChecked, AuthoritativeChecked, RepairChecked,
   AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}

SpecActions(c) ==
  CASE c \in AcceptedCauses ->
      {CauseAccepted}
    [] c \in {MissingQcCause, OtherCause} ->
      {CauseIgnored, AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] c = NonFrontierHeight ->
      {CauseAccepted, HeightMismatched, AllowViewChange,
       NoSeedRecovery, NoUrgentBodyFetch}
    [] c = CommittedEdgeSuppression ->
      {CauseAccepted, HeightMatched, CommitEdgeChecked,
       CommitEdgeSuppresses, SuppressViewChange,
       NoSeedRecovery, NoUrgentBodyFetch}
    [] c = PassiveCatchupSuppression ->
      {CauseAccepted, HeightMatched, CommitEdgeChecked, PassiveChecked,
       PassiveSuppresses, SuppressViewChange,
       NoSeedRecovery, NoUrgentBodyFetch}
    [] c = DirectAdvanceRequest ->
      {CauseAccepted, HeightMatched, CommitEdgeChecked, PassiveChecked,
       DirectAdvanceChecked, DirectAdvanceAllows, AllowViewChange,
       NoSeedRecovery, NoUrgentBodyFetch}
    [] c = AuthoritativePayload ->
      {CauseAccepted, HeightMatched, CommitEdgeChecked, PassiveChecked,
       DirectAdvanceChecked, AuthoritativeChecked, AuthoritativeAllows,
       AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] c = NoRepairActive ->
      BaseAllowAfterChecks \cup {NoRepairSource}
    [] c = ExactRepairActive ->
      BaseRepairSuppression \cup {ExactRepairSource, ExactViewMatched,
       ExactFetchArmed, ExactBodyMissing, ExactNormalMode}
    [] c = ExactRepairViewMismatch ->
      BaseAllowAfterChecks \cup {NoRepairSource, ExactViewMismatched,
       ExactFetchArmed, ExactBodyMissing, ExactNormalMode}
    [] c = ExactRepairUnarmed ->
      BaseAllowAfterChecks \cup {NoRepairSource, ExactViewMatched,
       ExactFetchUnarmed, ExactBodyMissing, ExactNormalMode}
    [] c = ExactRepairBodyPresent ->
      BaseAllowAfterChecks \cup {NoRepairSource, ExactViewMatched,
       ExactFetchArmed, ExactBodyPresent, ExactNormalMode}
    [] c = ExactRepairWrongMode ->
      BaseAllowAfterChecks \cup {NoRepairSource, ExactViewMatched,
       ExactFetchArmed, ExactBodyMissing, ExactWrongMode}
    [] c = MissingPayloadRecoveryActive ->
      BaseRepairSuppression \cup {MissingPayloadSource}
    [] c = ReassemblyActive ->
      BaseRepairSuppression \cup {ReassemblySource}
    [] c = MultipleRepairSources ->
      BaseRepairSuppression \cup {ExactRepairSource, MissingPayloadSource,
       ReassemblySource, ExactViewMatched, ExactFetchArmed, ExactBodyMissing,
       ExactNormalMode}
    [] c = RepairExistingFrontierSlotSeed ->
      BaseRepairSuppression \cup {ExactRepairSource, SeedSameSlot,
       SeedQuorumTimeoutCause, ActiveViewMaxed}
    [] c = RepairNoSlotSeedFromEvidence ->
      BaseRepairSuppression \cup {MissingPayloadSource, SeedFromEvidence,
       SeedQuorumTimeoutCause}
    [] c = RepairExistingRecoveryNoSeed ->
      (BaseRepairSuppression \ {SeedRecovery}) \cup {NoSeedRecovery,
       ReassemblySource, SeedExistingRecoveryBlocked}
    [] c = RepairNoEvidenceCreatesCatchup ->
      BaseRepairSuppression \cup {ReassemblySource, SeedCreatesCatchup,
       SeedQuorumTimeoutCause, NoProgressWindowsTwo}
    [] c = CommitEdgePreemptsDirectAdvance ->
      {CauseAccepted, HeightMatched, CommitEdgeChecked,
       CommitEdgeSuppresses, DirectAdvanceSkipped, SuppressViewChange,
       NoSeedRecovery, NoUrgentBodyFetch}
    [] c = PassivePreemptsAuthoritative ->
      {CauseAccepted, HeightMatched, CommitEdgeChecked, PassiveChecked,
       PassiveSuppresses, AuthoritativeSkipped, SuppressViewChange,
       NoSeedRecovery, NoUrgentBodyFetch}
    [] OTHER -> {}

ImplementationActions(c) ==
  CASE Bug = "reject_quorum_cause"
       /\ c = QuorumCause ->
      {CauseIgnored, AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] Bug = "reject_stake_quorum_cause"
       /\ c = StakeQuorumCause ->
      {CauseIgnored, AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] Bug = "accept_missing_qc_cause"
       /\ c = MissingQcCause ->
      {CauseAccepted}
    [] Bug = "suppress_nonfrontier_height"
       /\ c = NonFrontierHeight ->
      (SpecActions(c) \ {AllowViewChange}) \cup {SuppressViewChange}
    [] Bug = "skip_committed_edge_suppression"
       /\ c = CommittedEdgeSuppression ->
      (SpecActions(c) \ {SuppressViewChange, CommitEdgeSuppresses})
        \cup {AllowViewChange}
    [] Bug = "committed_edge_seeds_fetches"
       /\ c = CommittedEdgeSuppression ->
      (SpecActions(c) \ {NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SeedRecovery, UrgentBodyFetch}
    [] Bug = "skip_passive_catchup_suppression"
       /\ c = PassiveCatchupSuppression ->
      (SpecActions(c) \ {SuppressViewChange, PassiveSuppresses})
        \cup {AllowViewChange}
    [] Bug = "passive_catchup_seeds_fetches"
       /\ c = PassiveCatchupSuppression ->
      (SpecActions(c) \ {NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SeedRecovery, UrgentBodyFetch}
    [] Bug = "direct_advance_suppresses"
       /\ c = DirectAdvanceRequest ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "authoritative_payload_suppresses"
       /\ c = AuthoritativePayload ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "no_repair_suppresses"
       /\ c = NoRepairActive ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "exact_view_mismatch_suppresses"
       /\ c = ExactRepairViewMismatch ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "unarmed_exact_suppresses"
       /\ c = ExactRepairUnarmed ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "body_present_suppresses"
       /\ c = ExactRepairBodyPresent ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "wrong_mode_suppresses"
       /\ c = ExactRepairWrongMode ->
      (SpecActions(c) \ {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch})
        \cup {SuppressViewChange, SeedRecovery, UrgentBodyFetch}
    [] Bug = "reject_exact_repair"
       /\ c = ExactRepairActive ->
      (SpecActions(c) \ {SuppressViewChange, SeedRecovery, UrgentBodyFetch})
        \cup {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] Bug = "reject_missing_payload_recovery"
       /\ c = MissingPayloadRecoveryActive ->
      (SpecActions(c) \ {SuppressViewChange, SeedRecovery, UrgentBodyFetch})
        \cup {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] Bug = "reject_reassembly_recovery"
       /\ c = ReassemblyActive ->
      (SpecActions(c) \ {SuppressViewChange, SeedRecovery, UrgentBodyFetch})
        \cup {AllowViewChange, NoSeedRecovery, NoUrgentBodyFetch}
    [] Bug = "repair_skips_seed"
       /\ c = ExactRepairActive ->
      (SpecActions(c) \ {SeedRecovery}) \cup {NoSeedRecovery}
    [] Bug = "repair_skips_fetch"
       /\ c = ExactRepairActive ->
      (SpecActions(c) \ {UrgentBodyFetch}) \cup {NoUrgentBodyFetch}
    [] Bug = "seed_wrong_cause"
       /\ c = RepairExistingFrontierSlotSeed ->
      SpecActions(c) \ {SeedQuorumTimeoutCause}
    [] Bug = "seed_lowers_active_view"
       /\ c = RepairExistingFrontierSlotSeed ->
      SpecActions(c) \ {ActiveViewMaxed}
    [] Bug = "seed_wrong_no_progress"
       /\ c = RepairNoEvidenceCreatesCatchup ->
      SpecActions(c) \ {NoProgressWindowsTwo}
    [] Bug = "seed_overwrites_existing_recovery"
       /\ c = RepairExistingRecoveryNoSeed ->
      (SpecActions(c) \ {NoSeedRecovery, SeedExistingRecoveryBlocked})
        \cup {SeedRecovery, SeedCreatesCatchup}
    [] Bug = "commit_edge_checks_direct_first"
       /\ c = CommitEdgePreemptsDirectAdvance ->
      (SpecActions(c) \ {SuppressViewChange, DirectAdvanceSkipped})
        \cup {AllowViewChange, DirectAdvanceChecked}
    [] Bug = "passive_checks_authoritative_first"
       /\ c = PassivePreemptsAuthoritative ->
      (SpecActions(c) \ {SuppressViewChange, AuthoritativeSkipped})
        \cup {AllowViewChange, AuthoritativeChecked}
    [] OTHER -> SpecActions(c)

Init ==
  checked \in Cases

Next ==
  UNCHANGED checked

TypeInvariant ==
  /\ checked \in Cases
  /\ \A c \in Cases : SpecActions(c) \subseteq ActionUniverse
  /\ \A c \in Cases : ImplementationActions(c) \subseteq ActionUniverse

CauseSafety ==
  \A c \in CauseCases : ImplementationActions(c) = SpecActions(c)

EarlyExitSafety ==
  /\ ImplementationActions(NonFrontierHeight) = SpecActions(NonFrontierHeight)
  /\ ImplementationActions(CommittedEdgeSuppression) = SpecActions(CommittedEdgeSuppression)
  /\ ImplementationActions(PassiveCatchupSuppression) = SpecActions(PassiveCatchupSuppression)
  /\ ImplementationActions(DirectAdvanceRequest) = SpecActions(DirectAdvanceRequest)
  /\ ImplementationActions(AuthoritativePayload) = SpecActions(AuthoritativePayload)

RepairSourceSafety ==
  /\ ImplementationActions(NoRepairActive) = SpecActions(NoRepairActive)
  /\ ImplementationActions(ExactRepairActive) = SpecActions(ExactRepairActive)
  /\ ImplementationActions(ExactRepairViewMismatch) = SpecActions(ExactRepairViewMismatch)
  /\ ImplementationActions(ExactRepairUnarmed) = SpecActions(ExactRepairUnarmed)
  /\ ImplementationActions(ExactRepairBodyPresent) = SpecActions(ExactRepairBodyPresent)
  /\ ImplementationActions(ExactRepairWrongMode) = SpecActions(ExactRepairWrongMode)
  /\ ImplementationActions(MissingPayloadRecoveryActive) = SpecActions(MissingPayloadRecoveryActive)
  /\ ImplementationActions(ReassemblyActive) = SpecActions(ReassemblyActive)
  /\ ImplementationActions(MultipleRepairSources) = SpecActions(MultipleRepairSources)

SeedAndFetchSafety ==
  /\ ImplementationActions(RepairExistingFrontierSlotSeed) = SpecActions(RepairExistingFrontierSlotSeed)
  /\ ImplementationActions(RepairNoSlotSeedFromEvidence) = SpecActions(RepairNoSlotSeedFromEvidence)
  /\ ImplementationActions(RepairExistingRecoveryNoSeed) = SpecActions(RepairExistingRecoveryNoSeed)
  /\ ImplementationActions(RepairNoEvidenceCreatesCatchup) = SpecActions(RepairNoEvidenceCreatesCatchup)

PrecedenceSafety ==
  /\ ImplementationActions(CommitEdgePreemptsDirectAdvance) = SpecActions(CommitEdgePreemptsDirectAdvance)
  /\ ImplementationActions(PassivePreemptsAuthoritative) = SpecActions(PassivePreemptsAuthoritative)

SafetyFast ==
  /\ CauseSafety
  /\ EarlyExitSafety
  /\ RepairSourceSafety
  /\ SeedAndFetchSafety
  /\ PrecedenceSafety

CauseAnchors ==
  /\ CauseSafety
  /\ \A c \in CauseCases : ImplementationActions(c) = SpecActions(c)

EarlyExitAnchors ==
  /\ EarlyExitSafety
  /\ ImplementationActions(NonFrontierHeight) = SpecActions(NonFrontierHeight)
  /\ ImplementationActions(CommittedEdgeSuppression) = SpecActions(CommittedEdgeSuppression)
  /\ ImplementationActions(PassiveCatchupSuppression) = SpecActions(PassiveCatchupSuppression)
  /\ ImplementationActions(DirectAdvanceRequest) = SpecActions(DirectAdvanceRequest)
  /\ ImplementationActions(AuthoritativePayload) = SpecActions(AuthoritativePayload)

RepairSourceAnchors ==
  /\ RepairSourceSafety
  /\ ImplementationActions(NoRepairActive) = SpecActions(NoRepairActive)
  /\ ImplementationActions(ExactRepairActive) = SpecActions(ExactRepairActive)
  /\ ImplementationActions(ExactRepairViewMismatch) = SpecActions(ExactRepairViewMismatch)
  /\ ImplementationActions(ExactRepairUnarmed) = SpecActions(ExactRepairUnarmed)
  /\ ImplementationActions(ExactRepairBodyPresent) = SpecActions(ExactRepairBodyPresent)
  /\ ImplementationActions(ExactRepairWrongMode) = SpecActions(ExactRepairWrongMode)
  /\ ImplementationActions(MissingPayloadRecoveryActive) = SpecActions(MissingPayloadRecoveryActive)
  /\ ImplementationActions(ReassemblyActive) = SpecActions(ReassemblyActive)
  /\ ImplementationActions(MultipleRepairSources) = SpecActions(MultipleRepairSources)

SeedAndFetchAnchors ==
  /\ SeedAndFetchSafety
  /\ ImplementationActions(RepairExistingFrontierSlotSeed) = SpecActions(RepairExistingFrontierSlotSeed)
  /\ ImplementationActions(RepairNoSlotSeedFromEvidence) = SpecActions(RepairNoSlotSeedFromEvidence)
  /\ ImplementationActions(RepairExistingRecoveryNoSeed) = SpecActions(RepairExistingRecoveryNoSeed)
  /\ ImplementationActions(RepairNoEvidenceCreatesCatchup) = SpecActions(RepairNoEvidenceCreatesCatchup)

PrecedenceAnchors ==
  /\ PrecedenceSafety
  /\ ImplementationActions(CommitEdgePreemptsDirectAdvance) = SpecActions(CommitEdgePreemptsDirectAdvance)
  /\ ImplementationActions(PassivePreemptsAuthoritative) = SpecActions(PassivePreemptsAuthoritative)

FrontierRepairViewChangeSafetyAnchors ==
  /\ CauseAnchors
  /\ EarlyExitAnchors
  /\ RepairSourceAnchors
  /\ SeedAndFetchAnchors
  /\ PrecedenceAnchors

Safety ==
  FrontierRepairViewChangeSafetyAnchors

====

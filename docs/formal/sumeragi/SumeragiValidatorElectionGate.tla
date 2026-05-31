---- MODULE SumeragiValidatorElectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for NPoS validator election.

This slice captures `filter_candidates_with_constraints(...)`,
`elect_validator_set(...)`, and `entity_key(...)` from
`crates/iroha_core/src/sumeragi/election.rs`. It abstracts concrete peer ids,
metadata, and Blake2 scores to the finite decisions that must remain
deterministic across nodes:

- council-style candidates without staking records are accepted only when all
  filtering constraints are disabled,
- self-bond, nomination-bond, and nominator-concentration constraints fail
  closed and use full-precision numeric comparisons,
- scoring binds the shared seed and peer key, then sorts by score with peer-id
  tie breaking independent of input order,
- `max_validators == 0`, base-take clamping, seat-band ceiling, and desired
  clamping produce the same selected-size window everywhere,
- entity-correlation caps use metadata first, then validator identity, then the
  peer id, floor at one seat, and use the banded desired size only when a seat
  band is active,
- deferred correlated candidates may fill the base target but not extra band
  seats, and
- the outcome binds epoch, snapshot height, seed, params, candidate count, and
  validator-set hash to the selected set.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CouncilNoRecordAllowed == "council_no_record_allowed"
CouncilNoRecordRejected == "council_no_record_rejected"
SelfBondPass == "self_bond_pass"
SelfBondReject == "self_bond_reject"
SelfNominationIgnored == "self_nomination_ignored"
UndersizedNominationReject == "undersized_nomination_reject"
ConcentrationDisabledAccept == "concentration_disabled_accept"
ConcentrationZeroTotalReject == "concentration_zero_total_reject"
ConcentrationNoNominatorAccept == "concentration_no_nominator_accept"
ConcentrationBoundaryAccept == "concentration_boundary_accept"
ConcentrationExceededReject == "concentration_exceeded_reject"
NumericPrecisionSelfBond == "numeric_precision_self_bond"
ElectionOrdering == "election_ordering"
MaxZeroTakesAll == "max_zero_takes_all"
MaxValidatorsCaps == "max_validators_caps"
SeatBandCeil == "seat_band_ceil"
DesiredClamp == "desired_clamp"
EntityMetadataPreferred == "entity_metadata_preferred"
EntityValidatorFallback == "entity_validator_fallback"
EntityPeerFallback == "entity_peer_fallback"
CorrelationDisabled == "correlation_disabled"
CorrelationNoBandCap == "correlation_no_band_cap"
CorrelationBandCap == "correlation_band_cap"
CorrelationDefersToFillBase == "correlation_defers_to_fill_base"
CorrelationTrimsBand == "correlation_trims_band"
EmptyElection == "empty_election"
OutcomeFields == "outcome_fields"

Cases == {
  CouncilNoRecordAllowed,
  CouncilNoRecordRejected,
  SelfBondPass,
  SelfBondReject,
  SelfNominationIgnored,
  UndersizedNominationReject,
  ConcentrationDisabledAccept,
  ConcentrationZeroTotalReject,
  ConcentrationNoNominatorAccept,
  ConcentrationBoundaryAccept,
  ConcentrationExceededReject,
  NumericPrecisionSelfBond,
  ElectionOrdering,
  MaxZeroTakesAll,
  MaxValidatorsCaps,
  SeatBandCeil,
  DesiredClamp,
  EntityMetadataPreferred,
  EntityValidatorFallback,
  EntityPeerFallback,
  CorrelationDisabled,
  CorrelationNoBandCap,
  CorrelationBandCap,
  CorrelationDefersToFillBase,
  CorrelationTrimsBand,
  EmptyElection,
  OutcomeFields
}

CandidateHasRecord == 1
CandidateMissingRecord == 2
ConstraintsDisabled == 3
MinSelfBondChecked == 4
MinSelfBondRejected == 5
MinSelfBondAccepted == 6
NominationBondChecked == 7
SelfNominationIgnoredAction == 8
UndersizedNominationRejected == 9
ConcentrationChecked == 10
ZeroTotalRejected == 11
MaxNominatorSelected == 12
ConcentrationAccepted == 13
ConcentrationRejected == 14
CandidateAccepted == 15
CandidateRejected == 16
NumericFullPrecision == 17
ScoreComputedFromSeedAndPeer == 18
TieBreakSortedByScore == 19
TieBreakPeerIdTiebreak == 20
InputOrderIgnored == 21
MaxZeroUsesAllCandidates == 22
MaxValidatorsCapsBase == 23
BaseTakeClampedToCandidateCount == 24
SeatBandCeilApplied == 25
DesiredClampedToCandidateCount == 26
EntityKeyMetadataPreferred == 27
EntityKeyValidatorFallback == 28
EntityKeyPeerFallback == 29
CorrelationDisabledAction == 30
EntityCapUsesDesiredWhenBand == 31
EntityCapUsesBaseWithoutBand == 32
EntityCapFlooredAtOne == 33
CorrelatedCandidateDeferred == 34
DeferredFillsBaseTarget == 35
CorrelatedBandSeatTrimmed == 36
ValidatorSetHashFromSelected == 37
CandidatesTotalFromTieBreak == 38
OutcomeBindsEpochSnapshotSeedParams == 39
EmptyCandidateRejection == 40
SelectionNonemptyNoRejection == 41
ValidatorSetInTieBreakOrder == 42

Actions == 1..42

Accept == {CandidateAccepted}
Reject == {CandidateRejected}
Record == {CandidateHasRecord}
NoRecord == {CandidateMissingRecord}
OrderingActions ==
  {ScoreComputedFromSeedAndPeer, TieBreakSortedByScore,
   TieBreakPeerIdTiebreak, InputOrderIgnored, ValidatorSetInTieBreakOrder}
OutcomeActions ==
  {ValidatorSetHashFromSelected, CandidatesTotalFromTieBreak,
   OutcomeBindsEpochSnapshotSeedParams}

SpecActions(c) ==
  CASE c = CouncilNoRecordAllowed ->
      NoRecord \cup {ConstraintsDisabled} \cup Accept
    [] c = CouncilNoRecordRejected ->
      NoRecord \cup {MinSelfBondChecked} \cup Reject
    [] c = SelfBondPass ->
      Record \cup {MinSelfBondChecked, MinSelfBondAccepted} \cup Accept
    [] c = SelfBondReject ->
      Record \cup {MinSelfBondChecked, MinSelfBondRejected} \cup Reject
    [] c = SelfNominationIgnored ->
      Record \cup {NominationBondChecked, SelfNominationIgnoredAction} \cup Accept
    [] c = UndersizedNominationReject ->
      Record \cup {NominationBondChecked, UndersizedNominationRejected} \cup Reject
    [] c = ConcentrationDisabledAccept ->
      Record \cup Accept
    [] c = ConcentrationZeroTotalReject ->
      Record \cup {ConcentrationChecked, ZeroTotalRejected} \cup Reject
    [] c = ConcentrationNoNominatorAccept ->
      Record \cup {ConcentrationChecked, MaxNominatorSelected,
                   ConcentrationAccepted} \cup Accept
    [] c = ConcentrationBoundaryAccept ->
      Record \cup {ConcentrationChecked, MaxNominatorSelected,
                   ConcentrationAccepted} \cup Accept
    [] c = ConcentrationExceededReject ->
      Record \cup {ConcentrationChecked, MaxNominatorSelected,
                   ConcentrationRejected} \cup Reject
    [] c = NumericPrecisionSelfBond ->
      Record \cup {MinSelfBondChecked, NumericFullPrecision,
                   MinSelfBondAccepted} \cup Accept
    [] c = ElectionOrdering ->
      OrderingActions
    [] c = MaxZeroTakesAll ->
      {MaxZeroUsesAllCandidates, DesiredClampedToCandidateCount,
       SelectionNonemptyNoRejection} \cup OutcomeActions
    [] c = MaxValidatorsCaps ->
      {MaxValidatorsCapsBase, BaseTakeClampedToCandidateCount,
       DesiredClampedToCandidateCount}
    [] c = SeatBandCeil ->
      {MaxValidatorsCapsBase, SeatBandCeilApplied,
       DesiredClampedToCandidateCount}
    [] c = DesiredClamp ->
      {BaseTakeClampedToCandidateCount, SeatBandCeilApplied,
       DesiredClampedToCandidateCount}
    [] c = EntityMetadataPreferred ->
      {EntityKeyMetadataPreferred}
    [] c = EntityValidatorFallback ->
      {EntityKeyValidatorFallback}
    [] c = EntityPeerFallback ->
      {EntityKeyPeerFallback}
    [] c = CorrelationDisabled ->
      {CorrelationDisabledAction}
    [] c = CorrelationNoBandCap ->
      {EntityCapUsesBaseWithoutBand, EntityCapFlooredAtOne}
    [] c = CorrelationBandCap ->
      {EntityCapUsesDesiredWhenBand, EntityCapFlooredAtOne}
    [] c = CorrelationDefersToFillBase ->
      {EntityCapUsesBaseWithoutBand, EntityCapFlooredAtOne,
       CorrelatedCandidateDeferred, DeferredFillsBaseTarget,
       SelectionNonemptyNoRejection}
    [] c = CorrelationTrimsBand ->
      {EntityCapUsesDesiredWhenBand, EntityCapFlooredAtOne,
       CorrelatedBandSeatTrimmed, SelectionNonemptyNoRejection}
    [] c = EmptyElection ->
      {CandidatesTotalFromTieBreak, EmptyCandidateRejection}
    [] c = OutcomeFields ->
      OutcomeActions
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "council_record_required" /\ c = CouncilNoRecordAllowed ->
      (spec \ (Accept \cup {ConstraintsDisabled})) \cup
        {MinSelfBondChecked} \cup Reject
    [] Bug = "council_constraints_ignored" /\ c = CouncilNoRecordRejected ->
      (spec \ Reject) \cup {ConstraintsDisabled} \cup Accept
    [] Bug = "self_bond_not_checked" /\ c \in {SelfBondPass, SelfBondReject} ->
      spec \ {MinSelfBondChecked}
    [] Bug = "self_bond_low_accepted" /\ c = SelfBondReject ->
      (spec \ ({MinSelfBondRejected} \cup Reject)) \cup
        {MinSelfBondAccepted} \cup Accept
    [] Bug = "self_bond_boundary_rejected" /\ c = SelfBondPass ->
      (spec \ ({MinSelfBondAccepted} \cup Accept)) \cup
        {MinSelfBondRejected} \cup Reject
    [] Bug = "self_nomination_counted" /\ c = SelfNominationIgnored ->
      (spec \ ({SelfNominationIgnoredAction} \cup Accept)) \cup
        {UndersizedNominationRejected} \cup Reject
    [] Bug = "undersized_nomination_accepted" /\ c = UndersizedNominationReject ->
      (spec \ ({UndersizedNominationRejected} \cup Reject)) \cup Accept
    [] Bug = "concentration_zero_total_accepted" /\ c = ConcentrationZeroTotalReject ->
      (spec \ ({ZeroTotalRejected} \cup Reject)) \cup
        {ConcentrationAccepted} \cup Accept
    [] Bug = "concentration_disabled_rejects" /\ c = ConcentrationDisabledAccept ->
      (spec \ Accept) \cup {ConcentrationChecked, ConcentrationRejected} \cup Reject
    [] Bug = "concentration_boundary_rejected" /\ c = ConcentrationBoundaryAccept ->
      (spec \ ({ConcentrationAccepted} \cup Accept)) \cup
        {ConcentrationRejected} \cup Reject
    [] Bug = "concentration_exceeded_accepted" /\ c = ConcentrationExceededReject ->
      (spec \ ({ConcentrationRejected} \cup Reject)) \cup
        {ConcentrationAccepted} \cup Accept
    [] Bug = "numeric_truncates_stake" /\ c = NumericPrecisionSelfBond ->
      spec \ {NumericFullPrecision}
    [] Bug = "score_drops_seed" /\ c = ElectionOrdering ->
      spec \ {ScoreComputedFromSeedAndPeer}
    [] Bug = "score_drops_peer" /\ c = ElectionOrdering ->
      spec \ {ScoreComputedFromSeedAndPeer, TieBreakPeerIdTiebreak}
    [] Bug = "input_order_kept" /\ c = ElectionOrdering ->
      spec \ {InputOrderIgnored}
    [] Bug = "peer_tie_ignored" /\ c = ElectionOrdering ->
      spec \ {TieBreakPeerIdTiebreak}
    [] Bug = "max_zero_selects_none" /\ c = MaxZeroTakesAll ->
      (spec \ ({MaxZeroUsesAllCandidates, SelectionNonemptyNoRejection}
               \cup OutcomeActions)) \cup {EmptyCandidateRejection}
    [] Bug = "max_validators_ignored" /\ c = MaxValidatorsCaps ->
      (spec \ {MaxValidatorsCapsBase}) \cup {MaxZeroUsesAllCandidates}
    [] Bug = "base_take_not_clamped" /\ c = MaxValidatorsCaps ->
      spec \ {BaseTakeClampedToCandidateCount}
    [] Bug = "seat_band_floor" /\ c = SeatBandCeil ->
      spec \ {SeatBandCeilApplied}
    [] Bug = "desired_not_clamped" /\ c = DesiredClamp ->
      spec \ {DesiredClampedToCandidateCount}
    [] Bug = "entity_metadata_ignored" /\ c = EntityMetadataPreferred ->
      {EntityKeyValidatorFallback}
    [] Bug = "entity_validator_fallback_skipped" /\ c = EntityValidatorFallback ->
      {EntityKeyPeerFallback}
    [] Bug = "entity_peer_fallback_skipped" /\ c = EntityPeerFallback ->
      {EntityKeyValidatorFallback}
    [] Bug = "correlation_disabled_caps" /\ c = CorrelationDisabled ->
      {EntityCapUsesBaseWithoutBand, EntityCapFlooredAtOne}
    [] Bug = "correlation_cap_uses_base_with_band" /\ c = CorrelationBandCap ->
      (spec \ {EntityCapUsesDesiredWhenBand}) \cup {EntityCapUsesBaseWithoutBand}
    [] Bug = "correlation_cap_uses_desired_without_band" /\ c = CorrelationNoBandCap ->
      (spec \ {EntityCapUsesBaseWithoutBand}) \cup {EntityCapUsesDesiredWhenBand}
    [] Bug = "correlation_cap_zero" /\
          c \in {CorrelationNoBandCap, CorrelationBandCap} ->
      spec \ {EntityCapFlooredAtOne}
    [] Bug = "correlated_candidate_not_deferred" /\ c = CorrelationDefersToFillBase ->
      spec \ {CorrelatedCandidateDeferred, DeferredFillsBaseTarget}
    [] Bug = "deferred_does_not_fill_base" /\ c = CorrelationDefersToFillBase ->
      (spec \ {DeferredFillsBaseTarget, SelectionNonemptyNoRejection}) \cup
        {CorrelatedBandSeatTrimmed, EmptyCandidateRejection}
    [] Bug = "correlated_band_not_trimmed" /\ c = CorrelationTrimsBand ->
      (spec \ {CorrelatedBandSeatTrimmed}) \cup {CorrelatedCandidateDeferred,
                                                 DeferredFillsBaseTarget}
    [] Bug = "empty_election_no_rejection" /\ c = EmptyElection ->
      (spec \ {EmptyCandidateRejection}) \cup {SelectionNonemptyNoRejection}
    [] Bug = "nonempty_selection_rejected" /\ c = MaxZeroTakesAll ->
      (spec \ {SelectionNonemptyNoRejection}) \cup {EmptyCandidateRejection}
    [] Bug = "validator_set_hash_uses_tie_break" /\ c = OutcomeFields ->
      (spec \ {ValidatorSetHashFromSelected}) \cup {TieBreakSortedByScore}
    [] Bug = "candidates_total_uses_selected" /\ c = OutcomeFields ->
      spec \ {CandidatesTotalFromTieBreak}
    [] Bug = "outcome_drops_epoch" /\ c = OutcomeFields ->
      spec \ {OutcomeBindsEpochSnapshotSeedParams}
    [] OTHER -> spec

Bugs == {
  "none",
  "council_record_required",
  "council_constraints_ignored",
  "self_bond_not_checked",
  "self_bond_low_accepted",
  "self_bond_boundary_rejected",
  "self_nomination_counted",
  "undersized_nomination_accepted",
  "concentration_zero_total_accepted",
  "concentration_disabled_rejects",
  "concentration_boundary_rejected",
  "concentration_exceeded_accepted",
  "numeric_truncates_stake",
  "score_drops_seed",
  "score_drops_peer",
  "input_order_kept",
  "peer_tie_ignored",
  "max_zero_selects_none",
  "max_validators_ignored",
  "base_take_not_clamped",
  "seat_band_floor",
  "desired_not_clamped",
  "entity_metadata_ignored",
  "entity_validator_fallback_skipped",
  "entity_peer_fallback_skipped",
  "correlation_disabled_caps",
  "correlation_cap_uses_base_with_band",
  "correlation_cap_uses_desired_without_band",
  "correlation_cap_zero",
  "correlated_candidate_not_deferred",
  "deferred_does_not_fill_base",
  "correlated_band_not_trimmed",
  "empty_election_no_rejection",
  "nonempty_selection_rejected",
  "validator_set_hash_uses_tie_break",
  "candidates_total_uses_selected",
  "outcome_drops_epoch"
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

NoBugInvariant ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

Safety == NoBugInvariant

====

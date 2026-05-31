---- MODULE SumeragiConsensusPenaltyActionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi consensus evidence penalty actions.

This slice captures the stateful contract around
`derive_consensus_penalty_actions(...)` and the consensus-evidence branches of
`apply_npos_consensus_effects_to_transaction(...)`. It builds on the offender
selection model and pins when stored evidence is eligible to mutate chain state:
already-applied or cancelled evidence is ignored, slashing delay is inclusive at
the boundary, missing roster/seed/validator/slash amount leaves evidence
pending, legitimate empty invalid-QC evidence is marked without slash, and
actual slashes are paired with an applied marker that binds the evidence key and
current height. Applying actions mutates only the intended transaction state and
outcome counters.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

AlreadyApplied == "already_applied"
Cancelled == "cancelled"
DelayPending == "delay_pending"
DelayBoundary == "delay_boundary"
NoRoster == "no_roster"
NposMissingSeed == "npos_missing_seed"
UnmappedOffender == "unmapped_offender"
MissingSlashAmount == "missing_slash_amount"
SlashOne == "slash_one"
SlashTwo == "slash_two"
LegitEmptyInvalidQc == "legit_empty_invalid_qc"
NonlegitEmptyDoubleVote == "nonlegit_empty_double_vote"
EmptyCensorship == "empty_censorship"
CensorshipSlash == "censorship_slash"
EffectsSortDedup == "effects_sort_dedup"
ApplyConsensusSlash == "apply_consensus_slash"
ApplyMarkEvidence == "apply_mark_evidence"
ApplyMarkMissing == "apply_mark_missing"

Cases == {
  AlreadyApplied,
  Cancelled,
  DelayPending,
  DelayBoundary,
  NoRoster,
  NposMissingSeed,
  UnmappedOffender,
  MissingSlashAmount,
  SlashOne,
  SlashTwo,
  LegitEmptyInvalidQc,
  NonlegitEmptyDoubleVote,
  EmptyCensorship,
  CensorshipSlash,
  EffectsSortDedup,
  ApplyConsensusSlash,
  ApplyMarkEvidence,
  ApplyMarkMissing
}

FilterAlreadyApplied == 1
FilterCancelled == 2
CheckSlashingDelay == 3
SkipNotDue == 4
DelayBoundaryDue == 5
ResolveModeAndEpoch == 6
RequireRoster == 7
SkipMissingRoster == 8
RequireNposSeed == 9
SkipMissingSeed == 10
ResolveOffenders == 11
LocateOffender == 12
SkipUnmappedOffender == 13
ResolveSlashAmount == 14
SkipMissingSlashAmount == 15
EmitConsensusSlash == 16
EmitTwoConsensusSlashes == 17
EmitMarkApplied == 18
MarkCurrentHeight == 19
SlashIdBindsEvidenceKey == 20
LegitEmptyInvalidQcMarks == 21
EmptyNonlegitSkips == 22
EmptyCensorshipSkips == 23
SortPenaltyActions == 24
DedupPenaltyActions == 25
ApplySlashTransaction == 26
OutcomeAppliedSaturatingInc == 27
OutcomeSlashedSaturatingInc == 28
OutcomeWrappingInc == 29
ApplyMarkExistingOnly == 30
SetPenaltyApplied == 31
SetPenaltyAppliedHeight == 32
MissingMarkNoop == 33
NoActions == 34

Actions == 1..34

SlashBase ==
  {ResolveModeAndEpoch, RequireRoster, ResolveOffenders, LocateOffender,
   ResolveSlashAmount, EmitConsensusSlash, EmitMarkApplied,
   MarkCurrentHeight, SlashIdBindsEvidenceKey}

PendingNoActionBase ==
  {ResolveModeAndEpoch, RequireRoster, ResolveOffenders, NoActions}

SpecActions(c) ==
  CASE c = AlreadyApplied ->
      {FilterAlreadyApplied, NoActions}
    [] c = Cancelled ->
      {FilterCancelled, NoActions}
    [] c = DelayPending ->
      {CheckSlashingDelay, SkipNotDue, NoActions}
    [] c = DelayBoundary ->
      {CheckSlashingDelay, DelayBoundaryDue} \cup SlashBase
    [] c = NoRoster ->
      {ResolveModeAndEpoch, RequireRoster, SkipMissingRoster, NoActions}
    [] c = NposMissingSeed ->
      {ResolveModeAndEpoch, RequireRoster, RequireNposSeed,
       SkipMissingSeed, NoActions}
    [] c = UnmappedOffender ->
      {ResolveModeAndEpoch, RequireRoster, ResolveOffenders,
       LocateOffender, SkipUnmappedOffender, NoActions}
    [] c = MissingSlashAmount ->
      {ResolveModeAndEpoch, RequireRoster, ResolveOffenders,
       LocateOffender, ResolveSlashAmount, SkipMissingSlashAmount,
       NoActions}
    [] c = SlashOne ->
      SlashBase
    [] c = SlashTwo ->
      (SlashBase \ {EmitConsensusSlash}) \cup {EmitTwoConsensusSlashes}
    [] c = LegitEmptyInvalidQc ->
      {ResolveModeAndEpoch, RequireRoster, ResolveOffenders,
       LegitEmptyInvalidQcMarks, EmitMarkApplied, MarkCurrentHeight}
    [] c = NonlegitEmptyDoubleVote ->
      PendingNoActionBase \cup {EmptyNonlegitSkips}
    [] c = EmptyCensorship ->
      PendingNoActionBase \cup {EmptyCensorshipSkips}
    [] c = CensorshipSlash ->
      SlashBase
    [] c = EffectsSortDedup ->
      {EmitConsensusSlash, EmitMarkApplied, SortPenaltyActions,
       DedupPenaltyActions}
    [] c = ApplyConsensusSlash ->
      {ApplySlashTransaction, OutcomeAppliedSaturatingInc,
       OutcomeSlashedSaturatingInc}
    [] c = ApplyMarkEvidence ->
      {ApplyMarkExistingOnly, SetPenaltyApplied, SetPenaltyAppliedHeight,
       MarkCurrentHeight}
    [] c = ApplyMarkMissing ->
      {ApplyMarkExistingOnly, MissingMarkNoop}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "already_applied_processed"
       /\ c = AlreadyApplied ->
      (spec \ {FilterAlreadyApplied, NoActions}) \cup SlashBase
    [] Bug = "cancelled_processed"
       /\ c = Cancelled ->
      (spec \ {FilterCancelled, NoActions}) \cup {EmitMarkApplied}
    [] Bug = "delay_pending_processed"
       /\ c = DelayPending ->
      (spec \ {SkipNotDue, NoActions}) \cup SlashBase
    [] Bug = "delay_boundary_blocked"
       /\ c = DelayBoundary ->
      (spec \ {DelayBoundaryDue, EmitConsensusSlash, EmitMarkApplied}) \cup
        {SkipNotDue, NoActions}
    [] Bug = "missing_roster_marks"
       /\ c = NoRoster ->
      (spec \ {SkipMissingRoster, NoActions}) \cup {EmitMarkApplied}
    [] Bug = "npos_missing_seed_falls_back"
       /\ c = NposMissingSeed ->
      (spec \ {RequireNposSeed, SkipMissingSeed, NoActions}) \cup SlashBase
    [] Bug = "unmapped_offender_marks"
       /\ c = UnmappedOffender ->
      (spec \ {SkipUnmappedOffender, NoActions}) \cup {EmitMarkApplied}
    [] Bug = "missing_slash_amount_marks"
       /\ c = MissingSlashAmount ->
      (spec \ {SkipMissingSlashAmount, NoActions}) \cup {EmitMarkApplied}
    [] Bug = "slash_omits_mark"
       /\ c = SlashOne ->
      spec \ {EmitMarkApplied}
    [] Bug = "slash_omits_slash"
       /\ c = SlashOne ->
      spec \ {EmitConsensusSlash}
    [] Bug = "slash_id_omits_key"
       /\ c = SlashOne ->
      spec \ {SlashIdBindsEvidenceKey}
    [] Bug = "slash_mark_uses_record_height"
       /\ c = SlashOne ->
      spec \ {MarkCurrentHeight}
    [] Bug = "two_slashes_collapsed"
       /\ c = SlashTwo ->
      (spec \ {EmitTwoConsensusSlashes}) \cup {EmitConsensusSlash}
    [] Bug = "empty_invalid_qc_not_marked"
       /\ c = LegitEmptyInvalidQc ->
      (spec \ {LegitEmptyInvalidQcMarks, EmitMarkApplied}) \cup {NoActions}
    [] Bug = "empty_double_vote_marked"
       /\ c = NonlegitEmptyDoubleVote ->
      (spec \ {EmptyNonlegitSkips, NoActions}) \cup {EmitMarkApplied}
    [] Bug = "empty_censorship_marked"
       /\ c = EmptyCensorship ->
      (spec \ {EmptyCensorshipSkips, NoActions}) \cup {EmitMarkApplied}
    [] Bug = "censorship_slash_omits_mark"
       /\ c = CensorshipSlash ->
      spec \ {EmitMarkApplied}
    [] Bug = "effects_not_sorted"
       /\ c = EffectsSortDedup ->
      spec \ {SortPenaltyActions}
    [] Bug = "effects_not_deduped"
       /\ c = EffectsSortDedup ->
      spec \ {DedupPenaltyActions}
    [] Bug = "apply_slash_skips_transaction"
       /\ c = ApplyConsensusSlash ->
      spec \ {ApplySlashTransaction}
    [] Bug = "apply_slash_skips_applied_counter"
       /\ c = ApplyConsensusSlash ->
      spec \ {OutcomeAppliedSaturatingInc}
    [] Bug = "apply_slash_skips_slashed_counter"
       /\ c = ApplyConsensusSlash ->
      spec \ {OutcomeSlashedSaturatingInc}
    [] Bug = "apply_slash_wraps_counter"
       /\ c = ApplyConsensusSlash ->
      (spec \ {OutcomeAppliedSaturatingInc}) \cup {OutcomeWrappingInc}
    [] Bug = "mark_skips_existing_lookup"
       /\ c = ApplyMarkEvidence ->
      spec \ {ApplyMarkExistingOnly}
    [] Bug = "mark_skips_flag"
       /\ c = ApplyMarkEvidence ->
      spec \ {SetPenaltyApplied}
    [] Bug = "mark_skips_height"
       /\ c = ApplyMarkEvidence ->
      spec \ {SetPenaltyAppliedHeight, MarkCurrentHeight}
    [] Bug = "mark_missing_inserts_record"
       /\ c = ApplyMarkMissing ->
      (spec \ {MissingMarkNoop}) \cup {SetPenaltyApplied}
    [] Bug = "mark_increments_outcome"
       /\ c = ApplyMarkEvidence ->
      spec \cup {OutcomeAppliedSaturatingInc}
    [] OTHER -> spec

Bugs == {
  "none",
  "already_applied_processed",
  "cancelled_processed",
  "delay_pending_processed",
  "delay_boundary_blocked",
  "missing_roster_marks",
  "npos_missing_seed_falls_back",
  "unmapped_offender_marks",
  "missing_slash_amount_marks",
  "slash_omits_mark",
  "slash_omits_slash",
  "slash_id_omits_key",
  "slash_mark_uses_record_height",
  "two_slashes_collapsed",
  "empty_invalid_qc_not_marked",
  "empty_double_vote_marked",
  "empty_censorship_marked",
  "censorship_slash_omits_mark",
  "effects_not_sorted",
  "effects_not_deduped",
  "apply_slash_skips_transaction",
  "apply_slash_skips_applied_counter",
  "apply_slash_skips_slashed_counter",
  "apply_slash_wraps_counter",
  "mark_skips_existing_lookup",
  "mark_skips_flag",
  "mark_skips_height",
  "mark_missing_inserts_record",
  "mark_increments_outcome"
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

SafetyFast == NoBugInvariant

EligibilityActionsMatchSpec ==
  \A c \in {
    AlreadyApplied,
    Cancelled,
    DelayPending,
    DelayBoundary,
    NoRoster,
    NposMissingSeed,
    UnmappedOffender,
    MissingSlashAmount
  }:
    ImplementationActions(c) = SpecActions(c)

SlashDerivationActionsMatchSpec ==
  \A c \in {
    SlashOne,
    SlashTwo,
    LegitEmptyInvalidQc,
    NonlegitEmptyDoubleVote,
    EmptyCensorship,
    CensorshipSlash,
    EffectsSortDedup
  }:
    ImplementationActions(c) = SpecActions(c)

ApplyActionsMatchSpec ==
  \A c \in {
    ApplyConsensusSlash,
    ApplyMarkEvidence,
    ApplyMarkMissing
  }:
    ImplementationActions(c) = SpecActions(c)

EligibilityAnchors ==
  /\ FilterAlreadyApplied \in ImplementationActions(AlreadyApplied)
  /\ FilterCancelled \in ImplementationActions(Cancelled)
  /\ SkipNotDue \in ImplementationActions(DelayPending)
  /\ DelayBoundaryDue \in ImplementationActions(DelayBoundary)
  /\ SkipMissingRoster \in ImplementationActions(NoRoster)
  /\ SkipMissingSeed \in ImplementationActions(NposMissingSeed)
  /\ SkipUnmappedOffender \in ImplementationActions(UnmappedOffender)
  /\ SkipMissingSlashAmount \in ImplementationActions(MissingSlashAmount)

SlashAndMarkerAnchors ==
  /\ EmitConsensusSlash \in ImplementationActions(SlashOne)
  /\ EmitMarkApplied \in ImplementationActions(SlashOne)
  /\ MarkCurrentHeight \in ImplementationActions(SlashOne)
  /\ SlashIdBindsEvidenceKey \in ImplementationActions(SlashOne)
  /\ EmitTwoConsensusSlashes \in ImplementationActions(SlashTwo)
  /\ LegitEmptyInvalidQcMarks \in ImplementationActions(LegitEmptyInvalidQc)
  /\ EmptyNonlegitSkips \in ImplementationActions(NonlegitEmptyDoubleVote)
  /\ EmptyCensorshipSkips \in ImplementationActions(EmptyCensorship)
  /\ EmitMarkApplied \in ImplementationActions(CensorshipSlash)

EffectOrderingAnchors ==
  /\ SortPenaltyActions \in ImplementationActions(EffectsSortDedup)
  /\ DedupPenaltyActions \in ImplementationActions(EffectsSortDedup)

ApplyMutationAnchors ==
  /\ ApplySlashTransaction \in ImplementationActions(ApplyConsensusSlash)
  /\ OutcomeAppliedSaturatingInc \in
       ImplementationActions(ApplyConsensusSlash)
  /\ OutcomeSlashedSaturatingInc \in
       ImplementationActions(ApplyConsensusSlash)
  /\ ApplyMarkExistingOnly \in ImplementationActions(ApplyMarkEvidence)
  /\ SetPenaltyApplied \in ImplementationActions(ApplyMarkEvidence)
  /\ SetPenaltyAppliedHeight \in ImplementationActions(ApplyMarkEvidence)
  /\ MissingMarkNoop \in ImplementationActions(ApplyMarkMissing)

ConsensusPenaltyActionSafetyAnchors ==
  /\ EligibilityActionsMatchSpec
  /\ SlashDerivationActionsMatchSpec
  /\ ApplyActionsMatchSpec
  /\ EligibilityAnchors
  /\ SlashAndMarkerAnchors
  /\ EffectOrderingAnchors
  /\ ApplyMutationAnchors

====

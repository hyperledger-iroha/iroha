---- MODULE SumeragiActiveTopologySelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for active validator topology selection.

This slice captures the deterministic branch contract of
`derive_active_topology_from_views(...)` in `main_loop/roster.rs`. The model
abstracts peers, BLS identity checks, and proof-of-possession validation into a
finite set of representative cases. It pins the observable consensus contract:
commit topology is preferred over world peers, world peers are preferred over
trusted fallback, all outputs are deduplicated/canonicalized after BLS filtering,
primary commit/world rosters are not PoP-filtered when the PoP map is
incomplete, trusted-derived rosters do drop missing PoPs, complete PoP filters
must preserve quorum or fall back to the baseline, and an empty primary result
falls back through trusted validators rather than synthesizing peers.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommitWins == "commit_wins"
WorldFallback == "world_fallback"
TrustedFallbackEmpty == "trusted_fallback_empty"
CommitBlsFilter == "commit_bls_filter"
CommitMissingPopsSkip == "commit_missing_pops_skip"
TrustedMissingPopsDrop == "trusted_missing_pops_drop"
PopFilterSubquorumFallback == "pop_filter_subquorum_fallback"
PopFilterQuorumAccepted == "pop_filter_quorum_accepted"
PrimaryEmptyUsesTrusted == "primary_empty_uses_trusted"
TrustedFallbackPopsFiltered == "trusted_fallback_pops_filtered"
EmptyEverywhere == "empty_everywhere"
SmallRosterNeedsAllPops == "small_roster_needs_all_pops"
LargeRosterQuorumThreshold == "large_roster_quorum_threshold"

Candidates == {
  CommitWins,
  WorldFallback,
  TrustedFallbackEmpty,
  CommitBlsFilter,
  CommitMissingPopsSkip,
  TrustedMissingPopsDrop,
  PopFilterSubquorumFallback,
  PopFilterQuorumAccepted,
  PrimaryEmptyUsesTrusted,
  TrustedFallbackPopsFiltered,
  EmptyEverywhere,
  SmallRosterNeedsAllPops,
  LargeRosterQuorumThreshold
}

SourcePriorityCases == {
  CommitWins,
  WorldFallback,
  TrustedFallbackEmpty
}

OutputNormalizationCases == Candidates \ {EmptyEverywhere}

PopFilterCases == {
  CommitMissingPopsSkip,
  TrustedMissingPopsDrop,
  PopFilterSubquorumFallback,
  PopFilterQuorumAccepted,
  SmallRosterNeedsAllPops,
  LargeRosterQuorumThreshold
}

FallbackCases == {
  PrimaryEmptyUsesTrusted,
  TrustedFallbackPopsFiltered,
  EmptyEverywhere
}

UseCommitBaseline == 1
UseWorldBaseline == 2
UseTrustedBaseline == 3
BlsFiltered == 4
Deduped == 5
CanonicalSorted == 6
PrimaryMissingPopsSkipFilter == 7
TrustedMissingPopsFiltered == 8
CompletePopsFiltered == 9
SubquorumPreservesBaseline == 10
QuorumFilteredAccepted == 11
FinalTrustedFallback == 12
FallbackPopsFiltered == 13
ReturnEmpty == 14
ReturnNonEmpty == 15
SmallRosterRequiresAll == 16
LargeRosterTwoThirdsPlusOne == 17

Actions == 1..17

BaseOutputActions == {BlsFiltered, Deduped, CanonicalSorted, ReturnNonEmpty}

SpecActions(c) ==
  CASE c = CommitWins ->
      BaseOutputActions \cup {UseCommitBaseline}
    [] c = WorldFallback ->
      BaseOutputActions \cup {UseWorldBaseline}
    [] c = TrustedFallbackEmpty ->
      BaseOutputActions \cup {UseTrustedBaseline}
    [] c = CommitBlsFilter ->
      BaseOutputActions \cup {UseCommitBaseline}
    [] c = CommitMissingPopsSkip ->
      BaseOutputActions \cup {UseCommitBaseline, PrimaryMissingPopsSkipFilter}
    [] c = TrustedMissingPopsDrop ->
      BaseOutputActions \cup {UseTrustedBaseline, TrustedMissingPopsFiltered}
    [] c = PopFilterSubquorumFallback ->
      BaseOutputActions \cup {UseCommitBaseline, CompletePopsFiltered,
        SubquorumPreservesBaseline, LargeRosterTwoThirdsPlusOne}
    [] c = PopFilterQuorumAccepted ->
      BaseOutputActions \cup {UseCommitBaseline, CompletePopsFiltered,
        QuorumFilteredAccepted, LargeRosterTwoThirdsPlusOne}
    [] c = PrimaryEmptyUsesTrusted ->
      BaseOutputActions \cup {UseCommitBaseline, FinalTrustedFallback}
    [] c = TrustedFallbackPopsFiltered ->
      BaseOutputActions \cup {UseCommitBaseline, FinalTrustedFallback,
        FallbackPopsFiltered}
    [] c = EmptyEverywhere ->
      {UseTrustedBaseline, BlsFiltered, Deduped, CanonicalSorted, ReturnEmpty}
    [] c = SmallRosterNeedsAllPops ->
      BaseOutputActions \cup {UseWorldBaseline, CompletePopsFiltered,
        SubquorumPreservesBaseline, SmallRosterRequiresAll}
    [] c = LargeRosterQuorumThreshold ->
      BaseOutputActions \cup {UseWorldBaseline, CompletePopsFiltered,
        QuorumFilteredAccepted, LargeRosterTwoThirdsPlusOne}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE c = CommitWins /\ Bug = "commit_priority_ignored" ->
      (spec \ {UseCommitBaseline}) \cup {UseWorldBaseline}
    [] c = WorldFallback /\ Bug = "world_fallback_ignored" ->
      (spec \ {UseWorldBaseline}) \cup {UseTrustedBaseline}
    [] c = TrustedFallbackEmpty /\ Bug = "trusted_fallback_missing" ->
      (spec \ {UseTrustedBaseline, ReturnNonEmpty}) \cup {ReturnEmpty}
    [] c = CommitBlsFilter /\ Bug = "bls_filter_skipped" ->
      spec \ {BlsFiltered}
    [] c = CommitWins /\ Bug = "dedup_skipped" ->
      spec \ {Deduped}
    [] c = WorldFallback /\ Bug = "canonical_sort_skipped" ->
      spec \ {CanonicalSorted}
    [] c = CommitMissingPopsSkip /\ Bug = "missing_pops_filters_commit" ->
      (spec \ {PrimaryMissingPopsSkipFilter}) \cup {TrustedMissingPopsFiltered}
    [] c = TrustedMissingPopsDrop /\ Bug = "trusted_missing_pops_kept" ->
      spec \ {TrustedMissingPopsFiltered}
    [] c = PopFilterSubquorumFallback /\
          Bug = "subquorum_pop_filter_applied" ->
      (spec \ {SubquorumPreservesBaseline}) \cup {QuorumFilteredAccepted}
    [] c = PopFilterQuorumAccepted /\ Bug = "quorum_pop_filter_rejected" ->
      (spec \ {QuorumFilteredAccepted}) \cup {SubquorumPreservesBaseline}
    [] c = PrimaryEmptyUsesTrusted /\ Bug = "final_fallback_skipped" ->
      (spec \ {FinalTrustedFallback, ReturnNonEmpty}) \cup {ReturnEmpty}
    [] c = TrustedFallbackPopsFiltered /\ Bug = "fallback_pops_ignored" ->
      spec \ {FallbackPopsFiltered}
    [] c = SmallRosterNeedsAllPops /\
          Bug = "small_roster_threshold_too_low" ->
      (spec \ {SmallRosterRequiresAll, SubquorumPreservesBaseline}) \cup
        {QuorumFilteredAccepted}
    [] c = LargeRosterQuorumThreshold /\
          Bug = "large_roster_threshold_too_high" ->
      (spec \ {LargeRosterTwoThirdsPlusOne, QuorumFilteredAccepted}) \cup
        {SubquorumPreservesBaseline}
    [] c = EmptyEverywhere /\ Bug = "empty_sources_synthesize_peer" ->
      (spec \ {ReturnEmpty}) \cup {ReturnNonEmpty}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked \in 0..0
  /\ Bug \in {
       "none",
       "commit_priority_ignored",
       "world_fallback_ignored",
       "trusted_fallback_missing",
       "bls_filter_skipped",
       "dedup_skipped",
       "canonical_sort_skipped",
       "missing_pops_filters_commit",
       "trusted_missing_pops_kept",
       "subquorum_pop_filter_applied",
       "quorum_pop_filter_rejected",
       "final_fallback_skipped",
       "fallback_pops_ignored",
       "small_roster_threshold_too_low",
       "large_roster_threshold_too_high",
       "empty_sources_synthesize_peer"
     }
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

AllCasesMatchSpec ==
  \A c \in Candidates: ImplementationActions(c) = SpecActions(c)

PrimarySourcePriority ==
  /\ UseCommitBaseline \in ImplementationActions(CommitWins)
  /\ UseWorldBaseline \notin ImplementationActions(CommitWins)
  /\ UseWorldBaseline \in ImplementationActions(WorldFallback)
  /\ UseTrustedBaseline \notin ImplementationActions(WorldFallback)

PopFilteringContract ==
  /\ PrimaryMissingPopsSkipFilter \in
       ImplementationActions(CommitMissingPopsSkip)
  /\ TrustedMissingPopsFiltered \in
       ImplementationActions(TrustedMissingPopsDrop)
  /\ SubquorumPreservesBaseline \in
       ImplementationActions(PopFilterSubquorumFallback)
  /\ QuorumFilteredAccepted \in
       ImplementationActions(PopFilterQuorumAccepted)
  /\ SmallRosterRequiresAll \in
       ImplementationActions(SmallRosterNeedsAllPops)
  /\ LargeRosterTwoThirdsPlusOne \in
       ImplementationActions(LargeRosterQuorumThreshold)

FallbackContract ==
  /\ FinalTrustedFallback \in ImplementationActions(PrimaryEmptyUsesTrusted)
  /\ FallbackPopsFiltered \in
       ImplementationActions(TrustedFallbackPopsFiltered)
  /\ ReturnEmpty \in ImplementationActions(EmptyEverywhere)
  /\ ReturnNonEmpty \notin ImplementationActions(EmptyEverywhere)

OutputShapeContract ==
  \A c \in Candidates \ {EmptyEverywhere}:
    /\ BlsFiltered \in ImplementationActions(c)
    /\ Deduped \in ImplementationActions(c)
    /\ CanonicalSorted \in ImplementationActions(c)
    /\ ReturnNonEmpty \in ImplementationActions(c)

SafetyFast ==
  /\ AllCasesMatchSpec
  /\ PrimarySourcePriority
  /\ PopFilteringContract
  /\ FallbackContract
  /\ OutputShapeContract

ActiveTopologySourcePriorityExact ==
  /\ \A c \in SourcePriorityCases:
       ImplementationActions(c) = SpecActions(c)
  /\ PrimarySourcePriority

ActiveTopologyOutputNormalizationExact ==
  /\ \A c \in OutputNormalizationCases:
       ImplementationActions(c) = SpecActions(c)
  /\ OutputShapeContract

ActiveTopologyPopFilterExact ==
  /\ \A c \in PopFilterCases:
       ImplementationActions(c) = SpecActions(c)
  /\ PopFilteringContract

ActiveTopologyFallbackExact ==
  /\ \A c \in FallbackCases:
       ImplementationActions(c) = SpecActions(c)
  /\ FallbackContract

ActiveTopologySelectionExactness ==
  /\ SafetyFast
  /\ AllCasesMatchSpec
  /\ ActiveTopologySourcePriorityExact
  /\ ActiveTopologyOutputNormalizationExact
  /\ ActiveTopologyPopFilterExact
  /\ ActiveTopologyFallbackExact

ActiveTopologySelectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ActiveTopologySelectionExactness

====

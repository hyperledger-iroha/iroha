---- MODULE SumeragiRbcRs16InitialFanoutGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `rs16_initial_chunk_indices_for_target(...)`.

The helper decides whether RS16 RBC should send all chunks to a target or a
deterministic reduced subset. For reduced fanout, the subset must stay sorted,
deduplicated, in range, and large enough per stripe for RS16 reconstruction.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  returns_some,
  \* @type: Int;
  required_count,
  \* @type: Int;
  selected_len,
  \* @type: Int;
  min_selected_per_stripe,
  \* @type: Int;
  max_selected_per_stripe,
  \* @type: Bool;
  has_out_of_range,
  \* @type: Bool;
  has_duplicates,
  \* @type: Bool;
  sorted,
  \* @type: Bool;
  covers_all_stripes

\* @type: <<Str, Bool, Int, Int, Int, Int, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate, returns_some, required_count, selected_len,
    min_selected_per_stripe, max_selected_per_stripe, has_out_of_range,
    has_duplicates, sorted, covers_all_stripes>>

Cases == {
  "full_fanout_rs16",
  "plain_data_fanout",
  "rs16_zero_data",
  "rs16_data_single",
  "rs16_data_plus_one_single",
  "rs16_data_multi",
  "rs16_data_plus_one_multi",
  "rs16_single_shard_plus_one",
  "rs16_data_plus_one_no_parity"
}

CountValues == 0..64

Fanout(c) ==
  CASE c = "full_fanout_rs16" -> "Full"
    [] c \in {
         "rs16_data_plus_one_single",
         "rs16_data_plus_one_multi",
         "rs16_single_shard_plus_one",
         "rs16_data_plus_one_no_parity"
       } -> "DataPlusOne"
    [] OTHER -> "Data"

IsRs16(c) ==
  c # "plain_data_fanout"

DataShards(c) ==
  CASE c = "rs16_zero_data" -> 0
    [] c = "rs16_single_shard_plus_one" -> 1
    [] c = "rs16_data_plus_one_no_parity" -> 2
    [] OTHER -> 4

ParityShards(c) ==
  CASE c = "rs16_single_shard_plus_one" -> 1
    [] c = "rs16_data_plus_one_no_parity" -> 0
    [] c = "plain_data_fanout" -> 0
    [] OTHER -> 2

StripeCount(c) ==
  CASE c \in {"rs16_data_multi", "rs16_data_plus_one_multi"} -> 3
    [] c = "rs16_data_plus_one_no_parity" -> 2
    [] OTHER -> 1

StripeWidth(c) ==
  IF IsRs16(c)
  THEN DataShards(c) + ParityShards(c)
  ELSE 1

TotalChunks(c) ==
  StripeCount(c) * StripeWidth(c)

RequestedRequired(c) ==
  CASE Fanout(c) = "Full" -> 0
    [] Fanout(c) = "Data" -> DataShards(c)
    [] Fanout(c) = "DataPlusOne" -> DataShards(c) + 1

Min(a, b) ==
  IF a <= b THEN a ELSE b

SpecReturnsSome(c) ==
  /\ Fanout(c) # "Full"
  /\ IsRs16(c)
  /\ RequestedRequired(c) # 0

SpecRequired(c) ==
  IF SpecReturnsSome(c)
  THEN Min(RequestedRequired(c), StripeWidth(c))
  ELSE 0

SpecSelectedLen(c) ==
  IF SpecReturnsSome(c)
  THEN StripeCount(c) * SpecRequired(c)
  ELSE 0

SpecMinSelectedPerStripe(c) ==
  IF SpecReturnsSome(c) THEN SpecRequired(c) ELSE 0

SpecMaxSelectedPerStripe(c) ==
  SpecMinSelectedPerStripe(c)

SpecCoversAllStripes(c) ==
  SpecReturnsSome(c)

ActualReturnsSome(c) ==
  CASE Bug = "full_returns_some" /\ c = "full_fanout_rs16" -> TRUE
    [] Bug = "plain_returns_some" /\ c = "plain_data_fanout" -> TRUE
    [] Bug = "zero_required_returns_some" /\ c = "rs16_zero_data" -> TRUE
    [] OTHER -> SpecReturnsSome(c)

ActualRequired(c) ==
  CASE Bug = "full_returns_some" /\ c = "full_fanout_rs16" -> StripeWidth(c)
    [] Bug = "plain_returns_some" /\ c = "plain_data_fanout" -> 1
    [] Bug = "zero_required_returns_some" /\ c = "rs16_zero_data" -> 1
    [] Bug = "data_uses_data_plus_one" /\ c = "rs16_data_multi" ->
         DataShards(c) + 1
    [] Bug = "data_plus_one_omits_extra" /\ c = "rs16_data_plus_one_multi" ->
         DataShards(c)
    [] Bug = "skip_width_clamp" /\ c = "rs16_data_plus_one_no_parity" ->
         RequestedRequired(c)
    [] OTHER -> SpecRequired(c)

ActualSelectedLen(c) ==
  CASE Bug = "full_returns_some" /\ c = "full_fanout_rs16" -> TotalChunks(c)
    [] Bug = "plain_returns_some" /\ c = "plain_data_fanout" -> 1
    [] Bug = "zero_required_returns_some" /\ c = "rs16_zero_data" -> 1
    [] Bug = "wrong_stripe_count" /\ c = "rs16_data_plus_one_multi" ->
         (StripeCount(c) - 1) * SpecRequired(c)
    [] Bug = "skip_width_clamp" /\ c = "rs16_data_plus_one_no_parity" ->
         StripeCount(c) * RequestedRequired(c)
    [] OTHER ->
         IF ActualReturnsSome(c)
         THEN StripeCount(c) * ActualRequired(c)
         ELSE 0

ActualMinSelectedPerStripe(c) ==
  CASE Bug = "wrong_stripe_count" /\ c = "rs16_data_plus_one_multi" -> 0
    [] Bug = "missing_stripe" /\ c = "rs16_data_multi" -> 0
    [] Bug = "under_reconstructable" /\ c = "rs16_data_multi" ->
         DataShards(c) - 1
    [] OTHER -> IF ActualReturnsSome(c) THEN ActualRequired(c) ELSE 0

ActualMaxSelectedPerStripe(c) ==
  CASE Bug = "missing_stripe" /\ c = "rs16_data_multi" ->
         SpecRequired(c) + 1
    [] Bug = "under_reconstructable" /\ c = "rs16_data_multi" ->
         DataShards(c) + 1
    [] OTHER -> IF ActualReturnsSome(c) THEN ActualRequired(c) ELSE 0

ActualHasOutOfRange(c) ==
  CASE Bug = "out_of_range_index" /\ c = "rs16_data_multi" -> TRUE
    [] Bug = "skip_width_clamp" /\ c = "rs16_data_plus_one_no_parity" -> TRUE
    [] OTHER -> FALSE

ActualHasDuplicates(c) ==
  CASE Bug = "duplicate_indices" /\ c = "rs16_data_multi" -> TRUE
    [] OTHER -> FALSE

ActualSorted(c) ==
  CASE Bug = "unsorted_indices" /\ c = "rs16_data_plus_one_multi" -> FALSE
    [] OTHER -> TRUE

ActualCoversAllStripes(c) ==
  CASE Bug = "wrong_stripe_count" /\ c = "rs16_data_plus_one_multi" -> FALSE
    [] Bug = "missing_stripe" /\ c = "rs16_data_multi" -> FALSE
    [] OTHER -> IF ActualReturnsSome(c) THEN TRUE ELSE FALSE

TypeInvariant ==
  /\ Bug \in {
       "none",
       "full_returns_some",
       "plain_returns_some",
       "zero_required_returns_some",
       "data_uses_data_plus_one",
       "data_plus_one_omits_extra",
       "skip_width_clamp",
       "wrong_stripe_count",
       "duplicate_indices",
       "out_of_range_index",
       "unsorted_indices",
       "missing_stripe",
       "under_reconstructable"
     }
  /\ candidate \in Cases
  /\ returns_some \in BOOLEAN
  /\ required_count \in CountValues
  /\ selected_len \in CountValues
  /\ min_selected_per_stripe \in CountValues
  /\ max_selected_per_stripe \in CountValues
  /\ has_out_of_range \in BOOLEAN
  /\ has_duplicates \in BOOLEAN
  /\ sorted \in BOOLEAN
  /\ covers_all_stripes \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ returns_some = ActualReturnsSome(candidate)
  /\ required_count = ActualRequired(candidate)
  /\ selected_len = ActualSelectedLen(candidate)
  /\ min_selected_per_stripe = ActualMinSelectedPerStripe(candidate)
  /\ max_selected_per_stripe = ActualMaxSelectedPerStripe(candidate)
  /\ has_out_of_range = ActualHasOutOfRange(candidate)
  /\ has_duplicates = ActualHasDuplicates(candidate)
  /\ sorted = ActualSorted(candidate)
  /\ covers_all_stripes = ActualCoversAllStripes(candidate)

Next ==
  UNCHANGED vars

ReturnDecisionMatchesSpec ==
  returns_some = SpecReturnsSome(candidate)

RequiredCountMatchesSpec ==
  required_count = SpecRequired(candidate)

SelectedLenMatchesSpec ==
  selected_len = SpecSelectedLen(candidate)

FullFanoutReturnsNone ==
  Fanout(candidate) = "Full" => ~returns_some

NonRs16ReturnsNone ==
  ~IsRs16(candidate) => ~returns_some

ZeroRequiredReturnsNone ==
  RequestedRequired(candidate) = 0 => ~returns_some

DataFanoutUsesDataShardCount ==
  Fanout(candidate) = "Data" /\ returns_some =>
    required_count = DataShards(candidate)

DataPlusOneAddsOneWhenAvailable ==
  Fanout(candidate) = "DataPlusOne" /\ returns_some /\ ParityShards(candidate) # 0 =>
    required_count = DataShards(candidate) + 1

RequiredNeverExceedsStripeWidth ==
  returns_some => required_count <= StripeWidth(candidate)

SelectionHasNoOutOfRange ==
  ~has_out_of_range

SelectionHasNoDuplicates ==
  ~has_duplicates

SelectionSortedAfterDedup ==
  sorted

EveryStripeCoveredForReducedFanout ==
  returns_some =>
    /\ covers_all_stripes
    /\ min_selected_per_stripe >= 1

PerStripeSelectionMatchesRequired ==
  returns_some =>
    /\ min_selected_per_stripe = required_count
    /\ max_selected_per_stripe = required_count

SelectionLengthEqualsPerStripeTotal ==
  returns_some =>
    selected_len = StripeCount(candidate) * required_count

ReducedFanoutIsReconstructable ==
  returns_some =>
    min_selected_per_stripe >= DataShards(candidate)

TotalSelectionWithinChunkRange ==
  returns_some =>
    selected_len <= TotalChunks(candidate)

RbcRs16InitialFanoutCoreSafety ==
  /\ ReturnDecisionMatchesSpec
  /\ RequiredCountMatchesSpec
  /\ SelectedLenMatchesSpec
  /\ FullFanoutReturnsNone
  /\ NonRs16ReturnsNone
  /\ ZeroRequiredReturnsNone
  /\ DataFanoutUsesDataShardCount
  /\ DataPlusOneAddsOneWhenAvailable
  /\ RequiredNeverExceedsStripeWidth
  /\ SelectionHasNoOutOfRange
  /\ SelectionHasNoDuplicates
  /\ SelectionSortedAfterDedup
  /\ EveryStripeCoveredForReducedFanout
  /\ PerStripeSelectionMatchesRequired
  /\ SelectionLengthEqualsPerStripeTotal
  /\ ReducedFanoutIsReconstructable
  /\ TotalSelectionWithinChunkRange

Safety == RbcRs16InitialFanoutCoreSafety

====

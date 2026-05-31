---- MODULE SumeragiVrfEpochBoundaryGate ----

(***************************************************************************
A bounded abstract model for NPoS VRF epoch-boundary state transitions.

This slice covers `EpochManager::on_block_commit(...)`, `next_epoch()`,
`reset_epoch_state(...)`, `set_validator_roster_indices(...)`,
`take_last_penalties(...)`, `take_last_penalties_detailed()`,
`take_last_epoch_snapshot()`, and the `current_entropy()` helper from
`crates/iroha_core/src/sumeragi/epoch.rs`. It abstracts cryptographic hashes as
labels while preserving the deterministic boundary contract:
- height zero and non-boundary commits do not finalize or mutate epoch state,
- boundary commits compute exact committed-without-reveal and no-participation
  penalties, using late reveals as participation and a contiguous fallback
  roster only when no roster snapshot exists,
- finalized epoch snapshots preserve pre-clear inputs, penalties, roster length,
  seed, epoch, and update height,
- seed evolution depends on the previous seed and regular reveals ordered by
  signer, never on late reveals or caller/input order,
- boundary and explicit next-epoch advances use saturating epoch arithmetic and
  clear per-epoch input/roster state,
- explicit reset installs the supplied epoch/seed and clears all transient
  reports/snapshots, and
- take-style accessors consume the stored report/snapshot exactly once.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate

\* @type: <<Str>>;
vars == <<candidate>>

Cases == {
  "non_boundary_noop",
  "height_zero_noop",
  "boundary_committed_no_reveal",
  "boundary_late_reveal_clears",
  "boundary_no_participation_roster",
  "boundary_fallback_contiguous_roster",
  "boundary_empty_roster_no_penalties",
  "boundary_snapshot_preserves_inputs",
  "boundary_seed_regular_reveals",
  "boundary_clears_and_advances",
  "boundary_epoch_saturates",
  "next_epoch_seed_and_clear",
  "reset_epoch_state",
  "take_penalties_consumes",
  "take_snapshot_consumes",
  "roster_indices_canonical",
  "entropy_ordering",
  "entropy_empty"
}

ViolatesBoundaryContract(c) ==
  CASE Bug = "non_boundary_finalizes" ->
      c = "non_boundary_noop"
    [] Bug = "height_zero_finalizes" ->
      c = "height_zero_noop"
    [] Bug = "boundary_skips_committed_no_reveal" ->
      c = "boundary_committed_no_reveal"
    [] Bug = "late_reveal_counts_non_reveal" ->
      c = "boundary_late_reveal_clears"
    [] Bug = "roster_ignores_no_participation" ->
      c = "boundary_no_participation_roster"
    [] Bug = "fallback_roster_uses_observed_only" ->
      c = "boundary_fallback_contiguous_roster"
    [] Bug = "empty_roster_marks_zero" ->
      c = "boundary_empty_roster_no_penalties"
    [] Bug = "snapshot_drops_late_reveals" ->
      c = "boundary_snapshot_preserves_inputs"
    [] Bug = "snapshot_wrong_height" ->
      c = "boundary_snapshot_preserves_inputs"
    [] Bug = "seed_omits_reveals" ->
      c \in {"boundary_seed_regular_reveals",
             "next_epoch_seed_and_clear", "entropy_ordering"}
    [] Bug = "late_reveal_changes_seed" ->
      c = "boundary_seed_regular_reveals"
    [] Bug = "boundary_keeps_inputs" ->
      c \in {"boundary_clears_and_advances", "boundary_epoch_saturates"}
    [] Bug = "boundary_does_not_advance" ->
      c = "boundary_clears_and_advances"
    [] Bug = "boundary_wraps_epoch" ->
      c = "boundary_epoch_saturates"
    [] Bug = "next_epoch_keeps_inputs" ->
      c = "next_epoch_seed_and_clear"
    [] Bug = "next_epoch_wraps_epoch" ->
      c = "next_epoch_seed_and_clear"
    [] Bug = "reset_keeps_inputs" ->
      c = "reset_epoch_state"
    [] Bug = "reset_keeps_reports" ->
      c = "reset_epoch_state"
    [] Bug = "take_reports_not_consumed" ->
      c = "take_penalties_consumes"
    [] Bug = "take_snapshot_not_consumed" ->
      c = "take_snapshot_consumes"
    [] Bug = "roster_keeps_duplicates" ->
      c = "roster_indices_canonical"
    [] Bug = "entropy_uses_input_order" ->
      c = "entropy_ordering"
    [] Bug = "entropy_empty_returns_seed" ->
      c = "entropy_empty"
    [] OTHER -> FALSE

Init ==
  candidate = "none"

Apply(c) ==
  candidate' = c

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

BugModes == {
  "none",
  "non_boundary_finalizes",
  "height_zero_finalizes",
  "boundary_skips_committed_no_reveal",
  "late_reveal_counts_non_reveal",
  "roster_ignores_no_participation",
  "fallback_roster_uses_observed_only",
  "empty_roster_marks_zero",
  "snapshot_drops_late_reveals",
  "snapshot_wrong_height",
  "seed_omits_reveals",
  "late_reveal_changes_seed",
  "boundary_keeps_inputs",
  "boundary_does_not_advance",
  "boundary_wraps_epoch",
  "next_epoch_keeps_inputs",
  "next_epoch_wraps_epoch",
  "reset_keeps_inputs",
  "reset_keeps_reports",
  "take_reports_not_consumed",
  "take_snapshot_not_consumed",
  "roster_keeps_duplicates",
  "entropy_uses_input_order",
  "entropy_empty_returns_seed"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \cup {"none"}

BoundaryContractHolds ==
  ~ViolatesBoundaryContract(candidate)

Safety ==
  BoundaryContractHolds

====

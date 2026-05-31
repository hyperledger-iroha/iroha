---- MODULE SumeragiVrfEpochWindowGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for VRF epoch-window arithmetic.

`EpochManager::set_params(...)`, `position_in_epoch(...)`,
`epoch_for_height(...)`, `is_commit_window_position(...)`, and
`is_reveal_window_position(...)` define the time windows used by VRF commit and
reveal admission. This model fixes the observable edge cases: zero epoch
lengths are clamped to one block, offsets are clamped to the effective length,
block height zero has no epoch position and maps to epoch zero, normal heights
use one-based positions with `(height - 1)` epoch indexing, commit windows are
inclusive from position one through the clamped commit offset, and reveal
windows are inclusive only after the commit window through the clamped reveal
offset.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Set(Str);
  fields

\* @type: <<Str, Set(Str)>>;
vars == <<candidate, fields>>

Cases == {
  "set_params_zero_length",
  "set_params_clamps_offsets",
  "height_zero",
  "first_block",
  "epoch_boundary",
  "commit_boundary",
  "reveal_start",
  "reveal_boundary",
  "after_reveal",
  "reveal_before_commit"
}

ParamFields == {
  "length_max_one",
  "commit_offset_clamped",
  "reveal_offset_clamped"
}

PositionFields == {
  "height_zero_position_none",
  "height_one_position_one",
  "position_one_based",
  "position_wraps_mod_length"
}

EpochFields == {
  "height_zero_epoch_zero",
  "epoch_uses_height_minus_one"
}

CommitWindowFields == {
  "commit_requires_positive_position",
  "commit_starts_at_one",
  "commit_includes_boundary",
  "commit_uses_clamped_end"
}

RevealWindowFields == {
  "reveal_starts_after_commit",
  "reveal_includes_boundary",
  "reveal_uses_clamped_end",
  "reveal_empty_when_end_before_start"
}

OutsideFields == {"outside_windows_rejected"}

BadFields == {
  "length_zero_preserved",
  "commit_offset_raw",
  "reveal_offset_raw",
  "height_zero_position_some",
  "height_one_position_zero",
  "position_zero_based",
  "position_no_wrap",
  "height_zero_epoch_current",
  "epoch_uses_height_direct",
  "commit_accepts_zero_position",
  "commit_excludes_boundary",
  "commit_uses_raw_end",
  "reveal_starts_at_commit",
  "reveal_excludes_boundary",
  "reveal_uses_raw_end",
  "reveal_allows_end_before_start",
  "outside_windows_accepted"
}

SpecFields(c) ==
  CASE c = "set_params_zero_length" ->
      {"length_max_one", "commit_offset_clamped", "reveal_offset_clamped"}
    [] c = "set_params_clamps_offsets" ->
      {"commit_offset_clamped", "reveal_offset_clamped"}
    [] c = "height_zero" ->
      {"height_zero_position_none", "height_zero_epoch_zero",
       "commit_requires_positive_position", "outside_windows_rejected"}
    [] c = "first_block" ->
      {"height_one_position_one", "position_one_based",
       "epoch_uses_height_minus_one", "commit_starts_at_one"}
    [] c = "epoch_boundary" ->
      {"position_wraps_mod_length", "position_one_based",
       "epoch_uses_height_minus_one", "commit_starts_at_one"}
    [] c = "commit_boundary" ->
      {"commit_includes_boundary", "commit_uses_clamped_end"}
    [] c = "reveal_start" ->
      {"reveal_starts_after_commit", "outside_windows_rejected"}
    [] c = "reveal_boundary" ->
      {"reveal_includes_boundary", "reveal_uses_clamped_end"}
    [] c = "after_reveal" ->
      {"outside_windows_rejected", "commit_uses_clamped_end",
       "reveal_uses_clamped_end"}
    [] c = "reveal_before_commit" ->
      {"commit_includes_boundary", "reveal_empty_when_end_before_start"}
    [] OTHER -> {}

ActualFields(c) ==
  CASE c = "set_params_zero_length" /\ Bug = "preserve_zero_length" ->
      (SpecFields(c) \ {"length_max_one"}) \union {"length_zero_preserved"}
    [] c \in {"set_params_zero_length", "set_params_clamps_offsets"} /\
          Bug = "skip_commit_offset_clamp" ->
      (SpecFields(c) \ {"commit_offset_clamped"}) \union {"commit_offset_raw"}
    [] c \in {"set_params_zero_length", "set_params_clamps_offsets"} /\
          Bug = "skip_reveal_offset_clamp" ->
      (SpecFields(c) \ {"reveal_offset_clamped"}) \union {"reveal_offset_raw"}
    [] c = "height_zero" /\ Bug = "height_zero_has_position" ->
      (SpecFields(c) \ {"height_zero_position_none"}) \union {"height_zero_position_some"}
    [] c = "first_block" /\ Bug = "height_one_zero_based" ->
      (SpecFields(c) \ {"height_one_position_one"}) \union {"height_one_position_zero"}
    [] c \in {"first_block", "epoch_boundary"} /\ Bug = "position_zero_based" ->
      (SpecFields(c) \ {"position_one_based"}) \union {"position_zero_based"}
    [] c = "epoch_boundary" /\ Bug = "position_does_not_wrap" ->
      (SpecFields(c) \ {"position_wraps_mod_length"}) \union {"position_no_wrap"}
    [] c = "height_zero" /\ Bug = "height_zero_epoch_current" ->
      (SpecFields(c) \ {"height_zero_epoch_zero"}) \union {"height_zero_epoch_current"}
    [] c \in {"first_block", "epoch_boundary"} /\ Bug = "epoch_uses_height_direct" ->
      (SpecFields(c) \ {"epoch_uses_height_minus_one"}) \union {"epoch_uses_height_direct"}
    [] c = "height_zero" /\ Bug = "commit_accepts_zero_position" ->
      (SpecFields(c) \ {"commit_requires_positive_position"}) \union {"commit_accepts_zero_position"}
    [] c \in {"commit_boundary", "reveal_before_commit"} /\
          Bug = "commit_excludes_boundary" ->
      (SpecFields(c) \ {"commit_includes_boundary"}) \union {"commit_excludes_boundary"}
    [] c \in {"commit_boundary", "after_reveal"} /\ Bug = "commit_uses_raw_end" ->
      (SpecFields(c) \ {"commit_uses_clamped_end"}) \union {"commit_uses_raw_end"}
    [] c = "reveal_start" /\ Bug = "reveal_starts_at_commit" ->
      (SpecFields(c) \ {"reveal_starts_after_commit"}) \union {"reveal_starts_at_commit"}
    [] c = "reveal_boundary" /\ Bug = "reveal_excludes_boundary" ->
      (SpecFields(c) \ {"reveal_includes_boundary"}) \union {"reveal_excludes_boundary"}
    [] c \in {"reveal_boundary", "after_reveal"} /\ Bug = "reveal_uses_raw_end" ->
      (SpecFields(c) \ {"reveal_uses_clamped_end"}) \union {"reveal_uses_raw_end"}
    [] c = "reveal_before_commit" /\ Bug = "reveal_allows_end_before_start" ->
      (SpecFields(c) \ {"reveal_empty_when_end_before_start"}) \union {"reveal_allows_end_before_start"}
    [] c \in {"height_zero", "reveal_start", "after_reveal"} /\
          Bug = "outside_windows_accepted" ->
      (SpecFields(c) \ {"outside_windows_rejected"}) \union {"outside_windows_accepted"}
    [] OTHER -> SpecFields(c)

BugModes == {
  "none",
  "preserve_zero_length",
  "skip_commit_offset_clamp",
  "skip_reveal_offset_clamp",
  "height_zero_has_position",
  "height_one_zero_based",
  "position_zero_based",
  "position_does_not_wrap",
  "height_zero_epoch_current",
  "epoch_uses_height_direct",
  "commit_accepts_zero_position",
  "commit_excludes_boundary",
  "commit_uses_raw_end",
  "reveal_starts_at_commit",
  "reveal_excludes_boundary",
  "reveal_uses_raw_end",
  "reveal_allows_end_before_start",
  "outside_windows_accepted"
}

AllFields ==
  ParamFields
    \union PositionFields
    \union EpochFields
    \union CommitWindowFields
    \union RevealWindowFields
    \union OutsideFields
    \union BadFields

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ fields \subseteq AllFields

Init ==
  /\ candidate = "none"
  /\ fields = {}

Apply(c) ==
  /\ candidate' = c
  /\ fields' = ActualFields(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

FieldsMatchSpec ==
  candidate = "none" \/ fields = SpecFields(candidate)

ParameterClampsHold ==
  candidate \in {"set_params_zero_length", "set_params_clamps_offsets"} =>
    /\ "commit_offset_clamped" \in fields
    /\ "reveal_offset_clamped" \in fields
    /\ fields \cap {"commit_offset_raw", "reveal_offset_raw"} = {}

ZeroLengthClamped ==
  candidate = "set_params_zero_length" =>
    /\ "length_max_one" \in fields
    /\ "length_zero_preserved" \notin fields

PositionMappingStable ==
  candidate \in {"height_zero", "first_block", "epoch_boundary"} =>
    /\ fields \cap {"height_zero_position_some", "height_one_position_zero",
                    "position_zero_based", "position_no_wrap"} = {}

EpochMappingStable ==
  candidate \in {"height_zero", "first_block", "epoch_boundary"} =>
    /\ fields \cap {"height_zero_epoch_current", "epoch_uses_height_direct"} = {}

CommitWindowStable ==
  candidate \in {"height_zero", "first_block", "epoch_boundary",
                 "commit_boundary", "after_reveal", "reveal_before_commit"} =>
    /\ fields \cap {"commit_accepts_zero_position", "commit_excludes_boundary",
                    "commit_uses_raw_end"} = {}

RevealWindowStable ==
  candidate \in {"reveal_start", "reveal_boundary", "after_reveal",
                 "reveal_before_commit"} =>
    /\ fields \cap {"reveal_starts_at_commit", "reveal_excludes_boundary",
                    "reveal_uses_raw_end", "reveal_allows_end_before_start"} = {}

OutsideWindowsRejected ==
  candidate \in {"height_zero", "reveal_start", "after_reveal"} =>
    /\ "outside_windows_rejected" \in fields
    /\ "outside_windows_accepted" \notin fields

Safety ==
  /\ FieldsMatchSpec
  /\ ParameterClampsHold
  /\ ZeroLengthClamped
  /\ PositionMappingStable
  /\ EpochMappingStable
  /\ CommitWindowStable
  /\ RevealWindowStable
  /\ OutsideWindowsRejected

====

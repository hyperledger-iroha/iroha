---- MODULE SumeragiBackgroundFrameCapGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for background consensus frame-cap preparation.

The model covers `trim_block_sync_update_for_frame_cap(...)`,
`trim_block_body_response_for_frame_cap(...)`,
`direct_block_sync_update_fallback_for_body_response(...)`, and
`prepare_background_block_message(...)`:

- payload-carrying messages use the consensus payload frame cap, while
  request/control messages use the ordinary consensus frame cap,
- under-cap messages pass unchanged,
- oversized `BlockSyncUpdate` payloads first drop commit votes, then trim
  mode-specific sidecars while preserving as much verifiable proof material as
  possible,
- permissioned updates may drop stale stake snapshots before splitting the
  validator checkpoint and commit QC; NPoS updates keep stake snapshots,
- when both checkpoint and commit QC are present, commit QC is tried first so a
  validator checkpoint can be retained when that is sufficient,
- oversized `BlockBodyResponse(BlockSyncUpdate)` values trim the embedded
  update, then downgrade to a `BlockCreated` response, and finally prefer a
  direct `BlockSyncUpdate` fallback when the trimmed direct update fits,
- untrimmed oversized messages are dropped.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "fetch_request_control_cap",
  "block_created_payload_cap",
  "block_created_over_payload",
  "update_under",
  "update_votes_fit",
  "permissioned_stake_fit",
  "permissioned_drop_qc_fit",
  "permissioned_drop_checkpoint_fit",
  "npos_drop_qc_fit",
  "npos_drop_checkpoint_fit",
  "update_still_oversized",
  "response_created_under",
  "response_created_over",
  "response_update_under",
  "response_update_trim_fits",
  "response_update_direct_fallback",
  "response_update_downgrade_created",
  "response_update_drop"
}

PayloadCapCases == {
  "block_created_payload_cap",
  "block_created_over_payload",
  "update_under",
  "update_votes_fit",
  "permissioned_stake_fit",
  "permissioned_drop_qc_fit",
  "permissioned_drop_checkpoint_fit",
  "npos_drop_qc_fit",
  "npos_drop_checkpoint_fit",
  "update_still_oversized",
  "response_created_under",
  "response_created_over",
  "response_update_under",
  "response_update_trim_fits",
  "response_update_direct_fallback",
  "response_update_downgrade_created",
  "response_update_drop"
}

SpecCap(c) ==
  IF c \in PayloadCapCases THEN "payload" ELSE "control"

SpecAccept(c) ==
  c \notin {
    "block_created_over_payload",
    "update_still_oversized",
    "response_created_over",
    "response_update_drop"
  }

SpecResult(c) ==
  CASE c = "fetch_request_control_cap" -> "same_fetch_request"
    [] c = "block_created_payload_cap" -> "same_block_created"
    [] c = "block_created_over_payload" -> "dropped"
    [] c = "update_under" -> "same_update"
    [] c = "update_votes_fit" -> "update_without_votes"
    [] c = "permissioned_stake_fit" -> "permissioned_update_without_stake"
    [] c = "permissioned_drop_qc_fit" -> "update_without_commit_qc"
    [] c = "permissioned_drop_checkpoint_fit" -> "update_without_checkpoint"
    [] c = "npos_drop_qc_fit" -> "update_without_commit_qc"
    [] c = "npos_drop_checkpoint_fit" -> "update_without_checkpoint"
    [] c = "update_still_oversized" -> "dropped"
    [] c = "response_created_under" -> "same_response_created"
    [] c = "response_created_over" -> "dropped"
    [] c = "response_update_under" -> "same_response_update"
    [] c = "response_update_trim_fits" -> "trimmed_response_update"
    [] c = "response_update_direct_fallback" -> "direct_block_sync_update"
    [] c = "response_update_downgrade_created" -> "response_created"
    [] OTHER -> "dropped"

SpecVotesPresent(c) ==
  c \notin {"update_votes_fit"}

SpecStakePresent(c) ==
  c \notin {"permissioned_stake_fit"}

SpecCommitQcPresent(c) ==
  c \notin {
    "permissioned_drop_qc_fit",
    "npos_drop_qc_fit"
  }

SpecCheckpointPresent(c) ==
  c \notin {
    "permissioned_drop_checkpoint_fit",
    "npos_drop_checkpoint_fit"
  }

SpecDirectFallback(c) ==
  c = "response_update_direct_fallback"

SpecChanged(c) ==
  SpecResult(c) \notin {
    "same_fetch_request",
    "same_block_created",
    "same_update",
    "same_response_created",
    "same_response_update",
    "dropped"
  }

ActualCap(c) ==
  CASE Bug = "use_payload_cap_for_fetch_request"
       /\ c = "fetch_request_control_cap" -> "payload"
    [] Bug = "use_control_cap_for_payload"
       /\ c = "block_created_payload_cap" -> "control"
    [] OTHER -> SpecCap(c)

ActualAccept(c) ==
  CASE Bug = "drop_under_cap"
       /\ c = "update_under" -> FALSE
    [] Bug = "oversized_payload_allowed"
       /\ c = "block_created_over_payload" -> TRUE
    [] Bug = "oversized_update_allowed"
       /\ c = "update_still_oversized" -> TRUE
    [] Bug = "created_response_over_allowed"
       /\ c = "response_created_over" -> TRUE
    [] Bug = "response_update_drop_allowed"
       /\ c = "response_update_drop" -> TRUE
    [] Bug = "use_payload_cap_for_fetch_request"
       /\ c = "fetch_request_control_cap" -> FALSE
    [] Bug = "use_control_cap_for_payload"
       /\ c = "block_created_payload_cap" -> FALSE
    [] OTHER -> SpecAccept(c)

ActualResult(c) ==
  CASE Bug = "mutate_under_cap_update"
       /\ c = "update_under" -> "update_without_votes"
    [] Bug = "skip_vote_trim"
       /\ c = "update_votes_fit" -> "same_update"
    [] Bug = "permissioned_keeps_stake"
       /\ c = "permissioned_stake_fit" -> "same_update"
    [] Bug = "permissioned_drops_checkpoint_first"
       /\ c = "permissioned_drop_qc_fit" -> "update_without_checkpoint"
    [] Bug = "permissioned_drops_qc_when_checkpoint_needed"
       /\ c = "permissioned_drop_checkpoint_fit" -> "update_without_commit_qc"
    [] Bug = "npos_drops_checkpoint_first"
       /\ c = "npos_drop_qc_fit" -> "update_without_checkpoint"
    [] Bug = "skip_response_update_trim"
       /\ c = "response_update_trim_fits" -> "response_created"
    [] Bug = "skip_response_created_downgrade"
       /\ c = "response_update_downgrade_created" -> "dropped"
    [] Bug = "drop_response_direct_fallback"
       /\ c = "response_update_direct_fallback" -> "dropped"
    [] Bug = "direct_fallback_not_preferred"
       /\ c = "response_update_direct_fallback" -> "response_created"
    [] Bug = "direct_fallback_without_fit"
       /\ c = "response_update_downgrade_created" -> "direct_block_sync_update"
    [] Bug = "oversized_payload_allowed"
       /\ c = "block_created_over_payload" -> "same_block_created"
    [] Bug = "oversized_update_allowed"
       /\ c = "update_still_oversized" -> "same_update"
    [] Bug = "created_response_over_allowed"
       /\ c = "response_created_over" -> "same_response_created"
    [] Bug = "response_update_drop_allowed"
       /\ c = "response_update_drop" -> "response_created"
    [] OTHER -> SpecResult(c)

ActualVotesPresent(c) ==
  CASE Bug = "skip_vote_trim"
       /\ c = "update_votes_fit" -> TRUE
    [] OTHER -> SpecVotesPresent(c)

ActualStakePresent(c) ==
  CASE Bug = "permissioned_keeps_stake"
       /\ c = "permissioned_stake_fit" -> TRUE
    [] Bug = "npos_drops_stake"
       /\ c = "npos_drop_qc_fit" -> FALSE
    [] OTHER -> SpecStakePresent(c)

ActualCommitQcPresent(c) ==
  CASE Bug = "permissioned_drops_checkpoint_first"
       /\ c = "permissioned_drop_qc_fit" -> TRUE
    [] Bug = "permissioned_drops_qc_when_checkpoint_needed"
       /\ c = "permissioned_drop_checkpoint_fit" -> FALSE
    [] Bug = "npos_drops_checkpoint_first"
       /\ c = "npos_drop_qc_fit" -> TRUE
    [] OTHER -> SpecCommitQcPresent(c)

ActualCheckpointPresent(c) ==
  CASE Bug = "permissioned_drops_checkpoint_first"
       /\ c = "permissioned_drop_qc_fit" -> FALSE
    [] Bug = "permissioned_drops_qc_when_checkpoint_needed"
       /\ c = "permissioned_drop_checkpoint_fit" -> TRUE
    [] Bug = "npos_drops_checkpoint_first"
       /\ c = "npos_drop_qc_fit" -> FALSE
    [] OTHER -> SpecCheckpointPresent(c)

ActualDirectFallback(c) ==
  CASE Bug = "drop_response_direct_fallback"
       /\ c = "response_update_direct_fallback" -> FALSE
    [] Bug = "direct_fallback_not_preferred"
       /\ c = "response_update_direct_fallback" -> FALSE
    [] Bug = "direct_fallback_without_fit"
       /\ c = "response_update_downgrade_created" -> TRUE
    [] OTHER -> SpecDirectFallback(c)

ActualChanged(c) ==
  CASE Bug = "mutate_under_cap_update"
       /\ c = "update_under" -> TRUE
    [] Bug = "skip_vote_trim"
       /\ c = "update_votes_fit" -> FALSE
    [] OTHER -> ActualResult(c) \notin {
         "same_fetch_request",
         "same_block_created",
         "same_update",
         "same_response_created",
         "same_response_update",
         "dropped"
       }

Matches(c) ==
  /\ ActualCap(c) = SpecCap(c)
  /\ ActualAccept(c) = SpecAccept(c)
  /\ ActualResult(c) = SpecResult(c)
  /\ ActualVotesPresent(c) = SpecVotesPresent(c)
  /\ ActualStakePresent(c) = SpecStakePresent(c)
  /\ ActualCommitQcPresent(c) = SpecCommitQcPresent(c)
  /\ ActualCheckpointPresent(c) = SpecCheckpointPresent(c)
  /\ ActualDirectFallback(c) = SpecDirectFallback(c)
  /\ ActualChanged(c) = SpecChanged(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "use_payload_cap_for_fetch_request",
       "use_control_cap_for_payload",
       "drop_under_cap",
       "mutate_under_cap_update",
       "oversized_payload_allowed",
       "skip_vote_trim",
       "permissioned_keeps_stake",
       "permissioned_drops_checkpoint_first",
       "permissioned_drops_qc_when_checkpoint_needed",
       "npos_drops_stake",
       "npos_drops_checkpoint_first",
       "oversized_update_allowed",
       "created_response_over_allowed",
       "skip_response_update_trim",
       "skip_response_created_downgrade",
       "drop_response_direct_fallback",
       "direct_fallback_not_preferred",
       "direct_fallback_without_fit",
       "response_update_drop_allowed"
     }
  /\ checked = 0

FramePreparationMatchesSpec ==
  \A c \in Cases: Matches(c)

BackgroundFrameCapExactness ==
  /\ FramePreparationMatchesSpec

BackgroundFrameCapCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BackgroundFrameCapExactness

SafetyFast == BackgroundFrameCapExactness

FetchRequestUsesControlCap ==
  Matches("fetch_request_control_cap")

BlockCreatedUsesPayloadCap ==
  Matches("block_created_payload_cap")

OversizedPayloadDropped ==
  Matches("block_created_over_payload")

UnderCapUpdateUnchanged ==
  Matches("update_under")

CommitVotesTrimFirst ==
  Matches("update_votes_fit")

PermissionedDropsStakeWhenNeeded ==
  Matches("permissioned_stake_fit")

PermissionedDropsCommitQcBeforeCheckpoint ==
  Matches("permissioned_drop_qc_fit")

PermissionedDropsCheckpointOnlyWhenQcDropInsufficient ==
  Matches("permissioned_drop_checkpoint_fit")

NposDropsCommitQcBeforeCheckpoint ==
  Matches("npos_drop_qc_fit")

NposKeepsStakeSnapshot ==
  Matches("npos_drop_qc_fit")

OversizedUpdateDropped ==
  Matches("update_still_oversized")

UnderCapResponseCreatedUnchanged ==
  Matches("response_created_under")

OversizedResponseCreatedDropped ==
  Matches("response_created_over")

UnderCapResponseUpdateUnchanged ==
  Matches("response_update_under")

ResponseUpdateTrimmedBeforeDowngrade ==
  Matches("response_update_trim_fits")

ResponseUpdateDirectFallbackPreferred ==
  Matches("response_update_direct_fallback")

NoDirectFallbackWithoutFit ==
  Matches("response_update_downgrade_created")

ResponseUpdateDowngradesToCreated ==
  Matches("response_update_downgrade_created")

OversizedResponseUpdateDropped ==
  Matches("response_update_drop")

====

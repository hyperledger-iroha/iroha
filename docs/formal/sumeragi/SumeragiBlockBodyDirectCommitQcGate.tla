---- MODULE SumeragiBlockBodyDirectCommitQcGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `direct_commit_qc_from_block_body_response(...)`.

The helper may detach a commit QC from a block-body response only when the body
block identity matches the response tuple. `BlockSyncUpdate` responses prefer
an embedded commit QC, then a validator-checkpoint-derived QC, then a locally
available direct commit QC for the block. `BlockCreated` responses can only use
the locally available direct commit QC for the block.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

None == "none"
EmbeddedQc == "embedded_qc"
CheckpointQc == "checkpoint_qc"
LocalQc == "local_qc"

Created == "created"
Update == "update"

Cases == {
  "update_embedded_qc",
  "update_embedded_over_checkpoint",
  "update_checkpoint_qc",
  "update_checkpoint_over_local",
  "update_local_qc",
  "update_no_qc",
  "update_hash_mismatch",
  "update_height_mismatch",
  "update_view_mismatch",
  "created_local_qc",
  "created_no_qc",
  "created_hash_mismatch",
  "created_height_mismatch",
  "created_view_mismatch"
}

BodyKind(c) ==
  IF c \in {
       "created_local_qc",
       "created_no_qc",
       "created_hash_mismatch",
       "created_height_mismatch",
       "created_view_mismatch"
     } THEN Created ELSE Update

IdentityMatches(c) ==
  c \notin {
    "update_hash_mismatch",
    "update_height_mismatch",
    "update_view_mismatch",
    "created_hash_mismatch",
    "created_height_mismatch",
    "created_view_mismatch"
  }

EmbeddedCommitQc(c) ==
  c \in {"update_embedded_qc", "update_embedded_over_checkpoint"}

CheckpointCommitQc(c) ==
  c \in {
    "update_embedded_over_checkpoint",
    "update_checkpoint_qc",
    "update_checkpoint_over_local"
  }

LocalDirectQc(c) ==
  c \in {"update_checkpoint_over_local", "update_local_qc", "created_local_qc"}

SpecSource(c) ==
  IF ~IdentityMatches(c) THEN
    None
  ELSE IF BodyKind(c) = Update THEN
    IF EmbeddedCommitQc(c) THEN
      EmbeddedQc
    ELSE IF CheckpointCommitQc(c) THEN
      CheckpointQc
    ELSE IF LocalDirectQc(c) THEN
      LocalQc
    ELSE
      None
  ELSE IF LocalDirectQc(c) THEN
    LocalQc
  ELSE
    None

ActualSource(c) ==
  CASE Bug = "drop_update_embedded"
       /\ c = "update_embedded_qc" -> None
    [] Bug = "checkpoint_overrides_embedded"
       /\ c = "update_embedded_over_checkpoint" -> CheckpointQc
    [] Bug = "drop_update_checkpoint"
       /\ c = "update_checkpoint_qc" -> None
    [] Bug = "local_overrides_checkpoint"
       /\ c = "update_checkpoint_over_local" -> LocalQc
    [] Bug = "drop_update_local"
       /\ c = "update_local_qc" -> None
    [] Bug = "update_no_qc_returns_local"
       /\ c = "update_no_qc" -> LocalQc
    [] Bug = "update_hash_mismatch_allowed"
       /\ c = "update_hash_mismatch" -> EmbeddedQc
    [] Bug = "update_height_mismatch_allowed"
       /\ c = "update_height_mismatch" -> EmbeddedQc
    [] Bug = "update_view_mismatch_allowed"
       /\ c = "update_view_mismatch" -> EmbeddedQc
    [] Bug = "drop_created_local"
       /\ c = "created_local_qc" -> None
    [] Bug = "created_no_qc_returns_local"
       /\ c = "created_no_qc" -> LocalQc
    [] Bug = "created_hash_mismatch_allowed"
       /\ c = "created_hash_mismatch" -> LocalQc
    [] Bug = "created_height_mismatch_allowed"
       /\ c = "created_height_mismatch" -> LocalQc
    [] Bug = "created_view_mismatch_allowed"
       /\ c = "created_view_mismatch" -> LocalQc
    [] OTHER -> SpecSource(c)

Matches(c) ==
  ActualSource(c) = SpecSource(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_update_embedded",
       "checkpoint_overrides_embedded",
       "drop_update_checkpoint",
       "local_overrides_checkpoint",
       "drop_update_local",
       "update_no_qc_returns_local",
       "update_hash_mismatch_allowed",
       "update_height_mismatch_allowed",
       "update_view_mismatch_allowed",
       "drop_created_local",
       "created_no_qc_returns_local",
       "created_hash_mismatch_allowed",
       "created_height_mismatch_allowed",
       "created_view_mismatch_allowed"
     }
  /\ checked = 0

DirectCommitQcMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockBodyDirectCommitQcExactness ==
  /\ DirectCommitQcMatchesSpec

BlockBodyDirectCommitQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockBodyDirectCommitQcExactness

SafetyFast == BlockBodyDirectCommitQcExactness

UpdateEmbeddedReturned ==
  Matches("update_embedded_qc")

EmbeddedBeatsCheckpoint ==
  Matches("update_embedded_over_checkpoint")

UpdateCheckpointReturned ==
  Matches("update_checkpoint_qc")

CheckpointBeatsLocal ==
  Matches("update_checkpoint_over_local")

UpdateLocalReturned ==
  Matches("update_local_qc")

UpdateNoQcRejected ==
  Matches("update_no_qc")

UpdateHashMismatchRejected ==
  Matches("update_hash_mismatch")

UpdateHeightMismatchRejected ==
  Matches("update_height_mismatch")

UpdateViewMismatchRejected ==
  Matches("update_view_mismatch")

CreatedLocalReturned ==
  Matches("created_local_qc")

CreatedNoQcRejected ==
  Matches("created_no_qc")

CreatedHashMismatchRejected ==
  Matches("created_hash_mismatch")

CreatedHeightMismatchRejected ==
  Matches("created_height_mismatch")

CreatedViewMismatchRejected ==
  Matches("created_view_mismatch")

====

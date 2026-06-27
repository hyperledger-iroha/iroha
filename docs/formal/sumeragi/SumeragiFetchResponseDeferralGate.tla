---- MODULE SumeragiFetchResponseDeferralGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for
`should_defer_canonical_committed_fetch_response(...)`.

The helper prevents exact fetch/body responses from serving a canonical
committed tip as a bare payload while portable commit proof is still missing.
It should defer only when the block is exactly the local committed height, the
local committed hash for that height matches the block hash, and the response
payload is either `BlockCreated` or a `BlockSyncUpdate` without commit-QC or
validator-checkpoint sidecars.
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
Block == "block"
Other == "other"

Created == "created"
Update == "update"
OtherMessage == "other_message"

Cases == {
  "canonical_block_created",
  "canonical_bare_update",
  "canonical_update_with_commit_qc",
  "canonical_update_with_checkpoint",
  "canonical_update_with_both_proofs",
  "canonical_other_message",
  "next_height_block_created",
  "historical_block_created",
  "same_height_hash_mismatch",
  "same_height_hash_unknown"
}

LocalCommittedHeight(c) == 3

BlockHeight(c) ==
  CASE c = "next_height_block_created" -> 4
    [] c = "historical_block_created" -> 2
    [] OTHER -> 3

BlockHash(c) == Block

CommittedHash(c) ==
  CASE c = "same_height_hash_mismatch" -> Other
    [] c = "same_height_hash_unknown" -> None
    [] OTHER -> Block

MessageKind(c) ==
  CASE c = "canonical_other_message" -> OtherMessage
    [] c \in {
         "canonical_bare_update",
         "canonical_update_with_commit_qc",
         "canonical_update_with_checkpoint",
         "canonical_update_with_both_proofs"
       } -> Update
    [] OTHER -> Created

HasCommitQc(c) ==
  c \in {
    "canonical_update_with_commit_qc",
    "canonical_update_with_both_proofs"
  }

HasValidatorCheckpoint(c) ==
  c \in {
    "canonical_update_with_checkpoint",
    "canonical_update_with_both_proofs"
  }

SpecDefer(c) ==
  /\ BlockHeight(c) = LocalCommittedHeight(c)
  /\ CommittedHash(c) = BlockHash(c)
  /\ \/ MessageKind(c) = Created
     \/ /\ MessageKind(c) = Update
        /\ ~HasCommitQc(c)
        /\ ~HasValidatorCheckpoint(c)

ActualDefer(c) ==
  CASE Bug = "ignore_next_height"
       /\ c = "next_height_block_created" -> TRUE
    [] Bug = "ignore_historical_height"
       /\ c = "historical_block_created" -> TRUE
    [] Bug = "ignore_hash_mismatch"
       /\ c = "same_height_hash_mismatch" -> TRUE
    [] Bug = "unknown_hash_matches"
       /\ c = "same_height_hash_unknown" -> TRUE
    [] Bug = "block_created_not_deferred"
       /\ c = "canonical_block_created" -> FALSE
    [] Bug = "bare_update_not_deferred"
       /\ c = "canonical_bare_update" -> FALSE
    [] Bug = "commit_qc_ignored"
       /\ c = "canonical_update_with_commit_qc" -> TRUE
    [] Bug = "checkpoint_ignored"
       /\ c = "canonical_update_with_checkpoint" -> TRUE
    [] Bug = "both_proofs_ignored"
       /\ c = "canonical_update_with_both_proofs" -> TRUE
    [] Bug = "other_message_deferred"
       /\ c = "canonical_other_message" -> TRUE
    [] OTHER -> SpecDefer(c)

Matches(c) ==
  ActualDefer(c) = SpecDefer(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "ignore_next_height",
       "ignore_historical_height",
       "ignore_hash_mismatch",
       "unknown_hash_matches",
       "block_created_not_deferred",
       "bare_update_not_deferred",
       "commit_qc_ignored",
       "checkpoint_ignored",
       "both_proofs_ignored",
       "other_message_deferred"
     }
  /\ checked = 0

FetchResponseDeferralMatchesSpec ==
  \A c \in Cases: Matches(c)

FetchResponseDeferralExactness ==
  FetchResponseDeferralMatchesSpec

FetchResponseDeferralCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FetchResponseDeferralExactness

SafetyFast ==
  FetchResponseDeferralExactness

NextHeightNotDeferred ==
  Matches("next_height_block_created")

HistoricalHeightNotDeferred ==
  Matches("historical_block_created")

HashMismatchNotDeferred ==
  Matches("same_height_hash_mismatch")

UnknownHashNotDeferred ==
  Matches("same_height_hash_unknown")

BlockCreatedDeferred ==
  Matches("canonical_block_created")

BareUpdateDeferred ==
  Matches("canonical_bare_update")

UpdateWithCommitQcNotDeferred ==
  Matches("canonical_update_with_commit_qc")

UpdateWithCheckpointNotDeferred ==
  Matches("canonical_update_with_checkpoint")

UpdateWithBothProofsNotDeferred ==
  Matches("canonical_update_with_both_proofs")

OtherMessageNotDeferred ==
  Matches("canonical_other_message")

====

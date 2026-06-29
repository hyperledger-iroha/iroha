---- MODULE SumeragiBlockBodyRepairGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `allow_rbc_session_block_body_repair(...)` and
the body identity extracted by `block_body_response_payload_identity(...)`.

The helper admits an exact block-body repair only for the current frontier
height while DA/RBC is enabled, an RBC session already exists for the response
slot, the session metadata still matches local progress, and the session still
needs an authoritative payload. The body carried by either `BlockCreated` or
`BlockSyncUpdate` must identify the same block hash, height, view, and payload
hash as the response/session.
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
OtherBlock == "other_block"
Payload == "payload"
OtherPayload == "other_payload"

Created == "created"
SyncUpdate == "sync_update"

Cases == {
  "happy_block_created",
  "happy_block_sync_update",
  "da_disabled",
  "not_frontier_exact",
  "session_missing",
  "metadata_mismatch",
  "authoritative_payload_known",
  "missing_expected_payload_hash",
  "response_block_hash_mismatch",
  "response_height_mismatch",
  "response_view_mismatch",
  "response_payload_hash_mismatch"
}

RuntimeDaEnabled(c) ==
  c # "da_disabled"

FrontierHeight(c) == 3

ResponseHeight(c) ==
  IF c = "not_frontier_exact" THEN 4 ELSE 3

FrontierSlotExact(c) ==
  ResponseHeight(c) = FrontierHeight(c)

ResponseBlockHash(c) == Block

ResponseView(c) == 1

SessionExists(c) ==
  c # "session_missing"

SessionMetadataMatches(c) ==
  c # "metadata_mismatch"

SessionHasAuthoritativePayload(c) ==
  c = "authoritative_payload_known"

ExpectedPayloadHash(c) ==
  IF c = "missing_expected_payload_hash" THEN None ELSE Payload

BodyVariant(c) ==
  IF c = "happy_block_created" THEN Created ELSE SyncUpdate

BodyBlockHash(c) ==
  IF c = "response_block_hash_mismatch" THEN OtherBlock ELSE Block

BodyHeight(c) ==
  IF c = "response_height_mismatch" THEN 4 ELSE ResponseHeight(c)

BodyView(c) ==
  IF c = "response_view_mismatch" THEN 2 ELSE ResponseView(c)

BodyPayloadHash(c) ==
  IF c = "response_payload_hash_mismatch" THEN OtherPayload ELSE Payload

IdentityMatchesResponse(c) ==
  /\ BodyBlockHash(c) = ResponseBlockHash(c)
  /\ BodyHeight(c) = ResponseHeight(c)
  /\ BodyView(c) = ResponseView(c)

SpecAllow(c) ==
  /\ RuntimeDaEnabled(c)
  /\ FrontierSlotExact(c)
  /\ SessionExists(c)
  /\ SessionMetadataMatches(c)
  /\ ~SessionHasAuthoritativePayload(c)
  /\ ExpectedPayloadHash(c) # None
  /\ IdentityMatchesResponse(c)
  /\ BodyPayloadHash(c) = ExpectedPayloadHash(c)
  /\ BodyVariant(c) \in {Created, SyncUpdate}

ActualAllow(c) ==
  CASE Bug = "skip_da_gate"
       /\ c = "da_disabled" -> TRUE
    [] Bug = "skip_frontier_gate"
       /\ c = "not_frontier_exact" -> TRUE
    [] Bug = "allow_missing_session"
       /\ c = "session_missing" -> TRUE
    [] Bug = "skip_metadata_gate"
       /\ c = "metadata_mismatch" -> TRUE
    [] Bug = "allow_authoritative_payload"
       /\ c = "authoritative_payload_known" -> TRUE
    [] Bug = "allow_missing_expected_payload_hash"
       /\ c = "missing_expected_payload_hash" -> TRUE
    [] Bug = "ignore_block_hash"
       /\ c = "response_block_hash_mismatch" -> TRUE
    [] Bug = "ignore_height"
       /\ c = "response_height_mismatch" -> TRUE
    [] Bug = "ignore_view"
       /\ c = "response_view_mismatch" -> TRUE
    [] Bug = "ignore_payload_hash"
       /\ c = "response_payload_hash_mismatch" -> TRUE
    [] Bug = "reject_block_created"
       /\ c = "happy_block_created" -> FALSE
    [] Bug = "reject_block_sync_update"
       /\ c = "happy_block_sync_update" -> FALSE
    [] OTHER -> SpecAllow(c)

Matches(c) ==
  ActualAllow(c) = SpecAllow(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_da_gate",
       "skip_frontier_gate",
       "allow_missing_session",
       "skip_metadata_gate",
       "allow_authoritative_payload",
       "allow_missing_expected_payload_hash",
       "ignore_block_hash",
       "ignore_height",
       "ignore_view",
       "ignore_payload_hash",
       "reject_block_created",
       "reject_block_sync_update"
     }
  /\ checked = 0

RepairAdmissionMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockBodyRepairExactness ==
  /\ RepairAdmissionMatchesSpec

BlockBodyRepairCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockBodyRepairExactness

SafetyFast == BlockBodyRepairExactness

BlockCreatedAllowed ==
  Matches("happy_block_created")

BlockSyncUpdateAllowed ==
  Matches("happy_block_sync_update")

DaDisabledRejected ==
  Matches("da_disabled")

NonFrontierRejected ==
  Matches("not_frontier_exact")

SessionMissingRejected ==
  Matches("session_missing")

MetadataMismatchRejected ==
  Matches("metadata_mismatch")

AuthoritativePayloadRejected ==
  Matches("authoritative_payload_known")

MissingExpectedPayloadHashRejected ==
  Matches("missing_expected_payload_hash")

BodyBlockHashMismatchRejected ==
  Matches("response_block_hash_mismatch")

BodyHeightMismatchRejected ==
  Matches("response_height_mismatch")

BodyViewMismatchRejected ==
  Matches("response_view_mismatch")

BodyPayloadHashMismatchRejected ==
  Matches("response_payload_hash_mismatch")

====

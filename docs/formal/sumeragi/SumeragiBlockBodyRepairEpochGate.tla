---- MODULE SumeragiBlockBodyRepairEpochGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `observed_commit_qc_epoch_for_body_repair(...)`.

The helper chooses the commit-QC epoch used to upgrade same-height
`BlockCreated` body repair from payload-only recovery to commit-evidence
repair. It must prefer an exact cached commit QC, then a matching deferred
missing-payload commit QC, then a pending block whose commit QC has already
been observed with an epoch.
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
CacheEpoch == "cache_epoch"
DeferredEpoch == "deferred_epoch"
PendingEpoch == "pending_epoch"

Block == "block"
OtherBlock == "other_block"
Commit == "commit"
Prepare == "prepare"

Cases == {
  "cache_only",
  "cache_over_deferred",
  "cache_over_pending",
  "deferred_only",
  "deferred_over_pending",
  "pending_only",
  "deferred_wrong_phase",
  "deferred_hash_mismatch",
  "deferred_height_mismatch",
  "deferred_view_mismatch",
  "pending_not_observed",
  "pending_epoch_missing",
  "no_sources"
}

ResponseHash(c) == Block

ResponseHeight(c) == 4

ResponseView(c) == 1

CachedQc(c) ==
  c \in {"cache_only", "cache_over_deferred", "cache_over_pending"}

DeferredExists(c) ==
  c \in {
    "cache_over_deferred",
    "deferred_only",
    "deferred_over_pending",
    "deferred_wrong_phase",
    "deferred_hash_mismatch",
    "deferred_height_mismatch",
    "deferred_view_mismatch"
  }

DeferredPhase(c) ==
  IF c = "deferred_wrong_phase" THEN Prepare ELSE Commit

DeferredHash(c) ==
  IF c = "deferred_hash_mismatch" THEN OtherBlock ELSE ResponseHash(c)

DeferredHeight(c) ==
  IF c = "deferred_height_mismatch" THEN 5 ELSE ResponseHeight(c)

DeferredView(c) ==
  IF c = "deferred_view_mismatch" THEN 2 ELSE ResponseView(c)

DeferredMatches(c) ==
  /\ DeferredExists(c)
  /\ DeferredPhase(c) = Commit
  /\ DeferredHash(c) = ResponseHash(c)
  /\ DeferredHeight(c) = ResponseHeight(c)
  /\ DeferredView(c) = ResponseView(c)

PendingExistsForHash(c) ==
  c \in {
    "cache_over_pending",
    "deferred_over_pending",
    "pending_only",
    "pending_not_observed",
    "pending_epoch_missing"
  }

PendingCommitQcObserved(c) ==
  c # "pending_not_observed"

PendingEpochPresent(c) ==
  c # "pending_epoch_missing"

PendingMatches(c) ==
  /\ PendingExistsForHash(c)
  /\ PendingCommitQcObserved(c)
  /\ PendingEpochPresent(c)

SpecEpoch(c) ==
  IF CachedQc(c) THEN
    CacheEpoch
  ELSE IF DeferredMatches(c) THEN
    DeferredEpoch
  ELSE IF PendingMatches(c) THEN
    PendingEpoch
  ELSE
    None

ActualEpoch(c) ==
  CASE Bug = "drop_cache"
       /\ c = "cache_only" -> None
    [] Bug = "deferred_overrides_cache"
       /\ c = "cache_over_deferred" -> DeferredEpoch
    [] Bug = "pending_overrides_cache"
       /\ c = "cache_over_pending" -> PendingEpoch
    [] Bug = "drop_deferred"
       /\ c = "deferred_only" -> None
    [] Bug = "pending_overrides_deferred"
       /\ c = "deferred_over_pending" -> PendingEpoch
    [] Bug = "drop_pending"
       /\ c = "pending_only" -> None
    [] Bug = "deferred_wrong_phase_allowed"
       /\ c = "deferred_wrong_phase" -> DeferredEpoch
    [] Bug = "deferred_hash_mismatch_allowed"
       /\ c = "deferred_hash_mismatch" -> DeferredEpoch
    [] Bug = "deferred_height_mismatch_allowed"
       /\ c = "deferred_height_mismatch" -> DeferredEpoch
    [] Bug = "deferred_view_mismatch_allowed"
       /\ c = "deferred_view_mismatch" -> DeferredEpoch
    [] Bug = "pending_not_observed_allowed"
       /\ c = "pending_not_observed" -> PendingEpoch
    [] Bug = "pending_epoch_missing_allowed"
       /\ c = "pending_epoch_missing" -> PendingEpoch
    [] Bug = "no_source_allowed"
       /\ c = "no_sources" -> PendingEpoch
    [] OTHER -> SpecEpoch(c)

Matches(c) ==
  ActualEpoch(c) = SpecEpoch(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_cache",
       "deferred_overrides_cache",
       "pending_overrides_cache",
       "drop_deferred",
       "pending_overrides_deferred",
       "drop_pending",
       "deferred_wrong_phase_allowed",
       "deferred_hash_mismatch_allowed",
       "deferred_height_mismatch_allowed",
       "deferred_view_mismatch_allowed",
       "pending_not_observed_allowed",
       "pending_epoch_missing_allowed",
       "no_source_allowed"
     }
  /\ checked = 0

RepairEpochMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockBodyRepairEpochExactness ==
  RepairEpochMatchesSpec

BlockBodyRepairEpochCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockBodyRepairEpochExactness

SafetyFast == BlockBodyRepairEpochExactness

CacheReturned ==
  Matches("cache_only")

CacheBeatsDeferred ==
  Matches("cache_over_deferred")

CacheBeatsPending ==
  Matches("cache_over_pending")

DeferredReturned ==
  Matches("deferred_only")

DeferredBeatsPending ==
  Matches("deferred_over_pending")

PendingReturned ==
  Matches("pending_only")

DeferredWrongPhaseRejected ==
  Matches("deferred_wrong_phase")

DeferredHashMismatchRejected ==
  Matches("deferred_hash_mismatch")

DeferredHeightMismatchRejected ==
  Matches("deferred_height_mismatch")

DeferredViewMismatchRejected ==
  Matches("deferred_view_mismatch")

PendingNotObservedRejected ==
  Matches("pending_not_observed")

PendingEpochMissingRejected ==
  Matches("pending_epoch_missing")

NoSourceRejected ==
  Matches("no_sources")

====

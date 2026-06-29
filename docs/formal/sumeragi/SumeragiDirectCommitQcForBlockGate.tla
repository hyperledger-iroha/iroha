---- MODULE SumeragiDirectCommitQcForBlockGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `direct_commit_qc_for_block(...)`.

The helper supplies direct commit-QC companions for block-sync/body responses.
It must preserve the source order and quorum gate used by the Rust code:

- cached commit QC for the exact block wins immediately,
- without a cache hit, a world-derived commit QC wins before local vote
  formation,
- vote formation uses the exact round roster when present, otherwise the
  effective commit topology, and never skips from a non-empty exact roster to
  the fallback topology,
- vote formation only runs once the commit quorum floor
  `min_votes_for_commit().max(1)` is met,
- any locally formed result must be read back as the cached commit QC for the
  same block, height, and view.
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
  "cached_only",
  "cached_world_votes",
  "world_only",
  "world_votes",
  "primary_enough_forms",
  "primary_enough_no_form",
  "primary_under",
  "primary_under_fallback_available",
  "fallback_enough_forms",
  "fallback_enough_no_form",
  "fallback_under",
  "no_topology",
  "zero_min_zero_votes",
  "zero_min_one_vote_forms"
}

Cached(c) ==
  c \in {"cached_only", "cached_world_votes"}

WorldAvailable(c) ==
  c \in {"cached_world_votes", "world_only", "world_votes"}

PrimaryAvailable(c) ==
  c \in {
    "cached_world_votes",
    "world_votes",
    "primary_enough_forms",
    "primary_enough_no_form",
    "primary_under",
    "primary_under_fallback_available"
  }

FallbackAvailable(c) ==
  c \in {
    "primary_under_fallback_available",
    "fallback_enough_forms",
    "fallback_enough_no_form",
    "fallback_under",
    "zero_min_zero_votes",
    "zero_min_one_vote_forms"
  }

VotesMeetFloor(c) ==
  c \in {
    "cached_world_votes",
    "world_votes",
    "primary_enough_forms",
    "primary_enough_no_form",
    "fallback_enough_forms",
    "fallback_enough_no_form",
    "zero_min_one_vote_forms"
  }

FormsQc(c) ==
  c \in {
    "primary_enough_forms",
    "fallback_enough_forms",
    "zero_min_one_vote_forms"
  }

SpecWorldConsulted(c) ==
  ~Cached(c)

SpecTopologySource(c) ==
  IF Cached(c) \/ WorldAvailable(c)
  THEN "none"
  ELSE IF PrimaryAvailable(c)
  THEN "primary"
  ELSE IF FallbackAvailable(c)
  THEN "fallback"
  ELSE "none"

SpecTryForm(c) ==
  /\ ~Cached(c)
  /\ ~WorldAvailable(c)
  /\ SpecTopologySource(c) \in {"primary", "fallback"}
  /\ VotesMeetFloor(c)

SpecTryPhase(c) ==
  IF SpecTryForm(c) THEN "commit" ELSE "none"

SpecTrySubject(c) ==
  IF SpecTryForm(c) THEN "block" ELSE "none"

SpecResult(c) ==
  IF Cached(c)
  THEN "cache"
  ELSE IF WorldAvailable(c)
  THEN "world"
  ELSE IF SpecTryForm(c) /\ FormsQc(c)
  THEN "formed"
  ELSE "none"

ActualWorldConsulted(c) ==
  CASE Bug = "consult_world_after_cache"
       /\ c = "cached_only" -> TRUE
    [] OTHER -> SpecWorldConsulted(c)

ActualTopologySource(c) ==
  CASE Bug = "fallback_overrides_primary"
       /\ c = "primary_under_fallback_available" -> "fallback"
    [] Bug = "skip_fallback_topology"
       /\ c = "fallback_enough_forms" -> "none"
    [] OTHER -> SpecTopologySource(c)

ActualTryForm(c) ==
  CASE Bug = "try_after_cache"
       /\ c = "cached_world_votes" -> TRUE
    [] Bug = "try_after_world"
       /\ c = "world_votes" -> TRUE
    [] Bug = "try_without_topology"
       /\ c = "no_topology" -> TRUE
    [] Bug = "zero_min_allows_zero_votes"
       /\ c = "zero_min_zero_votes" -> TRUE
    [] Bug = "skip_try_with_enough_votes"
       /\ c = "primary_enough_forms" -> FALSE
    [] Bug = "try_with_under_quorum"
       /\ c = "primary_under" -> TRUE
    [] OTHER -> SpecTryForm(c)

ActualTryPhase(c) ==
  CASE Bug = "try_prepare_phase"
       /\ c = "primary_enough_forms" -> "prepare"
    [] ActualTryForm(c) -> "commit"
    [] OTHER -> "none"

ActualTrySubject(c) ==
  CASE Bug = "try_wrong_subject"
       /\ c = "fallback_enough_forms" -> "other"
    [] ActualTryForm(c) -> "block"
    [] OTHER -> "none"

ActualResult(c) ==
  CASE Bug = "drop_cache"
       /\ c = "cached_only" -> "none"
    [] Bug = "world_overrides_cache"
       /\ c = "cached_world_votes" -> "world"
    [] Bug = "drop_world"
       /\ c = "world_only" -> "none"
    [] Bug = "return_formed_without_cache"
       /\ c = "primary_enough_no_form" -> "formed"
    [] Bug = "drop_formed_qc"
       /\ c = "primary_enough_forms" -> "none"
    [] Cached(c) -> "cache"
    [] ~Cached(c) /\ WorldAvailable(c) -> "world"
    [] ActualTryForm(c) /\ FormsQc(c) -> "formed"
    [] OTHER -> "none"

Matches(c) ==
  /\ ActualWorldConsulted(c) = SpecWorldConsulted(c)
  /\ ActualTopologySource(c) = SpecTopologySource(c)
  /\ ActualTryForm(c) = SpecTryForm(c)
  /\ ActualTryPhase(c) = SpecTryPhase(c)
  /\ ActualTrySubject(c) = SpecTrySubject(c)
  /\ ActualResult(c) = SpecResult(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_cache",
       "world_overrides_cache",
       "try_after_cache",
       "consult_world_after_cache",
       "drop_world",
       "try_after_world",
       "fallback_overrides_primary",
       "skip_fallback_topology",
       "try_without_topology",
       "zero_min_allows_zero_votes",
       "skip_try_with_enough_votes",
       "try_with_under_quorum",
       "return_formed_without_cache",
       "drop_formed_qc",
       "try_prepare_phase",
       "try_wrong_subject"
     }
  /\ checked = 0

DirectCommitQcForBlockMatchesSpec ==
  \A c \in Cases: Matches(c)

DirectCommitQcForBlockExactness ==
  /\ DirectCommitQcForBlockMatchesSpec

DirectCommitQcForBlockCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DirectCommitQcForBlockExactness

SafetyFast == DirectCommitQcForBlockExactness

CachedQcReturned ==
  Matches("cached_only")

CachedPriority ==
  Matches("cached_world_votes")

NoTryAfterCache ==
  Matches("cached_world_votes")

NoWorldAfterCache ==
  Matches("cached_only")

WorldQcReturned ==
  Matches("world_only")

NoTryAfterWorld ==
  Matches("world_votes")

PrimaryBlocksFallback ==
  Matches("primary_under_fallback_available")

FallbackTopologyUsed ==
  Matches("fallback_enough_forms")

NoTopologyNoTry ==
  Matches("no_topology")

ZeroMinVotesFloored ==
  Matches("zero_min_zero_votes")

EnoughVotesTryForm ==
  Matches("primary_enough_forms")

UnderQuorumNoTry ==
  Matches("primary_under")

FormedOnlyAfterCacheReadback ==
  Matches("primary_enough_no_form")

FormedQcReturned ==
  Matches("primary_enough_forms")

TryUsesCommitPhase ==
  Matches("primary_enough_forms")

TryUsesBlockSubject ==
  Matches("fallback_enough_forms")

====

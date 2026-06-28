---- MODULE SumeragiSignedQuorumFetchFallbackGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `committed_signed_quorum_fetch_fallback_available`
and `signed_commit_quorum_signer_count`.

The fallback is used when a committed block can be served with signed quorum
evidence instead of a full commit QC. It must stay conservative:

- the committed block hash at the requested height must match before signer
  evidence is consulted,
- the exact round roster is used when present, otherwise the effective commit
  topology is the only fallback,
- permissioned mode requires `min_votes_for_commit().max(1)` valid signers,
- NPoS mode additionally requires a matching stake snapshot, signer peers that
  map into the topology, and a successful stake quorum check.
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
  "uncommitted_permissioned_quorum",
  "permissioned_primary_exact",
  "permissioned_primary_under",
  "permissioned_primary_under_fallback_exact",
  "permissioned_zero_min_zero",
  "permissioned_zero_min_one",
  "permissioned_fallback_exact",
  "permissioned_no_topology",
  "permissioned_invalid_signers",
  "npos_state_snapshot_quorum",
  "npos_cache_snapshot_quorum",
  "npos_no_snapshot",
  "npos_state_snapshot_false_cache_true",
  "npos_signer_peers_error",
  "npos_stake_quorum_false",
  "npos_stake_error"
}

CommittedMatch(c) ==
  c /= "uncommitted_permissioned_quorum"

IsNpos(c) ==
  c \in {
    "npos_state_snapshot_quorum",
    "npos_cache_snapshot_quorum",
    "npos_no_snapshot",
    "npos_state_snapshot_false_cache_true",
    "npos_signer_peers_error",
    "npos_stake_quorum_false",
    "npos_stake_error"
  }

PrimaryRoster(c) ==
  c \in {
    "permissioned_primary_exact",
    "permissioned_primary_under",
    "permissioned_primary_under_fallback_exact",
    "permissioned_zero_min_zero",
    "permissioned_zero_min_one",
    "permissioned_invalid_signers",
    "npos_state_snapshot_quorum",
    "npos_no_snapshot",
    "npos_state_snapshot_false_cache_true",
    "npos_signer_peers_error",
    "npos_stake_quorum_false",
    "npos_stake_error"
  }

FallbackRoster(c) ==
  c \in {
    "permissioned_primary_under_fallback_exact",
    "permissioned_fallback_exact",
    "npos_cache_snapshot_quorum"
  }

SignerValidationOk(c) ==
  c /= "permissioned_invalid_signers"

PermissionedMeetsFloor(c) ==
  c \in {
    "permissioned_primary_exact",
    "permissioned_fallback_exact",
    "permissioned_zero_min_one"
  }

StateSnapshotMatches(c) ==
  c \in {
    "npos_state_snapshot_quorum",
    "npos_state_snapshot_false_cache_true",
    "npos_signer_peers_error",
    "npos_stake_quorum_false",
    "npos_stake_error"
  }

CacheSnapshotAvailable(c) ==
  c \in {
    "npos_cache_snapshot_quorum",
    "npos_state_snapshot_false_cache_true"
  }

SignerPeersOk(c) ==
  c /= "npos_signer_peers_error"

StakeQuorumOk(c) ==
  c \in {
    "npos_state_snapshot_quorum",
    "npos_cache_snapshot_quorum"
  }

StakeQuorumError(c) ==
  c = "npos_stake_error"

SpecSignerCountConsulted(c) ==
  CommittedMatch(c)

SpecTopologySource(c) ==
  IF ~SpecSignerCountConsulted(c)
  THEN "none"
  ELSE IF PrimaryRoster(c)
  THEN "primary"
  ELSE IF FallbackRoster(c)
  THEN "fallback"
  ELSE "none"

SpecStakeSnapshotSource(c) ==
  IF ~IsNpos(c) \/ SpecTopologySource(c) = "none"
  THEN "none"
  ELSE IF StateSnapshotMatches(c)
  THEN "state"
  ELSE IF CacheSnapshotAvailable(c)
  THEN "cache"
  ELSE "none"

SpecSignerCountSome(c) ==
  /\ SpecSignerCountConsulted(c)
  /\ SpecTopologySource(c) \in {"primary", "fallback"}
  /\ SignerValidationOk(c)
  /\ IF IsNpos(c)
     THEN /\ SpecStakeSnapshotSource(c) \in {"state", "cache"}
          /\ SignerPeersOk(c)
          /\ StakeQuorumOk(c)
          /\ ~StakeQuorumError(c)
     ELSE PermissionedMeetsFloor(c)

SpecAvailable(c) ==
  CommittedMatch(c) /\ SpecSignerCountSome(c)

ActualSignerCountConsulted(c) ==
  CASE Bug = "count_on_uncommitted"
       /\ c = "uncommitted_permissioned_quorum" -> TRUE
    [] OTHER -> SpecSignerCountConsulted(c)

ActualTopologySource(c) ==
  CASE Bug = "fallback_overrides_primary"
       /\ c = "permissioned_primary_under_fallback_exact" -> "fallback"
    [] Bug = "skip_fallback_roster"
       /\ c = "permissioned_fallback_exact" -> "none"
    [] Bug = "accept_empty_topology"
       /\ c = "permissioned_no_topology" -> "fallback"
    [] OTHER -> SpecTopologySource(c)

ActualStakeSnapshotSource(c) ==
  CASE Bug = "cache_overrides_state_snapshot"
       /\ c = "npos_state_snapshot_false_cache_true" -> "cache"
    [] OTHER -> SpecStakeSnapshotSource(c)

ActualSignerCountSome(c) ==
  CASE Bug = "drop_permissioned_quorum"
       /\ c = "permissioned_primary_exact" -> FALSE
    [] Bug = "accept_permissioned_under_quorum"
       /\ c = "permissioned_primary_under" -> TRUE
    [] Bug = "zero_min_not_floored"
       /\ c = "permissioned_zero_min_zero" -> TRUE
    [] Bug = "accept_invalid_signers"
       /\ c = "permissioned_invalid_signers" -> TRUE
    [] Bug = "drop_npos_state_snapshot"
       /\ c = "npos_state_snapshot_quorum" -> FALSE
    [] Bug = "drop_npos_cache_snapshot"
       /\ c = "npos_cache_snapshot_quorum" -> FALSE
    [] Bug = "accept_npos_without_snapshot"
       /\ c = "npos_no_snapshot" -> TRUE
    [] Bug = "accept_npos_signer_peers_error"
       /\ c = "npos_signer_peers_error" -> TRUE
    [] Bug = "accept_npos_stake_quorum_false"
       /\ c = "npos_stake_quorum_false" -> TRUE
    [] Bug = "accept_npos_stake_error"
       /\ c = "npos_stake_error" -> TRUE
    [] OTHER ->
       /\ ActualSignerCountConsulted(c)
       /\ ActualTopologySource(c) \in {"primary", "fallback"}
       /\ SignerValidationOk(c)
       /\ IF IsNpos(c)
          THEN /\ ActualStakeSnapshotSource(c) \in {"state", "cache"}
               /\ SignerPeersOk(c)
               /\ (StakeQuorumOk(c)
                   \/ (Bug = "cache_overrides_state_snapshot"
                       /\ c = "npos_state_snapshot_false_cache_true"))
               /\ ~StakeQuorumError(c)
          ELSE PermissionedMeetsFloor(c)

ActualAvailable(c) ==
  CASE Bug = "ignore_committed_hash"
       /\ c = "uncommitted_permissioned_quorum" -> TRUE
    [] OTHER -> CommittedMatch(c) /\ ActualSignerCountSome(c)

Matches(c) ==
  /\ ActualSignerCountConsulted(c) = SpecSignerCountConsulted(c)
  /\ ActualTopologySource(c) = SpecTopologySource(c)
  /\ ActualStakeSnapshotSource(c) = SpecStakeSnapshotSource(c)
  /\ ActualSignerCountSome(c) = SpecSignerCountSome(c)
  /\ ActualAvailable(c) = SpecAvailable(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "ignore_committed_hash",
       "count_on_uncommitted",
       "drop_permissioned_quorum",
       "accept_permissioned_under_quorum",
       "zero_min_not_floored",
       "fallback_overrides_primary",
       "skip_fallback_roster",
       "accept_empty_topology",
       "accept_invalid_signers",
       "drop_npos_state_snapshot",
       "drop_npos_cache_snapshot",
       "accept_npos_without_snapshot",
       "cache_overrides_state_snapshot",
       "accept_npos_signer_peers_error",
       "accept_npos_stake_quorum_false",
       "accept_npos_stake_error"
     }
  /\ checked = 0

SignedQuorumFetchFallbackMatchesSpec ==
  \A c \in Cases: Matches(c)

SignedQuorumFetchFallbackExactness ==
  /\ SignedQuorumFetchFallbackMatchesSpec

SignedQuorumFetchFallbackCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SignedQuorumFetchFallbackExactness

SafetyFast ==
  SignedQuorumFetchFallbackExactness

CommittedHashGate ==
  Matches("uncommitted_permissioned_quorum")

NoSignerCountOnUncommitted ==
  Matches("uncommitted_permissioned_quorum")

PermissionedQuorumAccepted ==
  Matches("permissioned_primary_exact")

PermissionedUnderRejected ==
  Matches("permissioned_primary_under")

ZeroMinFloored ==
  Matches("permissioned_zero_min_zero")

PrimaryBlocksFallback ==
  Matches("permissioned_primary_under_fallback_exact")

FallbackRosterUsed ==
  Matches("permissioned_fallback_exact")

EmptyTopologyRejected ==
  Matches("permissioned_no_topology")

InvalidSignersRejected ==
  Matches("permissioned_invalid_signers")

NposStateSnapshotAccepted ==
  Matches("npos_state_snapshot_quorum")

NposCacheSnapshotAccepted ==
  Matches("npos_cache_snapshot_quorum")

NposMissingSnapshotRejected ==
  Matches("npos_no_snapshot")

NposStateSnapshotPriority ==
  Matches("npos_state_snapshot_false_cache_true")

NposSignerPeersErrorRejected ==
  Matches("npos_signer_peers_error")

NposStakeQuorumFalseRejected ==
  Matches("npos_stake_quorum_false")

NposStakeErrorRejected ==
  Matches("npos_stake_error")

====

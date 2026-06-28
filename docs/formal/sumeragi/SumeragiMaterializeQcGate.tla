---- MODULE SumeragiMaterializeQcGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for QC materialization.

This slice captures `Actor::materialize_qc_for_header(...)` and the local
`recover_qc_from_kura_block(...)` fallback it uses. It abstracts signatures,
roots, and stake arithmetic to symbolic cases while preserving the helper
contract: existing cached QCs win immediately; empty rosters may only recover a
commit QC from Kura; non-empty rosters first try local vote formation, then Kura
recovery, then local signer aggregation; NPoS requires a stake roster and stake
quorum; commit-QC root filtering may shrink signers to zero; under-quorum,
signature-aggregation, and canonical mapping failures are fail-closed; and
recovered/rebuilt QCs are cached.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CachedExisting == "cached_existing"
EmptyRosterRecovery == "empty_roster_recovery"
EmptyRosterNoRecovery == "empty_roster_no_recovery"
FormedFromVotes == "formed_from_votes"
RecoverAfterFormMiss == "recover_after_form_miss"
NposMissingStakeRoster == "npos_missing_stake_roster"
CommitRootFilterEmpty == "commit_root_filter_empty"
NoVotes == "no_votes"
PermissionedUnderQuorum == "permissioned_under_quorum"
PermissionedQuorum == "permissioned_quorum"
PrepareQuorum == "prepare_quorum"
NposSignerMapError == "npos_signer_map_error"
NposStakeQuorumFalse == "npos_stake_quorum_false"
AggregateError == "aggregate_error"
CanonicalMappingIncomplete == "canonical_mapping_incomplete"

Cases == {
  CachedExisting,
  EmptyRosterRecovery,
  EmptyRosterNoRecovery,
  FormedFromVotes,
  RecoverAfterFormMiss,
  NposMissingStakeRoster,
  CommitRootFilterEmpty,
  NoVotes,
  PermissionedUnderQuorum,
  PermissionedQuorum,
  PrepareQuorum,
  NposSignerMapError,
  NposStakeQuorumFalse,
  AggregateError,
  CanonicalMappingIncomplete
}

NoneQc == "none"
CachedQc == "cached_qc"
RecoveredQc == "recovered_qc"
FormedQc == "formed_qc"
RebuiltQc == "rebuilt_qc"

QcValues == {NoneQc, CachedQc, RecoveredQc, FormedQc, RebuiltQc}

HasCached(c) == c = CachedExisting

EmptyRoster(c) ==
  c \in {EmptyRosterRecovery, EmptyRosterNoRecovery}

HasFormedAfterTry(c) ==
  c = FormedFromVotes

HasKuraRecovery(c) ==
  c \in {EmptyRosterRecovery, RecoverAfterFormMiss}

IsNpos(c) ==
  c \in {NposMissingStakeRoster, NposSignerMapError, NposStakeQuorumFalse}

IsCommit(c) ==
  c # PrepareQuorum

SpecTryFormVotes(c) ==
  ~HasCached(c) /\ ~EmptyRoster(c)

SpecAttemptsKuraRecovery(c) ==
  ~HasCached(c)
    /\ (EmptyRoster(c) \/ (~HasFormedAfterTry(c) /\ c = RecoverAfterFormMiss))

SpecResult(c) ==
  CASE c = CachedExisting -> CachedQc
    [] c \in {EmptyRosterRecovery, RecoverAfterFormMiss} -> RecoveredQc
    [] c = FormedFromVotes -> FormedQc
    [] c \in {PermissionedQuorum, PrepareQuorum} -> RebuiltQc
    [] OTHER -> NoneQc

SpecCachesMaterializedQc(c) ==
  SpecResult(c) \in {RecoveredQc, FormedQc, RebuiltQc}

ActualTryFormVotes(c) ==
  CASE Bug = "empty_roster_forms_votes"
       /\ c = EmptyRosterNoRecovery ->
      TRUE
    [] Bug = "nonempty_skips_try_form"
       /\ c = FormedFromVotes ->
      FALSE
    [] OTHER -> SpecTryFormVotes(c)

ActualAttemptsKuraRecovery(c) ==
  CASE Bug = "empty_roster_skips_recovery"
       /\ c = EmptyRosterRecovery ->
      FALSE
    [] Bug = "recovery_after_form_miss_skipped"
       /\ c = RecoverAfterFormMiss ->
      FALSE
    [] OTHER -> SpecAttemptsKuraRecovery(c)

ActualResult(c) ==
  CASE Bug = "ignore_cached_existing"
       /\ c = CachedExisting ->
      RebuiltQc
    [] Bug = "empty_roster_skips_recovery"
       /\ c = EmptyRosterRecovery ->
      NoneQc
    [] Bug = "formed_votes_ignored"
       /\ c = FormedFromVotes ->
      NoneQc
    [] Bug = "recovery_after_form_miss_skipped"
       /\ c = RecoverAfterFormMiss ->
      NoneQc
    [] Bug = "npos_missing_stake_accepted"
       /\ c = NposMissingStakeRoster ->
      RebuiltQc
    [] Bug = "commit_root_filter_skipped"
       /\ c = CommitRootFilterEmpty ->
      RebuiltQc
    [] Bug = "no_votes_accepted"
       /\ c = NoVotes ->
      RebuiltQc
    [] Bug = "permissioned_under_quorum_accepted"
       /\ c = PermissionedUnderQuorum ->
      RebuiltQc
    [] Bug = "npos_signer_map_error_accepted"
       /\ c = NposSignerMapError ->
      RebuiltQc
    [] Bug = "npos_stake_quorum_false_accepted"
       /\ c = NposStakeQuorumFalse ->
      RebuiltQc
    [] Bug = "aggregate_error_accepted"
       /\ c = AggregateError ->
      RebuiltQc
    [] Bug = "canonical_mismatch_accepted"
       /\ c = CanonicalMappingIncomplete ->
      RebuiltQc
    [] Bug = "prepare_quorum_rejected"
       /\ c = PrepareQuorum ->
      NoneQc
    [] OTHER -> SpecResult(c)

ActualCachesMaterializedQc(c) ==
  CASE Bug = "skip_cache_recovered"
       /\ c = RecoverAfterFormMiss ->
      FALSE
    [] Bug = "skip_cache_rebuilt"
       /\ c = PermissionedQuorum ->
      FALSE
    [] OTHER -> SpecCachesMaterializedQc(c)

Bugs == {
  "none",
  "ignore_cached_existing",
  "empty_roster_skips_recovery",
  "empty_roster_forms_votes",
  "nonempty_skips_try_form",
  "formed_votes_ignored",
  "recovery_after_form_miss_skipped",
  "npos_missing_stake_accepted",
  "commit_root_filter_skipped",
  "no_votes_accepted",
  "permissioned_under_quorum_accepted",
  "npos_signer_map_error_accepted",
  "npos_stake_quorum_false_accepted",
  "aggregate_error_accepted",
  "canonical_mismatch_accepted",
  "skip_cache_recovered",
  "skip_cache_rebuilt",
  "prepare_quorum_rejected"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in QcValues
       /\ ActualResult(c) \in QcValues
       /\ SpecTryFormVotes(c) \in BOOLEAN
       /\ ActualTryFormVotes(c) \in BOOLEAN
       /\ SpecAttemptsKuraRecovery(c) \in BOOLEAN
       /\ ActualAttemptsKuraRecovery(c) \in BOOLEAN
       /\ SpecCachesMaterializedQc(c) \in BOOLEAN
       /\ ActualCachesMaterializedQc(c) \in BOOLEAN

ResultMatchesSpec ==
  \A c \in Cases:
    ActualResult(c) = SpecResult(c)

TryFormMatchesSpec ==
  \A c \in Cases:
    ActualTryFormVotes(c) = SpecTryFormVotes(c)

KuraRecoveryMatchesSpec ==
  \A c \in Cases:
    ActualAttemptsKuraRecovery(c) = SpecAttemptsKuraRecovery(c)

CacheInsertionMatchesSpec ==
  \A c \in Cases:
    ActualCachesMaterializedQc(c) = SpecCachesMaterializedQc(c)

FailClosedCasesStayNone ==
  \A c \in {EmptyRosterNoRecovery, NposMissingStakeRoster,
            CommitRootFilterEmpty, NoVotes, PermissionedUnderQuorum,
            NposSignerMapError, NposStakeQuorumFalse, AggregateError,
            CanonicalMappingIncomplete}:
    ActualResult(c) = NoneQc

MaterializeQcCoreSafety ==
  /\ ResultMatchesSpec
  /\ TryFormMatchesSpec
  /\ KuraRecoveryMatchesSpec
  /\ CacheInsertionMatchesSpec
  /\ FailClosedCasesStayNone

MaterializeQcExactness ==
  /\ ResultMatchesSpec
  /\ TryFormMatchesSpec
  /\ KuraRecoveryMatchesSpec
  /\ CacheInsertionMatchesSpec
  /\ FailClosedCasesStayNone
MaterializeQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MaterializeQcExactness

NoBugInvariant == MaterializeQcExactness

SafetyFast == MaterializeQcExactness

BugIgnoreCachedExisting == NoBugInvariant
BugEmptyRosterSkipsRecovery == NoBugInvariant
BugEmptyRosterFormsVotes == NoBugInvariant
BugNonemptySkipsTryForm == NoBugInvariant
BugFormedVotesIgnored == NoBugInvariant
BugRecoveryAfterFormMissSkipped == NoBugInvariant
BugNposMissingStakeAccepted == NoBugInvariant
BugCommitRootFilterSkipped == NoBugInvariant
BugNoVotesAccepted == NoBugInvariant
BugPermissionedUnderQuorumAccepted == NoBugInvariant
BugNposSignerMapErrorAccepted == NoBugInvariant
BugNposStakeQuorumFalseAccepted == NoBugInvariant
BugAggregateErrorAccepted == NoBugInvariant
BugCanonicalMismatchAccepted == NoBugInvariant
BugSkipCacheRecovered == NoBugInvariant
BugSkipCacheRebuilt == NoBugInvariant
BugPrepareQuorumRejected == NoBugInvariant

====

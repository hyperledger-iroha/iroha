---- MODULE SumeragiEmbeddedQcRosterGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for embedded-QC roster bootstrapping.

This slice models `certified_embedded_qc_roster(...)` and
`try_bootstrap_qc_validation_from_embedded_roster(...)`, the fallback used when
normal cached-roster QC validation fails with `ValidatorSetMismatch`.  The
embedded roster may drive catch-up only when the QC advertises a non-empty V1
validator set with a matching hash, the mode tag matches, non-NewView QCs do
not carry a highest-QC reference, the advertised roster is anchored to an
authoritative topology candidate, every validator has cached proof of
possession, and quorum holds under the active consensus mode.  NPoS additionally
requires a matching stake snapshot and stake quorum.  After certification, QC
validation or aggregate recovery must still succeed before the caller replaces
the stale vote-roster cache and uses the QC for missing-payload recovery.
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
  "valid_permissioned_active",
  "valid_permissioned_live",
  "valid_permissioned_cached",
  "valid_permissioned_round_roster",
  "valid_npos_hint_snapshot",
  "valid_npos_cached_snapshot",
  "valid_validation_recovered",
  "mode_tag_mismatch",
  "empty_validator_set",
  "hash_version_mismatch",
  "validator_hash_mismatch",
  "non_new_view_highest_qc",
  "unanchored_roster",
  "missing_pop",
  "permissioned_under_quorum",
  "npos_missing_snapshot",
  "npos_snapshot_wrong_roster",
  "npos_signer_map_error",
  "npos_stake_quorum_false",
  "aggregate_inconsistent",
  "validation_error_no_recovery"
}

NposCases == {
  "valid_npos_hint_snapshot",
  "valid_npos_cached_snapshot",
  "npos_missing_snapshot",
  "npos_snapshot_wrong_roster",
  "npos_signer_map_error",
  "npos_stake_quorum_false"
}

ConsensusMode(c) ==
  IF c \in NposCases THEN "Npos" ELSE "Permissioned"

ModeTagMatches(c) ==
  c # "mode_tag_mismatch"

ValidatorSetNonempty(c) ==
  c # "empty_validator_set"

HashVersionV1(c) ==
  c # "hash_version_mismatch"

ValidatorHashMatches(c) ==
  c # "validator_hash_mismatch"

HighestQcShapeAllowed(c) ==
  c # "non_new_view_highest_qc"

AuthoritativeAnchored(c) ==
  c # "unanchored_roster"

AllPopsPresent(c) ==
  c # "missing_pop"

PermissionedQuorum(c) ==
  c # "permissioned_under_quorum"

NposSnapshotAvailable(c) ==
  c \in {
    "valid_npos_hint_snapshot",
    "valid_npos_cached_snapshot",
    "npos_signer_map_error",
    "npos_stake_quorum_false"
  }

NposSnapshotMatches(c) ==
  c # "npos_snapshot_wrong_roster"

NposSignerMapOk(c) ==
  c # "npos_signer_map_error"

NposStakeQuorum(c) ==
  c # "npos_stake_quorum_false"

AggregateConsistent(c) ==
  c # "aggregate_inconsistent"

ValidationOk(c) ==
  c # "validation_error_no_recovery" /\ c # "valid_validation_recovered"

AggregateRecoveryOk(c) ==
  c = "valid_validation_recovered"

SpecCertified(c) ==
  /\ ModeTagMatches(c)
  /\ ValidatorSetNonempty(c)
  /\ HashVersionV1(c)
  /\ ValidatorHashMatches(c)
  /\ HighestQcShapeAllowed(c)
  /\ AuthoritativeAnchored(c)
  /\ AllPopsPresent(c)
  /\ AggregateConsistent(c)
  /\ IF ConsensusMode(c) = "Permissioned"
     THEN PermissionedQuorum(c)
     ELSE
       /\ NposSnapshotAvailable(c)
       /\ NposSnapshotMatches(c)
       /\ NposSignerMapOk(c)
       /\ NposStakeQuorum(c)

SpecBootstrapSucceeds(c) ==
  /\ SpecCertified(c)
  /\ (ValidationOk(c) \/ AggregateRecoveryOk(c))

SpecStakeSnapshotReturned(c) ==
  /\ SpecBootstrapSucceeds(c)
  /\ ConsensusMode(c) = "Npos"

SpecCacheReplaced(c) ==
  SpecBootstrapSucceeds(c)

SpecPayloadRecoveryDeferred(c) ==
  SpecBootstrapSucceeds(c)

SpecGenericMissingRequestArmed(c) ==
  FALSE

ActualCertified(c) ==
  CASE Bug = "accept_mode_tag_mismatch"
       /\ c = "mode_tag_mismatch" -> TRUE
    [] Bug = "accept_empty_validator_set"
       /\ c = "empty_validator_set" -> TRUE
    [] Bug = "accept_hash_version_mismatch"
       /\ c = "hash_version_mismatch" -> TRUE
    [] Bug = "accept_validator_hash_mismatch"
       /\ c = "validator_hash_mismatch" -> TRUE
    [] Bug = "accept_non_new_view_highest_qc"
       /\ c = "non_new_view_highest_qc" -> TRUE
    [] Bug = "accept_unanchored_roster"
       /\ c = "unanchored_roster" -> TRUE
    [] Bug = "accept_missing_pop"
       /\ c = "missing_pop" -> TRUE
    [] Bug = "accept_permissioned_under_quorum"
       /\ c = "permissioned_under_quorum" -> TRUE
    [] Bug = "accept_npos_missing_snapshot"
       /\ c = "npos_missing_snapshot" -> TRUE
    [] Bug = "accept_npos_wrong_snapshot"
       /\ c = "npos_snapshot_wrong_roster" -> TRUE
    [] Bug = "accept_npos_signer_map_error"
       /\ c = "npos_signer_map_error" -> TRUE
    [] Bug = "accept_npos_stake_quorum_false"
       /\ c = "npos_stake_quorum_false" -> TRUE
    [] Bug = "accept_bad_aggregate"
       /\ c = "aggregate_inconsistent" -> TRUE
    [] Bug = "reject_valid_permissioned"
       /\ c \in {
            "valid_permissioned_active",
            "valid_permissioned_live",
            "valid_permissioned_cached",
            "valid_permissioned_round_roster"
          } -> FALSE
    [] Bug = "reject_valid_npos"
       /\ c \in {"valid_npos_hint_snapshot", "valid_npos_cached_snapshot"} -> FALSE
    [] OTHER -> SpecCertified(c)

ActualValidationOrRecoveryOk(c) ==
  CASE Bug = "accept_validation_error_without_recovery"
       /\ c = "validation_error_no_recovery" -> TRUE
    [] Bug = "reject_recovered_validation"
       /\ c = "valid_validation_recovered" -> FALSE
    [] OTHER -> ValidationOk(c) \/ AggregateRecoveryOk(c)

ActualBootstrapSucceeds(c) ==
  ActualCertified(c) /\ ActualValidationOrRecoveryOk(c)

ActualStakeSnapshotReturned(c) ==
  CASE Bug = "return_stake_for_permissioned"
       /\ ActualBootstrapSucceeds(c)
       /\ ConsensusMode(c) = "Permissioned" -> TRUE
    [] Bug = "drop_npos_stake_snapshot"
       /\ ActualBootstrapSucceeds(c)
       /\ ConsensusMode(c) = "Npos" -> FALSE
    [] OTHER -> ActualBootstrapSucceeds(c) /\ ConsensusMode(c) = "Npos"

ActualCacheReplaced(c) ==
  CASE Bug = "skip_cache_replace_success"
       /\ ActualBootstrapSucceeds(c) -> FALSE
    [] Bug = "replace_cache_on_reject"
       /\ ~ActualBootstrapSucceeds(c)
       /\ c = "unanchored_roster" -> TRUE
    [] OTHER -> ActualBootstrapSucceeds(c)

ActualPayloadRecoveryDeferred(c) ==
  CASE Bug = "skip_payload_recovery_success"
       /\ ActualBootstrapSucceeds(c) -> FALSE
    [] Bug = "payload_recovery_on_reject"
       /\ ~ActualBootstrapSucceeds(c)
       /\ c = "missing_pop" -> TRUE
    [] OTHER -> ActualBootstrapSucceeds(c)

ActualGenericMissingRequestArmed(c) ==
  Bug = "generic_request_on_success" /\ ActualBootstrapSucceeds(c)

Matches(c) ==
  /\ ActualCertified(c) = SpecCertified(c)
  /\ ActualBootstrapSucceeds(c) = SpecBootstrapSucceeds(c)
  /\ ActualStakeSnapshotReturned(c) = SpecStakeSnapshotReturned(c)
  /\ ActualCacheReplaced(c) = SpecCacheReplaced(c)
  /\ ActualPayloadRecoveryDeferred(c) = SpecPayloadRecoveryDeferred(c)
  /\ ActualGenericMissingRequestArmed(c) = SpecGenericMissingRequestArmed(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "accept_mode_tag_mismatch",
       "accept_empty_validator_set",
       "accept_hash_version_mismatch",
       "accept_validator_hash_mismatch",
       "accept_non_new_view_highest_qc",
       "accept_unanchored_roster",
       "accept_missing_pop",
       "accept_permissioned_under_quorum",
       "accept_npos_missing_snapshot",
       "accept_npos_wrong_snapshot",
       "accept_npos_signer_map_error",
       "accept_npos_stake_quorum_false",
       "accept_bad_aggregate",
       "accept_validation_error_without_recovery",
       "reject_valid_permissioned",
       "reject_valid_npos",
       "reject_recovered_validation",
       "return_stake_for_permissioned",
       "drop_npos_stake_snapshot",
       "skip_cache_replace_success",
       "replace_cache_on_reject",
       "skip_payload_recovery_success",
       "payload_recovery_on_reject",
       "generic_request_on_success"
     }
  /\ checked = 0

AllCasesMatchSpec ==
  \A c \in Cases: Matches(c)

EmbeddedQcRosterExactness ==
  /\ AllCasesMatchSpec

EmbeddedQcRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EmbeddedQcRosterExactness

Safety ==
  EmbeddedQcRosterExactness

ValidPermissionedBootstraps ==
  Matches("valid_permissioned_active")

ValidNposBootstraps ==
  Matches("valid_npos_hint_snapshot")

UnanchoredRosterRejected ==
  Matches("unanchored_roster")

MissingPopRejected ==
  Matches("missing_pop")

NposStakeGateFailClosed ==
  /\ Matches("npos_missing_snapshot")
  /\ Matches("npos_snapshot_wrong_roster")
  /\ Matches("npos_stake_quorum_false")

ValidationRecoveryRequired ==
  /\ Matches("valid_validation_recovered")
  /\ Matches("validation_error_no_recovery")

=============================================================================
====

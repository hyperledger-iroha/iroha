---- MODULE SumeragiPrecommitSignerRecordGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for cached-QC precommit signer record construction.

This slice models `precommit_signer_record_from_cached_qc(...)`. A cached QC
can seed precommit-signer history only when the commit topology is nonempty,
the signer bitmap parses for that topology, the aggregate signature is present,
and the relevant quorum policy is satisfied. Permissioned mode uses the
commit-quorum count and drops any stake snapshot; NPoS mode requires a present
stake snapshot, all parsed signer indexes to resolve into the commit topology,
and a successful stake-quorum check while preserving the snapshot in the
record.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "permissioned_valid_snapshot_input",
  "permissioned_below_quorum",
  "permissioned_len_one",
  "empty_topology",
  "invalid_bitmap",
  "empty_aggregate",
  "npos_valid",
  "npos_missing_snapshot",
  "npos_below_stake",
  "npos_snapshot_error",
  "npos_oob_signer"
}

PermissionedCases == {
  "permissioned_valid_snapshot_input",
  "permissioned_below_quorum",
  "permissioned_len_one"
}

CommonRejectCases == {
  "empty_topology",
  "invalid_bitmap",
  "empty_aggregate"
}

NposStakeCases == {
  "npos_valid",
  "npos_missing_snapshot",
  "npos_below_stake",
  "npos_snapshot_error",
  "npos_oob_signer"
}

SnapshotPolicyCases == {
  "permissioned_valid_snapshot_input",
  "npos_valid"
}

AcceptedMetadataCases == {
  "permissioned_valid_snapshot_input",
  "permissioned_len_one",
  "npos_valid"
}

Mode(c) ==
  IF c \in {
    "npos_valid",
    "npos_missing_snapshot",
    "npos_below_stake",
    "npos_snapshot_error",
    "npos_oob_signer"
  } THEN
    "Npos"
  ELSE
    "Permissioned"

TopologyLen(c) ==
  CASE c = "empty_topology" -> 0
    [] c = "permissioned_len_one" -> 1
    [] OTHER -> 4

ParsedOk(c) ==
  c # "invalid_bitmap"

AggregateNonEmpty(c) ==
  c # "empty_aggregate"

ParsedVotingCount(c) ==
  CASE c = "permissioned_below_quorum" -> 2
    [] c = "empty_topology" -> 0
    [] c = "invalid_bitmap" -> 0
    [] c = "permissioned_len_one" -> 1
    [] OTHER -> 3

PermissionedRequired(c) ==
  CASE TopologyLen(c) = 0 -> 1
    [] TopologyLen(c) = 1 -> 1
    [] TopologyLen(c) = 2 -> 2
    [] TopologyLen(c) = 3 -> 3
    [] OTHER -> 3

StakeSnapshotInput(c) ==
  c # "npos_missing_snapshot"

StakeQuorumOk(c) ==
  c \in {"npos_valid"}

StakeQuorumError(c) ==
  c = "npos_snapshot_error"

ParsedSignersInRange(c) ==
  c # "npos_oob_signer"

SpecAccepted(c) ==
  TopologyLen(c) > 0
    /\ ParsedOk(c)
    /\ AggregateNonEmpty(c)
    /\ IF Mode(c) = "Permissioned" THEN
         ParsedVotingCount(c) >= PermissionedRequired(c)
       ELSE
         StakeSnapshotInput(c)
           /\ ParsedSignersInRange(c)
           /\ StakeQuorumOk(c)
           /\ ~StakeQuorumError(c)

SpecSnapshotAttached(c) ==
  SpecAccepted(c) /\ Mode(c) = "Npos"

\* @type: Str => <<Bool, Bool, Int, Int>>;
SpecOutput(c) ==
  IF SpecAccepted(c) THEN
    <<TRUE, SpecSnapshotAttached(c), TopologyLen(c), ParsedVotingCount(c)>>
  ELSE
    <<FALSE, FALSE, 0, 0>>

\* @type: Str => <<Bool, Bool, Int, Int>>;
ActualOutput(c) ==
  CASE Bug = "drop_permissioned_valid"
       /\ c = "permissioned_valid_snapshot_input" -> <<FALSE, FALSE, 0, 0>>
    [] Bug = "accept_empty_topology"
       /\ c = "empty_topology" -> <<TRUE, FALSE, 0, 0>>
    [] Bug = "accept_invalid_bitmap"
       /\ c = "invalid_bitmap" -> <<TRUE, FALSE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "accept_empty_aggregate"
       /\ c = "empty_aggregate" -> <<TRUE, FALSE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "accept_permissioned_below_quorum"
       /\ c = "permissioned_below_quorum" -> <<TRUE, FALSE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "permissioned_preserves_stake_snapshot"
       /\ c = "permissioned_valid_snapshot_input" -> <<TRUE, TRUE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "drop_npos_valid"
       /\ c = "npos_valid" -> <<FALSE, FALSE, 0, 0>>
    [] Bug = "accept_npos_missing_snapshot"
       /\ c = "npos_missing_snapshot" -> <<TRUE, TRUE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "accept_npos_below_stake"
       /\ c = "npos_below_stake" -> <<TRUE, TRUE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "accept_npos_snapshot_error"
       /\ c = "npos_snapshot_error" -> <<TRUE, TRUE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "accept_npos_oob_signer"
       /\ c = "npos_oob_signer" -> <<TRUE, TRUE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "npos_drops_snapshot"
       /\ c = "npos_valid" -> <<TRUE, FALSE, TopologyLen(c), ParsedVotingCount(c)>>
    [] Bug = "wrong_roster_len"
       /\ c = "npos_valid" -> <<TRUE, TRUE, TopologyLen(c) + 1, ParsedVotingCount(c)>>
    [] Bug = "wrong_signer_count"
       /\ c = "npos_valid" -> <<TRUE, TRUE, TopologyLen(c), ParsedVotingCount(c) - 1>>
    [] OTHER -> SpecOutput(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "drop_permissioned_valid",
       "accept_empty_topology",
       "accept_invalid_bitmap",
       "accept_empty_aggregate",
       "accept_permissioned_below_quorum",
       "permissioned_preserves_stake_snapshot",
       "drop_npos_valid",
       "accept_npos_missing_snapshot",
       "accept_npos_below_stake",
       "accept_npos_snapshot_error",
       "accept_npos_oob_signer",
       "npos_drops_snapshot",
       "wrong_roster_len",
       "wrong_signer_count"
     }

SpecAdmissionAnchors ==
  /\ SpecAccepted("permissioned_valid_snapshot_input")
  /\ ~SpecAccepted("permissioned_below_quorum")
  /\ SpecAccepted("permissioned_len_one")
  /\ ~SpecAccepted("empty_topology")
  /\ ~SpecAccepted("invalid_bitmap")
  /\ ~SpecAccepted("empty_aggregate")
  /\ SpecAccepted("npos_valid")
  /\ ~SpecAccepted("npos_missing_snapshot")
  /\ ~SpecAccepted("npos_below_stake")
  /\ ~SpecAccepted("npos_snapshot_error")
  /\ ~SpecAccepted("npos_oob_signer")

SpecSnapshotPolicy ==
  /\ \A c \in Cases:
       Mode(c) = "Permissioned" => ~SpecSnapshotAttached(c)
  /\ \A c \in Cases:
       Mode(c) = "Npos" => (SpecSnapshotAttached(c) = SpecAccepted(c))

SpecAcceptanceRequiresCommonInputs ==
  \A c \in Cases:
    SpecAccepted(c) =>
      /\ TopologyLen(c) > 0
      /\ ParsedOk(c)
      /\ AggregateNonEmpty(c)

SpecNposAcceptanceRequiresStakeInputs ==
  \A c \in Cases:
    Mode(c) = "Npos" /\ SpecAccepted(c) =>
      /\ StakeSnapshotInput(c)
      /\ ParsedSignersInRange(c)
      /\ StakeQuorumOk(c)
      /\ ~StakeQuorumError(c)

SpecOutputAnchors ==
  /\ SpecOutput("permissioned_valid_snapshot_input") = <<TRUE, FALSE, 4, 3>>
  /\ SpecOutput("permissioned_below_quorum") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("permissioned_len_one") = <<TRUE, FALSE, 1, 1>>
  /\ SpecOutput("empty_topology") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("invalid_bitmap") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("empty_aggregate") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("npos_valid") = <<TRUE, TRUE, 4, 3>>
  /\ SpecOutput("npos_missing_snapshot") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("npos_below_stake") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("npos_snapshot_error") = <<FALSE, FALSE, 0, 0>>
  /\ SpecOutput("npos_oob_signer") = <<FALSE, FALSE, 0, 0>>

SafetyFast ==
  \A c \in Cases: ActualOutput(c) = SpecOutput(c)

PrecommitSignerPermissionedExact ==
  \A c \in PermissionedCases:
    ActualOutput(c) = SpecOutput(c)

PrecommitSignerCommonRejectExact ==
  \A c \in CommonRejectCases:
    ActualOutput(c) = SpecOutput(c)

PrecommitSignerNposStakeExact ==
  \A c \in NposStakeCases:
    ActualOutput(c) = SpecOutput(c)

PrecommitSignerSnapshotPolicyExact ==
  \A c \in SnapshotPolicyCases:
    ActualOutput(c) = SpecOutput(c)

PrecommitSignerOutputMetadataExact ==
  \A c \in AcceptedMetadataCases:
    ActualOutput(c) = SpecOutput(c)

PrecommitSignerRecordExactness ==
  /\ PrecommitSignerPermissionedExact
  /\ PrecommitSignerCommonRejectExact
  /\ PrecommitSignerNposStakeExact
  /\ PrecommitSignerSnapshotPolicyExact
  /\ PrecommitSignerOutputMetadataExact

BugDropPermissionedValid ==
  ActualOutput("permissioned_valid_snapshot_input") =
    SpecOutput("permissioned_valid_snapshot_input")

BugAcceptEmptyTopology ==
  ActualOutput("empty_topology") = SpecOutput("empty_topology")

BugAcceptInvalidBitmap ==
  ActualOutput("invalid_bitmap") = SpecOutput("invalid_bitmap")

BugAcceptEmptyAggregate ==
  ActualOutput("empty_aggregate") = SpecOutput("empty_aggregate")

BugAcceptPermissionedBelowQuorum ==
  ActualOutput("permissioned_below_quorum") =
    SpecOutput("permissioned_below_quorum")

BugPermissionedPreservesStakeSnapshot ==
  ActualOutput("permissioned_valid_snapshot_input") =
    SpecOutput("permissioned_valid_snapshot_input")

BugDropNposValid ==
  ActualOutput("npos_valid") = SpecOutput("npos_valid")

BugAcceptNposMissingSnapshot ==
  ActualOutput("npos_missing_snapshot") = SpecOutput("npos_missing_snapshot")

BugAcceptNposBelowStake ==
  ActualOutput("npos_below_stake") = SpecOutput("npos_below_stake")

BugAcceptNposSnapshotError ==
  ActualOutput("npos_snapshot_error") = SpecOutput("npos_snapshot_error")

BugAcceptNposOobSigner ==
  ActualOutput("npos_oob_signer") = SpecOutput("npos_oob_signer")

BugNposDropsSnapshot ==
  ActualOutput("npos_valid") = SpecOutput("npos_valid")

BugWrongRosterLen ==
  ActualOutput("npos_valid") = SpecOutput("npos_valid")

BugWrongSignerCount ==
  ActualOutput("npos_valid") = SpecOutput("npos_valid")

====

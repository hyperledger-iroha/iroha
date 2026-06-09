---- MODULE SumeragiQuorumRetransmitTargetsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `quorum_retransmit_targets_for_missing_votes`.

The helper maps observed commit-vote signer indices through the view-aligned
topology back to canonical peer ids, targets missing remote voters by default,
fans out to every remote peer when one more vote would reach commit quorum, and
falls back to every remote peer if signer-to-peer mapping fails.
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
  "empty_topology",
  "single_local_only",
  "no_votes_four",
  "one_remote_observed_below",
  "near_two_remote_observed",
  "near_local_remote_observed",
  "at_quorum_missing",
  "min_zero_no_near",
  "all_remote_observed",
  "mapping_fail",
  "local_middle_order",
  "view_mapped_missing"
}

EmptyLocalCases == {
  "empty_topology",
  "single_local_only"
}

MissingTargetCases == {
  "no_votes_four",
  "one_remote_observed_below",
  "all_remote_observed",
  "local_middle_order",
  "view_mapped_missing"
}

FanoutGateCases == {
  "near_two_remote_observed",
  "near_local_remote_observed",
  "at_quorum_missing",
  "min_zero_no_near"
}

MappingFallbackCases == {
  "mapping_fail"
}

OrderDistinctCases == {
  "one_remote_observed_below",
  "local_middle_order",
  "view_mapped_missing"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

TopologyLen(c) ==
  CASE c = "empty_topology" -> 0
    [] c = "single_local_only" -> 1
    [] c = "local_middle_order" -> 5
    [] OTHER -> 4

LocalIndex(c) ==
  IF c = "local_middle_order" THEN 2 ELSE 0

MinVotes(c) ==
  CASE c = "min_zero_no_near" -> 0
    [] c = "local_middle_order" -> 4
    [] c = "single_local_only" -> 1
    [] c = "empty_topology" -> 0
    [] OTHER -> 3

VoteCount(c) ==
  CASE c \in {"empty_topology", "single_local_only", "no_votes_four",
       "min_zero_no_near"} -> 0
    [] c \in {"one_remote_observed_below", "mapping_fail"} -> 1
    [] c \in {"near_two_remote_observed", "near_local_remote_observed",
       "local_middle_order"} -> 2
    [] OTHER -> 3

MappingOk(c) ==
  c # "mapping_fail"

InTopology(c, i) ==
  i < TopologyLen(c)

AllNonLocal(c, i) ==
  InTopology(c, i) /\ i # LocalIndex(c)

Observed(c, i) ==
  CASE c = "one_remote_observed_below" -> i = 1
    [] c = "near_two_remote_observed" -> i = 1 \/ i = 2
    [] c = "near_local_remote_observed" -> i = 0 \/ i = 1
    [] c = "at_quorum_missing" -> i = 1 \/ i = 2
    [] c = "min_zero_no_near" -> i = 1
    [] c = "all_remote_observed" -> i \in {1, 2, 3}
    [] c = "local_middle_order" -> i = 1 \/ i = 4
    [] c = "view_mapped_missing" -> i \in {0, 1, 3}
    [] OTHER -> FALSE

NonLocalCount(c) ==
  BoolToInt(AllNonLocal(c, 0)) + BoolToInt(AllNonLocal(c, 1))
    + BoolToInt(AllNonLocal(c, 2)) + BoolToInt(AllNonLocal(c, 3))
    + BoolToInt(AllNonLocal(c, 4))

NearCommitQuorum(c) ==
  MinVotes(c) > 0
    /\ VoteCount(c) < MinVotes(c)
    /\ VoteCount(c) + 1 >= MinVotes(c)

FullFanout(c) ==
  ~MappingOk(c) \/ (NearCommitQuorum(c) /\ NonLocalCount(c) > 0)

SpecTarget(c, i) ==
  AllNonLocal(c, i) /\ (FullFanout(c) \/ ~Observed(c, i))

SpecLen(c) ==
  BoolToInt(SpecTarget(c, 0)) + BoolToInt(SpecTarget(c, 1))
    + BoolToInt(SpecTarget(c, 2)) + BoolToInt(SpecTarget(c, 3))
    + BoolToInt(SpecTarget(c, 4))

SpecCountBefore(c, i) ==
  BoolToInt(0 < i /\ SpecTarget(c, 0))
    + BoolToInt(1 < i /\ SpecTarget(c, 1))
    + BoolToInt(2 < i /\ SpecTarget(c, 2))
    + BoolToInt(3 < i /\ SpecTarget(c, 3))
    + BoolToInt(4 < i /\ SpecTarget(c, 4))

SpecTargetAt(c, rank) ==
  CASE SpecTarget(c, 0) /\ SpecCountBefore(c, 0) = rank -> 0
    [] SpecTarget(c, 1) /\ SpecCountBefore(c, 1) = rank -> 1
    [] SpecTarget(c, 2) /\ SpecCountBefore(c, 2) = rank -> 2
    [] SpecTarget(c, 3) /\ SpecCountBefore(c, 3) = rank -> 3
    [] SpecTarget(c, 4) /\ SpecCountBefore(c, 4) = rank -> 4
    [] OTHER -> 0

SpecOutput(c) ==
  <<SpecLen(c), SpecTargetAt(c, 0), SpecTargetAt(c, 1),
    SpecTargetAt(c, 2), SpecTargetAt(c, 3), SpecTarget(c, 0),
    SpecTarget(c, 1), SpecTarget(c, 2), SpecTarget(c, 3),
    SpecTarget(c, 4), FullFanout(c), NearCommitQuorum(c), TRUE>>

ActualTarget(c, i) ==
  CASE Bug = "empty_returns_full" /\ c = "empty_topology" -> i = 0
    [] Bug = "includes_local" /\ c = "no_votes_four" ->
         i = LocalIndex(c) \/ SpecTarget(c, i)
    [] Bug = "observed_below_quorum_targeted"
       /\ c = "one_remote_observed_below" ->
         i = 1 \/ SpecTarget(c, i)
    [] Bug = "unobserved_missing_omitted"
       /\ c = "one_remote_observed_below" ->
         SpecTarget(c, i) /\ i # 2
    [] Bug = "near_quorum_only_missing"
       /\ c = "near_two_remote_observed" -> i = 3
    [] Bug = "at_quorum_full_fanout" /\ c = "at_quorum_missing" ->
         AllNonLocal(c, i)
    [] Bug = "min_zero_full_fanout" /\ c = "min_zero_no_near" ->
         AllNonLocal(c, i)
    [] Bug = "mapping_failure_empty" /\ c = "mapping_fail" -> FALSE
    [] Bug = "all_observed_full_fanout" /\ c = "all_remote_observed" ->
         AllNonLocal(c, i)
    [] Bug = "view_mapping_wrong_peer" /\ c = "view_mapped_missing" ->
         i = 3
    [] OTHER -> SpecTarget(c, i)

ActualLen(c) ==
  CASE Bug = "duplicate_target" /\ c = "one_remote_observed_below" -> 2
    [] OTHER ->
         BoolToInt(ActualTarget(c, 0)) + BoolToInt(ActualTarget(c, 1))
           + BoolToInt(ActualTarget(c, 2)) + BoolToInt(ActualTarget(c, 3))
           + BoolToInt(ActualTarget(c, 4))

ActualCountBefore(c, i) ==
  BoolToInt(0 < i /\ ActualTarget(c, 0))
    + BoolToInt(1 < i /\ ActualTarget(c, 1))
    + BoolToInt(2 < i /\ ActualTarget(c, 2))
    + BoolToInt(3 < i /\ ActualTarget(c, 3))
    + BoolToInt(4 < i /\ ActualTarget(c, 4))

ActualTargetAt(c, rank) ==
  CASE Bug = "local_middle_wrong_order" /\ c = "local_middle_order"
       /\ rank = 0 -> 3
    [] Bug = "local_middle_wrong_order" /\ c = "local_middle_order"
       /\ rank = 1 -> 0
    [] Bug = "duplicate_target" /\ c = "one_remote_observed_below"
       /\ rank = 0 -> 2
    [] Bug = "duplicate_target" /\ c = "one_remote_observed_below"
       /\ rank = 1 -> 2
    [] ActualTarget(c, 0) /\ ActualCountBefore(c, 0) = rank -> 0
    [] ActualTarget(c, 1) /\ ActualCountBefore(c, 1) = rank -> 1
    [] ActualTarget(c, 2) /\ ActualCountBefore(c, 2) = rank -> 2
    [] ActualTarget(c, 3) /\ ActualCountBefore(c, 3) = rank -> 3
    [] ActualTarget(c, 4) /\ ActualCountBefore(c, 4) = rank -> 4
    [] OTHER -> 0

ActualDistinct(c) ==
  CASE Bug = "duplicate_target" /\ c = "one_remote_observed_below" -> FALSE
    [] OTHER -> TRUE

ActualFullFanout(c) ==
  CASE Bug = "near_quorum_only_missing"
       /\ c = "near_two_remote_observed" -> FALSE
    [] Bug = "at_quorum_full_fanout" /\ c = "at_quorum_missing" -> TRUE
    [] Bug = "min_zero_full_fanout" /\ c = "min_zero_no_near" -> TRUE
    [] Bug = "mapping_failure_empty" /\ c = "mapping_fail" -> FALSE
    [] Bug = "all_observed_full_fanout" /\ c = "all_remote_observed" -> TRUE
    [] OTHER -> FullFanout(c)

ActualOutput(c) ==
  <<ActualLen(c), ActualTargetAt(c, 0), ActualTargetAt(c, 1),
    ActualTargetAt(c, 2), ActualTargetAt(c, 3), ActualTarget(c, 0),
    ActualTarget(c, 1), ActualTarget(c, 2), ActualTarget(c, 3),
    ActualTarget(c, 4), ActualFullFanout(c), NearCommitQuorum(c),
    ActualDistinct(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_returns_full",
       "includes_local",
       "observed_below_quorum_targeted",
       "unobserved_missing_omitted",
       "near_quorum_only_missing",
       "at_quorum_full_fanout",
       "min_zero_full_fanout",
       "mapping_failure_empty",
       "all_observed_full_fanout",
       "local_middle_wrong_order",
       "duplicate_target",
       "view_mapping_wrong_peer"
     }
  /\ checked = 0

QuorumRetransmitTargetCoreSafety ==
  /\ ActualOutput("empty_topology") = SpecOutput("empty_topology")
  /\ ActualOutput("single_local_only") = SpecOutput("single_local_only")
  /\ ActualOutput("no_votes_four") = SpecOutput("no_votes_four")
  /\ ActualOutput("one_remote_observed_below") =
       SpecOutput("one_remote_observed_below")
  /\ ActualOutput("near_two_remote_observed") =
       SpecOutput("near_two_remote_observed")
  /\ ActualOutput("near_local_remote_observed") =
       SpecOutput("near_local_remote_observed")
  /\ ActualOutput("at_quorum_missing") = SpecOutput("at_quorum_missing")
  /\ ActualOutput("min_zero_no_near") = SpecOutput("min_zero_no_near")
  /\ ActualOutput("all_remote_observed") = SpecOutput("all_remote_observed")
  /\ ActualOutput("mapping_fail") = SpecOutput("mapping_fail")
  /\ ActualOutput("local_middle_order") = SpecOutput("local_middle_order")
  /\ ActualOutput("view_mapped_missing") = SpecOutput("view_mapped_missing")

SafetyFast ==
  QuorumRetransmitTargetCoreSafety

QuorumRetransmitEmptyLocalExact ==
  \A c \in EmptyLocalCases:
    ActualOutput(c) = SpecOutput(c)

QuorumRetransmitMissingTargetsExact ==
  \A c \in MissingTargetCases:
    ActualOutput(c) = SpecOutput(c)

QuorumRetransmitFanoutGateExact ==
  \A c \in FanoutGateCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualFullFanout(c) = FullFanout(c)

QuorumRetransmitMappingFallbackExact ==
  \A c \in MappingFallbackCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualFullFanout(c) = FullFanout(c)

QuorumRetransmitOrderDistinctExact ==
  \A c \in OrderDistinctCases:
    /\ ActualOutput(c) = SpecOutput(c)
    /\ ActualDistinct(c)

QuorumRetransmitTargetExactness ==
  /\ QuorumRetransmitTargetCoreSafety
  /\ QuorumRetransmitEmptyLocalExact
  /\ QuorumRetransmitMissingTargetsExact
  /\ QuorumRetransmitFanoutGateExact
  /\ QuorumRetransmitMappingFallbackExact
  /\ QuorumRetransmitOrderDistinctExact

BugEmptyReturnsFull ==
  ActualOutput("empty_topology") = SpecOutput("empty_topology")

BugIncludesLocal ==
  ActualOutput("no_votes_four") = SpecOutput("no_votes_four")

BugObservedBelowQuorumTargeted ==
  ActualOutput("one_remote_observed_below") =
    SpecOutput("one_remote_observed_below")

BugUnobservedMissingOmitted ==
  ActualOutput("one_remote_observed_below") =
    SpecOutput("one_remote_observed_below")

BugNearQuorumOnlyMissing ==
  ActualOutput("near_two_remote_observed") =
    SpecOutput("near_two_remote_observed")

BugAtQuorumFullFanout ==
  ActualOutput("at_quorum_missing") = SpecOutput("at_quorum_missing")

BugMinZeroFullFanout ==
  ActualOutput("min_zero_no_near") = SpecOutput("min_zero_no_near")

BugMappingFailureEmpty ==
  ActualOutput("mapping_fail") = SpecOutput("mapping_fail")

BugAllObservedFullFanout ==
  ActualOutput("all_remote_observed") = SpecOutput("all_remote_observed")

BugLocalMiddleWrongOrder ==
  ActualOutput("local_middle_order") = SpecOutput("local_middle_order")

BugDuplicateTarget ==
  ActualOutput("one_remote_observed_below") =
    SpecOutput("one_remote_observed_below")

BugViewMappingWrongPeer ==
  ActualOutput("view_mapped_missing") = SpecOutput("view_mapped_missing")

Safety ==
  QuorumRetransmitTargetCoreSafety

=============================================================================
====

---- MODULE SumeragiTopologyRoleFilterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for topology role classification and role-filtered
signature selection.

This slice covers `Topology::role(...)`,
`Topology::filter_signatures_by_roles(...)`,
`NonEmptyTopology::leader(...)`, `ConsensusTopology::{leader, proxy_tail,
validating_peers, set_b_validators, voting_peers}`, and
`audit_roles_for_prev_block_hash(...)`.

Peer ids and signature indices are abstracted into small integers. The model
pins the exact leader/proxy-tail/Set A/Set B partition, the single-peer
proxy-tail filtering special case, invalid signature-index rejection, input
signature order preservation, duplicate-signature preservation, canonical
roster sorting/deduplication for audit roles, and hash-derived rotation of the
commit-quorum prefix.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoPeer == -1

RoleLeader == 1
RoleValidatingPeer == 2
RoleSetBValidator == 3
RoleProxyTail == 4
RoleUndefined == 5

RoleCases == {
  "role_empty_peer0",
  "role_len1_peer0",
  "role_len1_unknown",
  "role_len2_peer0",
  "role_len2_peer1",
  "role_len3_peer1",
  "role_len3_peer2",
  "role_len4_peer3",
  "role_len7_peer0",
  "role_len7_peer4",
  "role_len7_peer5",
  "role_len7_unknown"
}

GroupCases == {
  "group_len0",
  "group_len1",
  "group_len2",
  "group_len3",
  "group_len4",
  "group_len7"
}

FilterCases == {
  "filter_leader_len7",
  "filter_proxy_len1",
  "filter_proxy_len2",
  "filter_proxy_len7",
  "filter_validating_len2",
  "filter_validating_len3",
  "filter_validating_len7",
  "filter_setb_len3",
  "filter_setb_len7",
  "filter_union_leader_proxy_len7",
  "filter_all_roles_len7",
  "filter_invalid_indices_len3",
  "filter_duplicate_signatures_len7",
  "filter_duplicate_roles_len7",
  "filter_undefined_len7"
}

AuditCases == {
  "audit_len1_seed9",
  "audit_len4_seed0",
  "audit_len4_seed1",
  "audit_len7_seed2",
  "audit_dedup_sort_seed1"
}

\* @type: (Bool, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => <<Bool, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
GroupOut(consensus, leader, proxy, validatingLen, validatingFirst,
    validatingLast, setBLen, setBFirst, setBLast, votingLen, votingFirst,
    votingLast) ==
  <<consensus, leader, proxy, validatingLen, validatingFirst, validatingLast,
    setBLen, setBFirst, setBLast, votingLen, votingFirst, votingLast>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Bool) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Bool>>;
AuditOut(len, p1, r1, p2, r2, p3, r3, p4, r4, p5, r5, p6, r6,
    p7, r7, distinct) ==
  <<len, p1, r1, p2, r2, p3, r3, p4, r4, p5, r5, p6, r6, p7, r7,
    distinct>>

SpecRole(c) ==
  CASE c = "role_empty_peer0" -> RoleUndefined
    [] c = "role_len1_peer0" -> RoleLeader
    [] c = "role_len1_unknown" -> RoleUndefined
    [] c = "role_len2_peer0" -> RoleLeader
    [] c = "role_len2_peer1" -> RoleProxyTail
    [] c = "role_len3_peer1" -> RoleValidatingPeer
    [] c = "role_len3_peer2" -> RoleProxyTail
    [] c = "role_len4_peer3" -> RoleSetBValidator
    [] c = "role_len7_peer0" -> RoleLeader
    [] c = "role_len7_peer4" -> RoleProxyTail
    [] c = "role_len7_peer5" -> RoleSetBValidator
    [] c = "role_len7_unknown" -> RoleUndefined
    [] OTHER -> RoleUndefined

ActualRole(c) ==
  CASE Bug = "role_leader_as_validating" /\ c = "role_len7_peer0" ->
         RoleValidatingPeer
    [] Bug = "role_proxy_tail_as_validating" /\ c = "role_len7_peer4" ->
         RoleValidatingPeer
    [] Bug = "role_set_b_as_validating" /\ c = "role_len7_peer5" ->
         RoleValidatingPeer
    [] Bug = "role_unknown_as_set_b" /\ c = "role_len7_unknown" ->
         RoleSetBValidator
    [] Bug = "role_single_proxy_tail" /\ c = "role_len1_peer0" ->
         RoleProxyTail
    [] Bug = "role_len2_peer1_set_b" /\ c = "role_len2_peer1" ->
         RoleSetBValidator
    [] OTHER -> SpecRole(c)

SpecGroup(c) ==
  CASE c = "group_len0" ->
         GroupOut(FALSE, NoPeer, NoPeer, 0, NoPeer, NoPeer, 0, NoPeer,
           NoPeer, 0, NoPeer, NoPeer)
    [] c = "group_len1" ->
         GroupOut(FALSE, 0, NoPeer, 0, NoPeer, NoPeer, 0, NoPeer, NoPeer,
           0, NoPeer, NoPeer)
    [] c = "group_len2" ->
         GroupOut(TRUE, 0, 1, 0, NoPeer, NoPeer, 0, NoPeer, NoPeer,
           2, 0, 1)
    [] c = "group_len3" ->
         GroupOut(TRUE, 0, 2, 1, 1, 1, 0, NoPeer, NoPeer, 3, 0, 2)
    [] c = "group_len4" ->
         GroupOut(TRUE, 0, 2, 1, 1, 1, 1, 3, 3, 4, 0, 3)
    [] c = "group_len7" ->
         GroupOut(TRUE, 0, 4, 3, 1, 3, 2, 5, 6, 7, 0, 6)
    [] OTHER ->
         GroupOut(FALSE, NoPeer, NoPeer, 0, NoPeer, NoPeer, 0, NoPeer,
           NoPeer, 0, NoPeer, NoPeer)

ActualGroup(c) ==
  CASE Bug = "group_empty_has_leader" /\ c = "group_len0" ->
         GroupOut(FALSE, 0, NoPeer, 0, NoPeer, NoPeer, 0, NoPeer, NoPeer,
           0, NoPeer, NoPeer)
    [] Bug = "group_single_requires_consensus" /\ c = "group_len1" ->
         GroupOut(TRUE, 0, 0, 0, NoPeer, NoPeer, 0, NoPeer, NoPeer, 1, 0, 0)
    [] Bug = "group_leader_not_first" /\ c = "group_len7" ->
         GroupOut(TRUE, 1, 4, 3, 1, 3, 2, 5, 6, 7, 0, 6)
    [] Bug = "group_proxy_tail_off_by_one" /\ c = "group_len7" ->
         GroupOut(TRUE, 0, 3, 2, 1, 2, 3, 4, 6, 7, 0, 6)
    [] Bug = "group_validating_includes_leader" /\ c = "group_len7" ->
         GroupOut(TRUE, 0, 4, 4, 0, 3, 2, 5, 6, 7, 0, 6)
    [] Bug = "group_validating_drops_last" /\ c = "group_len7" ->
         GroupOut(TRUE, 0, 4, 2, 1, 2, 2, 5, 6, 7, 0, 6)
    [] Bug = "group_set_b_includes_proxy_tail" /\ c = "group_len7" ->
         GroupOut(TRUE, 0, 4, 3, 1, 3, 3, 4, 6, 7, 0, 6)
    [] Bug = "group_voting_drops_leader" /\ c = "group_len7" ->
         GroupOut(TRUE, 0, 4, 3, 1, 3, 2, 5, 6, 6, 1, 6)
    [] OTHER -> SpecGroup(c)

SpecFilter(c) ==
  CASE c = "filter_leader_len7" -> <<0>>
    [] c = "filter_proxy_len1" -> <<0>>
    [] c = "filter_proxy_len2" -> <<1>>
    [] c = "filter_proxy_len7" -> <<4>>
    [] c = "filter_validating_len2" -> <<>>
    [] c = "filter_validating_len3" -> <<1>>
    [] c = "filter_validating_len7" -> <<1, 2, 3>>
    [] c = "filter_setb_len3" -> <<>>
    [] c = "filter_setb_len7" -> <<5, 6>>
    [] c = "filter_union_leader_proxy_len7" -> <<4, 0>>
    [] c = "filter_all_roles_len7" -> <<6, 5, 4, 3, 2, 1, 0>>
    [] c = "filter_invalid_indices_len3" -> <<0, 2>>
    [] c = "filter_duplicate_signatures_len7" -> <<1, 1, 2, 3>>
    [] c = "filter_duplicate_roles_len7" -> <<0>>
    [] c = "filter_undefined_len7" -> <<>>
    [] OTHER -> <<>>

ActualFilter(c) ==
  CASE Bug = "filter_drops_leader" /\ c = "filter_leader_len7" -> <<>>
    [] Bug = "filter_keeps_wrong_role" /\ c = "filter_leader_len7" ->
         <<0, 1>>
    [] Bug = "filter_validating_includes_proxy_tail"
       /\ c = "filter_validating_len7" -> <<1, 2, 3, 4>>
    [] Bug = "filter_set_b_drops_last" /\ c = "filter_setb_len7" -> <<5>>
    [] Bug = "filter_proxy_single_drops_local" /\ c = "filter_proxy_len1" ->
         <<>>
    [] Bug = "filter_keeps_invalid_index"
       /\ c = "filter_invalid_indices_len3" -> <<0, 999, 2>>
    [] Bug = "filter_union_reorders_by_role"
       /\ c = "filter_union_leader_proxy_len7" -> <<0, 4>>
    [] Bug = "filter_dedupes_signatures"
       /\ c = "filter_duplicate_signatures_len7" -> <<1, 2, 3>>
    [] Bug = "filter_undefined_includes_all"
       /\ c = "filter_undefined_len7" -> <<0, 1, 2, 3, 4, 5, 6>>
    [] Bug = "filter_duplicate_roles_adds_proxy"
       /\ c = "filter_duplicate_roles_len7" -> <<0, 4>>
    [] Bug = "filter_proxy_len2_uses_leader" /\ c = "filter_proxy_len2" ->
         <<0>>
    [] Bug = "filter_validating_len3_empty"
       /\ c = "filter_validating_len3" -> <<>>
    [] OTHER -> SpecFilter(c)

SpecAudit(c) ==
  CASE c = "audit_len1_seed9" ->
         AuditOut(1, 0, RoleLeader, NoPeer, RoleUndefined, NoPeer,
           RoleUndefined, NoPeer, RoleUndefined, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] c = "audit_len4_seed0" ->
         AuditOut(4, 0, RoleLeader, 1, RoleValidatingPeer, 2,
           RoleProxyTail, 3, RoleSetBValidator, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] c = "audit_len4_seed1" ->
         AuditOut(4, 1, RoleLeader, 2, RoleValidatingPeer, 0,
           RoleProxyTail, 3, RoleSetBValidator, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] c = "audit_len7_seed2" ->
         AuditOut(7, 2, RoleLeader, 3, RoleValidatingPeer, 4,
           RoleValidatingPeer, 0, RoleValidatingPeer, 1, RoleProxyTail,
           5, RoleSetBValidator, 6, RoleSetBValidator, TRUE)
    [] c = "audit_dedup_sort_seed1" ->
         AuditOut(3, 2, RoleLeader, 3, RoleValidatingPeer, 1,
           RoleProxyTail, NoPeer, RoleUndefined, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] OTHER ->
         AuditOut(0, NoPeer, RoleUndefined, NoPeer, RoleUndefined, NoPeer,
           RoleUndefined, NoPeer, RoleUndefined, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)

ActualAudit(c) ==
  CASE Bug = "audit_uses_unrotated_topology" /\ c = "audit_len7_seed2" ->
         AuditOut(7, 0, RoleLeader, 1, RoleValidatingPeer, 2,
           RoleValidatingPeer, 3, RoleValidatingPeer, 4, RoleProxyTail,
           5, RoleSetBValidator, 6, RoleSetBValidator, TRUE)
    [] Bug = "audit_rotation_off_by_one" /\ c = "audit_len7_seed2" ->
         AuditOut(7, 1, RoleLeader, 2, RoleValidatingPeer, 3,
           RoleValidatingPeer, 4, RoleValidatingPeer, 0, RoleProxyTail,
           5, RoleSetBValidator, 6, RoleSetBValidator, TRUE)
    [] Bug = "audit_skips_canonical_sort" /\ c = "audit_dedup_sort_seed1" ->
         AuditOut(3, 1, RoleLeader, 2, RoleValidatingPeer, 3,
           RoleProxyTail, NoPeer, RoleUndefined, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] Bug = "audit_keeps_duplicates" /\ c = "audit_dedup_sort_seed1" ->
         AuditOut(4, 2, RoleLeader, 3, RoleValidatingPeer, 1,
           RoleProxyTail, 3, RoleSetBValidator, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, FALSE)
    [] Bug = "audit_roles_before_rotation" /\ c = "audit_len4_seed1" ->
         AuditOut(4, 1, RoleValidatingPeer, 2, RoleProxyTail, 0,
           RoleLeader, 3, RoleSetBValidator, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] Bug = "audit_drops_prev_hash" /\ c = "audit_len4_seed1" ->
         AuditOut(4, 0, RoleLeader, 1, RoleValidatingPeer, 2,
           RoleProxyTail, 3, RoleSetBValidator, NoPeer, RoleUndefined,
           NoPeer, RoleUndefined, NoPeer, RoleUndefined, TRUE)
    [] OTHER -> SpecAudit(c)

Init ==
  checked = 0

Next ==
  /\ checked = 0
  /\ checked' = 1

TypeInvariant ==
  checked \in 0..1

SafetyFast ==
  /\ \A c \in RoleCases: ActualRole(c) = SpecRole(c)
  /\ \A c \in GroupCases: ActualGroup(c) = SpecGroup(c)
  /\ \A c \in FilterCases: ActualFilter(c) = SpecFilter(c)
  /\ \A c \in AuditCases: ActualAudit(c) = SpecAudit(c)

TopologyRolePartitionExact ==
  \A c \in RoleCases:
    ActualRole(c) = SpecRole(c)

TopologyRoleSliceExact ==
  \A c \in GroupCases:
    ActualGroup(c) = SpecGroup(c)

TopologySignatureFilterExact ==
  \A c \in FilterCases:
    ActualFilter(c) = SpecFilter(c)

TopologyAuditRoleRotationExact ==
  \A c \in AuditCases:
    ActualAudit(c) = SpecAudit(c)

TopologyRoleFilterExactness ==
  /\ \A c \in RoleCases: ActualRole(c) = SpecRole(c)
  /\ \A c \in GroupCases: ActualGroup(c) = SpecGroup(c)
  /\ \A c \in FilterCases: ActualFilter(c) = SpecFilter(c)
  /\ \A c \in AuditCases: ActualAudit(c) = SpecAudit(c)
  /\ TopologyRolePartitionExact
  /\ TopologyRoleSliceExact
  /\ TopologySignatureFilterExact
  /\ TopologyAuditRoleRotationExact

TopologyRoleFilterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ TopologyRoleFilterExactness

Safety ==
  checked = 1 => SafetyFast

=============================================================================
====

---- MODULE SumeragiManifestGateRescheduleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the `ManifestGuard` branch inside
`Actor::reschedule_pending_quorum_block(...)`.

The implementation treats a pending block stopped at the DA manifest guard as
effective quorum-reschedule work: the block is not classified as zombie state
solely because no commit votes are present. That admission must stay bounded:
manifest-gated blocks are retained, do not trigger immediate authoritative
frontier rotation, and are marked as plain quorum-reschedule attempts only when
the branch reaches the normal action path. Empty target sets remain no-op
reschedules.
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
  "plain_zero",
  "manifest_no_votes",
  "manifest_payload",
  "manifest_votes_payload",
  "manifest_no_targets",
  "votes_payload",
  "votes_backlog",
  "votes_no_slot",
  "votes_no_targets",
  "same_slot_evidence",
  "frontier_owner_vote_backed"
}

ManifestGateCases == {
  "manifest_no_votes",
  "manifest_payload",
  "manifest_votes_payload",
  "manifest_no_targets"
}

ManifestActionCases == {
  "manifest_no_votes",
  "manifest_payload",
  "manifest_votes_payload"
}

NoTargetNoopCases == {
  "manifest_no_targets",
  "votes_no_targets"
}

VoteBackedActionCases == {
  "votes_payload",
  "votes_backlog",
  "votes_no_slot"
}

AuthoritativeRotationCases == {
  "votes_payload",
  "votes_no_targets"
}

AuthoritativeSuppressedCases == {
  "manifest_payload",
  "manifest_votes_payload",
  "votes_backlog",
  "votes_no_slot"
}

PassiveVoteBackedEvidenceCases == {
  "same_slot_evidence",
  "frontier_owner_vote_backed"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

NoMarker == 0
QuorumMarker == 1
VoteBackedMarker == 2

ManifestGatePending(c) ==
  c \in {
    "manifest_no_votes",
    "manifest_payload",
    "manifest_votes_payload",
    "manifest_no_targets"
  }

HasRescheduleVotes(c) ==
  c \in {
    "manifest_votes_payload",
    "votes_payload",
    "votes_backlog",
    "votes_no_slot",
    "votes_no_targets"
  }

SameSlotVoteBackedEvidence(c) ==
  c = "same_slot_evidence"

FrontierVoteBackedOwnerActive(c) ==
  c = "frontier_owner_vote_backed"

ContiguousFrontier(c) ==
  c \in {
    "manifest_payload",
    "manifest_votes_payload",
    "votes_payload",
    "votes_backlog",
    "votes_no_slot",
    "votes_no_targets",
    "same_slot_evidence",
    "frontier_owner_vote_backed"
  }

AuthoritativePayloadPresent(c) ==
  c \in {
    "manifest_payload",
    "manifest_votes_payload",
    "votes_payload",
    "votes_backlog",
    "votes_no_slot",
    "votes_no_targets"
  }

FrontierSlotPresentForView(c) ==
  c \in {
    "manifest_payload",
    "manifest_votes_payload",
    "votes_payload",
    "votes_backlog",
    "votes_no_targets"
  }

ResilienceIngressBacklogActive(c) ==
  c = "votes_backlog"

TargetsEmpty(c) ==
  c \in {"manifest_no_targets", "votes_no_targets"}

LocalVoteEmitted(c) ==
  FALSE

RebroadcastWork(c) ==
  HasRescheduleVotes(c) /\ ~TargetsEmpty(c) /\ ~ManifestGatePending(c)

SpecEffective(c) ==
  HasRescheduleVotes(c)
    \/ SameSlotVoteBackedEvidence(c)
    \/ FrontierVoteBackedOwnerActive(c)
    \/ ManifestGatePending(c)

SpecDropPending(c) ==
  ~SpecEffective(c)

SpecAuthoritativeRotation(c) ==
  ContiguousFrontier(c)
    /\ SpecEffective(c)
    /\ ~ManifestGatePending(c)
    /\ AuthoritativePayloadPresent(c)
    /\ FrontierSlotPresentForView(c)
    /\ ~ResilienceIngressBacklogActive(c)

SpecNoTargetNoop(c) ==
  ~SpecDropPending(c) /\ TargetsEmpty(c)

SpecActionTaken(c) ==
  IF SpecNoTargetNoop(c) THEN FALSE
  ELSE SpecDropPending(c)
    \/ ManifestGatePending(c)
    \/ LocalVoteEmitted(c)
    \/ RebroadcastWork(c)

SpecRecordedVoteCount(c) ==
  IF HasRescheduleVotes(c) \/ LocalVoteEmitted(c) THEN 1 ELSE 0

SpecMarker(c) ==
  IF SpecNoTargetNoop(c) \/ ~SpecActionTaken(c) THEN NoMarker
  ELSE IF SpecRecordedVoteCount(c) > 0 THEN VoteBackedMarker
  ELSE QuorumMarker

SpecRetained(c) ==
  IF SpecNoTargetNoop(c) THEN TRUE ELSE ~SpecDropPending(c)

SpecReturn(c) ==
  IF SpecNoTargetNoop(c) THEN SpecAuthoritativeRotation(c)
  ELSE SpecActionTaken(c) \/ SpecAuthoritativeRotation(c)

SpecCleanup(c) ==
  SpecDropPending(c) /\ SpecActionTaken(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<BoolToInt(SpecEffective(c)),
    BoolToInt(SpecDropPending(c)),
    BoolToInt(SpecActionTaken(c)),
    BoolToInt(SpecRetained(c)),
    SpecMarker(c),
    BoolToInt(SpecAuthoritativeRotation(c)),
    BoolToInt(SpecReturn(c)),
    BoolToInt(SpecCleanup(c))>>

ActualEffective(c) ==
  CASE Bug = "manifest_not_effective" /\ c = "manifest_no_votes" ->
         HasRescheduleVotes(c)
           \/ SameSlotVoteBackedEvidence(c)
           \/ FrontierVoteBackedOwnerActive(c)
    [] Bug = "votes_not_effective" /\ c = "votes_payload" -> FALSE
    [] Bug = "same_slot_not_effective" /\ c = "same_slot_evidence" -> FALSE
    [] Bug = "frontier_owner_not_effective" /\
          c = "frontier_owner_vote_backed" -> FALSE
    [] OTHER -> SpecEffective(c)

ActualDropPending(c) ==
  CASE Bug = "manifest_dropped" /\ c = "manifest_no_votes" -> TRUE
    [] Bug = "plain_zero_not_dropped" /\ c = "plain_zero" -> FALSE
    [] OTHER -> ~ActualEffective(c)

ActualAuthoritativeRotation(c) ==
  CASE Bug = "manifest_rotates_authoritative" /\ c = "manifest_payload" ->
         TRUE
    [] Bug = "manifest_votes_rotates_authoritative" /\
          c = "manifest_votes_payload" -> TRUE
    [] Bug = "auth_backlog_ignored" /\ c = "votes_backlog" -> TRUE
    [] Bug = "auth_ignores_slot" /\ c = "votes_no_slot" -> TRUE
    [] Bug = "auth_rejects_vote_payload" /\ c = "votes_payload" -> FALSE
    [] OTHER ->
         ContiguousFrontier(c)
           /\ ActualEffective(c)
           /\ ~ManifestGatePending(c)
           /\ AuthoritativePayloadPresent(c)
           /\ FrontierSlotPresentForView(c)
           /\ ~ResilienceIngressBacklogActive(c)

ActualNoTargetNoop(c) ==
  CASE Bug = "manifest_no_targets_ignores_noop" /\
          c = "manifest_no_targets" -> FALSE
    [] Bug = "votes_no_targets_marks" /\ c = "votes_no_targets" -> FALSE
    [] OTHER -> ~ActualDropPending(c) /\ TargetsEmpty(c)

ActualRebroadcastWork(c) ==
  RebroadcastWork(c)

ActualActionTaken(c) ==
  IF ActualNoTargetNoop(c) THEN FALSE
  ELSE
    CASE Bug = "manifest_action_missing" /\ c = "manifest_no_votes" ->
           FALSE
      [] Bug = "same_slot_action_taken" /\ c = "same_slot_evidence" ->
           TRUE
      [] Bug = "frontier_owner_action_taken" /\
            c = "frontier_owner_vote_backed" -> TRUE
      [] Bug = "votes_no_targets_marks" /\ c = "votes_no_targets" ->
           TRUE
      [] OTHER ->
           ActualDropPending(c)
             \/ ManifestGatePending(c)
             \/ LocalVoteEmitted(c)
             \/ ActualRebroadcastWork(c)

ActualRecordedVoteCount(c) ==
  CASE Bug = "manifest_marks_vote_backed" /\ c = "manifest_no_votes" -> 1
    [] Bug = "manifest_votes_mark_quorum" /\
          c = "manifest_votes_payload" -> 0
    [] Bug = "votes_mark_quorum" /\ c = "votes_payload" -> 0
    [] OTHER -> SpecRecordedVoteCount(c)

ActualMarker(c) ==
  IF ActualNoTargetNoop(c) \/ ~ActualActionTaken(c) THEN NoMarker
  ELSE IF ActualRecordedVoteCount(c) > 0 THEN VoteBackedMarker
  ELSE QuorumMarker

ActualRetained(c) ==
  CASE Bug = "manifest_not_retained" /\ c = "manifest_no_votes" -> FALSE
    [] Bug = "manifest_no_targets_dropped" /\ c = "manifest_no_targets" ->
         FALSE
    [] Bug = "plain_zero_retained" /\ c = "plain_zero" -> TRUE
    [] OTHER ->
         IF ActualNoTargetNoop(c) THEN TRUE ELSE ~ActualDropPending(c)

ActualReturn(c) ==
  CASE Bug = "manifest_no_targets_returns_true" /\
          c = "manifest_no_targets" -> TRUE
    [] OTHER ->
         IF ActualNoTargetNoop(c) THEN ActualAuthoritativeRotation(c)
         ELSE ActualActionTaken(c) \/ ActualAuthoritativeRotation(c)

ActualCleanup(c) ==
  CASE Bug = "drop_clean_skipped" /\ c = "plain_zero" -> FALSE
    [] Bug = "manifest_cleaned" /\ c = "manifest_no_votes" -> TRUE
    [] OTHER -> ActualDropPending(c) /\ ActualActionTaken(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<BoolToInt(ActualEffective(c)),
    BoolToInt(ActualDropPending(c)),
    BoolToInt(ActualActionTaken(c)),
    BoolToInt(ActualRetained(c)),
    ActualMarker(c),
    BoolToInt(ActualAuthoritativeRotation(c)),
    BoolToInt(ActualReturn(c)),
    BoolToInt(ActualCleanup(c))>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "manifest_not_effective",
       "manifest_dropped",
       "manifest_action_missing",
       "manifest_not_retained",
       "manifest_marks_vote_backed",
       "manifest_rotates_authoritative",
       "manifest_votes_rotates_authoritative",
       "manifest_votes_mark_quorum",
       "manifest_no_targets_ignores_noop",
       "manifest_no_targets_returns_true",
       "manifest_no_targets_dropped",
       "plain_zero_not_dropped",
       "plain_zero_retained",
       "drop_clean_skipped",
       "manifest_cleaned",
       "votes_not_effective",
       "votes_mark_quorum",
       "auth_backlog_ignored",
       "auth_ignores_slot",
       "auth_rejects_vote_payload",
       "votes_no_targets_marks",
       "same_slot_not_effective",
       "same_slot_action_taken",
       "frontier_owner_not_effective",
       "frontier_owner_action_taken"
     }
  /\ checked = 0

ManifestGateRescheduleCoreMatchesSpec ==
  /\ ActualOutput("plain_zero") = SpecOutput("plain_zero")
  /\ ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")
  /\ ActualOutput("manifest_payload") = SpecOutput("manifest_payload")
  /\ ActualOutput("manifest_votes_payload") =
       SpecOutput("manifest_votes_payload")
  /\ ActualOutput("manifest_no_targets") = SpecOutput("manifest_no_targets")
  /\ ActualOutput("votes_payload") = SpecOutput("votes_payload")
  /\ ActualOutput("votes_backlog") = SpecOutput("votes_backlog")
  /\ ActualOutput("votes_no_slot") = SpecOutput("votes_no_slot")
  /\ ActualOutput("votes_no_targets") = SpecOutput("votes_no_targets")
  /\ ActualOutput("same_slot_evidence") = SpecOutput("same_slot_evidence")
  /\ ActualOutput("frontier_owner_vote_backed") =
       SpecOutput("frontier_owner_vote_backed")

SafetyFast ==
  ManifestGateRescheduleCoreMatchesSpec

ManifestGateEffectExact ==
  \A c \in ManifestGateCases:
    /\ ActualEffective(c) = TRUE
    /\ ActualDropPending(c) = FALSE
    /\ ActualRetained(c) = TRUE
    /\ ActualCleanup(c) = FALSE
    /\ ActualOutput(c) = SpecOutput(c)

ManifestGateActionExact ==
  \A c \in ManifestActionCases:
    /\ ActualActionTaken(c) = TRUE
    /\ ActualAuthoritativeRotation(c) = FALSE
    /\ ActualReturn(c) = TRUE
    /\ ActualMarker(c) =
         IF HasRescheduleVotes(c) THEN VoteBackedMarker ELSE QuorumMarker
    /\ ActualOutput(c) = SpecOutput(c)

ManifestGateNoTargetExact ==
  /\ ActualNoTargetNoop("manifest_no_targets") = TRUE
  /\ ActualActionTaken("manifest_no_targets") = FALSE
  /\ ActualMarker("manifest_no_targets") = NoMarker
  /\ ActualRetained("manifest_no_targets") = TRUE
  /\ ActualReturn("manifest_no_targets") = FALSE
  /\ ActualOutput("manifest_no_targets") =
       SpecOutput("manifest_no_targets")

PlainZeroDropCleanupExact ==
  /\ ActualEffective("plain_zero") = FALSE
  /\ ActualDropPending("plain_zero") = TRUE
  /\ ActualActionTaken("plain_zero") = TRUE
  /\ ActualRetained("plain_zero") = FALSE
  /\ ActualMarker("plain_zero") = QuorumMarker
  /\ ActualCleanup("plain_zero") = TRUE
  /\ ActualReturn("plain_zero") = TRUE
  /\ ActualOutput("plain_zero") = SpecOutput("plain_zero")

VoteBackedActionExact ==
  \A c \in VoteBackedActionCases:
    /\ ActualEffective(c) = TRUE
    /\ ActualActionTaken(c) = TRUE
    /\ ActualMarker(c) = VoteBackedMarker
    /\ ActualRetained(c) = TRUE
    /\ ActualCleanup(c) = FALSE
    /\ ActualOutput(c) = SpecOutput(c)

VoteBackedNoTargetExact ==
  /\ ActualNoTargetNoop("votes_no_targets") = TRUE
  /\ ActualActionTaken("votes_no_targets") = FALSE
  /\ ActualMarker("votes_no_targets") = NoMarker
  /\ ActualRetained("votes_no_targets") = TRUE
  /\ ActualAuthoritativeRotation("votes_no_targets") = TRUE
  /\ ActualReturn("votes_no_targets") = TRUE
  /\ ActualOutput("votes_no_targets") = SpecOutput("votes_no_targets")

AuthoritativeRotationExact ==
  /\ \A c \in AuthoritativeRotationCases:
       /\ ActualAuthoritativeRotation(c) = TRUE
       /\ ActualReturn(c) = TRUE
       /\ ActualOutput(c) = SpecOutput(c)
  /\ \A c \in AuthoritativeSuppressedCases:
       /\ ActualAuthoritativeRotation(c) = FALSE
       /\ ActualOutput(c) = SpecOutput(c)

PassiveVoteBackedEvidenceExact ==
  \A c \in PassiveVoteBackedEvidenceCases:
    /\ ActualEffective(c) = TRUE
    /\ ActualActionTaken(c) = FALSE
    /\ ActualMarker(c) = NoMarker
    /\ ActualRetained(c) = TRUE
    /\ ActualReturn(c) = FALSE
    /\ ActualCleanup(c) = FALSE
    /\ ActualOutput(c) = SpecOutput(c)

ManifestGateRescheduleExactness ==
  /\ ManifestGateRescheduleCoreMatchesSpec
  /\ ManifestGateEffectExact
  /\ ManifestGateActionExact
  /\ ManifestGateNoTargetExact
  /\ PlainZeroDropCleanupExact
  /\ VoteBackedActionExact
  /\ VoteBackedNoTargetExact
  /\ AuthoritativeRotationExact
  /\ PassiveVoteBackedEvidenceExact

ManifestGateRescheduleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ManifestGateRescheduleExactness

BugManifestNotEffective ==
  ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")

BugManifestDropped ==
  ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")

BugManifestActionMissing ==
  ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")

BugManifestNotRetained ==
  ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")

BugManifestMarksVoteBacked ==
  ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")

BugManifestRotatesAuthoritative ==
  ActualOutput("manifest_payload") = SpecOutput("manifest_payload")

BugManifestVotesRotatesAuthoritative ==
  ActualOutput("manifest_votes_payload") =
    SpecOutput("manifest_votes_payload")

BugManifestVotesMarkQuorum ==
  ActualOutput("manifest_votes_payload") =
    SpecOutput("manifest_votes_payload")

BugManifestNoTargetsIgnoresNoop ==
  ActualOutput("manifest_no_targets") = SpecOutput("manifest_no_targets")

BugManifestNoTargetsReturnsTrue ==
  ActualOutput("manifest_no_targets") = SpecOutput("manifest_no_targets")

BugManifestNoTargetsDropped ==
  ActualOutput("manifest_no_targets") = SpecOutput("manifest_no_targets")

BugPlainZeroNotDropped ==
  ActualOutput("plain_zero") = SpecOutput("plain_zero")

BugPlainZeroRetained ==
  ActualOutput("plain_zero") = SpecOutput("plain_zero")

BugDropCleanSkipped ==
  ActualOutput("plain_zero") = SpecOutput("plain_zero")

BugManifestCleaned ==
  ActualOutput("manifest_no_votes") = SpecOutput("manifest_no_votes")

BugVotesNotEffective ==
  ActualOutput("votes_payload") = SpecOutput("votes_payload")

BugVotesMarkQuorum ==
  ActualOutput("votes_payload") = SpecOutput("votes_payload")

BugAuthBacklogIgnored ==
  ActualOutput("votes_backlog") = SpecOutput("votes_backlog")

BugAuthIgnoresSlot ==
  ActualOutput("votes_no_slot") = SpecOutput("votes_no_slot")

BugAuthRejectsVotePayload ==
  ActualOutput("votes_payload") = SpecOutput("votes_payload")

BugVotesNoTargetsMarks ==
  ActualOutput("votes_no_targets") = SpecOutput("votes_no_targets")

BugSameSlotNotEffective ==
  ActualOutput("same_slot_evidence") = SpecOutput("same_slot_evidence")

BugSameSlotActionTaken ==
  ActualOutput("same_slot_evidence") = SpecOutput("same_slot_evidence")

BugFrontierOwnerNotEffective ==
  ActualOutput("frontier_owner_vote_backed") =
    SpecOutput("frontier_owner_vote_backed")

BugFrontierOwnerActionTaken ==
  ActualOutput("frontier_owner_vote_backed") =
    SpecOutput("frontier_owner_vote_backed")

====

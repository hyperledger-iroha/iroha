---- MODULE SumeragiRbcUnverifiedRosterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `allow_unverified_rbc_roster(...)` and the
roster-availability branch of `rbc_roster_for_session(...)`.

The helper is the permissioned-mode escape hatch that lets RBC keep processing
INIT-supplied rosters only while the locally derived roster is unavailable. A
derived roster is available when a vote roster is known, when next-height (or
older) sessions can use the active fallback, or when an authoritative same-epoch
future payload lets the session rejoin the active topology.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Permissioned == "permissioned"
Npos == "npos"

Cases == {
  "permissioned_future_no_payload_empty",
  "permissioned_future_payload_active",
  "permissioned_future_payload_active_empty",
  "permissioned_next_active",
  "permissioned_next_active_empty",
  "permissioned_future_vote_roster",
  "permissioned_next_epoch_payload_empty",
  "npos_future_no_payload_empty",
  "npos_future_payload_active",
  "npos_next_active"
}

Mode(c) ==
  IF c \in {
       "npos_future_no_payload_empty",
       "npos_future_payload_active",
       "npos_next_active"
     }
  THEN Npos
  ELSE Permissioned

BeyondNextHeight(c) ==
  c \in {
    "permissioned_future_no_payload_empty",
    "permissioned_future_payload_active",
    "permissioned_future_payload_active_empty",
    "permissioned_future_vote_roster",
    "permissioned_next_epoch_payload_empty",
    "npos_future_no_payload_empty",
    "npos_future_payload_active"
  }

PayloadKnown(c) ==
  c \in {
    "permissioned_future_payload_active",
    "permissioned_future_payload_active_empty",
    "permissioned_next_epoch_payload_empty",
    "npos_future_payload_active"
  }

SameEpoch(c) ==
  c # "permissioned_next_epoch_payload_empty"

VoteRosterKnown(c) ==
  c = "permissioned_future_vote_roster"

ActiveFallbackNonempty(c) ==
  c \in {
    "permissioned_future_no_payload_empty",
    "permissioned_future_payload_active",
    "permissioned_next_active",
    "permissioned_next_epoch_payload_empty",
    "npos_future_no_payload_empty",
    "npos_future_payload_active",
    "npos_next_active"
  }

FallbackAllowed(c) ==
  ~BeyondNextHeight(c) \/ (PayloadKnown(c) /\ SameEpoch(c))

SpecRosterNonempty(c) ==
  \/ /\ PayloadKnown(c)
     /\ SameEpoch(c)
     /\ BeyondNextHeight(c)
     /\ ActiveFallbackNonempty(c)
  \/ VoteRosterKnown(c)
  \/ /\ FallbackAllowed(c)
     /\ ActiveFallbackNonempty(c)

ActualRosterNonempty(c) ==
  CASE Bug = "next_height_skip_active_fallback"
       /\ c = "permissioned_next_active" -> FALSE
    [] Bug = "payload_same_epoch_skip_active_fallback"
       /\ c = "permissioned_future_payload_active" -> FALSE
    [] Bug = "future_without_payload_uses_active"
       /\ c = "permissioned_future_no_payload_empty" -> TRUE
    [] Bug = "next_epoch_payload_uses_active"
       /\ c = "permissioned_next_epoch_payload_empty" -> TRUE
    [] Bug = "ignore_vote_roster"
       /\ c = "permissioned_future_vote_roster" -> FALSE
    [] Bug = "active_empty_treated_nonempty"
       /\ c = "permissioned_next_active_empty" -> TRUE
    [] OTHER -> SpecRosterNonempty(c)

SpecAllow(c) ==
  Mode(c) = Permissioned /\ ~SpecRosterNonempty(c)

ActualAllow(c) ==
  CASE Bug = "allow_npos_empty"
       /\ c = "npos_future_no_payload_empty" -> TRUE
    [] Bug = "allow_npos_with_roster"
       /\ c = "npos_future_payload_active" -> TRUE
    [] Bug = "reject_permissioned_empty"
       /\ c = "permissioned_future_no_payload_empty" -> FALSE
    [] Bug = "reject_permissioned_active_empty"
       /\ c = "permissioned_next_active_empty" -> FALSE
    [] Bug = "allow_with_nonempty_roster"
       /\ c = "permissioned_next_active" -> TRUE
    [] OTHER -> Mode(c) = Permissioned /\ ~ActualRosterNonempty(c)

Matches(c) ==
  /\ ActualRosterNonempty(c) = SpecRosterNonempty(c)
  /\ ActualAllow(c) = SpecAllow(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 11
     /\ checked' = checked + 1
  \/ /\ checked = 11
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in {
       "none",
       "next_height_skip_active_fallback",
       "payload_same_epoch_skip_active_fallback",
       "future_without_payload_uses_active",
       "next_epoch_payload_uses_active",
       "ignore_vote_roster",
       "active_empty_treated_nonempty",
       "allow_npos_empty",
       "allow_npos_with_roster",
       "reject_permissioned_empty",
       "reject_permissioned_active_empty",
       "allow_with_nonempty_roster"
     }
  /\ checked \in 0..11

RbcUnverifiedRosterMatchesSpec ==
  \A c \in Cases: Matches(c)

SafetyFast ==
  RbcUnverifiedRosterMatchesSpec

AllCasesMatch ==
  \A c \in Cases:
    Matches(c)

AllRosterAvailabilityMatches ==
  \A c \in Cases:
    ActualRosterNonempty(c) = SpecRosterNonempty(c)

AllAllowDecisionsMatch ==
  \A c \in Cases:
    ActualAllow(c) = SpecAllow(c)

PermissionedAllowAnchors ==
  /\ ActualAllow("permissioned_future_no_payload_empty")
  /\ ActualAllow("permissioned_future_payload_active_empty")
  /\ ActualAllow("permissioned_next_active_empty")
  /\ ActualAllow("permissioned_next_epoch_payload_empty")
  /\ ~ActualAllow("permissioned_future_payload_active")
  /\ ~ActualAllow("permissioned_next_active")
  /\ ~ActualAllow("permissioned_future_vote_roster")

NposRejectAnchors ==
  /\ ~ActualAllow("npos_future_no_payload_empty")
  /\ ~ActualAllow("npos_future_payload_active")
  /\ ~ActualAllow("npos_next_active")

RosterAvailabilityAnchors ==
  /\ ~ActualRosterNonempty("permissioned_future_no_payload_empty")
  /\ ActualRosterNonempty("permissioned_future_payload_active")
  /\ ~ActualRosterNonempty("permissioned_future_payload_active_empty")
  /\ ActualRosterNonempty("permissioned_next_active")
  /\ ~ActualRosterNonempty("permissioned_next_active_empty")
  /\ ActualRosterNonempty("permissioned_future_vote_roster")
  /\ ~ActualRosterNonempty("permissioned_next_epoch_payload_empty")
  /\ ~ActualRosterNonempty("npos_future_no_payload_empty")
  /\ ActualRosterNonempty("npos_future_payload_active")
  /\ ActualRosterNonempty("npos_next_active")

FallbackRuleAnchors ==
  /\ ~FallbackAllowed("permissioned_future_no_payload_empty")
  /\ FallbackAllowed("permissioned_future_payload_active")
  /\ FallbackAllowed("permissioned_next_active")
  /\ ~FallbackAllowed("permissioned_next_epoch_payload_empty")
  /\ VoteRosterKnown("permissioned_future_vote_roster")
  /\ ActiveFallbackNonempty("permissioned_next_active")
  /\ ~ActiveFallbackNonempty("permissioned_next_active_empty")

UnverifiedRosterSafetyAnchors ==
  /\ AllCasesMatch
  /\ AllRosterAvailabilityMatches
  /\ AllAllowDecisionsMatch
  /\ PermissionedAllowAnchors
  /\ NposRejectAnchors
  /\ RosterAvailabilityAnchors
  /\ FallbackRuleAnchors

RbcUnverifiedRosterExactness ==
  /\ RbcUnverifiedRosterMatchesSpec
  /\ UnverifiedRosterSafetyAnchors

RbcUnverifiedRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcUnverifiedRosterExactness

PermissionedFutureNoPayloadAllows ==
  Matches("permissioned_future_no_payload_empty")

PermissionedFuturePayloadActiveRejects ==
  Matches("permissioned_future_payload_active")

PermissionedFuturePayloadActiveEmptyAllows ==
  Matches("permissioned_future_payload_active_empty")

PermissionedNextActiveRejects ==
  Matches("permissioned_next_active")

PermissionedNextActiveEmptyAllows ==
  Matches("permissioned_next_active_empty")

PermissionedFutureVoteRosterRejects ==
  Matches("permissioned_future_vote_roster")

PermissionedNextEpochPayloadAllows ==
  Matches("permissioned_next_epoch_payload_empty")

NposFutureNoPayloadRejects ==
  Matches("npos_future_no_payload_empty")

NposFuturePayloadActiveRejects ==
  Matches("npos_future_payload_active")

NposNextActiveRejects ==
  Matches("npos_next_active")

====

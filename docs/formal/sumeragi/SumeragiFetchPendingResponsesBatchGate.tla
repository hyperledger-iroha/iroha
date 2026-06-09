---- MODULE SumeragiFetchPendingResponsesBatchGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `send_fetch_pending_block_responses(...)`.

The helper fans pending missing-block responses out to several requesters:

- commit-QC-only requesters are handled before payload construction and are
  restashed with the commit-QC-only bit when the direct response cannot be sent,
- only non-commit-QC-only requesters receive the block payload,
- consensus-priority payload requesters get an exact BlockBodyResponse
  companion before the payload path,
- hintless `BlockSyncUpdate` responses are decided per requester: only callers
  that allow the hintless path and requesters with known roster proof receive
  the update, while others are downgraded to `BlockCreated`,
- roster-hinted `BlockSyncUpdate` responses send a fitting `BlockCreated`
  companion before the main update, and
- normal `BlockCreated` responses use the rosterless-created bypass only when
  the caller enabled the hintless block-sync bypass.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PeerIds == {"a", "b"}

Cases == {
  "empty_peers",
  "only_commit_qc_direct_success",
  "only_commit_qc_deferred",
  "mixed_commit_qc_and_payload",
  "consensus_payload_companion",
  "background_payload_no_companion",
  "hintless_allowed_peer",
  "hintless_downgraded_no_roster",
  "hintless_downgraded_no_allow",
  "hintless_mixed_two_peers",
  "hintless_consensus_companion",
  "roster_update_companion_fits",
  "roster_update_companion_oversized",
  "plain_created_with_hintless_bypass",
  "plain_created_without_hintless_bypass",
  "plain_other_payload",
  "force_plain_other_payload"
}

RequestPeers(c) ==
  CASE c = "empty_peers" -> {}
    [] c \in {"mixed_commit_qc_and_payload", "hintless_mixed_two_peers"} -> {"a", "b"}
    [] OTHER -> {"a"}

CommitQcOnly(c, p) ==
  CASE c \in {"only_commit_qc_direct_success", "only_commit_qc_deferred"} -> p = "a"
    [] c = "mixed_commit_qc_and_payload" -> p = "a"
    [] OTHER -> FALSE

CommitQcDispatchSucceeds(c, p) ==
  /\ CommitQcOnly(c, p)
  /\ c \in {"only_commit_qc_direct_success", "mixed_commit_qc_and_payload"}

SpecDispatchCommitQcOnly(c, p) ==
  CommitQcOnly(c, p)

SpecRestash(c, p) ==
  /\ CommitQcOnly(c, p)
  /\ ~CommitQcDispatchSucceeds(c, p)

SpecRestashCommitQcOnlyFlag(c, p) ==
  SpecRestash(c, p)

SpecPayloadPeer(c, p) ==
  /\ p \in RequestPeers(c)
  /\ ~CommitQcOnly(c, p)

SpecBuildPayload(c) ==
  \E p \in PeerIds: SpecPayloadPeer(c, p)

PayloadKind(c) ==
  CASE c \in {
       "hintless_allowed_peer",
       "hintless_downgraded_no_roster",
       "hintless_downgraded_no_allow",
       "hintless_mixed_two_peers",
       "hintless_consensus_companion"
     } -> "hintless_update"
    [] c \in {"roster_update_companion_fits", "roster_update_companion_oversized"} -> "roster_update"
    [] c \in {
       "plain_created_with_hintless_bypass",
       "plain_created_without_hintless_bypass"
     } -> "created"
    [] OTHER -> "other"

IsHintlessPayload(c) ==
  PayloadKind(c) = "hintless_update"

IsRosterUpdate(c) ==
  PayloadKind(c) = "roster_update"

ForceBypass(c) ==
  c = "force_plain_other_payload"

AllowHintlessBypass(c) ==
  c \in {
    "hintless_allowed_peer",
    "hintless_downgraded_no_roster",
    "hintless_mixed_two_peers",
    "hintless_consensus_companion",
    "roster_update_companion_fits",
    "roster_update_companion_oversized",
    "plain_created_with_hintless_bypass"
  }

RequesterRosterProofKnown(c, p) ==
  CASE c = "hintless_downgraded_no_roster" -> FALSE
    [] c = "hintless_mixed_two_peers" /\ p = "b" -> FALSE
    [] OTHER -> TRUE

ConsensusPriority(c, p) ==
  /\ p \in RequestPeers(c)
  /\ c \in {"consensus_payload_companion", "hintless_consensus_companion"}

CreatedCompanionFits(c) ==
  c = "roster_update_companion_fits"

SpecExactBodyCompanion(c, p) ==
  /\ SpecPayloadPeer(c, p)
  /\ ConsensusPriority(c, p)

SpecHintlessAllowed(c, p) ==
  /\ SpecPayloadPeer(c, p)
  /\ IsHintlessPayload(c)
  /\ AllowHintlessBypass(c)
  /\ RequesterRosterProofKnown(c, p)

SpecPayloadMessage(c, p) ==
  IF ~SpecPayloadPeer(c, p) THEN "none"
  ELSE CASE IsHintlessPayload(c) ->
            IF SpecHintlessAllowed(c, p)
            THEN "BlockSyncUpdate"
            ELSE "BlockCreated"
         [] IsRosterUpdate(c) -> "BlockSyncUpdate"
         [] PayloadKind(c) = "created" -> "BlockCreated"
         [] OTHER -> "Other"

SpecPayloadSent(c, p) ==
  SpecPayloadPeer(c, p)

SpecCreatedCompanion(c, p) ==
  /\ SpecPayloadPeer(c, p)
  /\ IsRosterUpdate(c)
  /\ CreatedCompanionFits(c)

SpecPayloadPos(c, p) ==
  IF SpecPayloadSent(c, p)
  THEN 1
       + (IF SpecExactBodyCompanion(c, p) THEN 1 ELSE 0)
       + (IF SpecCreatedCompanion(c, p) THEN 1 ELSE 0)
  ELSE 0

SpecCreatedCompanionPos(c, p) ==
  IF SpecCreatedCompanion(c, p)
  THEN 1 + (IF SpecExactBodyCompanion(c, p) THEN 1 ELSE 0)
  ELSE 0

SpecCreatedCompanionBeforePayload(c, p) ==
  IF SpecCreatedCompanion(c, p)
  THEN SpecCreatedCompanionPos(c, p) < SpecPayloadPos(c, p)
  ELSE TRUE

SpecPayloadForceBypassArg(c, p) ==
  IF ~SpecPayloadSent(c, p) THEN FALSE
  ELSE IF IsHintlessPayload(c) THEN ForceBypass(c)
  ELSE
    \/ ForceBypass(c)
    \/ /\ AllowHintlessBypass(c)
       /\ PayloadKind(c) = "created"

SpecPayloadAllowHintlessArg(c, p) ==
  IF ~SpecPayloadSent(c, p) THEN FALSE
  ELSE IF IsHintlessPayload(c)
  THEN SpecHintlessAllowed(c, p)
  ELSE AllowHintlessBypass(c)

SpecPayloadRosterProofArg(c, p) ==
  IF SpecPayloadSent(c, p) THEN RequesterRosterProofKnown(c, p) ELSE FALSE

SpecPayloadConsensusPriorityArg(c, p) ==
  IF SpecPayloadSent(c, p) THEN ConsensusPriority(c, p) ELSE FALSE

SpecExtraFanoutAfterHintless(c) ==
  FALSE

ActualDispatchCommitQcOnly(c, p) ==
  CASE Bug = "skip_commit_qc_only_dispatch"
       /\ c = "only_commit_qc_direct_success"
       /\ p = "a" -> FALSE
    [] OTHER -> SpecDispatchCommitQcOnly(c, p)

ActualRestash(c, p) ==
  CASE Bug = "restash_success"
       /\ c = "only_commit_qc_direct_success"
       /\ p = "a" -> TRUE
    [] Bug = "drop_commit_qc_failure_restash"
       /\ c = "only_commit_qc_deferred"
       /\ p = "a" -> FALSE
    [] OTHER -> SpecRestash(c, p)

ActualRestashCommitQcOnlyFlag(c, p) ==
  IF ~ActualRestash(c, p) THEN FALSE
  ELSE CASE Bug = "restash_not_commit_qc_only"
            /\ c = "only_commit_qc_deferred"
            /\ p = "a" -> FALSE
         [] OTHER -> TRUE

ActualPayloadPeer(c, p) ==
  CASE Bug = "commit_qc_peer_leaks_to_payload"
       /\ c = "mixed_commit_qc_and_payload"
       /\ p = "a" -> TRUE
    [] Bug = "mixed_payload_dropped"
       /\ c = "mixed_commit_qc_and_payload"
       /\ p = "b" -> FALSE
    [] OTHER -> SpecPayloadPeer(c, p)

ActualBuildPayload(c) ==
  CASE Bug = "skip_empty_return"
       /\ c = "empty_peers" -> TRUE
    [] Bug = "build_payload_without_payload_peers"
       /\ c = "only_commit_qc_direct_success" -> TRUE
    [] OTHER -> \E p \in PeerIds: ActualPayloadPeer(c, p)

ActualExactBodyCompanion(c, p) ==
  CASE Bug = "drop_consensus_companion"
       /\ c = "consensus_payload_companion"
       /\ p = "a" -> FALSE
    [] Bug = "background_gets_companion"
       /\ c = "background_payload_no_companion"
       /\ p = "a" -> TRUE
    [] OTHER ->
       /\ ActualPayloadPeer(c, p)
       /\ ConsensusPriority(c, p)

ActualHintlessAllowed(c, p) ==
  CASE Bug = "allow_hintless_without_roster"
       /\ c = "hintless_downgraded_no_roster"
       /\ p = "a" -> TRUE
    [] Bug = "reject_valid_hintless"
       /\ c = "hintless_allowed_peer"
       /\ p = "a" -> FALSE
    [] Bug = "hintless_no_allow_sent_update"
       /\ c = "hintless_downgraded_no_allow"
       /\ p = "a" -> TRUE
    [] Bug = "hintless_mixed_one_policy_for_all"
       /\ c = "hintless_mixed_two_peers"
       /\ p = "b" -> TRUE
    [] OTHER ->
       /\ ActualPayloadPeer(c, p)
       /\ IsHintlessPayload(c)
       /\ AllowHintlessBypass(c)
       /\ RequesterRosterProofKnown(c, p)

ActualPayloadSent(c, p) ==
  CASE Bug = "roster_main_dropped"
       /\ c = "roster_update_companion_fits"
       /\ p = "a" -> FALSE
    [] OTHER -> ActualPayloadPeer(c, p)

ActualPayloadMessage(c, p) ==
  IF ~ActualPayloadSent(c, p) THEN "none"
  ELSE CASE IsHintlessPayload(c) ->
            IF ActualHintlessAllowed(c, p)
            THEN "BlockSyncUpdate"
            ELSE "BlockCreated"
         [] IsRosterUpdate(c) -> "BlockSyncUpdate"
         [] PayloadKind(c) = "created" -> "BlockCreated"
         [] OTHER -> "Other"

ActualCreatedCompanion(c, p) ==
  CASE Bug = "roster_companion_dropped_when_fits"
       /\ c = "roster_update_companion_fits"
       /\ p = "a" -> FALSE
    [] Bug = "roster_companion_sent_when_oversized"
       /\ c = "roster_update_companion_oversized"
       /\ p = "a" -> TRUE
    [] OTHER ->
       /\ ActualPayloadPeer(c, p)
       /\ IsRosterUpdate(c)
       /\ CreatedCompanionFits(c)

ActualPayloadPos(c, p) ==
  IF ActualPayloadSent(c, p)
  THEN 1
       + (IF ActualExactBodyCompanion(c, p) THEN 1 ELSE 0)
       + (IF /\ ActualCreatedCompanion(c, p)
             /\ ~(Bug = "roster_companion_after_main"
                  /\ c = "roster_update_companion_fits"
                  /\ p = "a")
          THEN 1 ELSE 0)
  ELSE 0

ActualCreatedCompanionPos(c, p) ==
  IF ~ActualCreatedCompanion(c, p) THEN 0
  ELSE CASE Bug = "roster_companion_after_main"
            /\ c = "roster_update_companion_fits"
            /\ p = "a" -> ActualPayloadPos(c, p) + 1
         [] OTHER -> 1 + (IF ActualExactBodyCompanion(c, p) THEN 1 ELSE 0)

ActualCreatedCompanionBeforePayload(c, p) ==
  IF ActualCreatedCompanion(c, p)
  THEN ActualCreatedCompanionPos(c, p) < ActualPayloadPos(c, p)
  ELSE TRUE

ActualPayloadForceBypassArg(c, p) ==
  IF ~ActualPayloadSent(c, p) THEN FALSE
  ELSE CASE Bug = "created_bypass_missing"
            /\ c = "plain_created_with_hintless_bypass"
            /\ p = "a" -> FALSE
         [] Bug = "created_bypass_without_allow"
            /\ c = "plain_created_without_hintless_bypass"
            /\ p = "a" -> TRUE
         [] Bug = "force_bypass_lost"
            /\ c = "force_plain_other_payload"
            /\ p = "a" -> FALSE
         [] IsHintlessPayload(c) -> ForceBypass(c)
         [] OTHER ->
            \/ ForceBypass(c)
            \/ /\ AllowHintlessBypass(c)
               /\ PayloadKind(c) = "created"

ActualPayloadAllowHintlessArg(c, p) ==
  IF ~ActualPayloadSent(c, p) THEN FALSE
  ELSE CASE Bug = "hintless_drops_allow_arg"
            /\ c = "hintless_allowed_peer"
            /\ p = "a" -> FALSE
         [] Bug = "nonhintless_allow_arg_lost"
            /\ c = "roster_update_companion_fits"
            /\ p = "a" -> FALSE
         [] IsHintlessPayload(c) -> ActualHintlessAllowed(c, p)
         [] OTHER -> AllowHintlessBypass(c)

ActualPayloadRosterProofArg(c, p) ==
  IF ~ActualPayloadSent(c, p) THEN FALSE
  ELSE CASE Bug = "requester_roster_arg_lost"
            /\ c = "hintless_allowed_peer"
            /\ p = "a" -> FALSE
         [] OTHER -> RequesterRosterProofKnown(c, p)

ActualPayloadConsensusPriorityArg(c, p) ==
  IF ~ActualPayloadSent(c, p) THEN FALSE
  ELSE CASE Bug = "consensus_priority_dropped"
            /\ c = "consensus_payload_companion"
            /\ p = "a" -> FALSE
         [] OTHER -> ConsensusPriority(c, p)

ActualExtraFanoutAfterHintless(c) ==
  /\ Bug = "hintless_main_fanout_after_branch"
  /\ c = "hintless_allowed_peer"

Matches(c) ==
  /\ ActualBuildPayload(c) = SpecBuildPayload(c)
  /\ ActualExtraFanoutAfterHintless(c) = SpecExtraFanoutAfterHintless(c)
  /\ \A p \in PeerIds:
       /\ ActualDispatchCommitQcOnly(c, p) = SpecDispatchCommitQcOnly(c, p)
       /\ ActualRestash(c, p) = SpecRestash(c, p)
       /\ ActualRestashCommitQcOnlyFlag(c, p) = SpecRestashCommitQcOnlyFlag(c, p)
       /\ ActualPayloadPeer(c, p) = SpecPayloadPeer(c, p)
       /\ ActualExactBodyCompanion(c, p) = SpecExactBodyCompanion(c, p)
       /\ ActualHintlessAllowed(c, p) = SpecHintlessAllowed(c, p)
       /\ ActualPayloadSent(c, p) = SpecPayloadSent(c, p)
       /\ ActualPayloadMessage(c, p) = SpecPayloadMessage(c, p)
       /\ ActualCreatedCompanion(c, p) = SpecCreatedCompanion(c, p)
       /\ ActualPayloadPos(c, p) = SpecPayloadPos(c, p)
       /\ ActualCreatedCompanionPos(c, p) = SpecCreatedCompanionPos(c, p)
       /\ ActualCreatedCompanionBeforePayload(c, p) = SpecCreatedCompanionBeforePayload(c, p)
       /\ ActualPayloadForceBypassArg(c, p) = SpecPayloadForceBypassArg(c, p)
       /\ ActualPayloadAllowHintlessArg(c, p) = SpecPayloadAllowHintlessArg(c, p)
       /\ ActualPayloadRosterProofArg(c, p) = SpecPayloadRosterProofArg(c, p)
       /\ ActualPayloadConsensusPriorityArg(c, p) = SpecPayloadConsensusPriorityArg(c, p)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_empty_return",
       "skip_commit_qc_only_dispatch",
       "restash_success",
       "drop_commit_qc_failure_restash",
       "restash_not_commit_qc_only",
       "commit_qc_peer_leaks_to_payload",
       "mixed_payload_dropped",
       "build_payload_without_payload_peers",
       "drop_consensus_companion",
       "background_gets_companion",
       "allow_hintless_without_roster",
       "reject_valid_hintless",
       "hintless_no_allow_sent_update",
       "hintless_mixed_one_policy_for_all",
       "hintless_main_fanout_after_branch",
       "hintless_drops_allow_arg",
       "requester_roster_arg_lost",
       "roster_companion_dropped_when_fits",
       "roster_companion_sent_when_oversized",
       "roster_companion_after_main",
       "roster_main_dropped",
       "created_bypass_missing",
       "created_bypass_without_allow",
       "force_bypass_lost",
       "nonhintless_allow_arg_lost",
       "consensus_priority_dropped"
     }
  /\ checked = 0

FetchPendingResponsesBatchMatchesSpec ==
  \A c \in Cases: Matches(c)

SafetyFast ==
  FetchPendingResponsesBatchMatchesSpec

EmptyPeerMapReturns ==
  Matches("empty_peers")

CommitQcOnlyDispatches ==
  Matches("only_commit_qc_direct_success")

CommitQcSuccessNotRestashed ==
  Matches("only_commit_qc_direct_success")

CommitQcFailureRestashed ==
  Matches("only_commit_qc_deferred")

CommitQcRestashKeepsFlag ==
  Matches("only_commit_qc_deferred")

CommitQcOnlyExcludedFromPayload ==
  Matches("mixed_commit_qc_and_payload")

MixedPayloadStillSent ==
  Matches("mixed_commit_qc_and_payload")

NoPayloadSkipsBuild ==
  Matches("only_commit_qc_direct_success")

ConsensusCompanionSent ==
  Matches("consensus_payload_companion")

BackgroundSkipsCompanion ==
  Matches("background_payload_no_companion")

HintlessRequiresRosterProof ==
  Matches("hintless_downgraded_no_roster")

ValidHintlessAllowed ==
  Matches("hintless_allowed_peer")

HintlessRequiresAllowFlag ==
  Matches("hintless_downgraded_no_allow")

HintlessPolicyPerPeer ==
  Matches("hintless_mixed_two_peers")

HintlessBranchReturns ==
  Matches("hintless_allowed_peer")

HintlessAllowArgForwarded ==
  /\ Bug # "hintless_drops_allow_arg"
  /\ ActualPayloadAllowHintlessArg("hintless_allowed_peer", "a")
  /\ Matches("hintless_allowed_peer")

RequesterRosterArgForwarded ==
  Matches("hintless_allowed_peer")

RosterCompanionSentWhenFits ==
  Matches("roster_update_companion_fits")

RosterCompanionSkippedWhenOversized ==
  Matches("roster_update_companion_oversized")

RosterCompanionBeforeMain ==
  Matches("roster_update_companion_fits")

RosterMainSent ==
  Matches("roster_update_companion_fits")

CreatedBypassWhenAllowed ==
  Matches("plain_created_with_hintless_bypass")

CreatedBypassRequiresAllow ==
  Matches("plain_created_without_hintless_bypass")

ForceBypassForwarded ==
  Matches("force_plain_other_payload")

NonHintlessAllowArgForwarded ==
  Matches("roster_update_companion_fits")

ConsensusPriorityForwarded ==
  Matches("consensus_payload_companion")

====

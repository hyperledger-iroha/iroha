---- MODULE SumeragiCachedProposalRebroadcastGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `maybe_rebroadcast_cached_proposal(...)`.

This slice pins the cached proposal replay path that keeps frontier recovery
moving without changing the proposal semantics. Replay must require retained
proposal or owner metadata, an active matching pending block, a usable live
topology containing the local peer, selected-leader agreement, proposal
metadata rebuilt from cache/hint/authoritative frontier state, an available
frontier `BlockCreated` wire payload, and at least one remote validator. Normal
replay respects relay backpressure and cooldowns; frontier-recovery replay may
bypass relay backpressure and uses the shorter recovery nudge cooldown.
Successful replay posts the enriched body, proposal hint, and proposal metadata
to every remote validator and never to the local peer.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == "none"

Bugs == {
  NoBug,
  "accept_no_signal",
  "accept_aborted_pending",
  "accept_payload_mismatch",
  "accept_owner_hint_mismatch",
  "accept_empty_roster",
  "accept_leader_error",
  "accept_local_not_validator",
  "accept_wrong_cached_leader",
  "accept_wrong_hint_block",
  "accept_without_metadata",
  "ignore_normal_relay_backpressure",
  "block_recovery_relay_backpressure",
  "recovery_uses_payload_cooldown",
  "normal_uses_recovery_cooldown",
  "ignore_cooldown",
  "accept_missing_frontier_wire",
  "accept_no_remote_validators",
  "skip_block_created_targets",
  "skip_proposal_hint_targets",
  "skip_proposal_targets",
  "send_to_local_peer",
  "drop_success_return",
  "hint_rebuild_not_cached",
  "authoritative_rebuild_not_cached",
  "remote_leader_not_relayed"
}

NoBlock == 0
BlockHash == 1
NoTargets == 0
AllRemotes == 1
PartialRemotes == 2
LocalPeerTargeted == 3

PayloadCooldown == 250
RecoveryNudgeCooldown == 125

RejectNoSignal ==
  IF Bug = "accept_no_signal" THEN TRUE ELSE FALSE

RejectAbortedPending ==
  IF Bug = "accept_aborted_pending" THEN TRUE ELSE FALSE

RejectPayloadMismatch ==
  IF Bug = "accept_payload_mismatch" THEN TRUE ELSE FALSE

RejectOwnerHintMismatch ==
  IF Bug = "accept_owner_hint_mismatch" THEN TRUE ELSE FALSE

RejectEmptyRoster ==
  IF Bug = "accept_empty_roster" THEN TRUE ELSE FALSE

RejectLeaderError ==
  IF Bug = "accept_leader_error" THEN TRUE ELSE FALSE

RejectLocalNotValidator ==
  IF Bug = "accept_local_not_validator" THEN TRUE ELSE FALSE

RejectWrongCachedLeader ==
  IF Bug = "accept_wrong_cached_leader" THEN TRUE ELSE FALSE

RejectWrongHintBlock ==
  IF Bug = "accept_wrong_hint_block" THEN TRUE ELSE FALSE

RejectNoMetadata ==
  IF Bug = "accept_without_metadata" THEN TRUE ELSE FALSE

NormalRelayBackpressureAllows ==
  IF Bug = "ignore_normal_relay_backpressure" THEN TRUE ELSE FALSE

RecoveryRelayBackpressureAllows ==
  IF Bug = "block_recovery_relay_backpressure" THEN FALSE ELSE TRUE

RecoveryCooldown ==
  IF Bug = "recovery_uses_payload_cooldown" THEN PayloadCooldown ELSE RecoveryNudgeCooldown

NormalCooldown ==
  IF Bug = "normal_uses_recovery_cooldown" THEN RecoveryNudgeCooldown ELSE PayloadCooldown

CooldownBlockedAllows ==
  IF Bug = "ignore_cooldown" THEN TRUE ELSE FALSE

RejectMissingFrontierWire ==
  IF Bug = "accept_missing_frontier_wire" THEN TRUE ELSE FALSE

RejectNoRemoteValidators ==
  IF Bug = "accept_no_remote_validators" THEN TRUE ELSE FALSE

BlockCreatedTargets ==
  IF Bug = "skip_block_created_targets" THEN PartialRemotes ELSE AllRemotes

ProposalHintTargets ==
  IF Bug = "skip_proposal_hint_targets" THEN PartialRemotes ELSE AllRemotes

ProposalTargets ==
  IF Bug = "skip_proposal_targets" THEN PartialRemotes ELSE AllRemotes

LocalPeerReceivesReplay ==
  IF Bug = "send_to_local_peer" THEN TRUE ELSE FALSE

SuccessReturn ==
  IF Bug = "drop_success_return" THEN NoBlock ELSE BlockHash

HintRebuildCachesProposal ==
  IF Bug = "hint_rebuild_not_cached" THEN FALSE ELSE TRUE

AuthoritativeRebuildCachesProposal ==
  IF Bug = "authoritative_rebuild_not_cached" THEN FALSE ELSE TRUE

RemoteLeaderReplayAllowed ==
  IF Bug = "remote_leader_not_relayed" THEN FALSE ELSE TRUE

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..25
  /\ RecoveryNudgeCooldown < PayloadCooldown

AdmissionRejectsUnsafeInputs ==
  /\ ~RejectNoSignal
  /\ ~RejectAbortedPending
  /\ ~RejectPayloadMismatch
  /\ ~RejectOwnerHintMismatch
  /\ ~RejectEmptyRoster
  /\ ~RejectLeaderError
  /\ ~RejectLocalNotValidator
  /\ ~RejectWrongCachedLeader
  /\ ~RejectWrongHintBlock
  /\ ~RejectNoMetadata
  /\ ~RejectMissingFrontierWire
  /\ ~RejectNoRemoteValidators

BackpressureAndCooldownSafety ==
  /\ ~NormalRelayBackpressureAllows
  /\ RecoveryRelayBackpressureAllows
  /\ NormalCooldown = PayloadCooldown
  /\ RecoveryCooldown = RecoveryNudgeCooldown
  /\ ~CooldownBlockedAllows

RebuildSourceSafety ==
  /\ HintRebuildCachesProposal
  /\ AuthoritativeRebuildCachesProposal
  /\ RemoteLeaderReplayAllowed

ReplayFanoutSafety ==
  /\ BlockCreatedTargets = AllRemotes
  /\ ProposalHintTargets = AllRemotes
  /\ ProposalTargets = AllRemotes
  /\ ~LocalPeerReceivesReplay
  /\ SuccessReturn = BlockHash

SafetyFast ==
  /\ AdmissionRejectsUnsafeInputs
  /\ BackpressureAndCooldownSafety
  /\ RebuildSourceSafety
  /\ ReplayFanoutSafety

AdmissionAnchors ==
  /\ AdmissionRejectsUnsafeInputs
  /\ ~RejectNoSignal
  /\ ~RejectAbortedPending
  /\ ~RejectPayloadMismatch
  /\ ~RejectOwnerHintMismatch
  /\ ~RejectEmptyRoster
  /\ ~RejectLeaderError
  /\ ~RejectLocalNotValidator
  /\ ~RejectWrongCachedLeader
  /\ ~RejectWrongHintBlock
  /\ ~RejectNoMetadata
  /\ ~RejectMissingFrontierWire
  /\ ~RejectNoRemoteValidators

BackpressureCooldownAnchors ==
  /\ BackpressureAndCooldownSafety
  /\ ~NormalRelayBackpressureAllows
  /\ RecoveryRelayBackpressureAllows
  /\ NormalCooldown = PayloadCooldown
  /\ RecoveryCooldown = RecoveryNudgeCooldown
  /\ ~CooldownBlockedAllows

RebuildSourceAnchors ==
  /\ RebuildSourceSafety
  /\ HintRebuildCachesProposal
  /\ AuthoritativeRebuildCachesProposal
  /\ RemoteLeaderReplayAllowed

ReplayFanoutAnchors ==
  /\ ReplayFanoutSafety
  /\ BlockCreatedTargets = AllRemotes
  /\ ProposalHintTargets = AllRemotes
  /\ ProposalTargets = AllRemotes
  /\ ~LocalPeerReceivesReplay
  /\ SuccessReturn = BlockHash

CachedProposalRebroadcastSafetyAnchors ==
  /\ AdmissionAnchors
  /\ BackpressureCooldownAnchors
  /\ RebuildSourceAnchors
  /\ ReplayFanoutAnchors

CachedProposalRebroadcastExactness ==
  /\ AdmissionRejectsUnsafeInputs
  /\ BackpressureAndCooldownSafety
  /\ RebuildSourceSafety
  /\ ReplayFanoutSafety
  /\ CachedProposalRebroadcastSafetyAnchors

CachedProposalRebroadcastCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CachedProposalRebroadcastExactness

Safety == CachedProposalRebroadcastSafetyAnchors

====

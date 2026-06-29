---- MODULE SumeragiVNextControlIngressGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for vNext control-certificate ingress in the actor.

The helper-level vNext models prove chain-order, re-chain, aggregate
signature, and signing-preimage rules. This actor-level gate checks the state
effects around `handle_vnext_rechain_certificate_received(...)`,
`require_vnext_view_change(...)`, and
`handle_vnext_view_change_certificate_received(...)`: missing rounds do not
mutate live round state, already-current re-chain certificates are no-ops,
invalid re-chain certificates are rejected without escalation, valid re-chain
certificates update and install only while within the taint bound, quorum-
weakening or over-taint evidence requires a live view change, and view-change
certificates abort only an installed highest slot while nonzero new views
trigger local view-change handling.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  chainUpdated,
  \* @type: Bool;
  lastRechainRecorded,
  \* @type: Bool;
  installCert,
  \* @type: Bool;
  rejectCert,
  \* @type: Bool;
  requireViewChange,
  \* @type: Bool;
  clearWorkerOwner,
  \* @type: Bool;
  broadcastViewChangeVote,
  \* @type: Bool;
  triggerViewChange,
  \* @type: Bool;
  abortHighestSlot

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    chainUpdated,
    lastRechainRecorded,
    installCert,
    rejectCert,
    requireViewChange,
    clearWorkerOwner,
    broadcastViewChangeVote,
    triggerViewChange,
    abortHighestSlot>>

Cases == {
  "rechain_no_round",
  "rechain_already_current",
  "rechain_hash_mismatch",
  "rechain_valid_within_max",
  "rechain_valid_exceeds_max",
  "rechain_would_weaken_quorum",
  "rechain_evidence_mismatch",
  "view_highest_installed",
  "view_highest_missing_round",
  "view_no_highest",
  "view_zero_highest_installed"
}

RechainCases == {
  "rechain_no_round",
  "rechain_already_current",
  "rechain_hash_mismatch",
  "rechain_valid_within_max",
  "rechain_valid_exceeds_max",
  "rechain_would_weaken_quorum",
  "rechain_evidence_mismatch"
}

ViewCases == {
  "view_highest_installed",
  "view_highest_missing_round",
  "view_no_highest",
  "view_zero_highest_installed"
}

RejectedRechainCases == {
  "rechain_hash_mismatch",
  "rechain_evidence_mismatch"
}

RequireRechainCases == {
  "rechain_valid_exceeds_max",
  "rechain_would_weaken_quorum"
}

NonzeroViewCases == {
  "view_highest_installed",
  "view_highest_missing_round",
  "view_no_highest"
}

SpecChainUpdated(c) ==
  c = "rechain_valid_within_max"

SpecLastRechainRecorded(c) ==
  c = "rechain_valid_within_max"

SpecInstall(c) ==
  c \in {"rechain_no_round", "rechain_valid_within_max"} \/ c \in ViewCases

SpecReject(c) ==
  c \in RejectedRechainCases

SpecRequire(c) ==
  c \in RequireRechainCases

SpecClearWorker(c) ==
  SpecRequire(c)

SpecBroadcastViewChangeVote(c) ==
  SpecRequire(c)

SpecTriggerViewChange(c) ==
  c \in RequireRechainCases \/ c \in NonzeroViewCases

SpecAbortHighest(c) ==
  c \in {"view_highest_installed", "view_zero_highest_installed"}

ActualChainUpdated(c) ==
  CASE c = "rechain_no_round" /\ Bug = "rechain_no_round_updates" -> TRUE
    [] c = "rechain_already_current" /\ Bug = "rechain_current_updates" -> TRUE
    [] c = "rechain_valid_within_max" /\ Bug = "rechain_valid_no_update" -> FALSE
    [] c = "rechain_valid_exceeds_max" /\ Bug = "rechain_excess_updates" -> TRUE
    [] c = "rechain_would_weaken_quorum" /\ Bug = "rechain_weakened_updates" -> TRUE
    [] c = "rechain_evidence_mismatch" /\ Bug = "rechain_evidence_updates" -> TRUE
    [] OTHER -> SpecChainUpdated(c)

ActualLastRechainRecorded(c) ==
  CASE c = "rechain_valid_within_max" /\ Bug = "rechain_valid_no_last_rechain" -> FALSE
    [] c # "rechain_valid_within_max" /\ Bug = "rechain_records_last_without_update" -> TRUE
    [] OTHER -> SpecLastRechainRecorded(c)

ActualInstall(c) ==
  CASE c = "rechain_hash_mismatch" /\ Bug = "rechain_hash_mismatch_installs" -> TRUE
    [] c = "rechain_evidence_mismatch" /\ Bug = "rechain_evidence_installs" -> TRUE
    [] c = "rechain_valid_within_max" /\ Bug = "rechain_valid_no_install" -> FALSE
    [] c \in ViewCases /\ Bug = "view_no_install" -> FALSE
    [] OTHER -> SpecInstall(c)

ActualReject(c) ==
  CASE c = "rechain_already_current" /\ Bug = "rechain_current_rejects" -> TRUE
    [] c = "rechain_hash_mismatch" /\ Bug = "rechain_hash_mismatch_no_reject" -> FALSE
    [] c = "rechain_evidence_mismatch" /\ Bug = "rechain_evidence_no_reject" -> FALSE
    [] c = "rechain_valid_within_max" /\ Bug = "rechain_valid_rejects" -> TRUE
    [] OTHER -> SpecReject(c)

ActualRequire(c) ==
  CASE c = "rechain_no_round" /\ Bug = "rechain_no_round_requires" -> TRUE
    [] c = "rechain_hash_mismatch" /\ Bug = "rechain_hash_mismatch_requires" -> TRUE
    [] c = "rechain_valid_exceeds_max" /\ Bug = "rechain_excess_no_require" -> FALSE
    [] c = "rechain_would_weaken_quorum" /\ Bug = "rechain_weakened_no_require" -> FALSE
    [] OTHER -> SpecRequire(c)

ActualClearWorker(c) ==
  CASE c \in RequireRechainCases /\ Bug = "rechain_require_no_clear" -> FALSE
    [] OTHER -> SpecClearWorker(c)

ActualBroadcastViewChangeVote(c) ==
  CASE c \in RequireRechainCases /\ Bug = "rechain_require_no_vote" -> FALSE
    [] OTHER -> SpecBroadcastViewChangeVote(c)

ActualTriggerViewChange(c) ==
  CASE c \in RequireRechainCases /\ Bug = "rechain_require_no_trigger" -> FALSE
    [] c \in NonzeroViewCases /\ Bug = "view_nonzero_no_trigger" -> FALSE
    [] c = "view_zero_highest_installed" /\ Bug = "view_zero_triggers" -> TRUE
    [] OTHER -> SpecTriggerViewChange(c)

ActualAbortHighest(c) ==
  CASE c = "view_highest_installed" /\ Bug = "view_highest_no_abort" -> FALSE
    [] c = "view_zero_highest_installed" /\ Bug = "view_highest_no_abort" -> FALSE
    [] c = "view_highest_missing_round" /\ Bug = "view_missing_round_aborts" -> TRUE
    [] c = "view_no_highest" /\ Bug = "view_no_highest_aborts" -> TRUE
    [] OTHER -> SpecAbortHighest(c)

BugModes == {
  "none",
  "rechain_no_round_updates",
  "rechain_no_round_requires",
  "rechain_current_rejects",
  "rechain_current_updates",
  "rechain_hash_mismatch_installs",
  "rechain_hash_mismatch_no_reject",
  "rechain_hash_mismatch_requires",
  "rechain_valid_no_update",
  "rechain_valid_no_last_rechain",
  "rechain_valid_no_install",
  "rechain_valid_rejects",
  "rechain_excess_updates",
  "rechain_excess_no_require",
  "rechain_weakened_updates",
  "rechain_weakened_no_require",
  "rechain_evidence_updates",
  "rechain_evidence_installs",
  "rechain_evidence_no_reject",
  "rechain_require_no_clear",
  "rechain_require_no_vote",
  "rechain_require_no_trigger",
  "rechain_records_last_without_update",
  "view_highest_no_abort",
  "view_missing_round_aborts",
  "view_no_highest_aborts",
  "view_zero_triggers",
  "view_nonzero_no_trigger",
  "view_no_install"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ chainUpdated \in BOOLEAN
  /\ lastRechainRecorded \in BOOLEAN
  /\ installCert \in BOOLEAN
  /\ rejectCert \in BOOLEAN
  /\ requireViewChange \in BOOLEAN
  /\ clearWorkerOwner \in BOOLEAN
  /\ broadcastViewChangeVote \in BOOLEAN
  /\ triggerViewChange \in BOOLEAN
  /\ abortHighestSlot \in BOOLEAN

Init ==
  /\ candidate = "rechain_already_current"
  /\ chainUpdated = FALSE
  /\ lastRechainRecorded = FALSE
  /\ installCert = FALSE
  /\ rejectCert = FALSE
  /\ requireViewChange = FALSE
  /\ clearWorkerOwner = FALSE
  /\ broadcastViewChangeVote = FALSE
  /\ triggerViewChange = FALSE
  /\ abortHighestSlot = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ chainUpdated' = ActualChainUpdated(c)
  /\ lastRechainRecorded' = ActualLastRechainRecorded(c)
  /\ installCert' = ActualInstall(c)
  /\ rejectCert' = ActualReject(c)
  /\ requireViewChange' = ActualRequire(c)
  /\ clearWorkerOwner' = ActualClearWorker(c)
  /\ broadcastViewChangeVote' = ActualBroadcastViewChangeVote(c)
  /\ triggerViewChange' = ActualTriggerViewChange(c)
  /\ abortHighestSlot' = ActualAbortHighest(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ chainUpdated = SpecChainUpdated(candidate)
  /\ lastRechainRecorded = SpecLastRechainRecorded(candidate)
  /\ installCert = SpecInstall(candidate)
  /\ rejectCert = SpecReject(candidate)
  /\ requireViewChange = SpecRequire(candidate)
  /\ clearWorkerOwner = SpecClearWorker(candidate)
  /\ broadcastViewChangeVote = SpecBroadcastViewChangeVote(candidate)
  /\ triggerViewChange = SpecTriggerViewChange(candidate)
  /\ abortHighestSlot = SpecAbortHighest(candidate)

RechainNoRoundDoesNotMutateLiveRound ==
  candidate = "rechain_no_round" =>
    /\ ~chainUpdated
    /\ ~lastRechainRecorded
    /\ installCert
    /\ ~rejectCert
    /\ ~requireViewChange

CurrentRechainCertificateIsNoOp ==
  candidate = "rechain_already_current" =>
    /\ ~chainUpdated
    /\ ~lastRechainRecorded
    /\ ~installCert
    /\ ~rejectCert
    /\ ~requireViewChange
    /\ ~triggerViewChange

RejectedRechainDoesNotInstallOrEscalate ==
  candidate \in RejectedRechainCases =>
    /\ rejectCert
    /\ ~installCert
    /\ ~requireViewChange
    /\ ~chainUpdated

ValidRechainInstallsAndUpdates ==
  candidate = "rechain_valid_within_max" =>
    /\ chainUpdated
    /\ lastRechainRecorded
    /\ installCert
    /\ ~rejectCert
    /\ ~requireViewChange

RequireViewChangeCasesDoNotInstallOrUpdate ==
  candidate \in RequireRechainCases =>
    /\ requireViewChange
    /\ ~installCert
    /\ ~rejectCert
    /\ ~chainUpdated

RequireViewChangeClearsVotesAndTriggers ==
  requireViewChange =>
    /\ clearWorkerOwner
    /\ broadcastViewChangeVote
    /\ triggerViewChange

LastRechainOnlyWhenChainUpdated ==
  lastRechainRecorded => chainUpdated

ViewCertificateAlwaysInstalls ==
  candidate \in ViewCases => installCert

ViewHighestAbortRequiresInstalledHighestSlot ==
  abortHighestSlot => candidate \in {"view_highest_installed", "view_zero_highest_installed"}

ViewMissingRoundOrNoHighestDoesNotAbort ==
  candidate \in {"view_highest_missing_round", "view_no_highest"} => ~abortHighestSlot

ZeroNewViewDoesNotTrigger ==
  candidate = "view_zero_highest_installed" => ~triggerViewChange

NonzeroViewTriggers ==
  candidate \in NonzeroViewCases => triggerViewChange

VNextControlIngressExactness ==
  /\ MatchesSpec
  /\ RechainNoRoundDoesNotMutateLiveRound
  /\ CurrentRechainCertificateIsNoOp
  /\ RejectedRechainDoesNotInstallOrEscalate
  /\ ValidRechainInstallsAndUpdates
  /\ RequireViewChangeCasesDoNotInstallOrUpdate
  /\ RequireViewChangeClearsVotesAndTriggers
  /\ LastRechainOnlyWhenChainUpdated
  /\ ViewCertificateAlwaysInstalls
  /\ ViewHighestAbortRequiresInstalledHighestSlot
  /\ ViewMissingRoundOrNoHighestDoesNotAbort
  /\ ZeroNewViewDoesNotTrigger
  /\ NonzeroViewTriggers

Safety ==
  VNextControlIngressExactness

VNextControlIngressCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextControlIngressExactness

=============================================================================
====

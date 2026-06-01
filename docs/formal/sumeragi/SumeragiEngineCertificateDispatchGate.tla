---- MODULE SumeragiEngineCertificateDispatchGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for pure-engine certificate prefilter dispatch.

This slice models `ConsensusEngine::on_certificate(...)` before the
phase-specific handlers run. The shared prefilter rejects already committed
heights, wrong height/epoch/validator-set context, wrong quorum policy, and
stale Prepare/Commit views. Matching Prepare and Commit certificates dispatch
only to their corresponding handlers. Matching NewView certificates dispatch
to the NewView handler regardless of lower, same, or higher view; the strict
newer-view check belongs to `on_new_view_qc(...)`, not the shared prefilter.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugDispatchCommittedHeight,
  \* @type: Bool;
  BugDispatchWrongHeight,
  \* @type: Bool;
  BugDispatchWrongEpoch,
  \* @type: Bool;
  BugDispatchWrongValidatorSet,
  \* @type: Bool;
  BugDispatchWrongQuorumPolicy,
  \* @type: Bool;
  BugDispatchStalePrepareCommit,
  \* @type: Bool;
  BugRejectSafePrepare,
  \* @type: Bool;
  BugRejectSafeCommit,
  \* @type: Bool;
  BugRejectNewViewSameOrPastAtPrefilter,
  \* @type: Bool;
  BugRejectNewViewFutureAtPrefilter,
  \* @type: Bool;
  BugDispatchPrepareAsCommit,
  \* @type: Bool;
  BugDispatchCommitAsPrepare,
  \* @type: Bool;
  BugDispatchNewViewAsPrepare

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

Cases == {
  "current_prepare",
  "current_commit",
  "new_view_lower_view",
  "new_view_same_view",
  "new_view_future_view",
  "stale_prepare",
  "stale_commit",
  "committed_prepare",
  "committed_commit",
  "committed_new_view",
  "wrong_height_prepare",
  "wrong_height_commit",
  "wrong_height_new_view",
  "wrong_epoch_prepare",
  "wrong_epoch_commit",
  "wrong_epoch_new_view",
  "wrong_validator_set_prepare",
  "wrong_validator_set_commit",
  "wrong_validator_set_new_view",
  "wrong_quorum_prepare",
  "wrong_quorum_commit",
  "wrong_quorum_new_view"
}

Handlers == {"none", "prepare", "commit", "newView"}

PhaseHandler(candidate) ==
  CASE candidate \in {
      "current_prepare",
      "stale_prepare",
      "committed_prepare",
      "wrong_height_prepare",
      "wrong_epoch_prepare",
      "wrong_validator_set_prepare",
      "wrong_quorum_prepare"
    } -> "prepare"
    [] candidate \in {
      "current_commit",
      "stale_commit",
      "committed_commit",
      "wrong_height_commit",
      "wrong_epoch_commit",
      "wrong_validator_set_commit",
      "wrong_quorum_commit"
    } -> "commit"
    [] OTHER -> "newView"

CommittedHeight(candidate) ==
  candidate \in {
    "committed_prepare",
    "committed_commit",
    "committed_new_view"
  }

WrongHeight(candidate) ==
  candidate \in {
    "wrong_height_prepare",
    "wrong_height_commit",
    "wrong_height_new_view"
  }

WrongEpoch(candidate) ==
  candidate \in {
    "wrong_epoch_prepare",
    "wrong_epoch_commit",
    "wrong_epoch_new_view"
  }

WrongValidatorSet(candidate) ==
  candidate \in {
    "wrong_validator_set_prepare",
    "wrong_validator_set_commit",
    "wrong_validator_set_new_view"
  }

WrongQuorumPolicy(candidate) ==
  candidate \in {
    "wrong_quorum_prepare",
    "wrong_quorum_commit",
    "wrong_quorum_new_view"
  }

StalePrepareCommit(candidate) ==
  candidate \in {"stale_prepare", "stale_commit"}

NewViewSameOrPast(candidate) ==
  candidate \in {"new_view_lower_view", "new_view_same_view"}

NewViewFuture(candidate) ==
  candidate = "new_view_future_view"

SpecHandler(candidate) ==
  IF \/ CommittedHeight(candidate)
     \/ WrongHeight(candidate)
     \/ WrongEpoch(candidate)
     \/ WrongValidatorSet(candidate)
     \/ WrongQuorumPolicy(candidate)
     \/ StalePrepareCommit(candidate)
  THEN "none"
  ELSE PhaseHandler(candidate)

BugAllowsRejected(candidate) ==
  \/ /\ CommittedHeight(candidate)
     /\ BugDispatchCommittedHeight
  \/ /\ WrongHeight(candidate)
     /\ BugDispatchWrongHeight
  \/ /\ WrongEpoch(candidate)
     /\ BugDispatchWrongEpoch
  \/ /\ WrongValidatorSet(candidate)
     /\ BugDispatchWrongValidatorSet
  \/ /\ WrongQuorumPolicy(candidate)
     /\ BugDispatchWrongQuorumPolicy
  \/ /\ StalePrepareCommit(candidate)
     /\ BugDispatchStalePrepareCommit

BugRejectsAccepted(candidate) ==
  \/ /\ candidate = "current_prepare"
     /\ BugRejectSafePrepare
  \/ /\ candidate = "current_commit"
     /\ BugRejectSafeCommit
  \/ /\ NewViewSameOrPast(candidate)
     /\ BugRejectNewViewSameOrPastAtPrefilter
  \/ /\ NewViewFuture(candidate)
     /\ BugRejectNewViewFutureAtPrefilter

BaseHandler(candidate) ==
  IF SpecHandler(candidate) = "none"
  THEN
    IF BugAllowsRejected(candidate)
    THEN PhaseHandler(candidate)
    ELSE "none"
  ELSE
    IF BugRejectsAccepted(candidate)
    THEN "none"
    ELSE PhaseHandler(candidate)

ImplementationHandler(candidate) ==
  CASE /\ BaseHandler(candidate) = "prepare"
       /\ BugDispatchPrepareAsCommit -> "commit"
    [] /\ BaseHandler(candidate) = "commit"
       /\ BugDispatchCommitAsPrepare -> "prepare"
    [] /\ BaseHandler(candidate) = "newView"
       /\ BugDispatchNewViewAsPrepare -> "prepare"
    [] OTHER -> BaseHandler(candidate)

TypeInvariant ==
  /\ BugDispatchCommittedHeight \in BOOLEAN
  /\ BugDispatchWrongHeight \in BOOLEAN
  /\ BugDispatchWrongEpoch \in BOOLEAN
  /\ BugDispatchWrongValidatorSet \in BOOLEAN
  /\ BugDispatchWrongQuorumPolicy \in BOOLEAN
  /\ BugDispatchStalePrepareCommit \in BOOLEAN
  /\ BugRejectSafePrepare \in BOOLEAN
  /\ BugRejectSafeCommit \in BOOLEAN
  /\ BugRejectNewViewSameOrPastAtPrefilter \in BOOLEAN
  /\ BugRejectNewViewFutureAtPrefilter \in BOOLEAN
  /\ BugDispatchPrepareAsCommit \in BOOLEAN
  /\ BugDispatchCommitAsPrepare \in BOOLEAN
  /\ BugDispatchNewViewAsPrepare \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ PhaseHandler(candidate) \in Handlers
    /\ SpecHandler(candidate) \in Handlers
    /\ BaseHandler(candidate) \in Handlers
    /\ ImplementationHandler(candidate) \in Handlers

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

DispatchMatchesSpec ==
  \A candidate \in tried:
    ImplementationHandler(candidate) = SpecHandler(candidate)

CurrentPrepareDispatchesPrepare ==
  "current_prepare" \in tried =>
    ImplementationHandler("current_prepare") = "prepare"

CurrentCommitDispatchesCommit ==
  "current_commit" \in tried =>
    ImplementationHandler("current_commit") = "commit"

NewViewsDispatchToNewViewHandler ==
  \A candidate \in tried:
    candidate \in {
      "new_view_lower_view",
      "new_view_same_view",
      "new_view_future_view"
    } => ImplementationHandler(candidate) = "newView"

WrongContextNeverDispatches ==
  \A candidate \in tried:
    (WrongHeight(candidate) \/ WrongEpoch(candidate) \/ WrongValidatorSet(candidate)) =>
      ImplementationHandler(candidate) = "none"

WrongQuorumNeverDispatches ==
  \A candidate \in tried:
    WrongQuorumPolicy(candidate) => ImplementationHandler(candidate) = "none"

CommittedHeightsNeverDispatch ==
  \A candidate \in tried:
    CommittedHeight(candidate) => ImplementationHandler(candidate) = "none"

StalePrepareCommitNeverDispatches ==
  \A candidate \in tried:
    StalePrepareCommit(candidate) => ImplementationHandler(candidate) = "none"

NoCrossPhaseDispatch ==
  \A candidate \in tried:
    ImplementationHandler(candidate) # "none" =>
      ImplementationHandler(candidate) = PhaseHandler(candidate)

Safety ==
  /\ DispatchMatchesSpec
  /\ CurrentPrepareDispatchesPrepare
  /\ CurrentCommitDispatchesCommit
  /\ NewViewsDispatchToNewViewHandler
  /\ WrongContextNeverDispatches
  /\ WrongQuorumNeverDispatches
  /\ CommittedHeightsNeverDispatch
  /\ StalePrepareCommitNeverDispatches
  /\ NoCrossPhaseDispatch

=============================================================================
====

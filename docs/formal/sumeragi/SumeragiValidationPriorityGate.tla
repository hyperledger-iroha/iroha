---- MODULE SumeragiValidationPriorityGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `pending_block_validation_priority_reason(...)`.

The implementation allows proposal-less validation/commit-pipeline progress
only for pending blocks that extend the current tip, and only when near-tip
consensus/availability evidence exists. Evidence has a deliberate priority:
commit QC, cached QC, commit votes, delivered RBC, then READY quorum. This
model proves the tip guard, reason selection, and proposal-less bypass
contract over representative pending-block states.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  reason,
  \* @type: Bool;
  can_bypass_proposal

\* @type: <<Str, Str, Bool>>;
vars == <<candidate, reason, can_bypass_proposal>>

Cases == {
  "off_tip_height_with_commit_qc",
  "off_tip_parent_with_commit_votes",
  "no_evidence",
  "observed_commit_qc",
  "cached_commit_qc",
  "cached_prepare_qc",
  "commit_votes",
  "rbc_deliver",
  "rbc_ready",
  "all_evidence",
  "qc_and_votes",
  "votes_and_rbc",
  "deliver_and_ready"
}

Reasons == {
  "none",
  "commit_qc",
  "cached_qc",
  "commit_votes",
  "rbc_deliver",
  "rbc_ready_quorum"
}

HeightOk(c) ==
  c # "off_tip_height_with_commit_qc"

ParentOk(c) ==
  c # "off_tip_parent_with_commit_votes"

ExtendsTip(c) ==
  HeightOk(c) /\ ParentOk(c)

ObservedCommitQc(c) ==
  c \in {"off_tip_height_with_commit_qc", "observed_commit_qc", "all_evidence"}

CachedCommitQc(c) ==
  c \in {"cached_commit_qc", "all_evidence"}

CachedQc(c) ==
  c \in {"cached_commit_qc", "cached_prepare_qc", "all_evidence", "qc_and_votes"}

CommitVotes(c) ==
  c \in {
    "off_tip_parent_with_commit_votes",
    "commit_votes",
    "all_evidence",
    "qc_and_votes",
    "votes_and_rbc"
  }

RbcDeliver(c) ==
  c \in {"rbc_deliver", "all_evidence", "votes_and_rbc", "deliver_and_ready"}

RbcReadyQuorum(c) ==
  c \in {"rbc_ready", "all_evidence", "deliver_and_ready"}

SpecReason(c) ==
  IF ~ExtendsTip(c)
  THEN "none"
  ELSE IF ObservedCommitQc(c) \/ CachedCommitQc(c)
       THEN "commit_qc"
  ELSE IF CachedQc(c)
       THEN "cached_qc"
  ELSE IF CommitVotes(c)
       THEN "commit_votes"
  ELSE IF RbcDeliver(c)
       THEN "rbc_deliver"
  ELSE IF RbcReadyQuorum(c)
       THEN "rbc_ready_quorum"
  ELSE "none"

ActualExtendsTip(c) ==
  CASE Bug = "skip_tip_gate" -> TRUE
    [] Bug = "use_height_only_tip_gate" -> HeightOk(c)
    [] Bug = "use_parent_only_tip_gate" -> ParentOk(c)
    [] OTHER -> ExtendsTip(c)

ActualObservedCommitQc(c) ==
  ObservedCommitQc(c) /\ Bug # "ignore_observed_commit_qc"

ActualCachedCommitQc(c) ==
  CachedCommitQc(c) /\ Bug # "ignore_cached_commit_qc"

ActualCachedQc(c) ==
  CachedQc(c) /\ Bug # "ignore_cached_qc"

ActualCommitVotes(c) ==
  CommitVotes(c) /\ Bug # "ignore_commit_votes"

ActualRbcDeliver(c) ==
  RbcDeliver(c) /\ Bug # "ignore_rbc_deliver"

ActualRbcReadyQuorum(c) ==
  RbcReadyQuorum(c) /\ Bug # "ignore_rbc_ready"

ActualReasonAfterTip(c) ==
  IF Bug = "cached_qc_before_commit_qc" /\ ActualCachedQc(c)
  THEN "cached_qc"
  ELSE IF Bug = "commit_votes_before_cached_qc" /\ ActualCommitVotes(c)
  THEN "commit_votes"
  ELSE IF Bug = "rbc_before_commit_votes" /\ ActualRbcDeliver(c)
  THEN "rbc_deliver"
  ELSE IF Bug = "ready_before_deliver" /\ ActualRbcReadyQuorum(c)
  THEN "rbc_ready_quorum"
  ELSE IF ActualObservedCommitQc(c) \/ ActualCachedCommitQc(c)
       THEN "commit_qc"
  ELSE IF ActualCachedQc(c)
       THEN "cached_qc"
  ELSE IF ActualCommitVotes(c)
       THEN "commit_votes"
  ELSE IF ActualRbcDeliver(c)
       THEN "rbc_deliver"
  ELSE IF ActualRbcReadyQuorum(c)
       THEN "rbc_ready_quorum"
  ELSE "none"

ActualReason(c) ==
  IF ActualExtendsTip(c) THEN ActualReasonAfterTip(c) ELSE "none"

SpecCanBypassProposal(c) ==
  SpecReason(c) # "none"

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_tip_gate",
       "use_height_only_tip_gate",
       "use_parent_only_tip_gate",
       "ignore_observed_commit_qc",
       "ignore_cached_commit_qc",
       "ignore_cached_qc",
       "ignore_commit_votes",
       "ignore_rbc_deliver",
       "ignore_rbc_ready",
       "cached_qc_before_commit_qc",
       "commit_votes_before_cached_qc",
       "rbc_before_commit_votes",
       "ready_before_deliver"
     }
  /\ candidate \in Cases
  /\ reason \in Reasons
  /\ can_bypass_proposal \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ reason = ActualReason(candidate)
  /\ can_bypass_proposal = (reason # "none")

Next ==
  UNCHANGED vars

ReasonMatchesSpec ==
  reason = SpecReason(candidate)

BypassMatchesSpec ==
  can_bypass_proposal = SpecCanBypassProposal(candidate)

TipGuardBlocksOffTipEvidence ==
  ~ExtendsTip(candidate) => reason = "none"

EvidenceRequiredForBypass ==
  can_bypass_proposal =>
    \/ ObservedCommitQc(candidate)
    \/ CachedCommitQc(candidate)
    \/ CachedQc(candidate)
    \/ CommitVotes(candidate)
    \/ RbcDeliver(candidate)
    \/ RbcReadyQuorum(candidate)

NoEvidenceDoesNotBypass ==
  candidate = "no_evidence" => reason = "none"

CommitQcDominates ==
  candidate \in {"observed_commit_qc", "cached_commit_qc", "all_evidence"} =>
    reason = "commit_qc"

CachedQcDominatesVotes ==
  candidate = "qc_and_votes" => reason = "cached_qc"

CommitVotesDominateRbc ==
  candidate = "votes_and_rbc" => reason = "commit_votes"

DeliveredRbcDominatesReadyQuorum ==
  candidate = "deliver_and_ready" => reason = "rbc_deliver"

RbcReadyCanBypassWhenAlone ==
  candidate = "rbc_ready" => reason = "rbc_ready_quorum"

Safety ==
  /\ ReasonMatchesSpec
  /\ BypassMatchesSpec
  /\ TipGuardBlocksOffTipEvidence
  /\ EvidenceRequiredForBypass
  /\ NoEvidenceDoesNotBypass
  /\ CommitQcDominates
  /\ CachedQcDominatesVotes
  /\ CommitVotesDominateRbc
  /\ DeliveredRbcDominatesReadyQuorum
  /\ RbcReadyCanBypassWhenAlone

====

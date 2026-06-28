---- MODULE SumeragiFastFinalityInlineValidationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `fast_finality_inline_validation_tx_count(...)`.

The helper returns a transaction count only for small, proposal-backed,
near-tip blocks that are safe to validate inline. It must stay disabled for
DA-off mode, consensus-priority evidence, non-next-height blocks, missing
proposal evidence, active inflight validation, mismatched local payload bytes,
zero caps, and blocks whose transaction count exceeds the configured cap.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  result

\* @type: <<Str, Str>>;
vars == <<candidate, result>>

Cases == {
  "eligible_empty",
  "eligible_one",
  "eligible_exact_cap",
  "da_disabled",
  "priority_reason",
  "wrong_height",
  "no_proposal_evidence",
  "validation_inflight",
  "payload_mismatch",
  "cap_zero",
  "over_cap"
}

ResultValues == {"none", "0", "1", "2"}

DaEnabled(c) ==
  c # "da_disabled"

PriorityReasonAbsent(c) ==
  c # "priority_reason"

NextHeight(c) ==
  c # "wrong_height"

ProposalEvidence(c) ==
  c # "no_proposal_evidence"

NoInflight(c) ==
  c # "validation_inflight"

PayloadMatches(c) ==
  c # "payload_mismatch"

TxCap(c) ==
  CASE c = "cap_zero" -> 0
    [] c = "over_cap" -> 1
    [] OTHER -> 2

TxCount(c) ==
  CASE c = "eligible_empty" -> 0
    [] c = "cap_zero" -> 0
    [] c = "eligible_one" -> 1
    [] OTHER -> 2

CountResult(count) ==
  CASE count = 0 -> "0"
    [] count = 1 -> "1"
    [] OTHER -> "2"

SpecEligible(c) ==
  /\ DaEnabled(c)
  /\ PriorityReasonAbsent(c)
  /\ NextHeight(c)
  /\ ProposalEvidence(c)
  /\ NoInflight(c)
  /\ PayloadMatches(c)
  /\ TxCap(c) > 0
  /\ TxCount(c) <= TxCap(c)

SpecResult(c) ==
  IF SpecEligible(c) THEN CountResult(TxCount(c)) ELSE "none"

ActualDaEnabled(c) ==
  IF Bug = "skip_da_gate" THEN TRUE ELSE DaEnabled(c)

ActualPriorityAbsent(c) ==
  IF Bug = "ignore_priority_reason" THEN TRUE ELSE PriorityReasonAbsent(c)

ActualNextHeight(c) ==
  IF Bug = "use_any_height" THEN TRUE ELSE NextHeight(c)

ActualProposalEvidence(c) ==
  IF Bug = "ignore_proposal_evidence" THEN TRUE ELSE ProposalEvidence(c)

ActualNoInflight(c) ==
  IF Bug = "ignore_inflight" THEN TRUE ELSE NoInflight(c)

ActualPayloadMatches(c) ==
  IF Bug = "ignore_payload_mismatch" THEN TRUE ELSE PayloadMatches(c)

ActualCapPositive(c) ==
  IF Bug = "cap_zero_allows" THEN TRUE ELSE TxCap(c) > 0

ActualWithinCap(c) ==
  CASE Bug = "allow_over_cap" -> TRUE
    [] Bug = "reject_exact_cap" -> TxCount(c) < TxCap(c)
    [] OTHER -> TxCount(c) <= TxCap(c)

ActualCountResult(c) ==
  IF Bug = "return_cap_instead_tx_count"
  THEN CountResult(TxCap(c))
  ELSE CountResult(TxCount(c))

ActualEligible(c) ==
  /\ ActualDaEnabled(c)
  /\ ActualPriorityAbsent(c)
  /\ ActualNextHeight(c)
  /\ ActualProposalEvidence(c)
  /\ ActualNoInflight(c)
  /\ ActualPayloadMatches(c)
  /\ ActualCapPositive(c)
  /\ ActualWithinCap(c)

ActualResult(c) ==
  IF ActualEligible(c) THEN ActualCountResult(c) ELSE "none"

TypeInvariant ==
  /\ Bug \in {
       "none",
       "skip_da_gate",
       "ignore_priority_reason",
       "use_any_height",
       "ignore_proposal_evidence",
       "ignore_inflight",
       "ignore_payload_mismatch",
       "cap_zero_allows",
       "allow_over_cap",
       "reject_exact_cap",
       "return_cap_instead_tx_count"
     }
  /\ candidate \in Cases
  /\ result \in ResultValues

Init ==
  /\ candidate \in Cases
  /\ result = ActualResult(candidate)

Next ==
  UNCHANGED vars

ResultMatchesSpec ==
  result = SpecResult(candidate)

InlineRequiresDa ==
  result # "none" => DaEnabled(candidate)

PriorityReasonDisablesInline ==
  ~PriorityReasonAbsent(candidate) => result = "none"

InlineRequiresNextHeight ==
  result # "none" => NextHeight(candidate)

InlineRequiresProposalEvidence ==
  result # "none" => ProposalEvidence(candidate)

InflightBlocksInline ==
  ~NoInflight(candidate) => result = "none"

InlineRequiresMatchingPayload ==
  result # "none" => PayloadMatches(candidate)

ZeroCapDisablesInline ==
  TxCap(candidate) = 0 => result = "none"

InlineRequiresTxCountWithinCap ==
  result # "none" => TxCount(candidate) <= TxCap(candidate)

ExactCapAllowed ==
  candidate = "eligible_exact_cap" => result = "2"

ReturnedCountMatchesTxCount ==
  SpecEligible(candidate) => result = CountResult(TxCount(candidate))

AllCandidateResultsMatchSpec ==
  \A c \in Cases:
    ActualResult(c) = SpecResult(c)

EligibleCaseResultAnchors ==
  /\ ActualResult("eligible_empty") = "0"
  /\ ActualResult("eligible_one") = "1"
  /\ ActualResult("eligible_exact_cap") = "2"

GateRejectionAnchors ==
  /\ ActualResult("da_disabled") = "none"
  /\ ActualResult("priority_reason") = "none"
  /\ ActualResult("wrong_height") = "none"
  /\ ActualResult("no_proposal_evidence") = "none"
  /\ ActualResult("validation_inflight") = "none"
  /\ ActualResult("payload_mismatch") = "none"

CapBoundaryAnchors ==
  /\ ActualResult("cap_zero") = "none"
  /\ ActualResult("over_cap") = "none"
  /\ ActualResult("eligible_exact_cap") = CountResult(TxCap("eligible_exact_cap"))

AcceptedCandidatesSatisfyAllGuards ==
  \A c \in Cases:
    ActualResult(c) # "none" =>
      /\ DaEnabled(c)
      /\ PriorityReasonAbsent(c)
      /\ NextHeight(c)
      /\ ProposalEvidence(c)
      /\ NoInflight(c)
      /\ PayloadMatches(c)
      /\ TxCap(c) > 0
      /\ TxCount(c) <= TxCap(c)

ReturnedCountsMatchEligibleTxCounts ==
  \A c \in Cases:
    SpecEligible(c) => ActualResult(c) = CountResult(TxCount(c))

Safety ==
  /\ ResultMatchesSpec
  /\ InlineRequiresDa
  /\ PriorityReasonDisablesInline
  /\ InlineRequiresNextHeight
  /\ InlineRequiresProposalEvidence
  /\ InflightBlocksInline
  /\ InlineRequiresMatchingPayload
  /\ ZeroCapDisablesInline
  /\ InlineRequiresTxCountWithinCap
  /\ ExactCapAllowed
  /\ ReturnedCountMatchesTxCount
  /\ AllCandidateResultsMatchSpec
  /\ EligibleCaseResultAnchors
  /\ GateRejectionAnchors
  /\ CapBoundaryAnchors
  /\ AcceptedCandidatesSatisfyAllGuards
  /\ ReturnedCountsMatchEligibleTxCounts

FastFinalityInlineValidationExactness ==
  /\ ResultMatchesSpec
  /\ InlineRequiresDa
  /\ PriorityReasonDisablesInline
  /\ InlineRequiresNextHeight
  /\ InlineRequiresProposalEvidence
  /\ InflightBlocksInline
  /\ InlineRequiresMatchingPayload
  /\ ZeroCapDisablesInline
  /\ InlineRequiresTxCountWithinCap
  /\ ExactCapAllowed
  /\ ReturnedCountMatchesTxCount
  /\ AllCandidateResultsMatchSpec
  /\ EligibleCaseResultAnchors
  /\ GateRejectionAnchors
  /\ CapBoundaryAnchors
  /\ AcceptedCandidatesSatisfyAllGuards
  /\ ReturnedCountsMatchEligibleTxCounts

FastFinalityInlineValidationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FastFinalityInlineValidationExactness

====

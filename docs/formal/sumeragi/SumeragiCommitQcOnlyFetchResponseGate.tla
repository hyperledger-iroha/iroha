---- MODULE SumeragiCommitQcOnlyFetchResponseGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `dispatch_commit_qc_only_fetch_response(...)`.

The helper responds to a commit-QC-only missing-block fetch. It must either
send direct commit-QC evidence, send a signed-quorum fallback update when a
direct QC is unavailable but the committed block carries enough signer
evidence, or defer the response:

- direct commit QC skips vote rebroadcast and signed-quorum fallback,
- certified proof companions are attempted only when a certified response can
  be built and before the direct commit-QC companion,
- without direct QC, cached commit votes are rebroadcast to recovery targets
  plus the requester, with the requester deduplicated,
- signed-quorum fallback responses force the bypass/highest-QC/hintless
  block-sync bypass flags and preserve the requester roster-proof flag,
- without direct QC and without signed-quorum fallback, the helper returns
  false after the vote-rebroadcast attempt.
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
  "direct_qc_with_certified_response",
  "direct_qc_without_certified_response",
  "no_qc_no_votes_no_fallback",
  "no_qc_votes_no_fallback",
  "no_qc_no_votes_fallback_unknown",
  "no_qc_votes_fallback_known",
  "no_qc_peer_already_target_fallback",
  "no_qc_peer_absent_target_no_fallback"
}

DirectQc(c) ==
  c \in {"direct_qc_with_certified_response", "direct_qc_without_certified_response"}

CertifiedResponse(c) ==
  c = "direct_qc_with_certified_response"

FallbackAvailable(c) ==
  c \in {
    "no_qc_no_votes_fallback_unknown",
    "no_qc_votes_fallback_known",
    "no_qc_peer_already_target_fallback"
  }

RequesterRosterProofKnown(c) ==
  c \in {"no_qc_votes_fallback_known", "no_qc_peer_already_target_fallback"}

SpecCertifiedProofAttempt(c) ==
  DirectQc(c) /\ CertifiedResponse(c)

SpecDirectQcCompanion(c) ==
  DirectQc(c)

SpecRebroadcastAttempt(c) ==
  ~DirectQc(c)

SpecRebroadcastPhase(c) ==
  IF SpecRebroadcastAttempt(c) THEN "commit" ELSE "none"

SpecRequesterTargetCount(c) ==
  IF SpecRebroadcastAttempt(c) THEN 1 ELSE 0

SpecFallbackSent(c) ==
  ~DirectQc(c) /\ FallbackAvailable(c)

SpecFallbackForceBypass(c) ==
  SpecFallbackSent(c)

SpecFallbackAllowHighestQcBypass(c) ==
  SpecFallbackSent(c)

SpecFallbackAllowHintlessBypass(c) ==
  SpecFallbackSent(c)

SpecFallbackRosterProofKnown(c) ==
  SpecFallbackSent(c) /\ RequesterRosterProofKnown(c)

SpecReturn(c) ==
  DirectQc(c) \/ SpecFallbackSent(c)

SpecPosProof(c) ==
  IF SpecCertifiedProofAttempt(c) THEN 1 ELSE 0

SpecPosDirectQc(c) ==
  IF SpecDirectQcCompanion(c)
  THEN 1 + (IF SpecCertifiedProofAttempt(c) THEN 1 ELSE 0)
  ELSE 0

ActualCertifiedProofAttempt(c) ==
  CASE Bug = "skip_proof_with_response"
       /\ c = "direct_qc_with_certified_response" -> FALSE
    [] Bug = "proof_without_response"
       /\ c = "direct_qc_without_certified_response" -> TRUE
    [] OTHER -> SpecCertifiedProofAttempt(c)

ActualDirectQcCompanion(c) ==
  CASE Bug = "drop_direct_qc_companion"
       /\ c = "direct_qc_without_certified_response" -> FALSE
    [] Bug = "direct_qc_without_qc"
       /\ c = "no_qc_no_votes_no_fallback" -> TRUE
    [] OTHER -> SpecDirectQcCompanion(c)

ActualRebroadcastAttempt(c) ==
  CASE Bug = "rebroadcast_after_direct_qc"
       /\ c = "direct_qc_without_certified_response" -> TRUE
    [] Bug = "skip_rebroadcast_without_qc"
       /\ c = "no_qc_no_votes_no_fallback" -> FALSE
    [] OTHER -> SpecRebroadcastAttempt(c)

ActualRebroadcastPhase(c) ==
  CASE Bug = "rebroadcast_prepare_phase"
       /\ c = "no_qc_votes_no_fallback" -> "prepare"
    [] ActualRebroadcastAttempt(c) -> "commit"
    [] OTHER -> "none"

ActualRequesterTargetCount(c) ==
  CASE Bug = "omit_requester_target"
       /\ c = "no_qc_peer_absent_target_no_fallback" -> 0
    [] Bug = "duplicate_requester_target"
       /\ c = "no_qc_peer_already_target_fallback" -> 2
    [] ActualRebroadcastAttempt(c) -> 1
    [] OTHER -> 0

ActualFallbackSent(c) ==
  CASE Bug = "fallback_after_direct_qc"
       /\ c = "direct_qc_without_certified_response" -> TRUE
    [] Bug = "skip_fallback_available"
       /\ c = "no_qc_votes_fallback_known" -> FALSE
    [] OTHER -> SpecFallbackSent(c)

ActualFallbackForceBypass(c) ==
  CASE Bug = "fallback_without_force_bypass"
       /\ c = "no_qc_no_votes_fallback_unknown" -> FALSE
    [] OTHER -> SpecFallbackForceBypass(c)

ActualFallbackAllowHighestQcBypass(c) ==
  CASE Bug = "fallback_without_force_bypass"
       /\ c = "no_qc_no_votes_fallback_unknown" -> FALSE
    [] OTHER -> SpecFallbackAllowHighestQcBypass(c)

ActualFallbackAllowHintlessBypass(c) ==
  CASE Bug = "fallback_without_force_bypass"
       /\ c = "no_qc_no_votes_fallback_unknown" -> FALSE
    [] OTHER -> SpecFallbackAllowHintlessBypass(c)

ActualFallbackRosterProofKnown(c) ==
  CASE Bug = "fallback_drops_requester_roster_proof_known"
       /\ c = "no_qc_votes_fallback_known" -> FALSE
    [] OTHER -> SpecFallbackRosterProofKnown(c)

ActualReturn(c) ==
  CASE Bug = "direct_path_returns_false"
       /\ c = "direct_qc_without_certified_response" -> FALSE
    [] Bug = "return_true_without_qc_or_fallback"
       /\ c = "no_qc_votes_no_fallback" -> TRUE
    [] Bug = "skip_fallback_available"
       /\ c = "no_qc_votes_fallback_known" -> FALSE
    [] OTHER -> ActualDirectQcCompanion(c) \/ ActualFallbackSent(c)

ActualPosProof(c) ==
  IF ~ActualCertifiedProofAttempt(c)
  THEN 0
  ELSE CASE Bug = "proof_after_direct_qc"
            /\ c = "direct_qc_with_certified_response" -> SpecPosDirectQc(c) + 1
         [] OTHER -> SpecPosProof(c)

ActualPosDirectQc(c) ==
  IF ActualDirectQcCompanion(c) THEN SpecPosDirectQc(c) ELSE 0

Matches(c) ==
  /\ ActualCertifiedProofAttempt(c) = SpecCertifiedProofAttempt(c)
  /\ ActualDirectQcCompanion(c) = SpecDirectQcCompanion(c)
  /\ ActualRebroadcastAttempt(c) = SpecRebroadcastAttempt(c)
  /\ ActualRebroadcastPhase(c) = SpecRebroadcastPhase(c)
  /\ ActualRequesterTargetCount(c) = SpecRequesterTargetCount(c)
  /\ ActualFallbackSent(c) = SpecFallbackSent(c)
  /\ ActualFallbackForceBypass(c) = SpecFallbackForceBypass(c)
  /\ ActualFallbackAllowHighestQcBypass(c) = SpecFallbackAllowHighestQcBypass(c)
  /\ ActualFallbackAllowHintlessBypass(c) = SpecFallbackAllowHintlessBypass(c)
  /\ ActualFallbackRosterProofKnown(c) = SpecFallbackRosterProofKnown(c)
  /\ ActualReturn(c) = SpecReturn(c)
  /\ ActualPosProof(c) = SpecPosProof(c)
  /\ ActualPosDirectQc(c) = SpecPosDirectQc(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_direct_qc_companion",
       "direct_path_returns_false",
       "skip_proof_with_response",
       "proof_without_response",
       "proof_after_direct_qc",
       "rebroadcast_after_direct_qc",
       "fallback_after_direct_qc",
       "skip_rebroadcast_without_qc",
       "rebroadcast_prepare_phase",
       "omit_requester_target",
       "duplicate_requester_target",
       "return_true_without_qc_or_fallback",
       "skip_fallback_available",
       "fallback_without_force_bypass",
       "fallback_drops_requester_roster_proof_known",
       "direct_qc_without_qc"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

DirectQcCompanionSent ==
  Matches("direct_qc_without_certified_response")

DirectPathReturnsTrue ==
  Matches("direct_qc_without_certified_response")

CertifiedProofAttempted ==
  Matches("direct_qc_with_certified_response")

NoProofWithoutCertifiedResponse ==
  Matches("direct_qc_without_certified_response")

ProofBeforeDirectQc ==
  Matches("direct_qc_with_certified_response")

NoRebroadcastAfterDirectQc ==
  Matches("direct_qc_without_certified_response")

NoFallbackAfterDirectQc ==
  Matches("direct_qc_without_certified_response")

NoQcRebroadcasts ==
  Matches("no_qc_no_votes_no_fallback")

RebroadcastCommitPhase ==
  Matches("no_qc_votes_no_fallback")

RequesterTargetIncluded ==
  Matches("no_qc_peer_absent_target_no_fallback")

RequesterTargetDeduped ==
  Matches("no_qc_peer_already_target_fallback")

NoQcNoFallbackReturnsFalse ==
  Matches("no_qc_votes_no_fallback")

FallbackAvailableSent ==
  Matches("no_qc_votes_fallback_known")

FallbackForceBypass ==
  Matches("no_qc_no_votes_fallback_unknown")

FallbackPreservesRosterProofKnown ==
  Matches("no_qc_votes_fallback_known")

NoDirectQcWithoutQc ==
  Matches("no_qc_no_votes_no_fallback")

====

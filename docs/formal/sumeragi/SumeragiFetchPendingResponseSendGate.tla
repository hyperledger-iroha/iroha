---- MODULE SumeragiFetchPendingResponseSendGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `send_fetch_pending_block_response(...)`.

The helper sends a single missing-block fetch response while preserving the
recovery evidence ordering expected by downstream peers:

- hintless `BlockSyncUpdate` payloads are allowed only when both the caller
  permits hintless bypass and the requester already proved roster context;
  otherwise the response is downgraded to `BlockCreated`,
- bypass selection is computed from the force flag, consensus priority,
  highest-QC targeting, eager body/RBC payload kinds, and accepted hintless
  block-sync responses,
- non-downgraded `BlockSyncUpdate` responses apply cached QC sidecars before
  direct commit-QC extraction and frame-cap trimming,
- direct commit-QC companions are sent before the final payload or before
  returning when the payload has to be dropped,
- when a `BlockSyncUpdate` cannot be trimmed to fit, a fitting `BlockCreated`
  fallback is sent with the originally computed bypass decision; if even that
  fallback is oversized, only the direct commit-QC companion may be sent.
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
  "background_update",
  "force_update",
  "consensus_update",
  "highest_update_allowed",
  "highest_update_disallowed",
  "hintless_allowed",
  "hintless_no_roster",
  "hintless_no_allow",
  "update_trim_fits_qc",
  "update_trim_fails_fallback_fits_qc",
  "update_trim_fails_fallback_oversized_qc",
  "created_with_qc",
  "created_no_qc",
  "rbc_ready_payload"
}

InitialKind(c) ==
  CASE c \in {"created_with_qc", "created_no_qc"} -> "created"
    [] c = "rbc_ready_payload" -> "rbc_ready"
    [] OTHER -> "update"

ForceBypass(c) ==
  c = "force_update"

ConsensusPriority(c) ==
  c = "consensus_update"

AllowHighest(c) ==
  c = "highest_update_allowed"

TargetsHighest(c) ==
  c \in {"highest_update_allowed", "highest_update_disallowed"}

Hintless(c) ==
  c \in {"hintless_allowed", "hintless_no_roster", "hintless_no_allow"}

AllowHintless(c) ==
  c \in {"hintless_allowed", "hintless_no_roster"}

RequesterRosterProofKnown(c) ==
  c = "hintless_allowed"

SpecHintlessAllowed(c) ==
  Hintless(c) /\ AllowHintless(c) /\ RequesterRosterProofKnown(c)

SpecDowngradeHintless(c) ==
  Hintless(c) /\ ~SpecHintlessAllowed(c)

SpecMessageAfterHintlessGate(c) ==
  IF SpecDowngradeHintless(c) THEN "created" ELSE InitialKind(c)

SpecApplyCachedQc(c) ==
  SpecMessageAfterHintlessGate(c) = "update"

SpecTrimAttempt(c) ==
  SpecMessageAfterHintlessGate(c) = "update"

TrimFits(c) ==
  c \notin {
    "update_trim_fails_fallback_fits_qc",
    "update_trim_fails_fallback_oversized_qc"
  }

FallbackFits(c) ==
  c = "update_trim_fails_fallback_fits_qc"

DirectQcAvailable(c) ==
  c \in {
    "update_trim_fits_qc",
    "update_trim_fails_fallback_fits_qc",
    "update_trim_fails_fallback_oversized_qc",
    "created_with_qc"
  }

SpecBypass(c) ==
  \/ ForceBypass(c)
  \/ ConsensusPriority(c)
  \/ /\ AllowHighest(c)
     /\ TargetsHighest(c)
  \/ SpecMessageAfterHintlessGate(c) \in {"created", "rbc_ready"}
  \/ /\ AllowHintless(c)
     /\ SpecHintlessAllowed(c)

SpecFinalPayload(c) ==
  CASE /\ SpecMessageAfterHintlessGate(c) = "update"
       /\ ~TrimFits(c)
       /\ FallbackFits(c) -> "created"
    [] /\ SpecMessageAfterHintlessGate(c) = "update"
       /\ ~TrimFits(c)
       /\ ~FallbackFits(c) -> "none"
    [] OTHER -> SpecMessageAfterHintlessGate(c)

SpecPayloadSent(c) ==
  SpecFinalPayload(c) # "none"

SpecDirectQcCompanion(c) ==
  DirectQcAvailable(c)

SpecCompanionBeforePayload(c) ==
  SpecDirectQcCompanion(c) /\ SpecPayloadSent(c)

SpecBypassUsedForPayload(c) ==
  IF SpecPayloadSent(c) THEN SpecBypass(c) ELSE FALSE

ActualHintlessAllowed(c) ==
  CASE Bug = "allow_hintless_without_roster"
       /\ c = "hintless_no_roster" -> TRUE
    [] Bug = "reject_valid_hintless"
       /\ c = "hintless_allowed" -> FALSE
    [] OTHER -> SpecHintlessAllowed(c)

ActualDowngradeHintless(c) ==
  CASE Bug = "skip_hintless_downgrade"
       /\ c = "hintless_no_roster" -> FALSE
    [] Bug = "downgrade_valid_hintless"
       /\ c = "hintless_allowed" -> TRUE
    [] OTHER -> Hintless(c) /\ ~ActualHintlessAllowed(c)

ActualMessageAfterHintlessGate(c) ==
  IF ActualDowngradeHintless(c) THEN "created" ELSE InitialKind(c)

ActualApplyCachedQc(c) ==
  CASE Bug = "skip_cached_qc_apply"
       /\ c = "update_trim_fits_qc" -> FALSE
    [] Bug = "apply_cached_qc_after_downgrade"
       /\ c = "hintless_no_allow" -> TRUE
    [] OTHER -> ActualMessageAfterHintlessGate(c) = "update"

ActualTrimAttempt(c) ==
  CASE Bug = "skip_update_trim"
       /\ c = "update_trim_fails_fallback_fits_qc" -> FALSE
    [] Bug = "trim_after_hintless_downgrade"
       /\ c = "hintless_no_roster" -> TRUE
    [] OTHER -> ActualMessageAfterHintlessGate(c) = "update"

ActualBypass(c) ==
  CASE Bug = "force_not_bypassed"
       /\ c = "force_update" -> FALSE
    [] Bug = "consensus_not_bypassed"
       /\ c = "consensus_update" -> FALSE
    [] Bug = "highest_ignored"
       /\ c = "highest_update_allowed" -> FALSE
    [] Bug = "highest_without_allow"
       /\ c = "highest_update_disallowed" -> TRUE
    [] Bug = "created_not_bypassed"
       /\ c = "created_no_qc" -> FALSE
    [] Bug = "hintless_not_bypassed"
       /\ c = "hintless_allowed" -> FALSE
    [] Bug = "downgrade_bypass_lost"
       /\ c = "hintless_no_roster" -> FALSE
    [] Bug = "recompute_bypass_after_fallback"
       /\ c = "update_trim_fails_fallback_fits_qc" -> TRUE
    [] OTHER ->
       \/ ForceBypass(c)
       \/ ConsensusPriority(c)
       \/ /\ AllowHighest(c)
          /\ TargetsHighest(c)
       \/ ActualMessageAfterHintlessGate(c) \in {"created", "rbc_ready"}
       \/ /\ AllowHintless(c)
          /\ ActualHintlessAllowed(c)

ActualFinalPayload(c) ==
  CASE Bug = "skip_fallback_payload"
       /\ c = "update_trim_fails_fallback_fits_qc" -> "none"
    [] Bug = "fallback_sends_update"
       /\ c = "update_trim_fails_fallback_fits_qc" -> "update"
    [] Bug = "dispatch_oversized_payload"
       /\ c = "update_trim_fails_fallback_oversized_qc" -> "created"
    [] /\ ActualMessageAfterHintlessGate(c) = "update"
       /\ ~TrimFits(c)
       /\ FallbackFits(c) -> "created"
    [] /\ ActualMessageAfterHintlessGate(c) = "update"
       /\ ~TrimFits(c)
       /\ ~FallbackFits(c) -> "none"
    [] OTHER -> ActualMessageAfterHintlessGate(c)

ActualPayloadSent(c) ==
  ActualFinalPayload(c) # "none"

ActualDirectQcCompanion(c) ==
  CASE Bug = "skip_direct_qc_update"
       /\ c = "update_trim_fits_qc" -> FALSE
    [] Bug = "skip_direct_qc_on_fallback"
       /\ c = "update_trim_fails_fallback_fits_qc" -> FALSE
    [] Bug = "skip_direct_qc_on_oversized"
       /\ c = "update_trim_fails_fallback_oversized_qc" -> FALSE
    [] Bug = "skip_direct_qc_created"
       /\ c = "created_with_qc" -> FALSE
    [] Bug = "companion_without_qc"
       /\ c = "created_no_qc" -> TRUE
    [] OTHER -> DirectQcAvailable(c)

ActualCompanionBeforePayload(c) ==
  CASE Bug = "direct_qc_after_payload"
       /\ c = "update_trim_fits_qc" -> FALSE
    [] Bug = "fallback_qc_after_payload"
       /\ c = "update_trim_fails_fallback_fits_qc" -> FALSE
    [] OTHER -> ActualDirectQcCompanion(c) /\ ActualPayloadSent(c)

ActualBypassUsedForPayload(c) ==
  IF ActualPayloadSent(c) THEN ActualBypass(c) ELSE FALSE

Matches(c) ==
  /\ ActualHintlessAllowed(c) = SpecHintlessAllowed(c)
  /\ ActualDowngradeHintless(c) = SpecDowngradeHintless(c)
  /\ ActualMessageAfterHintlessGate(c) = SpecMessageAfterHintlessGate(c)
  /\ ActualApplyCachedQc(c) = SpecApplyCachedQc(c)
  /\ ActualTrimAttempt(c) = SpecTrimAttempt(c)
  /\ ActualBypass(c) = SpecBypass(c)
  /\ ActualFinalPayload(c) = SpecFinalPayload(c)
  /\ ActualPayloadSent(c) = SpecPayloadSent(c)
  /\ ActualDirectQcCompanion(c) = SpecDirectQcCompanion(c)
  /\ ActualCompanionBeforePayload(c) = SpecCompanionBeforePayload(c)
  /\ ActualBypassUsedForPayload(c) = SpecBypassUsedForPayload(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "allow_hintless_without_roster",
       "reject_valid_hintless",
       "skip_hintless_downgrade",
       "downgrade_valid_hintless",
       "skip_cached_qc_apply",
       "apply_cached_qc_after_downgrade",
       "skip_update_trim",
       "trim_after_hintless_downgrade",
       "force_not_bypassed",
       "consensus_not_bypassed",
       "highest_ignored",
       "highest_without_allow",
       "created_not_bypassed",
       "hintless_not_bypassed",
       "downgrade_bypass_lost",
       "recompute_bypass_after_fallback",
       "skip_fallback_payload",
       "fallback_sends_update",
       "dispatch_oversized_payload",
       "skip_direct_qc_update",
       "skip_direct_qc_on_fallback",
       "skip_direct_qc_on_oversized",
       "skip_direct_qc_created",
       "companion_without_qc",
       "direct_qc_after_payload",
       "fallback_qc_after_payload"
     }
  /\ checked = 0

FetchPendingResponseSendMatchesSpec ==
  \A c \in Cases: Matches(c)

FetchPendingResponseSendExactness ==
  /\ FetchPendingResponseSendMatchesSpec

FetchPendingResponseSendCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FetchPendingResponseSendExactness

SafetyFast ==
  FetchPendingResponseSendExactness

HintlessRequiresRosterProof ==
  Matches("hintless_no_roster")

ValidHintlessAllowed ==
  Matches("hintless_allowed")

HintlessDowngradedWithoutAllow ==
  Matches("hintless_no_allow")

CachedQcAppliedBeforeUpdateSend ==
  Matches("update_trim_fits_qc")

NoCachedQcAfterDowngrade ==
  Matches("hintless_no_allow")

UpdateTrimAttempted ==
  Matches("update_trim_fails_fallback_fits_qc")

NoTrimAfterHintlessDowngrade ==
  Matches("hintless_no_roster")

ForceBypassHonored ==
  Matches("force_update")

ConsensusBypassHonored ==
  Matches("consensus_update")

HighestBypassRequiresAllow ==
  Matches("highest_update_disallowed")

HighestBypassHonored ==
  Matches("highest_update_allowed")

CreatedPayloadBypasses ==
  Matches("created_no_qc")

HintlessBypassHonored ==
  Matches("hintless_allowed")

DowngradeCreatedStillBypasses ==
  Matches("hintless_no_roster")

FallbackKeepsOriginalBypass ==
  Matches("update_trim_fails_fallback_fits_qc")

FallbackPayloadSent ==
  Matches("update_trim_fails_fallback_fits_qc")

FallbackSendsCreated ==
  Matches("update_trim_fails_fallback_fits_qc")

OversizedFallbackDropsPayload ==
  Matches("update_trim_fails_fallback_oversized_qc")

DirectQcForUpdateSent ==
  Matches("update_trim_fits_qc")

DirectQcForFallbackSent ==
  Matches("update_trim_fails_fallback_fits_qc")

DirectQcForOversizedSent ==
  Matches("update_trim_fails_fallback_oversized_qc")

DirectQcForCreatedSent ==
  Matches("created_with_qc")

NoCompanionWithoutQc ==
  Matches("created_no_qc")

DirectQcBeforePayload ==
  Matches("update_trim_fits_qc")

FallbackQcBeforePayload ==
  Matches("update_trim_fails_fallback_fits_qc")

====

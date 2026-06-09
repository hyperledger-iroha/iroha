---- MODULE SumeragiBlockBodyResponseDispatchGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `dispatch_block_body_response_with_plain_fallback`.

Exact BlockBodyResponse dispatch sends companion material before the rich body
response when it is useful and safe:

- a direct BlockCreated companion is sent only when its encoded frame fits,
- BlockSyncUpdate bodies get a plain BlockCreated fallback before the rich
  response,
- the original BlockBodyResponse is always sent,
- a direct commit-QC companion, when available, is sent after the body
  response,
- all dispatches use the bypass/background path.
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
  "created_under_no_qc",
  "created_under_qc",
  "created_over_no_qc",
  "created_over_qc",
  "sync_under_no_qc",
  "sync_under_qc",
  "sync_over_no_qc",
  "sync_over_qc"
}

IsSync(c) ==
  c \in {"sync_under_no_qc", "sync_under_qc", "sync_over_no_qc", "sync_over_qc"}

UnderCap(c) ==
  c \in {"created_under_no_qc", "created_under_qc", "sync_under_no_qc", "sync_under_qc"}

DirectQc(c) ==
  c \in {"created_under_qc", "created_over_qc", "sync_under_qc", "sync_over_qc"}

SpecCreatedCompanion(c) ==
  UnderCap(c)

SpecPlainFallback(c) ==
  IsSync(c)

SpecResponse(c) ==
  TRUE

SpecQcCompanion(c) ==
  DirectQc(c)

SpecPosCreated(c) ==
  IF SpecCreatedCompanion(c) THEN 1 ELSE 0

SpecPosPlain(c) ==
  IF SpecPlainFallback(c)
  THEN IF SpecCreatedCompanion(c) THEN 2 ELSE 1
  ELSE 0

SpecPosResponse(c) ==
  1
    + (IF SpecCreatedCompanion(c) THEN 1 ELSE 0)
    + (IF SpecPlainFallback(c) THEN 1 ELSE 0)

SpecPosQc(c) ==
  IF SpecQcCompanion(c) THEN SpecPosResponse(c) + 1 ELSE 0

ActualCreatedCompanion(c) ==
  CASE Bug = "drop_created_companion"
       /\ c = "created_under_no_qc" -> FALSE
    [] Bug = "send_oversized_created_companion"
       /\ c = "created_over_no_qc" -> TRUE
    [] OTHER -> SpecCreatedCompanion(c)

ActualPlainFallback(c) ==
  CASE Bug = "drop_plain_fallback"
       /\ c = "sync_over_no_qc" -> FALSE
    [] Bug = "plain_for_created"
       /\ c = "created_over_no_qc" -> TRUE
    [] OTHER -> SpecPlainFallback(c)

ActualResponse(c) ==
  CASE Bug = "drop_response"
       /\ c = "created_over_no_qc" -> FALSE
    [] OTHER -> SpecResponse(c)

ActualQcCompanion(c) ==
  CASE Bug = "drop_qc_companion"
       /\ c = "created_over_qc" -> FALSE
    [] Bug = "qc_without_direct"
       /\ c = "created_over_no_qc" -> TRUE
    [] OTHER -> SpecQcCompanion(c)

ActualPosCreated(c) ==
  IF ~ActualCreatedCompanion(c)
  THEN 0
  ELSE CASE Bug = "companion_after_response"
            /\ c = "created_under_no_qc" -> SpecPosResponse(c) + 1
         [] OTHER -> SpecPosCreated(c)

ActualPosPlain(c) ==
  IF ~ActualPlainFallback(c)
  THEN 0
  ELSE CASE Bug = "plain_after_response"
            /\ c = "sync_over_no_qc" -> SpecPosResponse(c) + 1
         [] OTHER -> SpecPosPlain(c)

ActualPosResponse(c) ==
  IF ActualResponse(c) THEN SpecPosResponse(c) ELSE 0

ActualPosQc(c) ==
  IF ~ActualQcCompanion(c)
  THEN 0
  ELSE CASE Bug = "qc_before_response"
            /\ c = "sync_under_qc" -> SpecPosResponse(c) - 1
         [] OTHER -> SpecPosQc(c)

ActualAllBypass(c) ==
  CASE Bug = "created_companion_not_bypassed"
       /\ c = "created_under_no_qc" -> FALSE
    [] Bug = "plain_not_bypassed"
       /\ c = "sync_over_no_qc" -> FALSE
    [] Bug = "response_not_bypassed"
       /\ c = "created_over_no_qc" -> FALSE
    [] Bug = "qc_not_bypassed"
       /\ c = "created_over_qc" -> FALSE
    [] OTHER -> TRUE

Matches(c) ==
  /\ ActualCreatedCompanion(c) = SpecCreatedCompanion(c)
  /\ ActualPlainFallback(c) = SpecPlainFallback(c)
  /\ ActualResponse(c) = SpecResponse(c)
  /\ ActualQcCompanion(c) = SpecQcCompanion(c)
  /\ ActualPosCreated(c) = SpecPosCreated(c)
  /\ ActualPosPlain(c) = SpecPosPlain(c)
  /\ ActualPosResponse(c) = SpecPosResponse(c)
  /\ ActualPosQc(c) = SpecPosQc(c)
  /\ ActualAllBypass(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_created_companion",
       "send_oversized_created_companion",
       "drop_plain_fallback",
       "plain_for_created",
       "drop_response",
       "drop_qc_companion",
       "qc_without_direct",
       "plain_after_response",
       "qc_before_response",
       "companion_after_response",
       "created_companion_not_bypassed",
       "plain_not_bypassed",
       "response_not_bypassed",
       "qc_not_bypassed"
     }
  /\ checked = 0

ResponseDispatchMatchesSpec ==
  \A c \in Cases: Matches(c)

SafetyFast == ResponseDispatchMatchesSpec

UnderCapSendsCreatedCompanion ==
  Matches("created_under_no_qc")

OverCapSkipsCreatedCompanion ==
  Matches("created_over_no_qc")

SyncSendsPlainFallback ==
  Matches("sync_over_no_qc")

CreatedSkipsPlainFallback ==
  Matches("created_over_no_qc")

ResponseAlwaysSent ==
  Matches("created_over_no_qc")

DirectQcCompanionSent ==
  Matches("created_over_qc")

NoDirectQcCompanionSkipped ==
  Matches("created_over_no_qc")

PlainBeforeResponse ==
  Matches("sync_over_no_qc")

QcAfterResponse ==
  Matches("sync_under_qc")

CreatedCompanionBeforeResponse ==
  Matches("created_under_no_qc")

CreatedCompanionBypassed ==
  Matches("created_under_no_qc")

PlainFallbackBypassed ==
  Matches("sync_over_no_qc")

ResponseBypassed ==
  Matches("created_over_no_qc")

QcCompanionBypassed ==
  Matches("created_over_qc")

====

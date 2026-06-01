---- MODULE SumeragiKnownBlockQcEnqueueGate ----

(***************************************************************************
A bounded abstract model for `enqueue_known_block_qc_work(...)`.

The live helper computes the `QcVoteKey` from the QC, drops duplicate queued
work without overwriting the existing entry, inserts new known-block QC work
under that key, records deferred aggregate-verification status, emits a queued
debug event, and attempts to wake the main loop only when a wake sender exists.
Wake send failures are deliberately ignored after the work is queued.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  keyPhase,
  \* @type: Bool;
  keyHash,
  \* @type: Bool;
  keyHeight,
  \* @type: Bool;
  keyView,
  \* @type: Bool;
  keyEpoch,
  \* @type: Bool;
  keyChainOrder,
  \* @type: Bool;
  keyRechain,
  \* @type: Bool;
  duplicateCheck,
  \* @type: Bool;
  duplicateFound,
  \* @type: Bool;
  duplicateDebug,
  \* @type: Bool;
  existingPreserved,
  \* @type: Bool;
  insertWork,
  \* @type: Bool;
  preserveWork,
  \* @type: Bool;
  recordStatus,
  \* @type: Bool;
  statusKindQc,
  \* @type: Bool;
  statusOutcomeDeferred,
  \* @type: Bool;
  statusReasonAggregateVerify,
  \* @type: Bool;
  queuedDebug,
  \* @type: Bool;
  queuedLenObserved,
  \* @type: Bool;
  wakeSenderPresent,
  \* @type: Bool;
  wakeAttempted,
  \* @type: Bool;
  wakeResultIgnored

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    keyPhase,
    keyHash,
    keyHeight,
    keyView,
    keyEpoch,
    keyChainOrder,
    keyRechain,
    duplicateCheck,
    duplicateFound,
    duplicateDebug,
    existingPreserved,
    insertWork,
    preserveWork,
    recordStatus,
    statusKindQc,
    statusOutcomeDeferred,
    statusReasonAggregateVerify,
    queuedDebug,
    queuedLenObserved,
    wakeSenderPresent,
    wakeAttempted,
    wakeResultIgnored>>

Cases == {
  "idle",
  "duplicate_existing",
  "new_without_wake",
  "new_with_wake_success",
  "new_with_wake_failure"
}

NewCases == {
  "new_without_wake",
  "new_with_wake_success",
  "new_with_wake_failure"
}

WakeCases == {
  "new_with_wake_success",
  "new_with_wake_failure"
}

SpecKeyPhase(c) == c # "idle"
SpecKeyHash(c) == c # "idle"
SpecKeyHeight(c) == c # "idle"
SpecKeyView(c) == c # "idle"
SpecKeyEpoch(c) == c # "idle"
SpecKeyChainOrder(c) == c # "idle"
SpecKeyRechain(c) == c # "idle"
SpecDuplicateCheck(c) == c # "idle"
SpecDuplicateFound(c) == c = "duplicate_existing"
SpecDuplicateDebug(c) == c = "duplicate_existing"
SpecExistingPreserved(c) == c = "duplicate_existing"
SpecInsertWork(c) == c \in NewCases
SpecPreserveWork(c) == c \in NewCases
SpecRecordStatus(c) == c \in NewCases
SpecStatusKindQc(c) == c \in NewCases
SpecStatusOutcomeDeferred(c) == c \in NewCases
SpecStatusReasonAggregateVerify(c) == c \in NewCases
SpecQueuedDebug(c) == c \in NewCases
SpecQueuedLenObserved(c) == c \in NewCases
SpecWakeSenderPresent(c) == c \in WakeCases
SpecWakeAttempted(c) == c \in WakeCases
SpecWakeResultIgnored(c) == c = "new_with_wake_failure"

ActualKeyPhase(c) ==
  CASE c # "idle" /\ Bug = "key_omits_phase" -> FALSE
    [] OTHER -> SpecKeyPhase(c)

ActualKeyHash(c) ==
  CASE c # "idle" /\ Bug = "key_omits_hash" -> FALSE
    [] OTHER -> SpecKeyHash(c)

ActualKeyHeight(c) ==
  CASE c # "idle" /\ Bug = "key_omits_height" -> FALSE
    [] OTHER -> SpecKeyHeight(c)

ActualKeyView(c) ==
  CASE c # "idle" /\ Bug = "key_omits_view" -> FALSE
    [] OTHER -> SpecKeyView(c)

ActualKeyEpoch(c) ==
  CASE c # "idle" /\ Bug = "key_omits_epoch" -> FALSE
    [] OTHER -> SpecKeyEpoch(c)

ActualKeyChainOrder(c) ==
  CASE c # "idle" /\ Bug = "key_omits_chain_order" -> FALSE
    [] OTHER -> SpecKeyChainOrder(c)

ActualKeyRechain(c) ==
  CASE c # "idle" /\ Bug = "key_omits_rechain" -> FALSE
    [] OTHER -> SpecKeyRechain(c)

ActualDuplicateCheck(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_skips_check" -> FALSE
    [] OTHER -> SpecDuplicateCheck(c)

ActualDuplicateFound(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_not_detected" -> FALSE
    [] OTHER -> SpecDuplicateFound(c)

ActualDuplicateDebug(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_skips_debug" -> FALSE
    [] OTHER -> SpecDuplicateDebug(c)

ActualExistingPreserved(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_overwrites_existing" -> FALSE
    [] OTHER -> SpecExistingPreserved(c)

ActualInsertWork(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_inserts" -> TRUE
    [] c \in NewCases /\ Bug = "new_skips_insert" -> FALSE
    [] c = "new_with_wake_failure" /\ Bug = "wake_failure_rolls_back_insert" -> FALSE
    [] OTHER -> SpecInsertWork(c)

ActualPreserveWork(c) ==
  CASE c \in NewCases /\ Bug = "new_drops_work" -> FALSE
    [] OTHER -> SpecPreserveWork(c)

ActualRecordStatus(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_records_deferred" -> TRUE
    [] c \in NewCases /\ Bug = "new_skips_status" -> FALSE
    [] c = "new_with_wake_failure" /\ Bug = "wake_failure_skips_status" -> FALSE
    [] OTHER -> SpecRecordStatus(c)

ActualStatusKindQc(c) ==
  CASE c \in NewCases /\ Bug = "new_wrong_status_kind" -> FALSE
    [] OTHER -> SpecStatusKindQc(c)

ActualStatusOutcomeDeferred(c) ==
  CASE c \in NewCases /\ Bug = "new_wrong_status_outcome" -> FALSE
    [] OTHER -> SpecStatusOutcomeDeferred(c)

ActualStatusReasonAggregateVerify(c) ==
  CASE c \in NewCases /\ Bug = "new_wrong_status_reason" -> FALSE
    [] OTHER -> SpecStatusReasonAggregateVerify(c)

ActualQueuedDebug(c) ==
  CASE c \in NewCases /\ Bug = "new_skips_debug" -> FALSE
    [] OTHER -> SpecQueuedDebug(c)

ActualQueuedLenObserved(c) ==
  CASE c \in NewCases /\ Bug = "new_skips_queued_len" -> FALSE
    [] OTHER -> SpecQueuedLenObserved(c)

ActualWakeSenderPresent(c) ==
  SpecWakeSenderPresent(c)

ActualWakeAttempted(c) ==
  CASE c = "duplicate_existing" /\ Bug = "duplicate_wakes" -> TRUE
    [] c = "new_without_wake" /\ Bug = "wake_absent_attempts" -> TRUE
    [] c \in WakeCases /\ Bug = "wake_present_skips_attempt" -> FALSE
    [] OTHER -> SpecWakeAttempted(c)

ActualWakeResultIgnored(c) ==
  CASE c = "new_with_wake_failure" /\ Bug = "wake_failure_not_ignored" -> FALSE
    [] OTHER -> SpecWakeResultIgnored(c)

Bugs == {
  "none",
  "key_omits_phase",
  "key_omits_hash",
  "key_omits_height",
  "key_omits_view",
  "key_omits_epoch",
  "key_omits_chain_order",
  "key_omits_rechain",
  "duplicate_skips_check",
  "duplicate_not_detected",
  "duplicate_skips_debug",
  "duplicate_inserts",
  "duplicate_overwrites_existing",
  "duplicate_records_deferred",
  "duplicate_wakes",
  "new_skips_insert",
  "new_drops_work",
  "new_skips_status",
  "new_wrong_status_kind",
  "new_wrong_status_outcome",
  "new_wrong_status_reason",
  "new_skips_debug",
  "new_skips_queued_len",
  "wake_absent_attempts",
  "wake_present_skips_attempt",
  "wake_failure_not_ignored",
  "wake_failure_rolls_back_insert",
  "wake_failure_skips_status"
}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ candidate \in Cases
  /\ keyPhase \in BOOLEAN
  /\ keyHash \in BOOLEAN
  /\ keyHeight \in BOOLEAN
  /\ keyView \in BOOLEAN
  /\ keyEpoch \in BOOLEAN
  /\ keyChainOrder \in BOOLEAN
  /\ keyRechain \in BOOLEAN
  /\ duplicateCheck \in BOOLEAN
  /\ duplicateFound \in BOOLEAN
  /\ duplicateDebug \in BOOLEAN
  /\ existingPreserved \in BOOLEAN
  /\ insertWork \in BOOLEAN
  /\ preserveWork \in BOOLEAN
  /\ recordStatus \in BOOLEAN
  /\ statusKindQc \in BOOLEAN
  /\ statusOutcomeDeferred \in BOOLEAN
  /\ statusReasonAggregateVerify \in BOOLEAN
  /\ queuedDebug \in BOOLEAN
  /\ queuedLenObserved \in BOOLEAN
  /\ wakeSenderPresent \in BOOLEAN
  /\ wakeAttempted \in BOOLEAN
  /\ wakeResultIgnored \in BOOLEAN

Init ==
  /\ candidate = "idle"
  /\ keyPhase = FALSE
  /\ keyHash = FALSE
  /\ keyHeight = FALSE
  /\ keyView = FALSE
  /\ keyEpoch = FALSE
  /\ keyChainOrder = FALSE
  /\ keyRechain = FALSE
  /\ duplicateCheck = FALSE
  /\ duplicateFound = FALSE
  /\ duplicateDebug = FALSE
  /\ existingPreserved = FALSE
  /\ insertWork = FALSE
  /\ preserveWork = FALSE
  /\ recordStatus = FALSE
  /\ statusKindQc = FALSE
  /\ statusOutcomeDeferred = FALSE
  /\ statusReasonAggregateVerify = FALSE
  /\ queuedDebug = FALSE
  /\ queuedLenObserved = FALSE
  /\ wakeSenderPresent = FALSE
  /\ wakeAttempted = FALSE
  /\ wakeResultIgnored = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ keyPhase' = ActualKeyPhase(c)
  /\ keyHash' = ActualKeyHash(c)
  /\ keyHeight' = ActualKeyHeight(c)
  /\ keyView' = ActualKeyView(c)
  /\ keyEpoch' = ActualKeyEpoch(c)
  /\ keyChainOrder' = ActualKeyChainOrder(c)
  /\ keyRechain' = ActualKeyRechain(c)
  /\ duplicateCheck' = ActualDuplicateCheck(c)
  /\ duplicateFound' = ActualDuplicateFound(c)
  /\ duplicateDebug' = ActualDuplicateDebug(c)
  /\ existingPreserved' = ActualExistingPreserved(c)
  /\ insertWork' = ActualInsertWork(c)
  /\ preserveWork' = ActualPreserveWork(c)
  /\ recordStatus' = ActualRecordStatus(c)
  /\ statusKindQc' = ActualStatusKindQc(c)
  /\ statusOutcomeDeferred' = ActualStatusOutcomeDeferred(c)
  /\ statusReasonAggregateVerify' = ActualStatusReasonAggregateVerify(c)
  /\ queuedDebug' = ActualQueuedDebug(c)
  /\ queuedLenObserved' = ActualQueuedLenObserved(c)
  /\ wakeSenderPresent' = ActualWakeSenderPresent(c)
  /\ wakeAttempted' = ActualWakeAttempted(c)
  /\ wakeResultIgnored' = ActualWakeResultIgnored(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ keyPhase = SpecKeyPhase(candidate)
  /\ keyHash = SpecKeyHash(candidate)
  /\ keyHeight = SpecKeyHeight(candidate)
  /\ keyView = SpecKeyView(candidate)
  /\ keyEpoch = SpecKeyEpoch(candidate)
  /\ keyChainOrder = SpecKeyChainOrder(candidate)
  /\ keyRechain = SpecKeyRechain(candidate)
  /\ duplicateCheck = SpecDuplicateCheck(candidate)
  /\ duplicateFound = SpecDuplicateFound(candidate)
  /\ duplicateDebug = SpecDuplicateDebug(candidate)
  /\ existingPreserved = SpecExistingPreserved(candidate)
  /\ insertWork = SpecInsertWork(candidate)
  /\ preserveWork = SpecPreserveWork(candidate)
  /\ recordStatus = SpecRecordStatus(candidate)
  /\ statusKindQc = SpecStatusKindQc(candidate)
  /\ statusOutcomeDeferred = SpecStatusOutcomeDeferred(candidate)
  /\ statusReasonAggregateVerify = SpecStatusReasonAggregateVerify(candidate)
  /\ queuedDebug = SpecQueuedDebug(candidate)
  /\ queuedLenObserved = SpecQueuedLenObserved(candidate)
  /\ wakeSenderPresent = SpecWakeSenderPresent(candidate)
  /\ wakeAttempted = SpecWakeAttempted(candidate)
  /\ wakeResultIgnored = SpecWakeResultIgnored(candidate)

SafetyFast == MatchesSpec

BugKeyOmitsPhase == ActualKeyPhase("new_without_wake")
BugKeyOmitsHash == ActualKeyHash("new_without_wake")
BugKeyOmitsHeight == ActualKeyHeight("new_without_wake")
BugKeyOmitsView == ActualKeyView("new_without_wake")
BugKeyOmitsEpoch == ActualKeyEpoch("new_without_wake")
BugKeyOmitsChainOrder == ActualKeyChainOrder("new_without_wake")
BugKeyOmitsRechain == ActualKeyRechain("new_without_wake")
BugDuplicateSkipsCheck == ActualDuplicateCheck("duplicate_existing")
BugDuplicateNotDetected == ActualDuplicateFound("duplicate_existing")
BugDuplicateSkipsDebug == ActualDuplicateDebug("duplicate_existing")
BugDuplicateInserts == ~ActualInsertWork("duplicate_existing")
BugDuplicateOverwritesExisting == ActualExistingPreserved("duplicate_existing")
BugDuplicateRecordsDeferred == ~ActualRecordStatus("duplicate_existing")
BugDuplicateWakes == ~ActualWakeAttempted("duplicate_existing")
BugNewSkipsInsert == ActualInsertWork("new_without_wake")
BugNewDropsWork == ActualPreserveWork("new_without_wake")
BugNewSkipsStatus == ActualRecordStatus("new_without_wake")
BugNewWrongStatusKind == ActualStatusKindQc("new_without_wake")
BugNewWrongStatusOutcome == ActualStatusOutcomeDeferred("new_without_wake")
BugNewWrongStatusReason == ActualStatusReasonAggregateVerify("new_without_wake")
BugNewSkipsDebug == ActualQueuedDebug("new_without_wake")
BugNewSkipsQueuedLen == ActualQueuedLenObserved("new_without_wake")
BugWakeAbsentAttempts == ~ActualWakeAttempted("new_without_wake")
BugWakePresentSkipsAttempt == ActualWakeAttempted("new_with_wake_success")
BugWakeFailureNotIgnored == ActualWakeResultIgnored("new_with_wake_failure")
BugWakeFailureRollsBackInsert == ActualInsertWork("new_with_wake_failure")
BugWakeFailureSkipsStatus == ActualRecordStatus("new_with_wake_failure")

=============================================================================
====

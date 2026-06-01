---- MODULE SumeragiDeferredBlockSyncReplayGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `try_replay_deferred_block_sync_updates(...)`.

The replay helper is intentionally small but safety-critical:

- no work is performed when the deferred map is empty,
- commit or validation work blocks replay and preserves the queue,
- replay chooses the first ordered key from the deferred map,
- the selected entry is removed before calling `handle_block_sync_update`,
- the stored update and sender are forwarded unchanged,
- handler errors are logged but still produce a successful replay return, and
- only one entry is consumed per call, leaving later ordered entries buffered.
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
  "empty_queue",
  "commit_inflight",
  "validation_inflight",
  "single_success",
  "single_error",
  "multiple_select_first",
  "remove_missing"
}

InitialLen(c) ==
  CASE c = "empty_queue" -> 0
    [] c = "multiple_select_first" -> 2
    [] OTHER -> 1

HasQueue(c) ==
  InitialLen(c) > 0

CommitInflight(c) ==
  c = "commit_inflight"

ValidationInflight(c) ==
  c = "validation_inflight"

RemoveSucceeds(c) ==
  c # "remove_missing"

HandleErrors(c) ==
  c = "single_error"

FirstKey(c) ==
  IF c = "multiple_select_first" THEN "early" ELSE "only"

SecondKey(c) ==
  IF c = "multiple_select_first" THEN "late" ELSE "none"

SpecReturn(c) ==
  /\ HasQueue(c)
  /\ ~CommitInflight(c)
  /\ ~ValidationInflight(c)
  /\ RemoveSucceeds(c)

SpecSelectedKey(c) ==
  IF SpecReturn(c) THEN FirstKey(c) ELSE "none"

SpecRemovedBeforeHandle(c) ==
  SpecReturn(c)

SpecHandleCalled(c) ==
  SpecReturn(c)

SpecUpdateForwarded(c) ==
  SpecHandleCalled(c)

SpecSenderForwarded(c) ==
  SpecHandleCalled(c)

SpecWarned(c) ==
  SpecHandleCalled(c) /\ HandleErrors(c)

SpecFinalLen(c) ==
  InitialLen(c) - (IF SpecReturn(c) THEN 1 ELSE 0)

SpecSecondEntryPreserved(c) ==
  IF c = "multiple_select_first" THEN SpecFinalLen(c) = 1 ELSE TRUE

ActualReturn(c) ==
  CASE Bug = "empty_returns_true"
       /\ c = "empty_queue" -> TRUE
    [] Bug = "commit_inflight_replays"
       /\ c = "commit_inflight" -> TRUE
    [] Bug = "validation_inflight_replays"
       /\ c = "validation_inflight" -> TRUE
    [] Bug = "ready_returns_false"
       /\ c = "single_success" -> FALSE
    [] Bug = "returns_false_on_handler_error"
       /\ c = "single_error" -> FALSE
    [] Bug = "remove_missing_returns_true"
       /\ c = "remove_missing" -> TRUE
    [] OTHER -> SpecReturn(c)

ActualSelectedKey(c) ==
  IF ~ActualReturn(c) THEN "none"
  ELSE CASE Bug = "selects_last_key"
            /\ c = "multiple_select_first" -> SecondKey(c)
         [] OTHER -> FirstKey(c)

ActualRemovedBeforeHandle(c) ==
  IF ~ActualReturn(c) THEN FALSE
  ELSE CASE Bug = "skips_remove_before_handle"
            /\ c = "single_success" -> FALSE
         [] OTHER -> TRUE

ActualHandleCalled(c) ==
  IF ~ActualReturn(c) THEN
    /\ Bug = "handle_on_remove_missing"
    /\ c = "remove_missing"
  ELSE CASE Bug = "handle_not_called"
            /\ c = "single_success" -> FALSE
         [] OTHER -> TRUE

ActualUpdateForwarded(c) ==
  IF ~ActualHandleCalled(c) THEN FALSE
  ELSE CASE Bug = "passes_wrong_update"
            /\ c = "single_success" -> FALSE
         [] OTHER -> TRUE

ActualSenderForwarded(c) ==
  IF ~ActualHandleCalled(c) THEN FALSE
  ELSE CASE Bug = "drops_sender"
            /\ c = "single_success" -> FALSE
         [] OTHER -> TRUE

ActualWarned(c) ==
  CASE Bug = "warn_on_success"
       /\ c = "single_success" -> TRUE
    [] Bug = "missing_warn_on_error"
       /\ c = "single_error" -> FALSE
    [] OTHER -> ActualHandleCalled(c) /\ HandleErrors(c)

ActualFinalLen(c) ==
  CASE Bug = "reinsert_on_handler_error"
       /\ c = "single_error" -> InitialLen(c)
    [] Bug = "removes_all_entries"
       /\ c = "multiple_select_first" -> 0
    [] ActualReturn(c) -> InitialLen(c) - 1
    [] OTHER -> InitialLen(c)

ActualSecondEntryPreserved(c) ==
  IF c = "multiple_select_first" THEN ActualFinalLen(c) = 1 ELSE TRUE

Matches(c) ==
  /\ ActualReturn(c) = SpecReturn(c)
  /\ ActualSelectedKey(c) = SpecSelectedKey(c)
  /\ ActualRemovedBeforeHandle(c) = SpecRemovedBeforeHandle(c)
  /\ ActualHandleCalled(c) = SpecHandleCalled(c)
  /\ ActualUpdateForwarded(c) = SpecUpdateForwarded(c)
  /\ ActualSenderForwarded(c) = SpecSenderForwarded(c)
  /\ ActualWarned(c) = SpecWarned(c)
  /\ ActualFinalLen(c) = SpecFinalLen(c)
  /\ ActualSecondEntryPreserved(c) = SpecSecondEntryPreserved(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_returns_true",
       "commit_inflight_replays",
       "validation_inflight_replays",
       "ready_returns_false",
       "selects_last_key",
       "skips_remove_before_handle",
       "handle_not_called",
       "passes_wrong_update",
       "drops_sender",
       "returns_false_on_handler_error",
       "reinsert_on_handler_error",
       "warn_on_success",
       "missing_warn_on_error",
       "removes_all_entries",
       "remove_missing_returns_true",
       "handle_on_remove_missing"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

EmptyQueueNoop ==
  Matches("empty_queue")

CommitInflightBlocksReplay ==
  Matches("commit_inflight")

ValidationInflightBlocksReplay ==
  Matches("validation_inflight")

ReadyReturnsTrue ==
  Matches("single_success")

OldestKeySelected ==
  Matches("multiple_select_first")

RemovedBeforeHandle ==
  Matches("single_success")

HandleCalled ==
  Matches("single_success")

UpdateForwarded ==
  Matches("single_success")

SenderForwarded ==
  Matches("single_success")

HandlerErrorStillReturnsTrue ==
  Matches("single_error")

HandlerErrorNotReinserted ==
  Matches("single_error")

NoWarnOnSuccess ==
  Matches("single_success")

WarnOnError ==
  Matches("single_error")

LaterEntryPreserved ==
  Matches("multiple_select_first")

RemoveMissingReturnsFalse ==
  Matches("remove_missing")

RemoveMissingNoHandle ==
  Matches("remove_missing")

=============================================================================
====

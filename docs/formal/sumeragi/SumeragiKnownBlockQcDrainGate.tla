---- MODULE SumeragiKnownBlockQcDrainGate ----

(***************************************************************************
A bounded abstract model for `drain_known_block_qc_work(...)`.

The live helper drains deferred known-block QC work outside the payload queue.
It must return without work on an empty queue, stop before applying a job when
the tick budget is exhausted, process at most `KNOWN_BLOCK_QC_WORK_PER_TICK`
items, remove each work item before applying it, preserve remaining queued work
for a later tick, and report progress iff at least one applied work item
returned progress.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  queueEmpty,
  \* @type: Bool;
  noApply,
  \* @type: Bool;
  initialBudgetBreak,
  \* @type: Bool;
  budgetBreakAfterFirst,
  \* @type: Bool;
  removeFirst,
  \* @type: Bool;
  removeSecond,
  \* @type: Bool;
  applyFirst,
  \* @type: Bool;
  applySecond,
  \* @type: Bool;
  firstProgress,
  \* @type: Bool;
  secondProgress,
  \* @type: Bool;
  noProgressObserved,
  \* @type: Bool;
  processedOne,
  \* @type: Bool;
  processedTwo,
  \* @type: Bool;
  capStopsAfterTwo,
  \* @type: Bool;
  remainingPreserved,
  \* @type: Bool;
  returnProgress,
  \* @type: Bool;
  debugLog,
  \* @type: Bool;
  removeBeforeApply

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    queueEmpty,
    noApply,
    initialBudgetBreak,
    budgetBreakAfterFirst,
    removeFirst,
    removeSecond,
    applyFirst,
    applySecond,
    firstProgress,
    secondProgress,
    noProgressObserved,
    processedOne,
    processedTwo,
    capStopsAfterTwo,
    remainingPreserved,
    returnProgress,
    debugLog,
    removeBeforeApply>>

Cases == {
  "idle",
  "empty_queue",
  "initial_budget_exhausted",
  "one_item_progress",
  "one_item_no_progress",
  "two_items_first_progress",
  "two_items_second_progress",
  "two_items_no_progress",
  "cap_two_leaves_remaining",
  "budget_after_one_progress",
  "budget_after_one_no_progress"
}

FirstWorkCases == {
  "one_item_progress",
  "one_item_no_progress",
  "two_items_first_progress",
  "two_items_second_progress",
  "two_items_no_progress",
  "cap_two_leaves_remaining",
  "budget_after_one_progress",
  "budget_after_one_no_progress"
}

SecondWorkCases == {
  "two_items_first_progress",
  "two_items_second_progress",
  "two_items_no_progress",
  "cap_two_leaves_remaining"
}

ProgressCases == {
  "one_item_progress",
  "two_items_first_progress",
  "two_items_second_progress",
  "budget_after_one_progress"
}

SpecQueueEmpty(c) ==
  c = "empty_queue"

SpecNoApply(c) ==
  c \in {"empty_queue", "initial_budget_exhausted"}

SpecInitialBudgetBreak(c) ==
  c = "initial_budget_exhausted"

SpecBudgetBreakAfterFirst(c) ==
  c \in {"budget_after_one_progress", "budget_after_one_no_progress"}

SpecRemoveFirst(c) ==
  c \in FirstWorkCases

SpecRemoveSecond(c) ==
  c \in SecondWorkCases

SpecApplyFirst(c) ==
  c \in FirstWorkCases

SpecApplySecond(c) ==
  c \in SecondWorkCases

SpecFirstProgress(c) ==
  c \in {
    "one_item_progress",
    "two_items_first_progress",
    "budget_after_one_progress"
  }

SpecSecondProgress(c) ==
  c = "two_items_second_progress"

SpecNoProgressObserved(c) ==
  c \in {
    "one_item_no_progress",
    "two_items_no_progress",
    "cap_two_leaves_remaining",
    "budget_after_one_no_progress"
  }

SpecProcessedOne(c) ==
  c \in FirstWorkCases

SpecProcessedTwo(c) ==
  c \in SecondWorkCases

SpecCapStopsAfterTwo(c) ==
  c = "cap_two_leaves_remaining"

SpecRemainingPreserved(c) ==
  c \in {
    "cap_two_leaves_remaining",
    "budget_after_one_progress",
    "budget_after_one_no_progress"
  }

SpecReturnProgress(c) ==
  c \in ProgressCases

SpecDebugLog(c) ==
  c \in FirstWorkCases

SpecRemoveBeforeApply(c) ==
  c \in FirstWorkCases

ActualQueueEmpty(c) ==
  SpecQueueEmpty(c)

ActualNoApply(c) ==
  CASE c = "empty_queue" /\ Bug = "empty_applies" -> FALSE
    [] c = "initial_budget_exhausted" /\ Bug = "initial_budget_removes" -> FALSE
    [] OTHER -> SpecNoApply(c)

ActualInitialBudgetBreak(c) ==
  SpecInitialBudgetBreak(c)

ActualBudgetBreakAfterFirst(c) ==
  CASE c = "budget_after_one_no_progress"
       /\ Bug = "budget_after_one_processes_second" -> FALSE
    [] OTHER -> SpecBudgetBreakAfterFirst(c)

ActualRemoveFirst(c) ==
  CASE c = "empty_queue" /\ Bug = "empty_applies" -> TRUE
    [] c = "initial_budget_exhausted" /\ Bug = "initial_budget_removes" -> TRUE
    [] OTHER -> SpecRemoveFirst(c)

ActualRemoveSecond(c) ==
  CASE c = "one_item_progress" /\ Bug = "one_item_processes_second" -> TRUE
    [] c = "budget_after_one_no_progress"
          /\ Bug = "budget_after_one_processes_second" -> TRUE
    [] OTHER -> SpecRemoveSecond(c)

ActualApplyFirst(c) ==
  ActualRemoveFirst(c)

ActualApplySecond(c) ==
  ActualRemoveSecond(c)

ActualFirstProgress(c) ==
  CASE c = "two_items_first_progress" /\ Bug = "two_first_progress_lost" -> FALSE
    [] OTHER -> SpecFirstProgress(c)

ActualSecondProgress(c) ==
  CASE c = "two_items_second_progress" /\ Bug = "two_second_progress_lost" -> FALSE
    [] OTHER -> SpecSecondProgress(c)

ActualNoProgressObserved(c) ==
  SpecNoProgressObserved(c)

ActualProcessedOne(c) ==
  CASE c = "one_item_progress" /\ Bug = "processed_one_not_counted" -> FALSE
    [] c = "empty_queue" /\ Bug = "empty_applies" -> TRUE
    [] c = "initial_budget_exhausted" /\ Bug = "initial_budget_removes" -> TRUE
    [] OTHER -> SpecProcessedOne(c)

ActualProcessedTwo(c) ==
  CASE c = "two_items_second_progress" /\ Bug = "processed_two_not_counted" -> FALSE
    [] c = "one_item_progress" /\ Bug = "one_item_processes_second" -> TRUE
    [] c = "budget_after_one_no_progress"
          /\ Bug = "budget_after_one_processes_second" -> TRUE
    [] OTHER -> SpecProcessedTwo(c)

ActualCapStopsAfterTwo(c) ==
  CASE c = "cap_two_leaves_remaining" /\ Bug = "cap_processes_third" -> FALSE
    [] OTHER -> SpecCapStopsAfterTwo(c)

ActualRemainingPreserved(c) ==
  CASE c = "cap_two_leaves_remaining"
       /\ Bug \in {"cap_processes_third", "cap_drops_remaining"} -> FALSE
    [] c = "budget_after_one_progress"
          /\ Bug = "budget_after_one_drops_remaining" -> FALSE
    [] c = "budget_after_one_no_progress"
          /\ Bug = "budget_after_one_processes_second" -> FALSE
    [] OTHER -> SpecRemainingPreserved(c)

ActualReturnProgress(c) ==
  CASE c = "empty_queue" /\ Bug = "empty_returns_true" -> TRUE
    [] c = "initial_budget_exhausted"
          /\ Bug = "initial_budget_returns_true" -> TRUE
    [] c = "one_item_progress" /\ Bug = "one_progress_returns_false" -> FALSE
    [] c = "one_item_no_progress" /\ Bug = "one_no_progress_returns_true" -> TRUE
    [] c = "two_items_first_progress" /\ Bug = "two_first_progress_lost" -> FALSE
    [] c = "two_items_second_progress" /\ Bug = "two_second_progress_lost" -> FALSE
    [] c = "two_items_no_progress" /\ Bug = "two_no_progress_returns_true" -> TRUE
    [] c = "cap_two_leaves_remaining" /\ Bug = "cap_processes_third" -> TRUE
    [] OTHER -> SpecReturnProgress(c)

ActualDebugLog(c) ==
  CASE c = "one_item_progress" /\ Bug = "debug_log_skipped" -> FALSE
    [] c = "empty_queue" /\ Bug = "empty_applies" -> TRUE
    [] c = "initial_budget_exhausted" /\ Bug = "initial_budget_removes" -> TRUE
    [] OTHER -> SpecDebugLog(c)

ActualRemoveBeforeApply(c) ==
  CASE c = "one_item_progress" /\ Bug = "apply_before_remove" -> FALSE
    [] c = "empty_queue" /\ Bug = "empty_applies" -> TRUE
    [] c = "initial_budget_exhausted" /\ Bug = "initial_budget_removes" -> TRUE
    [] OTHER -> SpecRemoveBeforeApply(c)

Bugs == {
  "none",
  "empty_returns_true",
  "empty_applies",
  "initial_budget_removes",
  "initial_budget_returns_true",
  "one_progress_returns_false",
  "one_no_progress_returns_true",
  "one_item_processes_second",
  "two_first_progress_lost",
  "two_second_progress_lost",
  "two_no_progress_returns_true",
  "cap_processes_third",
  "cap_drops_remaining",
  "budget_after_one_processes_second",
  "budget_after_one_drops_remaining",
  "processed_one_not_counted",
  "processed_two_not_counted",
  "debug_log_skipped",
  "apply_before_remove"
}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ candidate \in Cases
  /\ queueEmpty \in BOOLEAN
  /\ noApply \in BOOLEAN
  /\ initialBudgetBreak \in BOOLEAN
  /\ budgetBreakAfterFirst \in BOOLEAN
  /\ removeFirst \in BOOLEAN
  /\ removeSecond \in BOOLEAN
  /\ applyFirst \in BOOLEAN
  /\ applySecond \in BOOLEAN
  /\ firstProgress \in BOOLEAN
  /\ secondProgress \in BOOLEAN
  /\ noProgressObserved \in BOOLEAN
  /\ processedOne \in BOOLEAN
  /\ processedTwo \in BOOLEAN
  /\ capStopsAfterTwo \in BOOLEAN
  /\ remainingPreserved \in BOOLEAN
  /\ returnProgress \in BOOLEAN
  /\ debugLog \in BOOLEAN
  /\ removeBeforeApply \in BOOLEAN

Init ==
  /\ candidate = "idle"
  /\ queueEmpty = FALSE
  /\ noApply = FALSE
  /\ initialBudgetBreak = FALSE
  /\ budgetBreakAfterFirst = FALSE
  /\ removeFirst = FALSE
  /\ removeSecond = FALSE
  /\ applyFirst = FALSE
  /\ applySecond = FALSE
  /\ firstProgress = FALSE
  /\ secondProgress = FALSE
  /\ noProgressObserved = FALSE
  /\ processedOne = FALSE
  /\ processedTwo = FALSE
  /\ capStopsAfterTwo = FALSE
  /\ remainingPreserved = FALSE
  /\ returnProgress = FALSE
  /\ debugLog = FALSE
  /\ removeBeforeApply = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ queueEmpty' = ActualQueueEmpty(c)
  /\ noApply' = ActualNoApply(c)
  /\ initialBudgetBreak' = ActualInitialBudgetBreak(c)
  /\ budgetBreakAfterFirst' = ActualBudgetBreakAfterFirst(c)
  /\ removeFirst' = ActualRemoveFirst(c)
  /\ removeSecond' = ActualRemoveSecond(c)
  /\ applyFirst' = ActualApplyFirst(c)
  /\ applySecond' = ActualApplySecond(c)
  /\ firstProgress' = ActualFirstProgress(c)
  /\ secondProgress' = ActualSecondProgress(c)
  /\ noProgressObserved' = ActualNoProgressObserved(c)
  /\ processedOne' = ActualProcessedOne(c)
  /\ processedTwo' = ActualProcessedTwo(c)
  /\ capStopsAfterTwo' = ActualCapStopsAfterTwo(c)
  /\ remainingPreserved' = ActualRemainingPreserved(c)
  /\ returnProgress' = ActualReturnProgress(c)
  /\ debugLog' = ActualDebugLog(c)
  /\ removeBeforeApply' = ActualRemoveBeforeApply(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ queueEmpty = SpecQueueEmpty(candidate)
  /\ noApply = SpecNoApply(candidate)
  /\ initialBudgetBreak = SpecInitialBudgetBreak(candidate)
  /\ budgetBreakAfterFirst = SpecBudgetBreakAfterFirst(candidate)
  /\ removeFirst = SpecRemoveFirst(candidate)
  /\ removeSecond = SpecRemoveSecond(candidate)
  /\ applyFirst = SpecApplyFirst(candidate)
  /\ applySecond = SpecApplySecond(candidate)
  /\ firstProgress = SpecFirstProgress(candidate)
  /\ secondProgress = SpecSecondProgress(candidate)
  /\ noProgressObserved = SpecNoProgressObserved(candidate)
  /\ processedOne = SpecProcessedOne(candidate)
  /\ processedTwo = SpecProcessedTwo(candidate)
  /\ capStopsAfterTwo = SpecCapStopsAfterTwo(candidate)
  /\ remainingPreserved = SpecRemainingPreserved(candidate)
  /\ returnProgress = SpecReturnProgress(candidate)
  /\ debugLog = SpecDebugLog(candidate)
  /\ removeBeforeApply = SpecRemoveBeforeApply(candidate)

KnownBlockQcDrainExactness ==
  MatchesSpec

KnownBlockQcDrainCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ KnownBlockQcDrainExactness

SafetyFast ==
  KnownBlockQcDrainExactness

BugEmptyReturnsTrue ==
  ~ActualReturnProgress("empty_queue")

BugEmptyApplies ==
  /\ ActualNoApply("empty_queue")
  /\ ~ActualRemoveFirst("empty_queue")
  /\ ~ActualApplyFirst("empty_queue")
  /\ ~ActualProcessedOne("empty_queue")

BugInitialBudgetRemoves ==
  /\ ActualNoApply("initial_budget_exhausted")
  /\ ~ActualRemoveFirst("initial_budget_exhausted")
  /\ ~ActualApplyFirst("initial_budget_exhausted")
  /\ ~ActualProcessedOne("initial_budget_exhausted")

BugInitialBudgetReturnsTrue ==
  ~ActualReturnProgress("initial_budget_exhausted")

BugOneProgressReturnsFalse ==
  ActualReturnProgress("one_item_progress")

BugOneNoProgressReturnsTrue ==
  ~ActualReturnProgress("one_item_no_progress")

BugOneItemProcessesSecond ==
  /\ ~ActualRemoveSecond("one_item_progress")
  /\ ~ActualApplySecond("one_item_progress")
  /\ ~ActualProcessedTwo("one_item_progress")

BugTwoFirstProgressLost ==
  /\ ActualFirstProgress("two_items_first_progress")
  /\ ActualReturnProgress("two_items_first_progress")

BugTwoSecondProgressLost ==
  /\ ActualSecondProgress("two_items_second_progress")
  /\ ActualReturnProgress("two_items_second_progress")

BugTwoNoProgressReturnsTrue ==
  ~ActualReturnProgress("two_items_no_progress")

BugCapProcessesThird ==
  /\ ActualCapStopsAfterTwo("cap_two_leaves_remaining")
  /\ ActualRemainingPreserved("cap_two_leaves_remaining")
  /\ ~ActualReturnProgress("cap_two_leaves_remaining")

BugCapDropsRemaining ==
  ActualRemainingPreserved("cap_two_leaves_remaining")

BugBudgetAfterOneProcessesSecond ==
  /\ ActualBudgetBreakAfterFirst("budget_after_one_no_progress")
  /\ ActualRemainingPreserved("budget_after_one_no_progress")
  /\ ~ActualRemoveSecond("budget_after_one_no_progress")
  /\ ~ActualApplySecond("budget_after_one_no_progress")
  /\ ~ActualProcessedTwo("budget_after_one_no_progress")

BugBudgetAfterOneDropsRemaining ==
  ActualRemainingPreserved("budget_after_one_progress")

BugProcessedOneNotCounted ==
  ActualProcessedOne("one_item_progress")

BugProcessedTwoNotCounted ==
  ActualProcessedTwo("two_items_second_progress")

BugDebugLogSkipped ==
  ActualDebugLog("one_item_progress")

BugApplyBeforeRemove ==
  ActualRemoveBeforeApply("one_item_progress")

=============================================================================
====

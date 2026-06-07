---- MODULE SumeragiCollectorPlanGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `CollectorPlan`.

The helper stores an ordered collector target list plus retry state. New plans
start unsent, restored plans cap the supplied sent count to the target length,
`peek()` is read-only, `next()` returns the current target and advances by one,
`exhausted()` is true exactly at or beyond the target length, and the gossip
fallback trigger returns true exactly once.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  input_len,
  \* @type: Int;
  input_sent,
  \* @type: Bool;
  input_gossip,
  \* @type: Int;
  target_len_after,
  \* @type: Int;
  plan_sent,
  \* @type: Int;
  sent_count,
  \* @type: Bool;
  peek_present,
  \* @type: Int;
  peek_index,
  \* @type: Int;
  sent_after_peek,
  \* @type: Bool;
  next_present,
  \* @type: Int;
  next_index,
  \* @type: Int;
  sent_after_next,
  \* @type: Bool;
  exhausted,
  \* @type: Bool;
  trigger_return,
  \* @type: Bool;
  gossip_after_trigger

\* @type: <<Str, Int, Int, Bool, Int, Int, Int, Bool, Int, Int, Bool, Int, Int, Bool, Bool, Bool>>;
vars ==
  <<candidate, input_len, input_sent, input_gossip, target_len_after,
    plan_sent, sent_count, peek_present, peek_index, sent_after_peek,
    next_present, next_index, sent_after_next, exhausted, trigger_return,
    gossip_after_trigger>>

Cases == {
  "new_empty",
  "new_three",
  "default_empty",
  "with_sent_zero",
  "with_sent_middle",
  "with_sent_exact",
  "with_sent_over",
  "already_gossip_triggered"
}

NewCases == {"new_empty", "new_three"}
DefaultCases == {"default_empty"}
WithSentCases == {
  "with_sent_zero",
  "with_sent_middle",
  "with_sent_exact",
  "with_sent_over"
}
ConstructorCases == NewCases \union DefaultCases \union WithSentCases

Min(a, b) == IF a <= b THEN a ELSE b

InputLen(c) ==
  CASE c \in {"new_empty", "default_empty"} -> 0
    [] OTHER -> 3

InputSent(c) ==
  CASE c \in (NewCases \union DefaultCases \union {"with_sent_zero"}) -> 0
    [] c = "with_sent_middle" -> 1
    [] c = "with_sent_exact" -> 3
    [] c = "with_sent_over" -> 5
    [] c = "already_gossip_triggered" -> 1

InputGossip(c) ==
  c = "already_gossip_triggered"

SpecTargetLen(c) == InputLen(c)

SpecPlanSent(c) ==
  IF c \in (NewCases \union DefaultCases)
  THEN 0
  ELSE Min(InputSent(c), InputLen(c))

SpecSentCount(c) == SpecPlanSent(c)
SpecPeekPresent(c) == SpecPlanSent(c) < SpecTargetLen(c)
SpecPeekIndex(c) == IF SpecPeekPresent(c) THEN SpecPlanSent(c) ELSE 0
SpecSentAfterPeek(c) == SpecPlanSent(c)
SpecNextPresent(c) == SpecPlanSent(c) < SpecTargetLen(c)
SpecNextIndex(c) == IF SpecNextPresent(c) THEN SpecPlanSent(c) ELSE 0
SpecSentAfterNext(c) ==
  IF SpecNextPresent(c) THEN SpecPlanSent(c) + 1 ELSE SpecPlanSent(c)
SpecExhausted(c) == SpecPlanSent(c) >= SpecTargetLen(c)
SpecTriggerReturn(c) == ~InputGossip(c)
SpecGossipAfterTrigger(c) == TRUE

ActualTargetLen(c) ==
  CASE Bug = "default_not_empty" /\ c = "default_empty" -> 1
    [] Bug = "target_len_mutated" /\ c = "new_three" -> 2
    [] OTHER -> SpecTargetLen(c)

ActualPlanSent(c) ==
  CASE Bug = "new_starts_at_one" /\ c = "new_three" -> 1
    [] Bug = "with_sent_not_capped" /\ c = "with_sent_over" ->
         InputSent(c)
    [] OTHER -> SpecPlanSent(c)

ActualSentCount(c) ==
  CASE Bug = "sent_count_reports_len" /\ c = "with_sent_middle" ->
         ActualTargetLen(c)
    [] OTHER -> ActualPlanSent(c)

ActualPeekPresent(c) ==
  CASE Bug = "peek_returns_when_exhausted" /\ c = "with_sent_exact" ->
         TRUE
    [] OTHER -> ActualPlanSent(c) < ActualTargetLen(c)

ActualPeekIndex(c) ==
  CASE Bug = "peek_off_by_one" /\ c = "with_sent_middle" ->
         ActualPlanSent(c) + 1
    [] OTHER -> IF ActualPeekPresent(c) THEN ActualPlanSent(c) ELSE 0

ActualSentAfterPeek(c) ==
  CASE Bug = "peek_advances" /\ c = "with_sent_middle" ->
         ActualPlanSent(c) + 1
    [] OTHER -> ActualPlanSent(c)

ActualNextPresent(c) ==
  IF ActualPlanSent(c) < ActualTargetLen(c) THEN TRUE ELSE FALSE

ActualNextIndex(c) ==
  CASE Bug = "next_off_by_one" /\ c = "with_sent_middle" ->
         ActualPlanSent(c) + 1
    [] OTHER -> IF ActualNextPresent(c) THEN ActualPlanSent(c) ELSE 0

ActualSentAfterNext(c) ==
  CASE Bug = "next_does_not_advance" /\ c = "with_sent_middle" ->
         ActualPlanSent(c)
    [] Bug = "next_advances_when_exhausted" /\ c = "with_sent_exact" ->
         ActualPlanSent(c) + 1
    [] OTHER ->
         IF ActualNextPresent(c) THEN ActualPlanSent(c) + 1 ELSE ActualPlanSent(c)

ActualExhausted(c) ==
  CASE Bug = "exhausted_strict_greater" /\ c = "with_sent_exact" ->
         ActualPlanSent(c) > ActualTargetLen(c)
    [] Bug = "exhausted_ignores_empty" /\ c = "new_empty" -> FALSE
    [] OTHER -> ActualPlanSent(c) >= ActualTargetLen(c)

ActualTriggerReturn(c) ==
  CASE Bug = "trigger_first_false" /\ c = "with_sent_middle" -> FALSE
    [] Bug = "trigger_second_repeats" /\ c = "already_gossip_triggered" ->
         TRUE
    [] OTHER -> ~InputGossip(c)

ActualGossipAfterTrigger(c) ==
  CASE Bug = "trigger_does_not_set_flag" /\ c = "with_sent_middle" ->
         FALSE
    [] Bug = "trigger_clears_flag" /\ c = "already_gossip_triggered" ->
         FALSE
    [] OTHER -> TRUE

Init ==
  /\ candidate \in Cases
  /\ input_len = InputLen(candidate)
  /\ input_sent = InputSent(candidate)
  /\ input_gossip = InputGossip(candidate)
  /\ target_len_after = ActualTargetLen(candidate)
  /\ plan_sent = ActualPlanSent(candidate)
  /\ sent_count = ActualSentCount(candidate)
  /\ peek_present = ActualPeekPresent(candidate)
  /\ peek_index = ActualPeekIndex(candidate)
  /\ sent_after_peek = ActualSentAfterPeek(candidate)
  /\ next_present = ActualNextPresent(candidate)
  /\ next_index = ActualNextIndex(candidate)
  /\ sent_after_next = ActualSentAfterNext(candidate)
  /\ exhausted = ActualExhausted(candidate)
  /\ trigger_return = ActualTriggerReturn(candidate)
  /\ gossip_after_trigger = ActualGossipAfterTrigger(candidate)

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "new_starts_at_one",
       "default_not_empty",
       "with_sent_not_capped",
       "sent_count_reports_len",
       "target_len_mutated",
       "peek_advances",
       "peek_off_by_one",
       "peek_returns_when_exhausted",
       "next_does_not_advance",
       "next_off_by_one",
       "next_advances_when_exhausted",
       "exhausted_strict_greater",
       "exhausted_ignores_empty",
       "trigger_first_false",
       "trigger_second_repeats",
       "trigger_does_not_set_flag",
       "trigger_clears_flag"
     }
  /\ candidate \in Cases
  /\ input_len \in 0..3
  /\ input_sent \in 0..5
  /\ input_gossip \in BOOLEAN
  /\ target_len_after \in 0..3
  /\ plan_sent \in 0..5
  /\ sent_count \in 0..5
  /\ peek_present \in BOOLEAN
  /\ peek_index \in 0..5
  /\ sent_after_peek \in 0..5
  /\ next_present \in BOOLEAN
  /\ next_index \in 0..5
  /\ sent_after_next \in 0..5
  /\ exhausted \in BOOLEAN
  /\ trigger_return \in BOOLEAN
  /\ gossip_after_trigger \in BOOLEAN

TargetLenPreserved ==
  target_len_after = SpecTargetLen(candidate)

PlanSentMatchesConstructor ==
  plan_sent = SpecPlanSent(candidate)

SentNeverExceedsTargets ==
  plan_sent <= target_len_after

NewAndDefaultPlansStartUnsent ==
  candidate \in (NewCases \union DefaultCases) => plan_sent = 0

WithSentCapsInputAtLength ==
  candidate \in WithSentCases => plan_sent = Min(input_sent, target_len_after)

SentCountMatchesPlan ==
  sent_count = plan_sent

PeekMatchesSpec ==
  /\ peek_present = SpecPeekPresent(candidate)
  /\ peek_index = SpecPeekIndex(candidate)

PeekDoesNotAdvance ==
  sent_after_peek = plan_sent

NextMatchesSpec ==
  /\ next_present = SpecNextPresent(candidate)
  /\ next_index = SpecNextIndex(candidate)
  /\ sent_after_next = SpecSentAfterNext(candidate)

NextReturnsCurrentTargetAndAdvances ==
  next_present =>
    /\ next_index = plan_sent
    /\ sent_after_next = plan_sent + 1

NextAtExhaustionIsNoop ==
  exhausted =>
    /\ ~next_present
    /\ sent_after_next = plan_sent

ExhaustedMatchesSpec ==
  exhausted = SpecExhausted(candidate)

ExhaustedIffSentAtOrBeyondLength ==
  exhausted <=> plan_sent >= target_len_after

TriggerReturnMatchesSpec ==
  trigger_return = SpecTriggerReturn(candidate)

TriggerSetsGossipFlag ==
  gossip_after_trigger = TRUE

TriggerReturnsTrueOnlyFirstTime ==
  trigger_return <=> ~input_gossip

CollectorConstructorStateExact ==
  /\ TargetLenPreserved
  /\ PlanSentMatchesConstructor
  /\ SentNeverExceedsTargets
  /\ NewAndDefaultPlansStartUnsent
  /\ WithSentCapsInputAtLength
  /\ SentCountMatchesPlan

CollectorCursorReadExact ==
  /\ PeekMatchesSpec
  /\ PeekDoesNotAdvance

CollectorCursorAdvanceExact ==
  /\ NextMatchesSpec
  /\ NextReturnsCurrentTargetAndAdvances
  /\ NextAtExhaustionIsNoop

CollectorExhaustionExact ==
  /\ ExhaustedMatchesSpec
  /\ ExhaustedIffSentAtOrBeyondLength

CollectorGossipFallbackExact ==
  /\ TriggerReturnMatchesSpec
  /\ TriggerSetsGossipFlag
  /\ TriggerReturnsTrueOnlyFirstTime

CollectorPlanRetryGossipExactness ==
  /\ CollectorConstructorStateExact
  /\ CollectorCursorReadExact
  /\ CollectorCursorAdvanceExact
  /\ CollectorExhaustionExact
  /\ CollectorGossipFallbackExact

Safety ==
  /\ TargetLenPreserved
  /\ PlanSentMatchesConstructor
  /\ SentNeverExceedsTargets
  /\ NewAndDefaultPlansStartUnsent
  /\ WithSentCapsInputAtLength
  /\ SentCountMatchesPlan
  /\ PeekMatchesSpec
  /\ PeekDoesNotAdvance
  /\ NextMatchesSpec
  /\ NextReturnsCurrentTargetAndAdvances
  /\ NextAtExhaustionIsNoop
  /\ ExhaustedMatchesSpec
  /\ ExhaustedIffSentAtOrBeyondLength
  /\ TriggerReturnMatchesSpec
  /\ TriggerSetsGossipFlag
  /\ TriggerReturnsTrueOnlyFirstTime
  /\ CollectorPlanRetryGossipExactness

=============================================================================
====

---- MODULE SumeragiRbcOutboundFlushGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for `flush_rbc_outbound_chunks(...)`.

The helper drains queued RBC outbound chunk batches after INIT payload
rebroadcasts. This slice pins the observable scheduler contract: observers and
DA-disabled nodes do not flush; an empty queue clears the cursor; relay
backpressure stops all work; queue backpressure stops all work unless at least
one queued session is payload-backpressure-exempt; the per-tick chunk budget is
floored to one; the cursor rotates in key order and wraps; non-exempt sessions
are skipped under queue backpressure; zero-send entries are removed only when
dispatch has already removed their queue entry; successful sends consume budget,
advance the cursor to the sending key, and report progress.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

None == "none"
A == "a"
B == "b"
C == "c"

Keys == {A, B, C}
CursorValues == {None, A, B, C}

Observer == "observer"
DaDisabled == "da_disabled"
EmptyClearsCursor == "empty_clears_cursor"
RelayBackpressure == "relay_backpressure"
QueueNoExempt == "queue_no_exempt"
NoCursorBudgetOne == "no_cursor_budget_one"
NoCursorBudgetTwo == "no_cursor_budget_two"
CursorWrapBudgetTwo == "cursor_wrap_budget_two"
BudgetZeroFloored == "budget_zero_floored"
QueueSkipsNonExempt == "queue_skips_non_exempt"
QueueAllExemptBudgetTwo == "queue_all_exempt_budget_two"
ZeroSentRemoved == "zero_sent_removed"
ZeroSentRetained == "zero_sent_retained"
SendTwoConsumesBudget == "send_two_consumes_budget"
SkipThenSendKeepsCursorToSender == "skip_then_send_keeps_cursor_to_sender"
AllSkippedKeepsCursor == "all_skipped_keeps_cursor"

Cases == {
  Observer,
  DaDisabled,
  EmptyClearsCursor,
  RelayBackpressure,
  QueueNoExempt,
  NoCursorBudgetOne,
  NoCursorBudgetTwo,
  CursorWrapBudgetTwo,
  BudgetZeroFloored,
  QueueSkipsNonExempt,
  QueueAllExemptBudgetTwo,
  ZeroSentRemoved,
  ZeroSentRetained,
  SendTwoConsumesBudget,
  SkipThenSendKeepsCursorToSender,
  AllSkippedKeepsCursor
}

IsObserver(c) == c = Observer
DaEnabled(c) == c /= DaDisabled
HasOutbound(c) == c /= EmptyClearsCursor
RelayPressure(c) == c = RelayBackpressure
QueuePressure(c) ==
  c \in {
    QueueNoExempt,
    QueueSkipsNonExempt,
    QueueAllExemptBudgetTwo,
    AllSkippedKeepsCursor
  }

ConfigBudget(c) ==
  CASE c = BudgetZeroFloored -> 0
    [] c \in {
      NoCursorBudgetTwo,
      CursorWrapBudgetTwo,
      QueueAllExemptBudgetTwo,
      SendTwoConsumesBudget
    } -> 2
    [] OTHER -> 1

Budget(c) == IF ConfigBudget(c) = 0 THEN 1 ELSE ConfigBudget(c)

InitialCursor(c) ==
  CASE c \in {
      CursorWrapBudgetTwo,
      SkipThenSendKeepsCursorToSender,
      AllSkippedKeepsCursor
    } -> B
    [] OTHER -> None

\* @type: Str => Seq(Str);
Order(c) ==
  CASE c = CursorWrapBudgetTwo -> <<C, A, B>>
    [] c \in {
      SkipThenSendKeepsCursorToSender,
      AllSkippedKeepsCursor
    } -> <<C, A, B>>
    [] c = EmptyClearsCursor -> <<>>
    [] OTHER -> <<A, B, C>>

Exempt(c, key) ==
  CASE c = QueueSkipsNonExempt -> key = B
    [] c = QueueAllExemptBudgetTwo -> key \in {A, B, C}
    [] OTHER -> FALSE

SpecFlush(c) ==
  IF IsObserver(c) \/ ~DaEnabled(c) THEN
    <<FALSE, InitialCursor(c), <<>>, <<>>, Budget(c)>>
  ELSE IF ~HasOutbound(c) THEN
    <<FALSE, None, <<>>, <<>>, Budget(c)>>
  ELSE IF RelayPressure(c) THEN
    <<FALSE, InitialCursor(c), <<>>, <<>>, Budget(c)>>
  ELSE IF QueuePressure(c) /\ ~(\E key \in Keys: Exempt(c, key)) THEN
    <<FALSE, InitialCursor(c), <<>>, <<>>, Budget(c)>>
  ELSE
    CASE c = NoCursorBudgetOne -> <<TRUE, A, <<A>>, <<>>, 0>>
      [] c = NoCursorBudgetTwo -> <<TRUE, B, <<A, B>>, <<>>, 0>>
      [] c = CursorWrapBudgetTwo -> <<TRUE, A, <<C, A>>, <<>>, 0>>
      [] c = BudgetZeroFloored -> <<TRUE, A, <<A>>, <<>>, 0>>
      [] c = QueueSkipsNonExempt -> <<TRUE, B, <<B>>, <<>>, 0>>
      [] c = QueueAllExemptBudgetTwo -> <<TRUE, B, <<A, B>>, <<>>, 0>>
      [] c = ZeroSentRemoved -> <<TRUE, B, <<B>>, <<A>>, 0>>
      [] c = ZeroSentRetained -> <<TRUE, B, <<B>>, <<>>, 0>>
      [] c = SendTwoConsumesBudget -> <<TRUE, A, <<A>>, <<>>, 0>>
      [] c = SkipThenSendKeepsCursorToSender -> <<TRUE, C, <<C>>, <<>>, 0>>
      [] OTHER -> <<FALSE, InitialCursor(c), <<>>, <<>>, Budget(c)>>

ActualFlush(c) ==
  CASE Bug = "observer_flushes"
       /\ c = Observer ->
         <<TRUE, A, <<A>>, <<>>, 0>>
    [] Bug = "da_disabled_flushes"
       /\ c = DaDisabled ->
         <<TRUE, A, <<A>>, <<>>, 0>>
    [] Bug = "empty_keeps_cursor"
       /\ c = EmptyClearsCursor ->
         <<FALSE, B, <<>>, <<>>, Budget(c)>>
    [] Bug = "relay_backpressure_flushes"
       /\ c = RelayBackpressure ->
         <<TRUE, A, <<A>>, <<>>, 0>>
    [] Bug = "queue_no_exempt_flushes"
       /\ c = QueueNoExempt ->
         <<TRUE, A, <<A>>, <<>>, 0>>
    [] Bug = "budget_zero_sends_none"
       /\ c = BudgetZeroFloored ->
         <<FALSE, None, <<>>, <<>>, 0>>
    [] Bug = "cursor_order_not_wrapped"
       /\ c = CursorWrapBudgetTwo ->
         <<TRUE, A, <<A, B>>, <<>>, 0>>
    [] Bug = "queue_sends_non_exempt"
       /\ c = QueueSkipsNonExempt ->
         <<TRUE, A, <<A>>, <<>>, 0>>
    [] Bug = "zero_sent_removed_kept"
       /\ c = ZeroSentRemoved ->
         <<FALSE, None, <<>>, <<>>, 1>>
    [] Bug = "zero_sent_retained_removed"
       /\ c = ZeroSentRetained ->
         <<FALSE, None, <<>>, <<A>>, 1>>
    [] Bug = "send_two_does_not_consume_budget"
       /\ c = SendTwoConsumesBudget ->
         <<TRUE, B, <<A, B>>, <<>>, 0>>
    [] Bug = "cursor_not_updated_to_sender"
       /\ c = SkipThenSendKeepsCursorToSender ->
         <<TRUE, B, <<C>>, <<>>, 0>>
    [] Bug = "all_skipped_clears_cursor"
       /\ c = AllSkippedKeepsCursor ->
         <<FALSE, None, <<>>, <<>>, 1>>
    [] OTHER -> SpecFlush(c)

BugSet == {
  "none",
  "observer_flushes",
  "da_disabled_flushes",
  "empty_keeps_cursor",
  "relay_backpressure_flushes",
  "queue_no_exempt_flushes",
  "budget_zero_sends_none",
  "cursor_order_not_wrapped",
  "queue_sends_non_exempt",
  "zero_sent_removed_kept",
  "zero_sent_retained_removed",
  "send_two_does_not_consume_budget",
  "cursor_not_updated_to_sender",
  "all_skipped_clears_cursor"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 13
     /\ checked' = checked + 1
  \/ /\ checked = 13
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..13
  /\ \A c \in Cases:
       /\ ActualFlush(c)[1] \in BOOLEAN
       /\ ActualFlush(c)[2] \in CursorValues
       /\ Len(ActualFlush(c)[3]) <= 3
       /\ \A i \in 1..Len(ActualFlush(c)[3]): ActualFlush(c)[3][i] \in Keys
       /\ Len(ActualFlush(c)[4]) <= 3
       /\ \A i \in 1..Len(ActualFlush(c)[4]): ActualFlush(c)[4][i] \in Keys
       /\ ActualFlush(c)[5] \in Nat

FlushExact ==
  \A c \in Cases:
    ActualFlush(c) = SpecFlush(c)

EntryGatesStable ==
  /\ ~ActualFlush(Observer)[1]
  /\ ~ActualFlush(DaDisabled)[1]
  /\ ActualFlush(EmptyClearsCursor)[2] = None
  /\ ~ActualFlush(RelayBackpressure)[1]
  /\ ~ActualFlush(QueueNoExempt)[1]

OrderAndBudgetStable ==
  /\ ActualFlush(NoCursorBudgetOne)[3] = <<A>>
  /\ ActualFlush(NoCursorBudgetOne)[2] = A
  /\ ActualFlush(NoCursorBudgetTwo)[3] = <<A, B>>
  /\ ActualFlush(CursorWrapBudgetTwo)[3] = <<C, A>>
  /\ ActualFlush(CursorWrapBudgetTwo)[2] = A
  /\ ActualFlush(BudgetZeroFloored)[3] = <<A>>
  /\ ActualFlush(SendTwoConsumesBudget)[3] = <<A>>
  /\ ActualFlush(SendTwoConsumesBudget)[5] = 0

BackpressureAndCleanupStable ==
  /\ ActualFlush(QueueSkipsNonExempt)[3] = <<B>>
  /\ ActualFlush(QueueAllExemptBudgetTwo)[3] = <<A, B>>
  /\ ActualFlush(ZeroSentRemoved)[4] = <<A>>
  /\ ActualFlush(ZeroSentRetained)[4] = <<>>
  /\ ActualFlush(SkipThenSendKeepsCursorToSender)[2] = C
  /\ ActualFlush(AllSkippedKeepsCursor)[2] = B
  /\ ~ActualFlush(AllSkippedKeepsCursor)[1]

RbcOutboundFlushCoreSafety ==
  /\ FlushExact
  /\ EntryGatesStable
  /\ OrderAndBudgetStable
  /\ BackpressureAndCleanupStable

SafetyFast ==
  RbcOutboundFlushCoreSafety

AllFlushCasesMatchSpec ==
  \A c \in Cases:
    ActualFlush(c) = SpecFlush(c)

EntryGateAnchors ==
  /\ ActualFlush(Observer) = <<FALSE, None, <<>>, <<>>, 1>>
  /\ ActualFlush(DaDisabled) = <<FALSE, None, <<>>, <<>>, 1>>
  /\ ActualFlush(EmptyClearsCursor) = <<FALSE, None, <<>>, <<>>, 1>>
  /\ ActualFlush(RelayBackpressure) = <<FALSE, None, <<>>, <<>>, 1>>
  /\ ActualFlush(QueueNoExempt) = <<FALSE, None, <<>>, <<>>, 1>>

BudgetOrderAnchors ==
  /\ ActualFlush(NoCursorBudgetOne) = <<TRUE, A, <<A>>, <<>>, 0>>
  /\ ActualFlush(NoCursorBudgetTwo) = <<TRUE, B, <<A, B>>, <<>>, 0>>
  /\ ActualFlush(CursorWrapBudgetTwo) = <<TRUE, A, <<C, A>>, <<>>, 0>>
  /\ ActualFlush(BudgetZeroFloored) = <<TRUE, A, <<A>>, <<>>, 0>>

BackpressureExemptAnchors ==
  /\ ActualFlush(QueueSkipsNonExempt) = <<TRUE, B, <<B>>, <<>>, 0>>
  /\ ActualFlush(QueueAllExemptBudgetTwo) = <<TRUE, B, <<A, B>>, <<>>, 0>>

CleanupAnchors ==
  /\ ActualFlush(ZeroSentRemoved) = <<TRUE, B, <<B>>, <<A>>, 0>>
  /\ ActualFlush(ZeroSentRetained) = <<TRUE, B, <<B>>, <<>>, 0>>

CursorProgressAnchors ==
  /\ ActualFlush(SendTwoConsumesBudget) = <<TRUE, A, <<A>>, <<>>, 0>>
  /\ ActualFlush(SkipThenSendKeepsCursorToSender) =
       <<TRUE, C, <<C>>, <<>>, 0>>
  /\ ActualFlush(AllSkippedKeepsCursor) = <<FALSE, B, <<>>, <<>>, 1>>

SafetyAnchors ==
  /\ AllFlushCasesMatchSpec
  /\ EntryGateAnchors
  /\ BudgetOrderAnchors
  /\ BackpressureExemptAnchors
  /\ CleanupAnchors
  /\ CursorProgressAnchors

RbcOutboundFlushExactness ==
  /\ RbcOutboundFlushCoreSafety
  /\ SafetyAnchors

RbcOutboundFlushCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcOutboundFlushExactness

====

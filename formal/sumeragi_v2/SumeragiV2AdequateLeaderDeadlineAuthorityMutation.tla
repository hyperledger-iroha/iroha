---- MODULE SumeragiV2AdequateLeaderDeadlineAuthorityMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Bounded mutation witness for pure deadline-receipt authority.

Both the real receipt and a fabricated later receipt have the expected pure
record shape.  Only the real receipt is bounded by the frozen roster's actual
timeout deadline.  At that deadline the corridor may exit: the real receipt
has expired, but the fabricated receipt still looks active if the roster
deadline conjunct is omitted.  The repaired predicate protects only receipts
which own the source-derived roster window.
***************************************************************************)

CONSTANT EnforceRosterDeadlineAuthority

ASSUME EnforceRosterDeadlineAuthority \in BOOLEAN

RosterDeadline == 3
RealReceiptDeadline == 3
FabricatedReceiptDeadline == 7
ReceiptDeadlines == {RealReceiptDeadline, FabricatedReceiptDeadline}

VARIABLES now, corridor, decided

vars == <<now, corridor, decided>>

TypeInvariant ==
  /\ now \in 0..RosterDeadline
  /\ corridor \in BOOLEAN
  /\ decided \in BOOLEAN

ReceiptOwnsFrozenRosterWindow(deadline) ==
  IF EnforceRosterDeadlineAuthority
  THEN deadline <= RosterDeadline
  ELSE TRUE

NoPrematureExit ==
  \A deadline \in ReceiptDeadlines:
    /\ ReceiptOwnsFrozenRosterWindow(deadline)
    /\ now < deadline
    /\ ~decided
      => corridor

Init ==
  /\ now = 0
  /\ corridor
  /\ ~decided

TickWithinWindow ==
  /\ corridor
  /\ now < RosterDeadline
  /\ now' = now + 1
  /\ corridor' = (now' < RosterDeadline)
  /\ UNCHANGED decided

Next == TickWithinWindow

Spec ==
  /\ Init
  /\ [][Next]_vars

=============================================================================

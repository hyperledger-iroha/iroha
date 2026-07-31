---- MODULE SumeragiV2ServeNonceMutation ----
EXTENDS Naturals, Sequences, FiniteSets, TLC

(***************************************************************************
Minimal mutation for occurrence-owned Serve FIFO progress.  Reusing a live
nonce permits an equal job value to be appended behind the occurrence being
serviced.  Weak fairness of the FIFO worker then admits a two-state lasso in
which that value never leaves scheduler ownership.  A fresh live nonce makes
the successor job distinguishable, so servicing the original occurrence
strictly exits its rank obligation.
***************************************************************************)

VARIABLE queue

vars == <<queue>>

Candidate == [identity |-> 0]

TargetJob == [class |-> "Serve", candidate |-> Candidate, nonce |-> 0]

FreshJob == [class |-> "Serve", candidate |-> Candidate, nonce |-> 1]

TargetOwned == TargetJob \in {queue[index]: index \in 1..Len(queue)}

ServeNonces == {queue[index].nonce: index \in 1..Len(queue)}

LiveNonceOwnership == Cardinality(ServeNonces) = Len(queue)

CorrectBinderCoversRecord ==
  {candidate \in {Candidate}: candidate = Candidate} = {Candidate}

CorrectBinderHasRecordInstance ==
  \E candidate \in {Candidate}, position \in {1}:
    /\ candidate = TargetJob.candidate
    /\ position = 1

Init == queue = <<TargetJob>>

Refill(replacement) ==
  /\ Len(queue) = 1
  /\ queue' = Append(queue, replacement)

Service ==
  /\ Len(queue) > 0
  /\ queue' = Tail(queue)

OldNext == Refill(TargetJob) \/ Service

FreshNext == (TargetOwned /\ Refill(FreshJob)) \/ Service

OldSpec == Init /\ [][OldNext]_vars /\ WF_vars(Service)

FreshSpec == Init /\ [][FreshNext]_vars /\ WF_vars(Service)

TargetEventuallyLeaves == TargetOwned ~> ~TargetOwned

=============================================================================

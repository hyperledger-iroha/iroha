---- MODULE SumeragiV2CrashReplayMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Bounded mutation witness for responsive crash/restart reconstruction.

The durable source and authenticated crash authority survive while every
volatile owner is cleared.  Authenticated restart first increments the
consumer generation, then a separately fair replay transition reconstructs
one pending signature, body, or application work owner.  Drop mutants lose
that work; the stale mutant accepts a completion from the pre-crash
generation.  This compact seam deliberately does not encode an exclusive
RestartReplay priority: production recovery composes multiple durable intents,
and the main asynchronous model must be source-bound to that composition.
***************************************************************************)

CONSTANTS Mode, PendingKind

NoAuthority == [node |-> 0, generation |-> 0, kind |-> "None"]

Authority(generation, kind) ==
  [node |-> 0, generation |-> generation, kind |-> kind]

Candidate(generation, kind) ==
  [consumerContext |-> "height-7/epoch-2",
   consumerView |-> 4,
   consumerGeneration |-> generation,
   evidence |-> [kind |-> kind, durable |-> TRUE],
   work |-> kind,
   body |-> IF kind = "Body" THEN "body-7" ELSE "body-none",
   manifest |-> "manifest-7",
   commitment |-> "execution-7"]

VARIABLES
  phase,
  up,
  generation,
  durableKind,
  authority,
  queue,
  completed,
  staleCompletion

vars ==
  <<phase, up, generation, durableKind, authority, queue,
    completed, staleCompletion>>

Init ==
  /\ phase = "Running"
  /\ up = TRUE
  /\ generation = 0
  /\ durableKind = PendingKind
  /\ authority = NoAuthority
  /\ queue = <<Candidate(0, PendingKind)>>
  /\ completed = FALSE
  /\ staleCompletion = FALSE

Crash ==
  /\ phase = "Running"
  /\ up
  /\ phase' = "RestartRequired"
  /\ up' = FALSE
  /\ authority' = Authority(generation, durableKind)
  /\ queue' = <<>>
  /\ UNCHANGED <<generation, durableKind, completed, staleCompletion>>

AuthenticatedRestart ==
  /\ phase = "RestartRequired"
  /\ ~up
  /\ authority = Authority(generation, durableKind)
  /\ phase' = "ReplayRequired"
  /\ up' = TRUE
  /\ generation' = generation + 1
  /\ authority' = Authority(generation + 1, durableKind)
  /\ queue' = <<>>
  /\ UNCHANGED <<durableKind, completed, staleCompletion>>

RepairedReplay ==
  /\ phase = "ReplayRequired"
  /\ up
  /\ authority = Authority(generation, durableKind)
  /\ phase' = "Recovered"
  /\ queue' = <<Candidate(generation, durableKind)>>
  /\ UNCHANGED <<up, generation, durableKind, authority,
                  completed, staleCompletion>>

DropReplay ==
  /\ phase = "ReplayRequired"
  /\ up
  /\ authority = Authority(generation, durableKind)
  /\ phase' = "Recovered"
  /\ queue' = <<>>
  /\ UNCHANGED <<up, generation, durableKind, authority,
                  completed, staleCompletion>>

StaleReplay ==
  /\ phase = "ReplayRequired"
  /\ up
  /\ authority = Authority(generation, durableKind)
  /\ phase' = "Recovered"
  /\ queue' = <<Candidate(generation - 1, durableKind)>>
  /\ UNCHANGED <<up, generation, durableKind, authority,
                  completed, staleCompletion>>

DispatchCurrent ==
  /\ phase = "Recovered"
  /\ up
  /\ queue # <<>>
  /\ Head(queue).consumerGeneration = generation
  /\ Head(queue).work = durableKind
  /\ queue' = Tail(queue)
  /\ completed' = TRUE
  /\ UNCHANGED <<phase, up, generation, durableKind, authority,
                 staleCompletion>>

DispatchStale ==
  /\ phase = "Recovered"
  /\ up
  /\ queue # <<>>
  /\ Head(queue).consumerGeneration # generation
  /\ queue' = Tail(queue)
  /\ completed' = TRUE
  /\ staleCompletion' = TRUE
  /\ UNCHANGED <<phase, up, generation, durableKind, authority>>

SelectedReplay ==
  CASE Mode = "Repaired" -> RepairedReplay
    [] Mode = "Drop" -> DropReplay
    [] OTHER -> StaleReplay

SelectedDispatch ==
  IF Mode = "Stale" THEN DispatchStale ELSE DispatchCurrent

Done == completed /\ UNCHANGED vars

Next ==
  Crash \/ AuthenticatedRestart \/ SelectedReplay \/ SelectedDispatch \/ Done

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Crash)
  /\ WF_vars(AuthenticatedRestart)
  /\ WF_vars(SelectedReplay)
  /\ WF_vars(SelectedDispatch)

RecoveryCompletes == <>completed

DurableWorkHasReplayOrRecovery ==
  completed
    \/ phase = "Running"
    \/ phase = "RestartRequired"
    \/ phase = "ReplayRequired"
    \/ queue # <<>>

(***************************************************************************
The pre-fix witness observes only the volatile signature carrier.  It fails on
the Crash step even in Repaired mode.  The crash-aware witness accepts only an
exact authority for the durable kind and current generation, and therefore
still rejects DropReplay after recovery authority is retired.
***************************************************************************)

VolatileSignatureProgressWitness ==
  durableKind # "Signature" \/ completed \/ queue # <<>>

CrashAwareSignatureProgressWitness ==
  \/ VolatileSignatureProgressWitness
  \/ /\ durableKind = "Signature"
     /\ phase \in {"RestartRequired", "ReplayRequired"}
     /\ authority = Authority(generation, durableKind)

NoStaleCompletion == ~staleCompletion

ExactRestartAuthority ==
  phase \in {"RestartRequired", "ReplayRequired"} =>
    authority = Authority(generation, durableKind)

AsyncRecoveryTypeInvariant ==
  /\ phase \in {"Running", "RestartRequired", "ReplayRequired", "Recovered"}
  /\ up \in BOOLEAN
  /\ generation \in Nat
  /\ durableKind \in {"Signature", "Body", "Application"}
  /\ authority.node = 0
  /\ authority.generation \in Nat
  /\ authority.kind \in
       {"None", "Signature", "Body", "Application"}
  /\ (phase = "Running" => up)
  /\ (phase = "RestartRequired" => ~up)
  /\ (phase \in {"ReplayRequired", "Recovered"} => up)

AsyncRestartAuthorityInvariant == ExactRestartAuthority

====

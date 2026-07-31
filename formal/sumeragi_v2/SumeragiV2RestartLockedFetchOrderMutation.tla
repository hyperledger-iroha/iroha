---- MODULE SumeragiV2RestartLockedFetchOrderMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Compact mutation kernel for restart reconstruction when one node owns both
an exact locked-Prepare body recovery and a durable signature.

The repaired replay contains both exact owners and places FetchBody before
the first Sign.  The DropFetch mutant loses the body-recovery owner, while
the Reverse mutant retains both owners but exposes Sign before FetchBody.
***************************************************************************)

CONSTANT Mode

RestartNode == "validator-0"
RestartContext == [height |-> 7, epoch |-> 2]
RestartView == 4
RestartGeneration == 3
LockedSubject == "block-7"

LockedPrepareEvidence ==
  [kind |-> "PrepareQC",
   context |-> RestartContext,
   view |-> RestartView,
   subject |-> LockedSubject]

DurableSignatureEvidence ==
  [kind |-> "CommitIntent",
   context |-> RestartContext,
   view |-> RestartView,
   subject |-> LockedSubject]

RestartOwner(ownerKind) ==
  [node |-> RestartNode,
   context |-> RestartContext,
   view |-> RestartView,
   generation |-> RestartGeneration,
   kind |-> ownerKind]

RestartRequest(requestKind, ownerKind, evidence) ==
  [node |-> RestartNode,
   context |-> RestartContext,
   view |-> RestartView,
   generation |-> RestartGeneration,
   subject |-> LockedSubject,
   kind |-> requestKind,
   owner |-> RestartOwner(ownerKind),
   evidence |-> evidence]

LockedFetchRequest ==
  RestartRequest(
    "FetchBody",
    "LockedPrepareBodyRecovery",
    LockedPrepareEvidence)

DurableSignRequest ==
  RestartRequest(
    "Sign",
    "DurableCommitSignature",
    DurableSignatureEvidence)

VARIABLES phase, replay

vars == <<phase, replay>>

SequenceSet(sequence) ==
  {sequence[index]: index \in 1..Len(sequence)}

RequestsUniqueByOwner(sequence) ==
  \A left, right \in 1..Len(sequence):
    sequence[left].owner = sequence[right].owner => left = right

Init ==
  /\ phase = "ReplayRequired"
  /\ replay = <<>>

SelectedRestartReplay ==
  /\ phase = "ReplayRequired"
  /\ phase' = "Replaying"
  /\ replay' =
       CASE Mode = "Repaired" ->
              <<LockedFetchRequest, DurableSignRequest>>
         [] Mode = "DropFetch" ->
              <<DurableSignRequest>>
         [] OTHER ->
              <<DurableSignRequest, LockedFetchRequest>>

Next == SelectedRestartReplay

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SelectedRestartReplay)

TypeInvariant ==
  /\ Mode \in {"Repaired", "DropFetch", "Reverse"}
  /\ phase \in {"ReplayRequired", "Replaying"}
  /\ replay \in Seq({LockedFetchRequest, DurableSignRequest})

RestartReplayHasExactOwnership ==
  phase = "Replaying" =>
    /\ Len(replay) = 2
    /\ SequenceSet(replay) =
         {LockedFetchRequest, DurableSignRequest}
    /\ RequestsUniqueByOwner(replay)

IsFirstSignAt(sequence, index) ==
  /\ index \in 1..Len(sequence)
  /\ sequence[index].kind = "Sign"
  /\ \A earlier \in 1..(index - 1):
       sequence[earlier].kind # "Sign"

LockedFetchPrecedesFirstSign ==
  phase = "Replaying" =>
    \A signIndex \in 1..Len(replay):
      IsFirstSignAt(replay, signIndex) =>
        \E fetchIndex \in 1..Len(replay):
          /\ fetchIndex < signIndex
          /\ replay[fetchIndex] = LockedFetchRequest

ReplayReconstructionCompletes ==
  <> (phase = "Replaying")

=============================================================================

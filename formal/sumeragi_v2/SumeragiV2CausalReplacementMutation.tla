---- MODULE SumeragiV2CausalReplacementMutation ----
EXTENDS Naturals

(***************************************************************************
An exact abstraction of the blind causal-successor append in
SumeragiV2AsyncNetwork.  `stage` is the sole downstream owner of one stale
FetchBody value.  A replayed DeliverChunk parent may add an equal causal copy
while that downstream owner remains live.  The old relation cycles through
runtime -> causal -> I/O -> ready -> runtime without crossing unowned or a
rank below <<3, 4>>.  The fixed relation consumes the parent but coalesces its
equal successor against scheduler-wide ownership.
***************************************************************************)

VARIABLES stage, causalCopy, parentReady, cursor

vars == <<stage, causalCopy, parentReady, cursor>>

Stages == {"None", "Runtime", "Io", "Ready"}
Cursors == {"Progress", "Normal"}

TypeInvariant ==
  /\ stage \in Stages
  /\ causalCopy \in BOOLEAN
  /\ parentReady \in BOOLEAN
  /\ cursor \in Cursors

CandidateOwned == stage # "None" \/ causalCopy

CandidateRank ==
  CASE stage = "Runtime" ->
         IF cursor = "Normal" THEN <<3, 4>> ELSE <<3, 5>>
    [] stage = "Ready" -> <<4, 1>>
    [] stage = "Io" -> <<5, 1>>
    [] causalCopy -> <<6, 2>>
    [] OTHER -> <<0, 0>>

ServiceRankLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
        /\ left[2] < right[2]

Init ==
  /\ stage = "Runtime"
  /\ causalCopy = FALSE
  /\ parentReady = TRUE
  /\ cursor = "Progress"

RetransmitChunkParent ==
  /\ ~parentReady
  /\ parentReady' = TRUE
  /\ UNCHANGED <<stage, causalCopy, cursor>>

BlindExecuteChunkParent ==
  /\ parentReady
  /\ stage = "Runtime"
  /\ cursor = "Progress"
  /\ causalCopy' = TRUE
  /\ parentReady' = FALSE
  /\ cursor' = "Normal"
  /\ UNCHANGED stage

CoalescedExecuteChunkParent ==
  /\ parentReady
  /\ stage = "Runtime"
  /\ cursor = "Progress"
  /\ causalCopy' = IF CandidateOwned THEN causalCopy ELSE TRUE
  /\ parentReady' = FALSE
  /\ cursor' = "Normal"
  /\ UNCHANGED stage

DispatchStaleFetch ==
  /\ stage = "Runtime"
  /\ cursor = "Normal"
  /\ stage' = "None"
  /\ cursor' = "Progress"
  /\ UNCHANGED <<causalCopy, parentReady>>

AdmitCausalFetch ==
  /\ stage = "None"
  /\ causalCopy
  /\ stage' = "Io"
  /\ causalCopy' = FALSE
  /\ UNCHANGED <<parentReady, cursor>>

ServiceIoFetch ==
  /\ stage = "Io"
  /\ stage' = "Ready"
  /\ UNCHANGED <<causalCopy, parentReady, cursor>>

AdmitReadyFetch ==
  /\ stage = "Ready"
  /\ stage' = "Runtime"
  /\ UNCHANGED <<causalCopy, parentReady, cursor>>

OldNext ==
  RetransmitChunkParent \/ BlindExecuteChunkParent \/ DispatchStaleFetch
    \/ AdmitCausalFetch \/ ServiceIoFetch \/ AdmitReadyFetch

OldSpec ==
  Init
    /\ [][OldNext]_vars
    /\ WF_vars(RetransmitChunkParent)
    /\ WF_vars(BlindExecuteChunkParent)
    /\ WF_vars(DispatchStaleFetch)
    /\ WF_vars(AdmitCausalFetch)
    /\ WF_vars(ServiceIoFetch)
    /\ WF_vars(AdmitReadyFetch)

CoalescedNext ==
  RetransmitChunkParent \/ CoalescedExecuteChunkParent \/ DispatchStaleFetch
    \/ AdmitCausalFetch \/ ServiceIoFetch \/ AdmitReadyFetch

CoalescedSpec ==
  Init
    /\ [][CoalescedNext]_vars
    /\ WF_vars(RetransmitChunkParent)
    /\ WF_vars(CoalescedExecuteChunkParent)
    /\ WF_vars(DispatchStaleFetch)
    /\ WF_vars(AdmitCausalFetch)
    /\ WF_vars(ServiceIoFetch)
    /\ WF_vars(AdmitReadyFetch)

RankProgress ==
  (CandidateOwned /\ CandidateRank = <<3, 4>>)
    ~> (~CandidateOwned
         \/ ServiceRankLess(CandidateRank, <<3, 4>>))

=============================================================================

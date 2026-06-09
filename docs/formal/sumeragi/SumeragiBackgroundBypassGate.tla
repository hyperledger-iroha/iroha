---- MODULE SumeragiBackgroundBypassGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for background scheduler bypass policy.

This slice captures the branch table in `schedule_background(...)` and the
queue-forcing behavior of `schedule_background_via_queue(...)`, after
`prepare_background_block_message(...)` has accepted the request. It pins which
prepared consensus block messages bypass the background worker immediately,
which request kinds must still enter the worker queue, and the stronger
disabled-worker rule that dispatches every accepted request inline.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Inline == 1
Fallback == 2
Queue == 3
Actions == {Inline, Fallback, Queue}

Post == 1
Broadcast == 2
PostControlFlow == 3
BroadcastControlFlow == 4
PostNativeAmx == 5
BroadcastNativeAmx == 6
RequestKinds == {
  Post,
  Broadcast,
  PostControlFlow,
  BroadcastControlFlow,
  PostNativeAmx,
  BroadcastNativeAmx
}

NoneMsg == 0
Proposal == 1
ProposalHint == 2
BlockCreated == 3
FetchBlockBody == 4
BlockBodyResponse == 5
Qc == 6
QcVote == 7
RbcInitRequest == 8
RbcChunkRequest == 9
RbcInit == 10
RbcChunk == 11
RbcChunkCompact == 12
RbcReady == 13
RbcDeliver == 14
ConsensusParams == 15

PostBypassMessages == {
  Proposal,
  ProposalHint,
  BlockCreated,
  FetchBlockBody,
  BlockBodyResponse,
  Qc,
  QcVote,
  RbcInitRequest,
  RbcChunkRequest,
  RbcInit,
  RbcChunk,
  RbcChunkCompact,
  RbcReady,
  RbcDeliver
}

BroadcastBypassMessages == {
  Proposal,
  ProposalHint,
  BlockCreated,
  BlockBodyResponse,
  RbcInit,
  RbcChunk,
  RbcChunkCompact,
  RbcReady,
  RbcDeliver
}

\* Cases 1..34 are normal `schedule_background` requests, 35..36 are
\* `schedule_background_via_queue`, and 37..41 have the worker disabled.
Cases == 1..41

RequestKind(c) ==
  CASE c \in (1..15) \cup {35, 37, 38} -> Post
    [] c \in (16..30) \cup {36, 39} -> Broadcast
    [] c \in {31, 40} -> PostControlFlow
    [] c = 32 -> BroadcastControlFlow
    [] c \in {33, 41} -> PostNativeAmx
    [] OTHER -> BroadcastNativeAmx

MessageKind(c) ==
  CASE c \in {1, 16} -> Proposal
    [] c \in {2, 17} -> ProposalHint
    [] c \in {3, 18, 36} -> BlockCreated
    [] c \in {4, 25} -> FetchBlockBody
    [] c \in {5, 19} -> BlockBodyResponse
    [] c \in {6, 26} -> Qc
    [] c \in {7, 27, 35, 38, 39} -> QcVote
    [] c \in {8, 28} -> RbcInitRequest
    [] c \in {9, 29} -> RbcChunkRequest
    [] c \in {10, 20} -> RbcInit
    [] c \in {11, 21} -> RbcChunk
    [] c \in {12, 22} -> RbcChunkCompact
    [] c \in {13, 23} -> RbcReady
    [] c \in {14, 24} -> RbcDeliver
    [] c \in {15, 30, 37} -> ConsensusParams
    [] OTHER -> NoneMsg

ViaQueue(c) ==
  c \in {35, 36}

WorkerDisabled(c) ==
  c \in {37, 38, 39, 40, 41}

SpecBypass(c) ==
  CASE RequestKind(c) = Post ->
      MessageKind(c) \in PostBypassMessages
    [] RequestKind(c) = Broadcast ->
      MessageKind(c) \in BroadcastBypassMessages
    [] OTHER -> FALSE

SpecAction(c) ==
  IF WorkerDisabled(c) THEN
    Inline
  ELSE IF ViaQueue(c) THEN
    Queue
  ELSE IF SpecBypass(c) THEN
    Fallback
  ELSE
    Queue

ActualBypass(c) ==
  CASE Bug = "post_proposal_queued"
       /\ RequestKind(c) = Post
       /\ MessageKind(c) = Proposal -> FALSE
    [] Bug = "post_qc_vote_queued"
       /\ RequestKind(c) = Post
       /\ MessageKind(c) = QcVote -> FALSE
    [] Bug = "post_rbc_chunk_queued"
       /\ RequestKind(c) = Post
       /\ MessageKind(c) \in {RbcChunk, RbcChunkCompact} -> FALSE
    [] Bug = "post_fetch_block_body_queued"
       /\ RequestKind(c) = Post
       /\ MessageKind(c) = FetchBlockBody -> FALSE
    [] Bug = "broadcast_proposal_queued"
       /\ RequestKind(c) = Broadcast
       /\ MessageKind(c) = Proposal -> FALSE
    [] Bug = "broadcast_block_body_response_queued"
       /\ RequestKind(c) = Broadcast
       /\ MessageKind(c) = BlockBodyResponse -> FALSE
    [] Bug = "broadcast_rbc_ready_queued"
       /\ RequestKind(c) = Broadcast
       /\ MessageKind(c) = RbcReady -> FALSE
    [] Bug = "broadcast_qc_vote_bypasses"
       /\ RequestKind(c) = Broadcast
       /\ MessageKind(c) = QcVote -> TRUE
    [] Bug = "broadcast_fetch_request_bypasses"
       /\ RequestKind(c) = Broadcast
       /\ MessageKind(c) \in {
            FetchBlockBody,
            RbcInitRequest,
            RbcChunkRequest
          } -> TRUE
    [] Bug = "control_flow_bypasses"
       /\ RequestKind(c) \in {
            PostControlFlow,
            BroadcastControlFlow
          } -> TRUE
    [] Bug = "native_amx_bypasses"
       /\ RequestKind(c) \in {
            PostNativeAmx,
            BroadcastNativeAmx
          } -> TRUE
    [] OTHER -> SpecBypass(c)

ActualAction(c) ==
  IF WorkerDisabled(c) THEN
    IF Bug = "worker_disabled_queues" THEN Queue ELSE Inline
  ELSE IF ViaQueue(c) THEN
    IF Bug = "via_queue_bypasses" /\ ActualBypass(c) THEN Fallback ELSE Queue
  ELSE IF ActualBypass(c) THEN
    Fallback
  ELSE
    Queue

Matches(c) ==
  /\ ActualBypass(c) = SpecBypass(c)
  /\ ActualAction(c) = SpecAction(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "post_proposal_queued",
       "post_qc_vote_queued",
       "post_rbc_chunk_queued",
       "post_fetch_block_body_queued",
       "broadcast_proposal_queued",
       "broadcast_block_body_response_queued",
       "broadcast_rbc_ready_queued",
       "broadcast_qc_vote_bypasses",
       "broadcast_fetch_request_bypasses",
       "control_flow_bypasses",
       "native_amx_bypasses",
       "worker_disabled_queues",
       "via_queue_bypasses"
     }
  /\ checked = 0
  /\ \A c \in Cases:
       /\ RequestKind(c) \in RequestKinds
       /\ MessageKind(c) \in PostBypassMessages \cup {
            ConsensusParams,
            NoneMsg
          }
       /\ SpecAction(c) \in Actions
       /\ ActualAction(c) \in Actions

BypassMatchesSpec ==
  \A c \in Cases: Matches(c)

SafetyFast == BypassMatchesSpec

PostBypassMatrix ==
  \A c \in Cases:
    RequestKind(c) = Post /\ MessageKind(c) \in PostBypassMessages
      => ActualAction(c) \in {Fallback, Inline}

BroadcastBypassMatrix ==
  \A c \in Cases:
    RequestKind(c) = Broadcast /\ MessageKind(c) \in BroadcastBypassMessages
      => ActualAction(c) \in {Fallback, Inline}

BroadcastConsensusVotesStayQueued ==
  \A c \in Cases:
    RequestKind(c) = Broadcast /\ MessageKind(c) \in {Qc, QcVote}
      => ActualAction(c) \in {Queue, Inline}

ControlAndNativeStayQueued ==
  \A c \in Cases:
    RequestKind(c) \in {
      PostControlFlow,
      BroadcastControlFlow,
      PostNativeAmx,
      BroadcastNativeAmx
    }
      => ActualAction(c) \in {Queue, Inline}

DisabledWorkerDispatchesInline ==
  \A c \in Cases:
    WorkerDisabled(c) => ActualAction(c) = Inline

ViaQueueNeverBypasses ==
  \A c \in Cases:
    ViaQueue(c) => ActualAction(c) = Queue

====

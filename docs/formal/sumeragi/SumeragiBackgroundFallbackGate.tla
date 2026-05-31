---- MODULE SumeragiBackgroundFallbackGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for background fallback dispatch.

This slice captures `dispatch_background_fallback(...)`: a returned or bypassed
`BackgroundRequest` must be converted into the matching P2P post/broadcast
operation, preserve peer targeting for posts, omit peers for broadcasts,
preserve the network payload class, and assign the correct priority. Block
messages use their embedded consensus priority; control-flow and native-AMX
messages are always high priority.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PostOp == 1
BroadcastOp == 2
Operations == {PostOp, BroadcastOp}

BlockPayload == 1
ControlPayload == 2
NativePayload == 3
Payloads == {BlockPayload, ControlPayload, NativePayload}

LowPriority == 1
HighPriority == 2
Priorities == {LowPriority, HighPriority}

PeerPreserved == 1
NoPeer == 2
PeerStates == {PeerPreserved, NoPeer}

\* 1 post block low, 2 post block high, 3 post control, 4 post native,
\* 5 broadcast block low, 6 broadcast block high, 7 broadcast control,
\* 8 broadcast native.
Cases == 1..8

SpecOperation(c) ==
  IF c \in 1..4 THEN PostOp ELSE BroadcastOp

SpecPayload(c) ==
  CASE c \in {1, 2, 5, 6} -> BlockPayload
    [] c \in {3, 7} -> ControlPayload
    [] OTHER -> NativePayload

InputBlockPriority(c) ==
  CASE c \in {1, 5} -> LowPriority
    [] c \in {2, 6} -> HighPriority
    [] OTHER -> HighPriority

SpecPriority(c) ==
  IF SpecPayload(c) = BlockPayload THEN
    InputBlockPriority(c)
  ELSE
    HighPriority

SpecPeerState(c) ==
  IF SpecOperation(c) = PostOp THEN PeerPreserved ELSE NoPeer

ActualOperation(c) ==
  CASE Bug = "post_block_broadcasts"
       /\ c \in {1, 2} -> BroadcastOp
    [] Bug = "broadcast_block_posts"
       /\ c \in {5, 6} -> PostOp
    [] Bug = "post_control_broadcasts"
       /\ c = 3 -> BroadcastOp
    [] Bug = "broadcast_native_posts"
       /\ c = 8 -> PostOp
    [] OTHER -> SpecOperation(c)

ActualPayload(c) ==
  CASE Bug = "block_payload_as_control"
       /\ SpecPayload(c) = BlockPayload -> ControlPayload
    [] Bug = "control_payload_as_block"
       /\ SpecPayload(c) = ControlPayload -> BlockPayload
    [] Bug = "native_payload_as_block"
       /\ SpecPayload(c) = NativePayload -> BlockPayload
    [] OTHER -> SpecPayload(c)

ActualPriority(c) ==
  CASE Bug = "block_priority_forced_high"
       /\ SpecPayload(c) = BlockPayload -> HighPriority
    [] Bug = "block_priority_forced_low"
       /\ SpecPayload(c) = BlockPayload -> LowPriority
    [] Bug = "control_priority_low"
       /\ SpecPayload(c) = ControlPayload -> LowPriority
    [] Bug = "native_priority_low"
       /\ SpecPayload(c) = NativePayload -> LowPriority
    [] OTHER -> SpecPriority(c)

ActualPeerState(c) ==
  CASE Bug = "post_peer_dropped"
       /\ SpecOperation(c) = PostOp -> NoPeer
    [] Bug = "broadcast_peer_added"
       /\ SpecOperation(c) = BroadcastOp -> PeerPreserved
    [] OTHER ->
       IF ActualOperation(c) = PostOp THEN PeerPreserved ELSE NoPeer

Matches(c) ==
  /\ ActualOperation(c) = SpecOperation(c)
  /\ ActualPayload(c) = SpecPayload(c)
  /\ ActualPriority(c) = SpecPriority(c)
  /\ ActualPeerState(c) = SpecPeerState(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "post_block_broadcasts",
       "broadcast_block_posts",
       "post_control_broadcasts",
       "broadcast_native_posts",
       "block_payload_as_control",
       "control_payload_as_block",
       "native_payload_as_block",
       "block_priority_forced_high",
       "block_priority_forced_low",
       "control_priority_low",
       "native_priority_low",
       "post_peer_dropped",
       "broadcast_peer_added"
     }
  /\ checked = 0
  /\ \A c \in Cases:
       /\ SpecOperation(c) \in Operations
       /\ ActualOperation(c) \in Operations
       /\ SpecPayload(c) \in Payloads
       /\ ActualPayload(c) \in Payloads
       /\ SpecPriority(c) \in Priorities
       /\ ActualPriority(c) \in Priorities
       /\ SpecPeerState(c) \in PeerStates
       /\ ActualPeerState(c) \in PeerStates

SafetyFast ==
  \A c \in Cases: Matches(c)

PostsStayPosts ==
  \A c \in Cases:
    SpecOperation(c) = PostOp => ActualOperation(c) = PostOp

BroadcastsStayBroadcasts ==
  \A c \in Cases:
    SpecOperation(c) = BroadcastOp => ActualOperation(c) = BroadcastOp

PayloadClassPreserved ==
  \A c \in Cases:
    ActualPayload(c) = SpecPayload(c)

BlockPriorityProjected ==
  \A c \in Cases:
    SpecPayload(c) = BlockPayload => ActualPriority(c) = InputBlockPriority(c)

ControlAndNativeHighPriority ==
  \A c \in Cases:
    SpecPayload(c) \in {ControlPayload, NativePayload}
      => ActualPriority(c) = HighPriority

PeerTargetingPreserved ==
  \A c \in Cases:
    ActualPeerState(c) = SpecPeerState(c)

====

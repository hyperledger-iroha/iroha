---- MODULE SumeragiBackgroundDispatchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for consensus background request dispatch.

This slice captures `background_request_allows_blocking(...)` and
`dispatch_background_request(...)`. It pins the non-droppable consensus
payload policy used by `schedule_background(...)`: every current
`BackgroundRequest` variant may fall back to caller-side inline sends when the
background queue is full, full queues record overflow but not drop status,
unattached or disconnected workers record drop status and still reconstruct the
request for fallback, ready queues enqueue exactly once, and kind labels stay
stable for telemetry/status accounting.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Kinds == {
  "Post",
  "PostControlFlow",
  "Broadcast",
  "BroadcastControlFlow",
  "PostNativeAmx",
  "BroadcastNativeAmx"
}

Cases == {
  "post_ready",
  "post_full",
  "post_no_worker",
  "post_disconnected",
  "post_control_ready",
  "post_control_full",
  "post_control_no_worker",
  "post_control_disconnected",
  "broadcast_ready",
  "broadcast_full",
  "broadcast_no_worker",
  "broadcast_disconnected",
  "broadcast_control_ready",
  "broadcast_control_full",
  "broadcast_control_no_worker",
  "broadcast_control_disconnected",
  "post_native_amx_ready",
  "post_native_amx_full",
  "post_native_amx_no_worker",
  "post_native_amx_disconnected",
  "broadcast_native_amx_ready",
  "broadcast_native_amx_full",
  "broadcast_native_amx_no_worker",
  "broadcast_native_amx_disconnected"
}

Kind(c) ==
  CASE c \in {
         "post_ready",
         "post_full",
         "post_no_worker",
         "post_disconnected"
       } -> "Post"
    [] c \in {
         "post_control_ready",
         "post_control_full",
         "post_control_no_worker",
         "post_control_disconnected"
       } -> "PostControlFlow"
    [] c \in {
         "broadcast_ready",
         "broadcast_full",
         "broadcast_no_worker",
         "broadcast_disconnected"
       } -> "Broadcast"
    [] c \in {
         "broadcast_control_ready",
         "broadcast_control_full",
         "broadcast_control_no_worker",
         "broadcast_control_disconnected"
       } -> "BroadcastControlFlow"
    [] c \in {
         "post_native_amx_ready",
         "post_native_amx_full",
         "post_native_amx_no_worker",
         "post_native_amx_disconnected"
       } -> "PostNativeAmx"
    [] OTHER -> "BroadcastNativeAmx"

Channel(c) ==
  CASE c \in {
         "post_ready",
         "post_control_ready",
         "broadcast_ready",
         "broadcast_control_ready",
         "post_native_amx_ready",
         "broadcast_native_amx_ready"
       } -> "ready"
    [] c \in {
         "post_full",
         "post_control_full",
         "broadcast_full",
         "broadcast_control_full",
         "post_native_amx_full",
         "broadcast_native_amx_full"
       } -> "full"
    [] c \in {
         "post_no_worker",
         "post_control_no_worker",
         "broadcast_no_worker",
         "broadcast_control_no_worker",
         "post_native_amx_no_worker",
         "broadcast_native_amx_no_worker"
       } -> "no_worker"
    [] OTHER -> "disconnected"

SpecAllowsBlocking(c) ==
  Kind(c) \in Kinds

SpecEnqueued(c) ==
  Channel(c) = "ready"

SpecReturned(c) ==
  Channel(c) \in {"full", "no_worker", "disconnected"}

SpecOverflowRecorded(c) ==
  Channel(c) = "full"

SpecDropRecorded(c) ==
  Channel(c) \in {"no_worker", "disconnected"}

SpecKindLabel(c) ==
  Kind(c)

SpecReturnedKind(c) ==
  IF SpecReturned(c) THEN Kind(c) ELSE "none"

ActualAllowsBlocking(c) ==
  CASE Bug = "drop_full_post"
       /\ Channel(c) = "full"
       /\ Kind(c) = "Post" -> FALSE
    [] Bug = "drop_full_broadcast"
       /\ Channel(c) = "full"
       /\ Kind(c) = "Broadcast" -> FALSE
    [] Bug = "drop_full_control_flow"
       /\ Channel(c) = "full"
       /\ Kind(c) \in {"PostControlFlow", "BroadcastControlFlow"} -> FALSE
    [] Bug = "drop_full_native_amx"
       /\ Channel(c) = "full"
       /\ Kind(c) \in {"PostNativeAmx", "BroadcastNativeAmx"} -> FALSE
    [] OTHER -> TRUE

ActualEnqueued(c) ==
  CASE Bug = "ready_returns_request"
       /\ Channel(c) = "ready" -> FALSE
    [] OTHER -> SpecEnqueued(c)

ActualReturned(c) ==
  CASE Bug = "ready_returns_request"
       /\ Channel(c) = "ready" -> TRUE
    [] OTHER -> SpecReturned(c)

ActualOverflowRecorded(c) ==
  SpecOverflowRecorded(c)

ActualDropRecorded(c) ==
  CASE Channel(c) = "full"
       /\ ~ActualAllowsBlocking(c) -> TRUE
    [] Bug = "no_worker_not_dropped"
       /\ Channel(c) = "no_worker" -> FALSE
    [] Bug = "disconnected_not_dropped"
       /\ Channel(c) = "disconnected" -> FALSE
    [] OTHER -> SpecDropRecorded(c)

ActualKindLabel(c) ==
  CASE Bug = "kind_label_mismatch"
       /\ Kind(c) = "BroadcastControlFlow" -> "Broadcast"
    [] OTHER -> SpecKindLabel(c)

ActualReturnedKind(c) ==
  CASE Bug = "reconstruct_wrong_kind"
       /\ SpecReturned(c)
       /\ Kind(c) = "PostNativeAmx" -> "Post"
    [] Bug = "ready_returns_request"
       /\ Channel(c) = "ready" -> Kind(c)
    [] OTHER -> SpecReturnedKind(c)

Matches(c) ==
  /\ ActualAllowsBlocking(c) = SpecAllowsBlocking(c)
  /\ ActualEnqueued(c) = SpecEnqueued(c)
  /\ ActualReturned(c) = SpecReturned(c)
  /\ ActualOverflowRecorded(c) = SpecOverflowRecorded(c)
  /\ ActualDropRecorded(c) = SpecDropRecorded(c)
  /\ ActualKindLabel(c) = SpecKindLabel(c)
  /\ ActualReturnedKind(c) = SpecReturnedKind(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "drop_full_post",
       "drop_full_broadcast",
       "drop_full_control_flow",
       "drop_full_native_amx",
       "ready_returns_request",
       "no_worker_not_dropped",
       "disconnected_not_dropped",
       "kind_label_mismatch",
       "reconstruct_wrong_kind"
     }
  /\ checked = 0
  /\ \A c \in Cases:
       /\ Kind(c) \in Kinds
       /\ Channel(c) \in {"ready", "full", "no_worker", "disconnected"}

SafetyFast ==
  \A c \in Cases: Matches(c)

AllRequestsBlockingEligible ==
  \A c \in Cases: ActualAllowsBlocking(c) = TRUE

ReadyQueueEnqueuesOnly ==
  \A c \in Cases:
    Channel(c) = "ready" =>
      /\ ActualEnqueued(c)
      /\ ~ActualReturned(c)
      /\ ~ActualDropRecorded(c)
      /\ ~ActualOverflowRecorded(c)

FullQueueReturnsWithoutDrop ==
  \A c \in Cases:
    Channel(c) = "full" =>
      /\ ~ActualEnqueued(c)
      /\ ActualReturned(c)
      /\ ActualOverflowRecorded(c)
      /\ ~ActualDropRecorded(c)

UnavailableWorkerRecordsDropAndReturns ==
  \A c \in Cases:
    Channel(c) \in {"no_worker", "disconnected"} =>
      /\ ~ActualEnqueued(c)
      /\ ActualReturned(c)
      /\ ActualDropRecorded(c)
      /\ ~ActualOverflowRecorded(c)

LabelsPreserved ==
  \A c \in Cases: ActualKindLabel(c) = Kind(c)

ReturnedRequestsPreserveKind ==
  \A c \in Cases: ActualReturnedKind(c) = SpecReturnedKind(c)

====

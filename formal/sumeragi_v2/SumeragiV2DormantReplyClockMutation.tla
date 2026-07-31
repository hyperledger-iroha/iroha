---- MODULE SumeragiV2DormantReplyClockMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite-state regression for exact-reply request clock ownership.

An authenticated exact request is retained before its responder can answer.
A timeout certificate may advance the requester to the next view while that
packet remains dormant.  In the repaired model, the Serve semantic gate—not
the later capacity or selector gates—decides whether the packet owns the
clock deadline.  Thus the dormant packet cannot freeze the next-view clock,
but the same due packet immediately owns the deadline when serviceability or
lifecycle ownership appears, even while a physical gate remains closed.

`AllDuePacketsOwnClock` is the old behavior: every due retained request owns
the clock before the responder can serve it.  The retained packet then
freezes time after the view advance.  This bounded model is mutation evidence
only and is not a substitute for the AsyncNetwork proof.
***************************************************************************)

CONSTANT MutationMode

MutationModes ==
  {"Fixed", "AllDuePacketsOwnClock"}

RequestDeadline == 1
NextViewTimeout == 3

VARIABLES
  asyncNow,
  nodeView,
  tcInstalled,
  requestRetained,
  requestServiceable,
  lifecycleOwned,
  capacityOpen,
  selectorOpen,
  served,
  lastTransition

dormantReplyClockVars ==
  <<asyncNow,
    nodeView,
    tcInstalled,
    requestRetained,
    requestServiceable,
    lifecycleOwned,
    capacityOpen,
    selectorOpen,
    served,
    lastTransition>>

AsyncServeTransportAdmissionGateAllows ==
  requestServiceable \/ lifecycleOwned

AsyncPhysicalAdmissionGateAllows ==
  capacityOpen /\ selectorOpen

AsyncDormantExactReplyRequestPacket ==
  /\ requestRetained
  /\ ~AsyncServeTransportAdmissionGateAllows

AsyncReplyRequestPacketDue ==
  /\ requestRetained
  /\ RequestDeadline <= asyncNow

AsyncPacketOwnsClockDeadline ==
  /\ AsyncReplyRequestPacketDue
  /\ IF MutationMode = "AllDuePacketsOwnClock"
        THEN TRUE
        ELSE ~AsyncDormantExactReplyRequestPacket

AsyncTickEnabled ==
  /\ asyncNow < NextViewTimeout
  /\ ~AsyncPacketOwnsClockDeadline

Init ==
  /\ MutationMode \in MutationModes
  /\ asyncNow = RequestDeadline
  /\ nodeView = 0
  /\ tcInstalled = FALSE
  /\ requestRetained = TRUE
  /\ requestServiceable = FALSE
  /\ lifecycleOwned = FALSE
  /\ capacityOpen = FALSE
  /\ selectorOpen = FALSE
  /\ served = FALSE
  /\ lastTransition = "Init"

InstallTimeoutCertificate ==
  /\ nodeView = 0
  /\ nodeView' = 1
  /\ tcInstalled' = TRUE
  /\ lastTransition' = "InstallTimeoutCertificate"
  /\ UNCHANGED <<asyncNow, requestRetained, requestServiceable,
                  lifecycleOwned, capacityOpen, selectorOpen, served>>

AsyncTick ==
  /\ AsyncTickEnabled
  /\ asyncNow' = asyncNow + 1
  /\ lastTransition' = "AsyncTick"
  /\ UNCHANGED <<nodeView, tcInstalled, requestRetained,
                  requestServiceable, lifecycleOwned,
                  capacityOpen, selectorOpen, served>>

(***************************************************************************
These activation actions are deliberately available only after the repaired
clock reaches the next-view timeout.  They expose the re-entry and physical
gate invariants without providing the buggy clock with an escape action.
***************************************************************************)
MakeRequestServiceable ==
  /\ asyncNow = NextViewTimeout
  /\ ~requestServiceable
  /\ requestServiceable' = TRUE
  /\ lastTransition' = "MakeRequestServiceable"
  /\ UNCHANGED <<asyncNow, nodeView, tcInstalled, requestRetained,
                  lifecycleOwned, capacityOpen, selectorOpen, served>>

AcquireRequestLifecycle ==
  /\ asyncNow = NextViewTimeout
  /\ ~lifecycleOwned
  /\ lifecycleOwned' = TRUE
  /\ lastTransition' = "AcquireRequestLifecycle"
  /\ UNCHANGED <<asyncNow, nodeView, tcInstalled, requestRetained,
                  requestServiceable, capacityOpen, selectorOpen, served>>

OpenCapacity ==
  /\ asyncNow = NextViewTimeout
  /\ ~capacityOpen
  /\ capacityOpen' = TRUE
  /\ lastTransition' = "OpenCapacity"
  /\ UNCHANGED <<asyncNow, nodeView, tcInstalled, requestRetained,
                  requestServiceable, lifecycleOwned, selectorOpen, served>>

OpenSelector ==
  /\ asyncNow = NextViewTimeout
  /\ ~selectorOpen
  /\ selectorOpen' = TRUE
  /\ lastTransition' = "OpenSelector"
  /\ UNCHANGED <<asyncNow, nodeView, tcInstalled, requestRetained,
                  requestServiceable, lifecycleOwned, capacityOpen, served>>

ServeRetainedRequest ==
  /\ requestRetained
  /\ AsyncServeTransportAdmissionGateAllows
  /\ AsyncPhysicalAdmissionGateAllows
  /\ requestRetained' = FALSE
  /\ served' = TRUE
  /\ lastTransition' = "ServeRetainedRequest"
  /\ UNCHANGED <<asyncNow, nodeView, tcInstalled, requestServiceable,
                  lifecycleOwned, capacityOpen, selectorOpen>>

Next ==
  \/ InstallTimeoutCertificate
  \/ AsyncTick
  \/ MakeRequestServiceable
  \/ AcquireRequestLifecycle
  \/ OpenCapacity
  \/ OpenSelector
  \/ ServeRetainedRequest

Spec ==
  /\ Init
  /\ [][Next]_dormantReplyClockVars
  /\ WF_dormantReplyClockVars(InstallTimeoutCertificate)
  /\ WF_dormantReplyClockVars(AsyncTick)

TypeInvariant ==
  /\ MutationMode \in MutationModes
  /\ asyncNow \in RequestDeadline..NextViewTimeout
  /\ nodeView \in {0, 1}
  /\ tcInstalled \in BOOLEAN
  /\ requestRetained \in BOOLEAN
  /\ requestServiceable \in BOOLEAN
  /\ lifecycleOwned \in BOOLEAN
  /\ capacityOpen \in BOOLEAN
  /\ selectorOpen \in BOOLEAN
  /\ served \in BOOLEAN
  /\ lastTransition
       \in {"Init",
            "InstallTimeoutCertificate",
            "AsyncTick",
            "MakeRequestServiceable",
            "AcquireRequestLifecycle",
            "OpenCapacity",
            "OpenSelector",
            "ServeRetainedRequest"}

DormantRequestRemainsRetained ==
  AsyncDormantExactReplyRequestPacket => requestRetained

ServeGateExcludesPhysicalAdmission ==
  AsyncServeTransportAdmissionGateAllows
    <=> (requestServiceable \/ lifecycleOwned)

GateOpenDuePacketOwnsClockImmediately ==
  /\ AsyncReplyRequestPacketDue
  /\ AsyncServeTransportAdmissionGateAllows
  => AsyncPacketOwnsClockDeadline

PhysicalAdmissionBlockDoesNotReleaseClock ==
  /\ AsyncReplyRequestPacketDue
  /\ AsyncServeTransportAdmissionGateAllows
  /\ ~AsyncPhysicalAdmissionGateAllows
  => AsyncPacketOwnsClockDeadline

FixedDormantPacketDoesNotOwnClock ==
  MutationMode = "Fixed"
    => (AsyncPacketOwnsClockDeadline
          <=> /\ AsyncReplyRequestPacketDue
              /\ ~AsyncDormantExactReplyRequestPacket)

NextViewClockProgress ==
  (nodeView = 1) ~> (asyncNow = NextViewTimeout)

====

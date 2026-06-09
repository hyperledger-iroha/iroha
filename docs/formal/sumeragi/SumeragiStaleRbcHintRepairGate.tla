---- MODULE SumeragiStaleRbcHintRepairGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for stale RBC repair through cached proposal hints.

This slice captures the proposal-hint branch of
`should_drop_stale_rbc_message(...)` and the no-session `handle_rbc_chunk(...)`
path that follows it. It abstracts hashes, heights, views, and message kinds to
finite cases while preserving the helper contract: a stale RBC chunk without a
session may seed exact-frontier repair only when DA is enabled, the RBC message
kind is allowed to seed a session, the height is the exact frontier, and the
cached proposal hint at the same height/view names the same block hash.
Rejected stale RBC messages must still drop as stale and must not stash a chunk
or arm exact frontier repair.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

HintBackedChunk == "hint_backed_chunk"
DaDisabled == "da_disabled"
NonRepairKind == "non_repair_kind"
NonFrontierHeight == "non_frontier_height"
NoCachedHint == "no_cached_hint"
HintHeightMismatch == "hint_height_mismatch"
HintViewMismatch == "hint_view_mismatch"
HintHashMismatch == "hint_hash_mismatch"
RejectDoesNotDrop == "reject_does_not_drop"
RejectDoesNotStash == "reject_does_not_stash"
RejectDoesNotArmRepair == "reject_does_not_arm_repair"

Cases == {
  HintBackedChunk,
  DaDisabled,
  NonRepairKind,
  NonFrontierHeight,
  NoCachedHint,
  HintHeightMismatch,
  HintViewMismatch,
  HintHashMismatch,
  RejectDoesNotDrop,
  RejectDoesNotStash,
  RejectDoesNotArmRepair
}

ReturnAccept == 1
ReturnDrop == 2
RequireDa == 3
RequireRepairKind == 4
RequireExactFrontier == 5
RequireCachedHint == 6
MatchHintHeight == 7
MatchHintView == 8
MatchHintHash == 9
ContinueAfterStaleView == 10
StashPendingChunk == 11
ArmExactFrontierRepair == 12
StaleViewDrop == 13
NoPendingStash == 14
NoRepairArm == 15
NoAcceptWithoutHint == 16

ActionUniverse == 1..16

AcceptActions ==
  {ReturnAccept, RequireDa, RequireRepairKind, RequireExactFrontier,
   RequireCachedHint, MatchHintHeight, MatchHintView, MatchHintHash,
   ContinueAfterStaleView, StashPendingChunk, ArmExactFrontierRepair}

RejectBaseActions ==
  {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}

SpecActions(c) ==
  CASE c = HintBackedChunk ->
      AcceptActions
    [] c = DaDisabled ->
      RejectBaseActions \cup {RequireDa}
    [] c = NonRepairKind ->
      RejectBaseActions \cup {RequireDa, RequireRepairKind}
    [] c = NonFrontierHeight ->
      RejectBaseActions \cup {RequireDa, RequireRepairKind,
                              RequireExactFrontier}
    [] c = NoCachedHint ->
      RejectBaseActions \cup {RequireDa, RequireRepairKind,
                              RequireExactFrontier, RequireCachedHint,
                              NoAcceptWithoutHint}
    [] c = HintHeightMismatch ->
      RejectBaseActions \cup {RequireDa, RequireRepairKind,
                              RequireExactFrontier, RequireCachedHint,
                              MatchHintHeight}
    [] c = HintViewMismatch ->
      RejectBaseActions \cup {RequireDa, RequireRepairKind,
                              RequireExactFrontier, RequireCachedHint,
                              MatchHintHeight, MatchHintView}
    [] c = HintHashMismatch ->
      RejectBaseActions \cup {RequireDa, RequireRepairKind,
                              RequireExactFrontier, RequireCachedHint,
                              MatchHintHeight, MatchHintView, MatchHintHash}
    [] c \in {RejectDoesNotDrop, RejectDoesNotStash,
              RejectDoesNotArmRepair} ->
      RejectBaseActions
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "reject_matching_hint"
       /\ c = HintBackedChunk ->
      (spec \ {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
               ArmExactFrontierRepair}) \cup
        {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}
    [] Bug = "allow_da_disabled"
       /\ c = DaDisabled ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "allow_non_repair_kind"
       /\ c = NonRepairKind ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "allow_non_frontier_height"
       /\ c = NonFrontierHeight ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "allow_without_hint"
       /\ c = NoCachedHint ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "allow_height_mismatch"
       /\ c = HintHeightMismatch ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "allow_view_mismatch"
       /\ c = HintViewMismatch ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "allow_hash_mismatch"
       /\ c = HintHashMismatch ->
      (spec \ {ReturnDrop, StaleViewDrop, NoPendingStash, NoRepairArm}) \cup
        {ReturnAccept, ContinueAfterStaleView, StashPendingChunk,
         ArmExactFrontierRepair}
    [] Bug = "skip_stale_drop_after_reject"
       /\ c = RejectDoesNotDrop ->
      spec \ {StaleViewDrop}
    [] Bug = "stash_rejected_chunk"
       /\ c = RejectDoesNotStash ->
      (spec \ {NoPendingStash}) \cup {StashPendingChunk}
    [] Bug = "arm_repair_on_reject"
       /\ c = RejectDoesNotArmRepair ->
      (spec \ {NoRepairArm}) \cup {ArmExactFrontierRepair}
    [] OTHER -> spec

Bugs == {
  "none",
  "reject_matching_hint",
  "allow_da_disabled",
  "allow_non_repair_kind",
  "allow_non_frontier_height",
  "allow_without_hint",
  "allow_height_mismatch",
  "allow_view_mismatch",
  "allow_hash_mismatch",
  "skip_stale_drop_after_reject",
  "stash_rejected_chunk",
  "arm_repair_on_reject"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

HintBackedStaleChunkSeedsRepair ==
  /\ ReturnAccept \in ImplementationActions(HintBackedChunk)
  /\ ContinueAfterStaleView \in ImplementationActions(HintBackedChunk)
  /\ StashPendingChunk \in ImplementationActions(HintBackedChunk)
  /\ ArmExactFrontierRepair \in ImplementationActions(HintBackedChunk)
  /\ ~(StaleViewDrop \in ImplementationActions(HintBackedChunk))

HintRepairRequiresDaKindAndFrontier ==
  \A c \in {DaDisabled, NonRepairKind, NonFrontierHeight}:
    /\ ReturnDrop \in ImplementationActions(c)
    /\ StaleViewDrop \in ImplementationActions(c)
    /\ ~(ReturnAccept \in ImplementationActions(c))

HintRepairRequiresExactCachedHint ==
  \A c \in {NoCachedHint, HintHeightMismatch, HintViewMismatch,
            HintHashMismatch}:
    /\ ReturnDrop \in ImplementationActions(c)
    /\ StaleViewDrop \in ImplementationActions(c)
    /\ ~(ReturnAccept \in ImplementationActions(c))

RejectedStaleRbcStillDrops ==
  \A c \in Cases \ {HintBackedChunk}:
    StaleViewDrop \in ImplementationActions(c)

RejectedStaleRbcHasNoChunkSideEffects ==
  \A c \in Cases \ {HintBackedChunk}:
    /\ NoPendingStash \in ImplementationActions(c)
    /\ NoRepairArm \in ImplementationActions(c)
    /\ ~(StashPendingChunk \in ImplementationActions(c))
    /\ ~(ArmExactFrontierRepair \in ImplementationActions(c))

NoCachedHintNeverRepairs ==
  /\ NoAcceptWithoutHint \in ImplementationActions(NoCachedHint)
  /\ ~(ReturnAccept \in ImplementationActions(NoCachedHint))

StaleRbcHintRepairCoreSafety ==
  /\ ActionsMatchSpec
  /\ HintBackedStaleChunkSeedsRepair
  /\ HintRepairRequiresDaKindAndFrontier
  /\ HintRepairRequiresExactCachedHint
  /\ RejectedStaleRbcStillDrops
  /\ RejectedStaleRbcHasNoChunkSideEffects
  /\ NoCachedHintNeverRepairs

NoBugInvariant == StaleRbcHintRepairCoreSafety

SafetyFast == StaleRbcHintRepairCoreSafety

====

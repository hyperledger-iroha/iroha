---- MODULE SumeragiBlockSyncImplicitRecoveryGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the post-known-block implicit recovery flag in
`handle_block_sync_update(...)`.

After the known-hintless fast path, DA-enabled BlockSyncUpdate handling treats
an unknown block at or below `local_height + 1` as missing-block recovery when
implicit frontier recovery is allowed. This branch only updates the local
`requested_missing_block` gate. It does not record status, clear requests,
defer the update, or return early. Blocks already requested remain requested,
known local blocks and too-far future blocks do not become requested here, and
NPoS vote-only traffic can disable the implicit path through
`implicit_frontier_recovery_allowed`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "already_requested",
  "da_disabled",
  "known_local",
  "above_frontier_bound",
  "implicit_disallowed",
  "same_height_implicit",
  "next_height_implicit",
  "saturated_boundary_implicit"
}

InitialRequested(c) ==
  c = "already_requested"

DaEnabled(c) ==
  c # "da_disabled"

BlockKnownLocally(c) ==
  c = "known_local"

WithinFrontierBound(c) ==
  c # "above_frontier_bound"

ImplicitAllowed(c) ==
  c # "implicit_disallowed"

SpecImplicitSetsRequested(c) ==
  /\ DaEnabled(c)
  /\ ~InitialRequested(c)
  /\ ~BlockKnownLocally(c)
  /\ WithinFrontierBound(c)
  /\ ImplicitAllowed(c)

SpecRequestedAfter(c) ==
  InitialRequested(c) \/ SpecImplicitSetsRequested(c)

SpecChanged(c) ==
  SpecRequestedAfter(c) # InitialRequested(c)

SpecRecordsStatus(c) ==
  FALSE

SpecClearsMissing(c) ==
  FALSE

SpecDefers(c) ==
  FALSE

SpecReturnKind(c) ==
  "continue"

ActualRequestedAfter(c) ==
  CASE Bug = "already_requested_cleared"
       /\ c = "already_requested" -> FALSE
    [] Bug = "da_disabled_requests"
       /\ c = "da_disabled" -> TRUE
    [] Bug = "known_local_requests"
       /\ c = "known_local" -> TRUE
    [] Bug = "above_frontier_requests"
       /\ c = "above_frontier_bound" -> TRUE
    [] Bug = "implicit_disallowed_requests"
       /\ c = "implicit_disallowed" -> TRUE
    [] Bug = "same_height_not_requested"
       /\ c = "same_height_implicit" -> FALSE
    [] Bug = "next_height_not_requested"
       /\ c = "next_height_implicit" -> FALSE
    [] Bug = "saturated_boundary_not_requested"
       /\ c = "saturated_boundary_implicit" -> FALSE
    [] OTHER -> SpecRequestedAfter(c)

ActualChanged(c) ==
  ActualRequestedAfter(c) # InitialRequested(c)

ActualRecordsStatus(c) ==
  Bug = "implicit_records_status" /\ c = "next_height_implicit"

ActualClearsMissing(c) ==
  Bug = "implicit_clears_missing" /\ c = "next_height_implicit"

ActualDefers(c) ==
  Bug = "implicit_defers_update" /\ c = "next_height_implicit"

ActualReturnKind(c) ==
  CASE Bug = "implicit_returns_early"
       /\ c = "next_height_implicit" -> "Ok"
    [] OTHER -> "continue"

Matches(c) ==
  /\ ActualRequestedAfter(c) = SpecRequestedAfter(c)
  /\ ActualChanged(c) = SpecChanged(c)
  /\ ActualRecordsStatus(c) = SpecRecordsStatus(c)
  /\ ActualClearsMissing(c) = SpecClearsMissing(c)
  /\ ActualDefers(c) = SpecDefers(c)
  /\ ActualReturnKind(c) = SpecReturnKind(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "already_requested_cleared",
       "da_disabled_requests",
       "known_local_requests",
       "above_frontier_requests",
       "implicit_disallowed_requests",
       "same_height_not_requested",
       "next_height_not_requested",
       "saturated_boundary_not_requested",
       "implicit_records_status",
       "implicit_clears_missing",
       "implicit_defers_update",
       "implicit_returns_early"
     }
  /\ checked = 0

ImplicitRecoveryMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncImplicitRecoveryExactness ==
  ImplicitRecoveryMatchesSpec

BlockSyncImplicitRecoveryCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncImplicitRecoveryExactness

SafetyFast ==
  BlockSyncImplicitRecoveryExactness

AlreadyRequestedPreserved ==
  Matches("already_requested")

DaDisabledDoesNotRequest ==
  Matches("da_disabled")

KnownLocalDoesNotRequest ==
  Matches("known_local")

AboveFrontierDoesNotRequest ==
  Matches("above_frontier_bound")

ImplicitDisallowedDoesNotRequest ==
  Matches("implicit_disallowed")

SameHeightImplicitRequests ==
  Matches("same_height_implicit")

NextHeightImplicitRequests ==
  Matches("next_height_implicit")

SaturatedBoundaryRequests ==
  Matches("saturated_boundary_implicit")

ImplicitHasNoStatus ==
  Matches("next_height_implicit")

ImplicitDoesNotClear ==
  Matches("next_height_implicit")

ImplicitDoesNotDefer ==
  Matches("next_height_implicit")

ImplicitContinues ==
  Matches("next_height_implicit")

=============================================================================
====

---- MODULE SumeragiVotePayloadActionableGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `vote_payload_actionable_for_proposal(...)`.

The helper lets vote/QC-backed proposal evidence become actionable when the
referenced block payload is already authoritative, validation work is already
in flight, the pending-processing slot owns the block, or a deferred
BlockSyncUpdate exactly matches the queried height, view, and block hash.
Deferred records with only partial identity matches must not count, and a bad
deferred record must not suppress an earlier authoritative or validation source.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoSource == "no_source"
AuthoritativePayload == "authoritative_payload"
ValidationInflight == "validation_inflight"
VnextInflight == "vnext_inflight"
PendingProcessing == "pending_processing"
DeferredExact == "deferred_exact"
DeferredWrongHeight == "deferred_wrong_height"
DeferredWrongView == "deferred_wrong_view"
DeferredWrongHash == "deferred_wrong_hash"
AuthoritativeWithBadDeferred == "authoritative_with_bad_deferred"
ValidationWithBadDeferred == "validation_with_bad_deferred"

Cases == {
  NoSource,
  AuthoritativePayload,
  ValidationInflight,
  VnextInflight,
  PendingProcessing,
  DeferredExact,
  DeferredWrongHeight,
  DeferredWrongView,
  DeferredWrongHash,
  AuthoritativeWithBadDeferred,
  ValidationWithBadDeferred
}

AuthoritativeCases == {
  AuthoritativePayload,
  AuthoritativeWithBadDeferred
}

ValidationInflightCases == {
  ValidationInflight,
  ValidationWithBadDeferred
}

VnextInflightCases == {VnextInflight}
PendingProcessingCases == {PendingProcessing}
DeferredExactCases == {DeferredExact}

DeferredMismatchCases == {
  DeferredWrongHeight,
  DeferredWrongView,
  DeferredWrongHash
}

HasBadDeferredCases == {
  AuthoritativeWithBadDeferred,
  ValidationWithBadDeferred
} \cup DeferredMismatchCases

SpecActionable(c) ==
  c \in AuthoritativeCases
    \/ c \in ValidationInflightCases
    \/ c \in VnextInflightCases
    \/ c \in PendingProcessingCases
    \/ c \in DeferredExactCases

ImplementationActionable(c) ==
  CASE Bug = "reject_authoritative_payload"
       /\ c = AuthoritativePayload ->
      FALSE
    [] Bug = "reject_validation_inflight"
       /\ c = ValidationInflight ->
      FALSE
    [] Bug = "reject_vnext_inflight"
       /\ c = VnextInflight ->
      FALSE
    [] Bug = "reject_pending_processing"
       /\ c = PendingProcessing ->
      FALSE
    [] Bug = "reject_deferred_exact"
       /\ c = DeferredExact ->
      FALSE
    [] Bug = "accept_no_source"
       /\ c = NoSource ->
      TRUE
    [] Bug = "accept_deferred_wrong_height"
       /\ c = DeferredWrongHeight ->
      TRUE
    [] Bug = "accept_deferred_wrong_view"
       /\ c = DeferredWrongView ->
      TRUE
    [] Bug = "accept_deferred_wrong_hash"
       /\ c = DeferredWrongHash ->
      TRUE
    [] Bug = "bad_deferred_blocks_early_source"
       /\ c \in {AuthoritativeWithBadDeferred, ValidationWithBadDeferred} ->
      FALSE
    [] OTHER -> SpecActionable(c)

Bugs == {
  "none",
  "reject_authoritative_payload",
  "reject_validation_inflight",
  "reject_vnext_inflight",
  "reject_pending_processing",
  "reject_deferred_exact",
  "accept_no_source",
  "accept_deferred_wrong_height",
  "accept_deferred_wrong_view",
  "accept_deferred_wrong_hash",
  "bad_deferred_blocks_early_source"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActionable(c) \in BOOLEAN
       /\ ImplementationActionable(c) \in BOOLEAN

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationActionable(c) = SpecActionable(c)

EachSourceActionable ==
  /\ ImplementationActionable(AuthoritativePayload)
  /\ ImplementationActionable(ValidationInflight)
  /\ ImplementationActionable(VnextInflight)
  /\ ImplementationActionable(PendingProcessing)
  /\ ImplementationActionable(DeferredExact)

DeferredIdentityMustMatch ==
  /\ ~ImplementationActionable(DeferredWrongHeight)
  /\ ~ImplementationActionable(DeferredWrongView)
  /\ ~ImplementationActionable(DeferredWrongHash)

NoSourceRejected ==
  ~ImplementationActionable(NoSource)

BadDeferredDoesNotBlockEarlySource ==
  /\ ImplementationActionable(AuthoritativeWithBadDeferred)
  /\ ImplementationActionable(ValidationWithBadDeferred)

VotePayloadActionableCoreSafety ==
  /\ ResultMatchesSpec
  /\ EachSourceActionable
  /\ DeferredIdentityMustMatch
  /\ NoSourceRejected
  /\ BadDeferredDoesNotBlockEarlySource

NoBugInvariant == VotePayloadActionableCoreSafety

SafetyFast == VotePayloadActionableCoreSafety

====

---- MODULE SumeragiRbcSessionReadyDeliverGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC READY and DELIVER session recording.

This slice captures `RbcSession::record_ready(...)`,
`record_ready_with_roster_hash(...)`, and `record_deliver(...)`: first READY
signatures are stored, duplicate READY signatures are idempotent, conflicting
READY signatures invalidate the session without overwriting, READY roster hashes
are set once and enforced thereafter, first DELIVER records sender/signature and
advances progress, and DELIVER replays never rewrite the recorded delivery.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ReadyFirst == "ready_first"
ReadyDuplicateSame == "ready_duplicate_same"
ReadyDuplicateDifferent == "ready_duplicate_different"
ReadyWithRosterFirst == "ready_with_roster_first"
ReadyWithRosterMatchNewSender == "ready_with_roster_match_new_sender"
ReadyWithRosterMismatch == "ready_with_roster_mismatch"
ReadyWithRosterDuplicateSame == "ready_with_roster_duplicate_same"
ReadyWithRosterDuplicateDifferent == "ready_with_roster_duplicate_different"
DeliverFirst == "deliver_first"
DeliverReplaySame == "deliver_replay_same"
DeliverReplayDifferentSender == "deliver_replay_different_sender"
DeliverReplayDifferentSignature == "deliver_replay_different_signature"

Cases == {
  ReadyFirst,
  ReadyDuplicateSame,
  ReadyDuplicateDifferent,
  ReadyWithRosterFirst,
  ReadyWithRosterMatchNewSender,
  ReadyWithRosterMismatch,
  ReadyWithRosterDuplicateSame,
  ReadyWithRosterDuplicateDifferent,
  DeliverFirst,
  DeliverReplaySame,
  DeliverReplayDifferentSender,
  DeliverReplayDifferentSignature
}

ReadyDuplicateCases == {
  ReadyDuplicateSame,
  ReadyDuplicateDifferent,
  ReadyWithRosterDuplicateSame,
  ReadyWithRosterDuplicateDifferent
}

DeliverReplayCases == {
  DeliverReplaySame,
  DeliverReplayDifferentSender,
  DeliverReplayDifferentSignature
}

ReturnTrue == 1
ReturnFalse == 2
ReadyStored == 3
ReadyNotStored == 4
ReadyDuplicatePreserved == 5
ReadyConflictInvalid == 6
ReadyConflictOverwritten == 7
InvalidSet == 8
InvalidPreserved == 9
RosterHashSet == 10
RosterHashPreserved == 11
RosterHashUpdated == 12
RosterHashNotSet == 13
RosterMismatchRejected == 14
DeliveredSet == 15
DeliverSenderSet == 16
DeliverSignatureSet == 17
ProgressDelivered == 18
DeliveredPreserved == 19
DeliverSenderPreserved == 20
DeliverSignaturePreserved == 21
DeliverUpdated == 22

ActionUniverse == 1..22

SpecActions(c) ==
  CASE c = ReadyFirst ->
      {ReturnTrue, ReadyStored, InvalidPreserved, RosterHashNotSet}
    [] c = ReadyDuplicateSame ->
      {ReturnFalse, ReadyDuplicatePreserved, InvalidPreserved,
       RosterHashNotSet}
    [] c = ReadyDuplicateDifferent ->
      {ReturnFalse, ReadyDuplicatePreserved, ReadyConflictInvalid,
       InvalidSet, RosterHashNotSet}
    [] c = ReadyWithRosterFirst ->
      {ReturnTrue, ReadyStored, RosterHashSet, InvalidPreserved}
    [] c = ReadyWithRosterMatchNewSender ->
      {ReturnTrue, ReadyStored, RosterHashPreserved, InvalidPreserved}
    [] c = ReadyWithRosterMismatch ->
      {ReturnFalse, ReadyNotStored, RosterMismatchRejected,
       RosterHashPreserved, InvalidSet}
    [] c = ReadyWithRosterDuplicateSame ->
      {ReturnFalse, ReadyDuplicatePreserved, RosterHashPreserved,
       InvalidPreserved}
    [] c = ReadyWithRosterDuplicateDifferent ->
      {ReturnFalse, ReadyDuplicatePreserved, ReadyConflictInvalid,
       RosterHashPreserved, InvalidSet}
    [] c = DeliverFirst ->
      {ReturnTrue, DeliveredSet, DeliverSenderSet, DeliverSignatureSet,
       ProgressDelivered, InvalidPreserved}
    [] c \in DeliverReplayCases ->
      {ReturnFalse, DeliveredPreserved, DeliverSenderPreserved,
       DeliverSignaturePreserved, InvalidPreserved}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "ready_first_returns_false" /\ c = ReadyFirst ->
      (spec \ {ReturnTrue}) \cup {ReturnFalse}
    [] Bug = "ready_first_not_stored" /\ c = ReadyFirst ->
      (spec \ {ReadyStored}) \cup {ReadyNotStored}
    [] Bug = "ready_duplicate_same_invalidates" /\
       c = ReadyDuplicateSame ->
      (spec \ {InvalidPreserved}) \cup {InvalidSet}
    [] Bug = "ready_duplicate_same_stores_again" /\
       c = ReadyDuplicateSame ->
      (spec \ {ReadyDuplicatePreserved}) \cup {ReadyStored}
    [] Bug = "ready_duplicate_diff_not_invalid" /\
       c = ReadyDuplicateDifferent ->
      (spec \ {ReadyConflictInvalid, InvalidSet}) \cup {InvalidPreserved}
    [] Bug = "ready_duplicate_diff_overwrites" /\
       c = ReadyDuplicateDifferent ->
      (spec \ {ReadyDuplicatePreserved}) \cup {ReadyConflictOverwritten}
    [] Bug = "roster_first_not_set" /\ c = ReadyWithRosterFirst ->
      (spec \ {RosterHashSet}) \cup {RosterHashNotSet}
    [] Bug = "roster_mismatch_records_ready" /\
       c = ReadyWithRosterMismatch ->
      (spec \ {ReadyNotStored, RosterMismatchRejected}) \cup {ReadyStored}
    [] Bug = "roster_mismatch_updates_hash" /\
       c = ReadyWithRosterMismatch ->
      (spec \ {RosterHashPreserved}) \cup {RosterHashUpdated}
    [] Bug = "roster_mismatch_not_invalid" /\
       c = ReadyWithRosterMismatch ->
      (spec \ {InvalidSet}) \cup {InvalidPreserved}
    [] Bug = "roster_match_rejects_new_sender" /\
       c = ReadyWithRosterMatchNewSender ->
      (spec \ {ReturnTrue, ReadyStored}) \cup {ReturnFalse, ReadyNotStored}
    [] Bug = "roster_duplicate_diff_not_invalid" /\
       c = ReadyWithRosterDuplicateDifferent ->
      (spec \ {ReadyConflictInvalid, InvalidSet}) \cup {InvalidPreserved}
    [] Bug = "deliver_first_returns_false" /\ c = DeliverFirst ->
      (spec \ {ReturnTrue}) \cup {ReturnFalse}
    [] Bug = "deliver_first_missing_sender" /\ c = DeliverFirst ->
      spec \ {DeliverSenderSet}
    [] Bug = "deliver_first_missing_signature" /\ c = DeliverFirst ->
      spec \ {DeliverSignatureSet}
    [] Bug = "deliver_first_no_progress" /\ c = DeliverFirst ->
      spec \ {ProgressDelivered}
    [] Bug = "deliver_replay_updates_sender" /\ c \in DeliverReplayCases ->
      (spec \ {DeliverSenderPreserved}) \cup {DeliverUpdated}
    [] Bug = "deliver_replay_updates_signature" /\ c \in DeliverReplayCases ->
      (spec \ {DeliverSignaturePreserved}) \cup {DeliverUpdated}
    [] Bug = "deliver_replay_invalidates" /\ c \in DeliverReplayCases ->
      (spec \ {InvalidPreserved}) \cup {InvalidSet}
    [] Bug = "deliver_replay_returns_true" /\ c \in DeliverReplayCases ->
      (spec \ {ReturnFalse}) \cup {ReturnTrue}
    [] OTHER -> spec

Bugs == {
  "none",
  "ready_first_returns_false",
  "ready_first_not_stored",
  "ready_duplicate_same_invalidates",
  "ready_duplicate_same_stores_again",
  "ready_duplicate_diff_not_invalid",
  "ready_duplicate_diff_overwrites",
  "roster_first_not_set",
  "roster_mismatch_records_ready",
  "roster_mismatch_updates_hash",
  "roster_mismatch_not_invalid",
  "roster_match_rejects_new_sender",
  "roster_duplicate_diff_not_invalid",
  "deliver_first_returns_false",
  "deliver_first_missing_sender",
  "deliver_first_missing_signature",
  "deliver_first_no_progress",
  "deliver_replay_updates_sender",
  "deliver_replay_updates_signature",
  "deliver_replay_invalidates",
  "deliver_replay_returns_true"
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

ReadyRecordingIsIdempotentAndConflictAware ==
  /\ ReturnTrue \in ImplementationActions(ReadyFirst)
  /\ ReadyStored \in ImplementationActions(ReadyFirst)
  /\ ReturnFalse \in ImplementationActions(ReadyDuplicateSame)
  /\ ReadyDuplicatePreserved \in ImplementationActions(ReadyDuplicateSame)
  /\ InvalidPreserved \in ImplementationActions(ReadyDuplicateSame)
  /\ ReturnFalse \in ImplementationActions(ReadyDuplicateDifferent)
  /\ ReadyDuplicatePreserved \in
       ImplementationActions(ReadyDuplicateDifferent)
  /\ ReadyConflictInvalid \in ImplementationActions(ReadyDuplicateDifferent)
  /\ InvalidSet \in ImplementationActions(ReadyDuplicateDifferent)
  /\ ReadyConflictOverwritten \notin
       ImplementationActions(ReadyDuplicateDifferent)

ReadyRosterHashIsSetOnceAndEnforced ==
  /\ ReturnTrue \in ImplementationActions(ReadyWithRosterFirst)
  /\ ReadyStored \in ImplementationActions(ReadyWithRosterFirst)
  /\ RosterHashSet \in ImplementationActions(ReadyWithRosterFirst)
  /\ ReturnTrue \in ImplementationActions(ReadyWithRosterMatchNewSender)
  /\ ReadyStored \in ImplementationActions(ReadyWithRosterMatchNewSender)
  /\ RosterHashPreserved \in
       ImplementationActions(ReadyWithRosterMatchNewSender)
  /\ ReturnFalse \in ImplementationActions(ReadyWithRosterMismatch)
  /\ ReadyNotStored \in ImplementationActions(ReadyWithRosterMismatch)
  /\ RosterMismatchRejected \in
       ImplementationActions(ReadyWithRosterMismatch)
  /\ RosterHashPreserved \in ImplementationActions(ReadyWithRosterMismatch)
  /\ RosterHashUpdated \notin ImplementationActions(ReadyWithRosterMismatch)
  /\ InvalidSet \in ImplementationActions(ReadyWithRosterMismatch)
  /\ ReturnFalse \in ImplementationActions(ReadyWithRosterDuplicateSame)
  /\ ReadyDuplicatePreserved \in
       ImplementationActions(ReadyWithRosterDuplicateSame)
  /\ InvalidPreserved \in ImplementationActions(ReadyWithRosterDuplicateSame)
  /\ ReadyConflictInvalid \in
       ImplementationActions(ReadyWithRosterDuplicateDifferent)
  /\ InvalidSet \in ImplementationActions(ReadyWithRosterDuplicateDifferent)

DeliverFirstRecordsAndAdvances ==
  /\ ReturnTrue \in ImplementationActions(DeliverFirst)
  /\ DeliveredSet \in ImplementationActions(DeliverFirst)
  /\ DeliverSenderSet \in ImplementationActions(DeliverFirst)
  /\ DeliverSignatureSet \in ImplementationActions(DeliverFirst)
  /\ ProgressDelivered \in ImplementationActions(DeliverFirst)
  /\ InvalidPreserved \in ImplementationActions(DeliverFirst)

DeliverReplaysAreImmutable ==
  \A c \in DeliverReplayCases:
    /\ ReturnFalse \in ImplementationActions(c)
    /\ DeliveredPreserved \in ImplementationActions(c)
    /\ DeliverSenderPreserved \in ImplementationActions(c)
    /\ DeliverSignaturePreserved \in ImplementationActions(c)
    /\ DeliverUpdated \notin ImplementationActions(c)
    /\ InvalidPreserved \in ImplementationActions(c)

RbcSessionReadyDeliverCoreSafety ==
  /\ ActionsMatchSpec
  /\ ReadyRecordingIsIdempotentAndConflictAware
  /\ ReadyRosterHashIsSetOnceAndEnforced
  /\ DeliverFirstRecordsAndAdvances
  /\ DeliverReplaysAreImmutable

RbcSessionReadyDeliverExactness ==
  /\ ActionsMatchSpec
  /\ ReadyRecordingIsIdempotentAndConflictAware
  /\ ReadyRosterHashIsSetOnceAndEnforced
  /\ DeliverFirstRecordsAndAdvances
  /\ DeliverReplaysAreImmutable

RbcSessionReadyDeliverCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcSessionReadyDeliverExactness

SafetyFast ==
  RbcSessionReadyDeliverExactness

====

---- MODULE SumeragiKuraReplicaAdvertGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `Actor::handle_kura_replica_advert(...)`.

This slice captures the ingress gate around durable block-body replica adverts:
unauthenticated adverts are dropped before Kura state is touched, adverts from
the local peer are ignored, and authenticated remote adverts record the sender,
height, block hash, and payload length exactly.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

UnauthenticatedDrop == "unauthenticated_drop"
SelfAdvertDrop == "self_advert_drop"
RemoteAdvertRecord == "remote_advert_record"
RemoteZeroPayloadRecord == "remote_zero_payload_record"
RemoteHighTupleRecord == "remote_high_tuple_record"

Cases == {
  UnauthenticatedDrop,
  SelfAdvertDrop,
  RemoteAdvertRecord,
  RemoteZeroPayloadRecord,
  RemoteHighTupleRecord
}

RemoteCases == {
  RemoteAdvertRecord,
  RemoteZeroPayloadRecord,
  RemoteHighTupleRecord
}

NoRecord == 1
AuthenticatedSenderRequired == 2
SelfIgnored == 3
NoLocalRecord == 4
RemoteRecorded == 5
RecordPeerRemote == 6
RecordPeerSelf == 7
RecordPeerUnknown == 8
RecordHeightExact == 9
RecordWrongHeight == 10
RecordHashExact == 11
RecordWrongHash == 12
RecordPayloadLenExact == 13
RecordPayloadLenZero == 14
RecordPayloadLenWrong == 15
ZeroPayloadLenPreserved == 16
HighTupleExact == 17
HighTupleNotNormalized == 18

ActionUniverse == 1..18

SpecActions(c) ==
  CASE c = UnauthenticatedDrop ->
      {NoRecord}
    [] c = SelfAdvertDrop ->
      {AuthenticatedSenderRequired, SelfIgnored, NoRecord, NoLocalRecord}
    [] c = RemoteAdvertRecord ->
      {AuthenticatedSenderRequired, RemoteRecorded, RecordPeerRemote,
       RecordHeightExact, RecordHashExact, RecordPayloadLenExact}
    [] c = RemoteZeroPayloadRecord ->
      {AuthenticatedSenderRequired, RemoteRecorded, RecordPeerRemote,
       RecordHeightExact, RecordHashExact, RecordPayloadLenExact,
       RecordPayloadLenZero, ZeroPayloadLenPreserved}
    [] c = RemoteHighTupleRecord ->
      {AuthenticatedSenderRequired, RemoteRecorded, RecordPeerRemote,
       RecordHeightExact, RecordHashExact, RecordPayloadLenExact,
       HighTupleExact, HighTupleNotNormalized}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "accept_unauthenticated" /\ c = UnauthenticatedDrop ->
      (spec \ {NoRecord}) \cup
        {AuthenticatedSenderRequired, RemoteRecorded, RecordPeerUnknown,
         RecordHeightExact, RecordHashExact, RecordPayloadLenExact}
    [] Bug = "unauthenticated_records_self" /\ c = UnauthenticatedDrop ->
      (spec \ {NoRecord}) \cup
        {AuthenticatedSenderRequired, RemoteRecorded, RecordPeerSelf,
         RecordHeightExact, RecordHashExact, RecordPayloadLenExact}
    [] Bug = "record_self" /\ c = SelfAdvertDrop ->
      (spec \ {NoRecord, NoLocalRecord}) \cup
        {RemoteRecorded, RecordPeerSelf, RecordHeightExact, RecordHashExact,
         RecordPayloadLenExact}
    [] Bug = "self_records_remote" /\ c = SelfAdvertDrop ->
      (spec \ {NoRecord}) \cup
        {RemoteRecorded, RecordPeerRemote, RecordHeightExact, RecordHashExact,
         RecordPayloadLenExact}
    [] Bug = "drop_remote" /\ c = RemoteAdvertRecord ->
      {AuthenticatedSenderRequired, NoRecord}
    [] Bug = "remote_wrong_peer" /\ c \in RemoteCases ->
      (spec \ {RecordPeerRemote}) \cup {RecordPeerSelf}
    [] Bug = "remote_missing_peer" /\ c \in RemoteCases ->
      spec \ {RecordPeerRemote}
    [] Bug = "remote_wrong_height" /\ c \in RemoteCases ->
      (spec \ {RecordHeightExact}) \cup {RecordWrongHeight}
    [] Bug = "remote_wrong_hash" /\ c \in RemoteCases ->
      (spec \ {RecordHashExact}) \cup {RecordWrongHash}
    [] Bug = "remote_wrong_payload_len" /\ c \in RemoteCases ->
      (spec \ {RecordPayloadLenExact}) \cup {RecordPayloadLenWrong}
    [] Bug = "zero_payload_rejected" /\ c = RemoteZeroPayloadRecord ->
      {AuthenticatedSenderRequired, NoRecord}
    [] Bug = "zero_payload_len_normalized" /\ c = RemoteZeroPayloadRecord ->
      (spec \ {RecordPayloadLenExact, RecordPayloadLenZero,
               ZeroPayloadLenPreserved}) \cup {RecordPayloadLenWrong}
    [] OTHER -> spec

Bugs == {
  "none",
  "accept_unauthenticated",
  "unauthenticated_records_self",
  "record_self",
  "self_records_remote",
  "drop_remote",
  "remote_wrong_peer",
  "remote_missing_peer",
  "remote_wrong_height",
  "remote_wrong_hash",
  "remote_wrong_payload_len",
  "zero_payload_rejected",
  "zero_payload_len_normalized"
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

UnauthenticatedAdvertsDoNotMutate ==
  /\ NoRecord \in ImplementationActions(UnauthenticatedDrop)
  /\ RemoteRecorded \notin ImplementationActions(UnauthenticatedDrop)
  /\ RecordPeerRemote \notin ImplementationActions(UnauthenticatedDrop)
  /\ RecordPeerSelf \notin ImplementationActions(UnauthenticatedDrop)
  /\ RecordPeerUnknown \notin ImplementationActions(UnauthenticatedDrop)

SelfAdvertsDoNotMutate ==
  /\ NoRecord \in ImplementationActions(SelfAdvertDrop)
  /\ SelfIgnored \in ImplementationActions(SelfAdvertDrop)
  /\ NoLocalRecord \in ImplementationActions(SelfAdvertDrop)
  /\ RemoteRecorded \notin ImplementationActions(SelfAdvertDrop)
  /\ RecordPeerRemote \notin ImplementationActions(SelfAdvertDrop)
  /\ RecordPeerSelf \notin ImplementationActions(SelfAdvertDrop)

RemoteAdvertsAreRecordedExactly ==
  \A c \in RemoteCases:
    /\ RemoteRecorded \in ImplementationActions(c)
    /\ RecordPeerRemote \in ImplementationActions(c)
    /\ RecordPeerSelf \notin ImplementationActions(c)
    /\ RecordPeerUnknown \notin ImplementationActions(c)
    /\ RecordHeightExact \in ImplementationActions(c)
    /\ RecordWrongHeight \notin ImplementationActions(c)
    /\ RecordHashExact \in ImplementationActions(c)
    /\ RecordWrongHash \notin ImplementationActions(c)
    /\ RecordPayloadLenExact \in ImplementationActions(c)
    /\ RecordPayloadLenWrong \notin ImplementationActions(c)

ZeroPayloadLenRecordedExactly ==
  /\ RemoteRecorded \in ImplementationActions(RemoteZeroPayloadRecord)
  /\ RecordPayloadLenExact \in ImplementationActions(RemoteZeroPayloadRecord)
  /\ RecordPayloadLenZero \in ImplementationActions(RemoteZeroPayloadRecord)
  /\ ZeroPayloadLenPreserved \in
       ImplementationActions(RemoteZeroPayloadRecord)
  /\ RecordPayloadLenWrong \notin
       ImplementationActions(RemoteZeroPayloadRecord)

HighAdvertTupleRecordedExactly ==
  /\ HighTupleExact \in ImplementationActions(RemoteHighTupleRecord)
  /\ HighTupleNotNormalized \in ImplementationActions(RemoteHighTupleRecord)
  /\ RecordWrongHeight \notin ImplementationActions(RemoteHighTupleRecord)
  /\ RecordWrongHash \notin ImplementationActions(RemoteHighTupleRecord)
  /\ RecordPayloadLenWrong \notin
       ImplementationActions(RemoteHighTupleRecord)

KuraReplicaAdvertExactness ==
  /\ ActionsMatchSpec
  /\ UnauthenticatedAdvertsDoNotMutate
  /\ SelfAdvertsDoNotMutate
  /\ RemoteAdvertsAreRecordedExactly
  /\ ZeroPayloadLenRecordedExactly
  /\ HighAdvertTupleRecordedExactly

KuraReplicaAdvertCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ KuraReplicaAdvertExactness

SafetyFast ==
  KuraReplicaAdvertExactness

====

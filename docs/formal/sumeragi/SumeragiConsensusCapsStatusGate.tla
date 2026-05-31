---- MODULE SumeragiConsensusCapsStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi consensus capability status projection.

This slice captures `set_consensus_caps(...)`, `consensus_caps()`, and
`snapshot().consensus_caps`. Handshake capability construction and fingerprint
binding are covered by `SumeragiConsensusHandshakeCapsGate`; this gate pins the
operator-visible storage/projection contract for every `ConsensusConfigCaps`
field after those caps have been recomputed.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialAbsent == 1
StoreFanout == 2
StoreDaAndEncoding == 3
StoreRs16 == 4
StoreRbcSession == 5
StoreRbcStoreSessions == 6
StoreRbcStoreBytes == 7
GetterProjectsAll == 8
StatusProjectsAll == 9
OverwriteFanout == 10
OverwriteDaAndEncoding == 11
OverwriteRs16 == 12
OverwriteSession == 13
OverwriteStoreSessions == 14
OverwriteStoreBytes == 15

Candidates == 1..15

InitialNone == 1
CollectorsStored == 2
RedundantStored == 3
DaFlagStored == 4
ChunkBytesStored == 5
EncodingStored == 6
Rs16DataStored == 7
Rs16ParityStored == 8
SessionTtlStored == 9
StoreMaxSessionsStored == 10
StoreSoftSessionsStored == 11
StoreMaxBytesStored == 12
StoreSoftBytesStored == 13
GetterPresent == 14
GetterFanoutMatches == 15
GetterDaEncodingMatches == 16
GetterRs16Matches == 17
GetterSessionMatches == 18
GetterStoreMatches == 19
StatusPresent == 20
StatusFanoutMatches == 21
StatusDaEncodingMatches == 22
StatusRs16Matches == 23
StatusSessionMatches == 24
StatusStoreMatches == 25
FanoutOverwriteLatest == 26
DaEncodingOverwriteLatest == 27
Rs16OverwriteLatest == 28
SessionOverwriteLatest == 29
StoreSessionsOverwriteLatest == 30
StoreBytesOverwriteLatest == 31
MaxSoftSessionsDistinct == 32
MaxSoftBytesDistinct == 33

FanoutStoreActions == {CollectorsStored, RedundantStored}
DaEncodingStoreActions == {DaFlagStored, ChunkBytesStored, EncodingStored}
Rs16StoreActions == {Rs16DataStored, Rs16ParityStored}
StoreSessionActions == {StoreMaxSessionsStored, StoreSoftSessionsStored}
StoreBytesActions == {StoreMaxBytesStored, StoreSoftBytesStored}
GetterActions ==
  {GetterPresent, GetterFanoutMatches, GetterDaEncodingMatches,
   GetterRs16Matches, GetterSessionMatches, GetterStoreMatches}
StatusActions ==
  {StatusPresent, StatusFanoutMatches, StatusDaEncodingMatches,
   StatusRs16Matches, StatusSessionMatches, StatusStoreMatches}

SpecActions(candidate) ==
  CASE candidate = InitialAbsent ->
      {InitialNone}
    [] candidate = StoreFanout ->
      FanoutStoreActions
    [] candidate = StoreDaAndEncoding ->
      DaEncodingStoreActions
    [] candidate = StoreRs16 ->
      Rs16StoreActions
    [] candidate = StoreRbcSession ->
      {SessionTtlStored}
    [] candidate = StoreRbcStoreSessions ->
      StoreSessionActions \cup {MaxSoftSessionsDistinct}
    [] candidate = StoreRbcStoreBytes ->
      StoreBytesActions \cup {MaxSoftBytesDistinct}
    [] candidate = GetterProjectsAll ->
      GetterActions
    [] candidate = StatusProjectsAll ->
      StatusActions
    [] candidate = OverwriteFanout ->
      FanoutStoreActions \cup {FanoutOverwriteLatest}
    [] candidate = OverwriteDaAndEncoding ->
      DaEncodingStoreActions \cup {DaEncodingOverwriteLatest}
    [] candidate = OverwriteRs16 ->
      Rs16StoreActions \cup {Rs16OverwriteLatest}
    [] candidate = OverwriteSession ->
      {SessionTtlStored, SessionOverwriteLatest}
    [] candidate = OverwriteStoreSessions ->
      StoreSessionActions \cup {StoreSessionsOverwriteLatest,
       MaxSoftSessionsDistinct}
    [] candidate = OverwriteStoreBytes ->
      StoreBytesActions \cup {StoreBytesOverwriteLatest, MaxSoftBytesDistinct}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialAbsent /\ Bug = "initial_caps_present" ->
      spec \ {InitialNone}
    [] candidate \in {StoreFanout, OverwriteFanout} /\
          Bug = "collectors_not_stored" ->
      spec \ {CollectorsStored}
    [] candidate \in {StoreFanout, OverwriteFanout} /\
          Bug = "redundant_not_stored" ->
      spec \ {RedundantStored}
    [] candidate \in {StoreDaAndEncoding, OverwriteDaAndEncoding} /\
          Bug = "da_flag_not_stored" ->
      spec \ {DaFlagStored}
    [] candidate \in {StoreDaAndEncoding, OverwriteDaAndEncoding} /\
          Bug = "chunk_bytes_not_stored" ->
      spec \ {ChunkBytesStored}
    [] candidate \in {StoreDaAndEncoding, OverwriteDaAndEncoding} /\
          Bug = "encoding_not_stored" ->
      spec \ {EncodingStored}
    [] candidate \in {StoreRs16, OverwriteRs16} /\
          Bug = "rs16_data_not_stored" ->
      spec \ {Rs16DataStored}
    [] candidate \in {StoreRs16, OverwriteRs16} /\
          Bug = "rs16_parity_not_stored" ->
      spec \ {Rs16ParityStored}
    [] candidate \in {StoreRs16, OverwriteRs16} /\
          Bug = "rs16_data_parity_swapped" ->
      spec \ Rs16StoreActions
    [] candidate \in {StoreRbcSession, OverwriteSession} /\
          Bug = "ttl_not_stored" ->
      spec \ {SessionTtlStored}
    [] candidate \in {StoreRbcStoreSessions, OverwriteStoreSessions} /\
          Bug = "store_max_sessions_not_stored" ->
      spec \ {StoreMaxSessionsStored}
    [] candidate \in {StoreRbcStoreSessions, OverwriteStoreSessions} /\
          Bug = "store_soft_sessions_not_stored" ->
      spec \ {StoreSoftSessionsStored}
    [] candidate \in {StoreRbcStoreSessions, OverwriteStoreSessions} /\
          Bug = "store_sessions_swapped" ->
      spec \ {MaxSoftSessionsDistinct}
    [] candidate \in {StoreRbcStoreBytes, OverwriteStoreBytes} /\
          Bug = "store_max_bytes_not_stored" ->
      spec \ {StoreMaxBytesStored}
    [] candidate \in {StoreRbcStoreBytes, OverwriteStoreBytes} /\
          Bug = "store_soft_bytes_not_stored" ->
      spec \ {StoreSoftBytesStored}
    [] candidate \in {StoreRbcStoreBytes, OverwriteStoreBytes} /\
          Bug = "store_bytes_swapped" ->
      spec \ {MaxSoftBytesDistinct}
    [] candidate = GetterProjectsAll /\ Bug = "getter_missing" ->
      spec \ {GetterPresent}
    [] candidate = GetterProjectsAll /\ Bug = "getter_fields_mismatch" ->
      spec \ (GetterActions \ {GetterPresent})
    [] candidate = StatusProjectsAll /\ Bug = "status_missing" ->
      spec \ {StatusPresent}
    [] candidate = StatusProjectsAll /\ Bug = "status_fields_mismatch" ->
      spec \ (StatusActions \ {StatusPresent})
    [] candidate = OverwriteFanout /\ Bug = "overwrite_fanout_ignored" ->
      spec \ {FanoutOverwriteLatest}
    [] candidate = OverwriteDaAndEncoding /\
          Bug = "overwrite_da_encoding_ignored" ->
      spec \ {DaEncodingOverwriteLatest}
    [] candidate = OverwriteRs16 /\ Bug = "overwrite_rs16_ignored" ->
      spec \ {Rs16OverwriteLatest}
    [] candidate = OverwriteSession /\ Bug = "overwrite_session_ignored" ->
      spec \ {SessionOverwriteLatest}
    [] candidate = OverwriteStoreSessions /\
          Bug = "overwrite_store_sessions_ignored" ->
      spec \ {StoreSessionsOverwriteLatest}
    [] candidate = OverwriteStoreBytes /\
          Bug = "overwrite_store_bytes_ignored" ->
      spec \ {StoreBytesOverwriteLatest}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 15
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..15

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugInitialCapsPresent ==
  ImplementationActions(InitialAbsent) = SpecActions(InitialAbsent)

BugCollectorsNotStored ==
  ImplementationActions(StoreFanout) = SpecActions(StoreFanout)

BugRedundantNotStored ==
  ImplementationActions(StoreFanout) = SpecActions(StoreFanout)

BugDaFlagNotStored ==
  ImplementationActions(StoreDaAndEncoding) = SpecActions(StoreDaAndEncoding)

BugChunkBytesNotStored ==
  ImplementationActions(StoreDaAndEncoding) = SpecActions(StoreDaAndEncoding)

BugEncodingNotStored ==
  ImplementationActions(StoreDaAndEncoding) = SpecActions(StoreDaAndEncoding)

BugRs16DataNotStored ==
  ImplementationActions(StoreRs16) = SpecActions(StoreRs16)

BugRs16ParityNotStored ==
  ImplementationActions(StoreRs16) = SpecActions(StoreRs16)

BugRs16DataParitySwapped ==
  ImplementationActions(StoreRs16) = SpecActions(StoreRs16)

BugTtlNotStored ==
  ImplementationActions(StoreRbcSession) = SpecActions(StoreRbcSession)

BugStoreMaxSessionsNotStored ==
  ImplementationActions(StoreRbcStoreSessions) =
    SpecActions(StoreRbcStoreSessions)

BugStoreSoftSessionsNotStored ==
  ImplementationActions(StoreRbcStoreSessions) =
    SpecActions(StoreRbcStoreSessions)

BugStoreSessionsSwapped ==
  ImplementationActions(StoreRbcStoreSessions) =
    SpecActions(StoreRbcStoreSessions)

BugStoreMaxBytesNotStored ==
  ImplementationActions(StoreRbcStoreBytes) =
    SpecActions(StoreRbcStoreBytes)

BugStoreSoftBytesNotStored ==
  ImplementationActions(StoreRbcStoreBytes) =
    SpecActions(StoreRbcStoreBytes)

BugStoreBytesSwapped ==
  ImplementationActions(StoreRbcStoreBytes) =
    SpecActions(StoreRbcStoreBytes)

BugGetterMissing ==
  ImplementationActions(GetterProjectsAll) = SpecActions(GetterProjectsAll)

BugGetterFieldsMismatch ==
  ImplementationActions(GetterProjectsAll) = SpecActions(GetterProjectsAll)

BugStatusMissing ==
  ImplementationActions(StatusProjectsAll) = SpecActions(StatusProjectsAll)

BugStatusFieldsMismatch ==
  ImplementationActions(StatusProjectsAll) = SpecActions(StatusProjectsAll)

BugOverwriteFanoutIgnored ==
  ImplementationActions(OverwriteFanout) = SpecActions(OverwriteFanout)

BugOverwriteDaEncodingIgnored ==
  ImplementationActions(OverwriteDaAndEncoding) =
    SpecActions(OverwriteDaAndEncoding)

BugOverwriteRs16Ignored ==
  ImplementationActions(OverwriteRs16) = SpecActions(OverwriteRs16)

BugOverwriteSessionIgnored ==
  ImplementationActions(OverwriteSession) = SpecActions(OverwriteSession)

BugOverwriteStoreSessionsIgnored ==
  ImplementationActions(OverwriteStoreSessions) =
    SpecActions(OverwriteStoreSessions)

BugOverwriteStoreBytesIgnored ==
  ImplementationActions(OverwriteStoreBytes) =
    SpecActions(OverwriteStoreBytes)

=============================================================================

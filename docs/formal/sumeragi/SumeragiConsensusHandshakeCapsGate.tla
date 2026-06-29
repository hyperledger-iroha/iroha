---- MODULE SumeragiConsensusHandshakeCapsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for consensus handshake capability construction.

This slice captures the pure construction path that feeds `HandshakeGate`:
`npos_timeout_base_for_handshake_fingerprint_ms(...)`,
`consensus_genesis_params_from_parameters(...)`, and
`compute_consensus_handshake_caps_from_world(...)`. It abstracts the fingerprint
as a set of required preimage bindings instead of hashing bytes. The contract is
that runtime mode chooses the exact mode tag and BLS domain, handshake caps copy
the configured transport caps and protocol version, canonical genesis params
preserve consensus fields, NPoS epoch lengths are floored to one, permissioned
mode carries no NPoS payload, and NPoS timeout-base fingerprinting uses the
stable max(block_time.max(1), min_finality.max(1)) value.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Permissioned == "permissioned"
PermissionedDa == "permissioned_da"
NposCustomPositive == "npos_custom_positive_epoch"
NposCustomZero == "npos_custom_zero_epoch"
NposDefaultPositive == "npos_default_positive_epoch"
NposDefaultZero == "npos_default_zero_epoch"
NposTimeoutBlockDominates == "npos_timeout_block_dominates"
NposTimeoutFinalityDominates == "npos_timeout_finality_dominates"
NposTimeoutZeroFloor == "npos_timeout_zero_floor"

Cases == {
  Permissioned,
  PermissionedDa,
  NposCustomPositive,
  NposCustomZero,
  NposDefaultPositive,
  NposDefaultZero,
  NposTimeoutBlockDominates,
  NposTimeoutFinalityDominates,
  NposTimeoutZeroFloor
}

PermissionedCases == {Permissioned, PermissionedDa}
NposCases == Cases \ PermissionedCases
CustomNposCases == {NposCustomPositive, NposCustomZero}
DefaultNposCases == NposCases \ CustomNposCases
ZeroEpochNposCases == {NposCustomZero, NposDefaultZero}
PositiveEpochNposCases == NposCases \ ZeroEpochNposCases

ModePermissioned == 1
ModeNpos == 2
DomainPermissioned == 3
DomainNpos == 4
CapsProtoVersion == 5
CapsConfigCopied == 6
FingerprintBindsMode == 7
FingerprintBindsProto == 8
FingerprintBindsChain == 9
FingerprintBindsCanonParams == 10
CanonBindsBlsDomain == 11
CanonCopiesBlockTiming == 12
CanonCopiesCollectorConfig == 13
CanonCopiesBlockMaxTransactions == 14
CanonCopiesDaFlag == 15
NposAbsent == 16
NposPresent == 17
NposSourceCustom == 18
NposSourceDefault == 19
EpochZero == 20
EpochFloorOne == 21
EpochPositive == 22
TimeoutUsesBlockTime == 23
TimeoutUsesMinFinality == 24
TimeoutFloorOne == 25

Actions == 1..25

SpecActions(c) ==
  CASE c \in PermissionedCases ->
      {ModePermissioned, DomainPermissioned, CapsProtoVersion,
       CapsConfigCopied, FingerprintBindsMode, FingerprintBindsProto,
       FingerprintBindsChain, FingerprintBindsCanonParams,
       CanonBindsBlsDomain, CanonCopiesBlockTiming,
       CanonCopiesCollectorConfig, CanonCopiesBlockMaxTransactions,
       CanonCopiesDaFlag, NposAbsent, EpochZero}
    [] c \in CustomNposCases ->
      {ModeNpos, DomainNpos, CapsProtoVersion, CapsConfigCopied,
       FingerprintBindsMode, FingerprintBindsProto, FingerprintBindsChain,
       FingerprintBindsCanonParams, CanonBindsBlsDomain,
       CanonCopiesBlockTiming, CanonCopiesCollectorConfig,
       CanonCopiesBlockMaxTransactions, CanonCopiesDaFlag, NposPresent,
       NposSourceCustom} \cup
        IF c \in ZeroEpochNposCases THEN {EpochFloorOne} ELSE {EpochPositive}
    [] c \in DefaultNposCases ->
      {ModeNpos, DomainNpos, CapsProtoVersion, CapsConfigCopied,
       FingerprintBindsMode, FingerprintBindsProto, FingerprintBindsChain,
       FingerprintBindsCanonParams, CanonBindsBlsDomain,
       CanonCopiesBlockTiming, CanonCopiesCollectorConfig,
       CanonCopiesBlockMaxTransactions, CanonCopiesDaFlag, NposPresent,
       NposSourceDefault} \cup
        IF c \in ZeroEpochNposCases THEN {EpochFloorOne}
        ELSE IF c = NposTimeoutBlockDominates THEN {EpochPositive, TimeoutUsesBlockTime}
        ELSE IF c = NposTimeoutFinalityDominates THEN {EpochPositive, TimeoutUsesMinFinality}
        ELSE IF c = NposTimeoutZeroFloor THEN {EpochFloorOne, TimeoutFloorOne}
        ELSE {EpochPositive}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "npos_uses_permissioned_mode"
       /\ c \in NposCases ->
      (spec \ {ModeNpos}) \cup {ModePermissioned}
    [] Bug = "permissioned_uses_npos_domain"
       /\ c \in PermissionedCases ->
      (spec \ {DomainPermissioned}) \cup {DomainNpos}
    [] Bug = "npos_uses_permissioned_domain"
       /\ c \in NposCases ->
      (spec \ {DomainNpos}) \cup {DomainPermissioned}
    [] Bug = "caps_proto_zero" ->
      spec \ {CapsProtoVersion}
    [] Bug = "caps_config_not_copied" ->
      spec \ {CapsConfigCopied}
    [] Bug = "fingerprint_omits_mode" ->
      spec \ {FingerprintBindsMode}
    [] Bug = "fingerprint_omits_proto" ->
      spec \ {FingerprintBindsProto}
    [] Bug = "fingerprint_omits_chain" ->
      spec \ {FingerprintBindsChain}
    [] Bug = "fingerprint_omits_canon_params" ->
      spec \ {FingerprintBindsCanonParams}
    [] Bug = "canon_omits_bls_domain" ->
      spec \ {CanonBindsBlsDomain}
    [] Bug = "canon_omits_block_timing" ->
      spec \ {CanonCopiesBlockTiming}
    [] Bug = "canon_omits_collector_config" ->
      spec \ {CanonCopiesCollectorConfig}
    [] Bug = "canon_omits_block_max_transactions" ->
      spec \ {CanonCopiesBlockMaxTransactions}
    [] Bug = "canon_omits_da_flag" ->
      spec \ {CanonCopiesDaFlag}
    [] Bug = "permissioned_keeps_npos_payload"
       /\ c \in PermissionedCases ->
      (spec \ {NposAbsent}) \cup {NposPresent}
    [] Bug = "permissioned_epoch_nonzero"
       /\ c \in PermissionedCases ->
      (spec \ {EpochZero}) \cup {EpochPositive}
    [] Bug = "npos_missing_payload"
       /\ c \in NposCases ->
      (spec \ {NposPresent}) \cup {NposAbsent}
    [] Bug = "custom_npos_uses_defaults"
       /\ c \in CustomNposCases ->
      (spec \ {NposSourceCustom}) \cup {NposSourceDefault}
    [] Bug = "default_npos_uses_custom"
       /\ c \in DefaultNposCases ->
      (spec \ {NposSourceDefault}) \cup {NposSourceCustom}
    [] Bug = "custom_epoch_zero_not_floored"
       /\ c = NposCustomZero ->
      (spec \ {EpochFloorOne}) \cup {EpochZero}
    [] Bug = "default_epoch_zero_not_floored"
       /\ c \in {NposDefaultZero, NposTimeoutZeroFloor} ->
      (spec \ {EpochFloorOne}) \cup {EpochZero}
    [] Bug = "timeout_ignores_min_finality"
       /\ c = NposTimeoutFinalityDominates ->
      (spec \ {TimeoutUsesMinFinality}) \cup {TimeoutUsesBlockTime}
    [] Bug = "timeout_ignores_block_time"
       /\ c = NposTimeoutBlockDominates ->
      (spec \ {TimeoutUsesBlockTime}) \cup {TimeoutUsesMinFinality}
    [] Bug = "timeout_allows_zero"
       /\ c = NposTimeoutZeroFloor ->
      (spec \ {TimeoutFloorOne}) \cup {TimeoutUsesBlockTime}
    [] OTHER -> spec

Bugs == {
  "none",
  "npos_uses_permissioned_mode",
  "permissioned_uses_npos_domain",
  "npos_uses_permissioned_domain",
  "caps_proto_zero",
  "caps_config_not_copied",
  "fingerprint_omits_mode",
  "fingerprint_omits_proto",
  "fingerprint_omits_chain",
  "fingerprint_omits_canon_params",
  "canon_omits_bls_domain",
  "canon_omits_block_timing",
  "canon_omits_collector_config",
  "canon_omits_block_max_transactions",
  "canon_omits_da_flag",
  "permissioned_keeps_npos_payload",
  "permissioned_epoch_nonzero",
  "npos_missing_payload",
  "custom_npos_uses_defaults",
  "default_npos_uses_custom",
  "custom_epoch_zero_not_floored",
  "default_epoch_zero_not_floored",
  "timeout_ignores_min_finality",
  "timeout_ignores_block_time",
  "timeout_allows_zero"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

ConsensusHandshakeCapsCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ConsensusHandshakeCapsExactness ==
  /\ ConsensusHandshakeCapsCoreSafety

ConsensusHandshakeCapsCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ConsensusHandshakeCapsExactness

NoBugInvariant == ConsensusHandshakeCapsCoreSafety

SafetyFast ==
  ConsensusHandshakeCapsExactness

====

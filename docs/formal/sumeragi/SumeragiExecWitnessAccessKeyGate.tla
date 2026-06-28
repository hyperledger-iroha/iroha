---- MODULE SumeragiExecWitnessAccessKeyGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for execution-witness access-key parsing.

This slice captures `sumeragi::witness::record_read_from_access_key(...)`.
The recorder lifecycle and key encoding helpers are covered by
`SumeragiExecWitnessRecorderGate`; this model focuses on the deterministic
parser contract that chooses which pre-value witness key is recorded:

- supported access-key prefixes route to the intended witness namespace,
- malformed or unsupported keys are ignored,
- account ids are parsed from encoded text and then canonicalized,
- domain/asset-definition/NFT/role ids must parse before recording,
- metadata/detail reads record empty bytes when the entity or field is absent,
- role and permission probes record canonical JSON booleans,
- `perm.account` canonicalizes the account segment before keying,
- `perm.role` preserves the parsed role text segment used by the Rust key, and
- asset and asset-definition total reads record numeric JSON or empty bytes.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

AccountDetailPresent == 1
AccountDetailMissing == 2
DomainDetailPresent == 3
AssetDefDetailPresent == 4
NftDetailPresent == 5
RoleBindingPresent == 6
RoleBindingMissing == 7
RolePresent == 8
RoleMissing == 9
PermAccountPresent == 10
PermAccountMissing == 11
PermRolePresent == 12
PermRoleMissing == 13
AssetPresent == 14
AssetMissing == 15
AssetDefTotalPresent == 16
AssetDefTotalMissing == 17
UnsupportedKey == 18
MalformedSupportedKey == 19
InvalidParsedId == 20
AccountCanonicalization == 21
PermAccountCanonicalization == 22
PermRoleRawSegment == 23
SplitTailPreserved == 24
PrefixIsolation == 25

Cases == 1..25

AccountDetailPrefix == 1
AccountIdParsedEncoded == 2
AccountKeyCanonicalized == 3
AccountMetadataNameParsed == 4
AccountMissingRecordsEmpty == 5
DomainDetailPrefix == 6
DomainIdParsedFq == 7
DomainMissingRecordsEmpty == 8
AssetDefDetailPrefix == 9
AssetDefIdParsedAddress == 10
AssetDefMissingRecordsEmpty == 11
NftDetailPrefix == 12
NftIdParsed == 13
NftMissingRecordsEmpty == 14
RoleBindingPrefix == 15
RoleBindingParsesAccountRole == 16
RoleBindingRecordsBoolean == 17
RolePrefix == 18
RoleIdParsed == 19
RoleRecordsBoolean == 20
PermAccountPrefix == 21
PermAccountParsesAccount == 22
PermAccountCanonicalizesAccount == 23
PermAccountRecordsBoolean == 24
PermRolePrefix == 25
PermRoleParsesRole == 26
PermRoleKeepsRoleSegment == 27
PermRoleRecordsBoolean == 28
AssetPrefix == 29
AssetIdParsedLiteral == 30
AssetRecordsNumericOrEmpty == 31
AssetDefTotalPrefix == 32
AssetDefTotalParsesAddress == 33
AssetDefTotalRecordsNumericOrEmpty == 34
UnsupportedIgnored == 35
MalformedIgnored == 36
InvalidIdIgnored == 37
SplitNPreservesTail == 38
PrefixDoesNotFallThrough == 39

Actions == 1..39

SpecActions(candidate) ==
  CASE candidate = AccountDetailPresent ->
      {AccountDetailPrefix, AccountIdParsedEncoded, AccountKeyCanonicalized,
       AccountMetadataNameParsed}
    [] candidate = AccountDetailMissing ->
      {AccountDetailPrefix, AccountIdParsedEncoded, AccountKeyCanonicalized,
       AccountMissingRecordsEmpty}
    [] candidate = DomainDetailPresent ->
      {DomainDetailPrefix, DomainIdParsedFq, AccountMetadataNameParsed}
    [] candidate = AssetDefDetailPresent ->
      {AssetDefDetailPrefix, AssetDefIdParsedAddress, AccountMetadataNameParsed}
    [] candidate = NftDetailPresent ->
      {NftDetailPrefix, NftIdParsed, AccountMetadataNameParsed}
    [] candidate = RoleBindingPresent ->
      {RoleBindingPrefix, RoleBindingParsesAccountRole,
       RoleBindingRecordsBoolean}
    [] candidate = RoleBindingMissing ->
      {RoleBindingPrefix, RoleBindingParsesAccountRole,
       RoleBindingRecordsBoolean}
    [] candidate = RolePresent ->
      {RolePrefix, RoleIdParsed, RoleRecordsBoolean}
    [] candidate = RoleMissing ->
      {RolePrefix, RoleIdParsed, RoleRecordsBoolean}
    [] candidate = PermAccountPresent ->
      {PermAccountPrefix, PermAccountParsesAccount,
       PermAccountCanonicalizesAccount, PermAccountRecordsBoolean}
    [] candidate = PermAccountMissing ->
      {PermAccountPrefix, PermAccountParsesAccount,
       PermAccountCanonicalizesAccount, PermAccountRecordsBoolean}
    [] candidate = PermRolePresent ->
      {PermRolePrefix, PermRoleParsesRole, PermRoleKeepsRoleSegment,
       PermRoleRecordsBoolean}
    [] candidate = PermRoleMissing ->
      {PermRolePrefix, PermRoleParsesRole, PermRoleKeepsRoleSegment,
       PermRoleRecordsBoolean}
    [] candidate = AssetPresent ->
      {AssetPrefix, AssetIdParsedLiteral, AssetRecordsNumericOrEmpty}
    [] candidate = AssetMissing ->
      {AssetPrefix, AssetIdParsedLiteral, AssetRecordsNumericOrEmpty}
    [] candidate = AssetDefTotalPresent ->
      {AssetDefTotalPrefix, AssetDefTotalParsesAddress,
       AssetDefTotalRecordsNumericOrEmpty}
    [] candidate = AssetDefTotalMissing ->
      {AssetDefTotalPrefix, AssetDefTotalParsesAddress,
       AssetDefTotalRecordsNumericOrEmpty}
    [] candidate = UnsupportedKey ->
      {UnsupportedIgnored}
    [] candidate = MalformedSupportedKey ->
      {MalformedIgnored}
    [] candidate = InvalidParsedId ->
      {InvalidIdIgnored}
    [] candidate = AccountCanonicalization ->
      {AccountIdParsedEncoded, AccountKeyCanonicalized}
    [] candidate = PermAccountCanonicalization ->
      {PermAccountParsesAccount, PermAccountCanonicalizesAccount}
    [] candidate = PermRoleRawSegment ->
      {PermRoleParsesRole, PermRoleKeepsRoleSegment}
    [] candidate = SplitTailPreserved ->
      {SplitNPreservesTail}
    [] candidate = PrefixIsolation ->
      {PrefixDoesNotFallThrough}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = UnsupportedKey /\ Bug = "unsupported_records" ->
      spec \ {UnsupportedIgnored}
    [] candidate = MalformedSupportedKey /\ Bug = "malformed_records" ->
      spec \ {MalformedIgnored}
    [] candidate = InvalidParsedId /\ Bug = "invalid_id_records" ->
      spec \ {InvalidIdIgnored}
    [] candidate \in {AccountDetailPresent, AccountDetailMissing} /\
          Bug = "account_prefix_ignored" ->
      spec \ {AccountDetailPrefix}
    [] candidate = AccountCanonicalization /\
          Bug = "account_uses_raw_segment" ->
      spec \ {AccountKeyCanonicalized}
    [] candidate = AccountDetailPresent /\ Bug = "account_skips_name_parse" ->
      spec \ {AccountMetadataNameParsed}
    [] candidate = AccountDetailMissing /\
          Bug = "account_missing_records_nonempty" ->
      spec \ {AccountMissingRecordsEmpty}
    [] candidate = DomainDetailPresent /\ Bug = "domain_prefix_ignored" ->
      spec \ {DomainDetailPrefix}
    [] candidate = DomainDetailPresent /\ Bug = "domain_uses_account_parser" ->
      spec \ {DomainIdParsedFq}
    [] candidate = AssetDefDetailPresent /\
          Bug = "asset_def_detail_prefix_ignored" ->
      spec \ {AssetDefDetailPrefix}
    [] candidate = AssetDefDetailPresent /\
          Bug = "asset_def_detail_uses_total_key" ->
      spec \ {AssetDefIdParsedAddress}
    [] candidate = NftDetailPresent /\ Bug = "nft_prefix_ignored" ->
      spec \ {NftDetailPrefix}
    [] candidate = NftDetailPresent /\ Bug = "nft_skips_id_parse" ->
      spec \ {NftIdParsed}
    [] candidate \in {RoleBindingPresent, RoleBindingMissing} /\
          Bug = "role_binding_prefix_ignored" ->
      spec \ {RoleBindingPrefix}
    [] candidate \in {RoleBindingPresent, RoleBindingMissing} /\
          Bug = "role_binding_skips_boolean" ->
      spec \ {RoleBindingRecordsBoolean}
    [] candidate \in {RolePresent, RoleMissing} /\
          Bug = "role_prefix_ignored" ->
      spec \ {RolePrefix}
    [] candidate = RoleMissing /\ Bug = "role_missing_records_true" ->
      spec \ {RoleRecordsBoolean}
    [] candidate \in {PermAccountPresent, PermAccountMissing} /\
          Bug = "perm_account_prefix_ignored" ->
      spec \ {PermAccountPrefix}
    [] candidate = PermAccountCanonicalization /\
          Bug = "perm_account_uses_raw_segment" ->
      spec \ {PermAccountCanonicalizesAccount}
    [] candidate \in {PermAccountPresent, PermAccountMissing} /\
          Bug = "perm_account_skips_boolean" ->
      spec \ {PermAccountRecordsBoolean}
    [] candidate \in {PermRolePresent, PermRoleMissing} /\
          Bug = "perm_role_prefix_ignored" ->
      spec \ {PermRolePrefix}
    [] candidate = PermRoleRawSegment /\ Bug = "perm_role_canonicalizes_role" ->
      spec \ {PermRoleKeepsRoleSegment}
    [] candidate \in {PermRolePresent, PermRoleMissing} /\
          Bug = "perm_role_skips_boolean" ->
      spec \ {PermRoleRecordsBoolean}
    [] candidate \in {AssetPresent, AssetMissing} /\
          Bug = "asset_prefix_ignored" ->
      spec \ {AssetPrefix}
    [] candidate = AssetMissing /\ Bug = "asset_missing_records_nonempty" ->
      spec \ {AssetRecordsNumericOrEmpty}
    [] candidate \in {AssetDefTotalPresent, AssetDefTotalMissing} /\
          Bug = "asset_def_total_prefix_ignored" ->
      spec \ {AssetDefTotalPrefix}
    [] candidate = AssetDefTotalMissing /\
          Bug = "asset_def_total_missing_records_nonempty" ->
      spec \ {AssetDefTotalRecordsNumericOrEmpty}
    [] candidate = SplitTailPreserved /\ Bug = "split_tail_dropped" ->
      spec \ {SplitNPreservesTail}
    [] candidate = PrefixIsolation /\ Bug = "prefix_fallthrough_records_extra" ->
      spec \ {PrefixDoesNotFallThrough}
    [] OTHER -> spec

Bugs == {
  "none",
  "unsupported_records",
  "malformed_records",
  "invalid_id_records",
  "account_prefix_ignored",
  "account_uses_raw_segment",
  "account_skips_name_parse",
  "account_missing_records_nonempty",
  "domain_prefix_ignored",
  "domain_uses_account_parser",
  "asset_def_detail_prefix_ignored",
  "asset_def_detail_uses_total_key",
  "nft_prefix_ignored",
  "nft_skips_id_parse",
  "role_binding_prefix_ignored",
  "role_binding_skips_boolean",
  "role_prefix_ignored",
  "role_missing_records_true",
  "perm_account_prefix_ignored",
  "perm_account_uses_raw_segment",
  "perm_account_skips_boolean",
  "perm_role_prefix_ignored",
  "perm_role_canonicalizes_role",
  "perm_role_skips_boolean",
  "asset_prefix_ignored",
  "asset_missing_records_nonempty",
  "asset_def_total_prefix_ignored",
  "asset_def_total_missing_records_nonempty",
  "split_tail_dropped",
  "prefix_fallthrough_records_extra"
}

Init ==
  checked = 0

Next ==
  /\ checked < 25
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..25
  /\ \A candidate \in Cases:
       /\ SpecActions(candidate) \subseteq Actions
       /\ ImplementationActions(candidate) \subseteq Actions

ActionsMatchSpec ==
  \A candidate \in Cases:
    ImplementationActions(candidate) = SpecActions(candidate)

ExecWitnessAccessKeyExactness ==
  ActionsMatchSpec

ExecWitnessAccessKeyCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ExecWitnessAccessKeyExactness

Safety ==
  ExecWitnessAccessKeyExactness

====

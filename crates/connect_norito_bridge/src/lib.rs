//! FFI bridge exposing Norito/Connect helpers for the mobile SDKs and bridge targets.
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::missing_safety_doc)]

#[cfg(test)]
use core::ffi::c_void;
use std::{
    collections::HashSet,
    num::{NonZeroU32, NonZeroU64},
    path::PathBuf,
    ptr, slice,
    str::FromStr as _,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose as b64gp};
use blake3::hash as blake3_hash;
use iroha_crypto::{
    Algorithm, EcdsaSecp256k1Sha256, Error as CryptoError, Hash, KeyGenOption, KeyPair, PrivateKey,
    PublicKey, RamLfeBackend, RamLfeVerificationMode, Signature,
    kex::KeyExchangeScheme,
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature},
};
use iroha_data_model::{
    ChainId,
    account::{
        AccountId,
        address::{AccountAddress, AccountAddressError},
    },
    asset::id::{AssetBalanceScope, AssetDefinitionId, AssetId},
    confidential::{CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1, ConfidentialEncryptedPayload},
    da::manifest::DaManifestV1,
    domain::DomainId,
    governance::types::AtWindow,
    identifier::{IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload},
    isi::{
        InstructionBox, RemoveAssetKeyValue, RemoveKeyValue, SetAssetKeyValue, SetKeyValue,
        governance::{
            CastPlainBallot, CastZkBallot, CouncilDerivationKind, EnactReferendum,
            FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract, VotingMode,
        },
        identifier::ClaimIdentifier,
        mint_burn::{Burn, Mint},
        transfer::Transfer,
        zk,
    },
    metadata::Metadata,
    name::Name,
    nexus::DataSpaceId,
    proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    ram_lfe::RamLfeReceiptAttestation,
    ram_lfe::{RamLfeExecutionReceiptPayload, RamLfeProgramId},
    rwa::RwaId,
    smart_contract::manifest::ContractManifest,
    transaction::{
        Executable, SignedTransaction, TransactionSubmissionReceipt, signed::TransactionBuilder,
    },
};
use iroha_executor_data_model::isi::multisig::{MultisigRegister, MultisigSpec};
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_torii_shared::{connect as proto, connect_sdk};
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
use ivm::{AccelerationConfig, BackendRuntimeStatus};
use libc::{c_char, c_int, c_uchar, c_ulong, free, malloc};
use norito::decode_from_bytes;
use norito::json::{Map as JsonMap, Value as JsonValue};
use sorafs_car::{
    ChunkStore, ChunkStoreError, InMemoryPayload, PorProof, build_plan_from_da_manifest,
    local_fetch::{
        self, LocalFetchError, LocalFetchOptions, LocalFetchResult, LocalProviderInput,
        ProviderMetadataInput, RangeCapabilityInput, StreamBudgetInput, TelemetryEntryInput,
        TransportHintInput,
    },
};

const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 6;

const ERR_NULL_PTR: c_int = -1;
const ERR_UTF8: c_int = -2;
const ERR_CHAIN_ID_PARSE: c_int = -3;
const ERR_AUTHORITY_PARSE: c_int = -4;
const ERR_ASSET_DEFINITION_PARSE: c_int = -5;
const ERR_DESTINATION_PARSE: c_int = -6;
const ERR_QUANTITY_PARSE: c_int = -7;
const ERR_INVALID_TTL: c_int = -8;
const ERR_PRIVATE_KEY_PARSE: c_int = -9;
const ERR_ALLOC: c_int = -10;
const ERR_HASH_OUT_LEN: c_int = -11;
const ERR_BUFFER_TOO_SMALL: c_int = -12;
const ERR_SM2_DERIVE: c_int = -13;
const ERR_INVALID_NOTE_COMMITMENT: c_int = -14;
const ERR_CONFIDENTIAL_PAYLOAD: c_int = -15;
const ERR_SM2_VERIFY: c_int = -16;
const ERR_SM2_PARSE: c_int = -17;
const ERR_PROOF_ATTACHMENT: c_int = -18;
const ERR_INVALID_NULLIFIERS: c_int = -19;
const ERR_INVALID_ROOT_HINT: c_int = -20;
const ERR_UNSUPPORTED_ALGORITHM: c_int = -21;
const ERR_SECP_PARSE: c_int = -22;
const ERR_SECP_SIGN: c_int = -23;
const ERR_SECP_VERIFY: c_int = -24;
const ERR_METADATA_TARGET: c_int = -25;
const ERR_METADATA_KEY: c_int = -26;
const ERR_METADATA_VALUE: c_int = -27;
const ERR_GOVERNANCE: c_int = -28;
const ERR_HEX: c_int = -29;
const ERR_ACCOUNT_LIST: c_int = -30;
const ERR_INVALID_NONCE: c_int = -31;
const ERR_FETCH_PLAN_JSON: c_int = -100;
const ERR_FETCH_PROVIDERS_JSON: c_int = -101;
const ERR_FETCH_OPTIONS_JSON: c_int = -102;
const ERR_FETCH_NO_PROVIDERS: c_int = -103;
const ERR_FETCH_DUPLICATE_PROVIDER: c_int = -104;
const ERR_FETCH_PROVIDER_PATH_MISSING: c_int = -105;
const ERR_FETCH_PROVIDER_PATH_NOT_FILE: c_int = -106;
const ERR_FETCH_INVALID_MAX_CONCURRENT: c_int = -107;
const ERR_FETCH_INVALID_WEIGHT: c_int = -108;
const ERR_FETCH_SCOREBOARD_METADATA: c_int = -109;
const ERR_FETCH_SCOREBOARD_EXCLUDED: c_int = -110;
const ERR_FETCH_SCOREBOARD_BUILD: c_int = -111;
const ERR_FETCH_EXECUTION: c_int = -112;
const ERR_FETCH_UNKNOWN_CHUNKER: c_int = -113;
const ERR_ACCOUNT_ADDRESS: c_int = -200;
const ERR_ASSET_ID_PARSE: c_int = -301;
const ERR_JSON_SERIALIZE: c_int = -304;
const ERR_OFFLINE_NOTE_PROVE: c_int = -310;
const ERR_KAGEMUSHA_PROVE: c_int = -311;
const ERR_DA_PROOF_SUMMARY: c_int = -401;
const ERR_MULTISIG_SPEC: c_int = -402;
const ERR_VERIFYING_KEY_ID: c_int = -403;
const ERR_ZK_ASSET_MODE: c_int = -404;
const ERR_CONNECT_ENCODE: c_int = -405;
const ERR_IDENTIFIER_RECEIPT: c_int = -406;
const ERR_CONNECT_KEYPAIR: c_int = -407;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum BridgeError {
    NullPtr,
    Utf8,
    ChainId,
    Authority,
    AssetDefinition,
    Destination,
    Quantity,
    InvalidTtl,
    InvalidNonce,
    PrivateKey,
    Alloc,
    HashOutBuffer,
    InvalidNoteCommitment,
    ConfidentialPayload,
    ProofAttachment,
    InvalidNullifiers,
    InvalidRootHint,
    AssetId,
    JsonSerialize,
    OfflineNoteProve,
    KagemushaProve,
    UnsupportedAlgorithm,
    MetadataTarget,
    MetadataKey,
    MetadataValue,
    Governance,
    Hex,
    AccountList,
    MultisigSpec,
    IdentifierReceipt,
    VerifyingKeyId,
    ZkAssetMode,
    SecpParse,
    SecpSign,
    SecpVerify,
}

impl BridgeError {
    const fn code(self) -> c_int {
        match self {
            BridgeError::NullPtr => ERR_NULL_PTR,
            BridgeError::Utf8 => ERR_UTF8,
            BridgeError::ChainId => ERR_CHAIN_ID_PARSE,
            BridgeError::Authority => ERR_AUTHORITY_PARSE,
            BridgeError::AssetDefinition => ERR_ASSET_DEFINITION_PARSE,
            BridgeError::Destination => ERR_DESTINATION_PARSE,
            BridgeError::Quantity => ERR_QUANTITY_PARSE,
            BridgeError::InvalidTtl => ERR_INVALID_TTL,
            BridgeError::InvalidNonce => ERR_INVALID_NONCE,
            BridgeError::PrivateKey => ERR_PRIVATE_KEY_PARSE,
            BridgeError::Alloc => ERR_ALLOC,
            BridgeError::HashOutBuffer => ERR_HASH_OUT_LEN,
            BridgeError::InvalidNoteCommitment => ERR_INVALID_NOTE_COMMITMENT,
            BridgeError::ConfidentialPayload => ERR_CONFIDENTIAL_PAYLOAD,
            BridgeError::ProofAttachment => ERR_PROOF_ATTACHMENT,
            BridgeError::InvalidNullifiers => ERR_INVALID_NULLIFIERS,
            BridgeError::InvalidRootHint => ERR_INVALID_ROOT_HINT,
            BridgeError::AssetId => ERR_ASSET_ID_PARSE,
            BridgeError::JsonSerialize => ERR_JSON_SERIALIZE,
            BridgeError::OfflineNoteProve => ERR_OFFLINE_NOTE_PROVE,
            BridgeError::KagemushaProve => ERR_KAGEMUSHA_PROVE,
            BridgeError::UnsupportedAlgorithm => ERR_UNSUPPORTED_ALGORITHM,
            BridgeError::MetadataTarget => ERR_METADATA_TARGET,
            BridgeError::MetadataKey => ERR_METADATA_KEY,
            BridgeError::MetadataValue => ERR_METADATA_VALUE,
            BridgeError::Governance => ERR_GOVERNANCE,
            BridgeError::Hex => ERR_HEX,
            BridgeError::AccountList => ERR_ACCOUNT_LIST,
            BridgeError::MultisigSpec => ERR_MULTISIG_SPEC,
            BridgeError::IdentifierReceipt => ERR_IDENTIFIER_RECEIPT,
            BridgeError::VerifyingKeyId => ERR_VERIFYING_KEY_ID,
            BridgeError::ZkAssetMode => ERR_ZK_ASSET_MODE,
            BridgeError::SecpParse => ERR_SECP_PARSE,
            BridgeError::SecpSign => ERR_SECP_SIGN,
            BridgeError::SecpVerify => ERR_SECP_VERIFY,
        }
    }
}

type BridgeResult<T> = Result<T, BridgeError>;

/// Return the current native bridge C ABI version.
///
/// Clients that resolve symbols dynamically must check this before calling other
/// entrypoints; stale bridge artifacts can otherwise crash before Rust receives
/// enough arguments to validate the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_bridge_abi_version() -> u32 {
    CONNECT_NORITO_BRIDGE_ABI_VERSION
}

fn account_address_error_fields(err: &AccountAddressError) -> Option<JsonMap> {
    use AccountAddressError::*;

    let mut fields = JsonMap::new();
    match err {
        UnsupportedAlgorithm(algorithm) => {
            fields.insert("algorithm".into(), JsonValue::from(algorithm.to_string()));
        }
        KeyPayloadTooLong(len) => {
            fields.insert("length".into(), JsonValue::from(u64::from(*len)));
        }
        InvalidHeaderVersion(value) => {
            fields.insert("value".into(), JsonValue::from(u64::from(*value)));
        }
        InvalidNormVersion(value) => {
            fields.insert("value".into(), JsonValue::from(u64::from(*value)));
        }
        InvalidDomainLabel(label) => {
            fields.insert("label".into(), JsonValue::from(label.to_string()));
        }
        UnexpectedNetworkPrefix { expected, found } => {
            fields.insert("expected".into(), JsonValue::from(u64::from(*expected)));
            fields.insert("found".into(), JsonValue::from(u64::from(*found)));
        }
        UnknownAddressClass(value) | UnknownControllerTag(value) => {
            fields.insert("value".into(), JsonValue::from(u64::from(*value)));
        }
        UnknownCurve(value) => {
            fields.insert("value".into(), JsonValue::from(u64::from(*value)));
        }
        InvalidI105Char(ch) => {
            fields.insert("char".into(), JsonValue::from(ch.to_string()));
        }
        MultisigMemberOverflow(count) => {
            fields.insert("count".into(), JsonValue::from(*count as u64));
        }
        InvalidMultisigPolicy(policy) => {
            fields.insert("policy_error".into(), JsonValue::from(policy.to_string()));
        }
        _ => {}
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

fn encode_account_address_error(err: AccountAddressError) -> Vec<u8> {
    let mut map = JsonMap::new();
    map.insert("code".into(), JsonValue::from(err.code_str()));
    map.insert("message".into(), JsonValue::from(err.to_string()));
    if let Some(fields) = account_address_error_fields(&err) {
        map.insert("fields".into(), JsonValue::Object(fields));
    }
    norito::json::to_vec(&JsonValue::Object(map))
        .unwrap_or_else(|_| b"{\"code\":\"ERR_ADDRESS_PARSE\"}".to_vec())
}

fn write_account_address_error(
    err: AccountAddressError,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    if !out_ptr.is_null() {
        unsafe { *out_ptr = ptr::null_mut() };
    }
    if !out_len.is_null() {
        unsafe { *out_len = 0 };
    }
    if out_ptr.is_null() || out_len.is_null() {
        return ERR_ACCOUNT_ADDRESS;
    }
    let payload = encode_account_address_error(err);
    match unsafe { write_bytes(out_ptr, out_len, &payload) } {
        Ok(()) => ERR_ACCOUNT_ADDRESS,
        Err(code) => code,
    }
}

unsafe fn read_string_bridge(ptr: *const c_char, len: c_ulong) -> BridgeResult<String> {
    if ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let slice = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    let s = std::str::from_utf8(slice).map_err(|_| BridgeError::Utf8)?;
    Ok(s.to_owned())
}

unsafe fn write_bytes(
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
    bytes: &[u8],
) -> Result<(), c_int> {
    if out_ptr.is_null() || out_len.is_null() {
        return Err(ERR_NULL_PTR);
    }
    let len = bytes.len();
    if len == 0 {
        unsafe {
            *out_ptr = ptr::null_mut();
            *out_len = 0;
        }
        return Ok(());
    }
    let mem = unsafe { malloc(len) };
    if mem.is_null() {
        return Err(ERR_ALLOC);
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
    }
    Ok(())
}

unsafe fn write_bytes_bridge(
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
    bytes: &[u8],
) -> BridgeResult<()> {
    unsafe { write_bytes(out_ptr, out_len, bytes) }.map_err(|code| match code {
        ERR_NULL_PTR => BridgeError::NullPtr,
        ERR_ALLOC => BridgeError::Alloc,
        _ => BridgeError::Alloc,
    })
}

fn parse_account_id(value: String) -> BridgeResult<AccountId> {
    AccountId::parse_encoded(&value)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| BridgeError::Authority)
}

fn parse_destination(value: String) -> BridgeResult<AccountId> {
    AccountId::parse_encoded(&value)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| BridgeError::Destination)
}

fn parse_asset_definition(value: String) -> BridgeResult<AssetDefinitionId> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BridgeError::AssetDefinition);
    }

    AssetDefinitionId::parse_address_literal(trimmed).map_err(|_| BridgeError::AssetDefinition)
}

fn parse_asset_definition_with_balance_scope(
    value: String,
) -> BridgeResult<(AssetDefinitionId, AssetBalanceScope)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BridgeError::AssetDefinition);
    }
    let Some((definition_literal, scope_literal)) = trimmed.split_once("#dataspace:") else {
        return parse_asset_definition(trimmed.to_owned())
            .map(|definition| (definition, AssetBalanceScope::Global));
    };
    if definition_literal.is_empty() || scope_literal.is_empty() || scope_literal.contains('#') {
        return Err(BridgeError::AssetDefinition);
    }
    let definition = parse_asset_definition(definition_literal.to_owned())?;
    let dataspace_id = scope_literal
        .parse::<u64>()
        .map(DataSpaceId::new)
        .map_err(|_| BridgeError::AssetDefinition)?;
    Ok((definition, AssetBalanceScope::Dataspace(dataspace_id)))
}

fn parse_quantity(value: String) -> BridgeResult<Numeric> {
    Numeric::from_str(&value).map_err(|_| BridgeError::Quantity)
}

fn parse_private_key(bytes: &[u8]) -> BridgeResult<PrivateKey> {
    parse_private_key_with_algorithm(bytes, Algorithm::Ed25519)
}

fn parse_private_key_with_algorithm(
    bytes: &[u8],
    algorithm: Algorithm,
) -> BridgeResult<PrivateKey> {
    PrivateKey::from_bytes(algorithm, bytes).map_err(|_| BridgeError::PrivateKey)
}

fn parse_algorithm_code(code: u8) -> BridgeResult<Algorithm> {
    Algorithm::try_from(code).map_err(|_| BridgeError::UnsupportedAlgorithm)
}

fn checked_public_key_payload(public_key: &PublicKey) -> BridgeResult<&[u8]> {
    public_key
        .try_to_bytes()
        .map(|(_algorithm, payload)| payload)
        .map_err(|_| BridgeError::PrivateKey)
}

fn parse_ttl(ttl_ms: u64, present: bool) -> BridgeResult<Option<NonZeroU64>> {
    if !present {
        return Ok(None);
    }
    NonZeroU64::new(ttl_ms)
        .map(Some)
        .ok_or(BridgeError::InvalidTtl)
}

fn parse_nonce(nonce: u32, present: bool) -> BridgeResult<Option<NonZeroU32>> {
    if !present {
        return Ok(None);
    }
    NonZeroU32::new(nonce)
        .map(Some)
        .ok_or(BridgeError::InvalidNonce)
}

fn parse_zk_asset_mode(code: u8) -> BridgeResult<zk::ZkAssetMode> {
    match code {
        0 => Ok(zk::ZkAssetMode::ZkNative),
        1 => Ok(zk::ZkAssetMode::Hybrid),
        _ => Err(BridgeError::ZkAssetMode),
    }
}

fn parse_voting_mode(code: u8) -> BridgeResult<VotingMode> {
    match code {
        0 => Ok(VotingMode::Zk),
        1 => Ok(VotingMode::Plain),
        _ => Err(BridgeError::Governance),
    }
}

fn parse_name(value: String) -> BridgeResult<Name> {
    Name::from_str(&value).map_err(|_| BridgeError::MetadataKey)
}

unsafe fn parse_optional_account_id_bridge(
    ptr: *const c_char,
    len: c_ulong,
) -> BridgeResult<Option<AccountId>> {
    if ptr.is_null() || len == 0 {
        return Ok(None);
    }
    let raw = unsafe { read_string_bridge(ptr, len)? };
    parse_account_id(raw).map(Some)
}

fn build_fee_sponsor_metadata(fee_sponsor: Option<AccountId>) -> Metadata {
    let mut metadata = Metadata::default();
    if let Some(fee_sponsor) = fee_sponsor {
        metadata.insert(
            Name::from_str("fee_sponsor").expect("fee_sponsor is a valid metadata key"),
            Json::new(fee_sponsor.to_string()),
        );
    }
    metadata
}

fn parse_json_value(bytes: &[u8]) -> BridgeResult<Json> {
    let value: norito::json::Value =
        norito::json::from_slice(bytes).map_err(|_| BridgeError::MetadataValue)?;
    Json::from_norito_value_ref(&value).map_err(|_| BridgeError::MetadataValue)
}

fn normalize_zk_ballot_public_inputs(value: &mut JsonValue) -> BridgeResult<()> {
    let map = match value {
        JsonValue::Object(map) => map,
        _ => return Err(BridgeError::Governance),
    };
    reject_zk_public_input_key(map, "durationBlocks", "duration_blocks")?;
    reject_zk_public_input_key(map, "root_hint_hex", "root_hint")?;
    reject_zk_public_input_key(map, "rootHintHex", "root_hint")?;
    reject_zk_public_input_key(map, "rootHint", "root_hint")?;
    reject_zk_public_input_key(map, "nullifier_hex", "nullifier")?;
    reject_zk_public_input_key(map, "nullifierHex", "nullifier")?;
    canonicalize_hex32_public_input(map, "root_hint")?;
    canonicalize_hex32_public_input(map, "nullifier")?;
    let has_owner = zk_hint_present(map, "owner");
    let has_amount = zk_hint_present(map, "amount");
    let has_duration = zk_hint_present(map, "duration_blocks");
    let any = has_owner || has_amount || has_duration;
    if any && !(has_owner && has_amount && has_duration) {
        return Err(BridgeError::Governance);
    }
    ensure_zk_public_input_owner_canonical(map)?;
    Ok(())
}

fn reject_zk_public_input_key(map: &JsonMap, key: &str, _canonical: &str) -> BridgeResult<()> {
    if map.contains_key(key) {
        return Err(BridgeError::Governance);
    }
    Ok(())
}

fn ensure_zk_public_input_owner_canonical(map: &JsonMap) -> BridgeResult<()> {
    let Some(value) = map.get("owner") else {
        return Ok(());
    };
    if matches!(value, JsonValue::Null) {
        return Ok(());
    }
    let owner = value.as_str().ok_or(BridgeError::Governance)?;
    let canonical = AccountId::canonicalize(owner).map_err(|_| BridgeError::Governance)?;
    if canonical != owner {
        return Err(BridgeError::Governance);
    }
    Ok(())
}

fn canonicalize_hex32_public_input(map: &mut JsonMap, key: &str) -> BridgeResult<()> {
    let Some(value) = map.get_mut(key) else {
        return Ok(());
    };
    if matches!(value, JsonValue::Null) {
        return Ok(());
    }
    let raw = value.as_str().ok_or(BridgeError::Governance)?;
    let canonical = canonicalize_hex32_value(raw).ok_or(BridgeError::Governance)?;
    *value = JsonValue::String(canonical);
    Ok(())
}

fn canonicalize_hex32_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
        if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
            rest
        } else {
            return None;
        }
    } else {
        trimmed
    };
    let body = without_scheme.trim();
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .unwrap_or(body)
        .trim();
    if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(body.to_ascii_lowercase())
}

fn zk_hint_present(map: &JsonMap, key: &str) -> bool {
    map.get(key)
        .map(|value| !matches!(value, JsonValue::Null))
        .unwrap_or(false)
}

fn parse_hex_32(hex_str: &str) -> BridgeResult<[u8; 32]> {
    let bytes = hex::decode(hex_str).map_err(|_| BridgeError::Hex)?;
    bytes.try_into().map_err(|_| BridgeError::Hex)
}

fn parse_account_list(bytes: &[u8]) -> BridgeResult<Vec<AccountId>> {
    let raw: Vec<String> = norito::json::from_slice(bytes).map_err(|_| BridgeError::AccountList)?;
    raw.into_iter().map(parse_account_id).collect()
}

enum MetadataTarget {
    Domain(DomainId),
    Account(AccountId),
    Rwa(RwaId),
    AssetDefinition(AssetDefinitionId),
    Asset(AssetId),
}

fn parse_metadata_target(kind: u8, object: String) -> BridgeResult<MetadataTarget> {
    match kind {
        0 => DomainId::parse_fully_qualified(&object)
            .map(MetadataTarget::Domain)
            .map_err(|_| BridgeError::MetadataTarget),
        1 => parse_account_id(object).map(MetadataTarget::Account),
        4 => object
            .parse::<RwaId>()
            .map(MetadataTarget::Rwa)
            .map_err(|_| BridgeError::MetadataTarget),
        2 => parse_asset_definition(object).map(MetadataTarget::AssetDefinition),
        3 => AssetId::parse_literal(&object)
            .map(MetadataTarget::Asset)
            .map_err(|_| BridgeError::MetadataTarget),
        _ => Err(BridgeError::MetadataTarget),
    }
}

fn build_set_metadata_instruction(
    target: MetadataTarget,
    key: Name,
    value: Json,
) -> InstructionBox {
    match target {
        MetadataTarget::Domain(id) => InstructionBox::from(SetKeyValue::domain(id, key, value)),
        MetadataTarget::Account(id) => InstructionBox::from(SetKeyValue::account(id, key, value)),
        MetadataTarget::Rwa(id) => InstructionBox::from(SetKeyValue::rwa(id, key, value)),
        MetadataTarget::AssetDefinition(id) => {
            InstructionBox::from(SetKeyValue::asset_definition(id, key, value))
        }
        MetadataTarget::Asset(id) => InstructionBox::from(SetAssetKeyValue::new(id, key, value)),
    }
}

fn build_remove_metadata_instruction(target: MetadataTarget, key: Name) -> InstructionBox {
    match target {
        MetadataTarget::Domain(id) => InstructionBox::from(RemoveKeyValue::domain(id, key)),
        MetadataTarget::Account(id) => InstructionBox::from(RemoveKeyValue::account(id, key)),
        MetadataTarget::Rwa(id) => InstructionBox::from(RemoveKeyValue::rwa(id, key)),
        MetadataTarget::AssetDefinition(id) => {
            InstructionBox::from(RemoveKeyValue::asset_definition(id, key))
        }
        MetadataTarget::Asset(id) => InstructionBox::from(RemoveAssetKeyValue::new(id, key)),
    }
}

fn parse_verifying_key_id_value(value: &str) -> BridgeResult<VerifyingKeyId> {
    let trimmed = value.trim();
    let (backend, name) = trimmed.split_once(':').ok_or(BridgeError::VerifyingKeyId)?;
    if backend.is_empty() || name.is_empty() {
        return Err(BridgeError::VerifyingKeyId);
    }
    Ok(VerifyingKeyId::new(backend, name))
}

unsafe fn parse_optional_verifying_key_id(
    ptr: *const c_char,
    len: c_ulong,
    present: c_uchar,
) -> BridgeResult<Option<VerifyingKeyId>> {
    if present == 0 {
        return Ok(None);
    }
    let raw = unsafe { read_string_bridge(ptr, len) }?;
    if raw.trim().is_empty() {
        return Err(BridgeError::VerifyingKeyId);
    }
    parse_verifying_key_id_value(&raw).map(Some)
}

fn write_hash(
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
    hash: &[u8; 32],
) -> BridgeResult<()> {
    if out_hash_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    if out_hash_len < hash.len() as c_ulong {
        return Err(BridgeError::HashOutBuffer);
    }
    unsafe {
        ptr::copy_nonoverlapping(hash.as_ptr(), out_hash_ptr, hash.len());
    }
    Ok(())
}

fn bridge_result_to_code(result: BridgeResult<()>) -> c_int {
    match result {
        Ok(()) => 0,
        Err(err) => err.code(),
    }
}

fn parse_multisig_spec_bytes(ptr: *const c_char, len: c_ulong) -> BridgeResult<MultisigSpec> {
    if ptr.is_null() || len == 0 {
        return Err(BridgeError::MultisigSpec);
    }
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    norito::json::from_slice::<MultisigSpec>(bytes).map_err(|_| BridgeError::MultisigSpec)
}

fn parse_identifier_receipt_bytes(
    ptr: *const c_char,
    len: c_ulong,
) -> BridgeResult<IdentifierResolutionReceipt> {
    if ptr.is_null() || len == 0 {
        return Err(BridgeError::IdentifierReceipt);
    }
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    let value =
        norito::json::from_slice::<JsonValue>(bytes).map_err(|_| BridgeError::IdentifierReceipt)?;
    parse_identifier_receipt_value(value)
}

fn parse_identifier_receipt_value(value: JsonValue) -> BridgeResult<IdentifierResolutionReceipt> {
    let JsonValue::Object(object) = value else {
        return Err(BridgeError::IdentifierReceipt);
    };

    let payload = parse_identifier_receipt_payload_value(
        object
            .get("payload")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let attestation = parse_identifier_receipt_attestation(
        object
            .get("attestation")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    Ok(IdentifierResolutionReceipt {
        payload,
        attestation,
    })
}

fn parse_identifier_receipt_attestation(
    value: &JsonValue,
) -> BridgeResult<RamLfeReceiptAttestation> {
    let object = value.as_object().ok_or(BridgeError::IdentifierReceipt)?;
    let kind = object
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::IdentifierReceipt)?;
    match kind {
        "signed" => parse_identifier_receipt_signature(object.get("signature"))
            .map(RamLfeReceiptAttestation::Signed),
        "proof" => {
            let proof_backend = object
                .get("proof_backend")
                .and_then(JsonValue::as_str)
                .ok_or(BridgeError::IdentifierReceipt)?;
            let proof_b64 = object
                .get("proof_b64")
                .and_then(JsonValue::as_str)
                .ok_or(BridgeError::IdentifierReceipt)?;
            let bytes = b64gp::STANDARD
                .decode(proof_b64.trim())
                .map_err(|_| BridgeError::IdentifierReceipt)?;
            Ok(RamLfeReceiptAttestation::Proof(ProofBox::new(
                proof_backend.trim().to_owned(),
                bytes,
            )))
        }
        _ => Err(BridgeError::IdentifierReceipt),
    }
}

fn parse_identifier_receipt_signature(value: Option<&JsonValue>) -> BridgeResult<Signature> {
    let signature_hex = value
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::IdentifierReceipt)?;
    let signature_bytes = decode_identifier_receipt_hex(signature_hex)?;
    Ok(Signature::from_bytes(&signature_bytes))
}

fn parse_identifier_receipt_payload_value(
    value: &JsonValue,
) -> BridgeResult<IdentifierResolutionReceiptPayload> {
    let object = value.as_object().ok_or(BridgeError::IdentifierReceipt)?;
    let policy_id = parse_identifier_policy_id_value(
        object
            .get("policy_id")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let execution = parse_identifier_execution_payload_value(
        object
            .get("execution")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let opening = parse_identifier_output_opening_value(
        object
            .get("opening")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let opaque_id = parse_identifier_opaque_id_value(
        object
            .get("opaque_id")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let receipt_hash = parse_identifier_hash_value(
        object
            .get("receipt_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let uaid =
        parse_identifier_uaid_value(object.get("uaid").ok_or(BridgeError::IdentifierReceipt)?)?;
    let account_id = object
        .get("account_id")
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or(BridgeError::IdentifierReceipt)
        .and_then(parse_account_id)?;

    Ok(IdentifierResolutionReceiptPayload {
        policy_id,
        execution,
        opening,
        opaque_id,
        receipt_hash,
        uaid,
        account_id,
    })
}

fn parse_identifier_output_opening_value(
    value: &JsonValue,
) -> BridgeResult<iroha_data_model::ram_lfe::RamLfeOutputOpening> {
    let object = value.as_object().ok_or(BridgeError::IdentifierReceipt)?;
    let payload_value = object
        .get("payload")
        .ok_or(BridgeError::IdentifierReceipt)?;
    let payload_object = payload_value
        .as_object()
        .ok_or(BridgeError::IdentifierReceipt)?;
    let program_id = parse_identifier_program_id_value(
        payload_object
            .get("program_id")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let input_ciphertext_hash = parse_identifier_hash_value(
        payload_object
            .get("input_ciphertext_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let output_ciphertext_hash = parse_identifier_hash_value(
        payload_object
            .get("output_ciphertext_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let parameter_digest = parse_identifier_hash_value(
        payload_object
            .get("parameter_digest")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let evaluation_key_digest = parse_identifier_hash_value(
        payload_object
            .get("evaluation_key_digest")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let opened_output_hash = parse_identifier_hash_value(
        payload_object
            .get("opened_output_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let opened_at_ms = payload_object
        .get("opened_at_ms")
        .and_then(JsonValue::as_u64)
        .ok_or(BridgeError::IdentifierReceipt)?;
    let expires_at_ms = payload_object
        .get("expires_at_ms")
        .and_then(JsonValue::as_u64);
    let signature = parse_identifier_receipt_signature(object.get("signature"))?;
    Ok(iroha_data_model::ram_lfe::RamLfeOutputOpening {
        payload: iroha_data_model::ram_lfe::RamLfeOutputOpeningPayload {
            program_id,
            input_ciphertext_hash,
            output_ciphertext_hash,
            parameter_digest,
            evaluation_key_digest,
            opened_output_hash,
            opened_at_ms,
            expires_at_ms,
        },
        signature,
    })
}

fn parse_identifier_policy_id_value(
    value: &JsonValue,
) -> BridgeResult<iroha_data_model::identifier::IdentifierPolicyId> {
    if let Some(literal) = value.as_str() {
        return literal
            .trim()
            .parse()
            .map_err(|_| BridgeError::IdentifierReceipt);
    }
    let object = value.as_object().ok_or(BridgeError::IdentifierReceipt)?;
    let kind = object
        .get("kind")
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::IdentifierReceipt)?;
    let business_rule = object
        .get("business_rule")
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::IdentifierReceipt)?;
    format!("{}#{}", kind.trim(), business_rule.trim())
        .parse()
        .map_err(|_| BridgeError::IdentifierReceipt)
}

fn parse_identifier_program_id_value(value: &JsonValue) -> BridgeResult<RamLfeProgramId> {
    if let Some(literal) = value.as_str() {
        return literal
            .trim()
            .parse()
            .map_err(|_| BridgeError::IdentifierReceipt);
    }
    let object = value.as_object().ok_or(BridgeError::IdentifierReceipt)?;
    object
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::IdentifierReceipt)?
        .trim()
        .parse()
        .map_err(|_| BridgeError::IdentifierReceipt)
}

fn parse_identifier_receipt_backend(value: &JsonValue) -> BridgeResult<RamLfeBackend> {
    let backend = value
        .as_str()
        .ok_or(BridgeError::IdentifierReceipt)?
        .trim()
        .to_ascii_lowercase();
    match backend.as_str() {
        "hkdf-sha3-512-prf-v1" => Ok(RamLfeBackend::HkdfSha3_512PrfV1),
        "bfv-affine-sha3-256-v1" => Ok(RamLfeBackend::BfvAffineSha3_256V1),
        "bfv-programmed-sha3-256-v1" => Ok(RamLfeBackend::BfvProgrammedSha3_256V1),
        _ => Err(BridgeError::IdentifierReceipt),
    }
}

fn parse_identifier_receipt_verification_mode(
    value: &JsonValue,
) -> BridgeResult<RamLfeVerificationMode> {
    let mode = if let Some(literal) = value.as_str() {
        literal.trim().to_ascii_lowercase()
    } else {
        value
            .as_object()
            .and_then(|object| object.get("mode"))
            .and_then(JsonValue::as_str)
            .map(|literal| literal.trim().to_ascii_lowercase())
            .ok_or(BridgeError::IdentifierReceipt)?
    };
    match mode.as_str() {
        "signed" => Ok(RamLfeVerificationMode::Signed),
        "proof" => Ok(RamLfeVerificationMode::Proof),
        _ => Err(BridgeError::IdentifierReceipt),
    }
}

fn parse_identifier_hash_str(value: &str) -> BridgeResult<Hash> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BridgeError::IdentifierReceipt);
    }
    let body = if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("hash:"))
    {
        norito::literal::parse("hash", trimmed).map_err(|_| BridgeError::IdentifierReceipt)?
    } else {
        trimmed
    };
    Hash::from_str(body).map_err(|_| BridgeError::IdentifierReceipt)
}

fn parse_identifier_hash_value(value: &JsonValue) -> BridgeResult<Hash> {
    value
        .as_str()
        .ok_or(BridgeError::IdentifierReceipt)
        .and_then(parse_identifier_hash_str)
}

fn parse_identifier_opaque_id_value(
    value: &JsonValue,
) -> BridgeResult<iroha_data_model::account::OpaqueAccountId> {
    value
        .as_str()
        .ok_or(BridgeError::IdentifierReceipt)?
        .parse()
        .map_err(|_| BridgeError::IdentifierReceipt)
}

fn parse_identifier_uaid_value(
    value: &JsonValue,
) -> BridgeResult<iroha_data_model::nexus::UniversalAccountId> {
    value
        .as_str()
        .ok_or(BridgeError::IdentifierReceipt)?
        .parse()
        .map_err(|_| BridgeError::IdentifierReceipt)
}

fn parse_identifier_execution_payload_value(
    value: &JsonValue,
) -> BridgeResult<RamLfeExecutionReceiptPayload> {
    let object = value.as_object().ok_or(BridgeError::IdentifierReceipt)?;
    let program_id = parse_identifier_program_id_value(
        object
            .get("program_id")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let program_digest = parse_identifier_hash_value(
        object
            .get("program_digest")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let backend = parse_identifier_receipt_backend(
        object
            .get("backend")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let verification_mode = parse_identifier_receipt_verification_mode(
        object
            .get("verification_mode")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let output_hash = parse_identifier_hash_value(
        object
            .get("output_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let input_ciphertext_hash = parse_identifier_hash_value(
        object
            .get("input_ciphertext_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let output_ciphertext_hash = parse_identifier_hash_value(
        object
            .get("output_ciphertext_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let parameter_digest = parse_identifier_hash_value(
        object
            .get("parameter_digest")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let evaluation_key_digest = parse_identifier_hash_value(
        object
            .get("evaluation_key_digest")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let associated_data_hash = parse_identifier_hash_value(
        object
            .get("associated_data_hash")
            .ok_or(BridgeError::IdentifierReceipt)?,
    )?;
    let executed_at_ms = object
        .get("executed_at_ms")
        .and_then(JsonValue::as_u64)
        .ok_or(BridgeError::IdentifierReceipt)?;
    let expires_at_ms = object.get("expires_at_ms").and_then(JsonValue::as_u64);

    Ok(RamLfeExecutionReceiptPayload {
        program_id,
        program_digest,
        backend,
        verification_mode,
        input_ciphertext_hash,
        output_ciphertext_hash,
        parameter_digest,
        evaluation_key_digest,
        output_hash,
        associated_data_hash,
        executed_at_ms,
        expires_at_ms,
    })
}

fn decode_identifier_receipt_hex(value: &str) -> BridgeResult<Vec<u8>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BridgeError::IdentifierReceipt);
    }
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        return Err(BridgeError::IdentifierReceipt);
    }
    hex::decode(trimmed).map_err(|_| BridgeError::IdentifierReceipt)
}

fn write_optional_error(out_ptr: *mut *mut c_uchar, out_len: *mut c_ulong) {
    if !out_ptr.is_null() {
        unsafe { *out_ptr = ptr::null_mut() };
    }
    if !out_len.is_null() {
        unsafe { *out_len = 0 };
    }
}

// ---------------- Signing helpers ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_public_key_from_private(
    algorithm_code: u8,
    private_ptr: *const c_uchar,
    private_len: c_ulong,
    out_public_ptr: *mut *mut c_uchar,
    out_public_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if private_ptr.is_null() || out_public_ptr.is_null() || out_public_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let private_bytes = unsafe { slice::from_raw_parts(private_ptr, private_len as usize) };
        let private_key = parse_private_key_with_algorithm(private_bytes, algorithm)?;
        let key_pair =
            KeyPair::from_private_key(private_key).map_err(|_| BridgeError::PrivateKey)?;
        let public_bytes = checked_public_key_payload(key_pair.public_key())?;
        unsafe { write_bytes_bridge(out_public_ptr, out_public_len, public_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_keypair_from_seed(
    algorithm_code: u8,
    seed_ptr: *const c_uchar,
    seed_len: c_ulong,
    out_private_ptr: *mut *mut c_uchar,
    out_private_len: *mut c_ulong,
    out_public_ptr: *mut *mut c_uchar,
    out_public_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if seed_ptr.is_null()
            || out_private_ptr.is_null()
            || out_private_len.is_null()
            || out_public_ptr.is_null()
            || out_public_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let seed_bytes = unsafe { slice::from_raw_parts(seed_ptr, seed_len as usize) };
        let key_pair = KeyPair::from_seed(seed_bytes.to_vec(), algorithm);
        let (public_key, private_key) = key_pair.into_parts();
        let (_alg, private_bytes) = private_key.to_bytes();
        let public_bytes = checked_public_key_payload(&public_key)?;
        match unsafe { write_bytes(out_private_ptr, out_private_len, &private_bytes) } {
            Ok(()) => {}
            Err(code) => {
                return Err(match code {
                    ERR_NULL_PTR => BridgeError::NullPtr,
                    _ => BridgeError::Alloc,
                });
            }
        }
        match unsafe { write_bytes(out_public_ptr, out_public_len, public_bytes) } {
            Ok(()) => Ok(()),
            Err(code) => {
                unsafe {
                    free(*out_private_ptr as *mut _);
                    *out_private_ptr = ptr::null_mut();
                    *out_private_len = 0;
                }
                Err(match code {
                    ERR_NULL_PTR => BridgeError::NullPtr,
                    _ => BridgeError::Alloc,
                })
            }
        }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sign_detached(
    algorithm_code: u8,
    private_ptr: *const c_uchar,
    private_len: c_ulong,
    message_ptr: *const c_uchar,
    message_len: c_ulong,
    out_signature_ptr: *mut *mut c_uchar,
    out_signature_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if private_ptr.is_null()
            || message_ptr.is_null()
            || out_signature_ptr.is_null()
            || out_signature_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let private_bytes = unsafe { slice::from_raw_parts(private_ptr, private_len as usize) };
        let message = unsafe { slice::from_raw_parts(message_ptr, message_len as usize) };
        let private_key = parse_private_key_with_algorithm(private_bytes, algorithm)?;
        let signature = Signature::new(&private_key, message);
        unsafe { write_bytes_bridge(out_signature_ptr, out_signature_len, signature.payload()) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_verify_detached(
    algorithm_code: u8,
    public_ptr: *const c_uchar,
    public_len: c_ulong,
    message_ptr: *const c_uchar,
    message_len: c_ulong,
    signature_ptr: *const c_uchar,
    signature_len: c_ulong,
    out_valid: *mut c_uchar,
) -> c_int {
    let result = (|| {
        if public_ptr.is_null()
            || message_ptr.is_null()
            || signature_ptr.is_null()
            || out_valid.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        unsafe { *out_valid = 0 };
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let public_bytes = unsafe { slice::from_raw_parts(public_ptr, public_len as usize) };
        let public_key =
            PublicKey::from_bytes(algorithm, public_bytes).map_err(|_| BridgeError::PrivateKey)?;
        let message = unsafe { slice::from_raw_parts(message_ptr, message_len as usize) };
        let signature_bytes =
            unsafe { slice::from_raw_parts(signature_ptr, signature_len as usize) };
        let signature = Signature::from_bytes(signature_bytes);
        match signature.verify(&public_key, message) {
            Ok(()) => {
                unsafe { *out_valid = 1 };
                Ok(())
            }
            Err(CryptoError::BadSignature) => Ok(()),
            Err(_) => Err(BridgeError::UnsupportedAlgorithm),
        }
    })();

    bridge_result_to_code(result)
}

// ---------------- Chain discriminant helpers ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_get_chain_discriminant() -> u16 {
    iroha_data_model::account::address::chain_discriminant()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_set_chain_discriminant(discriminant: u16) -> u16 {
    iroha_data_model::account::address::set_chain_discriminant(discriminant)
}

// ---------------- Account address helpers ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_account_address_parse(
    input_ptr: *const c_char,
    input_len: c_ulong,
    expected_prefix: u16,
    expected_prefix_present: c_uchar,
    out_canonical_ptr: *mut *mut c_uchar,
    out_canonical_len: *mut c_ulong,
    out_network_prefix: *mut u16,
    out_error_json_ptr: *mut *mut c_uchar,
    out_error_json_len: *mut c_ulong,
) -> c_int {
    if input_ptr.is_null()
        || out_canonical_ptr.is_null()
        || out_canonical_len.is_null()
        || out_network_prefix.is_null()
    {
        return ERR_NULL_PTR;
    }

    write_optional_error(out_error_json_ptr, out_error_json_len);

    let input = match unsafe { read_string_bridge(input_ptr, input_len) } {
        Ok(value) => value,
        Err(err) => return err.code(),
    };
    let expect_prefix = if expected_prefix_present != 0 {
        Some(expected_prefix)
    } else {
        None
    };
    let address = match AccountAddress::parse_encoded(&input, expect_prefix) {
        Ok(value) => value,
        Err(err) => {
            return write_account_address_error(err, out_error_json_ptr, out_error_json_len);
        }
    };
    let canonical_hex = match address.canonical_hex() {
        Ok(value) => value,
        Err(err) => {
            return write_account_address_error(err, out_error_json_ptr, out_error_json_len);
        }
    };
    let hex_body = canonical_hex
        .strip_prefix("0x")
        .unwrap_or(canonical_hex.as_str());
    let canonical = match hex::decode(hex_body) {
        Ok(bytes) => bytes,
        Err(_) => {
            return write_account_address_error(
                AccountAddressError::InvalidHexAddress,
                out_error_json_ptr,
                out_error_json_len,
            );
        }
    };
    unsafe {
        if let Err(code) = write_bytes(out_canonical_ptr, out_canonical_len, &canonical) {
            return code;
        }
    }
    let prefix =
        expect_prefix.unwrap_or_else(iroha_data_model::account::address::chain_discriminant);
    unsafe {
        *out_network_prefix = prefix;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_account_address_render(
    canonical_ptr: *const c_uchar,
    canonical_len: c_ulong,
    network_prefix: u16,
    out_hex_ptr: *mut *mut c_uchar,
    out_hex_len: *mut c_ulong,
    out_i105_ptr: *mut *mut c_uchar,
    out_i105_len: *mut c_ulong,
    out_error_json_ptr: *mut *mut c_uchar,
    out_error_json_len: *mut c_ulong,
) -> c_int {
    if canonical_ptr.is_null()
        || out_hex_ptr.is_null()
        || out_hex_len.is_null()
        || out_i105_ptr.is_null()
        || out_i105_len.is_null()
    {
        return ERR_NULL_PTR;
    }

    write_optional_error(out_error_json_ptr, out_error_json_len);

    let canonical = unsafe { slice::from_raw_parts(canonical_ptr, canonical_len as usize) };
    let address = match AccountAddress::from_canonical_bytes(canonical) {
        Ok(value) => value,
        Err(err) => {
            return write_account_address_error(err, out_error_json_ptr, out_error_json_len);
        }
    };
    let canonical_hex = match address.canonical_hex() {
        Ok(value) => value,
        Err(err) => {
            return write_account_address_error(err, out_error_json_ptr, out_error_json_len);
        }
    };
    let i105 = match address.to_i105_for_discriminant(network_prefix) {
        Ok(value) => value,
        Err(err) => {
            return write_account_address_error(err, out_error_json_ptr, out_error_json_len);
        }
    };

    unsafe {
        if let Err(code) = write_bytes(out_hex_ptr, out_hex_len, canonical_hex.as_bytes()) {
            return code;
        }
        if let Err(code) = write_bytes(out_i105_ptr, out_i105_len, i105.as_bytes()) {
            return code;
        }
    }
    0
}

unsafe fn read_distid_or_default(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
) -> BridgeResult<String> {
    if distid_len == 0 {
        return Ok(Sm2PublicKey::default_distid());
    }
    if distid_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let len = distid_len as usize;
    let slice = unsafe { slice::from_raw_parts(distid_ptr as *const u8, len) };
    let distid = std::str::from_utf8(slice).map_err(|_| BridgeError::Utf8)?;
    Ok(distid.to_owned())
}

struct AssetTxInputs {
    chain_id: ChainId,
    authority: AccountId,
    asset_definition: AssetDefinitionId,
    asset_scope: AssetBalanceScope,
    destination: AccountId,
    quantity: Numeric,
    ttl: Option<NonZeroU64>,
    private_key: PrivateKey,
}

struct AssetInputPointers {
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    ttl_ms: u64,
    ttl_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
}

unsafe fn gather_asset_tx_inputs(ptrs: AssetInputPointers) -> BridgeResult<AssetTxInputs> {
    unsafe { gather_asset_tx_inputs_with_parser(ptrs, parse_private_key) }
}

unsafe fn gather_asset_tx_inputs_with_parser<F>(
    ptrs: AssetInputPointers,
    parse_key: F,
) -> BridgeResult<AssetTxInputs>
where
    F: Fn(&[u8]) -> BridgeResult<PrivateKey>,
{
    let AssetInputPointers {
        chain_ptr,
        chain_len,
        authority_ptr,
        authority_len,
        asset_definition_ptr,
        asset_definition_len,
        quantity_ptr,
        quantity_len,
        destination_ptr,
        destination_len,
        ttl_ms,
        ttl_present,
        private_key_ptr,
        private_key_len,
    } = ptrs;
    let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
    let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
    let asset_definition_str =
        unsafe { read_string_bridge(asset_definition_ptr, asset_definition_len) }?;
    let quantity_str = unsafe { read_string_bridge(quantity_ptr, quantity_len) }?;
    let destination_str = unsafe { read_string_bridge(destination_ptr, destination_len) }?;

    if private_key_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };

    let (asset_definition, asset_scope) =
        parse_asset_definition_with_balance_scope(asset_definition_str)?;

    Ok(AssetTxInputs {
        chain_id: chain.parse().map_err(|_| BridgeError::ChainId)?,
        authority: parse_account_id(authority_str)?,
        asset_definition,
        asset_scope,
        destination: parse_destination(destination_str)?,
        quantity: parse_quantity(quantity_str)?,
        ttl: parse_ttl(ttl_ms, ttl_present != 0)?,
        private_key: parse_key(key_slice)?,
    })
}

struct ShieldTxInputs {
    chain_id: ChainId,
    authority: AccountId,
    asset_definition: AssetDefinitionId,
    from_account: AccountId,
    amount: u128,
    ttl: Option<NonZeroU64>,
    private_key: PrivateKey,
}

struct ShieldInputPointers {
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    from_ptr: *const c_char,
    from_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    ttl_ms: u64,
    ttl_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
}

struct UnshieldTxInputs {
    chain_id: ChainId,
    authority: AccountId,
    asset_definition: AssetDefinitionId,
    destination: AccountId,
    amount: u128,
    inputs: Vec<[u8; 32]>,
    proof: ProofAttachment,
    root_hint: Option<[u8; 32]>,
    ttl: Option<NonZeroU64>,
    private_key: PrivateKey,
}

struct UnshieldInputPointers {
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    inputs_ptr: *const c_uchar,
    inputs_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    root_hint_ptr: *const c_uchar,
    root_hint_len: c_ulong,
    ttl_ms: u64,
    ttl_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
}

struct ZkTransferTxInputs {
    chain_id: ChainId,
    authority: AccountId,
    asset_definition: AssetDefinitionId,
    inputs: Vec<[u8; 32]>,
    outputs: Vec<[u8; 32]>,
    proof: ProofAttachment,
    root_hint: Option<[u8; 32]>,
    ttl: Option<NonZeroU64>,
    private_key: PrivateKey,
}

struct ZkTransferInputPointers {
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    inputs_ptr: *const c_uchar,
    inputs_len: c_ulong,
    outputs_ptr: *const c_uchar,
    outputs_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    root_hint_ptr: *const c_uchar,
    root_hint_len: c_ulong,
    ttl_ms: u64,
    ttl_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
}

fn parse_amount_u128(value: String) -> BridgeResult<u128> {
    value.parse::<u128>().map_err(|_| BridgeError::Quantity)
}

unsafe fn read_fixed_array<const N: usize>(
    ptr: *const c_uchar,
    len: c_ulong,
    err: BridgeError,
) -> BridgeResult<[u8; N]> {
    if ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    if len as usize != N {
        return Err(err);
    }
    let slice = unsafe { slice::from_raw_parts(ptr, N) };
    let mut out = [0u8; N];
    out.copy_from_slice(slice);
    Ok(out)
}

unsafe fn read_vec_bytes(ptr: *const c_uchar, len: c_ulong) -> BridgeResult<Vec<u8>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let slice = unsafe { slice::from_raw_parts(ptr, len as usize) };
    Ok(slice.to_vec())
}

fn build_confidential_encrypted_payload(
    ephemeral: [u8; 32],
    nonce: [u8; 24],
    ciphertext: Vec<u8>,
) -> BridgeResult<ConfidentialEncryptedPayload> {
    let payload = ConfidentialEncryptedPayload::new(ephemeral, nonce, ciphertext);
    payload
        .validate()
        .map_err(|_| BridgeError::ConfidentialPayload)?;
    Ok(payload)
}

fn decode_hex_array<const N: usize>(hex_str: &str) -> BridgeResult<[u8; N]> {
    let body = hex_str.trim().trim_start_matches("0x");
    let bytes = hex::decode(body).map_err(|_| BridgeError::ProofAttachment)?;
    if bytes.len() != N {
        return Err(BridgeError::ProofAttachment);
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_base64_bytes(value: &str) -> BridgeResult<Vec<u8>> {
    b64gp::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| BridgeError::ProofAttachment)
}

fn parse_proof_attachment_from_json_bytes(
    ptr: *const c_char,
    len: c_ulong,
) -> BridgeResult<ProofAttachment> {
    if ptr.is_null() || len == 0 {
        return Err(BridgeError::ProofAttachment);
    }
    let slice = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    let value =
        norito::json::from_slice::<JsonValue>(slice).map_err(|_| BridgeError::ProofAttachment)?;
    parse_proof_attachment_value(&value)
}

fn parse_proof_attachment_value(value: &JsonValue) -> BridgeResult<ProofAttachment> {
    let object = value.as_object().ok_or(BridgeError::ProofAttachment)?;
    for field in object.keys() {
        match field.as_str() {
            "backend" | "proof_backend" | "proof_b64" | "vk_ref" | "vk_commitment_hex"
            | "envelope_hash_hex" => {}
            "vk_inline" | "vkInline" | "verifyingKeyInline" | "verifying_key_inline" => {
                return Err(BridgeError::ProofAttachment);
            }
            _ => return Err(BridgeError::ProofAttachment),
        }
    }
    let backend_str = value
        .get("backend")
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::ProofAttachment)?;
    let backend_str = backend_str.trim();
    if backend_str.is_empty() {
        return Err(BridgeError::ProofAttachment);
    }
    let backend = backend_str
        .parse::<String>()
        .map_err(|_| BridgeError::ProofAttachment)?;
    let proof_bytes = value
        .get("proof_b64")
        .and_then(JsonValue::as_str)
        .map(decode_base64_bytes)
        .transpose()?
        .ok_or(BridgeError::ProofAttachment)?;
    let proof_backend = value
        .get("proof_backend")
        .and_then(JsonValue::as_str)
        .unwrap_or(backend_str)
        .parse::<String>()
        .map_err(|_| BridgeError::ProofAttachment)?;
    if proof_backend != backend {
        return Err(BridgeError::ProofAttachment);
    }
    let proof = ProofBox::new(proof_backend, proof_bytes);

    let attachment = if let Some(vk_ref) = value.get("vk_ref").and_then(JsonValue::as_object) {
        for field in vk_ref.keys() {
            match field.as_str() {
                "backend" | "name" => {}
                _ => return Err(BridgeError::ProofAttachment),
            }
        }
        let vk_backend = vk_ref
            .get("backend")
            .and_then(JsonValue::as_str)
            .ok_or(BridgeError::ProofAttachment)?
            .trim()
            .parse::<String>()
            .map_err(|_| BridgeError::ProofAttachment)?;
        if vk_backend.is_empty() {
            return Err(BridgeError::ProofAttachment);
        }
        if vk_backend != backend {
            return Err(BridgeError::ProofAttachment);
        }
        let name = vk_ref
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or(BridgeError::ProofAttachment)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(BridgeError::ProofAttachment);
        }
        let id = VerifyingKeyId::new(vk_backend, name);
        ProofAttachment::new_ref(backend.clone(), proof.clone(), id)
    } else {
        return Err(BridgeError::ProofAttachment);
    };

    let mut attachment = attachment;
    if let Some(commit_hex) = value.get("vk_commitment_hex").and_then(JsonValue::as_str) {
        attachment.vk_commitment = Some(decode_hex_array(commit_hex)?);
    }
    if let Some(envelope_hex) = value.get("envelope_hash_hex").and_then(JsonValue::as_str) {
        attachment.envelope_hash = Some(decode_hex_array(envelope_hex)?);
    }
    Ok(attachment)
}

fn parse_fixed_32_chunks(
    ptr: *const c_uchar,
    len: c_ulong,
    err: BridgeError,
) -> BridgeResult<Vec<[u8; 32]>> {
    if ptr.is_null() || len == 0 {
        return Err(err);
    }
    if !(len as usize).is_multiple_of(32_usize) {
        return Err(err);
    }
    let slice = unsafe { slice::from_raw_parts(ptr, len as usize) };
    Ok(slice
        .chunks_exact(32)
        .map(|chunk| {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(chunk);
            arr
        })
        .collect())
}

fn parse_unshield_nullifiers(ptr: *const c_uchar, len: c_ulong) -> BridgeResult<Vec<[u8; 32]>> {
    parse_fixed_32_chunks(ptr, len, BridgeError::InvalidNullifiers)
}

unsafe fn gather_shield_tx_inputs(ptrs: ShieldInputPointers) -> BridgeResult<ShieldTxInputs> {
    unsafe { gather_shield_tx_inputs_with_parser(ptrs, parse_private_key) }
}

unsafe fn gather_shield_tx_inputs_with_parser<F>(
    ptrs: ShieldInputPointers,
    parse_key: F,
) -> BridgeResult<ShieldTxInputs>
where
    F: Fn(&[u8]) -> BridgeResult<PrivateKey>,
{
    let ShieldInputPointers {
        chain_ptr,
        chain_len,
        authority_ptr,
        authority_len,
        asset_definition_ptr,
        asset_definition_len,
        from_ptr,
        from_len,
        amount_ptr,
        amount_len,
        ttl_ms,
        ttl_present,
        private_key_ptr,
        private_key_len,
    } = ptrs;

    let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
    let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
    let asset_definition_str =
        unsafe { read_string_bridge(asset_definition_ptr, asset_definition_len) }?;
    let from_str = unsafe { read_string_bridge(from_ptr, from_len) }?;
    let amount_str = unsafe { read_string_bridge(amount_ptr, amount_len) }?;

    if private_key_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };

    Ok(ShieldTxInputs {
        chain_id: chain.parse().map_err(|_| BridgeError::ChainId)?,
        authority: parse_account_id(authority_str)?,
        asset_definition: parse_asset_definition(asset_definition_str)?,
        from_account: parse_account_id(from_str)?,
        amount: parse_amount_u128(amount_str)?,
        ttl: parse_ttl(ttl_ms, ttl_present != 0)?,
        private_key: parse_key(key_slice)?,
    })
}

unsafe fn gather_unshield_tx_inputs(ptrs: UnshieldInputPointers) -> BridgeResult<UnshieldTxInputs> {
    unsafe { gather_unshield_tx_inputs_with_parser(ptrs, parse_private_key) }
}

unsafe fn gather_unshield_tx_inputs_with_parser<F>(
    ptrs: UnshieldInputPointers,
    parse_key: F,
) -> BridgeResult<UnshieldTxInputs>
where
    F: Fn(&[u8]) -> BridgeResult<PrivateKey>,
{
    let UnshieldInputPointers {
        chain_ptr,
        chain_len,
        authority_ptr,
        authority_len,
        asset_definition_ptr,
        asset_definition_len,
        destination_ptr,
        destination_len,
        amount_ptr,
        amount_len,
        inputs_ptr,
        inputs_len,
        proof_json_ptr,
        proof_json_len,
        root_hint_ptr,
        root_hint_len,
        ttl_ms,
        ttl_present,
        private_key_ptr,
        private_key_len,
    } = ptrs;

    let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
    let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
    let asset_definition_str =
        unsafe { read_string_bridge(asset_definition_ptr, asset_definition_len) }?;
    let destination_str = unsafe { read_string_bridge(destination_ptr, destination_len) }?;
    let amount_str = unsafe { read_string_bridge(amount_ptr, amount_len) }?;

    if private_key_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };

    let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
    let authority = parse_account_id(authority_str)?;
    let asset_definition = parse_asset_definition(asset_definition_str)?;
    let destination = parse_account_id(destination_str)?;
    let amount = parse_amount_u128(amount_str)?;
    let inputs = parse_unshield_nullifiers(inputs_ptr, inputs_len)?;
    let proof = parse_proof_attachment_from_json_bytes(proof_json_ptr, proof_json_len)?;

    let root_hint = if root_hint_len == 0 {
        None
    } else {
        if root_hint_ptr.is_null() || root_hint_len != 32 {
            return Err(BridgeError::InvalidRootHint);
        }
        let bytes = unsafe { slice::from_raw_parts(root_hint_ptr, 32) };
        let mut hint = [0u8; 32];
        hint.copy_from_slice(bytes);
        Some(hint)
    };

    Ok(UnshieldTxInputs {
        chain_id,
        authority,
        asset_definition,
        destination,
        amount,
        inputs,
        proof,
        root_hint,
        ttl: parse_ttl(ttl_ms, ttl_present != 0)?,
        private_key: parse_key(key_slice)?,
    })
}

unsafe fn gather_zk_transfer_tx_inputs(
    ptrs: ZkTransferInputPointers,
) -> BridgeResult<ZkTransferTxInputs> {
    unsafe { gather_zk_transfer_tx_inputs_with_parser(ptrs, parse_private_key) }
}

unsafe fn gather_zk_transfer_tx_inputs_with_parser<F>(
    ptrs: ZkTransferInputPointers,
    parse_key: F,
) -> BridgeResult<ZkTransferTxInputs>
where
    F: Fn(&[u8]) -> BridgeResult<PrivateKey>,
{
    let ZkTransferInputPointers {
        chain_ptr,
        chain_len,
        authority_ptr,
        authority_len,
        asset_definition_ptr,
        asset_definition_len,
        inputs_ptr,
        inputs_len,
        outputs_ptr,
        outputs_len,
        proof_json_ptr,
        proof_json_len,
        root_hint_ptr,
        root_hint_len,
        ttl_ms,
        ttl_present,
        private_key_ptr,
        private_key_len,
    } = ptrs;

    let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
    let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
    let asset_definition_str =
        unsafe { read_string_bridge(asset_definition_ptr, asset_definition_len) }?;

    if private_key_ptr.is_null() {
        return Err(BridgeError::NullPtr);
    }
    let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };

    let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
    let authority = parse_account_id(authority_str)?;
    let asset_definition = parse_asset_definition(asset_definition_str)?;
    let inputs = parse_fixed_32_chunks(inputs_ptr, inputs_len, BridgeError::InvalidNullifiers)?;
    let outputs =
        parse_fixed_32_chunks(outputs_ptr, outputs_len, BridgeError::InvalidNoteCommitment)?;
    let proof = parse_proof_attachment_from_json_bytes(proof_json_ptr, proof_json_len)?;

    let root_hint = if root_hint_len == 0 {
        None
    } else {
        if root_hint_ptr.is_null() || root_hint_len != 32 {
            return Err(BridgeError::InvalidRootHint);
        }
        let bytes = unsafe { slice::from_raw_parts(root_hint_ptr, 32) };
        let mut hint = [0u8; 32];
        hint.copy_from_slice(bytes);
        Some(hint)
    };

    Ok(ZkTransferTxInputs {
        chain_id,
        authority,
        asset_definition,
        inputs,
        outputs,
        proof,
        root_hint,
        ttl: parse_ttl(ttl_ms, ttl_present != 0)?,
        private_key: parse_key(key_slice)?,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_confidential_encrypted_payload(
    ephemeral_ptr: *const c_uchar,
    ephemeral_len: c_ulong,
    nonce_ptr: *const c_uchar,
    nonce_len: c_ulong,
    ciphertext_ptr: *const c_uchar,
    ciphertext_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    if ephemeral_ptr.is_null() || nonce_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
        return -1;
    }
    if ephemeral_len != 32 || nonce_len != 24 {
        return -3;
    }
    if ciphertext_len > (u32::MAX as c_ulong) {
        return -2;
    }
    // Safety: caller guarantees buffers are valid for the declared length.
    let mut ephemeral = [0u8; 32];
    ephemeral.copy_from_slice(unsafe { slice::from_raw_parts(ephemeral_ptr, 32) });
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(unsafe { slice::from_raw_parts(nonce_ptr, 24) });
    let ciphertext = match unsafe { read_vec_bytes(ciphertext_ptr, ciphertext_len) } {
        Ok(ciphertext) => ciphertext,
        Err(BridgeError::NullPtr) => return -1,
        Err(_) => return -3,
    };
    let payload = match build_confidential_encrypted_payload(ephemeral, nonce, ciphertext) {
        Ok(payload) => payload,
        Err(_) => return -3,
    };
    let ciphertext = payload.ciphertext();
    let mut encoded = Vec::with_capacity(
        1 + payload.ephemeral_pubkey().len() + payload.nonce().len() + ciphertext.len() + 10,
    );
    encoded.push(CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1);
    encoded.extend_from_slice(payload.ephemeral_pubkey());
    encoded.extend_from_slice(payload.nonce());
    encode_varint(ciphertext.len() as u64, &mut encoded);
    encoded.extend_from_slice(ciphertext);
    unsafe { write_bytes(out_ptr, out_len, &encoded) }.map_or_else(|err| err, |_| 0)
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn encode_connect_frame(frame: &proto::ConnectFrameV1) -> Result<Vec<u8>, norito::core::Error> {
    proto::encode_connect_frame_bare(frame)
}

fn decode_connect_frame(bytes: &[u8]) -> Result<proto::ConnectFrameV1, norito::core::Error> {
    proto::decode_connect_frame_bare(bytes)
}

fn decode_envelope(bytes: &[u8]) -> Result<proto::EnvelopeV1, norito::core::Error> {
    proto::decode_connect_envelope_framed(bytes)
}

fn encode_envelope_framed(env: &proto::EnvelopeV1) -> Result<Vec<u8>, norito::core::Error> {
    proto::encode_connect_envelope_framed(env)
}

fn decode_signed_transaction(bytes: &[u8]) -> Result<SignedTransaction, norito::core::Error> {
    SignedTransaction::decode_all_versioned(bytes)
        .map_err(|err| norito::core::Error::Message(err.to_string()))
}

fn signed_transaction_bridge_debug_json(tx: &SignedTransaction) -> JsonValue {
    use iroha_data_model::prelude::TransferBox;

    let mut transfer_asset_scopes = Vec::new();
    if let Executable::Instructions(instructions) = tx.instructions() {
        for instruction in instructions {
            let Some(transfer_box) = instruction.as_any().downcast_ref::<TransferBox>() else {
                continue;
            };
            let TransferBox::Asset(transfer) = transfer_box else {
                continue;
            };

            let mut scope = JsonMap::new();
            scope.insert("instruction".into(), JsonValue::from("transfer_asset"));
            scope.insert(
                "source_asset_definition_id".into(),
                JsonValue::from(transfer.source.definition().to_string()),
            );
            match transfer.source.scope() {
                AssetBalanceScope::Global => {
                    scope.insert("source_scope".into(), JsonValue::from("global"));
                }
                AssetBalanceScope::Dataspace(dataspace_id) => {
                    scope.insert("source_scope".into(), JsonValue::from("dataspace"));
                    scope.insert(
                        "source_dataspace_id".into(),
                        JsonValue::from(dataspace_id.as_u64()),
                    );
                }
            }
            transfer_asset_scopes.push(JsonValue::Object(scope));
        }
    }

    JsonValue::Object(JsonMap::from_iter([(
        "transfer_asset_scopes".into(),
        JsonValue::Array(transfer_asset_scopes),
    )]))
}

fn encode_asset_transaction<F>(
    chain_id: ChainId,
    authority: AccountId,
    creation_time_ms: u64,
    ttl_option: Option<NonZeroU64>,
    private_key: PrivateKey,
    build_executable: F,
) -> (Vec<u8>, [u8; 32])
where
    F: FnOnce() -> Executable,
{
    encode_asset_transaction_with_nonce(
        chain_id,
        authority,
        creation_time_ms,
        ttl_option,
        None,
        private_key,
        build_executable,
    )
}

fn encode_asset_transaction_with_nonce<F>(
    chain_id: ChainId,
    authority: AccountId,
    creation_time_ms: u64,
    ttl_option: Option<NonZeroU64>,
    nonce_option: Option<NonZeroU32>,
    private_key: PrivateKey,
    build_executable: F,
) -> (Vec<u8>, [u8; 32])
where
    F: FnOnce() -> Executable,
{
    encode_asset_transaction_with_nonce_and_metadata(
        chain_id,
        authority,
        creation_time_ms,
        ttl_option,
        nonce_option,
        Metadata::default(),
        private_key,
        build_executable,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_asset_transaction_with_nonce_and_metadata<F>(
    chain_id: ChainId,
    authority: AccountId,
    creation_time_ms: u64,
    ttl_option: Option<NonZeroU64>,
    nonce_option: Option<NonZeroU32>,
    metadata: Metadata,
    private_key: PrivateKey,
    build_executable: F,
) -> (Vec<u8>, [u8; 32])
where
    F: FnOnce() -> Executable,
{
    let ttl_duration = ttl_option.map(|ttl| Duration::from_millis(ttl.get()));
    let mut builder = TransactionBuilder::new(chain_id, authority);
    builder = builder.with_executable(build_executable());
    if !metadata.is_empty() {
        builder = builder.with_metadata(metadata);
    }
    if let Some(ttl) = ttl_duration {
        builder.set_ttl(ttl);
    }
    if let Some(nonce) = nonce_option {
        builder.set_nonce(nonce);
    }
    builder.set_creation_time(Duration::from_millis(creation_time_ms));
    let signed = builder.sign(&private_key);
    let signed_bytes = signed.encode_versioned();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(signed.hash().as_ref());
    (signed_bytes, hash)
}

fn encode_instruction_transaction(
    chain_id: ChainId,
    authority: AccountId,
    creation_time_ms: u64,
    ttl_option: Option<NonZeroU64>,
    private_key: PrivateKey,
    instruction: InstructionBox,
) -> (Vec<u8>, [u8; 32]) {
    encode_asset_transaction(
        chain_id,
        authority,
        creation_time_ms,
        ttl_option,
        private_key,
        move || Executable::from([instruction]),
    )
}

fn b64_encode(bytes: &[u8]) -> String {
    let eng = b64gp::STANDARD;
    let len = bytes.len().div_ceil(3) * 4;
    let mut out = vec![0u8; len];
    let wrote = eng.encode_slice(bytes, &mut out).expect("encode");
    out.truncate(wrote);
    String::from_utf8(out).expect("utf8")
}

fn json_object(pairs: impl IntoIterator<Item = (&'static str, JsonValue)>) -> JsonValue {
    let mut map = JsonMap::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), value);
    }
    JsonValue::Object(map)
}

fn json_string_array(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|s| JsonValue::from(s.as_str())).collect())
}

fn json_option_string_array(values: &Option<Vec<String>>) -> JsonValue {
    match values {
        Some(list) => json_string_array(list),
        None => JsonValue::Null,
    }
}

fn bool_to_u8(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

fn option_to_ffi(value: Option<usize>) -> (u64, u8) {
    match value {
        Some(v) => (v as u64, 1),
        None => (0, 0),
    }
}

unsafe fn parse_algorithm_cstr(
    alg_ptr: *const c_char,
    alg_len: c_ulong,
) -> Result<Algorithm, c_int> {
    if alg_ptr.is_null() {
        return Err(-6);
    }
    let bytes = unsafe { std::slice::from_raw_parts(alg_ptr as *const u8, alg_len as usize) };
    let alg_str = std::str::from_utf8(bytes).map_err(|_| -7)?;
    Algorithm::from_str(alg_str.trim()).map_err(|_| -8)
}

unsafe fn parse_permissions_bytes(
    permissions_ptr: *const u8,
    permissions_len: c_ulong,
) -> Result<Option<proto::PermissionsV1>, c_int> {
    if permissions_ptr.is_null() || permissions_len == 0 {
        return Ok(None);
    }
    let json = unsafe { std::slice::from_raw_parts(permissions_ptr, permissions_len as usize) };
    if json.is_empty() {
        return Ok(None);
    }
    let val = norito::json::from_slice::<JsonValue>(json).map_err(|_| -4)?;
    match val {
        JsonValue::Null => Ok(None),
        JsonValue::Object(map) => {
            let methods = map
                .get("methods")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_else(Vec::new);
            let events = map
                .get("events")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_else(Vec::new);
            let resources = map.get("resources").and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            });
            Ok(Some(proto::PermissionsV1 {
                methods,
                events,
                resources,
            }))
        }
        _ => Ok(None),
    }
}

unsafe fn parse_app_meta_bytes(
    app_meta_ptr: *const u8,
    app_meta_len: c_ulong,
) -> Result<Option<proto::AppMeta>, c_int> {
    if app_meta_ptr.is_null() || app_meta_len == 0 {
        return Ok(None);
    }
    let json = unsafe { std::slice::from_raw_parts(app_meta_ptr, app_meta_len as usize) };
    if json.is_empty() {
        return Ok(None);
    }
    let val = norito::json::from_slice::<JsonValue>(json).map_err(|_| -4)?;
    match val {
        JsonValue::Null => Ok(None),
        JsonValue::Object(map) => {
            let name = map
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let Some(name) = name else {
                return Ok(None);
            };
            let url = map
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let icon_hash = map
                .get("icon_hash")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(Some(proto::AppMeta {
                name: name.to_string(),
                url,
                icon_hash,
            }))
        }
        _ => Ok(None),
    }
}

unsafe fn parse_proof_bytes(
    proof_ptr: *const u8,
    proof_len: c_ulong,
) -> Result<Option<proto::SignInProofV1>, c_int> {
    if proof_ptr.is_null() || proof_len == 0 {
        return Ok(None);
    }
    let json = unsafe { std::slice::from_raw_parts(proof_ptr, proof_len as usize) };
    if json.is_empty() {
        return Ok(None);
    }
    let val = norito::json::from_slice::<JsonValue>(json).map_err(|_| -4)?;
    match val {
        JsonValue::Null => Ok(None),
        JsonValue::Object(map) => {
            let domain = map
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let uri = map
                .get("uri")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let statement = map
                .get("statement")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let issued_at = map
                .get("issued_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let nonce = map
                .get("nonce")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Some(proto::SignInProofV1 {
                domain,
                uri,
                statement,
                issued_at,
                nonce,
            }))
        }
        _ => Ok(None),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_ciphertext_frame(
    sid_ptr: *const c_uchar,  // 32 bytes
    dir: c_uchar,             // 0 = AppToWallet, 1 = WalletToApp
    seq: u64,                 // little-endian in header
    aead_ptr: *const c_uchar, // ChaChaPoly combined (ct||tag)
    aead_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null() || aead_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let aead = std::slice::from_raw_parts(aead_ptr, aead_len as usize);

        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };

        let ct = proto::ConnectCiphertextV1 {
            dir,
            aead: aead.to_vec(),
        };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Ciphertext(ct),
        };

        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -4;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

fn read_32_bytes(ptr: *const c_uchar) -> Result<[u8; 32], c_int> {
    if ptr.is_null() {
        return Err(-1);
    }
    let slice = unsafe { slice::from_raw_parts(ptr, 32) };
    let mut out = [0u8; 32];
    out.copy_from_slice(slice);
    Ok(out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_generate_keypair(
    out_pk: *mut c_uchar,
    out_sk: *mut c_uchar,
) -> c_int {
    unsafe {
        if out_pk.is_null() || out_sk.is_null() {
            return -1;
        }
        let scheme = iroha_crypto::kex::X25519Sha256::new();
        let (pk, sk) = match scheme.try_keypair(KeyGenOption::Random) {
            Ok(keypair) => keypair,
            Err(_) => return ERR_CONNECT_KEYPAIR,
        };
        ptr::copy_nonoverlapping(pk.as_bytes().as_ptr(), out_pk, 32);
        ptr::copy_nonoverlapping(sk.to_bytes().as_ref().as_ptr(), out_sk, 32);
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_public_from_private(
    sk_ptr: *const c_uchar,
    out_pk: *mut c_uchar,
) -> c_int {
    unsafe {
        if sk_ptr.is_null() || out_pk.is_null() {
            return -1;
        }
        let sk_bytes = match read_32_bytes(sk_ptr) {
            Ok(b) => b,
            Err(code) => return code,
        };
        let scheme = iroha_crypto::kex::X25519Sha256::new();
        let sk = x25519_dalek::StaticSecret::from(sk_bytes);
        let (derived_pk, _) = scheme.keypair(KeyGenOption::FromPrivateKey(sk));
        let pk = iroha_crypto::kex::X25519Sha256::encode_public_key(&derived_pk);
        let pk_slice: &[u8] = pk.as_ref();
        debug_assert_eq!(pk_slice.len(), 32);
        ptr::copy_nonoverlapping(pk_slice.as_ptr(), out_pk, pk_slice.len());
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_derive_keys(
    sk_ptr: *const c_uchar,
    peer_pk_ptr: *const c_uchar,
    sid_ptr: *const c_uchar,
    out_app_ptr: *mut c_uchar,
    out_wallet_ptr: *mut c_uchar,
) -> c_int {
    unsafe {
        if sk_ptr.is_null()
            || peer_pk_ptr.is_null()
            || sid_ptr.is_null()
            || out_app_ptr.is_null()
            || out_wallet_ptr.is_null()
        {
            return -1;
        }
        let local_sk = match read_32_bytes(sk_ptr) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let peer_pk = match read_32_bytes(peer_pk_ptr) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let sid = match read_32_bytes(sid_ptr) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let (app_key, wallet_key) = match connect_sdk::x25519_derive_keys(&local_sk, &peer_pk, &sid)
        {
            Ok(keys) => keys,
            Err(_) => return -2,
        };
        ptr::copy_nonoverlapping(app_key.as_ptr(), out_app_ptr, 32);
        ptr::copy_nonoverlapping(wallet_key.as_ptr(), out_wallet_ptr, 32);
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_encrypt_envelope(
    key_ptr: *const c_uchar,
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    env_ptr: *const c_uchar,
    env_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if key_ptr.is_null()
            || sid_ptr.is_null()
            || env_ptr.is_null()
            || out_ptr.is_null()
            || out_len.is_null()
        {
            return -1;
        }
        let key = match read_32_bytes(key_ptr) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let sid = match read_32_bytes(sid_ptr) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let direction = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };
        let env_bytes = std::slice::from_raw_parts(env_ptr, env_len as usize);
        let envelope = match decode_envelope(env_bytes) {
            Ok(env) => env,
            Err(_) => return -3,
        };
        let frame =
            connect_sdk::seal_envelope(&key, &sid, direction, envelope.seq, envelope.payload);
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        if let Err(code) = write_bytes(out_ptr, out_len, &buf) {
            return code;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_connect_decrypt_ciphertext(
    key_ptr: *const c_uchar,
    frame_ptr: *const c_uchar,
    frame_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if key_ptr.is_null() || frame_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let key = match read_32_bytes(key_ptr) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };
        let frame_bytes = std::slice::from_raw_parts(frame_ptr, frame_len as usize);
        let frame = match decode_connect_frame(frame_bytes) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let envelope = match connect_sdk::open_envelope(&key, &frame) {
            Ok(env) => env,
            Err(_) => return -3,
        };
        let buf = match encode_envelope_framed(&envelope) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        if let Err(code) = write_bytes(out_ptr, out_len, &buf) {
            return code;
        }
        0
    }
}

// ---------------- Control frame decode helpers ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_kind(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_sid_ptr: *mut c_uchar, // 32 bytes
    out_dir: *mut c_uchar,     // 0/1
    out_seq: *mut u64,
    out_kind: *mut u16,
) -> c_int {
    unsafe {
        if inp_ptr.is_null()
            || out_sid_ptr.is_null()
            || out_dir.is_null()
            || out_seq.is_null()
            || out_kind.is_null()
        {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let kind: u16 = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Open { .. }) => 1,
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { .. }) => 2,
            proto::FrameKind::Control(proto::ConnectControlV1::Reject { .. }) => 3,
            proto::FrameKind::Control(proto::ConnectControlV1::Close { .. }) => 4,
            proto::FrameKind::Control(proto::ConnectControlV1::Ping { .. }) => 5,
            proto::FrameKind::Control(proto::ConnectControlV1::Pong { .. }) => 6,
            proto::FrameKind::Control(proto::ConnectControlV1::ServerEvent { .. }) => 7,
            proto::FrameKind::Ciphertext(_) => 100,
        };
        ptr::copy_nonoverlapping(frame.sid.as_ptr(), out_sid_ptr, 32);
        *out_dir = match frame.dir {
            proto::Dir::AppToWallet => 0,
            proto::Dir::WalletToApp => 1,
        };
        *out_seq = frame.seq;
        *out_kind = kind;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_open_pub(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_pk: *mut c_uchar,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_pk.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Open { app_pk, .. }) => {
                ptr::copy_nonoverlapping(app_pk.as_ptr(), out_pk, 32);
                0
            }
            _ => -3,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_pub(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_pk: *mut c_uchar,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_pk.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { wallet_pk, .. }) => {
                ptr::copy_nonoverlapping(wallet_pk.as_ptr(), out_pk, 32);
                0
            }
            _ => -3,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_account(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { account_id, .. }) => {
                let bytes = account_id.as_bytes();
                let len = bytes.len();
                let mem = malloc(len);
                if mem.is_null() {
                    return -3;
                }
                ptr::copy_nonoverlapping(bytes.as_ptr(), mem as *mut u8, len);
                *out_ptr = mem as *mut u8;
                *out_len = len as c_ulong;
                0
            }
            _ => -4,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_sig(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_sig: *mut c_uchar,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_sig.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { sig_wallet, .. }) => {
                match sig_wallet.algorithm {
                    Algorithm::Ed25519 => {
                        let bytes = sig_wallet.bytes();
                        if bytes.len() != 64 {
                            return -3;
                        }
                        ptr::copy_nonoverlapping(bytes.as_ptr(), out_sig, 64);
                        0
                    }
                    _ => -5,
                }
            }
            _ => -4,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_open_chain_id(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let chain_id = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Open { constraints, .. }) => {
                constraints.chain_id
            }
            _ => return -3,
        };
        if let Err(code) = write_bytes(out_ptr, out_len, chain_id.as_bytes()) {
            return code;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_sig_alg(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_alg_ptr: *mut *mut c_char,
    out_alg_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_alg_ptr.is_null() || out_alg_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let alg_str = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { sig_wallet, .. }) => {
                sig_wallet.algorithm.as_static_str()
            }
            _ => return -3,
        };
        let bytes = alg_str.as_bytes();
        let len = bytes.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -4;
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), mem as *mut u8, len);
        *out_alg_ptr = mem as *mut c_char;
        *out_alg_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_close(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_role: *mut c_uchar,
    out_code: *mut u16,
    out_retryable: *mut c_uchar,
    out_reason_ptr: *mut *mut c_uchar,
    out_reason_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null()
            || out_role.is_null()
            || out_code.is_null()
            || out_retryable.is_null()
            || out_reason_ptr.is_null()
            || out_reason_len.is_null()
        {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let (who, code, reason, retryable) = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Close {
                who,
                code,
                reason,
                retryable,
            }) => (who, code, reason, retryable),
            _ => return -3,
        };
        *out_role = match who {
            proto::Role::App => 0,
            proto::Role::Wallet => 1,
        };
        *out_code = code;
        *out_retryable = if retryable { 1 } else { 0 };
        if let Err(code) = write_bytes(out_reason_ptr, out_reason_len, reason.as_bytes()) {
            return code;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_reject(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_code: *mut u16,
    out_code_id_ptr: *mut *mut c_uchar,
    out_code_id_len: *mut c_ulong,
    out_reason_ptr: *mut *mut c_uchar,
    out_reason_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null()
            || out_code.is_null()
            || out_code_id_ptr.is_null()
            || out_code_id_len.is_null()
            || out_reason_ptr.is_null()
            || out_reason_len.is_null()
        {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let (code, code_id, reason) = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Reject {
                code,
                code_id,
                reason,
            }) => (code, code_id, reason),
            _ => return -3,
        };
        *out_code = code;
        if let Err(code) = write_bytes(out_code_id_ptr, out_code_id_len, code_id.as_bytes()) {
            return code;
        }
        if let Err(code) = write_bytes(out_reason_ptr, out_reason_len, reason.as_bytes()) {
            return code;
        }
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_ping(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_nonce: *mut u64,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_nonce.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let nonce = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce }) => nonce,
            _ => return -3,
        };
        *out_nonce = nonce;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_pong(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_nonce: *mut u64,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_nonce.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let nonce = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Pong { nonce }) => nonce,
            _ => return -3,
        };
        *out_nonce = nonce;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_account_json(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let acct = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve {
                ref account_id, ..
            }) => account_id,
            _ => return -3,
        };
        let payload = json_object([("account_id", ::norito::json!(acct.clone()))]);
        let s = match norito::json::to_vec(&payload) {
            Ok(v) => v,
            Err(_) => return -4,
        };
        let len = s.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

// ---------------- Permissions/Proof JSON helpers ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_open_app_metadata_json(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let app_meta = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Open { ref app_meta, .. }) => {
                app_meta
            }
            _ => return -3,
        };
        let val = if let Some(meta) = app_meta {
            let url = meta
                .url
                .as_ref()
                .map(|value| JsonValue::from(value.as_str()))
                .unwrap_or(JsonValue::Null);
            let icon_hash = meta
                .icon_hash
                .as_ref()
                .map(|value| JsonValue::from(value.as_str()))
                .unwrap_or(JsonValue::Null);
            json_object([
                ("name", JsonValue::from(meta.name.as_str())),
                ("url", url),
                ("icon_hash", icon_hash),
            ])
        } else {
            json_object([])
        };
        let s = match norito::json::to_vec(&val) {
            Ok(v) => v,
            Err(_) => return -4,
        };
        let len = s.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_open_permissions_json(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let perms = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Open {
                ref permissions, ..
            }) => permissions,
            _ => return -3,
        };
        let val = if let Some(p) = perms {
            json_object([
                ("methods", json_string_array(&p.methods)),
                ("events", json_string_array(&p.events)),
                ("resources", json_option_string_array(&p.resources)),
            ])
        } else {
            json_object([])
        };
        let s = match norito::json::to_vec(&val) {
            Ok(v) => v,
            Err(_) => return -4,
        };
        let len = s.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_permissions_json(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let perms = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve {
                ref permissions, ..
            }) => permissions,
            _ => return -3,
        };
        let val = if let Some(p) = perms {
            json_object([
                ("methods", json_string_array(&p.methods)),
                ("events", json_string_array(&p.events)),
                ("resources", json_option_string_array(&p.resources)),
            ])
        } else {
            json_object([])
        };
        let s = match norito::json::to_vec(&val) {
            Ok(v) => v,
            Err(_) => return -4,
        };
        let len = s.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_control_approve_proof_json(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let proof = match frame.kind {
            proto::FrameKind::Control(proto::ConnectControlV1::Approve { ref proof, .. }) => proof,
            _ => return -3,
        };
        let val = if let Some(p) = proof {
            json_object([
                ("domain", ::norito::json!(p.domain.clone())),
                ("uri", ::norito::json!(p.uri.clone())),
                ("statement", ::norito::json!(p.statement.clone())),
                ("issued_at", ::norito::json!(p.issued_at.clone())),
                ("nonce", ::norito::json!(p.nonce.clone())),
            ])
        } else {
            json_object([])
        };
        let s = match norito::json::to_vec(&val) {
            Ok(v) => v,
            Err(_) => return -4,
        };
        let len = s.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

// ---------------- Extended control encoders ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_open_ext(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    app_pk_ptr: *const c_uchar,
    app_pk_len: c_ulong,
    app_meta_ptr: *const c_uchar,
    app_meta_len: c_ulong,
    chain_id_ptr: *const c_char,
    perms_ptr: *const c_uchar,
    perms_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null()
            || app_pk_ptr.is_null()
            || chain_id_ptr.is_null()
            || out_ptr.is_null()
            || out_len.is_null()
        {
            return -1;
        }
        if app_pk_len != 32 {
            return -2;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -3,
        };
        let app_pk = {
            let pk = std::slice::from_raw_parts(app_pk_ptr, 32);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(pk);
            arr
        };
        let chain_cstr = std::ffi::CStr::from_ptr(chain_id_ptr);
        let chain_id = chain_cstr.to_string_lossy().to_string();
        let app_meta = match parse_app_meta_bytes(app_meta_ptr, app_meta_len) {
            Ok(meta) => meta,
            Err(code) => return code,
        };
        let permissions = if !perms_ptr.is_null() && perms_len > 0 {
            let j = std::slice::from_raw_parts(perms_ptr, perms_len as usize);
            if let Ok(val) = norito::json::from_slice::<norito::json::Value>(j) {
                let methods = val
                    .get("methods")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_else(Vec::new);
                let events = val
                    .get("events")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_else(Vec::new);
                let resources = val.get("resources").and_then(|v| v.as_array()).map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                });
                Some(proto::PermissionsV1 {
                    methods,
                    events,
                    resources,
                })
            } else {
                None
            }
        } else {
            None
        };
        let ctrl = proto::ConnectControlV1::Open {
            app_pk,
            app_meta,
            constraints: proto::Constraints { chain_id },
            permissions,
        };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_approve_ext(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    wallet_pk_ptr: *const c_uchar,
    wallet_pk_len: c_ulong,
    account_cstr: *const c_char,
    perms_ptr: *const c_uchar,
    perms_len: c_ulong,
    proof_ptr: *const c_uchar,
    proof_len: c_ulong,
    sig_ptr: *const c_uchar,
    sig_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null()
            || wallet_pk_ptr.is_null()
            || account_cstr.is_null()
            || sig_ptr.is_null()
            || out_ptr.is_null()
            || out_len.is_null()
        {
            return -1;
        }
        if wallet_pk_len != 32 {
            return -2;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -3,
        };
        let wallet_pk = {
            let pk = std::slice::from_raw_parts(wallet_pk_ptr, 32);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(pk);
            arr
        };
        let account_id = std::ffi::CStr::from_ptr(account_cstr)
            .to_string_lossy()
            .to_string();
        let permissions = match parse_permissions_bytes(perms_ptr, perms_len) {
            Ok(p) => p,
            Err(code) => return code,
        };
        let proof = match parse_proof_bytes(proof_ptr, proof_len) {
            Ok(p) => p,
            Err(code) => return code,
        };
        let sig_bytes = std::slice::from_raw_parts(sig_ptr, sig_len as usize);
        let sig_wallet = match proto::WalletSignatureV1::from_ed25519_bytes(sig_bytes) {
            Some(sig) => sig,
            None => return -4,
        };
        let ctrl = proto::ConnectControlV1::Approve {
            wallet_pk,
            account_id,
            permissions,
            proof,
            sig_wallet,
        };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_approve_ext_with_alg(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    wallet_pk_ptr: *const c_uchar,
    account_ptr: *const c_char,
    account_len: c_ulong,
    permissions_json_ptr: *const c_char,
    permissions_json_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    alg_ptr: *const c_char,
    alg_len: c_ulong,
    sig_ptr: *const c_uchar,
    sig_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null()
            || wallet_pk_ptr.is_null()
            || account_ptr.is_null()
            || sig_ptr.is_null()
            || out_ptr.is_null()
            || out_len.is_null()
        {
            return -1;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };
        let wallet_pk = std::slice::from_raw_parts(wallet_pk_ptr, 32);
        let mut wallet_pk_arr = [0u8; 32];
        wallet_pk_arr.copy_from_slice(wallet_pk);

        let account_id = match std::str::from_utf8(std::slice::from_raw_parts(
            account_ptr as *const u8,
            account_len as usize,
        )) {
            Ok(s) => s.to_string(),
            Err(_) => return -3,
        };
        let permissions = match parse_permissions_bytes(
            permissions_json_ptr as *const u8,
            permissions_json_len,
        ) {
            Ok(p) => p,
            Err(code) => return code,
        };

        let proof = match parse_proof_bytes(proof_json_ptr as *const u8, proof_json_len) {
            Ok(p) => p,
            Err(code) => return code,
        };

        let algorithm = match parse_algorithm_cstr(alg_ptr, alg_len) {
            Ok(a) => a,
            Err(code) => return code,
        };
        let sig_bytes = std::slice::from_raw_parts(sig_ptr, sig_len as usize);
        let signature = Signature::from_bytes(sig_bytes);
        let ctrl = proto::ConnectControlV1::Approve {
            wallet_pk: wallet_pk_arr,
            account_id,
            permissions,
            proof,
            sig_wallet: proto::WalletSignatureV1::new(algorithm, signature),
        };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_reject(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    code: u16,
    code_id_ptr: *const c_char,
    code_id_len: c_ulong,
    reason_ptr: *const c_char,
    reason_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null() || code_id_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };
        let code_id_bytes =
            std::slice::from_raw_parts(code_id_ptr as *const u8, code_id_len as usize);
        let code_id = match std::str::from_utf8(code_id_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return -3,
        };
        let reason = if !reason_ptr.is_null() && reason_len > 0 {
            let bytes = std::slice::from_raw_parts(reason_ptr as *const u8, reason_len as usize);
            match std::str::from_utf8(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => return -4,
            }
        } else {
            String::new()
        };
        let ctrl = proto::ConnectControlV1::Reject {
            code,
            code_id,
            reason,
        };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_close(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    who_raw: c_uchar,
    code: u16,
    reason_ptr: *const c_char,
    reason_len: c_ulong,
    retryable: c_uchar,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };
        let who = match who_raw {
            0 => proto::Role::App,
            1 => proto::Role::Wallet,
            _ => return -3,
        };
        let reason = if !reason_ptr.is_null() && reason_len > 0 {
            let bytes = std::slice::from_raw_parts(reason_ptr as *const u8, reason_len as usize);
            match std::str::from_utf8(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => return -4,
            }
        } else {
            String::new()
        };
        let ctrl = proto::ConnectControlV1::Close {
            who,
            code,
            reason,
            retryable: retryable != 0,
        };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -5;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_ping(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    nonce: u64,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };
        let ctrl = proto::ConnectControlV1::Ping { nonce };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_control_pong(
    sid_ptr: *const c_uchar,
    dir: c_uchar,
    seq: u64,
    nonce: u64,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sid_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let sid = std::slice::from_raw_parts(sid_ptr, 32);
        let mut sid_arr = [0u8; 32];
        sid_arr.copy_from_slice(sid);
        let dir = match dir {
            0 => proto::Dir::AppToWallet,
            1 => proto::Dir::WalletToApp,
            _ => return -2,
        };
        let ctrl = proto::ConnectControlV1::Pong { nonce };
        let frame = proto::ConnectFrameV1 {
            sid: sid_arr,
            dir,
            seq,
            kind: proto::FrameKind::Control(ctrl),
        };
        let buf = match encode_connect_frame(&frame) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_ciphertext_frame(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_sid_ptr: *mut c_uchar, // must point to 32 bytes
    out_dir: *mut c_uchar,     // 0 or 1
    out_seq: *mut u64,
    out_aead_ptr: *mut *mut c_uchar,
    out_aead_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null()
            || out_sid_ptr.is_null()
            || out_dir.is_null()
            || out_seq.is_null()
            || out_aead_ptr.is_null()
            || out_aead_len.is_null()
        {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let frame = match decode_connect_frame(inp) {
            Ok(f) => f,
            Err(_) => return -2,
        };
        let (dir, seq, ct) = match frame.kind {
            proto::FrameKind::Ciphertext(ct) => (frame.dir, frame.seq, ct),
            _ => return -3,
        };
        ptr::copy_nonoverlapping(frame.sid.as_ptr(), out_sid_ptr, 32);
        *out_dir = match dir {
            proto::Dir::AppToWallet => 0,
            proto::Dir::WalletToApp => 1,
        };
        *out_seq = seq;
        let len = ct.aead.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -4;
        }
        ptr::copy_nonoverlapping(ct.aead.as_ptr(), mem as *mut u8, len);
        *out_aead_ptr = mem as *mut u8;
        *out_aead_len = len as c_ulong;
        0
    }
}

// ---------------- Offline Note prover helpers ----------------

/// Generate a recursive Halo2/IPA proof for an Offline redemption.
///
/// The input is Norito-archive bytes of
/// `iroha_data_model::offline::OfflineNoteRedeem`. The existing
/// `recursive_proof` field is ignored, so callers may pass a stub. The output
/// is Norito-archive bytes of `OfflineNoteRecursiveProof`, ready to slot back
/// into the redemption before transaction submission.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_prove_note_redeem(
    redeem_norito_ptr: *const c_uchar,
    redeem_norito_len: c_ulong,
    out_recursive_proof_ptr: *mut *mut c_uchar,
    out_recursive_proof_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if redeem_norito_ptr.is_null()
            || out_recursive_proof_ptr.is_null()
            || out_recursive_proof_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe { slice::from_raw_parts(redeem_norito_ptr, redeem_norito_len as usize) };
        let recursive = prove_offline_note_redeem_recursive(bytes)?;
        let archive = norito::to_bytes(&recursive).map_err(|_| BridgeError::OfflineNoteProve)?;
        unsafe { write_bytes_bridge(out_recursive_proof_ptr, out_recursive_proof_len, &archive) }
    })();

    bridge_result_to_code(result)
}

/// Generate a recursive Halo2/IPA proof for an Offline audit bundle.
///
/// The input is Norito-archive bytes of
/// `iroha_data_model::offline::OfflineNoteAuditBundle`. The existing
/// `recursive_proof` field is ignored, so callers may pass a stub. The output
/// is Norito-archive bytes of `OfflineNoteRecursiveProof`, ready to slot back
/// into the audit bundle before transaction submission.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_prove_note_audit(
    audit_norito_ptr: *const c_uchar,
    audit_norito_len: c_ulong,
    out_recursive_proof_ptr: *mut *mut c_uchar,
    out_recursive_proof_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if audit_norito_ptr.is_null()
            || out_recursive_proof_ptr.is_null()
            || out_recursive_proof_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe { slice::from_raw_parts(audit_norito_ptr, audit_norito_len as usize) };
        let recursive = prove_offline_note_audit_recursive(bytes)?;
        let archive = norito::to_bytes(&recursive).map_err(|_| BridgeError::OfflineNoteProve)?;
        unsafe { write_bytes_bridge(out_recursive_proof_ptr, out_recursive_proof_len, &archive) }
    })();

    bridge_result_to_code(result)
}

/// Encode and sign a `RedeemOfflineNote` on-chain transaction.
///
/// `redeem_norito` is the Norito archive of `OfflineNoteRedeem` with the
/// recursive proof already embedded. The output is canonical versioned
/// `SignedTransaction` bytes matching the transfer/mint encoders.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_redeem_offline_note_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    redeem_norito_ptr: *const c_uchar,
    redeem_norito_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if redeem_norito_ptr.is_null()
            || private_key_ptr.is_null()
            || out_signed_ptr.is_null()
            || out_signed_len.is_null()
            || out_hash_ptr.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let chain_id: ChainId = unsafe { read_string_bridge(chain_ptr, chain_len) }?
            .parse()
            .map_err(|_| BridgeError::ChainId)?;
        let authority =
            parse_account_id(unsafe { read_string_bridge(authority_ptr, authority_len) }?)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;
        let redeem_bytes =
            unsafe { slice::from_raw_parts(redeem_norito_ptr, redeem_norito_len as usize) };
        let redemption: iroha_data_model::offline::OfflineNoteRedeem =
            norito::decode_from_bytes(redeem_bytes).map_err(|_| BridgeError::OfflineNoteProve)?;
        let instruction = InstructionBox::from(
            iroha_data_model::isi::offline::RedeemOfflineNote::new(redemption),
        );
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            move || Executable::from([instruction]),
        );
        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

/// Encode and sign an `AuditOfflineNote` on-chain transaction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_audit_offline_note_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    audit_norito_ptr: *const c_uchar,
    audit_norito_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if audit_norito_ptr.is_null()
            || private_key_ptr.is_null()
            || out_signed_ptr.is_null()
            || out_signed_len.is_null()
            || out_hash_ptr.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let chain_id: ChainId = unsafe { read_string_bridge(chain_ptr, chain_len) }?
            .parse()
            .map_err(|_| BridgeError::ChainId)?;
        let authority =
            parse_account_id(unsafe { read_string_bridge(authority_ptr, authority_len) }?)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;
        let audit_bytes =
            unsafe { slice::from_raw_parts(audit_norito_ptr, audit_norito_len as usize) };
        let audit: iroha_data_model::offline::OfflineNoteAuditBundle =
            norito::decode_from_bytes(audit_bytes).map_err(|_| BridgeError::OfflineNoteProve)?;
        let instruction =
            InstructionBox::from(iroha_data_model::isi::offline::AuditOfflineNote::new(audit));
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            move || Executable::from([instruction]),
        );
        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

/// Encode and sign an `IssueOfflineNote` on-chain transaction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_issue_offline_note_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    issue_norito_ptr: *const c_uchar,
    issue_norito_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if issue_norito_ptr.is_null()
            || private_key_ptr.is_null()
            || out_signed_ptr.is_null()
            || out_signed_len.is_null()
            || out_hash_ptr.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let chain_id: ChainId = unsafe { read_string_bridge(chain_ptr, chain_len) }?
            .parse()
            .map_err(|_| BridgeError::ChainId)?;
        let authority =
            parse_account_id(unsafe { read_string_bridge(authority_ptr, authority_len) }?)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;
        let issue_bytes =
            unsafe { slice::from_raw_parts(issue_norito_ptr, issue_norito_len as usize) };
        let issue: iroha_data_model::offline::OfflineNoteIssue =
            norito::decode_from_bytes(issue_bytes).map_err(|_| BridgeError::OfflineNoteProve)?;
        let instruction =
            InstructionBox::from(iroha_data_model::isi::offline::IssueOfflineNote::new(issue));
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            move || Executable::from([instruction]),
        );
        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

/// Encode and sign a defund transaction: bearer audits followed by redemption.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_defund_offline_note_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    audit_trail_ptr: *const c_uchar,
    audit_trail_len: c_ulong,
    audit_trail_count: u32,
    redeem_norito_ptr: *const c_uchar,
    redeem_norito_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if redeem_norito_ptr.is_null()
            || private_key_ptr.is_null()
            || out_signed_ptr.is_null()
            || out_signed_len.is_null()
            || out_hash_ptr.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let chain_id: ChainId = unsafe { read_string_bridge(chain_ptr, chain_len) }?
            .parse()
            .map_err(|_| BridgeError::ChainId)?;
        let authority =
            parse_account_id(unsafe { read_string_bridge(authority_ptr, authority_len) }?)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let trail: &[u8] = if audit_trail_count > 0 {
            if audit_trail_ptr.is_null() {
                return Err(BridgeError::NullPtr);
            }
            unsafe { slice::from_raw_parts(audit_trail_ptr, audit_trail_len as usize) }
        } else {
            &[]
        };
        let mut instructions: Vec<InstructionBox> =
            Vec::with_capacity(audit_trail_count as usize + 1);
        let mut cursor: usize = 0;
        for _ in 0..audit_trail_count {
            if cursor + 8 > trail.len() {
                return Err(BridgeError::OfflineNoteProve);
            }
            let len = usize::try_from(u64::from_le_bytes(
                <[u8; 8]>::try_from(&trail[cursor..cursor + 8])
                    .map_err(|_| BridgeError::OfflineNoteProve)?,
            ))
            .map_err(|_| BridgeError::OfflineNoteProve)?;
            cursor += 8;
            if cursor + len > trail.len() {
                return Err(BridgeError::OfflineNoteProve);
            }
            let audit: iroha_data_model::offline::OfflineNoteAuditBundle =
                norito::decode_from_bytes(&trail[cursor..cursor + len])
                    .map_err(|_| BridgeError::OfflineNoteProve)?;
            cursor += len;
            instructions.push(InstructionBox::from(
                iroha_data_model::isi::offline::AuditOfflineNote::new(audit),
            ));
        }
        if cursor != trail.len() {
            return Err(BridgeError::OfflineNoteProve);
        }

        let redeem_bytes =
            unsafe { slice::from_raw_parts(redeem_norito_ptr, redeem_norito_len as usize) };
        let redemption: iroha_data_model::offline::OfflineNoteRedeem =
            norito::decode_from_bytes(redeem_bytes).map_err(|_| BridgeError::OfflineNoteProve)?;
        instructions.push(InstructionBox::from(
            iroha_data_model::isi::offline::RedeemOfflineNote::new(redemption),
        ));

        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            move || Executable::from(instructions),
        );
        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

fn prove_offline_note_redeem_recursive(
    redeem_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::OfflineNoteRecursiveProof> {
    use iroha_core::zk::{
        OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, offline_note_recursive_vk_box, prove_offline_note_redeem,
    };
    use iroha_data_model::{
        offline::{OfflineNoteRecursiveProof, OfflineNoteRedeem},
        proof::VerifyingKeyId,
    };

    let redemption: OfflineNoteRedeem =
        norito::decode_from_bytes(redeem_archive).map_err(|_| BridgeError::OfflineNoteProve)?;
    let vk_box = offline_note_recursive_vk_box().map_err(|_| BridgeError::OfflineNoteProve)?;
    let proof_box = prove_offline_note_redeem(
        OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
        &vk_box,
        &redemption,
        None,
    )
    .map_err(|_| BridgeError::OfflineNoteProve)?;
    let public_inputs_hash = redemption
        .public_inputs_hash()
        .map_err(|_| BridgeError::OfflineNoteProve)?;

    Ok(OfflineNoteRecursiveProof {
        verifier_key_id: VerifyingKeyId::new(
            vk_box.backend.clone(),
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
        ),
        public_inputs_hash,
        proof: proof_box,
    })
}

fn prove_offline_note_audit_recursive(
    audit_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::OfflineNoteRecursiveProof> {
    use iroha_core::zk::{
        OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, offline_note_recursive_vk_box, prove_offline_note_audit,
    };
    use iroha_data_model::{
        offline::{OfflineNoteAuditBundle, OfflineNoteRecursiveProof},
        proof::VerifyingKeyId,
    };

    let audit: OfflineNoteAuditBundle =
        norito::decode_from_bytes(audit_archive).map_err(|_| BridgeError::OfflineNoteProve)?;
    let vk_box = offline_note_recursive_vk_box().map_err(|_| BridgeError::OfflineNoteProve)?;
    let proof_box =
        prove_offline_note_audit(OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, &vk_box, &audit, None)
            .map_err(|_| BridgeError::OfflineNoteProve)?;
    let public_inputs_hash = audit
        .public_inputs_hash()
        .map_err(|_| BridgeError::OfflineNoteProve)?;

    Ok(OfflineNoteRecursiveProof {
        verifier_key_id: VerifyingKeyId::new(
            vk_box.backend.clone(),
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
        ),
        public_inputs_hash,
        proof: proof_box,
    })
}

/// Legacy unanchored Kagemusha compact-token prover entry point.
///
/// This symbol is retained for ABI compatibility only. Production compact-token
/// proving requires verifier-record trust anchors, so callers must use
/// [`connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`].
/// A syntactically valid unanchored
/// `iroha_data_model::offline::KagemushaVerifiedFoldBundle` returns
/// [`ERR_KAGEMUSHA_PROVE`] without producing output bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_prove_verified_compact_payment_token(
    verified_bundle_norito_ptr: *const c_uchar,
    verified_bundle_norito_len: c_ulong,
    out_compact_token_ptr: *mut *mut c_uchar,
    out_compact_token_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if verified_bundle_norito_ptr.is_null()
            || out_compact_token_ptr.is_null()
            || out_compact_token_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe {
            slice::from_raw_parts(
                verified_bundle_norito_ptr,
                verified_bundle_norito_len as usize,
            )
        };
        let _bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle =
            norito::decode_from_bytes(bytes).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe {
            *out_compact_token_ptr = ptr::null_mut();
            *out_compact_token_len = 0;
        }
        Err(BridgeError::KagemushaProve)
    })();

    bridge_result_to_code(result)
}

/// Verify private Kagemusha hop proofs against supplied verifier records and generate a compact token.
///
/// The input is Norito-archive bytes of
/// `iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle`. The bridge
/// enforces verifier-record metadata for every bundled hop before deriving
/// folded public inputs, then returns Norito-archive bytes of
/// `KagemushaCompactPaymentToken`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
    verified_record_bundle_norito_ptr: *const c_uchar,
    verified_record_bundle_norito_len: c_ulong,
    out_compact_token_ptr: *mut *mut c_uchar,
    out_compact_token_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if verified_record_bundle_norito_ptr.is_null()
            || out_compact_token_ptr.is_null()
            || out_compact_token_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe {
            slice::from_raw_parts(
                verified_record_bundle_norito_ptr,
                verified_record_bundle_norito_len as usize,
            )
        };
        let token = prove_verified_kagemusha_compact_token_from_record_bundle(bytes)?;
        let archive = norito::to_bytes(&token).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_compact_token_ptr, out_compact_token_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn prove_verified_kagemusha_compact_token_from_record_bundle(
    verified_record_bundle_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaCompactPaymentToken> {
    use iroha_core::zk::{
        KAGEMUSHA_FOLDED_CIRCUIT_ID, kagemusha_folded_vk_box,
        prove_verified_kagemusha_compact_payment_token_from_record_bundle,
    };
    use iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle;

    let record_bundle: KagemushaVerifiedFoldRecordBundle =
        norito::decode_from_bytes(verified_record_bundle_archive)
            .map_err(|_| BridgeError::KagemushaProve)?;
    let vk_box = kagemusha_folded_vk_box().map_err(|_| BridgeError::KagemushaProve)?;
    prove_verified_kagemusha_compact_payment_token_from_record_bundle(
        &record_bundle,
        KAGEMUSHA_FOLDED_CIRCUIT_ID,
        &vk_box,
        None,
    )
    .map_err(|_| BridgeError::KagemushaProve)
}

/// Verify private Kagemusha hop proofs and Pallas opening envelopes, then generate a recursive proof bundle.
///
/// Inputs are Norito-archive bytes of
/// `iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle` and a
/// `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`. The output is Norito-archive
/// bytes of `KagemushaRecursiveAggregationProofBundle`.
///
/// This symbol is proof-carrying and admission-neutral: compact-token
/// aggregation mode `2` remains reserved until the recursive circuit verifies
/// private-hop opening evidence in-circuit.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
    verified_record_bundle_norito_ptr: *const c_uchar,
    verified_record_bundle_norito_len: c_ulong,
    pallas_open_envelopes_norito_ptr: *const c_uchar,
    pallas_open_envelopes_norito_len: c_ulong,
    out_proof_bundle_ptr: *mut *mut c_uchar,
    out_proof_bundle_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if verified_record_bundle_norito_ptr.is_null()
            || pallas_open_envelopes_norito_ptr.is_null()
            || out_proof_bundle_ptr.is_null()
            || out_proof_bundle_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let record_bundle_bytes = unsafe {
            slice::from_raw_parts(
                verified_record_bundle_norito_ptr,
                verified_record_bundle_norito_len as usize,
            )
        };
        let pallas_open_envelope_bytes = unsafe {
            slice::from_raw_parts(
                pallas_open_envelopes_norito_ptr,
                pallas_open_envelopes_norito_len as usize,
            )
        };
        let proof_bundle =
            prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive(
                record_bundle_bytes,
                pallas_open_envelope_bytes,
            )?;
        let archive = norito::to_bytes(&proof_bundle).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_proof_bundle_ptr, out_proof_bundle_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive(
    verified_record_bundle_archive: &[u8],
    pallas_open_envelopes_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveAggregationProofBundle> {
    use iroha_core::zk::{
        KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID, kagemusha_recursive_aggregation_proof_vk_box,
        prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive,
    };
    use iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle;

    let record_bundle: KagemushaVerifiedFoldRecordBundle =
        norito::decode_from_bytes(verified_record_bundle_archive)
            .map_err(|_| BridgeError::KagemushaProve)?;
    let vk_box =
        kagemusha_recursive_aggregation_proof_vk_box().map_err(|_| BridgeError::KagemushaProve)?;
    prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive(
        &record_bundle,
        pallas_open_envelopes_archive,
        KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
        &vk_box,
        None,
    )
    .map_err(|_| BridgeError::KagemushaProve)
}

/// Initialize production recursive Kagemusha spendable offline cash.
///
/// Input is Norito archive bytes of
/// `iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1`.
/// Output is Norito archive bytes of `KagemushaRecursiveSpendBundleV1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_init(
    request_norito_ptr: *const c_uchar,
    request_norito_len: c_ulong,
    out_bundle_ptr: *mut *mut c_uchar,
    out_bundle_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_norito_ptr.is_null() || out_bundle_ptr.is_null() || out_bundle_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes =
            unsafe { slice::from_raw_parts(request_norito_ptr, request_norito_len as usize) };
        let bundle = kagemusha_recursive_spend_init_from_request_archive(bytes)?;
        let archive = norito::to_bytes(&bundle).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_bundle_ptr, out_bundle_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn kagemusha_recursive_spend_init_from_request_archive(
    request_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendBundleV1> {
    use iroha_core::zk::{
        KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID, kagemusha_recursive_aggregation_proof_vk_box,
        prove_kagemusha_recursive_spend_init_from_record_bundle_and_pallas_open_envelope_archive,
    };
    use iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1;

    let request: KagemushaRecursiveSpendInitRequestV1 =
        norito::decode_from_bytes(request_archive).map_err(|_| BridgeError::KagemushaProve)?;
    let vk_box =
        kagemusha_recursive_aggregation_proof_vk_box().map_err(|_| BridgeError::KagemushaProve)?;
    prove_kagemusha_recursive_spend_init_from_record_bundle_and_pallas_open_envelope_archive(
        &request.record_bundle,
        &request.pallas_open_envelopes_archive,
        request.current_note,
        KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
        &vk_box,
        None,
    )
    .map_err(|_| BridgeError::KagemushaProve)
}

/// Append one hop to production recursive Kagemusha spendable offline cash.
///
/// Input is Norito archive bytes of
/// `iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1`.
/// Output is Norito archive bytes of `KagemushaRecursiveSpendBundleV1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_append(
    request_norito_ptr: *const c_uchar,
    request_norito_len: c_ulong,
    out_bundle_ptr: *mut *mut c_uchar,
    out_bundle_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_norito_ptr.is_null() || out_bundle_ptr.is_null() || out_bundle_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes =
            unsafe { slice::from_raw_parts(request_norito_ptr, request_norito_len as usize) };
        let bundle = kagemusha_recursive_spend_append_from_request_archive(bytes)?;
        let archive = norito::to_bytes(&bundle).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_bundle_ptr, out_bundle_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn kagemusha_recursive_spend_append_from_request_archive(
    request_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendBundleV1> {
    use iroha_core::zk::{
        KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID, kagemusha_recursive_aggregation_proof_vk_box,
        prove_kagemusha_recursive_spend_append_from_record_bundle_and_pallas_open_envelope_archive,
    };
    use iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1;

    let request: KagemushaRecursiveSpendAppendRequestV1 =
        norito::decode_from_bytes(request_archive).map_err(|_| BridgeError::KagemushaProve)?;
    let vk_box =
        kagemusha_recursive_aggregation_proof_vk_box().map_err(|_| BridgeError::KagemushaProve)?;
    prove_kagemusha_recursive_spend_append_from_record_bundle_and_pallas_open_envelope_archive(
        &request.previous_bundle,
        &request.record_bundle,
        &request.pallas_open_envelopes_archive,
        request.current_note,
        KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
        &vk_box,
        None,
    )
    .map_err(|_| BridgeError::KagemushaProve)
}

/// Build the initial record-backed recursive spend lineage witness.
///
/// Inputs are Norito archive bytes of `KagemushaRecursiveSpendInitRequestV1`
/// and the resulting `KagemushaRecursiveSpendBundleV1`. Output is Norito
/// archive bytes of `KagemushaRecursiveSpendLineageWitnessV1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result(
    request_norito_ptr: *const c_uchar,
    request_norito_len: c_ulong,
    bundle_norito_ptr: *const c_uchar,
    bundle_norito_len: c_ulong,
    out_witness_ptr: *mut *mut c_uchar,
    out_witness_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_norito_ptr.is_null()
            || bundle_norito_ptr.is_null()
            || out_witness_ptr.is_null()
            || out_witness_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let request_bytes =
            unsafe { slice::from_raw_parts(request_norito_ptr, request_norito_len as usize) };
        let bundle_bytes =
            unsafe { slice::from_raw_parts(bundle_norito_ptr, bundle_norito_len as usize) };
        let witness = kagemusha_recursive_spend_lineage_witness_from_init_result_archives(
            request_bytes,
            bundle_bytes,
        )?;
        let archive = norito::to_bytes(&witness).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_witness_ptr, out_witness_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn kagemusha_recursive_spend_lineage_witness_from_init_result_archives(
    request_archive: &[u8],
    bundle_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1> {
    use iroha_data_model::offline::{
        KagemushaRecursiveSpendBundleV1, KagemushaRecursiveSpendInitRequestV1,
        kagemusha_recursive_spend_lineage_witness_from_init_result,
    };

    let request: KagemushaRecursiveSpendInitRequestV1 =
        norito::decode_from_bytes(request_archive).map_err(|_| BridgeError::KagemushaProve)?;
    let bundle: KagemushaRecursiveSpendBundleV1 =
        norito::decode_from_bytes(bundle_archive).map_err(|_| BridgeError::KagemushaProve)?;
    kagemusha_recursive_spend_lineage_witness_from_init_result(&request, &bundle)
        .map_err(|_| BridgeError::KagemushaProve)
}

/// Append one hop of record-backed recursive spend lineage witness material.
///
/// Inputs are Norito archive bytes of the previous
/// `KagemushaRecursiveSpendLineageWitnessV1`, the
/// `KagemushaRecursiveSpendAppendRequestV1`, and the resulting
/// `KagemushaRecursiveSpendBundleV1`. Output is Norito archive bytes of the
/// appended `KagemushaRecursiveSpendLineageWitnessV1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_lineage_witness_append_result(
    previous_witness_norito_ptr: *const c_uchar,
    previous_witness_norito_len: c_ulong,
    request_norito_ptr: *const c_uchar,
    request_norito_len: c_ulong,
    bundle_norito_ptr: *const c_uchar,
    bundle_norito_len: c_ulong,
    out_witness_ptr: *mut *mut c_uchar,
    out_witness_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if previous_witness_norito_ptr.is_null()
            || request_norito_ptr.is_null()
            || bundle_norito_ptr.is_null()
            || out_witness_ptr.is_null()
            || out_witness_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let previous_witness_bytes = unsafe {
            slice::from_raw_parts(
                previous_witness_norito_ptr,
                previous_witness_norito_len as usize,
            )
        };
        let request_bytes =
            unsafe { slice::from_raw_parts(request_norito_ptr, request_norito_len as usize) };
        let bundle_bytes =
            unsafe { slice::from_raw_parts(bundle_norito_ptr, bundle_norito_len as usize) };
        let witness = kagemusha_recursive_spend_lineage_witness_append_result_archives(
            previous_witness_bytes,
            request_bytes,
            bundle_bytes,
        )?;
        let archive = norito::to_bytes(&witness).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_witness_ptr, out_witness_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn kagemusha_recursive_spend_lineage_witness_append_result_archives(
    previous_witness_archive: &[u8],
    request_archive: &[u8],
    bundle_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1> {
    use iroha_data_model::offline::{
        KagemushaRecursiveSpendAppendRequestV1, KagemushaRecursiveSpendBundleV1,
        KagemushaRecursiveSpendLineageWitnessV1,
        kagemusha_recursive_spend_lineage_witness_append_result,
    };

    let previous_witness: KagemushaRecursiveSpendLineageWitnessV1 =
        norito::decode_from_bytes(previous_witness_archive)
            .map_err(|_| BridgeError::KagemushaProve)?;
    let request: KagemushaRecursiveSpendAppendRequestV1 =
        norito::decode_from_bytes(request_archive).map_err(|_| BridgeError::KagemushaProve)?;
    let bundle: KagemushaRecursiveSpendBundleV1 =
        norito::decode_from_bytes(bundle_archive).map_err(|_| BridgeError::KagemushaProve)?;
    kagemusha_recursive_spend_lineage_witness_append_result(&previous_witness, &request, &bundle)
        .map_err(|_| BridgeError::KagemushaProve)
}

/// Verify production recursive Kagemusha spendable offline cash.
///
/// Input is Norito archive bytes of
/// `iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1`.
/// Output is Norito archive bytes of `KagemushaRecursiveSpendVerifyResultV1`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_verify(
    request_norito_ptr: *const c_uchar,
    request_norito_len: c_ulong,
    out_result_ptr: *mut *mut c_uchar,
    out_result_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_norito_ptr.is_null() || out_result_ptr.is_null() || out_result_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes =
            unsafe { slice::from_raw_parts(request_norito_ptr, request_norito_len as usize) };
        let result = kagemusha_recursive_spend_verify_from_request_archive(bytes)?;
        let archive = norito::to_bytes(&result).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_result_ptr, out_result_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn kagemusha_recursive_spend_verify_from_request_archive(
    request_archive: &[u8],
) -> BridgeResult<iroha_data_model::offline::KagemushaRecursiveSpendVerifyResultV1> {
    use iroha_core::zk::kagemusha_recursive_spend_verify_result;
    use iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1;

    let request: KagemushaRecursiveSpendVerifyRequestV1 =
        norito::decode_from_bytes(request_archive).map_err(|_| BridgeError::KagemushaProve)?;
    kagemusha_recursive_spend_verify_result(&request.bundle)
        .map_err(|_| BridgeError::KagemushaProve)
}

/// Prepare an online recursive Kagemusha redeem instruction.
///
/// Input is Norito archive bytes of
/// `iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1`.
/// Output is Norito archive bytes of
/// `iroha_data_model::isi::offline::RedeemKagemushaRecursive`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_redeem(
    request_norito_ptr: *const c_uchar,
    request_norito_len: c_ulong,
    out_instruction_ptr: *mut *mut c_uchar,
    out_instruction_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_norito_ptr.is_null()
            || out_instruction_ptr.is_null()
            || out_instruction_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }
        let bytes =
            unsafe { slice::from_raw_parts(request_norito_ptr, request_norito_len as usize) };
        let instruction = kagemusha_recursive_spend_redeem_from_request_archive(bytes)?;
        let archive = norito::to_bytes(&instruction).map_err(|_| BridgeError::KagemushaProve)?;
        unsafe { write_bytes_bridge(out_instruction_ptr, out_instruction_len, &archive) }
    })();

    bridge_result_to_code(result)
}

fn kagemusha_recursive_spend_redeem_from_request_archive(
    request_archive: &[u8],
) -> BridgeResult<iroha_data_model::isi::offline::RedeemKagemushaRecursive> {
    use iroha_core::zk::{
        ensure_kagemusha_recursive_spend_chain_admission_proves_lineage,
        kagemusha_recursive_aggregation_proof_vk_box,
        verify_kagemusha_recursive_spend_lineage_witness_and_bundle_with_vk_box,
    };
    use iroha_data_model::{
        isi::offline::RedeemKagemushaRecursive, offline::KagemushaRecursiveSpendRedeemRequestV1,
    };

    let request: KagemushaRecursiveSpendRedeemRequestV1 =
        norito::decode_from_bytes(request_archive).map_err(|_| BridgeError::KagemushaProve)?;
    request
        .validate_public_binding()
        .map_err(|_| BridgeError::KagemushaProve)?;
    if let Some(lineage_witness) = &request.lineage_witness {
        let vk_box = kagemusha_recursive_aggregation_proof_vk_box()
            .map_err(|_| BridgeError::KagemushaProve)?;
        verify_kagemusha_recursive_spend_lineage_witness_and_bundle_with_vk_box(
            &request.bundle,
            lineage_witness,
            &vk_box,
        )
        .map_err(|_| BridgeError::KagemushaProve)?;
    } else {
        ensure_kagemusha_recursive_spend_chain_admission_proves_lineage(&request.bundle)
            .map_err(|_| BridgeError::KagemushaProve)?;
    }
    Ok(RedeemKagemushaRecursive::new_with_lineage_witness(
        request.bundle,
        request.recipient,
        request.public_amount,
        request.redeem_proof,
        request.lineage_witness,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_free(ptr_: *mut c_uchar) {
    if !ptr_.is_null() {
        unsafe {
            free(ptr_ as *mut _);
        }
    }
}

#[cfg(test)]
mod offline_note_prover_tests {
    use std::{ffi::CString, sync::OnceLock};

    use iroha_core::zk::{
        KAGEMUSHA_HOP_MAX_PROOF_BYTES, OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, ZK_BACKEND_HALO2_IPA,
        confidential_v2, hash_vk, kagemusha_folded_vk_box,
        kagemusha_pallas_open_envelope_metadata_for_verified_hop,
        kagemusha_recursive_aggregation_proof_vk_box,
        kagemusha_recursive_fixed_window_shared_table_manifest_digest,
        kagemusha_recursive_fixed_window_table_schedule_digest,
        kagemusha_verified_folded_public_inputs_from_record_bundle, offline_note_recursive_vk_box,
        verify_backend, verify_kagemusha_compact_payment_token,
        verify_kagemusha_recursive_aggregation_proof_bundle,
    };
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        offline::{
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, KagemushaCompactPaymentToken,
            KagemushaRecursiveAggregationProof, KagemushaRecursiveAggregationProofBundle,
            KagemushaRecursiveSpendAccumulatorV1, KagemushaRecursiveSpendAppendRequestV1,
            KagemushaRecursiveSpendBundleV1, KagemushaRecursiveSpendInitRequestV1,
            KagemushaRecursiveSpendLineageWitnessV1, KagemushaRecursiveSpendRedeemRequestV1,
            KagemushaRecursiveSpendVerifyRequestV1, KagemushaRecursiveSpendVerifyResultV1,
            KagemushaSpendableNoteDescriptorV1, KagemushaVerifiedFoldBundle,
            KagemushaVerifiedFoldRecordBundle, KagemushaVerifiedFoldStep,
            KagemushaVerifiedFoldVerifierRecord, OfflineNoteAuditBundle,
            OfflineNoteAuditOutputClaim, OfflineNoteIssue, OfflineNoteIssuedClaim,
            OfflineNoteKeyCertificate, OfflineNoteRecursiveProof, OfflineNoteRedeem,
            kagemusha_recursive_spend_public_inputs_from_accumulator,
        },
        proof::VerifyingKeyId,
    };

    use super::*;

    fn sample_signature(seed: u8) -> Signature {
        let mut bytes = [0u8; 64];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = seed.wrapping_add(u8::try_from(index).expect("signature index fits"));
        }
        Signature::from_bytes(&bytes)
    }

    fn sample_account(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn sample_authority_and_private_key(seed: u8) -> (CString, Vec<u8>) {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, private_key) = keypair.into_parts();
        let account = AccountId::new(public_key);
        let (_algorithm, private_bytes) = private_key.to_bytes();
        (
            CString::new(account.to_string()).expect("valid cstring"),
            private_bytes,
        )
    }

    fn sample_asset(account: AccountId) -> AssetId {
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "xor".parse().expect("asset definition name"),
        );
        AssetId::new(definition, account)
    }

    fn fixed_bytes(label: &[u8]) -> [u8; Hash::LENGTH] {
        Hash::new(label).into()
    }

    fn sample_kagemusha_recursive_spend_bundle() -> KagemushaRecursiveSpendBundleV1 {
        let chain_id: ChainId = "kagemusha-recursive-spend-bridge"
            .parse()
            .expect("chain id");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "kgmr".parse().expect("asset definition name"),
        );
        let current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_bytes(b"bridge-recursive-current-note"),
            spend_nullifier: fixed_bytes(b"bridge-recursive-current-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let mut topup_anchor_nullifiers = vec![
            fixed_bytes(b"bridge-recursive-topup-anchor-0"),
            fixed_bytes(b"bridge-recursive-topup-anchor-1"),
        ];
        topup_anchor_nullifiers.sort_unstable();
        let verifier_opening_len = 4;
        let accumulator = KagemushaRecursiveSpendAccumulatorV1 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN.to_owned(),
            chain_id,
            asset,
            initial_root: fixed_bytes(b"bridge-recursive-initial-root"),
            final_root: fixed_bytes(b"bridge-recursive-final-root"),
            topup_anchor_nullifiers,
            hop_count: 2,
            lineage_digest: fixed_bytes(b"bridge-recursive-lineage"),
            aggregation_transcript_digest: fixed_bytes(b"bridge-recursive-lineage"),
            nullifier_digest: Hash::new(b"bridge-recursive-nullifier-digest"),
            output_commitment_digest: Hash::new(b"bridge-recursive-output-digest"),
            fold_digest: Hash::new(b"bridge-recursive-fold-digest"),
            recursive_proof_chain_digest: fixed_bytes(b"bridge-recursive-proof-chain"),
            verifier_params_fingerprint: fixed_bytes(b"bridge-recursive-params"),
            fixed_window_table_schedule_digest:
                kagemusha_recursive_fixed_window_table_schedule_digest(verifier_opening_len)
                    .expect("canonical recursive schedule digest"),
            fixed_window_shared_table_manifest_digest:
                kagemusha_recursive_fixed_window_shared_table_manifest_digest(verifier_opening_len)
                    .expect("canonical recursive shared-table manifest digest"),
            fixed_window_table_base_digest: fixed_bytes(b"bridge-recursive-table-base"),
            verifier_witness_batch_digest: fixed_bytes(b"bridge-recursive-witness-batch"),
            verifier_opening_len: u32::try_from(verifier_opening_len)
                .expect("verifier opening length fits u32"),
            current_note,
        };
        let public_inputs = kagemusha_recursive_spend_public_inputs_from_accumulator(&accumulator)
            .expect("recursive spend public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive spend public-input hash");
        KagemushaRecursiveSpendBundleV1 {
            accumulator,
            recursive_proof: KagemushaRecursiveAggregationProof {
                verifier_key_id: VerifyingKeyId::new(
                    ZK_BACKEND_HALO2_IPA,
                    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                ),
                public_inputs,
                public_inputs_hash,
                proof: ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xA5; 64]),
            },
        }
    }

    fn sample_verifying_semantic_recursive_spend_bundle() -> KagemushaRecursiveSpendBundleV1 {
        sample_verifying_semantic_recursive_spend_lineage_fixture().0
    }

    fn sample_verifying_semantic_recursive_spend_lineage_fixture() -> (
        KagemushaRecursiveSpendBundleV1,
        KagemushaRecursiveSpendLineageWitnessV1,
    ) {
        static FIXTURE: OnceLock<(
            KagemushaRecursiveSpendBundleV1,
            KagemushaRecursiveSpendLineageWitnessV1,
        )> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let record_bundle = sample_kagemusha_verified_record_bundle();
                let metadata = kagemusha_pallas_open_envelope_metadata_for_verified_hop(
                    &record_bundle.bundle.chain_id,
                    &record_bundle.bundle.asset,
                    0,
                    &record_bundle.bundle.steps[0],
                )
                .expect("Pallas open-envelope hop metadata");
                let envelope = sample_pallas_open_envelope_with_metadata(
                    4,
                    "bridge-recursive-spend-verify-open-envelope",
                    metadata,
                );
                let envelope_archive =
                    norito::to_bytes(&vec![envelope]).expect("encode Pallas envelope archive");
                let current_note = KagemushaSpendableNoteDescriptorV1 {
                    note_commitment: record_bundle.bundle.steps[0].output_commitments[0],
                    spend_nullifier: fixed_bytes(
                        b"bridge-recursive-spend-verify-current-nullifier",
                    ),
                    amount: Numeric::new(7, 0),
                };
                let request = KagemushaRecursiveSpendInitRequestV1 {
                    record_bundle: record_bundle.clone(),
                    pallas_open_envelopes_archive: envelope_archive.clone(),
                    current_note: current_note.clone(),
                };
                let archive =
                    norito::to_bytes(&request).expect("encode recursive spend init request");
                kagemusha_recursive_spend_init_from_request_archive(&archive)
                    .map(|bundle| {
                        (
                            bundle,
                            KagemushaRecursiveSpendLineageWitnessV1 {
                                record_bundle,
                                pallas_open_envelopes_archive: envelope_archive,
                                current_notes: vec![current_note],
                                previous_recursive_proofs: Vec::new(),
                            },
                        )
                    })
                    .expect("semantic recursive spend init bundle with lineage witness")
            })
            .clone()
    }

    fn sample_recursive_spend_redeem_request(
        public_amount: u128,
    ) -> KagemushaRecursiveSpendRedeemRequestV1 {
        let mut redeem_proof = ProofAttachment::new_ref(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![0x5A; 64]),
            VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "bridge-recursive-unshield"),
        );
        redeem_proof.vk_commitment = Some(fixed_bytes(b"bridge-recursive-unshield-vk"));
        KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: sample_kagemusha_recursive_spend_bundle(),
            recipient: sample_account(0xB8),
            public_amount,
            redeem_proof,
            lineage_witness: None,
        }
    }

    fn sample_certificate(account: &AccountId, seed: u8) -> OfflineNoteKeyCertificate {
        let note_keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (_algorithm, public_key) = note_keypair
            .public_key()
            .try_to_bytes()
            .expect("checked public bytes");
        OfflineNoteKeyCertificate {
            version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: format!("one-use-key-{seed}"),
            device_id: "device-1".to_owned(),
            account_id: account.clone(),
            public_key: public_key.to_vec(),
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: sample_signature(seed.wrapping_add(1)),
        }
    }

    fn sample_issue() -> OfflineNoteIssue {
        let account = sample_account(0x91);
        OfflineNoteIssue {
            note_commitment: Hash::new(b"offline-note-issued-note"),
            key_certificate: sample_certificate(&account, 0x92),
            asset: sample_asset(account),
            amount: Numeric::new(10, 0),
        }
    }

    fn placeholder_recursive_proof() -> OfflineNoteRecursiveProof {
        OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new(
                ZK_BACKEND_HALO2_IPA,
                OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            ),
            public_inputs_hash: Hash::new(b"placeholder-offline-note-public-inputs"),
            proof: ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new()),
        }
    }

    fn sample_redemption() -> OfflineNoteRedeem {
        let account = sample_account(0xA1);
        let asset = sample_asset(account.clone());
        OfflineNoteRedeem {
            source_note_commitment: Hash::new(b"offline-note-source-note"),
            input_nullifiers: vec![Hash::new(b"offline-note-redeem-nullifier")],
            sender_key_certificate: sample_certificate(&account, 0xB1),
            recipient: account,
            asset,
            amount: Numeric::new(10, 0),
            recursive_proof: placeholder_recursive_proof(),
        }
    }

    fn sample_audit() -> OfflineNoteAuditBundle {
        let account = sample_account(0xC1);
        let asset = sample_asset(account.clone());
        let certificate = sample_certificate(&account, 0xD1);
        let issue = OfflineNoteIssue {
            note_commitment: Hash::new(b"offline-note-audit-input-note"),
            key_certificate: certificate.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
        };
        OfflineNoteAuditBundle {
            token_id: Hash::new(b"offline-note-audit-token"),
            sender_key_certificate: certificate.clone(),
            input_nullifiers: vec![Hash::new(b"offline-note-audit-nullifier")],
            input_claims: vec![OfflineNoteIssuedClaim::from_issue(&issue).expect("input claim")],
            output_commitments: vec![Hash::new(b"offline-note-audit-output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-audit-output-note"),
                key_certificate: certificate,
                asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: placeholder_recursive_proof(),
        }
    }

    fn sample_kagemusha_verified_bundle() -> KagemushaVerifiedFoldBundle {
        sample_kagemusha_verified_record_bundle().bundle
    }

    fn sample_kagemusha_verified_record_bundle() -> KagemushaVerifiedFoldRecordBundle {
        static BUNDLE: OnceLock<KagemushaVerifiedFoldRecordBundle> = OnceLock::new();
        BUNDLE
            .get_or_init(|| {
                let chain_id: ChainId = "kagemusha-bridge-chain".parse().expect("chain id");
                let asset = AssetDefinitionId::new(
                    DomainId::try_new("offline", "universal").expect("domain id"),
                    "kgm".parse().expect("asset definition name"),
                );
                let record = confidential_v2::confidential_transfer_v2_vk_record(
                    iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                    3,
                )
                .expect("confidential transfer v2 verifier record");
                let verifier_key = record.key.clone().expect("inline transfer verifier key");
                let spend_key = [0x11_u8; Hash::LENGTH];
                let input_rho = [0x21_u8; Hash::LENGTH];
                let input_diversifier =
                    confidential_v2::derive_confidential_diversifier_v2(b"kagemusha-bridge-input");
                let input_owner_tag =
                    confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &spend_key,
                        input_diversifier,
                    )
                    .expect("input owner tag");
                let input_commitment = confidential_v2::derive_confidential_note_v2(
                    &asset.to_string(),
                    7,
                    input_rho,
                    input_owner_tag,
                )
                .expect("input commitment");
                let tree_commitments = vec![input_commitment];
                let root_before = confidential_v2::compute_confidential_root_v2(&tree_commitments)
                    .expect("root before");
                let output_owner_tag =
                    confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &[0x41_u8; Hash::LENGTH],
                        confidential_v2::derive_confidential_diversifier_v2(
                            b"kagemusha-bridge-output",
                        ),
                    )
                    .expect("output owner tag");
                let proof = confidential_v2::build_confidential_transfer_proof_v2(
                    &chain_id,
                    &asset.to_string(),
                    &spend_key,
                    &tree_commitments,
                    &[confidential_v2::ConfidentialTransferInputV2 {
                        amount: 7,
                        rho: input_rho,
                        diversifier: input_diversifier,
                        leaf_index: 0,
                    }],
                    &[confidential_v2::ConfidentialTransferOutputV2 {
                        amount: 7,
                        rho: [0x31_u8; Hash::LENGTH],
                        owner_tag: output_owner_tag,
                    }],
                    root_before,
                    &record.circuit_id,
                    &verifier_key,
                )
                .expect("confidential transfer v2 proof");
                let mut next_tree = tree_commitments;
                next_tree.extend(proof.output_commitments.iter().copied());
                let root_after =
                    confidential_v2::compute_confidential_root_v2(&next_tree).expect("root after");
                let mut attachment = ProofAttachment::new_ref(
                    ZK_BACKEND_HALO2_IPA.into(),
                    proof.proof,
                    VerifyingKeyId::new(
                        ZK_BACKEND_HALO2_IPA,
                        "kagemusha-bridge-confidential-transfer-v2",
                    ),
                );
                attachment.vk_commitment = Some(hash_vk(&verifier_key));
                let step = KagemushaVerifiedFoldStep {
                    root_before,
                    input_nullifiers: proof.nullifiers,
                    output_commitments: proof.output_commitments,
                    root_after,
                    attachment,
                    verifier_key,
                };
                let id = step.attachment.vk_ref.clone();
                KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id,
                        asset,
                        steps: vec![step],
                    },
                    verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id, record }],
                }
            })
            .clone()
    }

    fn decode_recursive_proof(
        out_ptr: *mut c_uchar,
        out_len: c_ulong,
    ) -> OfflineNoteRecursiveProof {
        assert!(!out_ptr.is_null(), "prover output pointer must be set");
        let out = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        norito::decode_from_bytes(&out).expect("decode recursive proof")
    }

    fn decode_kagemusha_compact_token(
        out_ptr: *mut c_uchar,
        out_len: c_ulong,
    ) -> KagemushaCompactPaymentToken {
        assert!(!out_ptr.is_null(), "prover output pointer must be set");
        let out = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        norito::decode_from_bytes(&out).expect("decode Kagemusha compact token")
    }

    fn decode_kagemusha_recursive_aggregation_proof_bundle(
        out_ptr: *mut c_uchar,
        out_len: c_ulong,
    ) -> KagemushaRecursiveAggregationProofBundle {
        assert!(!out_ptr.is_null(), "prover output pointer must be set");
        let out = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        norito::decode_from_bytes(&out)
            .expect("decode Kagemusha recursive aggregation proof bundle")
    }

    fn decode_kagemusha_recursive_spend_lineage_witness(
        out_ptr: *mut c_uchar,
        out_len: c_ulong,
    ) -> KagemushaRecursiveSpendLineageWitnessV1 {
        assert!(!out_ptr.is_null(), "prover output pointer must be set");
        let out = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        norito::decode_from_bytes(&out).expect("decode Kagemusha recursive spend lineage witness")
    }

    fn mutate_kagemusha_bundle_hop_envelope(
        bundle: &mut KagemushaVerifiedFoldBundle,
        mutate: impl FnOnce(&mut iroha_data_model::zk::OpenVerifyEnvelope),
    ) {
        let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&bundle.steps[0].attachment.proof.bytes)
                .expect("Kagemusha hop proof should be an OpenVerifyEnvelope");
        mutate(&mut envelope);
        bundle.steps[0].attachment.proof.bytes =
            norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
    }

    fn sample_pallas_coeffs(n: usize) -> Vec<iroha_zkp_halo2::pallas::Scalar> {
        (0..n)
            .map(|index| iroha_zkp_halo2::pallas::Scalar::from((index + 1) as u64))
            .collect()
    }

    fn sample_pallas_open_envelope_with_metadata(
        n: usize,
        label: &str,
        metadata: iroha_zkp_halo2::PolyOpenTranscriptMetadata,
    ) -> iroha_zkp_halo2::OpenVerifyEnvelope {
        let params = iroha_zkp_halo2::pallas::Params::new(n).expect("Pallas params");
        let poly = iroha_zkp_halo2::pallas::Polynomial::from_coeffs(sample_pallas_coeffs(n));
        let commitment = poly.commit(&params).expect("Pallas commitment");
        let z = iroha_zkp_halo2::pallas::Scalar::from(5_u64);
        let mut transcript = iroha_zkp_halo2::Transcript::new(label);
        let (proof, t) = poly
            .open_with_metadata(&params, &mut transcript, z, commitment, metadata)
            .expect("Pallas opening proof");
        iroha_zkp_halo2::OpenVerifyEnvelope {
            params: iroha_zkp_halo2::norito_helpers::params_to_wire(&params),
            public: iroha_zkp_halo2::norito_helpers::poly_open_public::<
                iroha_zkp_halo2::pallas::PallasBackend,
            >(params.n(), z, t, commitment),
            proof: iroha_zkp_halo2::norito_helpers::proof_to_wire(&proof),
            transcript_label: label.to_owned(),
            vk_commitment: metadata.vk_commitment,
            public_inputs_schema_hash: metadata.public_inputs_schema_hash,
            domain_tag: metadata.domain_tag,
        }
    }

    fn assert_recursive_proof_verifies(
        recursive: &OfflineNoteRecursiveProof,
        expected_public_inputs_hash: Hash,
    ) {
        let vk_box = offline_note_recursive_vk_box().expect("offline note verifying key");
        assert_eq!(
            recursive.verifier_key_id,
            VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID)
        );
        assert_eq!(recursive.public_inputs_hash, expected_public_inputs_hash);
        assert!(
            verify_backend(ZK_BACKEND_HALO2_IPA, &recursive.proof, Some(&vk_box)),
            "bridge output must verify against the canonical Offline verifier"
        );
    }

    #[test]
    fn bridge_abi_version_advertises_kagemusha_compact_prover() {
        assert_eq!(unsafe { connect_norito_bridge_abi_version() }, 6);
    }

    #[test]
    fn kagemusha_unanchored_compact_token_ffi_rejects_valid_bundle_without_records() {
        let bundle = sample_kagemusha_verified_bundle();
        let archive = norito::to_bytes(&bundle).expect("encode verified fold bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(
            out_ptr.is_null(),
            "unanchored bridge entry point must not return an output buffer"
        );
        assert_eq!(
            out_len, 0,
            "unanchored bridge entry point must not return output bytes"
        );
    }

    #[test]
    #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
    fn kagemusha_verified_record_compact_token_ffi_returns_verifying_token() {
        let record_bundle = sample_kagemusha_verified_record_bundle();
        let expected_public_inputs =
            kagemusha_verified_folded_public_inputs_from_record_bundle(&record_bundle)
                .expect("record-backed verified folded public inputs");
        let archive =
            norito::to_bytes(&record_bundle).expect("encode record-backed verified fold bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, 0);
        let token = decode_kagemusha_compact_token(out_ptr, out_len);
        assert_eq!(token.public_inputs, expected_public_inputs);
        let vk_box = kagemusha_folded_vk_box().expect("Kagemusha folded verifying key");
        assert!(
            verify_kagemusha_compact_payment_token(&token, &vk_box),
            "record-backed bridge output must verify against the canonical Kagemusha verifier"
        );
    }

    #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    ))]
    #[test]
    #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
    fn kagemusha_verified_record_compact_token_jni_helper_uses_record_checks() {
        let record_bundle = sample_kagemusha_verified_record_bundle();
        let expected_public_inputs =
            kagemusha_verified_folded_public_inputs_from_record_bundle(&record_bundle)
                .expect("record-backed verified folded public inputs");
        let archive =
            norito::to_bytes(&record_bundle).expect("encode record-backed verified fold bundle");

        let token_archive =
            java_kagemusha_prove_verified_compact_payment_token_with_records(&archive)
                .expect("JNI helper should produce compact token archive");
        let token: KagemushaCompactPaymentToken =
            norito::decode_from_bytes(&token_archive).expect("decode compact token");

        assert_eq!(token.public_inputs, expected_public_inputs);
        let vk_box = kagemusha_folded_vk_box().expect("Kagemusha folded verifying key");
        assert!(
            verify_kagemusha_compact_payment_token(&token, &vk_box),
            "JNI helper output must verify against the canonical Kagemusha verifier"
        );

        let mut inactive = record_bundle;
        inactive.verifier_records[0].record.status = ConfidentialStatus::Withdrawn;
        let inactive_archive = norito::to_bytes(&inactive).expect("encode inactive-record bundle");
        assert!(
            java_kagemusha_prove_verified_compact_payment_token_with_records(&inactive_archive)
                .is_err(),
            "JNI helper must reject inactive verifier records"
        );
    }

    #[test]
    #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
    fn kagemusha_verified_record_recursive_aggregation_proof_bundle_ffi_returns_verifying_bundle() {
        let record_bundle = sample_kagemusha_verified_record_bundle();
        let metadata = kagemusha_pallas_open_envelope_metadata_for_verified_hop(
            &record_bundle.bundle.chain_id,
            &record_bundle.bundle.asset,
            0,
            &record_bundle.bundle.steps[0],
        )
        .expect("Pallas open-envelope hop metadata");
        let envelope = sample_pallas_open_envelope_with_metadata(
            4,
            "bridge-recursive-aggregation-open-envelope",
            metadata,
        );
        let record_archive =
            norito::to_bytes(&record_bundle).expect("encode record-backed verified fold bundle");
        let envelope_archive =
            norito::to_bytes(&vec![envelope]).expect("encode Pallas open-envelope archive");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                record_archive.as_ptr(),
                record_archive.len() as c_ulong,
                envelope_archive.as_ptr(),
                envelope_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, 0);
        let proof_bundle = decode_kagemusha_recursive_aggregation_proof_bundle(out_ptr, out_len);
        proof_bundle
            .validate_evidence_binding()
            .expect("recursive proof bundle evidence binding");
        assert_eq!(proof_bundle.evidence.verifier_witness_count, 1);
        let recursive_vk =
            kagemusha_recursive_aggregation_proof_vk_box().expect("recursive aggregation VK");
        assert!(
            verify_kagemusha_recursive_aggregation_proof_bundle(&proof_bundle, &recursive_vk),
            "bridge output must verify against the canonical recursive aggregation verifier"
        );
    }

    #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    ))]
    #[test]
    #[ignore = "heavy Kagemusha Halo2 IPA proof generation; run explicitly with --ignored --test-threads=1"]
    fn kagemusha_verified_record_recursive_aggregation_proof_bundle_jni_helper_uses_record_checks()
    {
        let record_bundle = sample_kagemusha_verified_record_bundle();
        let metadata = kagemusha_pallas_open_envelope_metadata_for_verified_hop(
            &record_bundle.bundle.chain_id,
            &record_bundle.bundle.asset,
            0,
            &record_bundle.bundle.steps[0],
        )
        .expect("Pallas open-envelope hop metadata");
        let envelope = sample_pallas_open_envelope_with_metadata(
            4,
            "bridge-recursive-aggregation-jni-open-envelope",
            metadata,
        );
        let record_archive =
            norito::to_bytes(&record_bundle).expect("encode record-backed verified fold bundle");
        let envelope_archive =
            norito::to_bytes(&vec![envelope]).expect("encode Pallas open-envelope archive");

        let proof_bundle_archive =
            java_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                &record_archive,
                &envelope_archive,
            )
            .expect("JNI helper should produce recursive aggregation proof bundle archive");
        let proof_bundle: KagemushaRecursiveAggregationProofBundle =
            norito::decode_from_bytes(&proof_bundle_archive)
                .expect("decode recursive aggregation proof bundle");
        let recursive_vk =
            kagemusha_recursive_aggregation_proof_vk_box().expect("recursive aggregation VK");
        assert!(
            verify_kagemusha_recursive_aggregation_proof_bundle(&proof_bundle, &recursive_vk),
            "JNI helper output must verify against the canonical recursive aggregation verifier"
        );

        let mut inactive = record_bundle;
        inactive.verifier_records[0].record.status = ConfidentialStatus::Withdrawn;
        let inactive_archive = norito::to_bytes(&inactive).expect("encode inactive-record bundle");
        assert!(
            java_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                &inactive_archive,
                &envelope_archive,
            )
            .is_err(),
            "JNI helper must reject inactive verifier records"
        );
    }

    #[test]
    fn kagemusha_verified_record_recursive_aggregation_proof_bundle_ffi_rejects_adversarial_inputs()
    {
        let record_bundle = sample_kagemusha_verified_record_bundle();
        let metadata = kagemusha_pallas_open_envelope_metadata_for_verified_hop(
            &record_bundle.bundle.chain_id,
            &record_bundle.bundle.asset,
            0,
            &record_bundle.bundle.steps[0],
        )
        .expect("Pallas open-envelope hop metadata");
        let envelope = sample_pallas_open_envelope_with_metadata(
            4,
            "bridge-recursive-aggregation-reject-open-envelope",
            metadata,
        );
        let record_archive =
            norito::to_bytes(&record_bundle).expect("encode record-backed verified fold bundle");
        let envelope_archive =
            norito::to_bytes(&vec![envelope.clone()]).expect("encode Pallas open-envelope archive");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let malformed_envelope_archive = b"not a norito archive";
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                record_archive.as_ptr(),
                record_archive.len() as c_ulong,
                malformed_envelope_archive.as_ptr(),
                malformed_envelope_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut missing = record_bundle.clone();
        missing.verifier_records.clear();
        let missing_archive = norito::to_bytes(&missing).expect("encode missing-record bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                missing_archive.as_ptr(),
                missing_archive.len() as c_ulong,
                envelope_archive.as_ptr(),
                envelope_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut wrong_schema = envelope;
        wrong_schema.public_inputs_schema_hash = Some([0x7B; Hash::LENGTH]);
        let wrong_schema_archive =
            norito::to_bytes(&vec![wrong_schema]).expect("encode wrong-schema envelope archive");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                record_archive.as_ptr(),
                record_archive.len() as c_ulong,
                wrong_schema_archive.as_ptr(),
                wrong_schema_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_record_compact_token_ffi_rejects_bad_records() {
        let mut missing = sample_kagemusha_verified_record_bundle();
        missing.verifier_records.clear();
        let archive = norito::to_bytes(&missing).expect("encode missing-record bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut extra = sample_kagemusha_verified_record_bundle();
        extra
            .verifier_records
            .push(KagemushaVerifiedFoldVerifierRecord {
                id: VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "unused-kagemusha-hop-vk"),
                record: extra.verifier_records[0].record.clone(),
            });
        let archive = norito::to_bytes(&extra).expect("encode extra-record bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut inactive = sample_kagemusha_verified_record_bundle();
        inactive.verifier_records[0].record.status = ConfidentialStatus::Withdrawn;
        let archive = norito::to_bytes(&inactive).expect("encode inactive-record bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut wrong_namespace = sample_kagemusha_verified_record_bundle();
        wrong_namespace.verifier_records[0].record.namespace =
            "generic_confidential_transfer".to_owned();
        let archive = norito::to_bytes(&wrong_namespace).expect("encode wrong-namespace bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut wrong_circuit_alias = sample_kagemusha_verified_record_bundle();
        wrong_circuit_alias.verifier_records[0].record.circuit_id =
            "anon-transfer-2x2-merkle16-poseidon-diversified".to_owned();
        let archive =
            norito::to_bytes(&wrong_circuit_alias).expect("encode wrong-circuit-alias bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut wrong_envelope_alias = sample_kagemusha_verified_record_bundle();
        mutate_kagemusha_bundle_hop_envelope(&mut wrong_envelope_alias.bundle, |envelope| {
            envelope.circuit_id = "anon-transfer-2x2-merkle16-poseidon-diversified".to_owned();
        });
        let archive =
            norito::to_bytes(&wrong_envelope_alias).expect("encode wrong-envelope-alias bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut wrong_schema = sample_kagemusha_verified_record_bundle();
        wrong_schema.verifier_records[0]
            .record
            .public_inputs_schema_hash = [0x33; Hash::LENGTH];
        let archive = norito::to_bytes(&wrong_schema).expect("encode wrong-schema bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let mut too_small = sample_kagemusha_verified_record_bundle();
        too_small.verifier_records[0].record.max_proof_bytes = u32::try_from(
            too_small.bundle.steps[0]
                .attachment
                .proof
                .bytes
                .len()
                .saturating_sub(1),
        )
        .expect("proof length fits u32");
        let archive = norito::to_bytes(&too_small).expect("encode proof-cap bundle");
        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_record_compact_token_ffi_rejects_forged_envelope_hash_metadata() {
        let mut record_bundle = sample_kagemusha_verified_record_bundle();
        record_bundle.bundle.steps[0].attachment.envelope_hash = Some([0xA7; Hash::LENGTH]);
        let archive =
            norito::to_bytes(&record_bundle).expect("encode forged-envelope-hash record bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_record_compact_token_ffi_rejects_hop_envelope_aux_substitution() {
        let mut record_bundle = sample_kagemusha_verified_record_bundle();
        mutate_kagemusha_bundle_hop_envelope(&mut record_bundle.bundle, |envelope| {
            envelope.aux = b"kagemusha-forged-record-hop-aux".to_vec();
        });
        let archive =
            norito::to_bytes(&record_bundle).expect("encode forged-hop-aux record bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token_with_records(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_missing_trust_anchor_metadata() {
        type BundleMutator = fn(&mut KagemushaVerifiedFoldBundle);

        let cases: [(&str, BundleMutator); 2] = [
            ("missing verifier-key commitment", |bundle| {
                bundle.steps[0].attachment.vk_commitment = None;
            }),
            ("empty verifier-key id name", |bundle| {
                bundle.steps[0].attachment.vk_ref.name = "   ".to_owned();
            }),
        ];

        for (case, mutate) in cases {
            let mut bundle = sample_kagemusha_verified_bundle();
            mutate(&mut bundle);
            let archive = norito::to_bytes(&bundle).expect("encode mutated trust-anchor bundle");
            let mut out_ptr: *mut c_uchar = ptr::null_mut();
            let mut out_len: c_ulong = 0;

            let status = unsafe {
                connect_norito_kagemusha_prove_verified_compact_payment_token(
                    archive.as_ptr(),
                    archive.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            };

            assert_eq!(status, ERR_KAGEMUSHA_PROVE, "{case} must reject");
            assert!(out_ptr.is_null(), "{case} must not return an output buffer");
            assert_eq!(out_len, 0, "{case} must not return output bytes");
        }
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_invalid_archive() {
        let bad_archive = b"not a norito archive";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                bad_archive.as_ptr(),
                bad_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_recursive_spend_ffi_rejects_invalid_archives_without_output() {
        type RecursiveSpendFfi =
            unsafe extern "C" fn(*const c_uchar, c_ulong, *mut *mut c_uchar, *mut c_ulong) -> c_int;

        let entries: [(&str, RecursiveSpendFfi); 4] = [
            (
                "init",
                connect_norito_kagemusha_recursive_spend_init as RecursiveSpendFfi,
            ),
            (
                "append",
                connect_norito_kagemusha_recursive_spend_append as RecursiveSpendFfi,
            ),
            (
                "verify",
                connect_norito_kagemusha_recursive_spend_verify as RecursiveSpendFfi,
            ),
            (
                "redeem",
                connect_norito_kagemusha_recursive_spend_redeem as RecursiveSpendFfi,
            ),
        ];
        let bad_archives: [(&str, &[u8]); 2] =
            [("empty", &[]), ("malformed", b"not a norito archive")];

        for (entry_name, entry) in entries {
            for (case, archive) in bad_archives {
                let mut out_ptr: *mut c_uchar = ptr::null_mut();
                let mut out_len: c_ulong = 0;
                let status = unsafe {
                    entry(
                        archive.as_ptr(),
                        archive.len() as c_ulong,
                        &mut out_ptr,
                        &mut out_len,
                    )
                };
                assert_eq!(status, ERR_KAGEMUSHA_PROVE, "{entry_name} {case}");
                assert!(
                    out_ptr.is_null(),
                    "{entry_name} {case} must not return bytes"
                );
                assert_eq!(out_len, 0, "{entry_name} {case} must not set length");
            }

            let mut out_ptr: *mut c_uchar = ptr::null_mut();
            let mut out_len: c_ulong = 0;
            let status = unsafe { entry(ptr::null(), 0, &mut out_ptr, &mut out_len) };
            assert_eq!(status, ERR_NULL_PTR, "{entry_name} null request");
            assert!(out_ptr.is_null(), "{entry_name} null must not return bytes");
            assert_eq!(out_len, 0, "{entry_name} null must not set length");
        }
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_bridge_helpers_reconstruct_init_witness() {
        let (bundle, witness) = sample_verifying_semantic_recursive_spend_lineage_fixture();
        let current_note = witness
            .current_notes
            .first()
            .expect("one-hop lineage witness current note")
            .clone();
        let request = KagemushaRecursiveSpendInitRequestV1 {
            record_bundle: witness.record_bundle.clone(),
            pallas_open_envelopes_archive: witness.pallas_open_envelopes_archive.clone(),
            current_note,
        };
        let request_archive = norito::to_bytes(&request).expect("encode init request");
        let bundle_archive = norito::to_bytes(&bundle).expect("encode recursive spend bundle");

        let rebuilt = kagemusha_recursive_spend_lineage_witness_from_init_result_archives(
            &request_archive,
            &bundle_archive,
        )
        .expect("rebuild lineage witness from matching init request and bundle");
        assert_eq!(rebuilt, witness);

        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result(
                request_archive.as_ptr(),
                request_archive.len() as c_ulong,
                bundle_archive.as_ptr(),
                bundle_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, 0);
        let ffi_witness = decode_kagemusha_recursive_spend_lineage_witness(out_ptr, out_len);
        assert_eq!(ffi_witness, witness);

        let wrong_bundle_archive =
            norito::to_bytes(&sample_kagemusha_recursive_spend_bundle()).expect("encode mismatch");
        assert!(
            kagemusha_recursive_spend_lineage_witness_from_init_result_archives(
                &request_archive,
                &wrong_bundle_archive,
            )
            .is_err(),
            "lineage helper must reject bundles that were not produced by the init request"
        );

        let witness_archive = norito::to_bytes(&witness).expect("encode lineage witness");
        assert!(
            kagemusha_recursive_spend_lineage_witness_append_result_archives(
                &witness_archive,
                b"not a recursive append request",
                &bundle_archive,
            )
            .is_err(),
            "append lineage helper must reject malformed append requests"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_ffi_rejects_invalid_inputs_without_output() {
        let good = norito::to_bytes(&sample_kagemusha_recursive_spend_bundle())
            .expect("encode recursive spend bundle");
        let bad = b"not a norito archive";

        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result(
                bad.as_ptr(),
                bad.len() as c_ulong,
                good.as_ptr(),
                good.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result(
                ptr::null(),
                0,
                good.as_ptr(),
                good.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_NULL_PTR);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_lineage_witness_append_result(
                bad.as_ptr(),
                bad.len() as c_ulong,
                good.as_ptr(),
                good.len() as c_ulong,
                good.as_ptr(),
                good.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);

        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_lineage_witness_append_result(
                good.as_ptr(),
                good.len() as c_ulong,
                ptr::null(),
                0,
                good.as_ptr(),
                good.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, ERR_NULL_PTR);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_recursive_spend_append_ffi_rejects_reserved_lineage_previous_without_output() {
        let mut previous_bundle = sample_kagemusha_recursive_spend_bundle();
        previous_bundle.recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        previous_bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_bytes(b"bridge-recursive-spend-append-lineage-scalar");
        previous_bundle.recursive_proof.public_inputs_hash = previous_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("reserved lineage previous public-input hash");
        let request = KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            record_bundle: sample_kagemusha_verified_record_bundle(),
            pallas_open_envelopes_archive: Vec::new(),
            current_note: KagemushaSpendableNoteDescriptorV1 {
                note_commitment: fixed_bytes(b"bridge-recursive-spend-append-lineage-note"),
                spend_nullifier: fixed_bytes(b"bridge-recursive-spend-append-lineage-nullifier"),
                amount: Numeric::new(42, 0),
            },
        };
        let archive = norito::to_bytes(&request)
            .expect("encode reserved-lineage recursive spend append request");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_append(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(
            out_ptr.is_null(),
            "reserved-lineage append must not return output bytes"
        );
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_recursive_spend_verify_ffi_returns_diagnostic_result_archive() {
        fn verify_result(
            request: &KagemushaRecursiveSpendVerifyRequestV1,
        ) -> KagemushaRecursiveSpendVerifyResultV1 {
            let archive = norito::to_bytes(request).expect("encode recursive spend verify request");
            let mut out_ptr: *mut c_uchar = ptr::null_mut();
            let mut out_len: c_ulong = 0;

            let status = unsafe {
                connect_norito_kagemusha_recursive_spend_verify(
                    archive.as_ptr(),
                    archive.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            };

            assert_eq!(status, 0);
            assert!(
                !out_ptr.is_null(),
                "verify should return a diagnostic result archive"
            );
            assert!(out_len > 0, "verify result archive must not be empty");
            let out = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
            connect_norito_free(out_ptr);
            norito::decode_from_bytes(&out).expect("decode recursive spend verify result")
        }

        let request = KagemushaRecursiveSpendVerifyRequestV1 {
            bundle: sample_kagemusha_recursive_spend_bundle(),
        };
        let result = verify_result(&request);
        assert!(!result.valid);
        assert!(!result.chain_admissible);
        assert_eq!(result.hop_count, request.bundle.accumulator.hop_count);
        assert!(result.encoded_bytes > 0);
        assert!(
            result.reason.contains("recursive spend proof envelope"),
            "unexpected verify rejection reason: {}",
            result.reason
        );
        assert_eq!(result.chain_admission_reason, "offline verification failed");

        let mut trusted_setup_backend = request.clone();
        trusted_setup_backend.bundle.recursive_proof.proof =
            ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
        let trusted_setup_result = verify_result(&trusted_setup_backend);
        assert!(!trusted_setup_result.valid);
        assert!(!trusted_setup_result.chain_admissible);
        assert!(
            trusted_setup_result.reason.contains("not supported"),
            "unexpected trusted-setup rejection reason: {}",
            trusted_setup_result.reason
        );

        let mut stark_recursive_bundle = request.clone();
        stark_recursive_bundle.bundle.recursive_proof.proof =
            ProofBox::new("stark/fri/transparent-v1".into(), vec![0xA5; 64]);
        stark_recursive_bundle
            .bundle
            .recursive_proof
            .verifier_key_id = VerifyingKeyId::new(
            "stark/fri/transparent-v1",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        let stark_result = verify_result(&stark_recursive_bundle);
        assert!(!stark_result.valid);
        assert!(!stark_result.chain_admissible);
        assert!(
            stark_result.reason.contains("proof.backend"),
            "unexpected STARK/FRI rejection reason: {}",
            stark_result.reason
        );

        let mut empty_recursive_proof = request;
        empty_recursive_proof.bundle.recursive_proof.proof =
            ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new());
        let empty_result = verify_result(&empty_recursive_proof);
        assert!(!empty_result.valid);
        assert!(!empty_result.chain_admissible);
        assert!(
            empty_result.reason.contains("proof.bytes"),
            "unexpected empty-proof rejection reason: {}",
            empty_result.reason
        );
    }

    #[test]
    fn kagemusha_recursive_spend_verify_reports_semantic_proof_offline_valid_chain_inadmissible() {
        let bundle = sample_verifying_semantic_recursive_spend_bundle();
        let request = KagemushaRecursiveSpendVerifyRequestV1 { bundle };
        let archive = norito::to_bytes(&request).expect("encode recursive spend verify request");

        let result = kagemusha_recursive_spend_verify_from_request_archive(&archive)
            .expect("recursive spend verify result");

        assert!(
            result.valid,
            "backend-valid semantic recursive spend proofs must be spendable offline"
        );
        assert!(
            !result.chain_admissible,
            "semantic recursive spend proofs without lineage witness are not directly redeemable"
        );
        assert_eq!(result.hop_count, request.bundle.accumulator.hop_count);
        assert!(result.encoded_bytes > 0);
        assert!(result.reason.is_empty());
        assert!(
            result.chain_admission_reason.contains("chain admission")
                && result
                    .chain_admission_reason
                    .contains("private-hop lineage"),
            "unexpected semantic recursive spend verify reason: {}",
            result.chain_admission_reason
        );
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_tampered_hop_proof() {
        let mut bundle = sample_kagemusha_verified_bundle();
        let last = bundle.steps[0]
            .attachment
            .proof
            .bytes
            .last_mut()
            .expect("proof bytes");
        *last ^= 0x01;
        let archive = norito::to_bytes(&bundle).expect("encode tampered bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_forged_envelope_hash_metadata() {
        let mut bundle = sample_kagemusha_verified_bundle();
        bundle.steps[0].attachment.envelope_hash = Some([0xA7; Hash::LENGTH]);
        let archive = norito::to_bytes(&bundle).expect("encode forged-envelope-hash bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_hop_envelope_aux_substitution() {
        let mut bundle = sample_kagemusha_verified_bundle();
        mutate_kagemusha_bundle_hop_envelope(&mut bundle, |envelope| {
            envelope.aux = b"kagemusha-forged-hop-aux".to_vec();
        });
        let archive = norito::to_bytes(&bundle).expect("encode forged-hop-aux bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_oversized_hop_proof() {
        let mut bundle = sample_kagemusha_verified_bundle();
        bundle.steps[0]
            .attachment
            .proof
            .bytes
            .resize(KAGEMUSHA_HOP_MAX_PROOF_BYTES as usize + 1, 0xA7);
        let archive = norito::to_bytes(&bundle).expect("encode oversized-hop bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_oversized_bundle_before_proof_decode() {
        let mut bundle = sample_kagemusha_verified_bundle();
        bundle.steps[0].attachment.proof.bytes = vec![0xA7];
        bundle.steps = vec![
            bundle.steps[0].clone();
            iroha_data_model::offline::KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ];
        let archive = norito::to_bytes(&bundle).expect("encode oversized-hop-count bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_bad_hop_shape_before_proof_decode() {
        let mut bundle = sample_kagemusha_verified_bundle();
        bundle.steps[0].input_nullifiers.clear();
        bundle.steps[0].attachment.proof.bytes = vec![0xA7];
        let archive = norito::to_bytes(&bundle).expect("encode bad-shape bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_root_discontinuity_before_proof_decode() {
        let mut bundle = sample_kagemusha_verified_bundle();
        bundle.steps[0].attachment.proof.bytes = vec![0xA7];
        bundle.steps.push(bundle.steps[0].clone());
        let archive = norito::to_bytes(&bundle).expect("encode root-discontinuous bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_verified_compact_token_ffi_rejects_vk_commitment_mismatch() {
        let mut bundle = sample_kagemusha_verified_bundle();
        bundle.steps[0].attachment.vk_commitment = Some([0xA5; Hash::LENGTH]);
        let archive = norito::to_bytes(&bundle).expect("encode mismatched bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_prove_verified_compact_payment_token(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_bridge_rejects_semantic_profile_after_public_binding() {
        let request = sample_recursive_spend_redeem_request(42);
        request
            .validate_public_binding()
            .expect("semantic recursive redeem request has valid public bindings");
        let archive = norito::to_bytes(&request).expect("encode recursive spend redeem request");
        assert!(
            kagemusha_recursive_spend_redeem_from_request_archive(&archive).is_err(),
            "bridge must not serialize semantic recursive spend redeems that chain admission rejects"
        );

        let mut wrong_amount = sample_recursive_spend_redeem_request(41);
        let wrong_amount_archive =
            norito::to_bytes(&wrong_amount).expect("encode wrong-amount request");
        assert!(
            kagemusha_recursive_spend_redeem_from_request_archive(&wrong_amount_archive).is_err(),
            "bridge must reject a public amount that is not the current spendable note amount"
        );

        wrong_amount.public_amount = 42;
        wrong_amount
            .bundle
            .accumulator
            .topup_anchor_nullifiers
            .clear();
        let missing_anchor_archive =
            norito::to_bytes(&wrong_amount).expect("encode missing-anchor request");
        assert!(
            kagemusha_recursive_spend_redeem_from_request_archive(&missing_anchor_archive).is_err(),
            "bridge must reject recursive redeem requests without top-up anchors"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_bridge_rejects_unwired_reserved_lineage_profile() {
        let mut request = sample_recursive_spend_redeem_request(42);
        request.bundle.recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        request
            .bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_bytes(b"bridge-recursive-lineage-scalar-projection");
        request.bundle.recursive_proof.public_inputs_hash = request
            .bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("lineage recursive spend public-input hash");

        request
            .validate_public_binding()
            .expect("reserved lineage recursive redeem request has valid public bindings");
        let archive = norito::to_bytes(&request).expect("encode lineage recursive redeem request");
        assert!(
            kagemusha_recursive_spend_redeem_from_request_archive(&archive).is_err(),
            "bridge must not serialize reserved lineage redeems until the lineage verifier is wired"
        );

        let mut missing_scalar = request;
        missing_scalar
            .bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest = [0u8; Hash::LENGTH];
        missing_scalar.bundle.recursive_proof.public_inputs_hash = missing_scalar
            .bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("zero lineage scalar public-input hash");
        let missing_scalar_archive =
            norito::to_bytes(&missing_scalar).expect("encode missing-scalar request");
        assert!(
            kagemusha_recursive_spend_redeem_from_request_archive(&missing_scalar_archive).is_err(),
            "bridge must reject reserved lineage recursive redeem requests without scalar projection"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_bridge_verifies_record_backed_lineage_final_proof() {
        let (bundle, witness) = sample_verifying_semantic_recursive_spend_lineage_fixture();
        let mut request = sample_recursive_spend_redeem_request(7);
        request.bundle = bundle;
        request.lineage_witness = Some(witness.clone());
        request
            .validate_public_binding()
            .expect("record-backed recursive redeem request has valid public bindings");

        let archive = norito::to_bytes(&request).expect("encode record-backed redeem request");
        let instruction = kagemusha_recursive_spend_redeem_from_request_archive(&archive)
            .expect("bridge accepts record-backed lineage witness with valid final proof");
        assert_eq!(instruction.public_amount, 7);
        assert_eq!(instruction.lineage_witness.as_ref(), Some(&witness));

        let mut tampered = request;
        tampered.bundle.recursive_proof.proof.bytes[0] ^= 0x01;
        let archive =
            norito::to_bytes(&tampered).expect("encode tampered record-backed redeem request");
        assert!(
            kagemusha_recursive_spend_redeem_from_request_archive(&archive).is_err(),
            "bridge must reject record-backed lineage redeems with a tampered final recursive proof"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_bridge_rejects_adversarial_lineage_witnesses() {
        let (bundle, witness) = sample_verifying_semantic_recursive_spend_lineage_fixture();
        let base_request = {
            let mut request = sample_recursive_spend_redeem_request(7);
            request.bundle = bundle;
            request.lineage_witness = Some(witness.clone());
            request
        };

        fn assert_rejects(request: KagemushaRecursiveSpendRedeemRequestV1, label: &str) {
            let archive = norito::to_bytes(&request)
                .unwrap_or_else(|err| panic!("encode {label} request: {err}"));
            assert!(
                kagemusha_recursive_spend_redeem_from_request_archive(&archive).is_err(),
                "bridge must reject adversarial lineage witness: {label}"
            );
        }

        let mut missing_record = base_request.clone();
        missing_record
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .verifier_records
            .clear();
        assert_rejects(missing_record, "missing verifier record");

        let mut duplicate_record = base_request.clone();
        let duplicate = duplicate_record
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .verifier_records[0]
            .clone();
        duplicate_record
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .verifier_records
            .push(duplicate);
        assert_rejects(duplicate_record, "duplicate verifier record");

        let mut unreferenced_record = base_request.clone();
        let mut extra = unreferenced_record
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .verifier_records[0]
            .clone();
        extra.id = VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "unused-bridge-lineage-hop");
        unreferenced_record
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .verifier_records
            .push(extra);
        assert_rejects(unreferenced_record, "unreferenced verifier record");

        let mut inactive_record = base_request.clone();
        inactive_record
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .verifier_records[0]
            .record
            .status = ConfidentialStatus::Withdrawn;
        assert_rejects(inactive_record, "inactive verifier record");

        let mut inline_key_mismatch = base_request.clone();
        inline_key_mismatch
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .verifier_records[0]
            .record
            .key = None;
        assert_rejects(inline_key_mismatch, "missing inline verifier key");

        let mut malformed_pallas_archive = base_request.clone();
        malformed_pallas_archive
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .pallas_open_envelopes_archive = vec![0xFF, 0x00, 0x01];
        assert_rejects(
            malformed_pallas_archive,
            "malformed Pallas envelope archive",
        );

        let mut note_commitment_mismatch = base_request.clone();
        note_commitment_mismatch
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[0]
            .note_commitment = fixed_bytes(b"bridge-lineage-wrong-current-note");
        assert_rejects(note_commitment_mismatch, "current note commitment mismatch");

        let mut unexpected_previous_proof = base_request.clone();
        let previous = unexpected_previous_proof.bundle.recursive_proof.clone();
        unexpected_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs
            .push(previous);
        assert_rejects(
            unexpected_previous_proof,
            "unexpected previous recursive proof for one-hop witness",
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_ffi_rejects_amount_mismatch_without_output() {
        let request = sample_recursive_spend_redeem_request(41);
        let archive = norito::to_bytes(&request).expect("encode wrong-amount request");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_redeem(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_KAGEMUSHA_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_ffi_rejects_malformed_redeem_proof_without_output() {
        fn assert_rejects_without_output(request: KagemushaRecursiveSpendRedeemRequestV1) {
            let archive = norito::to_bytes(&request).expect("encode malformed redeem proof");
            let mut out_ptr: *mut c_uchar = ptr::null_mut();
            let mut out_len: c_ulong = 0;

            let status = unsafe {
                connect_norito_kagemusha_recursive_spend_redeem(
                    archive.as_ptr(),
                    archive.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            };

            assert_eq!(status, ERR_KAGEMUSHA_PROVE);
            assert!(out_ptr.is_null());
            assert_eq!(out_len, 0);
        }

        let mut trusted_setup_backend = sample_recursive_spend_redeem_request(42);
        trusted_setup_backend.redeem_proof = ProofAttachment::new_ref(
            "groth16".into(),
            ProofBox::new("groth16".into(), vec![0x5A; 64]),
            VerifyingKeyId::new("groth16", "bridge-recursive-unshield"),
        );
        assert_rejects_without_output(trusted_setup_backend);

        let mut empty_proof = sample_recursive_spend_redeem_request(42);
        empty_proof.redeem_proof.proof = ProofBox::new("halo2/ipa".into(), Vec::new());
        assert_rejects_without_output(empty_proof);

        let mut missing_vk_commitment = sample_recursive_spend_redeem_request(42);
        missing_vk_commitment.redeem_proof.vk_commitment = None;
        assert_rejects_without_output(missing_vk_commitment);

        let mut zero_vk_commitment = sample_recursive_spend_redeem_request(42);
        zero_vk_commitment.redeem_proof.vk_commitment = Some([0u8; Hash::LENGTH]);
        assert_rejects_without_output(zero_vk_commitment);

        let mut envelope_hash_mismatch = sample_recursive_spend_redeem_request(42);
        envelope_hash_mismatch.redeem_proof.envelope_hash =
            Some(fixed_bytes(b"bridge-recursive-bad-envelope-hash"));
        assert_rejects_without_output(envelope_hash_mismatch);

        let mut stark_recursive_bundle = sample_recursive_spend_redeem_request(42);
        stark_recursive_bundle.bundle.recursive_proof.proof =
            ProofBox::new("stark/fri/production".into(), vec![0xA5; 64]);
        stark_recursive_bundle
            .bundle
            .recursive_proof
            .verifier_key_id = VerifyingKeyId::new(
            "stark/fri/production",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert_rejects_without_output(stark_recursive_bundle);

        let mut empty_recursive_proof = sample_recursive_spend_redeem_request(42);
        empty_recursive_proof.bundle.recursive_proof.proof =
            ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new());
        assert_rejects_without_output(empty_recursive_proof);
    }

    #[test]
    fn offline_note_redeem_ffi_returns_verifying_recursive_proof() {
        let redemption = sample_redemption();
        let archive = norito::to_bytes(&redemption).expect("encode redemption");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_offline_prove_note_redeem(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, 0);
        let recursive = decode_recursive_proof(out_ptr, out_len);
        assert_recursive_proof_verifies(
            &recursive,
            redemption.public_inputs_hash().expect("public input hash"),
        );
    }

    #[test]
    fn offline_note_audit_ffi_returns_verifying_recursive_proof() {
        let audit = sample_audit();
        let archive = norito::to_bytes(&audit).expect("encode audit bundle");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_offline_prove_note_audit(
                archive.as_ptr(),
                archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, 0);
        let recursive = decode_recursive_proof(out_ptr, out_len);
        assert_recursive_proof_verifies(
            &recursive,
            audit.public_inputs_hash().expect("public input hash"),
        );
    }

    #[test]
    fn offline_note_prover_ffi_rejects_invalid_archive() {
        let bad_archive = b"not a norito archive";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let status = unsafe {
            connect_norito_offline_prove_note_redeem(
                bad_archive.as_ptr(),
                bad_archive.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert_eq!(status, ERR_OFFLINE_NOTE_PROVE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    fn decode_signed_from_ffi_output(
        out_hash: [u8; Hash::LENGTH],
        out_ptr: *mut c_uchar,
        out_len: c_ulong,
    ) -> SignedTransaction {
        assert!(!out_ptr.is_null());
        let bytes = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) };
        let signed = decode_signed_transaction(bytes).expect("decode signed transaction");
        assert_eq!(out_hash, *signed.hash().as_ref());
        connect_norito_free(out_ptr);
        signed
    }

    fn assert_single_instruction<T: 'static>(signed: &SignedTransaction) {
        match signed.instructions() {
            Executable::Instructions(instructions) => {
                assert_eq!(instructions.len(), 1);
                assert!(instructions[0].as_any().downcast_ref::<T>().is_some());
            }
            other => panic!("unexpected executable: {other:?}"),
        }
    }

    #[test]
    fn offline_note_signed_transaction_ffis_encode_canonical_transactions() {
        let chain = CString::new("00000042").expect("valid chain id");
        let (authority, private_key) = sample_authority_and_private_key(0x55);
        let issue_archive = norito::to_bytes(&sample_issue()).expect("encode issue");
        let redeem_archive = norito::to_bytes(&sample_redemption()).expect("encode redemption");
        let audit_archive = norito::to_bytes(&sample_audit()).expect("encode audit");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let mut out_hash = [0u8; Hash::LENGTH];

        let issue_status = unsafe {
            connect_norito_encode_issue_offline_note_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_000_000_000,
                3_500,
                1,
                17,
                1,
                issue_archive.as_ptr(),
                issue_archive.len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(issue_status, 0);
        let signed = decode_signed_from_ffi_output(out_hash, out_ptr, out_len);
        assert_single_instruction::<iroha_data_model::isi::offline::IssueOfflineNote>(&signed);

        out_ptr = ptr::null_mut();
        out_len = 0;
        out_hash = [0u8; Hash::LENGTH];
        let redeem_status = unsafe {
            connect_norito_encode_redeem_offline_note_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_000_000_000,
                0,
                0,
                0,
                0,
                redeem_archive.as_ptr(),
                redeem_archive.len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(redeem_status, 0);
        let signed = decode_signed_from_ffi_output(out_hash, out_ptr, out_len);
        assert_single_instruction::<iroha_data_model::isi::offline::RedeemOfflineNote>(&signed);

        out_ptr = ptr::null_mut();
        out_len = 0;
        out_hash = [0u8; Hash::LENGTH];
        let audit_status = unsafe {
            connect_norito_encode_audit_offline_note_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_000_000_000,
                0,
                0,
                0,
                0,
                audit_archive.as_ptr(),
                audit_archive.len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(audit_status, 0);
        let signed = decode_signed_from_ffi_output(out_hash, out_ptr, out_len);
        assert_single_instruction::<iroha_data_model::isi::offline::AuditOfflineNote>(&signed);

        let mut trail = Vec::new();
        let audit_len = u64::try_from(audit_archive.len()).expect("audit archive length fits u64");
        trail.extend_from_slice(&audit_len.to_le_bytes());
        trail.extend_from_slice(&audit_archive);
        out_ptr = ptr::null_mut();
        out_len = 0;
        out_hash = [0u8; Hash::LENGTH];
        let defund_status = unsafe {
            connect_norito_encode_defund_offline_note_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_000_000_000,
                0,
                0,
                0,
                0,
                trail.as_ptr(),
                trail.len() as c_ulong,
                1,
                redeem_archive.as_ptr(),
                redeem_archive.len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(defund_status, 0);
        let signed = decode_signed_from_ffi_output(out_hash, out_ptr, out_len);
        match signed.instructions() {
            Executable::Instructions(instructions) => {
                assert_eq!(instructions.len(), 2);
                assert!(
                    instructions[0]
                        .as_any()
                        .downcast_ref::<iroha_data_model::isi::offline::AuditOfflineNote>()
                        .is_some()
                );
                assert!(
                    instructions[1]
                        .as_any()
                        .downcast_ref::<iroha_data_model::isi::offline::RedeemOfflineNote>()
                        .is_some()
                );
            }
            other => panic!("unexpected executable: {other:?}"),
        }
    }
}

// ---------------- EnvelopeV1 encode helpers (selected variants) ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_sign_result_ok(
    seq: u64,
    sig_ptr: *const c_uchar,
    sig_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sig_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let sig_bytes = std::slice::from_raw_parts(sig_ptr, sig_len as usize);
        let signature = match proto::WalletSignatureV1::from_ed25519_bytes(sig_bytes) {
            Some(sig) => sig,
            None => return -2,
        };
        let env = proto::EnvelopeV1 {
            seq,
            payload: proto::ConnectPayloadV1::SignResultOk { signature },
        };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_sign_result_ok_with_alg(
    seq: u64,
    alg_ptr: *const c_char,
    alg_len: c_ulong,
    sig_ptr: *const c_uchar,
    sig_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if sig_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let algorithm = match parse_algorithm_cstr(alg_ptr, alg_len) {
            Ok(a) => a,
            Err(code) => return code,
        };
        let sig_bytes = std::slice::from_raw_parts(sig_ptr, sig_len as usize);
        let signature = Signature::from_bytes(sig_bytes);
        let env = proto::EnvelopeV1 {
            seq,
            payload: proto::ConnectPayloadV1::SignResultOk {
                signature: proto::WalletSignatureV1::new(algorithm, signature),
            },
        };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_control_close(
    seq: u64,
    who: c_uchar, // 0=App,1=Wallet
    code: u16,
    reason_ptr: *const c_uchar,
    reason_len: c_ulong,
    retryable: c_uchar, // 0/1
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if reason_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let reason =
            String::from_utf8(std::slice::from_raw_parts(reason_ptr, reason_len as usize).to_vec())
                .map_err(|_| ())
                .unwrap_or_default();
        let who = match who {
            0 => proto::Role::App,
            1 => proto::Role::Wallet,
            _ => return -2,
        };
        let payload = proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Close {
            who,
            code,
            reason,
            retryable: retryable != 0,
        });
        let env = proto::EnvelopeV1 { seq, payload };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -4;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_control_reject(
    seq: u64,
    code: u16,
    code_id_ptr: *const c_uchar,
    code_id_len: c_ulong,
    reason_ptr: *const c_uchar,
    reason_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if code_id_ptr.is_null() || reason_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let code_id = String::from_utf8(
            std::slice::from_raw_parts(code_id_ptr, code_id_len as usize).to_vec(),
        )
        .map_err(|_| ())
        .unwrap_or_default();
        let reason =
            String::from_utf8(std::slice::from_raw_parts(reason_ptr, reason_len as usize).to_vec())
                .map_err(|_| ())
                .unwrap_or_default();
        let payload = proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Reject {
            code,
            code_id,
            reason,
        });
        let env = proto::EnvelopeV1 { seq, payload };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

// ---------------- EnvelopeV1 decode helpers (selected variants) ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_envelope_kind(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_seq: *mut u64,
    out_kind: *mut u16,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_seq.is_null() || out_kind.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let env = match decode_envelope(inp) {
            Ok(e) => e,
            Err(_) => return -2,
        };
        *out_seq = env.seq;
        let kind = match env.payload {
            proto::ConnectPayloadV1::SignRequestTx { .. } => 1,
            proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Close { .. }) => 2,
            proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Reject { .. }) => 3,
            proto::ConnectPayloadV1::SignResultOk { .. } => 4,
            proto::ConnectPayloadV1::SignRequestRaw { .. } => 5,
            proto::ConnectPayloadV1::SignResultErr { .. } => 6,
            proto::ConnectPayloadV1::DisplayRequest { .. } => 7,
        };
        *out_kind = kind;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_envelope_json(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let env = match decode_envelope(inp) {
            Ok(e) => e,
            Err(_) => return -2,
        };
        let payload_json = match env.payload {
            proto::ConnectPayloadV1::SignRequestTx { tx_bytes } => {
                let s = b64_encode(&tx_bytes);
                json_object([(
                    "SignRequestTx",
                    json_object([("tx_bytes_b64", ::norito::json!(s))]),
                )])
            }
            proto::ConnectPayloadV1::SignRequestRaw { domain_tag, bytes } => {
                let s = b64_encode(&bytes);
                json_object([(
                    "SignRequestRaw",
                    json_object([
                        ("domain_tag", ::norito::json!(domain_tag)),
                        ("bytes_b64", ::norito::json!(s)),
                    ]),
                )])
            }
            proto::ConnectPayloadV1::SignResultOk { signature } => {
                let alg = signature.algorithm.as_static_str();
                let s = b64_encode(signature.bytes());
                json_object([(
                    "SignResultOk",
                    json_object([
                        ("algorithm", ::norito::json!(alg)),
                        ("signature_b64", ::norito::json!(s)),
                    ]),
                )])
            }
            proto::ConnectPayloadV1::SignResultErr { code, message } => json_object([(
                "SignResultErr",
                json_object([
                    ("code", ::norito::json!(code)),
                    ("message", ::norito::json!(message.clone())),
                ]),
            )]),
            proto::ConnectPayloadV1::DisplayRequest { title, body } => json_object([(
                "DisplayRequest",
                json_object([
                    ("title", ::norito::json!(title.clone())),
                    ("body", ::norito::json!(body.clone())),
                ]),
            )]),
            proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Close {
                who,
                code,
                reason,
                retryable,
            }) => {
                let who_label = match who {
                    proto::Role::App => "App",
                    proto::Role::Wallet => "Wallet",
                };
                json_object([(
                    "Control",
                    json_object([(
                        "Close",
                        json_object([
                            ("who", ::norito::json!(who_label)),
                            ("code", ::norito::json!(code)),
                            ("reason", ::norito::json!(reason.clone())),
                            ("retryable", ::norito::json!(retryable)),
                        ]),
                    )]),
                )])
            }
            proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Reject {
                code,
                code_id,
                reason,
            }) => json_object([(
                "Control",
                json_object([(
                    "Reject",
                    json_object([
                        ("code", ::norito::json!(code)),
                        ("code_id", ::norito::json!(code_id)),
                        ("reason", ::norito::json!(reason.clone())),
                    ]),
                )]),
            )]),
        };
        let obj = json_object([("seq", ::norito::json!(env.seq)), ("payload", payload_json)]);
        let s = match norito::json::to_vec(&obj) {
            Ok(v) => v,
            Err(_) => return -3,
        };
        let len = s.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -4;
        }
        ptr::copy_nonoverlapping(s.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_envelope_sign_result_alg(
    inp_ptr: *const c_uchar,
    inp_len: c_ulong,
    out_alg_ptr: *mut *mut c_char,
    out_alg_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if inp_ptr.is_null() || out_alg_ptr.is_null() || out_alg_len.is_null() {
            return -1;
        }
        let inp = std::slice::from_raw_parts(inp_ptr, inp_len as usize);
        let env = match decode_envelope(inp) {
            Ok(e) => e,
            Err(_) => return -2,
        };
        let alg_str = match env.payload {
            proto::ConnectPayloadV1::SignResultOk { signature } => {
                signature.algorithm.as_static_str()
            }
            _ => return -3,
        };
        let bytes = alg_str.as_bytes();
        let len = bytes.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -4;
        }
        ptr::copy_nonoverlapping(bytes.as_ptr(), mem as *mut u8, len);
        *out_alg_ptr = mem as *mut c_char;
        *out_alg_len = len as c_ulong;
        0
    }
}

// Additional envelope encoders for parity

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_sign_request_tx(
    seq: u64,
    tx_ptr: *const c_uchar,
    tx_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if tx_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let tx = std::slice::from_raw_parts(tx_ptr, tx_len as usize).to_vec();
        let env = proto::EnvelopeV1 {
            seq,
            payload: proto::ConnectPayloadV1::SignRequestTx { tx_bytes: tx },
        };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_sign_request_raw(
    seq: u64,
    tag_ptr: *const c_uchar,
    tag_len: c_ulong,
    bytes_ptr: *const c_uchar,
    bytes_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if tag_ptr.is_null() || bytes_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let domain_tag =
            String::from_utf8(std::slice::from_raw_parts(tag_ptr, tag_len as usize).to_vec())
                .map_err(|_| ())
                .unwrap_or_default();
        let bytes = std::slice::from_raw_parts(bytes_ptr, bytes_len as usize).to_vec();
        let env = proto::EnvelopeV1 {
            seq,
            payload: proto::ConnectPayloadV1::SignRequestRaw { domain_tag, bytes },
        };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_envelope_sign_result_err(
    seq: u64,
    code_ptr: *const c_uchar,
    code_len: c_ulong,
    msg_ptr: *const c_uchar,
    msg_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if code_ptr.is_null() || msg_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
            return -1;
        }
        let code =
            String::from_utf8(std::slice::from_raw_parts(code_ptr, code_len as usize).to_vec())
                .map_err(|_| ())
                .unwrap_or_default();
        let message =
            String::from_utf8(std::slice::from_raw_parts(msg_ptr, msg_len as usize).to_vec())
                .map_err(|_| ())
                .unwrap_or_default();
        let env = proto::EnvelopeV1 {
            seq,
            payload: proto::ConnectPayloadV1::SignResultErr { code, message },
        };
        let buf = match encode_envelope_framed(&env) {
            Ok(buf) => buf,
            Err(_) => return ERR_CONNECT_ENCODE,
        };
        let len = buf.len();
        let mem = malloc(len);
        if mem.is_null() {
            return -3;
        }
        ptr::copy_nonoverlapping(buf.as_ptr(), mem as *mut u8, len);
        *out_ptr = mem as *mut u8;
        *out_len = len as c_ulong;
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_transfer_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let inputs = unsafe {
            gather_asset_tx_inputs(AssetInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                quantity_ptr,
                quantity_len,
                destination_ptr,
                destination_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id =
            AssetId::with_scope(asset_definition.clone(), authority.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            || {
                let transfer = Transfer::asset_numeric(asset_id, quantity, destination);
                Executable::from([InstructionBox::from(transfer)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_transfer_signed_transaction_with_fee_sponsor(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    fee_sponsor_ptr: *const c_char,
    fee_sponsor_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let inputs = unsafe {
            gather_asset_tx_inputs(AssetInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                quantity_ptr,
                quantity_len,
                destination_ptr,
                destination_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        let fee_sponsor =
            unsafe { parse_optional_account_id_bridge(fee_sponsor_ptr, fee_sponsor_len)? };
        let metadata = build_fee_sponsor_metadata(fee_sponsor);

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id =
            AssetId::with_scope(asset_definition.clone(), authority.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce_and_metadata(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            metadata,
            private_key,
            || {
                let transfer = Transfer::asset_numeric(asset_id, quantity, destination);
                Executable::from([InstructionBox::from(transfer)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_transfer_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_asset_tx_inputs_with_parser(
                AssetInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    quantity_ptr,
                    quantity_len,
                    destination_ptr,
                    destination_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id =
            AssetId::with_scope(asset_definition.clone(), authority.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            || {
                let transfer = Transfer::asset_numeric(asset_id, quantity, destination);
                Executable::from([InstructionBox::from(transfer)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_transfer_signed_transaction_with_fee_sponsor_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    fee_sponsor_ptr: *const c_char,
    fee_sponsor_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_asset_tx_inputs_with_parser(
                AssetInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    quantity_ptr,
                    quantity_len,
                    destination_ptr,
                    destination_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };
        let fee_sponsor =
            unsafe { parse_optional_account_id_bridge(fee_sponsor_ptr, fee_sponsor_len)? };
        let metadata = build_fee_sponsor_metadata(fee_sponsor);

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id =
            AssetId::with_scope(asset_definition.clone(), authority.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce_and_metadata(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            metadata,
            private_key,
            || {
                let transfer = Transfer::asset_numeric(asset_id, quantity, destination);
                Executable::from([InstructionBox::from(transfer)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_shield_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    from_ptr: *const c_char,
    from_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    note_commitment_ptr: *const c_uchar,
    note_commitment_len: c_ulong,
    payload_ephemeral_ptr: *const c_uchar,
    payload_ephemeral_len: c_ulong,
    payload_nonce_ptr: *const c_uchar,
    payload_nonce_len: c_ulong,
    payload_ciphertext_ptr: *const c_uchar,
    payload_ciphertext_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let inputs = unsafe {
            gather_shield_tx_inputs(ShieldInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                from_ptr,
                from_len,
                amount_ptr,
                amount_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        if payload_ciphertext_len > u32::MAX as c_ulong {
            return Err(BridgeError::ConfidentialPayload);
        }

        let note_commitment = unsafe {
            read_fixed_array::<32>(
                note_commitment_ptr,
                note_commitment_len,
                BridgeError::InvalidNoteCommitment,
            )?
        };
        let ephemeral = unsafe {
            read_fixed_array::<32>(
                payload_ephemeral_ptr,
                payload_ephemeral_len,
                BridgeError::ConfidentialPayload,
            )?
        };
        let nonce = unsafe {
            read_fixed_array::<24>(
                payload_nonce_ptr,
                payload_nonce_len,
                BridgeError::ConfidentialPayload,
            )?
        };
        let ciphertext = unsafe { read_vec_bytes(payload_ciphertext_ptr, payload_ciphertext_len)? };

        let payload = build_confidential_encrypted_payload(ephemeral, nonce, ciphertext)?;
        let asset = inputs.asset_definition.clone();
        let from_account = inputs.from_account.clone();
        let ttl = inputs.ttl;
        let amount = inputs.amount;
        let chain_id = inputs.chain_id;
        let authority = inputs.authority;
        let private_key = inputs.private_key;

        let (signed_bytes, hash_bytes) = encode_asset_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            || {
                let instruction = zk::Shield::new(
                    asset.clone(),
                    from_account.clone(),
                    amount,
                    note_commitment,
                    payload.clone(),
                );
                Executable::from([InstructionBox::from(instruction)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_shield_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    from_ptr: *const c_char,
    from_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    note_commitment_ptr: *const c_uchar,
    note_commitment_len: c_ulong,
    payload_ephemeral_ptr: *const c_uchar,
    payload_ephemeral_len: c_ulong,
    payload_nonce_ptr: *const c_uchar,
    payload_nonce_len: c_ulong,
    payload_ciphertext_ptr: *const c_uchar,
    payload_ciphertext_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_shield_tx_inputs_with_parser(
                ShieldInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    from_ptr,
                    from_len,
                    amount_ptr,
                    amount_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };

        if payload_ciphertext_len > u32::MAX as c_ulong {
            return Err(BridgeError::ConfidentialPayload);
        }

        let note_commitment = unsafe {
            read_fixed_array::<32>(
                note_commitment_ptr,
                note_commitment_len,
                BridgeError::InvalidNoteCommitment,
            )?
        };
        let ephemeral = unsafe {
            read_fixed_array::<32>(
                payload_ephemeral_ptr,
                payload_ephemeral_len,
                BridgeError::ConfidentialPayload,
            )?
        };
        let nonce = unsafe {
            read_fixed_array::<24>(
                payload_nonce_ptr,
                payload_nonce_len,
                BridgeError::ConfidentialPayload,
            )?
        };
        let ciphertext = unsafe { read_vec_bytes(payload_ciphertext_ptr, payload_ciphertext_len)? };
        let payload = build_confidential_encrypted_payload(ephemeral, nonce, ciphertext)?;

        let ShieldTxInputs {
            chain_id,
            authority,
            asset_definition,
            from_account,
            amount,
            ttl,
            private_key,
        } = inputs;

        let (signed_bytes, hash_bytes) = encode_asset_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            || {
                let instruction = zk::Shield::new(
                    asset_definition,
                    from_account,
                    amount,
                    note_commitment,
                    payload,
                );
                Executable::from([InstructionBox::from(instruction)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_unshield_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    inputs_ptr: *const c_uchar,
    inputs_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    root_hint_ptr: *const c_uchar,
    root_hint_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let inputs = unsafe {
            gather_unshield_tx_inputs(UnshieldInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                destination_ptr,
                destination_len,
                amount_ptr,
                amount_len,
                inputs_ptr,
                inputs_len,
                proof_json_ptr,
                proof_json_len,
                root_hint_ptr,
                root_hint_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        let asset = inputs.asset_definition.clone();
        let destination = inputs.destination.clone();
        let amount = inputs.amount;
        let nullifiers = inputs.inputs.clone();
        let proof = inputs.proof.clone();
        let root_hint = inputs.root_hint;
        let chain_id = inputs.chain_id;
        let authority = inputs.authority;
        let ttl = inputs.ttl;
        let private_key = inputs.private_key;

        let (signed_bytes, hash_bytes) = encode_asset_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            || {
                let instruction = zk::Unshield::new(
                    asset.clone(),
                    destination.clone(),
                    amount,
                    nullifiers.clone(),
                    proof.clone(),
                    root_hint,
                );
                Executable::from([InstructionBox::from(instruction)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_unshield_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    public_amount_ptr: *const c_char,
    public_amount_len: c_ulong,
    inputs_ptr: *const c_uchar,
    inputs_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    root_hint_ptr: *const c_uchar,
    root_hint_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_unshield_tx_inputs_with_parser(
                UnshieldInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    destination_ptr,
                    destination_len,
                    amount_ptr: public_amount_ptr,
                    amount_len: public_amount_len,
                    inputs_ptr,
                    inputs_len,
                    proof_json_ptr,
                    proof_json_len,
                    root_hint_ptr,
                    root_hint_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };

        let UnshieldTxInputs {
            chain_id,
            authority,
            asset_definition,
            destination,
            amount,
            inputs,
            proof,
            root_hint,
            ttl,
            private_key,
        } = inputs;

        let (signed_bytes, hash_bytes) = encode_asset_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            || {
                let instruction = zk::Unshield::new(
                    asset_definition,
                    destination,
                    amount,
                    inputs,
                    proof,
                    root_hint,
                );
                Executable::from([InstructionBox::from(instruction)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_zk_transfer_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    inputs_ptr: *const c_uchar,
    inputs_len: c_ulong,
    outputs_ptr: *const c_uchar,
    outputs_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    root_hint_ptr: *const c_uchar,
    root_hint_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let inputs = unsafe {
            gather_zk_transfer_tx_inputs(ZkTransferInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                inputs_ptr,
                inputs_len,
                outputs_ptr,
                outputs_len,
                proof_json_ptr,
                proof_json_len,
                root_hint_ptr,
                root_hint_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        let ZkTransferTxInputs {
            chain_id,
            authority,
            asset_definition,
            inputs: nullifiers,
            outputs,
            proof,
            root_hint,
            ttl,
            private_key,
        } = inputs;

        let (signed_bytes, hash_bytes) = encode_asset_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            || {
                let instruction =
                    zk::ZkTransfer::new(asset_definition, nullifiers, outputs, proof, root_hint);
                Executable::from([InstructionBox::from(instruction)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_zk_transfer_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    inputs_ptr: *const c_uchar,
    inputs_len: c_ulong,
    outputs_ptr: *const c_uchar,
    outputs_len: c_ulong,
    proof_json_ptr: *const c_char,
    proof_json_len: c_ulong,
    root_hint_ptr: *const c_uchar,
    root_hint_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_zk_transfer_tx_inputs_with_parser(
                ZkTransferInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    inputs_ptr,
                    inputs_len,
                    outputs_ptr,
                    outputs_len,
                    proof_json_ptr,
                    proof_json_len,
                    root_hint_ptr,
                    root_hint_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };

        let ZkTransferTxInputs {
            chain_id,
            authority,
            asset_definition,
            inputs: nullifiers,
            outputs,
            proof,
            root_hint,
            ttl,
            private_key,
        } = inputs;

        let (signed_bytes, hash_bytes) = encode_asset_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            || {
                let instruction =
                    zk::ZkTransfer::new(asset_definition, nullifiers, outputs, proof, root_hint);
                Executable::from([InstructionBox::from(instruction)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_register_zk_asset_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    mode_code: u8,
    allow_shield: c_uchar,
    allow_unshield: c_uchar,
    vk_transfer_ptr: *const c_char,
    vk_transfer_len: c_ulong,
    vk_transfer_present: c_uchar,
    vk_unshield_ptr: *const c_char,
    vk_unshield_len: c_ulong,
    vk_unshield_present: c_uchar,
    vk_shield_ptr: *const c_char,
    vk_shield_len: c_ulong,
    vk_shield_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let asset_definition_str =
            unsafe { read_string_bridge(asset_definition_ptr, asset_definition_len) }?;
        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let asset_definition = parse_asset_definition(asset_definition_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let mode = parse_zk_asset_mode(mode_code)?;
        let vk_transfer = unsafe {
            parse_optional_verifying_key_id(vk_transfer_ptr, vk_transfer_len, vk_transfer_present)
        }?;
        let vk_unshield = unsafe {
            parse_optional_verifying_key_id(vk_unshield_ptr, vk_unshield_len, vk_unshield_present)
        }?;
        let vk_shield = unsafe {
            parse_optional_verifying_key_id(vk_shield_ptr, vk_shield_len, vk_shield_present)
        }?;
        let allow_shield = allow_shield != 0;
        let allow_unshield = allow_unshield != 0;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let register = zk::RegisterZkAsset::new(
            asset_definition,
            mode,
            allow_shield,
            allow_unshield,
            vk_transfer,
            vk_unshield,
            vk_shield,
        );

        let (signed_bytes, hash_bytes) =
            encode_asset_transaction(chain_id, authority, creation_time_ms, ttl, private_key, {
                let register = register.clone();
                move || Executable::from([InstructionBox::from(register.clone())])
            });

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_register_zk_asset_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    mode_code: u8,
    allow_shield: c_uchar,
    allow_unshield: c_uchar,
    vk_transfer_ptr: *const c_char,
    vk_transfer_len: c_ulong,
    vk_transfer_present: c_uchar,
    vk_unshield_ptr: *const c_char,
    vk_unshield_len: c_ulong,
    vk_unshield_present: c_uchar,
    vk_shield_ptr: *const c_char,
    vk_shield_len: c_ulong,
    vk_shield_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let asset_definition_str =
            unsafe { read_string_bridge(asset_definition_ptr, asset_definition_len) }?;
        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let asset_definition = parse_asset_definition(asset_definition_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let mode = parse_zk_asset_mode(mode_code)?;
        let vk_transfer = unsafe {
            parse_optional_verifying_key_id(vk_transfer_ptr, vk_transfer_len, vk_transfer_present)
        }?;
        let vk_unshield = unsafe {
            parse_optional_verifying_key_id(vk_unshield_ptr, vk_unshield_len, vk_unshield_present)
        }?;
        let vk_shield = unsafe {
            parse_optional_verifying_key_id(vk_shield_ptr, vk_shield_len, vk_shield_present)
        }?;
        let allow_shield = allow_shield != 0;
        let allow_unshield = allow_unshield != 0;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let register = zk::RegisterZkAsset::new(
            asset_definition,
            mode,
            allow_shield,
            allow_unshield,
            vk_transfer,
            vk_unshield,
            vk_shield,
        );

        let (signed_bytes, hash_bytes) =
            encode_asset_transaction(chain_id, authority, creation_time_ms, ttl, private_key, {
                let register = register.clone();
                move || Executable::from([InstructionBox::from(register.clone())])
            });

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_set_key_value_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    target_kind: u8,
    object_ptr: *const c_char,
    object_len: c_ulong,
    key_ptr: *const c_char,
    key_len: c_ulong,
    value_ptr: *const c_uchar,
    value_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || value_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let object_str = unsafe { read_string_bridge(object_ptr, object_len) }?;
        let key_str = unsafe { read_string_bridge(key_ptr, key_len) }?;
        let value_slice = unsafe { slice::from_raw_parts(value_ptr, value_len as usize) };

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let target = parse_metadata_target(target_kind, object_str)?;
        let key = parse_name(key_str)?;
        let value = parse_json_value(value_slice)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let instruction = build_set_metadata_instruction(target, key, value);
        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            instruction,
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_set_key_value_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    target_kind: u8,
    object_ptr: *const c_char,
    object_len: c_ulong,
    key_ptr: *const c_char,
    key_len: c_ulong,
    value_ptr: *const c_uchar,
    value_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || value_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let object_str = unsafe { read_string_bridge(object_ptr, object_len) }?;
        let key_str = unsafe { read_string_bridge(key_ptr, key_len) }?;
        let value_slice = unsafe { slice::from_raw_parts(value_ptr, value_len as usize) };

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let target = parse_metadata_target(target_kind, object_str)?;
        let key = parse_name(key_str)?;
        let value = parse_json_value(value_slice)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let instruction = build_set_metadata_instruction(target, key, value);
        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            instruction,
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_remove_key_value_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    target_kind: u8,
    object_ptr: *const c_char,
    object_len: c_ulong,
    key_ptr: *const c_char,
    key_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let object_str = unsafe { read_string_bridge(object_ptr, object_len) }?;
        let key_str = unsafe { read_string_bridge(key_ptr, key_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let target = parse_metadata_target(target_kind, object_str)?;
        let key = parse_name(key_str)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let instruction = build_remove_metadata_instruction(target, key);
        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            instruction,
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_remove_key_value_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    target_kind: u8,
    object_ptr: *const c_char,
    object_len: c_ulong,
    key_ptr: *const c_char,
    key_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let object_str = unsafe { read_string_bridge(object_ptr, object_len) }?;
        let key_str = unsafe { read_string_bridge(key_ptr, key_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let target = parse_metadata_target(target_kind, object_str)?;
        let key = parse_name(key_str)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let instruction = build_remove_metadata_instruction(target, key);
        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            instruction,
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_propose_deploy_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    contract_address_ptr: *const c_char,
    contract_address_len: c_ulong,
    code_hash_ptr: *const c_char,
    code_hash_len: c_ulong,
    abi_hash_ptr: *const c_char,
    abi_hash_len: c_ulong,
    abi_version_ptr: *const c_char,
    abi_version_len: c_ulong,
    window_lower: u64,
    window_upper: u64,
    window_present: c_uchar,
    mode_code: u8,
    mode_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let contract_address_raw =
            unsafe { read_string_bridge(contract_address_ptr, contract_address_len) }?;
        let code_hash_raw = unsafe { read_string_bridge(code_hash_ptr, code_hash_len) }?;
        let abi_hash_raw = unsafe { read_string_bridge(abi_hash_ptr, abi_hash_len) }?;
        let abi_version = unsafe { read_string_bridge(abi_version_ptr, abi_version_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let contract_address = contract_address_raw
            .parse()
            .map_err(|_| BridgeError::Governance)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let code_hash_arr = parse_hex_32(&code_hash_raw)?;
        let abi_hash_arr = parse_hex_32(&abi_hash_raw)?;
        let code_hash_hex = hex::encode(code_hash_arr);
        let abi_hash_hex = hex::encode(abi_hash_arr);
        let window = if window_present != 0 {
            Some(AtWindow {
                lower: window_lower,
                upper: window_upper,
            })
        } else {
            None
        };
        let mode = if mode_present != 0 {
            Some(parse_voting_mode(mode_code)?)
        } else {
            None
        };

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let key_pair = KeyPair::from(private_key.clone());
        let manifest = ContractManifest {
            code_hash: Some(Hash::prehashed(code_hash_arr)),
            abi_hash: Some(Hash::prehashed(abi_hash_arr)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&key_pair);
        let manifest_provenance = manifest.provenance.clone().ok_or(BridgeError::Governance)?;

        let proposal = ProposeDeployContract {
            contract_address,
            code_hash_hex,
            abi_hash_hex,
            abi_version,
            window,
            mode,
            manifest_provenance: Some(manifest_provenance),
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(proposal),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_propose_deploy_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    contract_address_ptr: *const c_char,
    contract_address_len: c_ulong,
    code_hash_ptr: *const c_char,
    code_hash_len: c_ulong,
    abi_hash_ptr: *const c_char,
    abi_hash_len: c_ulong,
    abi_version_ptr: *const c_char,
    abi_version_len: c_ulong,
    window_lower: u64,
    window_upper: u64,
    window_present: c_uchar,
    mode_code: u8,
    mode_present: c_uchar,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let contract_address_raw =
            unsafe { read_string_bridge(contract_address_ptr, contract_address_len) }?;
        let code_hash_raw = unsafe { read_string_bridge(code_hash_ptr, code_hash_len) }?;
        let abi_hash_raw = unsafe { read_string_bridge(abi_hash_ptr, abi_hash_len) }?;
        let abi_version = unsafe { read_string_bridge(abi_version_ptr, abi_version_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let contract_address = contract_address_raw
            .parse()
            .map_err(|_| BridgeError::Governance)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let code_hash_arr = parse_hex_32(&code_hash_raw)?;
        let abi_hash_arr = parse_hex_32(&abi_hash_raw)?;
        let code_hash_hex = hex::encode(code_hash_arr);
        let abi_hash_hex = hex::encode(abi_hash_arr);
        let window = if window_present != 0 {
            Some(AtWindow {
                lower: window_lower,
                upper: window_upper,
            })
        } else {
            None
        };
        let mode = if mode_present != 0 {
            Some(parse_voting_mode(mode_code)?)
        } else {
            None
        };

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;
        let key_pair = KeyPair::from(private_key.clone());
        let manifest = ContractManifest {
            code_hash: Some(Hash::prehashed(code_hash_arr)),
            abi_hash: Some(Hash::prehashed(abi_hash_arr)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&key_pair);
        let manifest_provenance = manifest.provenance.clone().ok_or(BridgeError::Governance)?;

        let proposal = ProposeDeployContract {
            contract_address,
            code_hash_hex,
            abi_hash_hex,
            abi_version,
            window,
            mode,
            manifest_provenance: Some(manifest_provenance),
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(proposal),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_cast_plain_ballot_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    referendum_id_ptr: *const c_char,
    referendum_id_len: c_ulong,
    owner_ptr: *const c_char,
    owner_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    duration_blocks: u64,
    direction: u8,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if direction > 2 {
            return Err(BridgeError::Governance);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let referendum_id = unsafe { read_string_bridge(referendum_id_ptr, referendum_id_len) }?;
        let owner_str = unsafe { read_string_bridge(owner_ptr, owner_len) }?;
        let amount_str = unsafe { read_string_bridge(amount_ptr, amount_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let owner = parse_account_id(owner_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let amount = u128::from_str(&amount_str).map_err(|_| BridgeError::Governance)?;

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let ballot = CastPlainBallot {
            referendum_id,
            owner,
            amount,
            duration_blocks,
            direction,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(ballot),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_cast_plain_ballot_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    referendum_id_ptr: *const c_char,
    referendum_id_len: c_ulong,
    owner_ptr: *const c_char,
    owner_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    duration_blocks: u64,
    direction: u8,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if direction > 2 {
            return Err(BridgeError::Governance);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let referendum_id = unsafe { read_string_bridge(referendum_id_ptr, referendum_id_len) }?;
        let owner_str = unsafe { read_string_bridge(owner_ptr, owner_len) }?;
        let amount_str = unsafe { read_string_bridge(amount_ptr, amount_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let owner = parse_account_id(owner_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let amount = u128::from_str(&amount_str).map_err(|_| BridgeError::Governance)?;

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let ballot = CastPlainBallot {
            referendum_id,
            owner,
            amount,
            duration_blocks,
            direction,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(ballot),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_cast_zk_ballot_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    election_id_ptr: *const c_char,
    election_id_len: c_ulong,
    proof_b64_ptr: *const c_char,
    proof_b64_len: c_ulong,
    public_inputs_ptr: *const c_uchar,
    public_inputs_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || public_inputs_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let election_id = unsafe { read_string_bridge(election_id_ptr, election_id_len) }?;
        let proof_raw = unsafe { read_string_bridge(proof_b64_ptr, proof_b64_len) }?;
        let inputs_slice =
            unsafe { slice::from_raw_parts(public_inputs_ptr, public_inputs_len as usize) };

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;

        let proof_bytes = b64gp::STANDARD
            .decode(proof_raw)
            .map_err(|_| BridgeError::Governance)?;
        let proof_b64 = b64gp::STANDARD.encode(proof_bytes);
        let mut public_inputs_value: norito::json::Value =
            norito::json::from_slice(inputs_slice).map_err(|_| BridgeError::Governance)?;
        normalize_zk_ballot_public_inputs(&mut public_inputs_value)?;
        let public_inputs_json =
            norito::json::to_string(&public_inputs_value).map_err(|_| BridgeError::Governance)?;

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let ballot = CastZkBallot {
            election_id,
            proof_b64,
            public_inputs_json,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(ballot),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_cast_zk_ballot_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    election_id_ptr: *const c_char,
    election_id_len: c_ulong,
    proof_b64_ptr: *const c_char,
    proof_b64_len: c_ulong,
    public_inputs_ptr: *const c_uchar,
    public_inputs_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || public_inputs_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let election_id = unsafe { read_string_bridge(election_id_ptr, election_id_len) }?;
        let proof_raw = unsafe { read_string_bridge(proof_b64_ptr, proof_b64_len) }?;
        let inputs_slice =
            unsafe { slice::from_raw_parts(public_inputs_ptr, public_inputs_len as usize) };

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;

        let proof_bytes = b64gp::STANDARD
            .decode(proof_raw)
            .map_err(|_| BridgeError::Governance)?;
        let proof_b64 = b64gp::STANDARD.encode(proof_bytes);
        let mut public_inputs_value: norito::json::Value =
            norito::json::from_slice(inputs_slice).map_err(|_| BridgeError::Governance)?;
        normalize_zk_ballot_public_inputs(&mut public_inputs_value)?;
        let public_inputs_json =
            norito::json::to_string(&public_inputs_value).map_err(|_| BridgeError::Governance)?;

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let ballot = CastZkBallot {
            election_id,
            proof_b64,
            public_inputs_json,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(ballot),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_enact_referendum_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    referendum_id_ptr: *const c_char,
    referendum_id_len: c_ulong,
    preimage_hash_ptr: *const c_char,
    preimage_hash_len: c_ulong,
    window_lower: u64,
    window_upper: u64,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let referendum_hex = unsafe { read_string_bridge(referendum_id_ptr, referendum_id_len) }?;
        let preimage_hex = unsafe { read_string_bridge(preimage_hash_ptr, preimage_hash_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let referendum_id = parse_hex_32(&referendum_hex)?;
        let preimage_hash = parse_hex_32(&preimage_hex)?;
        let at_window = AtWindow {
            lower: window_lower,
            upper: window_upper,
        };

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let enact = EnactReferendum {
            referendum_id,
            preimage_hash,
            at_window,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(enact),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_enact_referendum_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    referendum_id_ptr: *const c_char,
    referendum_id_len: c_ulong,
    preimage_hash_ptr: *const c_char,
    preimage_hash_len: c_ulong,
    window_lower: u64,
    window_upper: u64,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let referendum_hex = unsafe { read_string_bridge(referendum_id_ptr, referendum_id_len) }?;
        let preimage_hex = unsafe { read_string_bridge(preimage_hash_ptr, preimage_hash_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let referendum_id = parse_hex_32(&referendum_hex)?;
        let preimage_hash = parse_hex_32(&preimage_hex)?;
        let at_window = AtWindow {
            lower: window_lower,
            upper: window_upper,
        };

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let enact = EnactReferendum {
            referendum_id,
            preimage_hash,
            at_window,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(enact),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_finalize_referendum_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    referendum_id_ptr: *const c_char,
    referendum_id_len: c_ulong,
    proposal_id_ptr: *const c_char,
    proposal_id_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let referendum_id = unsafe { read_string_bridge(referendum_id_ptr, referendum_id_len) }?;
        let proposal_hex = unsafe { read_string_bridge(proposal_id_ptr, proposal_id_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let proposal_id = parse_hex_32(&proposal_hex)?;

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let finalize = FinalizeReferendum {
            referendum_id,
            proposal_id,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(finalize),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_finalize_referendum_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    referendum_id_ptr: *const c_char,
    referendum_id_len: c_ulong,
    proposal_id_ptr: *const c_char,
    proposal_id_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let referendum_id = unsafe { read_string_bridge(referendum_id_ptr, referendum_id_len) }?;
        let proposal_hex = unsafe { read_string_bridge(proposal_id_ptr, proposal_id_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let proposal_id = parse_hex_32(&proposal_hex)?;

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let finalize = FinalizeReferendum {
            referendum_id,
            proposal_id,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(finalize),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_persist_council_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    epoch: u64,
    candidates_count: u32,
    derived_by: u8,
    members_json_ptr: *const c_uchar,
    members_json_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || members_json_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let members_slice =
            unsafe { slice::from_raw_parts(members_json_ptr, members_json_len as usize) };

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let members = parse_account_list(members_slice)?;
        let derived_by = match derived_by {
            0 => CouncilDerivationKind::Vrf,
            1 => CouncilDerivationKind::Fallback,
            _ => return Err(BridgeError::Governance),
        };

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;

        let persist = PersistCouncilForEpoch {
            epoch,
            members,
            alternates: Vec::new(),
            verified: 0,
            candidates_count,
            derived_by,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(persist),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_governance_persist_council_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    epoch: u64,
    candidates_count: u32,
    derived_by: u8,
    members_json_ptr: *const c_uchar,
    members_json_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || members_json_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let members_slice =
            unsafe { slice::from_raw_parts(members_json_ptr, members_json_len as usize) };

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let members = parse_account_list(members_slice)?;
        let derived_by = match derived_by {
            0 => CouncilDerivationKind::Vrf,
            1 => CouncilDerivationKind::Fallback,
            _ => return Err(BridgeError::Governance),
        };

        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;

        let persist = PersistCouncilForEpoch {
            epoch,
            members,
            alternates: Vec::new(),
            verified: 0,
            candidates_count,
            derived_by,
        };

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(persist),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

const SECP256K1_PRIVATE_LEN: usize = 32;
const SECP256K1_PUBLIC_LEN: usize = 33;
const SECP256K1_SIGNATURE_LEN: usize = 64;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_secp256k1_public_key(
    private_ptr: *const c_uchar,
    private_len: c_ulong,
    out_public_ptr: *mut c_uchar,
    out_public_len: c_ulong,
) -> c_int {
    if private_ptr.is_null() || out_public_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if private_len != SECP256K1_PRIVATE_LEN as c_ulong {
        return ERR_SECP_PARSE;
    }
    let private_bytes = unsafe { slice::from_raw_parts(private_ptr, private_len as usize) };
    let private_key = match EcdsaSecp256k1Sha256::parse_private_key(private_bytes) {
        Ok(key) => key,
        Err(_) => return ERR_SECP_PARSE,
    };
    let encoded = private_key.public_key().to_sec1_bytes();
    let encoded_bytes = encoded.as_ref();
    if encoded_bytes.len() != SECP256K1_PUBLIC_LEN
        || out_public_len < encoded_bytes.len() as c_ulong
    {
        return ERR_BUFFER_TOO_SMALL;
    }
    unsafe {
        ptr::copy_nonoverlapping(encoded_bytes.as_ptr(), out_public_ptr, encoded_bytes.len());
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_secp256k1_sign(
    private_ptr: *const c_uchar,
    private_len: c_ulong,
    message_ptr: *const c_uchar,
    message_len: c_ulong,
    out_signature_ptr: *mut c_uchar,
    out_signature_len: c_ulong,
) -> c_int {
    if private_ptr.is_null() || message_ptr.is_null() || out_signature_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if private_len != SECP256K1_PRIVATE_LEN as c_ulong {
        return ERR_SECP_PARSE;
    }
    let private_bytes = unsafe { slice::from_raw_parts(private_ptr, private_len as usize) };
    let message = unsafe { slice::from_raw_parts(message_ptr, message_len as usize) };
    let private_key = match EcdsaSecp256k1Sha256::parse_private_key(private_bytes) {
        Ok(key) => key,
        Err(_) => return ERR_SECP_PARSE,
    };
    let signature = EcdsaSecp256k1Sha256::sign(message, &private_key);
    if signature.len() != SECP256K1_SIGNATURE_LEN {
        return ERR_SECP_SIGN;
    }
    if out_signature_len < signature.len() as c_ulong {
        return ERR_BUFFER_TOO_SMALL;
    }
    unsafe {
        ptr::copy_nonoverlapping(
            signature.as_ptr(),
            out_signature_ptr,
            SECP256K1_SIGNATURE_LEN,
        );
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_secp256k1_verify(
    public_ptr: *const c_uchar,
    public_len: c_ulong,
    message_ptr: *const c_uchar,
    message_len: c_ulong,
    signature_ptr: *const c_uchar,
    signature_len: c_ulong,
) -> c_int {
    if public_ptr.is_null() || message_ptr.is_null() || signature_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if public_len != SECP256K1_PUBLIC_LEN as c_ulong
        || signature_len != SECP256K1_SIGNATURE_LEN as c_ulong
    {
        return ERR_SECP_PARSE;
    }

    let public_bytes = unsafe { slice::from_raw_parts(public_ptr, public_len as usize) };
    let message = unsafe { slice::from_raw_parts(message_ptr, message_len as usize) };
    let signature_bytes = unsafe { slice::from_raw_parts(signature_ptr, signature_len as usize) };
    let public_key = match EcdsaSecp256k1Sha256::parse_public_key(public_bytes) {
        Ok(pk) => pk,
        Err(_) => return ERR_SECP_PARSE,
    };
    match EcdsaSecp256k1Sha256::verify(message, signature_bytes, &public_key) {
        Ok(()) => 1,
        Err(CryptoError::BadSignature) => 0,
        Err(_) => ERR_SECP_VERIFY,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_default_distid(
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    let distid = Sm2PublicKey::default_distid();
    match unsafe { write_bytes(out_ptr, out_len, distid.as_bytes()) } {
        Ok(()) => 0,
        Err(code) => code,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_keypair_from_seed(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
    seed_ptr: *const c_uchar,
    seed_len: c_ulong,
    out_private_ptr: *mut c_uchar,
    out_private_len: c_ulong,
    out_public_ptr: *mut c_uchar,
    out_public_len: c_ulong,
) -> c_int {
    if seed_ptr.is_null() || out_private_ptr.is_null() || out_public_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if out_private_len < 32 || out_public_len < 65 {
        return ERR_BUFFER_TOO_SMALL;
    }
    let distid = match unsafe { read_distid_or_default(distid_ptr, distid_len) } {
        Ok(d) => d,
        Err(err) => return err.code(),
    };
    let seed = unsafe { slice::from_raw_parts(seed_ptr, seed_len as usize) };
    let key = match Sm2PrivateKey::from_seed(distid, seed) {
        Ok(k) => k,
        Err(_) => return ERR_SM2_DERIVE,
    };
    let private_bytes = key.secret_bytes();
    let public_bytes = key.public_key().to_sec1_bytes(false);
    unsafe {
        ptr::copy_nonoverlapping(private_bytes.as_ptr(), out_private_ptr, private_bytes.len());
        ptr::copy_nonoverlapping(public_bytes.as_ptr(), out_public_ptr, public_bytes.len());
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_sign(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
    private_ptr: *const c_uchar,
    private_len: c_ulong,
    message_ptr: *const c_uchar,
    message_len: c_ulong,
    out_signature_ptr: *mut c_uchar,
    out_signature_len: c_ulong,
) -> c_int {
    if private_ptr.is_null() || message_ptr.is_null() || out_signature_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if private_len != 32 || out_signature_len < Sm2Signature::LENGTH as c_ulong {
        return ERR_SM2_PARSE;
    }
    let distid = match unsafe { read_distid_or_default(distid_ptr, distid_len) } {
        Ok(d) => d,
        Err(err) => return err.code(),
    };
    let private_bytes = unsafe { slice::from_raw_parts(private_ptr, private_len as usize) };
    let message = unsafe { slice::from_raw_parts(message_ptr, message_len as usize) };
    let key = match Sm2PrivateKey::from_bytes(distid, private_bytes) {
        Ok(k) => k,
        Err(_) => return ERR_SM2_PARSE,
    };
    let signature = key.sign(message);
    let sig_bytes = signature.to_bytes();
    unsafe {
        ptr::copy_nonoverlapping(sig_bytes.as_ptr(), out_signature_ptr, Sm2Signature::LENGTH);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_verify(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
    public_ptr: *const c_uchar,
    public_len: c_ulong,
    message_ptr: *const c_uchar,
    message_len: c_ulong,
    signature_ptr: *const c_uchar,
    signature_len: c_ulong,
) -> c_int {
    if public_ptr.is_null() || message_ptr.is_null() || signature_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if public_len != 65 || signature_len != Sm2Signature::LENGTH as c_ulong {
        return ERR_SM2_PARSE;
    }
    let distid = match unsafe { read_distid_or_default(distid_ptr, distid_len) } {
        Ok(d) => d,
        Err(err) => return err.code(),
    };
    let public_bytes = unsafe { slice::from_raw_parts(public_ptr, public_len as usize) };
    let message = unsafe { slice::from_raw_parts(message_ptr, message_len as usize) };
    let signature_bytes = unsafe { slice::from_raw_parts(signature_ptr, signature_len as usize) };
    let public = match Sm2PublicKey::from_sec1_bytes(&distid, public_bytes) {
        Ok(pk) => pk,
        Err(_) => return ERR_SM2_PARSE,
    };
    let mut sig_raw = [0u8; Sm2Signature::LENGTH];
    sig_raw.copy_from_slice(signature_bytes);
    let signature = match Sm2Signature::from_bytes(&sig_raw) {
        Ok(sig) => sig,
        Err(_) => return ERR_SM2_PARSE,
    };
    match public.verify(message, &signature) {
        Ok(()) => 1,
        Err(CryptoError::BadSignature) => 0,
        Err(_) => ERR_SM2_VERIFY,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_public_key_prefixed(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
    public_ptr: *const c_uchar,
    public_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    if public_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
        return ERR_NULL_PTR;
    }
    if public_len != 65 {
        return ERR_SM2_PARSE;
    }
    let distid = match unsafe { read_distid_or_default(distid_ptr, distid_len) } {
        Ok(d) => d,
        Err(err) => return err.code(),
    };
    let public_bytes = unsafe { slice::from_raw_parts(public_ptr, public_len as usize) };
    let public = match Sm2PublicKey::from_sec1_bytes(&distid, public_bytes) {
        Ok(pk) => pk,
        Err(_) => return ERR_SM2_PARSE,
    };
    let prefixed = match public.try_to_prefixed_string() {
        Ok(value) => value,
        Err(_) => return ERR_SM2_PARSE,
    };
    match unsafe { write_bytes(out_ptr, out_len, prefixed.as_bytes()) } {
        Ok(()) => 0,
        Err(code) => code,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_public_key_multihash(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
    public_ptr: *const c_uchar,
    public_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    let status = unsafe {
        connect_norito_sm2_public_key_prefixed(
            distid_ptr, distid_len, public_ptr, public_len, out_ptr, out_len,
        )
    };
    if status != 0 {
        return status;
    }
    if out_ptr.is_null() || out_len.is_null() {
        return ERR_NULL_PTR;
    }
    unsafe {
        let ptr = *out_ptr;
        if ptr.is_null() {
            return ERR_ALLOC;
        }
        let len = *out_len as usize;
        let slice = slice::from_raw_parts_mut(ptr, len);
        let mut string = match std::str::from_utf8(slice) {
            Ok(s) => s.to_owned(),
            Err(_) => return ERR_UTF8,
        };
        if let Some(stripped) = string.strip_prefix("sm2:") {
            string = stripped.to_owned();
        }
        free(ptr as *mut _);
        match write_bytes(out_ptr, out_len, string.as_bytes()) {
            Ok(()) => 0,
            Err(code) => code,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sm2_compute_za(
    distid_ptr: *const c_char,
    distid_len: c_ulong,
    public_ptr: *const c_uchar,
    public_len: c_ulong,
    out_za_ptr: *mut c_uchar,
    out_za_len: c_ulong,
) -> c_int {
    if public_ptr.is_null() || out_za_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if public_len != 65 || out_za_len < 32 {
        return ERR_BUFFER_TOO_SMALL;
    }
    let distid = match unsafe { read_distid_or_default(distid_ptr, distid_len) } {
        Ok(d) => d,
        Err(err) => return err.code(),
    };
    let public_bytes = unsafe { slice::from_raw_parts(public_ptr, public_len as usize) };
    let public = match Sm2PublicKey::from_sec1_bytes(&distid, public_bytes) {
        Ok(pk) => pk,
        Err(_) => return ERR_SM2_PARSE,
    };
    let za = match public.compute_z(&distid) {
        Ok(za) => za,
        Err(_) => return ERR_SM2_PARSE,
    };
    unsafe {
        ptr::copy_nonoverlapping(za.as_ptr(), out_za_ptr, za.len());
    }
    0
}

#[cfg(test)]
mod test_support {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static CHAIN_DISCRIMINANT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    pub(super) fn chain_discriminant_guard() -> MutexGuard<'static, ()> {
        CHAIN_DISCRIMINANT_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod accel_tests {
    use std::{
        collections::BTreeMap,
        ffi::CString,
        num::{NonZeroU16, NonZeroU32, NonZeroU64},
        ptr, slice,
    };

    use iroha_crypto::KeyPair;
    use iroha_data_model::prelude::TransferBox;

    use super::*;

    pub(super) fn sample_account(_domain: &str, seed: u8) -> (CString, Vec<u8>) {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, private_key) = keypair.into_parts();
        let account_id = AccountId::new(public_key);
        let account = CString::new(account_id.to_string()).expect("valid cstring");
        let (_, bytes) = private_key.to_bytes();
        (account, bytes)
    }

    pub(super) fn sample_destination(_domain: &str, seed: u8) -> CString {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        let account_id = AccountId::new(public_key);
        CString::new(account_id.to_string()).expect("valid cstring")
    }

    pub(super) fn cstring(s: &str) -> CString {
        CString::new(s).expect("valid cstring")
    }

    fn chain_guard() -> std::sync::MutexGuard<'static, ()> {
        super::test_support::chain_discriminant_guard()
    }

    struct ChainDiscriminantReset {
        previous: u16,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ChainDiscriminantReset {
        fn new(discriminant: u16) -> Self {
            let guard = super::test_support::chain_discriminant_guard();
            let previous = unsafe { connect_norito_set_chain_discriminant(discriminant) };
            Self {
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for ChainDiscriminantReset {
        fn drop(&mut self) {
            unsafe {
                connect_norito_set_chain_discriminant(self.previous);
            }
        }
    }

    fn decode_signed(ptr: *mut u8, len: c_ulong) -> SignedTransaction {
        let bytes = unsafe { slice::from_raw_parts(ptr, len as usize) };
        decode_signed_transaction(bytes).expect("decode signed transaction")
    }

    fn asset_definition_literal(domain: &str, name: &str) -> String {
        AssetDefinitionId::new(
            DomainId::try_new(domain, "universal").expect("domain"),
            Name::from_str(name).expect("name"),
        )
        .to_string()
    }

    fn asset_definition_cstring(domain: &str, name: &str) -> CString {
        cstring(&asset_definition_literal(domain, name))
    }

    fn call_shield_encoder(
        ephemeral: &[u8; 32],
        ciphertext: &[u8],
        algorithm: Option<Algorithm>,
    ) -> (c_int, *mut u8, c_ulong, [u8; 32]) {
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let amount = cstring("7");
        let note_commitment = [0x33_u8; 32];
        let nonce = [0x44_u8; 24];
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0_u8; 32];
        let result = unsafe {
            if let Some(algorithm) = algorithm {
                connect_norito_encode_shield_signed_transaction_alg(
                    chain.as_ptr(),
                    chain.as_bytes().len() as c_ulong,
                    authority.as_ptr(),
                    authority.as_bytes().len() as c_ulong,
                    1,
                    0,
                    0,
                    asset_definition.as_ptr(),
                    asset_definition.as_bytes().len() as c_ulong,
                    authority.as_ptr(),
                    authority.as_bytes().len() as c_ulong,
                    amount.as_ptr(),
                    amount.as_bytes().len() as c_ulong,
                    note_commitment.as_ptr(),
                    note_commitment.len() as c_ulong,
                    ephemeral.as_ptr(),
                    ephemeral.len() as c_ulong,
                    nonce.as_ptr(),
                    nonce.len() as c_ulong,
                    ciphertext.as_ptr(),
                    ciphertext.len() as c_ulong,
                    private.as_ptr(),
                    private.len() as c_ulong,
                    algorithm as u8,
                    &mut out_signed_ptr,
                    &mut out_signed_len,
                    out_hash.as_mut_ptr(),
                    out_hash.len() as c_ulong,
                )
            } else {
                connect_norito_encode_shield_signed_transaction(
                    chain.as_ptr(),
                    chain.as_bytes().len() as c_ulong,
                    authority.as_ptr(),
                    authority.as_bytes().len() as c_ulong,
                    1,
                    0,
                    0,
                    asset_definition.as_ptr(),
                    asset_definition.as_bytes().len() as c_ulong,
                    authority.as_ptr(),
                    authority.as_bytes().len() as c_ulong,
                    amount.as_ptr(),
                    amount.as_bytes().len() as c_ulong,
                    note_commitment.as_ptr(),
                    note_commitment.len() as c_ulong,
                    ephemeral.as_ptr(),
                    ephemeral.len() as c_ulong,
                    nonce.as_ptr(),
                    nonce.len() as c_ulong,
                    ciphertext.as_ptr(),
                    ciphertext.len() as c_ulong,
                    private.as_ptr(),
                    private.len() as c_ulong,
                    &mut out_signed_ptr,
                    &mut out_signed_len,
                    out_hash.as_mut_ptr(),
                    out_hash.len() as c_ulong,
                )
            }
        };
        (result, out_signed_ptr, out_signed_len, out_hash)
    }

    fn call_confidential_payload_encoder(
        ephemeral: &[u8; 32],
        ciphertext: &[u8],
    ) -> (c_int, *mut u8, c_ulong) {
        let nonce = [0x44_u8; 24];
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let result = unsafe {
            connect_norito_encode_confidential_encrypted_payload(
                ephemeral.as_ptr(),
                ephemeral.len() as c_ulong,
                nonce.as_ptr(),
                nonce.len() as c_ulong,
                ciphertext.as_ptr(),
                ciphertext.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        (result, out_ptr, out_len)
    }

    #[test]
    fn decode_asset_id_json_returns_canonical_fields() {
        let _guard = chain_guard();
        let (account_cstr, _) = sample_account("bank", 0);
        let account_literal = account_cstr.to_str().expect("account literal");
        let account_id = AccountId::parse_encoded(account_literal)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("parse account");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("bank", "universal").expect("domain"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition.clone(), account_id.clone());
        let asset_literal = cstring(&asset.canonical_literal());

        let mut out_json_ptr: *mut u8 = ptr::null_mut();
        let mut out_json_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_decode_asset_id_json(
                asset_literal.as_ptr(),
                asset_literal.as_bytes().len() as c_ulong,
                &mut out_json_ptr,
                &mut out_json_len,
            )
        };
        assert_eq!(status, 0, "expected successful decode");
        assert!(
            !out_json_ptr.is_null(),
            "decoder should return JSON payload"
        );

        let body = unsafe { slice::from_raw_parts(out_json_ptr, out_json_len as usize) };
        let parsed: JsonValue = norito::json::from_slice(body).expect("decode bridge payload");
        connect_norito_free(out_json_ptr);

        let object = parsed.as_object().expect("json object");
        assert_eq!(
            object.get("asset_id").and_then(JsonValue::as_str),
            Some(asset.canonical_literal().as_str())
        );
        assert_eq!(
            object
                .get("asset_definition_id")
                .and_then(JsonValue::as_str),
            Some(definition.to_string().as_str())
        );
        assert_eq!(
            object.get("account_id").and_then(JsonValue::as_str),
            Some(account_id.to_string().as_str())
        );
    }

    #[test]
    fn chain_discriminant_roundtrip() {
        let _guard = super::test_support::chain_discriminant_guard();
        let previous = unsafe { connect_norito_get_chain_discriminant() };
        let returned = unsafe { connect_norito_set_chain_discriminant(42) };
        assert_eq!(returned, previous);
        let current = unsafe { connect_norito_get_chain_discriminant() };
        assert_eq!(current, 42);
        unsafe {
            connect_norito_set_chain_discriminant(previous);
        }
    }

    #[test]
    fn keypair_from_seed_roundtrip() {
        let _guard = chain_guard();
        let seed = vec![0xA5; 32];
        let expected = KeyPair::from_seed(seed.clone(), Algorithm::Ed25519);
        let (expected_public, expected_private) = expected.into_parts();
        let (_alg, expected_private_bytes) = expected_private.to_bytes();
        let (_alg, expected_public_bytes) = expected_public
            .try_to_bytes()
            .expect("checked public bytes");
        let mut out_private_ptr: *mut u8 = ptr::null_mut();
        let mut out_private_len: c_ulong = 0;
        let mut out_public_ptr: *mut u8 = ptr::null_mut();
        let mut out_public_len: c_ulong = 0;
        let result = unsafe {
            connect_norito_keypair_from_seed(
                Algorithm::Ed25519 as u8,
                seed.as_ptr(),
                seed.len() as c_ulong,
                &mut out_private_ptr,
                &mut out_private_len,
                &mut out_public_ptr,
                &mut out_public_len,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert!(!out_private_ptr.is_null());
        assert!(!out_public_ptr.is_null());
        let private_bytes =
            unsafe { slice::from_raw_parts(out_private_ptr, out_private_len as usize) };
        let public_bytes =
            unsafe { slice::from_raw_parts(out_public_ptr, out_public_len as usize) };
        assert_eq!(private_bytes, expected_private_bytes.as_slice());
        assert_eq!(public_bytes, expected_public_bytes);
        unsafe {
            free(out_private_ptr as *mut _);
            free(out_public_ptr as *mut _);
        }
    }

    #[test]
    fn keypair_from_seed_fixture_vector() {
        let _guard = chain_guard();
        let seed = hex::decode("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032")
            .expect("valid seed hex");
        let expected_public =
            hex::decode("1f857fe980524a2ee4fe65e5d346f7aaadcb636a640f1d191d1c6e158607ba1e")
                .expect("valid public key hex");
        let mut out_private_ptr: *mut u8 = ptr::null_mut();
        let mut out_private_len: c_ulong = 0;
        let mut out_public_ptr: *mut u8 = ptr::null_mut();
        let mut out_public_len: c_ulong = 0;
        let result = unsafe {
            connect_norito_keypair_from_seed(
                Algorithm::Ed25519 as u8,
                seed.as_ptr(),
                seed.len() as c_ulong,
                &mut out_private_ptr,
                &mut out_private_len,
                &mut out_public_ptr,
                &mut out_public_len,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert!(!out_public_ptr.is_null());
        let public_bytes =
            unsafe { slice::from_raw_parts(out_public_ptr, out_public_len as usize) };
        assert_eq!(public_bytes, expected_public.as_slice());
        unsafe {
            free(out_private_ptr as *mut _);
            free(out_public_ptr as *mut _);
        }
    }

    #[test]
    fn keypair_from_seed_mldsa_roundtrip() {
        let _guard = chain_guard();
        let seed = b"bridge-mldsa-seed-vector".to_vec();
        let expected = KeyPair::from_seed(seed.clone(), Algorithm::MlDsa);
        let (expected_public, expected_private) = expected.into_parts();
        let (_alg, expected_private_bytes) = expected_private.to_bytes();
        let (_alg, expected_public_bytes) = expected_public
            .try_to_bytes()
            .expect("checked public bytes");
        let mut out_private_ptr: *mut u8 = ptr::null_mut();
        let mut out_private_len: c_ulong = 0;
        let mut out_public_ptr: *mut u8 = ptr::null_mut();
        let mut out_public_len: c_ulong = 0;
        let result = unsafe {
            connect_norito_keypair_from_seed(
                Algorithm::MlDsa as u8,
                seed.as_ptr(),
                seed.len() as c_ulong,
                &mut out_private_ptr,
                &mut out_private_len,
                &mut out_public_ptr,
                &mut out_public_len,
            )
        };
        assert_eq!(result, 0, "expected success");
        let private_bytes =
            unsafe { slice::from_raw_parts(out_private_ptr, out_private_len as usize) };
        let public_bytes =
            unsafe { slice::from_raw_parts(out_public_ptr, out_public_len as usize) };
        assert_eq!(private_bytes, expected_private_bytes.as_slice());
        assert_eq!(public_bytes, expected_public_bytes);
        unsafe {
            free(out_private_ptr as *mut _);
            free(out_public_ptr as *mut _);
        }
    }

    #[test]
    fn connect_open_app_metadata_roundtrip() {
        let _guard = chain_guard();
        let sid = [0x11u8; 32];
        let app_pk = [0x22u8; 32];
        let chain = CString::new("chain").expect("valid chain id");
        let app_meta = json_object([
            ("name", JsonValue::from("demo")),
            ("url", JsonValue::from("https://example.test")),
            ("icon_hash", JsonValue::from("deadbeef")),
        ]);
        let app_meta_bytes = norito::json::to_vec(&app_meta).expect("encode app metadata");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_encode_control_open_ext(
                sid.as_ptr(),
                0,
                7,
                app_pk.as_ptr(),
                app_pk.len() as c_ulong,
                app_meta_bytes.as_ptr(),
                app_meta_bytes.len() as c_ulong,
                chain.as_ptr(),
                ptr::null::<c_uchar>(),
                0,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, 0, "expected open frame encode success");
        assert!(!out_ptr.is_null());

        let mut meta_ptr: *mut c_uchar = ptr::null_mut();
        let mut meta_len: c_ulong = 0;
        let meta_status = unsafe {
            connect_norito_decode_control_open_app_metadata_json(
                out_ptr,
                out_len,
                &mut meta_ptr,
                &mut meta_len,
            )
        };
        assert_eq!(meta_status, 0, "expected app metadata decode success");
        assert!(!meta_ptr.is_null());

        let meta_bytes = unsafe { slice::from_raw_parts(meta_ptr, meta_len as usize) };
        let parsed: JsonValue =
            norito::json::from_slice(meta_bytes).expect("parse app metadata json");
        let obj = parsed.as_object().expect("app metadata object");
        assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("demo"));
        assert_eq!(
            obj.get("url").and_then(|v| v.as_str()),
            Some("https://example.test")
        );
        assert_eq!(
            obj.get("icon_hash").and_then(|v| v.as_str()),
            Some("deadbeef")
        );

        unsafe {
            if !meta_ptr.is_null() {
                free(meta_ptr as *mut _);
            }
            if !out_ptr.is_null() {
                free(out_ptr as *mut _);
            }
        }
    }

    fn fixture_private_key() -> Vec<u8> {
        let seed = hex::decode("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032")
            .expect("fixture seed hex");
        let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
        let (_alg, private_bytes) = keypair.private_key().to_bytes();
        private_bytes
    }

    fn fixture_authority(_domain: &str) -> CString {
        let seed = hex::decode("616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032")
            .expect("fixture seed hex");
        let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        let account = AccountId::new(public_key);
        CString::new(account.to_string()).expect("valid cstring")
    }

    fn assert_signed_hash_matches(out_hash: [u8; 32], signed_ptr: *mut u8, signed_len: c_ulong) {
        let signed_bytes = unsafe { slice::from_raw_parts(signed_ptr, signed_len as usize) };
        let signed = decode_signed_transaction(signed_bytes).expect("decode signed transaction");
        assert_eq!(out_hash, *signed.hash().as_ref());
    }

    #[test]
    fn parse_asset_definition_rejects_noncanonical_textual_literal() {
        let err = parse_asset_definition("usd#bank".to_owned())
            .expect_err("noncanonical textual asset definition should fail");
        assert!(matches!(err, BridgeError::AssetDefinition));
    }

    #[test]
    fn parse_asset_definition_accepts_canonical_base58_literal() {
        let canonical = asset_definition_literal("wonderland", "rose");
        let parsed = parse_asset_definition(canonical.clone())
            .expect("canonical base58 asset definition should parse");
        let expected = AssetDefinitionId::parse_address_literal(&canonical)
            .expect("canonical base58 should parse");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_asset_definition_accepts_dataspace_balance_scope_suffix() {
        let canonical = asset_definition_literal("wonderland", "rose");
        let (parsed, scope) =
            parse_asset_definition_with_balance_scope(format!("{canonical}#dataspace:10"))
                .expect("canonical base58 asset definition with dataspace scope should parse");
        let expected = AssetDefinitionId::parse_address_literal(&canonical)
            .expect("canonical base58 should parse");
        assert_eq!(parsed, expected);
        assert_eq!(scope, AssetBalanceScope::Dataspace(DataSpaceId::new(10)));
    }

    #[test]
    fn encode_transfer_preserves_dataspace_balance_scope_suffix() {
        let _reset = ChainDiscriminantReset::new(42);
        let chain = cstring("00000042");
        let authority = fixture_authority("wonderland");
        let asset_definition = cstring(&format!(
            "{}#dataspace:10",
            asset_definition_literal("wonderland", "rose")
        ));
        let quantity = cstring("15.7500");
        let destination = authority.clone();
        let private_key = fixture_private_key();
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_000_000_000,
                3_500,
                1,
                17,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        match signed.instructions() {
            Executable::Instructions(instructions) => {
                let transfer = instructions
                    .first()
                    .and_then(|instruction| instruction.as_any().downcast_ref::<TransferBox>())
                    .expect("transfer instruction");
                let TransferBox::Asset(transfer) = transfer else {
                    panic!("expected asset transfer");
                };
                assert_eq!(
                    transfer.source.scope(),
                    &AssetBalanceScope::Dataspace(DataSpaceId::new(10))
                );
            }
            other => panic!("unexpected executable: {other:?}"),
        }
        let mut out_json_ptr: *mut u8 = ptr::null_mut();
        let mut out_json_len: c_ulong = 0;
        let decode_status = unsafe {
            connect_norito_decode_signed_transaction_json(
                out_signed_ptr,
                out_signed_len,
                &mut out_json_ptr,
                &mut out_json_len,
            )
        };
        assert_eq!(decode_status, 0, "expected debug JSON decode");
        let json_body = unsafe { slice::from_raw_parts(out_json_ptr, out_json_len as usize) };
        let parsed: JsonValue = norito::json::from_slice(json_body).expect("decode transfer JSON");
        let bridge_debug = parsed
            .as_object()
            .and_then(|object| object.get("bridge_debug"))
            .and_then(JsonValue::as_object)
            .expect("bridge debug object");
        let transfer_scopes = bridge_debug
            .get("transfer_asset_scopes")
            .and_then(JsonValue::as_array)
            .expect("transfer scope array");
        let scope = transfer_scopes
            .first()
            .and_then(JsonValue::as_object)
            .expect("first transfer scope object");
        assert_eq!(
            scope.get("source_scope").and_then(JsonValue::as_str),
            Some("dataspace")
        );
        assert_eq!(
            scope.get("source_dataspace_id").and_then(JsonValue::as_u64),
            Some(10)
        );
        connect_norito_free(out_json_ptr);
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn swift_parity_transfer_hash_matches_fixture() {
        let _reset = ChainDiscriminantReset::new(42);
        let chain = cstring("00000042");
        let authority = fixture_authority("wonderland");
        let asset_definition = asset_definition_cstring("wonderland", "rose");
        let quantity = cstring("15.7500");
        let destination = authority.clone();
        let private_key = fixture_private_key();
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_000_000_000,
                3_500,
                1,
                17,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert_signed_hash_matches(out_hash, out_signed_ptr, out_signed_len);
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn swift_parity_mint_hash_matches_fixture() {
        let _reset = ChainDiscriminantReset::new(42);
        let chain = cstring("00000043");
        let authority = fixture_authority("wonderland");
        let asset_definition = asset_definition_cstring("wonderland", "rose");
        let quantity = cstring("42.0100");
        let destination = authority.clone();
        let private_key = fixture_private_key();
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_mint_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_001_000_000,
                2_000,
                1,
                19,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert_signed_hash_matches(out_hash, out_signed_ptr, out_signed_len);
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn swift_parity_burn_hash_matches_fixture() {
        let _reset = ChainDiscriminantReset::new(42);
        let chain = cstring("00000044");
        let authority = fixture_authority("wonderland");
        let asset_definition = asset_definition_cstring("wonderland", "rose");
        let quantity = cstring("5.2500");
        let destination = authority.clone();
        let private_key = fixture_private_key();
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_burn_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1_736_002_000_000,
                1_800,
                1,
                23,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert_signed_hash_matches(out_hash, out_signed_ptr, out_signed_len);
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn transfer_encoder_success() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("10");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert!(!out_signed_ptr.is_null());
        assert!(out_signed_len > 0);
        assert_ne!(out_hash, [0u8; 32], "hash should be populated");

        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn transfer_encoder_nonce_roundtrip() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("10");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let nonce_value: u32 = 17;
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                nonce_value,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        assert_eq!(
            signed.payload().nonce,
            NonZeroU32::new(nonce_value),
            "nonce should be encoded"
        );
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn transfer_encoder_invalid_nonce() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("10");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                0,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_INVALID_NONCE);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
    }

    #[test]
    fn transfer_encoder_nonce_roundtrip_alg() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("10");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let nonce_value: u32 = 9;
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction_alg(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                nonce_value,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                Algorithm::Ed25519 as u8,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        assert_eq!(
            signed.payload().nonce,
            NonZeroU32::new(nonce_value),
            "nonce should be encoded"
        );
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn transfer_encoder_with_fee_sponsor_sets_metadata() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("10");
        let destination = sample_destination("bank", 1);
        let fee_sponsor = sample_destination("paynet", 2);
        let fee_sponsor_literal = fee_sponsor.to_str().expect("utf8 fee sponsor");
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction_with_fee_sponsor(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                fee_sponsor.as_ptr(),
                fee_sponsor.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        let metadata_key = Name::from_str("fee_sponsor").expect("metadata key");
        let metadata_value = signed
            .payload()
            .metadata
            .get(&metadata_key)
            .expect("fee sponsor metadata should be present");
        assert_eq!(metadata_value, &Json::new(fee_sponsor_literal));
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn mint_encoder_nonce_roundtrip() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("5");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let nonce_value: u32 = 21;
        let result = unsafe {
            connect_norito_encode_mint_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                nonce_value,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        assert_eq!(signed.payload().nonce, NonZeroU32::new(nonce_value));
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn mint_encoder_nonce_roundtrip_alg() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("5");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let nonce_value: u32 = 22;
        let result = unsafe {
            connect_norito_encode_mint_signed_transaction_alg(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                nonce_value,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                Algorithm::Ed25519 as u8,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        assert_eq!(signed.payload().nonce, NonZeroU32::new(nonce_value));
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn burn_encoder_nonce_roundtrip() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("3");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let nonce_value: u32 = 23;
        let result = unsafe {
            connect_norito_encode_burn_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                nonce_value,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        assert_eq!(signed.payload().nonce, NonZeroU32::new(nonce_value));
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn burn_encoder_nonce_roundtrip_alg() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("3");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let nonce_value: u32 = 24;
        let result = unsafe {
            connect_norito_encode_burn_signed_transaction_alg(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                nonce_value,
                1,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                Algorithm::Ed25519 as u8,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        let signed = decode_signed(out_signed_ptr, out_signed_len);
        assert_eq!(signed.payload().nonce, NonZeroU32::new(nonce_value));
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn shield_encoder_accepts_valid_confidential_payload() {
        let _guard = chain_guard();
        for algorithm in [None, Some(Algorithm::Ed25519)] {
            let (result, out_signed_ptr, out_signed_len, out_hash) =
                call_shield_encoder(&[0x07; 32], &[0x09, 0x0A], algorithm);
            assert_eq!(result, 0, "expected shield encoder success");
            assert!(!out_signed_ptr.is_null());
            assert!(out_signed_len > 0);
            assert_ne!(out_hash, [0u8; 32]);
            unsafe {
                free(out_signed_ptr as *mut _);
            }
        }
    }

    #[test]
    fn shield_encoder_rejects_low_order_confidential_payload_key() {
        let _guard = chain_guard();
        for algorithm in [None, Some(Algorithm::Ed25519)] {
            let (result, out_signed_ptr, out_signed_len, out_hash) =
                call_shield_encoder(&[0x00; 32], &[0x09, 0x0A], algorithm);
            assert_eq!(result, ERR_CONFIDENTIAL_PAYLOAD);
            assert!(out_signed_ptr.is_null());
            assert_eq!(out_signed_len, 0);
            assert_eq!(out_hash, [0u8; 32]);
        }
    }

    #[test]
    fn shield_encoder_rejects_empty_confidential_ciphertext() {
        let _guard = chain_guard();
        for algorithm in [None, Some(Algorithm::Ed25519)] {
            let (result, out_signed_ptr, out_signed_len, out_hash) =
                call_shield_encoder(&[0x07; 32], &[], algorithm);
            assert_eq!(result, ERR_CONFIDENTIAL_PAYLOAD);
            assert!(out_signed_ptr.is_null());
            assert_eq!(out_signed_len, 0);
            assert_eq!(out_hash, [0u8; 32]);
        }
    }

    #[test]
    fn confidential_payload_encoder_accepts_valid_payload() {
        let (result, out_ptr, out_len) =
            call_confidential_payload_encoder(&[0x07; 32], &[0x09, 0x0A]);
        assert_eq!(result, 0);
        assert!(!out_ptr.is_null());
        let encoded = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) };
        assert_eq!(encoded[0], CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1);
        assert_eq!(&encoded[1..33], &[0x07; 32]);
        assert_eq!(&encoded[33..57], &[0x44; 24]);
        assert_eq!(encoded[57], 2);
        assert_eq!(&encoded[58..], &[0x09, 0x0A]);
        unsafe {
            free(out_ptr as *mut _);
        }
    }

    #[test]
    fn confidential_payload_encoder_rejects_low_order_ephemeral_key() {
        let (result, out_ptr, out_len) =
            call_confidential_payload_encoder(&[0x00; 32], &[0x09, 0x0A]);
        assert_eq!(result, -3);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn confidential_payload_encoder_rejects_empty_ciphertext() {
        let (result, out_ptr, out_len) = call_confidential_payload_encoder(&[0x07; 32], &[]);
        assert_eq!(result, -3);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }

    #[test]
    fn connect_derive_keys_rejects_low_order_peer_public_key() {
        let local_private_key = [0x01_u8; 32];
        let low_order_peer_public_key = [0x00_u8; 32];
        let session_id = [0x02_u8; 32];
        let mut app_key = [0xA5_u8; 32];
        let mut wallet_key = [0x5A_u8; 32];

        let result = unsafe {
            connect_norito_connect_derive_keys(
                local_private_key.as_ptr(),
                low_order_peer_public_key.as_ptr(),
                session_id.as_ptr(),
                app_key.as_mut_ptr(),
                wallet_key.as_mut_ptr(),
            )
        };

        assert_eq!(result, -2);
        assert_eq!(app_key, [0xA5_u8; 32]);
        assert_eq!(wallet_key, [0x5A_u8; 32]);
    }

    #[test]
    fn zk_transfer_encoder_success() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let inputs = [0x11_u8; 32];
        let outputs = [0x22_u8; 32];
        let proof = cstring(
            r#"{"backend":"groth16","proof_b64":"AA==","vk_ref":{"backend":"groth16","name":"vk1"}}"#,
        );
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_zk_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                inputs.as_ptr(),
                inputs.len() as c_ulong,
                outputs.as_ptr(),
                outputs.len() as c_ulong,
                proof.as_ptr(),
                proof.as_bytes().len() as c_ulong,
                ptr::null(),
                0,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, 0, "expected success");
        assert!(!out_signed_ptr.is_null());
        assert!(out_signed_len > 0);
        assert_ne!(out_hash, [0u8; 32], "hash should be populated");
        unsafe {
            free(out_signed_ptr as *mut _);
        }
    }

    #[test]
    fn zk_transfer_encoder_rejects_legacy_inline_vk_field() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let inputs = [0x11_u8; 32];
        let outputs = [0x22_u8; 32];
        let proof = cstring(
            r#"{"backend":"groth16","proof_b64":"AA==","vk_ref":{"backend":"groth16","name":"vk1"},"verifyingKeyInline":{"backend":"groth16","bytes_b64":"AQID"}}"#,
        );
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_zk_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                inputs.as_ptr(),
                inputs.len() as c_ulong,
                outputs.as_ptr(),
                outputs.len() as c_ulong,
                proof.as_ptr(),
                proof.as_bytes().len() as c_ulong,
                ptr::null(),
                0,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_PROOF_ATTACHMENT);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
        assert_eq!(out_hash, [0u8; 32]);
    }

    #[test]
    fn zk_transfer_encoder_rejects_proof_backend_mismatch() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let inputs = [0x11_u8; 32];
        let outputs = [0x22_u8; 32];
        let proof = cstring(
            r#"{"backend":"halo2/ipa","proof_backend":"stark/fri","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1"}}"#,
        );
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_zk_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                inputs.as_ptr(),
                inputs.len() as c_ulong,
                outputs.as_ptr(),
                outputs.len() as c_ulong,
                proof.as_ptr(),
                proof.as_bytes().len() as c_ulong,
                ptr::null(),
                0,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_PROOF_ATTACHMENT);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
        assert_eq!(out_hash, [0u8; 32]);
    }

    #[test]
    fn zk_transfer_encoder_rejects_vk_ref_backend_mismatch() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let inputs = [0x11_u8; 32];
        let outputs = [0x22_u8; 32];
        let proof = cstring(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"stark/fri","name":"vk1"}}"#,
        );
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_zk_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                inputs.as_ptr(),
                inputs.len() as c_ulong,
                outputs.as_ptr(),
                outputs.len() as c_ulong,
                proof.as_ptr(),
                proof.as_bytes().len() as c_ulong,
                ptr::null(),
                0,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_PROOF_ATTACHMENT);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
        assert_eq!(out_hash, [0u8; 32]);
    }

    #[test]
    fn zk_transfer_encoder_rejects_vk_reference_shadow_field() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let inputs = [0x11_u8; 32];
        let outputs = [0x22_u8; 32];
        let proof = cstring(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1"},"vk_reference":{"backend":"halo2/ipa","name":"shadow"}}"#,
        );
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_zk_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                inputs.as_ptr(),
                inputs.len() as c_ulong,
                outputs.as_ptr(),
                outputs.len() as c_ulong,
                proof.as_ptr(),
                proof.as_bytes().len() as c_ulong,
                ptr::null(),
                0,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_PROOF_ATTACHMENT);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
        assert_eq!(out_hash, [0u8; 32]);
    }

    #[test]
    fn zk_transfer_encoder_rejects_nested_vk_ref_shadow_field() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let inputs = [0x11_u8; 32];
        let outputs = [0x22_u8; 32];
        let proof = cstring(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1","vk_reference":"shadow"}}"#,
        );
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_zk_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                inputs.as_ptr(),
                inputs.len() as c_ulong,
                outputs.as_ptr(),
                outputs.len() as c_ulong,
                proof.as_ptr(),
                proof.as_bytes().len() as c_ulong,
                ptr::null(),
                0,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_PROOF_ATTACHMENT);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
        assert_eq!(out_hash, [0u8; 32]);
    }

    #[test]
    fn proof_attachment_json_rejects_legacy_inline_vk_field() {
        for field in [
            "vk_inline",
            "vkInline",
            "verifyingKeyInline",
            "verifying_key_inline",
        ] {
            let json = format!(
                r#"{{"backend":"groth16","proof_b64":"AA==","vk_ref":{{"backend":"groth16","name":"vk1"}},"{field}":{{"backend":"groth16","bytes_b64":"AQID"}}}}"#
            );
            let value: JsonValue = norito::json::from_str(&json).expect("json");
            let err = parse_proof_attachment_value(&value)
                .expect_err("legacy inline verifying-key field rejected");
            assert!(matches!(err, BridgeError::ProofAttachment));
        }
    }

    #[test]
    fn proof_attachment_json_rejects_proof_backend_mismatch() {
        let value: JsonValue = norito::json::from_str(
            r#"{"backend":"halo2/ipa","proof_backend":"stark/fri","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1"}}"#,
        )
        .expect("json");
        let err = parse_proof_attachment_value(&value)
            .expect_err("proof backend mismatch should be rejected by bridge parser");
        assert!(matches!(err, BridgeError::ProofAttachment));
    }

    #[test]
    fn proof_attachment_json_rejects_bad_fixed_hash_lengths() {
        let value: JsonValue = norito::json::from_str(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1"},"vk_commitment_hex":"abcd"}"#,
        )
        .expect("json");
        let err = parse_proof_attachment_value(&value)
            .expect_err("short vk_commitment_hex should be rejected");
        assert!(matches!(err, BridgeError::ProofAttachment));
    }

    #[test]
    fn proof_attachment_json_rejects_vk_ref_backend_mismatch() {
        let value: JsonValue = norito::json::from_str(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"stark/fri","name":"vk1"}}"#,
        )
        .expect("json");
        let err = parse_proof_attachment_value(&value)
            .expect_err("vk_ref backend mismatch should be rejected");
        assert!(matches!(err, BridgeError::ProofAttachment));
    }

    #[test]
    fn proof_attachment_json_rejects_vk_reference_shadow_field() {
        let value: JsonValue = norito::json::from_str(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1"},"vk_reference":{"backend":"halo2/ipa","name":"shadow"}}"#,
        )
        .expect("json");
        let err = parse_proof_attachment_value(&value)
            .expect_err("vk_reference shadow field should be rejected");
        assert!(matches!(err, BridgeError::ProofAttachment));
    }

    #[test]
    fn proof_attachment_json_rejects_nested_vk_ref_shadow_field() {
        let value: JsonValue = norito::json::from_str(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1","vk_reference":"shadow"}}"#,
        )
        .expect("json");
        let err = parse_proof_attachment_value(&value)
            .expect_err("nested vk_ref shadow field should be rejected");
        assert!(matches!(err, BridgeError::ProofAttachment));
    }

    #[test]
    fn proof_attachment_json_rejects_blank_vk_ref_name() {
        let value: JsonValue = norito::json::from_str(
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"   "}}"#,
        )
        .expect("json");
        let err =
            parse_proof_attachment_value(&value).expect_err("blank vk_ref name should be rejected");
        assert!(matches!(err, BridgeError::ProofAttachment));
    }

    #[test]
    fn proof_attachment_json_rejects_blank_backend_fields() {
        for json in [
            r#"{"backend":"   ","proof_b64":"AA==","vk_ref":{"backend":"halo2/ipa","name":"vk1"}}"#,
            r#"{"backend":"halo2/ipa","proof_b64":"AA==","vk_ref":{"backend":"   ","name":"vk1"}}"#,
        ] {
            let value: JsonValue = norito::json::from_str(json).expect("json");
            let err = parse_proof_attachment_value(&value)
                .expect_err("blank backend field should be rejected");
            assert!(matches!(err, BridgeError::ProofAttachment));
        }
    }

    #[test]
    fn transfer_encoder_invalid_quantity() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let asset_definition = asset_definition_cstring("bank", "usd");
        let quantity = cstring("NaN");
        let destination = sample_destination("bank", 1);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let result = unsafe {
            connect_norito_encode_transfer_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                0,
                0,
                asset_definition.as_ptr(),
                asset_definition.as_bytes().len() as c_ulong,
                quantity.as_ptr(),
                quantity.as_bytes().len() as c_ulong,
                destination.as_ptr(),
                destination.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };
        assert_eq!(result, ERR_QUANTITY_PARSE);
        assert!(out_signed_ptr.is_null());
        assert_eq!(out_signed_len, 0);
        assert_eq!(out_hash, [0u8; 32], "hash should remain unchanged");
    }

    #[test]
    fn multisig_register_encoder_success() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("default", 0);
        let scoped_account = cstring(authority.to_str().unwrap());
        let member_a_str = sample_destination("default", 2);
        let member_b_str = sample_destination("default", 3);
        let member_a = AccountId::parse_encoded(member_a_str.to_str().unwrap())
            .expect("member A account id")
            .into_account_id();
        let member_b = AccountId::parse_encoded(member_b_str.to_str().unwrap())
            .expect("member B account id")
            .into_account_id();
        let mut members = BTreeMap::new();
        members.insert(member_a, 2);
        members.insert(member_b, 1);
        let spec = MultisigSpec::new(
            members,
            NonZeroU16::new(2).unwrap(),
            NonZeroU64::new(60_000).unwrap(),
        );
        let spec_json = norito::json::to_string(
            &norito::json::value::to_value(&spec).expect("spec json value"),
        )
        .expect("spec json");
        let spec_c = cstring(&spec_json);

        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];

        let result = unsafe {
            connect_norito_encode_multisig_register_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                spec_c.as_ptr(),
                spec_c.as_bytes().len() as c_ulong,
                scoped_account.as_ptr(),
                scoped_account.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };

        assert_eq!(result, 0, "expected success");
        assert!(!out_signed_ptr.is_null());
        assert!(out_signed_len > 0);

        unsafe { free(out_signed_ptr as *mut _) };
    }

    #[test]
    fn multisig_register_encoder_invalid_spec() {
        let _guard = chain_guard();
        let chain = cstring("test-chain");
        let (authority, private) = sample_account("bank", 0);
        let mut out_signed_ptr: *mut u8 = ptr::null_mut();
        let mut out_signed_len: c_ulong = 0;
        let mut out_hash = [0u8; 32];
        let invalid_spec = cstring("{}");

        let result = unsafe {
            connect_norito_encode_multisig_register_signed_transaction(
                chain.as_ptr(),
                chain.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                1,
                0,
                0,
                invalid_spec.as_ptr(),
                invalid_spec.as_bytes().len() as c_ulong,
                authority.as_ptr(),
                authority.as_bytes().len() as c_ulong,
                private.as_ptr(),
                private.len() as c_ulong,
                &mut out_signed_ptr,
                &mut out_signed_len,
                out_hash.as_mut_ptr(),
                out_hash.len() as c_ulong,
            )
        };

        assert_ne!(result, 0, "expected failure for invalid spec");
        assert!(out_signed_ptr.is_null());
    }
}

#[cfg(test)]
mod secp256k1_tests {
    use hex::decode;

    use super::*;

    const PRIVATE_KEY: &str = "e4f21b38e005d4f895a29e84948d7cc83eac79041aeb644ee4fab8d9da42f713";
    const PUBLIC_KEY: &str = "0242c1e1f775237a26da4fd51b8d75ee2709711f6e90303e511169a324ef0789c0";
    const SIGNATURE: &str = "0aab347be3530a3fd7d91c354956561101e6f273b8a1ea3d414f82fbd5939db34b99c54c16c45bf4cde8193b58d718e7efa8c055e7add7d9c9cbe8935e849200";
    const MESSAGE: &[u8] = b"This is a dummy message for use with tests";

    #[test]
    fn secp256k1_signs_and_verifies() {
        let private = decode(PRIVATE_KEY).expect("valid private key hex");
        let expected_public = decode(PUBLIC_KEY).expect("valid public key hex");
        let expected_signature = decode(SIGNATURE).expect("valid signature hex");
        let mut public_out = [0u8; 33];
        let mut signature_out = [0u8; 64];

        let public_status = unsafe {
            connect_norito_secp256k1_public_key(
                private.as_ptr(),
                private.len() as c_ulong,
                public_out.as_mut_ptr(),
                public_out.len() as c_ulong,
            )
        };
        assert_eq!(public_status, 0, "public key derivation failed");
        assert_eq!(public_out.as_slice(), expected_public.as_slice());

        let sign_status = unsafe {
            connect_norito_secp256k1_sign(
                private.as_ptr(),
                private.len() as c_ulong,
                MESSAGE.as_ptr(),
                MESSAGE.len() as c_ulong,
                signature_out.as_mut_ptr(),
                signature_out.len() as c_ulong,
            )
        };
        assert_eq!(sign_status, 0, "signing failed");
        assert_eq!(signature_out.as_slice(), expected_signature.as_slice());

        let verify_status = unsafe {
            connect_norito_secp256k1_verify(
                public_out.as_ptr(),
                public_out.len() as c_ulong,
                MESSAGE.as_ptr(),
                MESSAGE.len() as c_ulong,
                signature_out.as_ptr(),
                signature_out.len() as c_ulong,
            )
        };
        assert_eq!(verify_status, 1, "signature did not verify");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_mint_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let inputs = unsafe {
            gather_asset_tx_inputs(AssetInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                quantity_ptr,
                quantity_len,
                destination_ptr,
                destination_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id =
            AssetId::with_scope(asset_definition.clone(), destination.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            || {
                let mint = Mint::asset_numeric(quantity, asset_id);
                Executable::from([InstructionBox::from(mint)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_mint_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_asset_tx_inputs_with_parser(
                AssetInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    quantity_ptr,
                    quantity_len,
                    destination_ptr,
                    destination_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id =
            AssetId::with_scope(asset_definition.clone(), destination.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            || {
                let mint = Mint::asset_numeric(quantity, asset_id);
                Executable::from([InstructionBox::from(mint)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_multisig_register_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    spec_ptr: *const c_char,
    spec_len: c_ulong,
    account_ptr: *const c_char,
    account_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || account_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let account_str = unsafe { read_string_bridge(account_ptr, account_len) }?;
        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let account = parse_account_id(account_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let spec = parse_multisig_spec_bytes(spec_ptr, spec_len)?;

        let (signed_bytes, hash_bytes) =
            encode_asset_transaction(chain_id, authority, creation_time_ms, ttl, private_key, {
                let spec = spec.clone();
                let account = account.clone();
                move || {
                    let register = MultisigRegister::with_account(
                        account.clone(),
                        None::<DomainId>,
                        spec.clone(),
                    );
                    Executable::from([InstructionBox::from(register)])
                }
            });

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_multisig_register_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    spec_ptr: *const c_char,
    spec_len: c_ulong,
    account_ptr: *const c_char,
    account_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || account_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let account_str = unsafe { read_string_bridge(account_ptr, account_len) }?;
        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let account = parse_account_id(account_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;
        let spec = parse_multisig_spec_bytes(spec_ptr, spec_len)?;

        let (signed_bytes, hash_bytes) =
            encode_asset_transaction(chain_id, authority, creation_time_ms, ttl, private_key, {
                let spec = spec.clone();
                let account = account.clone();
                move || {
                    let register = MultisigRegister::with_account(
                        account.clone(),
                        None::<DomainId>,
                        spec.clone(),
                    );
                    Executable::from([InstructionBox::from(register)])
                }
            });

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_burn_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let inputs = unsafe {
            gather_asset_tx_inputs(AssetInputPointers {
                chain_ptr,
                chain_len,
                authority_ptr,
                authority_len,
                asset_definition_ptr,
                asset_definition_len,
                quantity_ptr,
                quantity_len,
                destination_ptr,
                destination_len,
                ttl_ms,
                ttl_present,
                private_key_ptr,
                private_key_len,
            })?
        };

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::with_scope(asset_definition, destination.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            || {
                let burn = Burn::asset_numeric(quantity, asset_id);
                Executable::from([InstructionBox::from(burn)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_claim_identifier_signed_transaction(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    account_ptr: *const c_char,
    account_len: c_ulong,
    receipt_ptr: *const c_char,
    receipt_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || account_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let account_str = unsafe { read_string_bridge(account_ptr, account_len) }?;
        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let account = parse_account_id(account_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key(key_slice)?;
        let receipt = parse_identifier_receipt_bytes(receipt_ptr, receipt_len)?;

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(ClaimIdentifier { account, receipt }),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_claim_identifier_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    account_ptr: *const c_char,
    account_len: c_ulong,
    receipt_ptr: *const c_char,
    receipt_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        if private_key_ptr.is_null() || account_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let chain = unsafe { read_string_bridge(chain_ptr, chain_len) }?;
        let authority_str = unsafe { read_string_bridge(authority_ptr, authority_len) }?;
        let account_str = unsafe { read_string_bridge(account_ptr, account_len) }?;
        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
        let account = parse_account_id(account_str)?;
        let ttl = parse_ttl(ttl_ms, ttl_present != 0)?;
        let key_slice = unsafe { slice::from_raw_parts(private_key_ptr, private_key_len as usize) };
        let private_key = parse_private_key_with_algorithm(key_slice, algorithm)?;
        let receipt = parse_identifier_receipt_bytes(receipt_ptr, receipt_len)?;

        let (signed_bytes, hash_bytes) = encode_instruction_transaction(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            private_key,
            InstructionBox::from(ClaimIdentifier { account, receipt }),
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_encode_burn_signed_transaction_alg(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    authority_ptr: *const c_char,
    authority_len: c_ulong,
    creation_time_ms: u64,
    ttl_ms: u64,
    ttl_present: c_uchar,
    nonce: u32,
    nonce_present: c_uchar,
    asset_definition_ptr: *const c_char,
    asset_definition_len: c_ulong,
    quantity_ptr: *const c_char,
    quantity_len: c_ulong,
    destination_ptr: *const c_char,
    destination_len: c_ulong,
    private_key_ptr: *const c_uchar,
    private_key_len: c_ulong,
    algorithm_code: u8,
    out_signed_ptr: *mut *mut c_uchar,
    out_signed_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_signed_ptr.is_null() || out_signed_len.is_null() || out_hash_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }

        let algorithm = parse_algorithm_code(algorithm_code)?;
        let inputs = unsafe {
            gather_asset_tx_inputs_with_parser(
                AssetInputPointers {
                    chain_ptr,
                    chain_len,
                    authority_ptr,
                    authority_len,
                    asset_definition_ptr,
                    asset_definition_len,
                    quantity_ptr,
                    quantity_len,
                    destination_ptr,
                    destination_len,
                    ttl_ms,
                    ttl_present,
                    private_key_ptr,
                    private_key_len,
                },
                |bytes| parse_private_key_with_algorithm(bytes, algorithm),
            )?
        };

        let AssetTxInputs {
            chain_id,
            authority,
            asset_definition,
            asset_scope,
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::with_scope(asset_definition, destination.clone(), asset_scope);
        let (signed_bytes, hash_bytes) = encode_asset_transaction_with_nonce(
            chain_id,
            authority,
            creation_time_ms,
            ttl,
            nonce,
            private_key,
            || {
                let burn = Burn::asset_numeric(quantity, asset_id);
                Executable::from([InstructionBox::from(burn)])
            },
        );

        write_hash(out_hash_ptr, out_hash_len, &hash_bytes)?;
        unsafe { write_bytes_bridge(out_signed_ptr, out_signed_len, &signed_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_signed_transaction_json(
    signed_ptr: *const c_uchar,
    signed_len: c_ulong,
    out_json_ptr: *mut *mut c_uchar,
    out_json_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if signed_ptr.is_null() || out_json_ptr.is_null() || out_json_len.is_null() {
            return -1;
        }
        let bytes = slice::from_raw_parts(signed_ptr, signed_len as usize);
        let tx = match decode_signed_transaction(bytes) {
            Ok(v) => v,
            Err(_) => return -2,
        };
        let mut json_value = match norito::json::value::to_value(&tx) {
            Ok(value) => value,
            Err(_) => return -3,
        };
        if let JsonValue::Object(root) = &mut json_value {
            root.insert(
                "bridge_debug".into(),
                signed_transaction_bridge_debug_json(&tx),
            );
        }
        let json_bytes = match norito::json::to_vec(&json_value) {
            Ok(vec) => vec,
            Err(_) => return -3,
        };
        if let Err(code) = write_bytes(out_json_ptr, out_json_len, &json_bytes) {
            return code;
        }
        0
    }
}

/// Decode a canonical internal `AssetId` balance-bucket literal into readable JSON fields.
///
/// Response JSON object fields:
/// - `asset_id`: canonical internal asset balance-bucket literal
///   (`<base58-asset-definition-id>#<i105-account-id>`)
/// - `asset_definition_id`: canonical asset definition id (unprefixed Base58 address)
/// - `account_id`: canonical I105 account id (i105 literal)
///
/// # Safety
/// All pointer arguments must be valid and non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_asset_id_json(
    asset_ptr: *const c_char,
    asset_len: c_ulong,
    out_json_ptr: *mut *mut c_uchar,
    out_json_len: *mut c_ulong,
) -> c_int {
    let result = (|| -> BridgeResult<()> {
        if out_json_ptr.is_null() || out_json_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let asset_literal = unsafe { read_string_bridge(asset_ptr, asset_len) }?;
        let asset = AssetId::parse_literal(&asset_literal).map_err(|_| BridgeError::AssetId)?;
        let payload = JsonValue::Object(JsonMap::from_iter([
            (
                "asset_id".to_owned(),
                JsonValue::String(asset.canonical_literal()),
            ),
            (
                "asset_definition_id".to_owned(),
                JsonValue::String(asset.definition().to_string()),
            ),
            (
                "account_id".to_owned(),
                JsonValue::String(asset.account().to_string()),
            ),
        ]));
        let json_bytes = norito::json::to_vec(&payload).map_err(|_| BridgeError::JsonSerialize)?;
        unsafe { write_bytes_bridge(out_json_ptr, out_json_len, &json_bytes) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

/// Decode a Norito-encoded `TransactionSubmissionReceipt` into JSON.
///
/// # Safety
/// All pointer arguments must be valid and non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_decode_transaction_receipt_json(
    receipt_ptr: *const c_uchar,
    receipt_len: c_ulong,
    out_json_ptr: *mut *mut c_uchar,
    out_json_len: *mut c_ulong,
) -> c_int {
    unsafe {
        if receipt_ptr.is_null() || out_json_ptr.is_null() || out_json_len.is_null() {
            return -1;
        }
        let bytes = slice::from_raw_parts(receipt_ptr, receipt_len as usize);
        let receipt: TransactionSubmissionReceipt = match norito::decode_from_bytes(bytes) {
            Ok(v) => v,
            Err(_) => return -2,
        };
        let json_bytes = match norito::json::to_vec(&receipt) {
            Ok(vec) => vec,
            Err(_) => return -3,
        };
        if let Err(code) = write_bytes(out_json_ptr, out_json_len, &json_bytes) {
            return code;
        }
        0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct connect_norito_acceleration_config {
    pub enable_simd: u8,
    pub enable_metal: u8,
    pub enable_cuda: u8,
    pub max_gpus: u64,
    pub max_gpus_present: u8,
    pub merkle_min_leaves_gpu: u64,
    pub merkle_min_leaves_gpu_present: u8,
    pub merkle_min_leaves_metal: u64,
    pub merkle_min_leaves_metal_present: u8,
    pub merkle_min_leaves_cuda: u64,
    pub merkle_min_leaves_cuda_present: u8,
    pub prefer_cpu_sha2_max_leaves_aarch64: u64,
    pub prefer_cpu_sha2_max_leaves_aarch64_present: u8,
    pub prefer_cpu_sha2_max_leaves_x86: u64,
    pub prefer_cpu_sha2_max_leaves_x86_present: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct connect_norito_acceleration_backend_status {
    pub supported: u8,
    pub configured: u8,
    pub available: u8,
    pub parity_ok: u8,
    pub last_error_ptr: *mut c_uchar,
    pub last_error_len: c_ulong,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct connect_norito_acceleration_state {
    pub config: connect_norito_acceleration_config,
    pub simd: connect_norito_acceleration_backend_status,
    pub metal: connect_norito_acceleration_backend_status,
    pub cuda: connect_norito_acceleration_backend_status,
}

fn encode_acceleration_config(cfg: AccelerationConfig) -> connect_norito_acceleration_config {
    let (max_gpus, max_gpus_present) = option_to_ffi(cfg.max_gpus);
    let (merkle_min_leaves_gpu, merkle_min_leaves_gpu_present) =
        option_to_ffi(cfg.merkle_min_leaves_gpu);
    let (merkle_min_leaves_metal, merkle_min_leaves_metal_present) =
        option_to_ffi(cfg.merkle_min_leaves_metal);
    let (merkle_min_leaves_cuda, merkle_min_leaves_cuda_present) =
        option_to_ffi(cfg.merkle_min_leaves_cuda);
    let (prefer_cpu_sha2_max_leaves_aarch64, prefer_cpu_sha2_max_leaves_aarch64_present) =
        option_to_ffi(cfg.prefer_cpu_sha2_max_leaves_aarch64);
    let (prefer_cpu_sha2_max_leaves_x86, prefer_cpu_sha2_max_leaves_x86_present) =
        option_to_ffi(cfg.prefer_cpu_sha2_max_leaves_x86);

    connect_norito_acceleration_config {
        enable_simd: bool_to_u8(cfg.enable_simd),
        enable_metal: bool_to_u8(cfg.enable_metal),
        enable_cuda: bool_to_u8(cfg.enable_cuda),
        max_gpus,
        max_gpus_present,
        merkle_min_leaves_gpu,
        merkle_min_leaves_gpu_present,
        merkle_min_leaves_metal,
        merkle_min_leaves_metal_present,
        merkle_min_leaves_cuda,
        merkle_min_leaves_cuda_present,
        prefer_cpu_sha2_max_leaves_aarch64,
        prefer_cpu_sha2_max_leaves_aarch64_present,
        prefer_cpu_sha2_max_leaves_x86,
        prefer_cpu_sha2_max_leaves_x86_present,
    }
}

fn encode_backend_status(
    status: BackendRuntimeStatus,
    last_error: Option<String>,
) -> connect_norito_acceleration_backend_status {
    let (last_error_ptr, last_error_len) = if let Some(message) = last_error {
        let bytes = message.into_bytes();
        if bytes.is_empty() {
            (ptr::null_mut(), 0)
        } else {
            let len = bytes.len();
            let mem = unsafe { malloc(len) };
            if mem.is_null() {
                (ptr::null_mut(), 0)
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), mem as *mut u8, len);
                }
                (mem as *mut u8, len as c_ulong)
            }
        }
    } else {
        (ptr::null_mut(), 0)
    };

    connect_norito_acceleration_backend_status {
        supported: bool_to_u8(status.supported),
        configured: bool_to_u8(status.configured),
        available: bool_to_u8(status.available),
        parity_ok: bool_to_u8(status.parity_ok),
        last_error_ptr,
        last_error_len,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_set_acceleration_config(
    cfg: *const connect_norito_acceleration_config,
) {
    unsafe {
        let cfg = if let Some(cfg_ref) = cfg.as_ref() {
            cfg_ref
        } else {
            ivm::set_acceleration_config(AccelerationConfig::default());
            return;
        };

        let bool_from = |v: u8| v != 0;
        let usize_option = |present: u8, value: u64| {
            if present != 0 {
                Some(value as usize)
            } else {
                None
            }
        };

        let rust_cfg = AccelerationConfig {
            enable_simd: bool_from(cfg.enable_simd),
            enable_metal: bool_from(cfg.enable_metal),
            enable_cuda: bool_from(cfg.enable_cuda),
            max_gpus: usize_option(cfg.max_gpus_present, cfg.max_gpus),
            merkle_min_leaves_gpu: usize_option(
                cfg.merkle_min_leaves_gpu_present,
                cfg.merkle_min_leaves_gpu,
            ),
            merkle_min_leaves_metal: usize_option(
                cfg.merkle_min_leaves_metal_present,
                cfg.merkle_min_leaves_metal,
            ),
            merkle_min_leaves_cuda: usize_option(
                cfg.merkle_min_leaves_cuda_present,
                cfg.merkle_min_leaves_cuda,
            ),
            prefer_cpu_sha2_max_leaves_aarch64: usize_option(
                cfg.prefer_cpu_sha2_max_leaves_aarch64_present,
                cfg.prefer_cpu_sha2_max_leaves_aarch64,
            ),
            prefer_cpu_sha2_max_leaves_x86: usize_option(
                cfg.prefer_cpu_sha2_max_leaves_x86_present,
                cfg.prefer_cpu_sha2_max_leaves_x86,
            ),
        };

        ivm::set_acceleration_config(rust_cfg);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_get_acceleration_config(
    out_cfg: *mut connect_norito_acceleration_config,
) -> c_int {
    unsafe {
        if out_cfg.is_null() {
            return -1;
        }
        let cfg = ivm::acceleration_config();
        let encoded = encode_acceleration_config(cfg);
        ptr::write(out_cfg, encoded);
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_get_acceleration_state(
    out_state: *mut connect_norito_acceleration_state,
) -> c_int {
    unsafe {
        if out_state.is_null() {
            return -1;
        }
        let cfg = ivm::acceleration_config();
        let runtime = ivm::acceleration_runtime_status();
        let errors = ivm::acceleration_runtime_errors();
        let ivm::AccelerationErrorStatus { simd, metal, cuda } = errors;
        let state = connect_norito_acceleration_state {
            config: encode_acceleration_config(cfg),
            simd: encode_backend_status(runtime.simd, simd),
            metal: encode_backend_status(runtime.metal, metal),
            cuda: encode_backend_status(runtime.cuda, cuda),
        };
        ptr::write(out_state, state);
        0
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeCudaAvailable(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    use std::panic::catch_unwind;

    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let available = catch_unwind(ivm::cuda_available).unwrap_or(false);
    if available { JNI_TRUE } else { JNI_FALSE }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeCudaDisabled(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    use std::panic::catch_unwind;

    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let disabled = catch_unwind(ivm::cuda_disabled).unwrap_or(false);
    if disabled { JNI_TRUE } else { JNI_FALSE }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn throw_java_illegal_argument(env: &mut jni::JNIEnv<'_>, message: String) {
    let _ = env.throw_new("java/lang/IllegalArgumentException", message);
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn throw_java_illegal_state(env: &mut jni::JNIEnv<'_>, message: String) {
    let _ = env.throw_new("java/lang/IllegalStateException", message);
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn catch_unwind_to_java<T, F>(env: &mut jni::JNIEnv<'_>, label: &str, f: F) -> Option<T>
where
    F: FnOnce() -> T,
{
    use std::panic::{AssertUnwindSafe, catch_unwind};

    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => Some(value),
        Err(_) => {
            throw_java_illegal_state(env, format!("{label} panicked"));
            None
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn read_java_byte_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    context: &str,
) -> Option<Vec<u8>> {
    let len = match env.get_array_length(array) {
        Ok(value) => value,
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            return None;
        }
    } as usize;
    let mut buf = vec![0i8; len];
    if let Err(err) = env.get_byte_array_region(array, 0, &mut buf) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to read array contents: {err}"),
        );
        return None;
    }
    Some(buf.into_iter().map(|byte| byte as u8).collect())
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_public_key_from_private_bytes(
    algorithm_code: jni::sys::jint,
    private_key: &[u8],
) -> Result<Vec<u8>, String> {
    let algorithm = parse_algorithm_code(algorithm_code as u8)
        .map_err(|_| format!("unsupported signing algorithm code: {algorithm_code}"))?;
    let private_key = parse_private_key_with_algorithm(private_key, algorithm)
        .map_err(|_| "invalid private key bytes".to_string())?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|_| "failed to derive public key".to_string())?;
    key_pair
        .public_key()
        .try_to_bytes()
        .map(|(_algorithm, payload)| payload.to_vec())
        .map_err(|_| "failed to extract public key bytes".to_string())
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_keypair_from_seed_bytes(
    algorithm_code: jni::sys::jint,
    seed: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let algorithm = parse_algorithm_code(algorithm_code as u8)
        .map_err(|_| format!("unsupported signing algorithm code: {algorithm_code}"))?;
    let key_pair = KeyPair::from_seed(seed.to_vec(), algorithm);
    let (public_key, private_key) = key_pair.into_parts();
    let public_bytes = public_key
        .try_to_bytes()
        .map(|(_algorithm, payload)| payload.to_vec())
        .map_err(|_| "failed to extract public key bytes".to_string())?;
    Ok((private_key.to_bytes().1.to_vec(), public_bytes))
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_sign_detached_bytes(
    algorithm_code: jni::sys::jint,
    private_key: &[u8],
    message: &[u8],
) -> Result<Vec<u8>, String> {
    let algorithm = parse_algorithm_code(algorithm_code as u8)
        .map_err(|_| format!("unsupported signing algorithm code: {algorithm_code}"))?;
    let private_key = parse_private_key_with_algorithm(private_key, algorithm)
        .map_err(|_| "invalid private key bytes".to_string())?;
    Ok(Signature::new(&private_key, message).payload().to_vec())
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_verify_detached_bytes(
    algorithm_code: jni::sys::jint,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<bool, String> {
    let algorithm = parse_algorithm_code(algorithm_code as u8)
        .map_err(|_| format!("unsupported signing algorithm code: {algorithm_code}"))?;
    let public_key = PublicKey::from_bytes(algorithm, public_key)
        .map_err(|_| "invalid public key bytes".to_string())?;
    let signature = Signature::from_bytes(signature);
    match signature.verify(&public_key, message) {
        Ok(()) => Ok(true),
        Err(CryptoError::BadSignature) => Ok(false),
        Err(_) => Err("signature verification failed".to_string()),
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_public_key_from_private(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_bytes = read_java_byte_array(env, &private_key, "privateKey")
            .ok_or_else(|| "invalid private key bytes".to_string())?;
        let public_bytes = java_public_key_from_private_bytes(algorithm_code, &private_bytes)?;
        let array = env
            .byte_array_from_slice(&public_bytes)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_keypair_from_seed(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    seed: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        let seed_bytes = read_java_byte_array(env, &seed, "seed")
            .ok_or_else(|| "invalid seed bytes".to_string())?;
        let (private_bytes, public_bytes) =
            java_keypair_from_seed_bytes(algorithm_code, &seed_bytes)?;
        let private_array = env
            .byte_array_from_slice(&private_bytes)
            .map_err(|err| err.to_string())?;
        let public_array = env
            .byte_array_from_slice(&public_bytes)
            .map_err(|err| err.to_string())?;
        let byte_array_class = env.find_class("[B").map_err(|err| err.to_string())?;
        let array = env
            .new_object_array(2, byte_array_class, jni::objects::JObject::null())
            .map_err(|err| err.to_string())?;
        env.set_object_array_element(&array, 0, &private_array)
            .map_err(|err| err.to_string())?;
        env.set_object_array_element(&array, 1, &public_array)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_sign_detached(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_bytes = read_java_byte_array(env, &private_key, "privateKey")
            .ok_or_else(|| "invalid private key bytes".to_string())?;
        let message_bytes = read_java_byte_array(env, &message, "message")
            .ok_or_else(|| "invalid message bytes".to_string())?;
        let signature = java_sign_detached_bytes(algorithm_code, &private_bytes, &message_bytes)?;
        let array = env
            .byte_array_from_slice(&signature)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_verify_detached(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    public_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    let result = (|| -> Result<jni::sys::jboolean, String> {
        let public_bytes = read_java_byte_array(env, &public_key, "publicKey")
            .ok_or_else(|| "invalid public key bytes".to_string())?;
        let message_bytes = read_java_byte_array(env, &message, "message")
            .ok_or_else(|| "invalid message bytes".to_string())?;
        let signature_bytes = read_java_byte_array(env, &signature, "signature")
            .ok_or_else(|| "invalid signature bytes".to_string())?;
        let valid = java_verify_detached_bytes(
            algorithm_code,
            &public_bytes,
            &message_bytes,
            &signature_bytes,
        )?;
        Ok(if valid { 1 } else { 0 })
    })();
    match result {
        Ok(valid) => valid,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            0
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_kagemusha_prove_verified_compact_payment_token_with_records(
    record_bundle_archive: &[u8],
) -> Result<Vec<u8>, String> {
    let token = prove_verified_kagemusha_compact_token_from_record_bundle(record_bundle_archive)
        .map_err(|_| "invalid Kagemusha verified fold record bundle".to_string())?;
    norito::to_bytes(&token)
        .map_err(|err| format!("failed to encode Kagemusha compact token: {err}"))
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
    record_bundle_archive: &[u8],
    pallas_open_envelopes_archive: &[u8],
) -> Result<Vec<u8>, String> {
    let proof_bundle =
        prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive(
            record_bundle_archive,
            pallas_open_envelopes_archive,
        )
        .map_err(|_| {
            "invalid Kagemusha recursive aggregation record bundle or Pallas open-envelope archive"
                .to_string()
        })?;
    norito::to_bytes(&proof_bundle).map_err(|err| {
        format!("failed to encode Kagemusha recursive aggregation proof bundle: {err}")
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_kagemusha_prove_verified_compact_payment_token_with_records(
    env: &mut jni::JNIEnv<'_>,
    record_bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let archive_bytes =
            read_java_byte_array(env, &record_bundle_archive, "recordBundleArchive")
                .ok_or_else(|| "invalid Kagemusha verified fold record bundle bytes".to_string())?;
        let token_archive =
            java_kagemusha_prove_verified_compact_payment_token_with_records(&archive_bytes)?;
        let array = env
            .byte_array_from_slice(&token_archive)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
    env: &mut jni::JNIEnv<'_>,
    record_bundle_archive: jni::objects::JByteArray<'_>,
    pallas_open_envelopes_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let record_bundle_bytes =
            read_java_byte_array(env, &record_bundle_archive, "recordBundleArchive")
                .ok_or_else(|| "invalid Kagemusha verified fold record bundle bytes".to_string())?;
        let pallas_open_envelope_bytes = read_java_byte_array(
            env,
            &pallas_open_envelopes_archive,
            "pallasOpenEnvelopesArchive",
        )
        .ok_or_else(|| "invalid Kagemusha Pallas open-envelope archive bytes".to_string())?;
        let proof_bundle_archive =
            java_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
                &record_bundle_bytes,
                &pallas_open_envelope_bytes,
            )?;
        let array = env
            .byte_array_from_slice(&proof_bundle_archive)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_kagemusha_recursive_spend_archive(
    env: &mut jni::JNIEnv<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    label: &str,
    run: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let request_bytes = read_java_byte_array(env, &request_archive, "requestArchive")
            .ok_or_else(|| format!("invalid Kagemusha recursive spend {label} request bytes"))?;
        let output_archive = run(&request_bytes)?;
        let array = env
            .byte_array_from_slice(&output_archive)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_kagemusha_recursive_spend_lineage_witness_from_init_result_archive(
    env: &mut jni::JNIEnv<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let request_bytes = read_java_byte_array(env, &request_archive, "requestArchive")
            .ok_or_else(|| "invalid Kagemusha recursive spend init request bytes".to_owned())?;
        let bundle_bytes = read_java_byte_array(env, &bundle_archive, "bundleArchive")
            .ok_or_else(|| "invalid Kagemusha recursive spend bundle bytes".to_owned())?;
        let witness = kagemusha_recursive_spend_lineage_witness_from_init_result_archives(
            &request_bytes,
            &bundle_bytes,
        )
        .map_err(|_| "invalid Kagemusha recursive spend lineage witness init input".to_owned())?;
        let archive = norito::to_bytes(&witness)
            .map_err(|err| format!("failed to encode lineage witness: {err}"))?;
        let array = env
            .byte_array_from_slice(&archive)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn java_native_kagemusha_recursive_spend_lineage_witness_append_result_archive(
    env: &mut jni::JNIEnv<'_>,
    previous_witness_archive: jni::objects::JByteArray<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let previous_witness_bytes =
            read_java_byte_array(env, &previous_witness_archive, "previousWitnessArchive")
                .ok_or_else(|| {
                    "invalid Kagemusha recursive spend previous lineage witness bytes".to_owned()
                })?;
        let request_bytes = read_java_byte_array(env, &request_archive, "requestArchive")
            .ok_or_else(|| "invalid Kagemusha recursive spend append request bytes".to_owned())?;
        let bundle_bytes = read_java_byte_array(env, &bundle_archive, "bundleArchive")
            .ok_or_else(|| "invalid Kagemusha recursive spend bundle bytes".to_owned())?;
        let witness = kagemusha_recursive_spend_lineage_witness_append_result_archives(
            &previous_witness_bytes,
            &request_bytes,
            &bundle_bytes,
        )
        .map_err(|_| "invalid Kagemusha recursive spend lineage witness append input".to_owned())?;
        let archive = norito::to_bytes(&witness)
            .map_err(|err| format!("failed to encode lineage witness: {err}"))?;
        let array = env
            .byte_array_from_slice(&archive)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativePublicKeyFromPrivate(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_public_key_from_private(&mut env, algorithm_code, private_key)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeKeypairFromSeed(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    seed: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_keypair_from_seed(&mut env, algorithm_code, seed)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeSignDetached(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_sign_detached(&mut env, algorithm_code, private_key, message)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeVerifyDetached(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    public_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_verify_detached(&mut env, algorithm_code, public_key, message, signature)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativePublicKeyFromPrivate(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_public_key_from_private(&mut env, algorithm_code, private_key)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeKeypairFromSeed(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    seed: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_keypair_from_seed(&mut env, algorithm_code, seed)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignDetached(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_sign_detached(&mut env, algorithm_code, private_key, message)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeVerifyDetached(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    public_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_verify_detached(&mut env, algorithm_code, public_key, message, signature)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaCompactPaymentTokenProver_nativeProveVerifiedCompactPaymentTokenWithRecords(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    record_bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_prove_verified_compact_payment_token_with_records(
        &mut env,
        record_bundle_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveAggregationProofBundleProver_nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    record_bundle_archive: jni::objects::JByteArray<'_>,
    pallas_open_envelopes_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
        &mut env,
        record_bundle_archive,
        pallas_open_envelopes_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeInitSpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "init", |bytes| {
        let bundle = kagemusha_recursive_spend_init_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend init request".to_owned())?;
        norito::to_bytes(&bundle).map_err(|err| format!("failed to encode init bundle: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeAppendSpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "append", |bytes| {
        let bundle = kagemusha_recursive_spend_append_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend append request".to_owned())?;
        norito::to_bytes(&bundle).map_err(|err| format!("failed to encode append bundle: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeLineageWitnessFromInitResult(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_lineage_witness_from_init_result_archive(
        &mut env,
        request_archive,
        bundle_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeLineageWitnessAppendResult(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    previous_witness_archive: jni::objects::JByteArray<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_lineage_witness_append_result_archive(
        &mut env,
        previous_witness_archive,
        request_archive,
        bundle_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifySpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "verify", |bytes| {
        let result = kagemusha_recursive_spend_verify_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend verify request".to_owned())?;
        norito::to_bytes(&result).map_err(|err| format!("failed to encode verify result: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeRedeemSpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "redeem", |bytes| {
        let instruction = kagemusha_recursive_spend_redeem_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend redeem request".to_owned())?;
        norito::to_bytes(&instruction)
            .map_err(|err| format!("failed to encode redeem instruction: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaCompactPaymentTokenProver_nativeProveVerifiedCompactPaymentTokenWithRecords(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    record_bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_prove_verified_compact_payment_token_with_records(
        &mut env,
        record_bundle_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveAggregationProofBundleProver_nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    record_bundle_archive: jni::objects::JByteArray<'_>,
    pallas_open_envelopes_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
        &mut env,
        record_bundle_archive,
        pallas_open_envelopes_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeInitSpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "init", |bytes| {
        let bundle = kagemusha_recursive_spend_init_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend init request".to_owned())?;
        norito::to_bytes(&bundle).map_err(|err| format!("failed to encode init bundle: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeAppendSpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "append", |bytes| {
        let bundle = kagemusha_recursive_spend_append_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend append request".to_owned())?;
        norito::to_bytes(&bundle).map_err(|err| format!("failed to encode append bundle: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeLineageWitnessFromInitResult(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_lineage_witness_from_init_result_archive(
        &mut env,
        request_archive,
        bundle_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeLineageWitnessAppendResult(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    previous_witness_archive: jni::objects::JByteArray<'_>,
    request_archive: jni::objects::JByteArray<'_>,
    bundle_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_lineage_witness_append_result_archive(
        &mut env,
        previous_witness_archive,
        request_archive,
        bundle_archive,
    )
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifySpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "verify", |bytes| {
        let result = kagemusha_recursive_spend_verify_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend verify request".to_owned())?;
        norito::to_bytes(&result).map_err(|err| format!("failed to encode verify result: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeRedeemSpend(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_recursive_spend_archive(&mut env, request_archive, "redeem", |bytes| {
        let instruction = kagemusha_recursive_spend_redeem_from_request_archive(bytes)
            .map_err(|_| "invalid Kagemusha recursive spend redeem request".to_owned())?;
        norito::to_bytes(&instruction)
            .map_err(|err| format!("failed to encode redeem instruction: {err}"))
    })
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn ensure_min_array_length(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    required: i32,
    context: &str,
) -> bool {
    match env.get_array_length(array) {
        Ok(len) if len >= required => true,
        Ok(len) => {
            throw_java_illegal_argument(
                env,
                format!("{context} expects an output array with length >= {required}, got {len}"),
            );
            false
        }
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            false
        }
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn read_long_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: &str,
) -> Option<Vec<i64>> {
    let len = match env.get_array_length(array) {
        Ok(value) => value,
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            return None;
        }
    } as usize;

    let mut buf = vec![0i64; len];
    if let Err(err) = env.get_long_array_region(array, 0, &mut buf) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to read array contents: {err}"),
        );
        return None;
    }
    Some(buf)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn write_long_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    values: &[i64],
    context: &str,
) -> bool {
    if let Err(err) = env.set_long_array_region(array, 0, values) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to write output array: {err}"),
        );
        return false;
    }
    true
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn convert_field_elem<L: Into<String>>(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: L,
) -> Option<[u64; 4]> {
    let context = context.into();
    let buf = read_long_array(env, array, &context)?;
    if buf.len() != 4 {
        throw_java_illegal_argument(env, format!("{context} expects an array of length 4"));
        return None;
    }
    let mut limbs = [0u64; 4];
    for (dst, src) in limbs.iter_mut().zip(buf.iter()) {
        *dst = *src as u64;
    }
    Some(limbs)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn convert_field_elems<L: Into<String>>(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: L,
) -> Option<Vec<[u64; 4]>> {
    let context = context.into();
    let buf = read_long_array(env, array, &context)?;
    if buf.len() % 4 != 0 {
        throw_java_illegal_argument(
            env,
            format!("{context} expects a flattened array with a length multiple of 4"),
        );
        return None;
    }
    let mut elems = Vec::with_capacity(buf.len() / 4);
    for chunk in buf.chunks_exact(4) {
        let mut limbs = [0u64; 4];
        for (dst, src) in limbs.iter_mut().zip(chunk.iter()) {
            *dst = *src as u64;
        }
        elems.push(limbs);
    }
    Some(elems)
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::sys::jlong,
    b: jni::sys::jlong,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    if !ensure_min_array_length(&mut env, &out, 1, "poseidon2") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon2_cuda", || {
        ivm::poseidon2_cuda(a as u64, b as u64)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(hash) = result {
        let value = [hash as i64];
        if write_long_array(&mut env, &out, &value, "poseidon2") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon2Batch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let buf = match read_long_array(&mut env, &inputs, "poseidon2Batch inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() % 2 != 0 {
        throw_java_illegal_argument(
            &mut env,
            "poseidon2Batch inputs must contain an even number of elements".into(),
        );
        return JNI_FALSE;
    }
    let batch_size = (buf.len() / 2) as i32;
    if !ensure_min_array_length(&mut env, &out, batch_size, "poseidon2Batch") {
        return JNI_FALSE;
    }
    let mut tuples = Vec::with_capacity(batch_size as usize);
    for chunk in buf.chunks_exact(2) {
        tuples.push((chunk[0] as u64, chunk[1] as u64));
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon2_cuda_many", || {
        ivm::poseidon2_cuda_many(&tuples)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(outputs) = result {
        let values: Vec<i64> = outputs.into_iter().map(|value| value as i64).collect();
        if write_long_array(&mut env, &out, &values, "poseidon2Batch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon6(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    if !ensure_min_array_length(&mut env, &out, 1, "poseidon6") {
        return JNI_FALSE;
    }
    let buf = match read_long_array(&mut env, &inputs, "poseidon6 inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() != 6 {
        throw_java_illegal_argument(&mut env, "poseidon6 expects six inputs".into());
        return JNI_FALSE;
    }
    let mut state = [0u64; 6];
    for (dst, src) in state.iter_mut().zip(buf.iter()) {
        *dst = *src as u64;
    }
    let result =
        match catch_unwind_to_java(&mut env, "poseidon6_cuda", || ivm::poseidon6_cuda(state)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(hash) = result {
        let value = [hash as i64];
        if write_long_array(&mut env, &out, &value, "poseidon6") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon6Batch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let buf = match read_long_array(&mut env, &inputs, "poseidon6Batch inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() % 6 != 0 {
        throw_java_illegal_argument(
            &mut env,
            "poseidon6Batch inputs must be multiples of six".into(),
        );
        return JNI_FALSE;
    }
    let batch_size = (buf.len() / 6) as i32;
    if !ensure_min_array_length(&mut env, &out, batch_size, "poseidon6Batch") {
        return JNI_FALSE;
    }
    let mut states = Vec::with_capacity(batch_size as usize);
    for chunk in buf.chunks_exact(6) {
        let mut state = [0u64; 6];
        for (dst, src) in state.iter_mut().zip(chunk.iter()) {
            *dst = *src as u64;
        }
        states.push(state);
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon6_cuda_many", || {
        ivm::poseidon6_cuda_many(&states)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(outputs) = result {
        let values: Vec<i64> = outputs.into_iter().map(|value| value as i64).collect();
        if write_long_array(&mut env, &out, &values, "poseidon6Batch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254Add(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::objects::JLongArray<'_>,
    b: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    if !ensure_min_array_length(&mut env, &out, 4, "bn254Add") {
        return JNI_FALSE;
    }
    let a = match convert_field_elem(&mut env, &a, "bn254Add input a") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let b = match convert_field_elem(&mut env, &b, "bn254Add input b") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let result =
        match catch_unwind_to_java(&mut env, "bn254_add_cuda", || ivm::bn254_add_cuda(a, b)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(field) = result {
        let values: Vec<i64> = field.into_iter().map(|limb| limb as i64).collect();
        if write_long_array(&mut env, &out, &values, "bn254Add") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254Sub(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::objects::JLongArray<'_>,
    b: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    if !ensure_min_array_length(&mut env, &out, 4, "bn254Sub") {
        return JNI_FALSE;
    }
    let a = match convert_field_elem(&mut env, &a, "bn254Sub input a") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let b = match convert_field_elem(&mut env, &b, "bn254Sub input b") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let result =
        match catch_unwind_to_java(&mut env, "bn254_sub_cuda", || ivm::bn254_sub_cuda(a, b)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(field) = result {
        let values: Vec<i64> = field.into_iter().map(|limb| limb as i64).collect();
        if write_long_array(&mut env, &out, &values, "bn254Sub") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254Mul(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::objects::JLongArray<'_>,
    b: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    if !ensure_min_array_length(&mut env, &out, 4, "bn254Mul") {
        return JNI_FALSE;
    }
    let a = match convert_field_elem(&mut env, &a, "bn254Mul input a") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let b = match convert_field_elem(&mut env, &b, "bn254Mul input b") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let result =
        match catch_unwind_to_java(&mut env, "bn254_mul_cuda", || ivm::bn254_mul_cuda(a, b)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(field) = result {
        let values: Vec<i64> = field.into_iter().map(|limb| limb as i64).collect();
        if write_long_array(&mut env, &out, &values, "bn254Mul") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254AddBatch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let lhs = match convert_field_elems(&mut env, &lhs, "bn254AddBatch lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254AddBatch rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(
            &mut env,
            "bn254AddBatch expects matching batch lengths".into(),
        );
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254AddBatch output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254AddBatch") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_add_batch_cuda", || {
        ivm::bn254_add_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254AddBatch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254SubBatch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let lhs = match convert_field_elems(&mut env, &lhs, "bn254SubBatch lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254SubBatch rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(
            &mut env,
            "bn254SubBatch expects matching batch lengths".into(),
        );
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254SubBatch output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254SubBatch") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_sub_batch_cuda", || {
        ivm::bn254_sub_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254SubBatch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254MulBatch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};

    let lhs = match convert_field_elems(&mut env, &lhs, "bn254MulBatch lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254MulBatch rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(
            &mut env,
            "bn254MulBatch expects matching batch lengths".into(),
        );
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254MulBatch output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254MulBatch") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_mul_batch_cuda", || {
        ivm::bn254_mul_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254MulBatch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

fn providers_from_json(value: &JsonValue) -> Result<Vec<LocalProviderInput>, c_int> {
    let arr = value.as_array().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let mut providers = Vec::with_capacity(arr.len());
    for entry in arr {
        let obj = entry.as_object().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
        let name = obj
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
            .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
        let path = obj
            .get("path")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
            .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
        let max_concurrent = obj
            .get("max_concurrent")
            .and_then(JsonValue::as_u64)
            .map(|value| {
                let converted =
                    u32::try_from(value).map_err(|_| ERR_FETCH_INVALID_MAX_CONCURRENT)?;
                if converted == 0 {
                    Err(ERR_FETCH_INVALID_MAX_CONCURRENT)
                } else {
                    Ok(converted)
                }
            })
            .transpose()?;
        let weight = obj
            .get("weight")
            .and_then(JsonValue::as_u64)
            .map(|value| u32::try_from(value).map_err(|_| ERR_FETCH_INVALID_WEIGHT))
            .transpose()?;
        let metadata = obj
            .get("metadata")
            .map(|value| provider_metadata_from_json(value, &name))
            .transpose()?;
        providers.push(LocalProviderInput {
            name,
            path: PathBuf::from(path),
            max_concurrent,
            weight,
            metadata,
        });
    }
    Ok(providers)
}

fn provider_metadata_from_json(
    value: &JsonValue,
    alias: &str,
) -> Result<ProviderMetadataInput, c_int> {
    let obj = value.as_object().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let provider_id = Some(
        obj.get("provider_id")
            .and_then(JsonValue::as_str)
            .unwrap_or(alias)
            .to_owned(),
    );
    let profile_id = obj
        .get("profile_id")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let profile_aliases =
        if let Some(aliases) = obj.get("profile_aliases").and_then(JsonValue::as_array) {
            let mut list = Vec::with_capacity(aliases.len());
            for alias in aliases {
                list.push(alias.as_str().ok_or(ERR_FETCH_PROVIDERS_JSON)?.to_owned());
            }
            Some(list)
        } else {
            None
        };
    let availability = obj
        .get("availability")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let stake_amount = obj
        .get("stake_amount")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let max_streams = obj
        .get("max_streams")
        .and_then(JsonValue::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| ERR_FETCH_PROVIDERS_JSON))
        .transpose()?;
    let refresh_deadline = obj.get("refresh_deadline").and_then(JsonValue::as_u64);
    let expires_at = obj.get("expires_at").and_then(JsonValue::as_u64);
    let ttl_secs = obj.get("ttl_secs").and_then(JsonValue::as_u64);
    let allow_unknown_capabilities = obj
        .get("allow_unknown_capabilities")
        .and_then(JsonValue::as_bool);
    let capability_names =
        if let Some(names) = obj.get("capability_names").and_then(JsonValue::as_array) {
            let mut list = Vec::with_capacity(names.len());
            for name in names {
                list.push(name.as_str().ok_or(ERR_FETCH_PROVIDERS_JSON)?.to_owned());
            }
            Some(list)
        } else {
            None
        };
    let rendezvous_topics =
        if let Some(topics) = obj.get("rendezvous_topics").and_then(JsonValue::as_array) {
            let mut list = Vec::with_capacity(topics.len());
            for topic in topics {
                list.push(topic.as_str().ok_or(ERR_FETCH_PROVIDERS_JSON)?.to_owned());
            }
            Some(list)
        } else {
            None
        };
    let notes = obj
        .get("notes")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let range_capability = obj
        .get("range_capability")
        .map(range_capability_from_json)
        .transpose()?;
    let stream_budget = obj
        .get("stream_budget")
        .map(stream_budget_from_json)
        .transpose()?;
    let transport_hints = obj
        .get("transport_hints")
        .map(transport_hints_from_json)
        .transpose()?;

    Ok(ProviderMetadataInput {
        provider_id,
        profile_id,
        profile_aliases,
        availability,
        stake_amount,
        max_streams,
        refresh_deadline,
        expires_at,
        ttl_secs,
        allow_unknown_capabilities,
        capability_names,
        rendezvous_topics,
        notes,
        range_capability,
        stream_budget,
        transport_hints,
    })
}

fn range_capability_from_json(value: &JsonValue) -> Result<RangeCapabilityInput, c_int> {
    let obj = value.as_object().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let max_chunk_span = obj
        .get("max_chunk_span")
        .and_then(JsonValue::as_u64)
        .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let min_granularity = obj
        .get("min_granularity")
        .and_then(JsonValue::as_u64)
        .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    Ok(RangeCapabilityInput {
        max_chunk_span: u32::try_from(max_chunk_span).map_err(|_| ERR_FETCH_PROVIDERS_JSON)?,
        min_granularity: u32::try_from(min_granularity).map_err(|_| ERR_FETCH_PROVIDERS_JSON)?,
        supports_sparse_offsets: obj
            .get("supports_sparse_offsets")
            .and_then(JsonValue::as_bool),
        requires_alignment: obj.get("requires_alignment").and_then(JsonValue::as_bool),
        supports_merkle_proof: obj
            .get("supports_merkle_proof")
            .and_then(JsonValue::as_bool),
    })
}

fn stream_budget_from_json(value: &JsonValue) -> Result<StreamBudgetInput, c_int> {
    let obj = value.as_object().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let max_in_flight = obj
        .get("max_in_flight")
        .and_then(JsonValue::as_u64)
        .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let max_bytes_per_sec = obj
        .get("max_bytes_per_sec")
        .and_then(JsonValue::as_u64)
        .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let burst_bytes = obj.get("burst_bytes").and_then(JsonValue::as_u64);
    Ok(StreamBudgetInput {
        max_in_flight: u16::try_from(max_in_flight).map_err(|_| ERR_FETCH_PROVIDERS_JSON)?,
        max_bytes_per_sec,
        burst_bytes,
    })
}

fn transport_hints_from_json(value: &JsonValue) -> Result<Vec<TransportHintInput>, c_int> {
    let arr = value.as_array().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
    let mut hints = Vec::with_capacity(arr.len());
    for entry in arr {
        let obj = entry.as_object().ok_or(ERR_FETCH_PROVIDERS_JSON)?;
        let protocol = obj
            .get("protocol")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
            .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
        let protocol_id = obj
            .get("protocol_id")
            .and_then(JsonValue::as_u64)
            .map(|value| u8::try_from(value).map_err(|_| ERR_FETCH_PROVIDERS_JSON))
            .transpose()?
            .ok_or(ERR_FETCH_PROVIDERS_JSON)?;
        let priority = obj.get("priority").and_then(JsonValue::as_i64).unwrap_or(0);
        hints.push(TransportHintInput {
            protocol,
            protocol_id,
            priority: priority as u8,
        });
    }
    Ok(hints)
}

fn telemetry_from_json(value: &JsonValue) -> Result<TelemetryEntryInput, c_int> {
    let obj = value.as_object().ok_or(ERR_FETCH_OPTIONS_JSON)?;
    let provider_id = obj
        .get("provider_id")
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or(ERR_FETCH_OPTIONS_JSON)?;
    Ok(TelemetryEntryInput {
        provider_id,
        qos_score: obj.get("qos_score").and_then(JsonValue::as_f64),
        latency_p95_ms: obj.get("latency_p95_ms").and_then(JsonValue::as_f64),
        failure_rate_ewma: obj.get("failure_rate_ewma").and_then(JsonValue::as_f64),
        token_health: obj.get("token_health").and_then(JsonValue::as_f64),
        staking_weight: obj.get("staking_weight").and_then(JsonValue::as_f64),
        penalty: obj.get("penalty").and_then(JsonValue::as_bool),
        last_updated_unix: obj.get("last_updated_unix").and_then(JsonValue::as_u64),
    })
}

fn options_from_json(value: &JsonValue) -> Result<LocalFetchOptions, c_int> {
    let obj = value.as_object().ok_or(ERR_FETCH_OPTIONS_JSON)?;
    let verify_digests = obj.get("verify_digests").and_then(JsonValue::as_bool);
    let verify_lengths = obj.get("verify_lengths").and_then(JsonValue::as_bool);
    let retry_budget = obj
        .get("retry_budget")
        .and_then(JsonValue::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| ERR_FETCH_OPTIONS_JSON))
        .transpose()?;
    let provider_failure_threshold = obj
        .get("provider_failure_threshold")
        .and_then(JsonValue::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| ERR_FETCH_OPTIONS_JSON))
        .transpose()?;
    let max_parallel = obj
        .get("max_parallel")
        .and_then(JsonValue::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| ERR_FETCH_OPTIONS_JSON))
        .transpose()?;
    let max_peers = obj
        .get("max_peers")
        .and_then(JsonValue::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| ERR_FETCH_OPTIONS_JSON))
        .transpose()?;
    let chunker_handle = obj
        .get("chunker_handle")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let telemetry_region = obj
        .get("telemetry_region")
        .and_then(JsonValue::as_str)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let telemetry = if let Some(telemetry) = obj.get("telemetry").and_then(JsonValue::as_array) {
        let mut entries = Vec::with_capacity(telemetry.len());
        for entry in telemetry {
            entries.push(telemetry_from_json(entry)?);
        }
        entries
    } else {
        Vec::new()
    };
    let use_scoreboard = obj.get("use_scoreboard").and_then(JsonValue::as_bool);
    let scoreboard_now_unix_secs = obj
        .get("scoreboard_now_unix_secs")
        .and_then(JsonValue::as_u64);
    let deny_providers = if let Some(deny) = obj.get("deny_providers").and_then(JsonValue::as_array)
    {
        let mut list = Vec::with_capacity(deny.len());
        for entry in deny {
            list.push(entry.as_str().ok_or(ERR_FETCH_OPTIONS_JSON)?.to_owned());
        }
        list
    } else {
        Vec::new()
    };
    let boost_providers =
        if let Some(boosts) = obj.get("boost_providers").and_then(JsonValue::as_array) {
            let mut list = Vec::with_capacity(boosts.len());
            for entry in boosts {
                let boost_obj = entry.as_object().ok_or(ERR_FETCH_OPTIONS_JSON)?;
                let provider = boost_obj
                    .get("provider")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
                    .ok_or(ERR_FETCH_OPTIONS_JSON)?;
                let delta = boost_obj
                    .get("delta")
                    .and_then(JsonValue::as_i64)
                    .ok_or(ERR_FETCH_OPTIONS_JSON)?;
                list.push((provider, delta));
            }
            list
        } else {
            Vec::new()
        };
    let return_scoreboard = obj.get("return_scoreboard").and_then(JsonValue::as_bool);
    Ok(LocalFetchOptions {
        verify_digests,
        verify_lengths,
        retry_budget,
        provider_failure_threshold,
        max_parallel,
        max_peers,
        chunker_handle,
        telemetry_region,
        telemetry,
        use_scoreboard,
        scoreboard_now_unix_secs,
        deny_providers,
        boost_providers,
        return_scoreboard,
    })
}

fn local_fetch_result_to_json(result: &LocalFetchResult) -> JsonValue {
    let mut root = JsonMap::new();
    root.insert(
        "chunk_count".into(),
        JsonValue::from(result.chunk_count as u64),
    );

    let provider_reports = result
        .outcome
        .provider_reports
        .iter()
        .map(|report| {
            let mut obj = JsonMap::new();
            obj.insert(
                "provider".into(),
                JsonValue::from(report.provider.id().as_str().to_owned()),
            );
            obj.insert("successes".into(), JsonValue::from(report.successes as u64));
            obj.insert("failures".into(), JsonValue::from(report.failures as u64));
            obj.insert("disabled".into(), JsonValue::from(report.disabled));
            JsonValue::Object(obj)
        })
        .collect::<Vec<_>>();
    root.insert(
        "provider_reports".into(),
        JsonValue::Array(provider_reports),
    );

    let receipts = result
        .outcome
        .chunk_receipts
        .iter()
        .map(|receipt| {
            let mut obj = JsonMap::new();
            obj.insert(
                "chunk_index".into(),
                JsonValue::from(receipt.chunk_index as u64),
            );
            obj.insert(
                "provider".into(),
                JsonValue::from(receipt.provider.as_str().to_owned()),
            );
            obj.insert("attempts".into(), JsonValue::from(receipt.attempts as u64));
            obj.insert("latency_ms".into(), JsonValue::from(receipt.latency_ms));
            obj.insert("bytes".into(), JsonValue::from(receipt.bytes as u64));
            JsonValue::Object(obj)
        })
        .collect::<Vec<_>>();
    root.insert("chunk_receipts".into(), JsonValue::Array(receipts));

    if let Some(scoreboard) = result.scoreboard.as_ref() {
        let entries = scoreboard
            .iter()
            .map(|entry| {
                let mut obj = JsonMap::new();
                obj.insert(
                    "provider_id".into(),
                    JsonValue::from(entry.provider_id.clone()),
                );
                obj.insert("alias".into(), JsonValue::from(entry.alias.clone()));
                obj.insert("raw_score".into(), JsonValue::from(entry.raw_score));
                obj.insert(
                    "normalized_weight".into(),
                    JsonValue::from(entry.normalized_weight),
                );
                obj.insert(
                    "eligibility".into(),
                    JsonValue::from(entry.eligibility.clone()),
                );
                JsonValue::Object(obj)
            })
            .collect::<Vec<_>>();
        root.insert("scoreboard".into(), JsonValue::Array(entries));
    } else {
        root.insert("scoreboard".into(), JsonValue::Null);
    }
    if let Some(region) = result.telemetry_region.as_deref() {
        root.insert("telemetry_region".into(), JsonValue::from(region));
    } else {
        root.insert("telemetry_region".into(), JsonValue::Null);
    }

    JsonValue::Object(root)
}

fn write_json_value(out_ptr: *mut *mut c_uchar, out_len: *mut c_ulong, value: &JsonValue) -> c_int {
    match norito::json::to_vec(value) {
        Ok(bytes) => unsafe { write_bytes(out_ptr, out_len, &bytes) }.map_or_else(|err| err, |_| 0),
        Err(_) => ERR_FETCH_EXECUTION,
    }
}

fn map_local_fetch_error(err: LocalFetchError) -> c_int {
    match err {
        LocalFetchError::NoProviders => ERR_FETCH_NO_PROVIDERS,
        LocalFetchError::DuplicateProvider(_) => ERR_FETCH_DUPLICATE_PROVIDER,
        LocalFetchError::ProviderPathMissing { .. } => ERR_FETCH_PROVIDER_PATH_MISSING,
        LocalFetchError::ProviderPathNotFile { .. } => ERR_FETCH_PROVIDER_PATH_NOT_FILE,
        LocalFetchError::InvalidMaxConcurrent => ERR_FETCH_INVALID_MAX_CONCURRENT,
        LocalFetchError::InvalidWeight => ERR_FETCH_INVALID_WEIGHT,
        LocalFetchError::InvalidPlan(_) => ERR_FETCH_PLAN_JSON,
        LocalFetchError::MissingScoreboardMetadata(_) => ERR_FETCH_SCOREBOARD_METADATA,
        LocalFetchError::ScoreboardExcludedAll => ERR_FETCH_SCOREBOARD_EXCLUDED,
        LocalFetchError::ScoreboardBuild(_) => ERR_FETCH_SCOREBOARD_BUILD,
        LocalFetchError::Fetch(_) => ERR_FETCH_EXECUTION,
        LocalFetchError::UnknownChunkerHandle(_) => ERR_FETCH_UNKNOWN_CHUNKER,
    }
}

#[derive(Clone)]
struct DaProofSummaryOptions {
    sample_count: usize,
    sample_seed: u64,
    explicit_indexes: Vec<usize>,
}

impl DaProofSummaryOptions {
    fn from_raw(sample_count: c_ulong, sample_seed: u64, indexes: &[usize]) -> Result<Self, c_int> {
        let sample_count = usize::try_from(sample_count).map_err(|_| ERR_DA_PROOF_SUMMARY)?;
        Ok(Self {
            sample_count,
            sample_seed,
            explicit_indexes: indexes.to_vec(),
        })
    }
}

#[derive(Clone, Copy)]
enum ProofOrigin {
    Sampled,
    Explicit,
}

impl ProofOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::Sampled => "sampled",
            Self::Explicit => "explicit",
        }
    }
}

struct ProofReport {
    origin: ProofOrigin,
    leaf_index: usize,
    proof: PorProof,
    verified: bool,
}

fn da_proof_summary_json(
    manifest_bytes: &[u8],
    payload_bytes: &[u8],
    options: &DaProofSummaryOptions,
) -> Result<JsonValue, c_int> {
    let manifest: DaManifestV1 =
        decode_from_bytes(manifest_bytes).map_err(|_| ERR_DA_PROOF_SUMMARY)?;
    let plan = build_plan_from_da_manifest(&manifest).map_err(|_| ERR_DA_PROOF_SUMMARY)?;
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    let mut ingest_source = InMemoryPayload::new(payload_bytes);
    store
        .ingest_plan_source(&plan, &mut ingest_source)
        .map_err(|_| ERR_DA_PROOF_SUMMARY)?;
    validate_manifest_consistency(&manifest, &store)?;

    let por_root = *store.por_tree().root();
    let mut reports = collect_sampled_proofs(&store, payload_bytes, options, &por_root)?;
    let mut explicit = collect_explicit_proofs(&store, payload_bytes, options, &por_root)?;
    reports.append(&mut explicit);

    let mut summary = JsonMap::new();
    summary.insert(
        "blob_hash_hex".into(),
        JsonValue::from(hex::encode(manifest.blob_hash.as_ref())),
    );
    summary.insert(
        "chunk_root_hex".into(),
        JsonValue::from(hex::encode(manifest.chunk_root.as_ref())),
    );
    summary.insert(
        "por_root_hex".into(),
        JsonValue::from(hex::encode(store.por_tree().root())),
    );
    summary.insert(
        "leaf_count".into(),
        value_from_usize(store.por_tree().leaf_count()),
    );
    summary.insert(
        "segment_count".into(),
        value_from_usize(store.por_tree().segment_count()),
    );
    summary.insert(
        "chunk_count".into(),
        value_from_usize(store.por_tree().chunks().len()),
    );
    summary.insert(
        "sample_count".into(),
        value_from_usize(options.sample_count),
    );
    summary.insert("sample_seed".into(), JsonValue::from(options.sample_seed));
    summary.insert("proof_count".into(), value_from_usize(reports.len()));
    let proof_values = reports.iter().map(proof_report_to_json).collect::<Vec<_>>();
    summary.insert("proofs".into(), JsonValue::Array(proof_values));
    Ok(JsonValue::Object(summary))
}

fn validate_manifest_consistency(manifest: &DaManifestV1, store: &ChunkStore) -> Result<(), c_int> {
    let blob_hash_bytes = manifest.blob_hash.as_ref();
    if store.payload_digest().as_bytes() != blob_hash_bytes {
        return Err(ERR_DA_PROOF_SUMMARY);
    }
    let chunk_root_bytes = manifest.chunk_root.as_ref();
    if store.por_tree().root() != chunk_root_bytes {
        return Err(ERR_DA_PROOF_SUMMARY);
    }
    Ok(())
}

fn collect_sampled_proofs(
    store: &ChunkStore,
    payload: &[u8],
    options: &DaProofSummaryOptions,
    por_root: &[u8; 32],
) -> Result<Vec<ProofReport>, c_int> {
    if options.sample_count == 0 {
        return Ok(Vec::new());
    }
    let mut source = InMemoryPayload::new(payload);
    let samples = store
        .sample_leaves_with(options.sample_count, options.sample_seed, &mut source)
        .map_err(chunk_store_error_code)?;
    Ok(samples
        .into_iter()
        .map(|(leaf_index, proof)| ProofReport {
            origin: ProofOrigin::Sampled,
            leaf_index,
            verified: proof.verify(por_root),
            proof,
        })
        .collect())
}

fn collect_explicit_proofs(
    store: &ChunkStore,
    payload: &[u8],
    options: &DaProofSummaryOptions,
    por_root: &[u8; 32],
) -> Result<Vec<ProofReport>, c_int> {
    if options.explicit_indexes.is_empty() {
        return Ok(Vec::new());
    }
    let mut source = InMemoryPayload::new(payload);
    let mut reports = Vec::with_capacity(options.explicit_indexes.len());
    let mut seen = HashSet::new();
    for &leaf_index in &options.explicit_indexes {
        if !seen.insert(leaf_index) {
            continue;
        }
        let (chunk_idx, segment_idx, inner_idx) = store
            .por_tree()
            .leaf_path(leaf_index)
            .ok_or(ERR_DA_PROOF_SUMMARY)?;
        let proof = store
            .por_tree()
            .prove_leaf_with(chunk_idx, segment_idx, inner_idx, &mut source)
            .map_err(chunk_store_error_code)?
            .ok_or(ERR_DA_PROOF_SUMMARY)?;
        reports.push(ProofReport {
            origin: ProofOrigin::Explicit,
            leaf_index,
            verified: proof.verify(por_root),
            proof,
        });
    }
    Ok(reports)
}

fn chunk_store_error_code(err: ChunkStoreError) -> c_int {
    tracing::debug!("chunk store error during DA proof summary: {err}");
    ERR_DA_PROOF_SUMMARY
}

fn proof_report_to_json(report: &ProofReport) -> JsonValue {
    let mut map = JsonMap::new();
    map.insert("origin".into(), JsonValue::from(report.origin.as_str()));
    map.insert("leaf_index".into(), value_from_usize(report.leaf_index));
    map.insert(
        "chunk_index".into(),
        value_from_usize(report.proof.chunk_index),
    );
    map.insert(
        "segment_index".into(),
        value_from_usize(report.proof.segment_index),
    );
    map.insert(
        "leaf_offset".into(),
        JsonValue::from(report.proof.leaf_offset),
    );
    map.insert(
        "leaf_length".into(),
        value_from_u32(report.proof.leaf_length),
    );
    map.insert(
        "segment_offset".into(),
        JsonValue::from(report.proof.segment_offset),
    );
    map.insert(
        "segment_length".into(),
        value_from_u32(report.proof.segment_length),
    );
    map.insert(
        "chunk_offset".into(),
        JsonValue::from(report.proof.chunk_offset),
    );
    map.insert(
        "chunk_length".into(),
        value_from_u32(report.proof.chunk_length),
    );
    map.insert(
        "payload_len".into(),
        JsonValue::from(report.proof.payload_len),
    );
    map.insert(
        "chunk_digest_hex".into(),
        JsonValue::from(hex::encode(report.proof.chunk_digest)),
    );
    map.insert(
        "chunk_root_hex".into(),
        JsonValue::from(hex::encode(report.proof.chunk_root)),
    );
    map.insert(
        "segment_digest_hex".into(),
        JsonValue::from(hex::encode(report.proof.segment_digest)),
    );
    map.insert(
        "leaf_digest_hex".into(),
        JsonValue::from(hex::encode(report.proof.leaf_digest)),
    );
    map.insert(
        "leaf_bytes_b64".into(),
        JsonValue::from(b64gp::STANDARD.encode(&report.proof.leaf_bytes)),
    );
    map.insert(
        "segment_leaves_hex".into(),
        JsonValue::Array(
            report
                .proof
                .segment_leaves
                .iter()
                .map(|digest| JsonValue::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_segments_hex".into(),
        JsonValue::Array(
            report
                .proof
                .chunk_segments
                .iter()
                .map(|digest| JsonValue::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert(
        "chunk_roots_hex".into(),
        JsonValue::Array(
            report
                .proof
                .chunk_roots
                .iter()
                .map(|digest| JsonValue::from(hex::encode(digest)))
                .collect(),
        ),
    );
    map.insert("verified".into(), JsonValue::from(report.verified));
    JsonValue::Object(map)
}

fn value_from_usize(value: usize) -> JsonValue {
    JsonValue::from(u64::try_from(value).unwrap_or(u64::MAX))
}

fn value_from_u32(value: u32) -> JsonValue {
    JsonValue::from(u64::from(value))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_sorafs_local_fetch(
    plan_ptr: *const c_char,
    plan_len: c_ulong,
    providers_ptr: *const c_char,
    providers_len: c_ulong,
    options_ptr: *const c_char,
    options_len: c_ulong,
    out_payload_ptr: *mut *mut c_uchar,
    out_payload_len: *mut c_ulong,
    out_report_ptr: *mut *mut c_uchar,
    out_report_len: *mut c_ulong,
) -> c_int {
    if out_payload_ptr.is_null()
        || out_payload_len.is_null()
        || out_report_ptr.is_null()
        || out_report_len.is_null()
    {
        return ERR_NULL_PTR;
    }

    let plan_str = match unsafe { read_string_bridge(plan_ptr, plan_len) } {
        Ok(value) => value,
        Err(err) => return err.code(),
    };
    let plan_json: JsonValue = match norito::json::from_str(&plan_str) {
        Ok(value) => value,
        Err(_) => return ERR_FETCH_PLAN_JSON,
    };

    let providers_str = match unsafe { read_string_bridge(providers_ptr, providers_len) } {
        Ok(value) => value,
        Err(err) => return err.code(),
    };
    let providers_json: JsonValue = match norito::json::from_str(&providers_str) {
        Ok(value) => value,
        Err(_) => return ERR_FETCH_PROVIDERS_JSON,
    };
    let providers = match providers_from_json(&providers_json) {
        Ok(list) => list,
        Err(code) => return code,
    };

    let options = if options_ptr.is_null() || options_len == 0 {
        LocalFetchOptions::default()
    } else {
        let options_str = match unsafe { read_string_bridge(options_ptr, options_len) } {
            Ok(value) => value,
            Err(err) => return err.code(),
        };
        let options_json: JsonValue = match norito::json::from_str(&options_str) {
            Ok(value) => value,
            Err(_) => return ERR_FETCH_OPTIONS_JSON,
        };
        match options_from_json(&options_json) {
            Ok(opts) => opts,
            Err(code) => return code,
        }
    };

    let result = match local_fetch::execute_local_fetch(&plan_json, providers, options) {
        Ok(result) => result,
        Err(err) => return map_local_fetch_error(err),
    };

    let payload = result.outcome.assemble_payload();
    let report_json = local_fetch_result_to_json(&result);

    let payload_code = unsafe { write_bytes(out_payload_ptr, out_payload_len, &payload) }
        .map_or_else(|err| err, |_| 0);
    if payload_code != 0 {
        return payload_code;
    }

    write_json_value(out_report_ptr, out_report_len, &report_json)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_da_proof_summary(
    manifest_ptr: *const c_uchar,
    manifest_len: c_ulong,
    payload_ptr: *const c_uchar,
    payload_len: c_ulong,
    sample_count: c_ulong,
    sample_seed: u64,
    leaf_indexes_ptr: *const c_ulong,
    leaf_indexes_len: c_ulong,
    out_json_ptr: *mut *mut c_uchar,
    out_json_len: *mut c_ulong,
) -> c_int {
    if out_json_ptr.is_null() || out_json_len.is_null() {
        return ERR_NULL_PTR;
    }
    if manifest_ptr.is_null() || payload_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if manifest_len == 0 || payload_len == 0 {
        return ERR_DA_PROOF_SUMMARY;
    }

    let manifest_bytes = unsafe { slice::from_raw_parts(manifest_ptr, manifest_len as usize) };
    let payload_bytes = unsafe { slice::from_raw_parts(payload_ptr, payload_len as usize) };

    let mut explicit_indexes = Vec::new();
    if leaf_indexes_len > 0 {
        if leaf_indexes_ptr.is_null() {
            return ERR_NULL_PTR;
        }
        let raw = unsafe { slice::from_raw_parts(leaf_indexes_ptr, leaf_indexes_len as usize) };
        explicit_indexes.reserve(raw.len());
        for value in raw {
            match usize::try_from(*value) {
                Ok(idx) => explicit_indexes.push(idx),
                Err(_) => return ERR_DA_PROOF_SUMMARY,
            }
        }
    }

    let options =
        match DaProofSummaryOptions::from_raw(sample_count, sample_seed, &explicit_indexes) {
            Ok(opts) => opts,
            Err(code) => return code,
        };

    match da_proof_summary_json(manifest_bytes, payload_bytes, &options) {
        Ok(json) => write_json_value(out_json_ptr, out_json_len, &json),
        Err(code) => code,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_blake3_hash(
    payload_ptr: *const c_uchar,
    payload_len: c_ulong,
    out_digest_ptr: *mut *mut c_uchar,
    out_digest_len: *mut c_ulong,
) -> c_int {
    if out_digest_ptr.is_null() || out_digest_len.is_null() {
        return ERR_NULL_PTR;
    }
    let payload = if payload_len == 0 {
        &[]
    } else {
        if payload_ptr.is_null() {
            return ERR_NULL_PTR;
        }
        unsafe { slice::from_raw_parts(payload_ptr, payload_len as usize) }
    };
    let digest = blake3_hash(payload);
    match unsafe { write_bytes(out_digest_ptr, out_digest_len, digest.as_bytes()) } {
        Ok(()) => 0,
        Err(code) => code,
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, mem::MaybeUninit};

    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_data_model::isi::rwa::RwaInstructionBox;

    use super::*;

    struct ResetConfig(AccelerationConfig);

    impl Drop for ResetConfig {
        fn drop(&mut self) {
            ivm::set_acceleration_config(self.0);
        }
    }

    fn canonical_bytes(address: &AccountAddress) -> Vec<u8> {
        let hex = address.canonical_hex().expect("canonical hex");
        let body = hex.strip_prefix("0x").unwrap_or(hex.as_str());
        hex::decode(body).expect("canonical decode")
    }

    fn sign_and_verify_roundtrip(
        algorithm: Algorithm,
        private_key: &[u8],
        message: &[u8],
    ) -> (Vec<u8>, Vec<u8>) {
        let mut pk_ptr: *mut c_uchar = ptr::null_mut();
        let mut pk_len: c_ulong = 0;
        let rc_pk = unsafe {
            connect_norito_public_key_from_private(
                algorithm as u8,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut pk_ptr,
                &mut pk_len,
            )
        };
        assert_eq!(rc_pk, 0, "public key derivation must succeed");
        let public_key = unsafe { slice::from_raw_parts(pk_ptr, pk_len as usize).to_vec() };
        connect_norito_free(pk_ptr);

        let mut sig_ptr: *mut c_uchar = ptr::null_mut();
        let mut sig_len: c_ulong = 0;
        let rc_sig = unsafe {
            connect_norito_sign_detached(
                algorithm as u8,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                &mut sig_ptr,
                &mut sig_len,
            )
        };
        assert_eq!(rc_sig, 0, "signing must succeed");
        let signature = unsafe { slice::from_raw_parts(sig_ptr, sig_len as usize).to_vec() };
        connect_norito_free(sig_ptr);

        let mut valid: c_uchar = 0;
        let rc_verify = unsafe {
            connect_norito_verify_detached(
                algorithm as u8,
                public_key.as_ptr(),
                public_key.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature.as_ptr(),
                signature.len() as c_ulong,
                &mut valid,
            )
        };
        assert_eq!(rc_verify, 0, "verification call must succeed");
        assert_eq!(valid, 1, "signature must verify");

        (signature, public_key)
    }

    fn sample_identifier_receipt_payload() -> IdentifierResolutionReceiptPayload {
        let signatory = KeyPair::random().public_key().clone();
        let opening_payload = iroha_data_model::ram_lfe::RamLfeOutputOpeningPayload {
            program_id: "identifier_lookup_retail"
                .parse()
                .expect("valid program id"),
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            opened_output_hash: Hash::new(b"opened-output"),
            opened_at_ms: 8,
            expires_at_ms: Some(107),
        };
        let opening_signer = KeyPair::random();
        IdentifierResolutionReceiptPayload {
            policy_id: "email#retail".parse().expect("valid policy id"),
            execution: iroha_data_model::ram_lfe::RamLfeExecutionReceiptPayload {
                program_id: "identifier_lookup_retail"
                    .parse()
                    .expect("valid program id"),
                program_digest: Hash::new(b"program"),
                backend: iroha_crypto::RamLfeBackend::BfvProgrammedSha3_256V1,
                verification_mode: iroha_crypto::RamLfeVerificationMode::Signed,
                input_ciphertext_hash: Hash::new(b"input-ciphertext"),
                output_ciphertext_hash: Hash::new(b"output-ciphertext"),
                parameter_digest: Hash::new(b"parameters"),
                evaluation_key_digest: Hash::new(b"evaluation-keys"),
                output_hash: Hash::new(b"output"),
                associated_data_hash: Hash::new(b"associated-data"),
                executed_at_ms: 7,
                expires_at_ms: Some(107),
            },
            opening: iroha_data_model::ram_lfe::RamLfeOutputOpening {
                signature: SignatureOf::new(opening_signer.private_key(), &opening_payload).into(),
                payload: opening_payload,
            },
            opaque_id: iroha_data_model::account::OpaqueAccountId::from_hash(Hash::new(b"opaque")),
            receipt_hash: Hash::new(b"receipt"),
            uaid: iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(b"uaid")),
            account_id: AccountId::new(signatory),
        }
    }

    fn sample_identifier_signature_hex() -> String {
        "ab".repeat(64)
    }

    fn hex_hash(hash: Hash) -> String {
        hex::encode(&hash.as_ref()[..])
    }

    fn sample_rwa_id_literal() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities.universal"
            .to_owned()
    }

    #[test]
    fn parse_identifier_receipt_accepts_canonical_payload_attestation() {
        let payload = sample_identifier_receipt_payload();
        let receipt = parse_identifier_receipt_value(json_object([
            (
                "payload",
                json_object([
                    ("policy_id", JsonValue::from("email#retail")),
                    (
                        "execution",
                        json_object([
                            ("program_id", JsonValue::from("identifier_lookup_retail")),
                            (
                                "program_digest",
                                JsonValue::from(hex_hash(payload.execution.program_digest)),
                            ),
                            ("backend", JsonValue::from("bfv-programmed-sha3-256-v1")),
                            ("verification_mode", JsonValue::from("signed")),
                            (
                                "input_ciphertext_hash",
                                JsonValue::from(hex_hash(payload.execution.input_ciphertext_hash)),
                            ),
                            (
                                "output_ciphertext_hash",
                                JsonValue::from(hex_hash(payload.execution.output_ciphertext_hash)),
                            ),
                            (
                                "parameter_digest",
                                JsonValue::from(hex_hash(payload.execution.parameter_digest)),
                            ),
                            (
                                "evaluation_key_digest",
                                JsonValue::from(hex_hash(payload.execution.evaluation_key_digest)),
                            ),
                            (
                                "output_hash",
                                JsonValue::from(hex_hash(payload.execution.output_hash)),
                            ),
                            (
                                "associated_data_hash",
                                JsonValue::from(hex_hash(payload.execution.associated_data_hash)),
                            ),
                            (
                                "executed_at_ms",
                                JsonValue::from(payload.execution.executed_at_ms),
                            ),
                            (
                                "expires_at_ms",
                                JsonValue::from(
                                    payload.execution.expires_at_ms.expect("sample expiry"),
                                ),
                            ),
                        ]),
                    ),
                    (
                        "opening",
                        json_object([
                            (
                                "payload",
                                json_object([
                                    (
                                        "program_id",
                                        JsonValue::from(
                                            payload.opening.payload.program_id.to_string(),
                                        ),
                                    ),
                                    (
                                        "input_ciphertext_hash",
                                        JsonValue::from(hex_hash(
                                            payload.opening.payload.input_ciphertext_hash,
                                        )),
                                    ),
                                    (
                                        "output_ciphertext_hash",
                                        JsonValue::from(hex_hash(
                                            payload.opening.payload.output_ciphertext_hash,
                                        )),
                                    ),
                                    (
                                        "parameter_digest",
                                        JsonValue::from(hex_hash(
                                            payload.opening.payload.parameter_digest,
                                        )),
                                    ),
                                    (
                                        "evaluation_key_digest",
                                        JsonValue::from(hex_hash(
                                            payload.opening.payload.evaluation_key_digest,
                                        )),
                                    ),
                                    (
                                        "opened_output_hash",
                                        JsonValue::from(hex_hash(
                                            payload.opening.payload.opened_output_hash,
                                        )),
                                    ),
                                    (
                                        "opened_at_ms",
                                        JsonValue::from(payload.opening.payload.opened_at_ms),
                                    ),
                                    (
                                        "expires_at_ms",
                                        JsonValue::from(
                                            payload
                                                .opening
                                                .payload
                                                .expires_at_ms
                                                .expect("sample opening expiry"),
                                        ),
                                    ),
                                ]),
                            ),
                            (
                                "signature",
                                JsonValue::from(hex::encode(payload.opening.signature.payload())),
                            ),
                        ]),
                    ),
                    ("opaque_id", JsonValue::from(payload.opaque_id.to_string())),
                    (
                        "receipt_hash",
                        JsonValue::from(hex_hash(payload.receipt_hash)),
                    ),
                    ("uaid", JsonValue::from(payload.uaid.to_string())),
                    (
                        "account_id",
                        JsonValue::from(payload.account_id.to_string()),
                    ),
                ]),
            ),
            (
                "attestation",
                json_object([
                    ("kind", JsonValue::from("signed")),
                    (
                        "signature",
                        JsonValue::from(sample_identifier_signature_hex()),
                    ),
                ]),
            ),
        ]))
        .expect("parse structured torii receipt");

        assert_eq!(receipt.payload, payload);
        let RamLfeReceiptAttestation::Signed(signature) = receipt.attestation else {
            panic!("receipt attestation must be signed");
        };
        assert_eq!(
            hex::encode(signature.payload()),
            sample_identifier_signature_hex()
        );
    }

    #[test]
    fn parse_identifier_receipt_rejects_legacy_payload_hex() {
        let err = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(sample_identifier_signature_hex()),
            ),
            ("signature_payload_hex", JsonValue::from("01020304A0")),
        ]))
        .expect_err("opaque payload hex is not canonical receipt input");
        assert!(matches!(err, BridgeError::IdentifierReceipt));
    }

    #[test]
    fn parse_identifier_receipt_rejects_legacy_signature_payload() {
        let err = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(sample_identifier_signature_hex()),
            ),
            (
                "signature_payload",
                json_object([("policy_id", JsonValue::from("email#retail"))]),
            ),
        ]))
        .expect_err("missing execution payload must fail closed");
        assert!(matches!(err, BridgeError::IdentifierReceipt));
    }

    #[test]
    fn print_sample_claim_identifier_wire_payload_hex() {
        use iroha_crypto::Signature;
        use iroha_data_model::identifier::IdentifierResolutionReceipt;
        use iroha_data_model::isi::{Instruction, InstructionBox, identifier::ClaimIdentifier};

        let payload = sample_identifier_receipt_payload();
        let receipt = IdentifierResolutionReceipt {
            payload: payload.clone(),
            attestation: RamLfeReceiptAttestation::Signed(
                Signature::from_hex(sample_identifier_signature_hex())
                    .expect("valid signature hex"),
            ),
        };
        let instruction = ClaimIdentifier {
            account: payload.account_id.clone(),
            receipt,
        };
        let bare = Instruction::dyn_encode(&instruction);
        let boxed = InstructionBox::from(instruction);
        let framed = norito::core::to_bytes(&boxed).expect("serialize instruction");
        let (wire_name, framed_payload) =
            norito::decode_from_bytes::<(String, Vec<u8>)>(&framed).expect("decode wire tuple");

        println!("RUST_CLAIM_WIRE_NAME={wire_name}");
        println!("RUST_CLAIM_BARE_HEX={}", hex::encode_upper(&bare));
        println!(
            "RUST_CLAIM_FRAMED_HEX={}",
            hex::encode_upper(framed_payload)
        );
    }

    #[test]
    fn rwa_metadata_target_parses_kind_four() {
        let literal = sample_rwa_id_literal();
        let target = parse_metadata_target(4, literal.clone()).expect("parse rwa target");
        match target {
            MetadataTarget::Rwa(id) => assert_eq!(id.to_string(), literal),
            _ => panic!("expected rwa metadata target"),
        }
    }

    #[test]
    fn rwa_metadata_target_builds_set_key_value_in_rwa_instruction_box() {
        let literal = sample_rwa_id_literal();
        let target = parse_metadata_target(4, literal.clone()).expect("parse rwa target");
        let key: Name = "serial".parse().expect("valid name");
        let instruction =
            build_set_metadata_instruction(target, key.clone(), Json::from("vault-01"));
        let rwa = instruction
            .as_any()
            .downcast_ref::<RwaInstructionBox>()
            .expect("rwa instruction box");
        match rwa {
            RwaInstructionBox::SetKeyValue(inner) => {
                assert_eq!(inner.object.to_string(), literal);
                assert_eq!(inner.key, key);
                assert_eq!(inner.value, Json::from("vault-01"));
            }
            other => panic!("expected SetKeyValue variant, got {other:?}"),
        }
    }

    #[test]
    fn rwa_metadata_target_builds_remove_key_value_in_rwa_instruction_box() {
        let literal = sample_rwa_id_literal();
        let target = parse_metadata_target(4, literal.clone()).expect("parse rwa target");
        let key: Name = "serial".parse().expect("valid name");
        let instruction = build_remove_metadata_instruction(target, key.clone());
        let rwa = instruction
            .as_any()
            .downcast_ref::<RwaInstructionBox>()
            .expect("rwa instruction box");
        match rwa {
            RwaInstructionBox::RemoveKeyValue(inner) => {
                assert_eq!(inner.object.to_string(), literal);
                assert_eq!(inner.key, key);
            }
            other => panic!("expected RemoveKeyValue variant, got {other:?}"),
        }
    }

    #[test]
    fn zk_ballot_public_inputs_canonicalizes_hex() {
        let mut map = JsonMap::new();
        let root_raw = format!("0x{}", "Aa".repeat(32));
        let nullifier_raw = format!("blake2b32:{}", "BB".repeat(32));
        map.insert("root_hint".to_owned(), JsonValue::from(root_raw));
        map.insert("nullifier".to_owned(), JsonValue::from(nullifier_raw));
        let mut value = JsonValue::Object(map);
        normalize_zk_ballot_public_inputs(&mut value).expect("normalize");
        let JsonValue::Object(map) = value else {
            panic!("normalized value must remain an object");
        };
        let root_expected = "aa".repeat(32);
        let nullifier_expected = "bb".repeat(32);
        assert_eq!(
            map.get("root_hint").and_then(JsonValue::as_str),
            Some(root_expected.as_str())
        );
        assert_eq!(
            map.get("nullifier").and_then(JsonValue::as_str),
            Some(nullifier_expected.as_str())
        );
    }

    #[test]
    fn zk_ballot_public_inputs_rejects_noncanonical_owner() {
        let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
        let keypair = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
        let account = AccountId::new(keypair.public_key().clone());
        let address_hex = account.to_canonical_hex().expect("canonical hex");
        let noncanonical = format!("{address_hex}@{domain}");
        let mut map = JsonMap::new();
        map.insert("owner".to_owned(), JsonValue::from(noncanonical));
        map.insert("amount".to_owned(), JsonValue::from("10"));
        map.insert("duration_blocks".to_owned(), JsonValue::from(64u64));
        let mut value = JsonValue::Object(map);
        assert!(normalize_zk_ballot_public_inputs(&mut value).is_err());
    }

    #[test]
    fn zk_ballot_public_inputs_rejects_partial_lock_hints() {
        let mut map = JsonMap::new();
        map.insert(
            "owner".to_owned(),
            JsonValue::from("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
        );
        let mut value = JsonValue::Object(map);
        assert!(normalize_zk_ballot_public_inputs(&mut value).is_err());
    }

    #[test]
    fn zk_ballot_public_inputs_rejects_non_object() {
        let mut value = JsonValue::Array(Vec::new());
        assert!(normalize_zk_ballot_public_inputs(&mut value).is_err());
    }

    #[test]
    fn zk_ballot_public_inputs_rejects_deprecated_keys() {
        let mut map = JsonMap::new();
        map.insert("nullifier_hex".to_owned(), JsonValue::from("aa".repeat(32)));
        let mut value = JsonValue::Object(map);
        assert!(normalize_zk_ballot_public_inputs(&mut value).is_err());
    }

    #[test]
    fn zk_ballot_public_inputs_rejects_invalid_hex() {
        let mut map = JsonMap::new();
        map.insert(
            "owner".to_owned(),
            JsonValue::from("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
        );
        map.insert("amount".to_owned(), JsonValue::from("100"));
        map.insert("duration_blocks".to_owned(), JsonValue::from(64u64));
        map.insert("root_hint".to_owned(), JsonValue::from("not-hex"));
        let mut value = JsonValue::Object(map);
        assert!(normalize_zk_ballot_public_inputs(&mut value).is_err());
    }

    #[test]
    fn ffi_sign_verify_ed25519() {
        let private = vec![0x11; 32];
        let message = b"ffi-ed25519-signing";
        let (signature, public) = sign_and_verify_roundtrip(Algorithm::Ed25519, &private, message);
        assert_eq!(public.len(), 32);
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn ffi_sign_verify_secp256k1() {
        let mut private = [0u8; 32];
        private[31] = 1;
        let message = b"ffi-secp256k1-signing";
        let (signature, public) =
            sign_and_verify_roundtrip(Algorithm::Secp256k1, &private, message);
        assert!(
            public.len() == 33 || public.len() == 65,
            "unexpected secp256k1 public key length {}",
            public.len()
        );
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn ffi_sign_verify_mldsa() {
        let keypair = KeyPair::from_seed(b"ffi-mldsa-signing".to_vec(), Algorithm::MlDsa);
        let (_public_key, private_key) = keypair.into_parts();
        let (_alg, private_bytes) = private_key.to_bytes();
        let message = b"ffi-mldsa-signing";
        let (signature, public) =
            sign_and_verify_roundtrip(Algorithm::MlDsa, &private_bytes, message);
        assert!(!public.is_empty(), "ML-DSA public key must not be empty");
        assert!(!signature.is_empty(), "ML-DSA signature must not be empty");
    }

    #[test]
    fn sm2_public_key_prefixed_ffi_uses_checked_formatter() {
        let distid = "connect-sm2-prefixed";
        let private =
            Sm2PrivateKey::from_seed(distid, b"connect-sm2-prefixed-seed").expect("derive SM2 key");
        let public = private.public_key();
        let public_bytes = public.to_sec1_bytes(false);
        let distid_c = CString::new(distid).expect("distid c string");

        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sm2_public_key_prefixed(
                distid_c.as_ptr(),
                distid_c.as_bytes().len() as c_ulong,
                public_bytes.as_ptr(),
                public_bytes.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "SM2 prefixed formatting must succeed");

        let formatted = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        let formatted = String::from_utf8(formatted).expect("prefixed UTF-8");
        assert_eq!(
            formatted,
            public
                .try_to_prefixed_string()
                .expect("checked SM2 prefixed formatter")
        );
    }

    #[test]
    fn secp256k1_helpers_expose_sign_and_verify() {
        let private =
            hex::decode("e4f21b38e005d4f895a29e84948d7cc83eac79041aeb644ee4fab8d9da42f713")
                .expect("hex decode");
        let message = b"bridge-secp256k1-roundtrip";

        let mut public_out = [0u8; SECP256K1_PUBLIC_LEN];
        let rc_public = unsafe {
            connect_norito_secp256k1_public_key(
                private.as_ptr(),
                private.len() as c_ulong,
                public_out.as_mut_ptr(),
                public_out.len() as c_ulong,
            )
        };
        assert_eq!(rc_public, 0);

        let mut signature_out = [0u8; SECP256K1_SIGNATURE_LEN];
        let rc_sign = unsafe {
            connect_norito_secp256k1_sign(
                private.as_ptr(),
                private.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature_out.as_mut_ptr(),
                signature_out.len() as c_ulong,
            )
        };
        assert_eq!(rc_sign, 0);

        let rc_verify = unsafe {
            connect_norito_secp256k1_verify(
                public_out.as_ptr(),
                public_out.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                signature_out.as_ptr(),
                signature_out.len() as c_ulong,
            )
        };
        assert_eq!(rc_verify, 1);

        let mut tampered = signature_out;
        tampered[0] ^= 0xFF;
        let rc_bad = unsafe {
            connect_norito_secp256k1_verify(
                public_out.as_ptr(),
                public_out.len() as c_ulong,
                message.as_ptr(),
                message.len() as c_ulong,
                tampered.as_ptr(),
                tampered.len() as c_ulong,
            )
        };
        assert_eq!(rc_bad, 0);
    }

    #[test]
    fn connect_encrypt_envelope_accepts_framed() {
        let key = [0x11_u8; 32];
        let session_id = [0x22_u8; 32];
        let env = proto::EnvelopeV1 {
            seq: 7,
            payload: proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Close {
                who: proto::Role::App,
                code: 1,
                reason: String::from("bye"),
                retryable: false,
            }),
        };
        let env_bytes = encode_envelope_framed(&env).expect("encode envelope");
        let decoded_env = decode_envelope(&env_bytes).expect("decode envelope");
        assert_eq!(decoded_env.seq, env.seq);
        assert_eq!(decoded_env.payload, env.payload);
        let direct_frame = connect_sdk::seal_envelope(
            &key,
            &session_id,
            proto::Dir::AppToWallet,
            env.seq,
            env.payload.clone(),
        );
        let direct_frame_bytes = encode_connect_frame(&direct_frame).expect("encode sealed frame");
        let decoded_direct_frame =
            decode_connect_frame(&direct_frame_bytes).expect("decode sealed frame");
        assert_eq!(decoded_direct_frame, direct_frame);
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_connect_encrypt_envelope(
                key.as_ptr(),
                session_id.as_ptr(),
                0,
                env_bytes.as_ptr(),
                env_bytes.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, 0);
        assert!(!out_ptr.is_null());
        let frame_bytes = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);

        let mut dec_ptr: *mut c_uchar = ptr::null_mut();
        let mut dec_len: c_ulong = 0;
        let status_dec = unsafe {
            connect_norito_connect_decrypt_ciphertext(
                key.as_ptr(),
                frame_bytes.as_ptr(),
                frame_bytes.len() as c_ulong,
                &mut dec_ptr,
                &mut dec_len,
            )
        };
        assert_eq!(status_dec, 0);
        assert!(!dec_ptr.is_null());
        let decrypted = unsafe { slice::from_raw_parts(dec_ptr, dec_len as usize).to_vec() };
        connect_norito_free(dec_ptr);

        let decoded = decode_envelope(&decrypted).expect("decode envelope");
        assert_eq!(decoded.seq, env.seq);
        assert_eq!(decoded.payload, env.payload);
    }

    #[test]
    fn connect_frame_roundtrip_uses_canonical_layout() {
        let frame = proto::ConnectFrameV1 {
            sid: [0xAB; 32],
            dir: proto::Dir::AppToWallet,
            seq: 5,
            kind: proto::FrameKind::Control(proto::ConnectControlV1::Ping { nonce: 55 }),
        };
        let encoded = encode_connect_frame(&frame).expect("encode frame");
        let decoded = decode_connect_frame(&encoded).expect("decode frame");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn account_address_parse_render_via_ffi() {
        let key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let account_id = AccountId::new(key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account_id).expect("address");
        let canonical = canonical_bytes(&address);
        let i105 = address.to_i105_for_discriminant(42).expect("i105 encoding");

        let literal = CString::new(i105.clone()).expect("cstring");
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let mut prefix: u16 = 0;
        let mut err_ptr: *mut c_uchar = ptr::null_mut();
        let mut err_len: c_ulong = 0;

        let rc = unsafe {
            connect_norito_account_address_parse(
                literal.as_ptr(),
                literal.as_bytes().len() as c_ulong,
                42,
                1,
                &mut out_ptr,
                &mut out_len,
                &mut prefix,
                &mut err_ptr,
                &mut err_len,
            )
        };
        assert_eq!(rc, 0);
        assert!(err_ptr.is_null());
        assert_eq!(prefix, 42);
        let parsed_bytes = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        assert_eq!(parsed_bytes, canonical);
        connect_norito_free(out_ptr);

        let mut hex_ptr: *mut c_uchar = ptr::null_mut();
        let mut hex_len: c_ulong = 0;
        let mut i105_ptr: *mut c_uchar = ptr::null_mut();
        let mut i105_len: c_ulong = 0;
        let mut render_err_ptr: *mut c_uchar = ptr::null_mut();
        let mut render_err_len: c_ulong = 0;

        let rc_render = unsafe {
            connect_norito_account_address_render(
                canonical.as_ptr(),
                canonical.len() as c_ulong,
                42,
                &mut hex_ptr,
                &mut hex_len,
                &mut i105_ptr,
                &mut i105_len,
                &mut render_err_ptr,
                &mut render_err_len,
            )
        };
        assert_eq!(rc_render, 0);
        assert!(render_err_ptr.is_null());
        let i105_rendered = unsafe { slice::from_raw_parts(i105_ptr, i105_len as usize) };
        assert_eq!(std::str::from_utf8(i105_rendered).unwrap(), i105);

        connect_norito_free(hex_ptr);
        connect_norito_free(i105_ptr);

        let canonical_literal =
            CString::new(address.canonical_hex().expect("canonical hex")).expect("cstring");
        let mut canonical_err_ptr: *mut c_uchar = ptr::null_mut();
        let mut canonical_err_len: c_ulong = 0;
        let mut canonical_out_ptr: *mut c_uchar = ptr::null_mut();
        let mut canonical_out_len: c_ulong = 0;
        let canonical_rc = unsafe {
            connect_norito_account_address_parse(
                canonical_literal.as_ptr(),
                canonical_literal.as_bytes().len() as c_ulong,
                0,
                0,
                &mut canonical_out_ptr,
                &mut canonical_out_len,
                &mut prefix,
                &mut canonical_err_ptr,
                &mut canonical_err_len,
            )
        };
        assert_eq!(
            canonical_rc, ERR_ACCOUNT_ADDRESS,
            "canonical hex must be rejected"
        );
        assert!(canonical_out_ptr.is_null());
        let canonical_err_value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(canonical_err_ptr, canonical_err_len as usize);
            norito::json::from_slice(bytes).expect("json")
        };
        assert_eq!(
            canonical_err_value.get("code").and_then(JsonValue::as_str),
            Some("ERR_UNSUPPORTED_ADDRESS_FORMAT")
        );
        connect_norito_free(canonical_err_ptr);

        let mut invalid_chars = i105.chars().collect::<Vec<_>>();
        let last = invalid_chars.len().saturating_sub(1);
        invalid_chars[last] = '0';
        let invalid_i105 = invalid_chars.into_iter().collect::<String>();
        let invalid_literal = CString::new(invalid_i105).expect("cstring");
        let mut invalid_err_ptr: *mut c_uchar = ptr::null_mut();
        let mut invalid_err_len: c_ulong = 0;
        let mut invalid_out_ptr: *mut c_uchar = ptr::null_mut();
        let mut invalid_out_len: c_ulong = 0;
        let invalid_rc = unsafe {
            connect_norito_account_address_parse(
                invalid_literal.as_ptr(),
                invalid_literal.as_bytes().len() as c_ulong,
                42,
                1,
                &mut invalid_out_ptr,
                &mut invalid_out_len,
                &mut prefix,
                &mut invalid_err_ptr,
                &mut invalid_err_len,
            )
        };
        assert_eq!(invalid_rc, ERR_ACCOUNT_ADDRESS);
        assert!(invalid_out_ptr.is_null());
        let invalid_err_value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(invalid_err_ptr, invalid_err_len as usize);
            norito::json::from_slice(bytes).expect("json")
        };
        assert_eq!(
            invalid_err_value.get("code").and_then(JsonValue::as_str),
            Some("ERR_INVALID_I105_CHAR")
        );
        assert_eq!(
            invalid_err_value
                .get("fields")
                .and_then(JsonValue::as_object)
                .and_then(|fields| fields.get("char"))
                .and_then(JsonValue::as_str),
            Some("0")
        );
        connect_norito_free(invalid_err_ptr);

        let invalid = CString::new("").expect("empty literal");
        let mut err_out_ptr: *mut c_uchar = ptr::null_mut();
        let mut err_out_len: c_ulong = 0;
        out_ptr = ptr::null_mut();
        out_len = 0;
        let rc_err = unsafe {
            connect_norito_account_address_parse(
                invalid.as_ptr(),
                invalid.as_bytes().len() as c_ulong,
                0,
                0,
                &mut out_ptr,
                &mut out_len,
                &mut prefix,
                &mut err_out_ptr,
                &mut err_out_len,
            )
        };
        assert_eq!(rc_err, ERR_ACCOUNT_ADDRESS);
        assert!(out_ptr.is_null());
        let err_value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(err_out_ptr, err_out_len as usize);
            norito::json::from_slice(bytes).expect("json")
        };
        assert_eq!(
            err_value.get("code").and_then(JsonValue::as_str),
            Some("ERR_INVALID_LENGTH")
        );
        connect_norito_free(err_out_ptr);
    }

    #[test]
    fn acceleration_config_roundtrip() {
        let previous = ivm::acceleration_config();
        let _reset = ResetConfig(previous);

        let new_cfg = connect_norito_acceleration_config {
            enable_simd: 1,
            enable_metal: 1,
            enable_cuda: 0,
            max_gpus: 2,
            max_gpus_present: 1,
            merkle_min_leaves_gpu: 128,
            merkle_min_leaves_gpu_present: 1,
            merkle_min_leaves_metal: 64,
            merkle_min_leaves_metal_present: 1,
            merkle_min_leaves_cuda: 0,
            merkle_min_leaves_cuda_present: 0,
            prefer_cpu_sha2_max_leaves_aarch64: 0,
            prefer_cpu_sha2_max_leaves_aarch64_present: 0,
            prefer_cpu_sha2_max_leaves_x86: 256,
            prefer_cpu_sha2_max_leaves_x86_present: 1,
        };

        unsafe {
            connect_norito_set_acceleration_config(&new_cfg);
        }

        let mut out_cfg = MaybeUninit::<connect_norito_acceleration_config>::uninit();
        let rc = unsafe { connect_norito_get_acceleration_config(out_cfg.as_mut_ptr()) };
        assert_eq!(rc, 0);
        let out_cfg = unsafe { out_cfg.assume_init() };

        assert_eq!(out_cfg.enable_metal, new_cfg.enable_metal);
        assert_eq!(out_cfg.enable_cuda, new_cfg.enable_cuda);
        assert_eq!(out_cfg.enable_simd, new_cfg.enable_simd);
        assert_eq!(out_cfg.max_gpus, new_cfg.max_gpus);
        assert_eq!(out_cfg.max_gpus_present, new_cfg.max_gpus_present);
        assert_eq!(out_cfg.merkle_min_leaves_gpu, new_cfg.merkle_min_leaves_gpu);
        assert_eq!(
            out_cfg.merkle_min_leaves_gpu_present,
            new_cfg.merkle_min_leaves_gpu_present
        );
        assert_eq!(
            out_cfg.merkle_min_leaves_metal,
            new_cfg.merkle_min_leaves_metal
        );
        assert_eq!(
            out_cfg.merkle_min_leaves_metal_present,
            new_cfg.merkle_min_leaves_metal_present
        );
        assert_eq!(
            out_cfg.merkle_min_leaves_cuda,
            new_cfg.merkle_min_leaves_cuda
        );
        assert_eq!(
            out_cfg.merkle_min_leaves_cuda_present,
            new_cfg.merkle_min_leaves_cuda_present
        );
        assert_eq!(
            out_cfg.prefer_cpu_sha2_max_leaves_aarch64,
            new_cfg.prefer_cpu_sha2_max_leaves_aarch64
        );
        assert_eq!(
            out_cfg.prefer_cpu_sha2_max_leaves_aarch64_present,
            new_cfg.prefer_cpu_sha2_max_leaves_aarch64_present
        );
        assert_eq!(
            out_cfg.prefer_cpu_sha2_max_leaves_x86,
            new_cfg.prefer_cpu_sha2_max_leaves_x86
        );
        assert_eq!(
            out_cfg.prefer_cpu_sha2_max_leaves_x86_present,
            new_cfg.prefer_cpu_sha2_max_leaves_x86_present
        );

        let rc_err = unsafe { connect_norito_get_acceleration_config(std::ptr::null_mut()) };
        assert_eq!(rc_err, -1);
    }

    #[test]
    fn blake3_hash_via_ffi() {
        let payload = b"da-ingest";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_blake3_hash(
                payload.as_ptr(),
                payload.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, 0, "expected success hashing payload");
        assert_eq!(out_len as usize, blake3_hash(payload).as_bytes().len());
        let digest = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        assert_eq!(digest, blake3_hash(payload).as_bytes());
        unsafe {
            if !out_ptr.is_null() {
                free(out_ptr as *mut c_void);
            }
        }
    }
}

#[cfg(test)]
mod signed_transaction_fixture_tests {
    use std::time::Duration;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::{AccountId, address},
        transaction::TransactionBuilder,
    };
    use iroha_version::codec::EncodeVersioned as _;

    use super::decode_signed_transaction;

    // Matches account::address::DEFAULT_CHAIN_DISCRIMINANT (i105 discriminant).
    const FIXTURE_CHAIN_DISCRIMINANT: u16 = 0x02F1;

    struct ChainDiscriminantReset {
        previous: u16,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl ChainDiscriminantReset {
        fn new(discriminant: u16) -> Self {
            let guard = super::test_support::chain_discriminant_guard();
            let previous = address::set_chain_discriminant(discriminant);
            Self {
                previous,
                _guard: guard,
            }
        }
    }

    impl Drop for ChainDiscriminantReset {
        fn drop(&mut self) {
            address::set_chain_discriminant(self.previous);
        }
    }

    #[test]
    fn signed_transaction_decoder_accepts_only_versioned_bytes() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "00000004".parse().expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let versioned = tx.encode_versioned();
        decode_signed_transaction(&versioned).expect("decode versioned signed tx");
        let bytes = norito::codec::encode_adaptive(&tx);
        assert!(decode_signed_transaction(&bytes).is_err());
        let framed = norito::to_bytes(&tx).expect("encode framed signed tx");
        assert!(decode_signed_transaction(&framed).is_err());
    }

    #[test]
    fn signed_transaction_versioned_reencode_match() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "00000004".parse().expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let bytes = tx.encode_versioned();
        let signed = decode_signed_transaction(&bytes).expect("decode versioned signed tx");
        assert_eq!(signed.encode_versioned(), bytes);
    }

    #[test]
    fn generated_signed_transaction_versioned_bytes_prefix_bare_payload() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "00000004".parse().expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let versioned = tx.encode_versioned();
        let bare = norito::codec::encode_adaptive(&tx);

        assert_eq!(versioned.first().copied(), Some(1));
        assert_eq!(&versioned[1..], bare.as_slice());
    }
}

#[cfg(test)]
mod da_proof_summary_tests {
    use iroha_data_model::{
        da::{
            manifest::{ChunkCommitment, ChunkRole},
            types::{
                BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                ExtraMetadata, GovernanceTag, MetadataEntry, MetadataVisibility, RetentionPolicy,
                StorageTicketId,
            },
        },
        nexus::LaneId,
        sorafs::pin_registry::StorageClass,
    };
    use sorafs_car::ChunkStore;

    use super::*;

    #[test]
    fn da_proof_summary_via_ffi() {
        let (manifest_bytes, payload) = sample_manifest_bytes();
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let status = unsafe {
            connect_norito_da_proof_summary(
                manifest_bytes.as_ptr(),
                manifest_bytes.len() as c_ulong,
                payload.as_ptr(),
                payload.len() as c_ulong,
                2,
                0,
                ptr::null(),
                0,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(status, 0, "da proof summary call failed");
        assert!(!out_ptr.is_null());
        let summary_bytes = unsafe { slice::from_raw_parts(out_ptr, out_len as usize).to_vec() };
        connect_norito_free(out_ptr);
        let value: JsonValue = norito::json::from_slice(&summary_bytes).expect("json summary");
        assert!(value.get("proofs").is_some(), "missing proofs array");
        assert!(
            value.get("blob_hash_hex").is_some(),
            "missing blob hash field"
        );
    }

    fn sample_manifest_bytes() -> (Vec<u8>, Vec<u8>) {
        let payload: Vec<u8> = (0..64).map(|idx| idx as u8).collect();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&payload);
        let data_shards = 2usize;
        let chunk_commitments = store
            .chunks()
            .iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let stripe_id = u32::try_from(idx / data_shards).unwrap_or(u32::MAX);
                ChunkCommitment::new_with_role(
                    idx as u32,
                    chunk.offset,
                    chunk.length,
                    ChunkDigest::new(chunk.blake3),
                    ChunkRole::Data,
                    stripe_id,
                )
            })
            .collect::<Vec<_>>();
        let chunk_size = chunk_commitments
            .first()
            .map(|commitment| commitment.length)
            .unwrap_or(payload.len() as u32);
        let metadata = ExtraMetadata {
            items: vec![
                MetadataEntry::new(
                    "taikai.event_id",
                    b"demo-event".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.stream_id",
                    b"primary-stream".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.rendition_id",
                    b"main-1080p".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.segment.sequence",
                    b"42".to_vec(),
                    MetadataVisibility::Public,
                ),
            ],
        };
        let chunk_root = BlobDigest::new(*store.por_tree().root());
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(7),
            epoch: 1,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new(String::from("custom.binary")),
            blob_hash: BlobDigest::new(*store.payload_digest().as_bytes()),
            chunk_root,
            storage_ticket: StorageTicketId::new([0x44; 32]),
            total_size: payload.len() as u64,
            chunk_size,
            total_stripes: chunk_commitments.len().div_ceil(2).try_into().unwrap_or(0),
            shards_per_stripe: 3,
            erasure_profile: ErasureProfile {
                data_shards: 2,
                parity_shards: 1,
                row_parity_stripes: 0,
                chunk_alignment: 1,
                fec_scheme: iroha_data_model::da::types::FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 10,
                cold_retention_secs: 20,
                required_replicas: 3,
                storage_class: StorageClass::Warm,
                governance_tag: GovernanceTag::new(String::from("da.test")),
            },
            rent_quote: DaRentQuote::default(),
            chunks: chunk_commitments,
            ipa_commitment: chunk_root,
            metadata,
            issued_at_unix: 123,
        };
        let manifest_bytes = norito::to_bytes(&manifest).expect("manifest encode");
        (manifest_bytes, payload)
    }
}

#[cfg(test)]
mod sorafs_tests {
    use std::{ffi::CString, fs, ptr, slice};

    use sorafs_car::{CarBuildPlan, fetch_plan::chunk_fetch_specs_to_string};
    use sorafs_chunker::ChunkProfile;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn sorafs_local_fetch_via_ffi() {
        let tempdir = tempdir().expect("tempdir");
        let payload: Vec<u8> = (0..(4 * 1024_usize))
            .map(|idx| u8::try_from(idx % 251).expect("within u8"))
            .collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("plan json render");

        let alpha_path = tempdir.path().join("alpha.bin");
        fs::write(&alpha_path, &payload).expect("write payload");

        let mut provider = JsonMap::new();
        provider.insert("name".into(), JsonValue::from("alpha"));
        provider.insert(
            "path".into(),
            JsonValue::from(alpha_path.display().to_string()),
        );
        provider.insert("max_concurrent".into(), JsonValue::from(2u64));
        provider.insert("weight".into(), JsonValue::from(1u64));

        let providers_json =
            norito::json::to_string(&JsonValue::Array(vec![JsonValue::Object(provider)]))
                .expect("providers json render");

        let plan_c = CString::new(plan_json).expect("plan cstring");
        let providers_c = CString::new(providers_json).expect("providers cstring");
        let options_c = CString::new("{}").expect("options cstring");

        let mut out_payload_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_payload_len: c_ulong = 0;
        let mut out_report_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_report_len: c_ulong = 0;

        let rc = unsafe {
            connect_norito_sorafs_local_fetch(
                plan_c.as_ptr(),
                plan_c.as_bytes().len() as c_ulong,
                providers_c.as_ptr(),
                providers_c.as_bytes().len() as c_ulong,
                options_c.as_ptr(),
                options_c.as_bytes().len() as c_ulong,
                &mut out_payload_ptr,
                &mut out_payload_len,
                &mut out_report_ptr,
                &mut out_report_len,
            )
        };
        assert_eq!(rc, 0, "ffi call should succeed");

        let assembled = unsafe {
            let bytes = slice::from_raw_parts(out_payload_ptr, out_payload_len as usize);
            bytes.to_vec()
        };
        assert_eq!(assembled, payload, "payload must match input bytes");

        let report_value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(out_report_ptr, out_report_len as usize);
            norito::json::from_slice(bytes).expect("report json")
        };

        let chunk_count = report_value
            .get("chunk_count")
            .and_then(JsonValue::as_u64)
            .expect("chunk_count present");
        assert_eq!(
            chunk_count as usize,
            plan.chunk_fetch_specs().len(),
            "chunk count matches plan"
        );

        let reports = report_value
            .get("provider_reports")
            .and_then(JsonValue::as_array)
            .expect("provider reports");
        assert_eq!(reports.len(), 1);
        let report = reports[0].as_object().expect("report object");
        assert_eq!(
            report
                .get("provider")
                .and_then(JsonValue::as_str)
                .expect("provider name"),
            "alpha"
        );
        assert_eq!(
            report
                .get("failures")
                .and_then(JsonValue::as_u64)
                .expect("failures"),
            0
        );

        let receipts = report_value
            .get("chunk_receipts")
            .and_then(JsonValue::as_array)
            .expect("chunk receipts");
        assert_eq!(receipts.len(), plan.chunk_fetch_specs().len());
        assert!(receipts.iter().all(|entry| {
            entry
                .get("provider")
                .and_then(JsonValue::as_str)
                .map(|name| name == "alpha")
                .unwrap_or(false)
        }));

        assert!(
            report_value
                .get("scoreboard")
                .map(JsonValue::is_null)
                .unwrap_or(false),
            "scoreboard should be null when not requested"
        );

        if !out_payload_ptr.is_null() {
            connect_norito_free(out_payload_ptr);
        }
        if !out_report_ptr.is_null() {
            connect_norito_free(out_report_ptr);
        }
    }
}

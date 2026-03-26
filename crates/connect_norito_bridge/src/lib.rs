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
    sync::OnceLock,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose as b64gp};
use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use blake3::hash as blake3_hash;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use iroha_crypto::{
    Algorithm, EcdsaSecp256k1Sha256, Error as CryptoError, Hash, KeyGenOption, KeyPair, PrivateKey,
    PublicKey, Signature,
    kex::KeyExchangeScheme,
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature},
};
use iroha_data_model::{
    ChainId,
    account::{
        AccountId, ScopedAccountId,
        address::{AccountAddress, AccountAddressError},
    },
    asset::id::{AssetDefinitionId, AssetId},
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
    name::Name,
    offline::{
        OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN, OFFLINE_FASTPQ_HKDF_DOMAIN,
        OFFLINE_FASTPQ_PROOF_VERSION_V1, OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN,
        OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN, OFFLINE_FASTPQ_SUM_NONCE_DOMAIN,
        OFFLINE_FASTPQ_SUM_PROOF_DOMAIN, OfflineBuildClaim, OfflineBuildClaimPlatform,
        OfflineFastpqCounterProof, OfflineFastpqReplayProof, OfflineFastpqSumProof,
        OfflinePlatformProof, OfflineProofBlindingSeed, OfflineProofRequestCounter,
        OfflineProofRequestReplay, OfflineProofRequestSum, OfflineReceiptChallengePreimage,
        OfflineSpendReceipt, OfflineSpendReceiptPayload, PoseidonDigest, chain_bound_receipt_hash,
        compute_receipts_root,
    },
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId},
    rwa::RwaId,
    smart_contract::manifest::ContractManifest,
    transaction::{
        Executable, SignedTransaction, TransactionSubmissionReceipt, signed::TransactionBuilder,
    },
};
use iroha_executor_data_model::isi::multisig::{MultisigRegister, MultisigSpec};
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_torii_shared::{connect as proto, connect_sdk};
use ivm::{AccelerationConfig, BackendRuntimeStatus};
use libc::{c_char, c_int, c_uchar, c_ulong, c_ulonglong, free, malloc};
use norito::json::{Map as JsonMap, Value as JsonValue};
use norito::{codec::DecodeAll, core::DecodeFromSlice};
use norito::{decode_from_bytes, to_bytes};
use rand::{RngCore, rng};
use sha2::{Digest, Sha256, Sha512};
use sorafs_car::{
    ChunkStore, ChunkStoreError, InMemoryPayload, PorProof, build_plan_from_da_manifest,
    local_fetch::{
        self, LocalFetchError, LocalFetchOptions, LocalFetchResult, LocalProviderInput,
        ProviderMetadataInput, RangeCapabilityInput, StreamBudgetInput, TelemetryEntryInput,
        TransportHintInput,
    },
};
use zeroize::Zeroize;

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
const ERR_OFFLINE_RECEIVER: c_int = -300;
const ERR_OFFLINE_ASSET: c_int = -301;
const ERR_OFFLINE_NONCE: c_int = -303;
const ERR_OFFLINE_SERIALIZE: c_int = -304;
const ERR_OFFLINE_COMMITMENT: c_int = -305;
const ERR_OFFLINE_BLINDING: c_int = -306;
const ERR_DA_PROOF_SUMMARY: c_int = -401;
const ERR_MULTISIG_SPEC: c_int = -402;
const ERR_VERIFYING_KEY_ID: c_int = -403;
const ERR_ZK_ASSET_MODE: c_int = -404;
const ERR_CONNECT_ENCODE: c_int = -405;
const ERR_IDENTIFIER_RECEIPT: c_int = -406;

const OFFLINE_BALANCE_PROOF_VERSION: u8 = 1;
const OFFLINE_DELTA_PROOF_BYTES: usize = 96;
const OFFLINE_RANGE_PROOF_BITS: usize = 64;
const OFFLINE_RANGE_PROOF_PER_BIT_BYTES: usize = 192;
const OFFLINE_RANGE_PROOF_BYTES: usize =
    OFFLINE_RANGE_PROOF_BITS * OFFLINE_RANGE_PROOF_PER_BIT_BYTES;
const OFFLINE_BALANCE_PROOF_BYTES: usize =
    1 + OFFLINE_DELTA_PROOF_BYTES + OFFLINE_RANGE_PROOF_BYTES;
const OFFLINE_PROOF_TRANSCRIPT_LABEL: &[u8] = b"iroha.offline.balance.v1";
const OFFLINE_RANGE_PROOF_TRANSCRIPT_LABEL: &[u8] = b"iroha.offline.balance.range.v1";
const OFFLINE_GENERATOR_LABEL: &[u8] = b"iroha.offline.balance.generator.H.v1";

static PEDERSEN_H: OnceLock<RistrettoPoint> = OnceLock::new();

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
    OfflineReceiver,
    OfflineAsset,
    OfflineNonce,
    OfflineSerialize,
    OfflineCommitment,
    OfflineBlinding,
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
            BridgeError::OfflineReceiver => ERR_OFFLINE_RECEIVER,
            BridgeError::OfflineAsset => ERR_OFFLINE_ASSET,
            BridgeError::OfflineNonce => ERR_OFFLINE_NONCE,
            BridgeError::OfflineSerialize => ERR_OFFLINE_SERIALIZE,
            BridgeError::OfflineCommitment => ERR_OFFLINE_COMMITMENT,
            BridgeError::OfflineBlinding => ERR_OFFLINE_BLINDING,
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

fn parse_scoped_account_id(value: String) -> BridgeResult<ScopedAccountId> {
    ScopedAccountId::from_str(&value).map_err(|_| BridgeError::Authority)
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
        0 => object
            .parse::<DomainId>()
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

fn pedersen_generator_h() -> &'static RistrettoPoint {
    PEDERSEN_H.get_or_init(|| RistrettoPoint::hash_from_bytes::<Sha512>(OFFLINE_GENERATOR_LABEL))
}

fn decode_commitment_point(bytes: &[u8]) -> BridgeResult<RistrettoPoint> {
    if bytes.len() != 32 {
        return Err(BridgeError::OfflineCommitment);
    }
    let compressed =
        CompressedRistretto::from_slice(bytes).map_err(|_| BridgeError::OfflineCommitment)?;
    compressed
        .decompress()
        .ok_or(BridgeError::OfflineCommitment)
}

fn decode_scalar_bytes(bytes: &[u8]) -> BridgeResult<Scalar> {
    if bytes.len() != 32 {
        return Err(BridgeError::OfflineBlinding);
    }
    let mut array: [u8; 32] = bytes.try_into().map_err(|_| BridgeError::OfflineBlinding)?;
    let scalar = Scalar::from_bytes_mod_order(array);
    array.zeroize();
    Ok(scalar)
}

fn numeric_to_scalar(value: &Numeric) -> BridgeResult<Scalar> {
    let mantissa = value.try_mantissa_u128().ok_or(BridgeError::Quantity)?;
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&mantissa.to_le_bytes());
    Ok(Scalar::from_bytes_mod_order(bytes))
}

fn numeric_to_le_bytes(value: &Numeric) -> BridgeResult<[u8; 16]> {
    let mantissa = value.try_mantissa_u128().ok_or(BridgeError::Quantity)?;
    let signed = i128::try_from(mantissa).map_err(|_| BridgeError::Quantity)?;
    Ok(signed.to_le_bytes())
}

fn numeric_to_u64(value: &Numeric) -> BridgeResult<u64> {
    let mantissa = value.try_mantissa_u128().ok_or(BridgeError::Quantity)?;
    u64::try_from(mantissa).map_err(|_| BridgeError::Quantity)
}

fn transcript_context(chain_id: &ChainId) -> [u8; 32] {
    Hash::new(chain_id.as_str().as_bytes()).into()
}

fn transcript_challenge(
    c_init: &RistrettoPoint,
    c_res: &RistrettoPoint,
    delta_bytes: &[u8; 16],
    context: &[u8; 32],
    u: &RistrettoPoint,
    r_point: &RistrettoPoint,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_PROOF_TRANSCRIPT_LABEL);
    hasher.update(c_init.compress().as_bytes());
    hasher.update(c_res.compress().as_bytes());
    hasher.update(delta_bytes);
    hasher.update(context);
    hasher.update(u.compress().as_bytes());
    hasher.update(r_point.compress().as_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    let scalar = Scalar::from_bytes_mod_order_wide(&output);
    output.zeroize();
    scalar
}

fn range_proof_challenge(
    context: &[u8; 32],
    bit_index: u8,
    commitment: &RistrettoPoint,
    a0: &RistrettoPoint,
    a1: &RistrettoPoint,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_RANGE_PROOF_TRANSCRIPT_LABEL);
    hasher.update(context);
    hasher.update(&[bit_index]);
    hasher.update(commitment.compress().as_bytes());
    hasher.update(a0.compress().as_bytes());
    hasher.update(a1.compress().as_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    let scalar = Scalar::from_bytes_mod_order_wide(&output);
    output.zeroize();
    scalar
}

fn random_scalar(rng: &mut impl RngCore) -> Scalar {
    let mut bytes = [0u8; 64];
    rng.fill_bytes(&mut bytes);
    let scalar = Scalar::from_bytes_mod_order_wide(&bytes);
    bytes.zeroize();
    scalar
}

fn generate_range_proof(
    chain_id: &ChainId,
    value: u64,
    resulting_blinding: &Scalar,
) -> BridgeResult<Vec<u8>> {
    let context = transcript_context(chain_id);
    let mut rng = rng();
    let mut blindings = Vec::with_capacity(OFFLINE_RANGE_PROOF_BITS);
    let mut sum = Scalar::ZERO;
    for bit_index in 0..(OFFLINE_RANGE_PROOF_BITS - 1) {
        let blinding = random_scalar(&mut rng);
        sum += Scalar::from(1u64 << bit_index) * blinding;
        blindings.push(blinding);
    }
    let last_weight = Scalar::from(1u64 << (OFFLINE_RANGE_PROOF_BITS - 1));
    let last_blinding = (resulting_blinding - sum) * last_weight.invert();
    blindings.push(last_blinding);

    let mut proof = Vec::with_capacity(OFFLINE_RANGE_PROOF_BYTES);
    for (bit_index, blinding) in blindings.iter().enumerate() {
        let bit = ((value >> bit_index) & 1) == 1;
        let bit_scalar = Scalar::from(u64::from(bit));
        let commitment = RISTRETTO_BASEPOINT_POINT * bit_scalar + pedersen_generator_h() * blinding;
        let (a0, a1, mut e0, mut s0, mut s1) = if bit {
            let mut alpha = random_scalar(&mut rng);
            let e0 = random_scalar(&mut rng);
            let s0 = random_scalar(&mut rng);
            let a0 = pedersen_generator_h() * s0 - commitment * e0;
            let a1 = pedersen_generator_h() * alpha;
            let challenge = range_proof_challenge(&context, bit_index as u8, &commitment, &a0, &a1);
            let mut e1 = challenge - e0;
            let s1 = alpha + e1 * blinding;
            alpha.zeroize();
            e1.zeroize();
            (a0, a1, e0, s0, s1)
        } else {
            let mut alpha = random_scalar(&mut rng);
            let mut e1 = random_scalar(&mut rng);
            let s1 = random_scalar(&mut rng);
            let a0 = pedersen_generator_h() * alpha;
            let commitment_minus_g = commitment - RISTRETTO_BASEPOINT_POINT;
            let a1 = pedersen_generator_h() * s1 - commitment_minus_g * e1;
            let challenge = range_proof_challenge(&context, bit_index as u8, &commitment, &a0, &a1);
            let e0 = challenge - e1;
            let s0 = alpha + e0 * blinding;
            alpha.zeroize();
            e1.zeroize();
            (a0, a1, e0, s0, s1)
        };
        proof.extend_from_slice(commitment.compress().as_bytes());
        proof.extend_from_slice(a0.compress().as_bytes());
        proof.extend_from_slice(a1.compress().as_bytes());
        proof.extend_from_slice(e0.to_bytes().as_ref());
        proof.extend_from_slice(s0.to_bytes().as_ref());
        proof.extend_from_slice(s1.to_bytes().as_ref());
        e0.zeroize();
        s0.zeroize();
        s1.zeroize();
    }
    for blinding in &mut blindings {
        blinding.zeroize();
    }
    sum.zeroize();
    Ok(proof)
}

fn generate_offline_balance_proof(
    chain_id: ChainId,
    claimed_delta: &Numeric,
    resulting_value: &Numeric,
    initial_commitment: &[u8],
    resulting_commitment: &[u8],
    initial_blinding: &[u8],
    resulting_blinding: &[u8],
) -> BridgeResult<Vec<u8>> {
    if claimed_delta.scale() != resulting_value.scale() {
        return Err(BridgeError::Quantity);
    }
    let c_init = decode_commitment_point(initial_commitment)?;
    let c_res = decode_commitment_point(resulting_commitment)?;
    let mut delta_scalar = numeric_to_scalar(claimed_delta)?;
    let mut delta_bytes = numeric_to_le_bytes(claimed_delta)?;
    let resulting_value_u64 = numeric_to_u64(resulting_value)?;
    let mut blind_init = decode_scalar_bytes(initial_blinding)?;
    let mut blind_res = decode_scalar_bytes(resulting_blinding)?;
    let mut blind_delta = blind_res - blind_init;
    let context = transcript_context(&chain_id);
    let u = c_res - c_init;
    let mut rng = rng();
    let mut alpha = random_scalar(&mut rng);
    let mut beta = random_scalar(&mut rng);
    let r_point = RISTRETTO_BASEPOINT_POINT * alpha + pedersen_generator_h() * beta;
    let challenge = transcript_challenge(&c_init, &c_res, &delta_bytes, &context, &u, &r_point);
    let mut s_g = alpha + challenge * delta_scalar;
    let mut s_h = beta + challenge * blind_delta;

    let expected_commitment = RISTRETTO_BASEPOINT_POINT * Scalar::from(resulting_value_u64)
        + pedersen_generator_h() * blind_res;
    if expected_commitment != c_res {
        alpha.zeroize();
        beta.zeroize();
        s_g.zeroize();
        s_h.zeroize();
        blind_init.zeroize();
        blind_res.zeroize();
        blind_delta.zeroize();
        delta_scalar.zeroize();
        delta_bytes.zeroize();
        return Err(BridgeError::OfflineCommitment);
    }

    let mut proof = Vec::with_capacity(OFFLINE_BALANCE_PROOF_BYTES);
    proof.push(OFFLINE_BALANCE_PROOF_VERSION);
    proof.extend_from_slice(r_point.compress().as_bytes());
    proof.extend_from_slice(s_g.to_bytes().as_ref());
    proof.extend_from_slice(s_h.to_bytes().as_ref());
    let range_proof = generate_range_proof(&chain_id, resulting_value_u64, &blind_res)?;
    proof.extend_from_slice(&range_proof);
    alpha.zeroize();
    beta.zeroize();
    s_g.zeroize();
    s_h.zeroize();
    blind_init.zeroize();
    blind_res.zeroize();
    blind_delta.zeroize();
    delta_scalar.zeroize();
    delta_bytes.zeroize();
    Ok(proof)
}

fn update_offline_commitment(
    initial_commitment: &[u8],
    claimed_delta: &Numeric,
    initial_blinding: &[u8],
    resulting_blinding: &[u8],
) -> BridgeResult<[u8; 32]> {
    let c_init = decode_commitment_point(initial_commitment)?;
    let mut delta_scalar = numeric_to_scalar(claimed_delta)?;
    let mut blind_init = decode_scalar_bytes(initial_blinding)?;
    let mut blind_res = decode_scalar_bytes(resulting_blinding)?;
    let mut blind_delta = blind_res - blind_init;
    let updated =
        c_init + RISTRETTO_BASEPOINT_POINT * delta_scalar + pedersen_generator_h() * blind_delta;
    let compressed = updated.compress();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(compressed.as_bytes());
    delta_scalar.zeroize();
    blind_init.zeroize();
    blind_res.zeroize();
    blind_delta.zeroize();
    Ok(bytes)
}

fn derive_offline_blinding_from_seed(
    initial_blinding: &[u8],
    certificate_id: Hash,
    counter: u64,
) -> BridgeResult<[u8; 32]> {
    let mut initial = decode_scalar_bytes(initial_blinding)?;
    let seed = OfflineProofBlindingSeed::derive(certificate_id, counter);
    let mut delta = blinding_scalar_from_seed(&seed);
    let resulting = initial + delta;
    let bytes = resulting.to_bytes();
    initial.zeroize();
    delta.zeroize();
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn compute_offline_receipt_challenge(
    chain_id_raw: String,
    invoice_id: String,
    receiver_raw: String,
    asset_raw: String,
    amount_raw: String,
    issued_at_ms: u64,
    sender_certificate_id_raw: String,
    nonce_raw: String,
) -> BridgeResult<(Vec<u8>, [u8; Hash::LENGTH], [u8; 32])> {
    if chain_id_raw.trim().is_empty() {
        return Err(BridgeError::ChainId);
    }
    let chain_id = ChainId::from_str(&chain_id_raw).map_err(|_| BridgeError::ChainId)?;
    let receiver = AccountId::parse_encoded(&receiver_raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| BridgeError::OfflineReceiver)?;
    let asset = AssetId::parse_literal(&asset_raw).map_err(|_| BridgeError::OfflineAsset)?;
    let amount = Numeric::from_str(&amount_raw).map_err(|_| BridgeError::Quantity)?;
    let sender_certificate_id =
        Hash::from_str(&sender_certificate_id_raw).map_err(|_| BridgeError::OfflineNonce)?;
    let nonce = Hash::from_str(&nonce_raw).map_err(|_| BridgeError::OfflineNonce)?;
    let preimage = OfflineReceiptChallengePreimage {
        invoice_id,
        receiver,
        asset,
        amount,
        issued_at_ms,
        sender_certificate_id,
        nonce,
    };
    let bytes = to_bytes(&preimage).map_err(|_| BridgeError::OfflineSerialize)?;
    let iroha_hash = chain_bound_receipt_hash(&chain_id, &bytes);
    let mut iroha_bytes = [0u8; Hash::LENGTH];
    iroha_bytes.copy_from_slice(iroha_hash.as_ref());
    let client_hash: [u8; 32] = Sha256::digest(iroha_hash.as_ref()).into();
    Ok((bytes, iroha_bytes, client_hash))
}

#[allow(clippy::too_many_arguments)]
fn encode_offline_spend_receipt_payload(
    tx_id_hex: String,
    from_raw: String,
    to_raw: String,
    asset_raw: String,
    amount_raw: String,
    issued_at_ms: u64,
    invoice_id: String,
    platform_proof_json: String,
    sender_certificate_id_hex: String,
) -> BridgeResult<Vec<u8>> {
    let tx_id = Hash::from_str(&tx_id_hex).map_err(|_| BridgeError::OfflineNonce)?;
    let from = AccountId::parse_encoded(&from_raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| BridgeError::Authority)?;
    let to = AccountId::parse_encoded(&to_raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| BridgeError::Destination)?;
    let asset = AssetId::parse_literal(&asset_raw).map_err(|_| BridgeError::OfflineAsset)?;
    let amount = Numeric::from_str(&amount_raw).map_err(|_| BridgeError::Quantity)?;
    let platform_proof: OfflinePlatformProof =
        norito::json::from_str(&platform_proof_json).map_err(|_| BridgeError::OfflineSerialize)?;
    let sender_certificate_id =
        Hash::from_str(&sender_certificate_id_hex).map_err(|_| BridgeError::OfflineNonce)?;

    let payload = OfflineSpendReceiptPayload {
        tx_id,
        from,
        to,
        asset,
        amount,
        issued_at_ms,
        invoice_id,
        platform_proof,
        sender_certificate_id,
    };

    to_bytes(&payload).map_err(|_| BridgeError::OfflineSerialize)
}

#[allow(clippy::too_many_arguments)]
fn encode_offline_build_claim_payload(
    claim_id_hex: String,
    platform: String,
    app_id: String,
    build_number: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
    lineage_scope: String,
    nonce_hex: String,
) -> BridgeResult<Vec<u8>> {
    let claim_id = Hash::from_str(&claim_id_hex).map_err(|_| BridgeError::OfflineNonce)?;
    let nonce = Hash::from_str(&nonce_hex).map_err(|_| BridgeError::OfflineNonce)?;
    let platform = match platform.as_str() {
        "Android" => OfflineBuildClaimPlatform::Android,
        "Apple" => OfflineBuildClaimPlatform::Apple,
        _ => return Err(BridgeError::OfflineSerialize),
    };

    let claim = OfflineBuildClaim {
        claim_id,
        platform,
        app_id,
        build_number,
        issued_at_ms,
        expires_at_ms,
        lineage_scope,
        nonce,
        operator_signature: Signature::from_bytes(&[0; 64]),
    };

    claim
        .signing_bytes()
        .map_err(|_| BridgeError::OfflineSerialize)
}

fn aggregate_receipt_amounts(amounts: &[Numeric]) -> BridgeResult<Numeric> {
    if amounts.is_empty() {
        return Err(BridgeError::Quantity);
    }
    let expected_scale = amounts[0].scale();
    let mut total = Numeric::zero();
    for amount in amounts {
        if amount.scale() != expected_scale {
            return Err(BridgeError::Quantity);
        }
        total = total
            .checked_add(amount.clone())
            .ok_or(BridgeError::Quantity)?;
    }
    Ok(total)
}

fn blinding_scalar_from_seed(seed: &OfflineProofBlindingSeed) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_FASTPQ_HKDF_DOMAIN);
    hasher.update(seed.hkdf_salt.as_ref());
    hasher.update(&seed.counter.to_be_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    let scalar = Scalar::from_bytes_mod_order_wide(&output);
    output.zeroize();
    scalar
}

fn sum_proof_nonce(
    bundle_id: &Hash,
    certificate_id: &Hash,
    receipts_root: &PoseidonDigest,
    delta_le: &[u8; 16],
    blind_sum: &Scalar,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_FASTPQ_SUM_NONCE_DOMAIN);
    hasher.update(bundle_id.as_ref());
    hasher.update(certificate_id.as_ref());
    hasher.update(receipts_root.as_bytes());
    hasher.update(delta_le);
    hasher.update(&blind_sum.to_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    let scalar = Scalar::from_bytes_mod_order_wide(&output);
    output.zeroize();
    scalar
}

fn sum_proof_challenge(
    bundle_id: &Hash,
    certificate_id: &Hash,
    receipts_root: &PoseidonDigest,
    c_init: &RistrettoPoint,
    c_res: &RistrettoPoint,
    delta_le: &[u8; 16],
    r_point: &RistrettoPoint,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_FASTPQ_SUM_PROOF_DOMAIN);
    hasher.update(bundle_id.as_ref());
    hasher.update(certificate_id.as_ref());
    hasher.update(receipts_root.as_bytes());
    hasher.update(c_init.compress().as_bytes());
    hasher.update(c_res.compress().as_bytes());
    hasher.update(delta_le);
    hasher.update(r_point.compress().as_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    let scalar = Scalar::from_bytes_mod_order_wide(&output);
    output.zeroize();
    scalar
}

fn replay_chain(head: Hash, tx_ids: &[Hash]) -> Hash {
    let mut current = head;
    for tx_id in tx_ids {
        let mut buf =
            Vec::with_capacity(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN.len() + Hash::LENGTH * 2);
        buf.extend_from_slice(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN);
        buf.extend_from_slice(current.as_ref());
        buf.extend_from_slice(tx_id.as_ref());
        current = Hash::new(buf);
    }
    current
}

fn counter_digest(
    bundle_id: &Hash,
    receipts_root: &PoseidonDigest,
    checkpoint: u64,
    counters: &[u64],
) -> Hash {
    let mut buf = Vec::with_capacity(
        OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN.len()
            + Hash::LENGTH
            + Hash::LENGTH
            + 8
            + counters.len() * 8,
    );
    buf.extend_from_slice(OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN);
    buf.extend_from_slice(bundle_id.as_ref());
    buf.extend_from_slice(receipts_root.as_bytes());
    buf.extend_from_slice(&checkpoint.to_be_bytes());
    for counter in counters {
        buf.extend_from_slice(&counter.to_be_bytes());
    }
    Hash::new(buf)
}

fn replay_digest(
    bundle_id: &Hash,
    receipts_root: &PoseidonDigest,
    head: &Hash,
    tail: &Hash,
    tx_ids: &[Hash],
) -> Hash {
    let mut buf = Vec::with_capacity(
        OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN.len() + Hash::LENGTH * 3 + Hash::LENGTH * tx_ids.len(),
    );
    buf.extend_from_slice(OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN);
    buf.extend_from_slice(bundle_id.as_ref());
    buf.extend_from_slice(receipts_root.as_bytes());
    buf.extend_from_slice(head.as_ref());
    buf.extend_from_slice(tail.as_ref());
    for tx_id in tx_ids {
        buf.extend_from_slice(tx_id.as_ref());
    }
    Hash::new(buf)
}

fn generate_offline_fastpq_sum_proof(
    request: &OfflineProofRequestSum,
) -> BridgeResult<OfflineFastpqSumProof> {
    if request.receipt_amounts.len() != request.blinding_seeds.len() {
        return Err(BridgeError::OfflineSerialize);
    }
    let total = aggregate_receipt_amounts(&request.receipt_amounts)?;
    if total != request.claimed_delta {
        return Err(BridgeError::Quantity);
    }
    let c_init = decode_commitment_point(&request.initial_commitment.commitment)?;
    let c_res = decode_commitment_point(&request.resulting_commitment)?;
    let delta_scalar = numeric_to_scalar(&total)?;
    let delta_le = numeric_to_le_bytes(&total)?;
    let mut blind_sum = request
        .blinding_seeds
        .iter()
        .map(blinding_scalar_from_seed)
        .fold(Scalar::ZERO, |acc, scalar| acc + scalar);
    let expected_delta =
        RISTRETTO_BASEPOINT_POINT * delta_scalar + pedersen_generator_h() * blind_sum;
    let commitment_delta = c_res - c_init;
    if commitment_delta != expected_delta {
        blind_sum.zeroize();
        return Err(BridgeError::OfflineCommitment);
    }
    let nonce = sum_proof_nonce(
        &request.header.bundle_id,
        &request.header.certificate_id,
        &request.header.receipts_root,
        &delta_le,
        &blind_sum,
    );
    let r_point = pedersen_generator_h() * nonce;
    let challenge = sum_proof_challenge(
        &request.header.bundle_id,
        &request.header.certificate_id,
        &request.header.receipts_root,
        &c_init,
        &c_res,
        &delta_le,
        &r_point,
    );
    let s_scalar = nonce + challenge * blind_sum;
    blind_sum.zeroize();
    Ok(OfflineFastpqSumProof {
        version: OFFLINE_FASTPQ_PROOF_VERSION_V1,
        receipts_root: request.header.receipts_root,
        r_point: r_point.compress().to_bytes(),
        s_scalar: s_scalar.to_bytes(),
    })
}

fn generate_offline_fastpq_counter_proof(
    request: &OfflineProofRequestCounter,
) -> BridgeResult<OfflineFastpqCounterProof> {
    if request.counters.is_empty() {
        return Err(BridgeError::Quantity);
    }
    let mut expected = request.counter_checkpoint;
    for counter in &request.counters {
        expected = expected.checked_add(1).ok_or(BridgeError::Quantity)?;
        if *counter != expected {
            return Err(BridgeError::Quantity);
        }
    }
    let digest = counter_digest(
        &request.header.bundle_id,
        &request.header.receipts_root,
        request.counter_checkpoint,
        &request.counters,
    );
    Ok(OfflineFastpqCounterProof {
        version: OFFLINE_FASTPQ_PROOF_VERSION_V1,
        receipts_root: request.header.receipts_root,
        counter_checkpoint: request.counter_checkpoint,
        digest,
    })
}

fn generate_offline_fastpq_replay_proof(
    request: &OfflineProofRequestReplay,
) -> BridgeResult<OfflineFastpqReplayProof> {
    if request.tx_ids.is_empty() {
        return Err(BridgeError::Quantity);
    }
    let computed_tail = replay_chain(request.replay_log_head, &request.tx_ids);
    if computed_tail != request.replay_log_tail {
        return Err(BridgeError::OfflineSerialize);
    }
    let digest = replay_digest(
        &request.header.bundle_id,
        &request.header.receipts_root,
        &request.replay_log_head,
        &request.replay_log_tail,
        &request.tx_ids,
    );
    Ok(OfflineFastpqReplayProof {
        version: OFFLINE_FASTPQ_PROOF_VERSION_V1,
        receipts_root: request.header.receipts_root,
        replay_log_head: request.replay_log_head,
        replay_log_tail: request.replay_log_tail,
        digest,
    })
}

fn offline_fastpq_proof_sum(request_json: &[u8]) -> BridgeResult<Vec<u8>> {
    let request: OfflineProofRequestSum =
        norito::json::from_slice(request_json).map_err(|err| {
            if cfg!(test) {
                eprintln!("offline sum proof JSON parse failed: {err}");
            }
            BridgeError::OfflineSerialize
        })?;
    let proof = generate_offline_fastpq_sum_proof(&request)?;
    to_bytes(&proof).map_err(|_| BridgeError::OfflineSerialize)
}

fn offline_fastpq_proof_counter(request_json: &[u8]) -> BridgeResult<Vec<u8>> {
    let request: OfflineProofRequestCounter =
        norito::json::from_slice(request_json).map_err(|_| BridgeError::OfflineSerialize)?;
    let proof = generate_offline_fastpq_counter_proof(&request)?;
    to_bytes(&proof).map_err(|_| BridgeError::OfflineSerialize)
}

fn offline_fastpq_proof_replay(request_json: &[u8]) -> BridgeResult<Vec<u8>> {
    let request: OfflineProofRequestReplay =
        norito::json::from_slice(request_json).map_err(|_| BridgeError::OfflineSerialize)?;
    let proof = generate_offline_fastpq_replay_proof(&request)?;
    to_bytes(&proof).map_err(|_| BridgeError::OfflineSerialize)
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
    if let Ok(receipt) = norito::json::from_slice::<IdentifierResolutionReceipt>(bytes) {
        return Ok(receipt);
    }
    let value =
        norito::json::from_slice::<JsonValue>(bytes).map_err(|_| BridgeError::IdentifierReceipt)?;
    parse_identifier_receipt_value(value)
}

fn parse_identifier_receipt_value(value: JsonValue) -> BridgeResult<IdentifierResolutionReceipt> {
    let JsonValue::Object(object) = value else {
        return Err(BridgeError::IdentifierReceipt);
    };

    let signature = parse_identifier_receipt_signature(object.get("signature"))?;

    if let Some(payload_hex) = object
        .get("signature_payload_hex")
        .and_then(JsonValue::as_str)
    {
        let payload_bytes = decode_identifier_receipt_hex(payload_hex)?;
        let decoded_payload =
            decode_from_bytes::<IdentifierResolutionReceiptPayload>(&payload_bytes).or_else(|_| {
                IdentifierResolutionReceiptPayload::decode_all(&mut payload_bytes.as_slice())
            });
        if let Ok(payload) = decoded_payload {
            return Ok(IdentifierResolutionReceipt {
                payload,
                signature: Some(signature.clone()),
                proof: None,
            });
        }
    }

    if let Some(payload_value) = object
        .get("signature_payload")
        .cloned()
        .or_else(|| object.get("payload").cloned())
        && let Ok(payload) =
            norito::json::from_value::<IdentifierResolutionReceiptPayload>(payload_value)
    {
        return Ok(IdentifierResolutionReceipt {
            payload,
            signature: Some(signature),
            proof: None,
        });
    }

    Err(BridgeError::IdentifierReceipt)
}

fn parse_identifier_receipt_signature(value: Option<&JsonValue>) -> BridgeResult<Signature> {
    let signature_hex = value
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::IdentifierReceipt)?;
    let signature_bytes = decode_identifier_receipt_hex(signature_hex)?;
    Ok(Signature::from_bytes(&signature_bytes))
}

fn decode_identifier_receipt_hex(value: &str) -> BridgeResult<Vec<u8>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BridgeError::IdentifierReceipt);
    }
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    hex::decode(body).map_err(|_| BridgeError::IdentifierReceipt)
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
        let (_alg, public_bytes) = key_pair.public_key().to_bytes();
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
        let (_alg, public_bytes) = public_key.to_bytes();
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

// ---------------- Offline allowance helpers ----------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_receipt_challenge(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    invoice_ptr: *const c_char,
    invoice_len: c_ulong,
    receiver_ptr: *const c_char,
    receiver_len: c_ulong,
    asset_ptr: *const c_char,
    asset_len: c_ulong,
    amount_ptr: *const c_char,
    amount_len: c_ulong,
    issued_at_ms: c_ulonglong,
    sender_certificate_id_ptr: *const c_char,
    sender_certificate_id_len: c_ulong,
    nonce_ptr: *const c_char,
    nonce_len: c_ulong,
    out_preimage_ptr: *mut *mut c_uchar,
    out_preimage_len: *mut c_ulong,
    out_hash_ptr: *mut c_uchar,
    out_hash_len: c_ulong,
    out_client_hash_ptr: *mut c_uchar,
    out_client_hash_len: c_ulong,
) -> c_int {
    let result = (|| {
        if out_preimage_ptr.is_null()
            || out_preimage_len.is_null()
            || out_hash_ptr.is_null()
            || out_client_hash_ptr.is_null()
        {
            return Err(BridgeError::NullPtr);
        }

        let chain_id = unsafe { read_string_bridge(chain_ptr, chain_len)? };
        let invoice = unsafe { read_string_bridge(invoice_ptr, invoice_len)? };
        let receiver = unsafe { read_string_bridge(receiver_ptr, receiver_len)? };
        let asset = unsafe { read_string_bridge(asset_ptr, asset_len)? };
        let amount = unsafe { read_string_bridge(amount_ptr, amount_len)? };
        let sender_certificate_id =
            unsafe { read_string_bridge(sender_certificate_id_ptr, sender_certificate_id_len)? };
        let nonce = unsafe { read_string_bridge(nonce_ptr, nonce_len)? };

        let (preimage, iroha_hash, client_hash) = compute_offline_receipt_challenge(
            chain_id,
            invoice,
            receiver,
            asset,
            amount,
            issued_at_ms,
            sender_certificate_id,
            nonce,
        )?;

        unsafe { write_bytes_bridge(out_preimage_ptr, out_preimage_len, &preimage) }?;
        write_hash(out_hash_ptr, out_hash_len, &iroha_hash)?;
        write_hash(out_client_hash_ptr, out_client_hash_len, &client_hash)?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_receipts_root(
    receipts_ptr: *const c_uchar,
    receipts_len: c_ulong,
    out_root_ptr: *mut c_uchar,
    out_root_len: c_ulong,
) -> c_int {
    let result = (|| {
        if receipts_ptr.is_null() || out_root_ptr.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe { slice::from_raw_parts(receipts_ptr, receipts_len as usize) };
        let receipts: Vec<OfflineSpendReceipt> =
            norito::json::from_slice(bytes).map_err(|_| BridgeError::OfflineSerialize)?;
        let root = compute_receipts_root(&receipts).map_err(|_| BridgeError::OfflineSerialize)?;
        write_hash(out_root_ptr, out_root_len, root.as_bytes())?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_proof_sum(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    out_proof_ptr: *mut *mut c_uchar,
    out_proof_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_ptr.is_null() || out_proof_ptr.is_null() || out_proof_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len as usize) };
        let proof = offline_fastpq_proof_sum(bytes)?;
        unsafe { write_bytes_bridge(out_proof_ptr, out_proof_len, &proof) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_proof_counter(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    out_proof_ptr: *mut *mut c_uchar,
    out_proof_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_ptr.is_null() || out_proof_ptr.is_null() || out_proof_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len as usize) };
        let proof = offline_fastpq_proof_counter(bytes)?;
        unsafe { write_bytes_bridge(out_proof_ptr, out_proof_len, &proof) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_proof_replay(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    out_proof_ptr: *mut *mut c_uchar,
    out_proof_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if request_ptr.is_null() || out_proof_ptr.is_null() || out_proof_len.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len as usize) };
        let proof = offline_fastpq_proof_replay(bytes)?;
        unsafe { write_bytes_bridge(out_proof_ptr, out_proof_len, &proof) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_commitment_update(
    initial_commitment_ptr: *const c_uchar,
    initial_commitment_len: c_ulong,
    claimed_delta_ptr: *const c_char,
    claimed_delta_len: c_ulong,
    initial_blinding_ptr: *const c_uchar,
    initial_blinding_len: c_ulong,
    resulting_blinding_ptr: *const c_uchar,
    resulting_blinding_len: c_ulong,
    out_commitment_ptr: *mut *mut c_uchar,
    out_commitment_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if initial_commitment_ptr.is_null()
            || initial_blinding_ptr.is_null()
            || resulting_blinding_ptr.is_null()
            || out_commitment_ptr.is_null()
            || out_commitment_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }

        let claimed_delta_str =
            unsafe { read_string_bridge(claimed_delta_ptr, claimed_delta_len)? };
        let claimed_delta =
            Numeric::from_str(&claimed_delta_str).map_err(|_| BridgeError::Quantity)?;

        let initial_commitment = unsafe {
            slice::from_raw_parts(initial_commitment_ptr, initial_commitment_len as usize)
        };
        let initial_blinding =
            unsafe { slice::from_raw_parts(initial_blinding_ptr, initial_blinding_len as usize) };
        let resulting_blinding = unsafe {
            slice::from_raw_parts(resulting_blinding_ptr, resulting_blinding_len as usize)
        };

        let updated = update_offline_commitment(
            initial_commitment,
            &claimed_delta,
            initial_blinding,
            resulting_blinding,
        )?;
        unsafe { write_bytes_bridge(out_commitment_ptr, out_commitment_len, &updated) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

/// Derive the resulting blinding scalar for an offline spend commitment.
///
/// Given the current (`initial`) blinding and the receipt's certificate ID + counter,
/// computes: `resulting_blinding = initial_blinding + blinding_scalar_from_seed(seed)`
/// where `seed = OfflineProofBlindingSeed::derive(certificate_id, counter)`.
///
/// This ensures the commitment update uses a deterministic blinding delta that
/// matches what `generate_offline_fastpq_sum_proof` will later verify.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_blinding_from_seed(
    initial_blinding_ptr: *const c_uchar,
    initial_blinding_len: c_ulong,
    certificate_id_ptr: *const c_uchar,
    certificate_id_len: c_ulong,
    counter: u64,
    out_blinding_ptr: *mut *mut c_uchar,
    out_blinding_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if initial_blinding_ptr.is_null()
            || certificate_id_ptr.is_null()
            || out_blinding_ptr.is_null()
            || out_blinding_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }

        let initial_blinding =
            unsafe { slice::from_raw_parts(initial_blinding_ptr, initial_blinding_len as usize) };
        let certificate_id_bytes =
            unsafe { slice::from_raw_parts(certificate_id_ptr, certificate_id_len as usize) };
        if certificate_id_bytes.len() != Hash::LENGTH {
            return Err(BridgeError::OfflineNonce);
        }
        let mut certificate_id_array = [0u8; Hash::LENGTH];
        certificate_id_array.copy_from_slice(certificate_id_bytes);
        // `Hash` has an invariant: the least significant bit must be 1. Reject raw Blake2b-32
        // digests here to avoid silently mutating caller-provided values.
        if certificate_id_array[Hash::LENGTH - 1] & 1 == 0 {
            return Err(BridgeError::OfflineNonce);
        }
        let certificate_id = Hash::prehashed(certificate_id_array);

        let resulting =
            derive_offline_blinding_from_seed(initial_blinding, certificate_id, counter)?;
        unsafe { write_bytes_bridge(out_blinding_ptr, out_blinding_len, &resulting) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_balance_proof(
    chain_ptr: *const c_char,
    chain_len: c_ulong,
    initial_commitment_ptr: *const c_uchar,
    initial_commitment_len: c_ulong,
    resulting_commitment_ptr: *const c_uchar,
    resulting_commitment_len: c_ulong,
    claimed_delta_ptr: *const c_char,
    claimed_delta_len: c_ulong,
    resulting_value_ptr: *const c_char,
    resulting_value_len: c_ulong,
    initial_blinding_ptr: *const c_uchar,
    initial_blinding_len: c_ulong,
    resulting_blinding_ptr: *const c_uchar,
    resulting_blinding_len: c_ulong,
    out_proof_ptr: *mut *mut c_uchar,
    out_proof_len: *mut c_ulong,
) -> c_int {
    let result = (|| {
        if initial_commitment_ptr.is_null()
            || resulting_commitment_ptr.is_null()
            || initial_blinding_ptr.is_null()
            || resulting_blinding_ptr.is_null()
            || out_proof_ptr.is_null()
            || out_proof_len.is_null()
        {
            return Err(BridgeError::NullPtr);
        }

        let chain = unsafe { read_string_bridge(chain_ptr, chain_len)? };
        let claimed_delta_str =
            unsafe { read_string_bridge(claimed_delta_ptr, claimed_delta_len)? };
        let claimed_delta =
            Numeric::from_str(&claimed_delta_str).map_err(|_| BridgeError::Quantity)?;
        let resulting_value_str =
            unsafe { read_string_bridge(resulting_value_ptr, resulting_value_len)? };
        let resulting_value =
            Numeric::from_str(&resulting_value_str).map_err(|_| BridgeError::Quantity)?;

        let initial_commitment = unsafe {
            slice::from_raw_parts(initial_commitment_ptr, initial_commitment_len as usize)
        };
        let resulting_commitment = unsafe {
            slice::from_raw_parts(resulting_commitment_ptr, resulting_commitment_len as usize)
        };
        let initial_blinding =
            unsafe { slice::from_raw_parts(initial_blinding_ptr, initial_blinding_len as usize) };
        let resulting_blinding = unsafe {
            slice::from_raw_parts(resulting_blinding_ptr, resulting_blinding_len as usize)
        };

        let proof = generate_offline_balance_proof(
            ChainId::from(chain),
            &claimed_delta,
            &resulting_value,
            initial_commitment,
            resulting_commitment,
            initial_blinding,
            resulting_blinding,
        )?;
        unsafe { write_bytes_bridge(out_proof_ptr, out_proof_len, &proof) }?;
        Ok(())
    })();

    bridge_result_to_code(result)
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

    Ok(AssetTxInputs {
        chain_id: chain.parse().map_err(|_| BridgeError::ChainId)?,
        authority: parse_account_id(authority_str)?,
        asset_definition: parse_asset_definition(asset_definition_str)?,
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
    let backend_str = value
        .get("backend")
        .and_then(JsonValue::as_str)
        .ok_or(BridgeError::ProofAttachment)?;
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
    let proof = ProofBox::new(proof_backend, proof_bytes);

    let attachment = if let Some(vk_ref) = value.get("vk_ref").and_then(JsonValue::as_object) {
        let vk_backend = vk_ref
            .get("backend")
            .and_then(JsonValue::as_str)
            .ok_or(BridgeError::ProofAttachment)?
            .parse::<String>()
            .map_err(|_| BridgeError::ProofAttachment)?;
        let name = vk_ref
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or(BridgeError::ProofAttachment)?;
        let id = VerifyingKeyId::new(vk_backend, name);
        ProofAttachment::new_ref(backend.clone(), proof.clone(), id)
    } else if let Some(vk_inline) = value.get("vk_inline").and_then(JsonValue::as_object) {
        let vk_backend = vk_inline
            .get("backend")
            .and_then(JsonValue::as_str)
            .ok_or(BridgeError::ProofAttachment)?
            .parse::<String>()
            .map_err(|_| BridgeError::ProofAttachment)?;
        let bytes_b64 = vk_inline
            .get("bytes_b64")
            .and_then(JsonValue::as_str)
            .ok_or(BridgeError::ProofAttachment)?;
        let bytes = decode_base64_bytes(bytes_b64)?;
        let vk = VerifyingKeyBox::new(vk_backend, bytes);
        ProofAttachment::new_inline(backend.clone(), proof.clone(), vk)
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
    let ephemeral = unsafe { slice::from_raw_parts(ephemeral_ptr, 32) };
    let nonce = unsafe { slice::from_raw_parts(nonce_ptr, 24) };
    let ciphertext = if ciphertext_len == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(ciphertext_ptr, ciphertext_len as usize) }.to_vec()
    };
    let mut encoded = Vec::with_capacity(1 + ephemeral.len() + nonce.len() + ciphertext.len() + 10);
    encoded.push(CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1);
    encoded.extend_from_slice(ephemeral);
    encoded.extend_from_slice(nonce);
    encode_varint(ciphertext.len() as u64, &mut encoded);
    encoded.extend_from_slice(&ciphertext);
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
    let view = norito::core::from_bytes_view(bytes)?;
    match view.decode::<proto::EnvelopeV1>() {
        Ok(envelope) => Ok(envelope),
        // Legacy Java fixtures encoded an older schema hash while keeping a
        // payload shape that remains decodable by the current type.
        Err(norito::core::Error::SchemaMismatch) => view.decode_unchecked::<proto::EnvelopeV1>(),
        Err(err) => Err(err),
    }
}

fn encode_envelope_framed(env: &proto::EnvelopeV1) -> Result<Vec<u8>, norito::core::Error> {
    let (payload, flags) = norito::codec::encode_with_header_flags(env);
    norito::core::frame_bare_with_header_flags::<proto::EnvelopeV1>(&payload, flags)
}

fn decode_signed_transaction(bytes: &[u8]) -> Result<SignedTransaction, norito::core::Error> {
    match norito::decode_from_bytes::<SignedTransaction>(bytes) {
        Ok(decoded) => Ok(decoded),
        Err(norito::core::Error::InvalidMagic) => {
            let (decoded, used) = SignedTransaction::decode_from_slice(bytes)?;
            if used != bytes.len() {
                return Err(norito::core::Error::LengthMismatch);
            }
            Ok(decoded)
        }
        Err(err) => Err(err),
    }
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
    let ttl_duration = ttl_option.map(|ttl| Duration::from_millis(ttl.get()));
    let mut builder = TransactionBuilder::new(chain_id, authority);
    builder = builder.with_executable(build_executable());
    if let Some(ttl) = ttl_duration {
        builder.set_ttl(ttl);
    }
    if let Some(nonce) = nonce_option {
        builder.set_nonce(nonce);
    }
    builder.set_creation_time(Duration::from_millis(creation_time_ms));
    let signed = builder.sign(&private_key);
    let (payload, flags) = norito::codec::encode_with_header_flags(&signed);
    let signed_bytes =
        norito::core::frame_bare_with_header_flags::<SignedTransaction>(&payload, flags)
            .expect("frame signed transaction");
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
        let (pk, sk) = scheme.keypair(KeyGenOption::Random);
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

#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_free(ptr_: *mut c_uchar) {
    if !ptr_.is_null() {
        unsafe {
            free(ptr_ as *mut _);
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
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::new(asset_definition.clone(), authority.clone());
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
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::new(asset_definition.clone(), authority.clone());
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

        let payload = ConfidentialEncryptedPayload::new(ephemeral, nonce, ciphertext);
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
        let ciphertext = unsafe {
            slice::from_raw_parts(payload_ciphertext_ptr, payload_ciphertext_len as usize).to_vec()
        };

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
                let payload = ConfidentialEncryptedPayload::new(ephemeral, nonce, ciphertext);
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
    namespace_ptr: *const c_char,
    namespace_len: c_ulong,
    contract_id_ptr: *const c_char,
    contract_id_len: c_ulong,
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
        let namespace = unsafe { read_string_bridge(namespace_ptr, namespace_len) }?;
        let contract_id = unsafe { read_string_bridge(contract_id_ptr, contract_id_len) }?;
        let code_hash_raw = unsafe { read_string_bridge(code_hash_ptr, code_hash_len) }?;
        let abi_hash_raw = unsafe { read_string_bridge(abi_hash_ptr, abi_hash_len) }?;
        let abi_version = unsafe { read_string_bridge(abi_version_ptr, abi_version_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
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
            kotoba: None,
            provenance: None,
        }
        .signed(&key_pair);
        let manifest_provenance = manifest.provenance.clone().ok_or(BridgeError::Governance)?;

        let proposal = ProposeDeployContract {
            namespace,
            contract_id,
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
    namespace_ptr: *const c_char,
    namespace_len: c_ulong,
    contract_id_ptr: *const c_char,
    contract_id_len: c_ulong,
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
        let namespace = unsafe { read_string_bridge(namespace_ptr, namespace_len) }?;
        let contract_id = unsafe { read_string_bridge(contract_id_ptr, contract_id_len) }?;
        let code_hash_raw = unsafe { read_string_bridge(code_hash_ptr, code_hash_len) }?;
        let abi_hash_raw = unsafe { read_string_bridge(abi_hash_ptr, abi_hash_len) }?;
        let abi_version = unsafe { read_string_bridge(abi_version_ptr, abi_version_len) }?;

        let chain_id = chain.parse().map_err(|_| BridgeError::ChainId)?;
        let authority = parse_account_id(authority_str)?;
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
            kotoba: None,
            provenance: None,
        }
        .signed(&key_pair);
        let manifest_provenance = manifest.provenance.clone().ok_or(BridgeError::Governance)?;

        let proposal = ProposeDeployContract {
            namespace,
            contract_id,
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
    let prefixed = public.to_prefixed_string();
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
            DomainId::from_str(domain).expect("domain"),
            Name::from_str(name).expect("name"),
        )
        .to_string()
    }

    fn asset_definition_cstring(domain: &str, name: &str) -> CString {
        cstring(&asset_definition_literal(domain, name))
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
            "bank".parse().expect("domain"),
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
        let (_alg, expected_public_bytes) = expected_public.to_bytes();
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
    fn parse_asset_definition_rejects_legacy_textual_literal() {
        let err = parse_asset_definition("usd#bank".to_owned())
            .expect_err("legacy textual asset definition should fail");
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
        let authority_id = AccountId::parse_encoded(authority.to_str().unwrap())
            .expect("authority account id")
            .into_account_id();
        let scoped_account = cstring(
            &ScopedAccountId::from_account_id(
                authority_id,
                "governance.dataspace"
                    .parse()
                    .expect("governance.dataspace domain"),
            )
            .canonical_encoded(),
        );
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

#[cfg(test)]
mod offline_challenge_tests {
    use std::{ffi::CString, ptr, slice};

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        asset::id::AssetDefinitionId,
        offline::{OfflineReceiptChallengePreimage, OfflineSpendReceipt},
    };
    use norito::{decode_from_bytes, json};

    use super::*;

    fn account_literal(account: &AccountId) -> String {
        account.to_string()
    }

    fn asset_literal(asset: &AssetId) -> String {
        asset.canonical_literal()
    }

    fn account_with_cstring(_domain: &str, seed: u8) -> (AccountId, CString) {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        let account = AccountId::new(public_key);
        let literal = account_literal(&account);
        let parsed = AccountId::parse_encoded(&literal)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("encoded account");
        let cstring = CString::new(literal).expect("account string");
        (parsed, cstring)
    }

    #[test]
    fn offline_challenge_roundtrip() {
        let _guard = super::test_support::chain_discriminant_guard();
        let chain_id = CString::new("test-chain").expect("chain id");
        let invoice = CString::new("inv-ffi").expect("invoice");
        let (controller_account, controller_cstr) = account_with_cstring("bank", 21);
        let (_, receiver_cstr) = account_with_cstring("bank", 99);
        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "bank".parse().unwrap(),
            "usd".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_definition, controller_account.clone());
        let asset_literal = asset_literal(&asset_id);
        let reparsed_asset = AssetId::parse_literal(&asset_literal).expect("asset literal parse");
        assert_eq!(reparsed_asset, asset_id);
        let asset_cstr = CString::new(asset_literal).expect("asset identifier string");
        let amount = CString::new("500").expect("amount");
        let issued_at_ms: u64 = 1_700_000_000_000;
        let sender_certificate_id = Hash::new(b"sender-certificate");
        let sender_certificate_id_cstr =
            CString::new(format!("{}", sender_certificate_id)).expect("sender certificate id");
        let nonce_hash = Hash::new(b"ffi-nonce");
        let nonce_cstr = CString::new(format!("{}", nonce_hash)).expect("nonce identifier");

        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let mut iroha_hash = [0u8; Hash::LENGTH];
        let mut client_hash = [0u8; 32];
        let code = unsafe {
            connect_norito_offline_receipt_challenge(
                chain_id.as_ptr(),
                chain_id.as_bytes().len() as c_ulong,
                invoice.as_ptr(),
                invoice.as_bytes().len() as c_ulong,
                receiver_cstr.as_ptr(),
                receiver_cstr.as_bytes().len() as c_ulong,
                asset_cstr.as_ptr(),
                asset_cstr.as_bytes().len() as c_ulong,
                amount.as_ptr(),
                amount.as_bytes().len() as c_ulong,
                issued_at_ms as c_ulonglong,
                sender_certificate_id_cstr.as_ptr(),
                sender_certificate_id_cstr.as_bytes().len() as c_ulong,
                nonce_cstr.as_ptr(),
                nonce_cstr.as_bytes().len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
                iroha_hash.as_mut_ptr(),
                iroha_hash.len() as c_ulong,
                client_hash.as_mut_ptr(),
                client_hash.len() as c_ulong,
            )
        };
        assert_eq!(code, 0, "expected success");
        assert!(!out_ptr.is_null());
        let preimage = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) }.to_vec();
        connect_norito_free(out_ptr);

        let decoded: OfflineReceiptChallengePreimage =
            decode_from_bytes(&preimage).expect("decode");
        let receiver_account =
            AccountId::parse_encoded(receiver_cstr.to_str().expect("receiver str"))
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("account id");
        assert_eq!(decoded.invoice_id, "inv-ffi");
        assert_eq!(decoded.receiver, receiver_account);
        assert_eq!(decoded.asset, asset_id);
        assert_eq!(decoded.amount, Numeric::from_str("500").unwrap());
        assert_eq!(decoded.issued_at_ms, issued_at_ms);
        assert_eq!(decoded.sender_certificate_id, sender_certificate_id);

        let chain_id_value = ChainId::from("test-chain");
        let expected_hash = chain_bound_receipt_hash(&chain_id_value, &preimage);
        assert_eq!(iroha_hash, *expected_hash.as_ref());
        let expected_client: [u8; 32] = Sha256::digest(expected_hash.as_ref()).into();
        assert_eq!(client_hash, expected_client);

        let controller_again =
            AccountId::parse_encoded(controller_cstr.to_str().expect("controller str"))
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("account id");
        assert_eq!(controller_again, controller_account);
    }

    #[test]
    fn offline_receipts_root_ffi_matches_poseidon() {
        let receipts: Vec<OfflineSpendReceipt> = Vec::new();
        let json_bytes = json::to_vec(&receipts).expect("serialize receipts");
        let mut out_root = [0u8; 32];
        let code = unsafe {
            connect_norito_offline_receipts_root(
                json_bytes.as_ptr(),
                json_bytes.len() as c_ulong,
                out_root.as_mut_ptr(),
                out_root.len() as c_ulong,
            )
        };
        assert_eq!(code, 0, "expected receipts root success");
        let expected = compute_receipts_root(&receipts).expect("root");
        assert_eq!(out_root.as_slice(), expected.as_bytes());
    }

    #[test]
    fn offline_blinding_from_seed_ffi_matches_derive() {
        let initial = Scalar::from(19u64);
        let initial_bytes = initial.to_bytes();
        let certificate_id = Hash::new(b"offline-certificate");
        let counter = 11u64;
        let seed = OfflineProofBlindingSeed::derive(certificate_id, counter);
        let expected = (initial + blinding_scalar_from_seed(&seed)).to_bytes();

        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let code = unsafe {
            connect_norito_offline_blinding_from_seed(
                initial_bytes.as_ptr(),
                initial_bytes.len() as c_ulong,
                certificate_id.as_ref().as_ptr(),
                Hash::LENGTH as c_ulong,
                counter,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(code, 0, "expected success");
        assert!(!out_ptr.is_null());
        assert_eq!(out_len as usize, expected.len());
        let derived = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) }.to_vec();
        connect_norito_free(out_ptr);
        assert_eq!(derived, expected);
    }

    #[test]
    fn offline_blinding_from_seed_rejects_non_hash_certificate_id_bytes() {
        let initial = Scalar::from(19u64);
        let initial_bytes = initial.to_bytes();
        let certificate_id = Hash::new(b"offline-certificate");
        let mut bad_certificate_bytes = *certificate_id.as_ref();
        bad_certificate_bytes[Hash::LENGTH - 1] &= !1;

        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let code = unsafe {
            connect_norito_offline_blinding_from_seed(
                initial_bytes.as_ptr(),
                initial_bytes.len() as c_ulong,
                bad_certificate_bytes.as_ptr(),
                bad_certificate_bytes.len() as c_ulong,
                1,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(code, ERR_OFFLINE_NONCE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }
}

#[cfg(test)]
mod offline_fastpq_proof_tests {
    use std::{ptr, slice};

    use curve25519_dalek::traits::Identity;
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        asset::id::AssetDefinitionId,
        offline::{
            OFFLINE_PROOF_REQUEST_VERSION_V1, OfflineAllowanceCommitment, OfflineProofBlindingSeed,
            OfflineProofRequestCounter, OfflineProofRequestHeader, OfflineProofRequestReplay,
            OfflineProofRequestSum, PoseidonDigest,
        },
    };
    use iroha_primitives::numeric::Numeric;
    use norito::json;

    use super::*;

    fn sample_account_id(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        let account = AccountId::new(public_key);
        let literal = account.to_string();
        AccountId::parse_encoded(&literal)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("encoded account")
    }

    fn sample_header(bundle_id: Hash, certificate_id: Hash) -> OfflineProofRequestHeader {
        OfflineProofRequestHeader {
            version: OFFLINE_PROOF_REQUEST_VERSION_V1,
            bundle_id,
            certificate_id,
            receipts_root: PoseidonDigest::zero(),
        }
    }

    fn sample_sum_request() -> OfflineProofRequestSum {
        let owner = sample_account_id(1);
        let bundle_id = Hash::new(b"bundle-fastpq");
        let certificate_id = Hash::new(b"cert-fastpq");
        let header = sample_header(bundle_id, certificate_id);
        let asset_definition =
            AssetDefinitionId::new("default".parse().unwrap(), "xor".parse().unwrap());
        let asset_id = AssetId::new(asset_definition, owner);
        let receipt_amounts = vec![Numeric::new(10, 0), Numeric::new(15, 0)];
        let claimed_delta = Numeric::new(25, 0);
        let blinding_seeds: Vec<_> = [10_u64, 11_u64]
            .iter()
            .map(|counter| OfflineProofBlindingSeed::derive(certificate_id, *counter))
            .collect();
        let blind_sum = blinding_seeds
            .iter()
            .map(blinding_scalar_from_seed)
            .fold(Scalar::ZERO, |acc, scalar| acc + scalar);
        let delta_scalar = numeric_to_scalar(&claimed_delta).expect("delta scalar");
        let c_init = RistrettoPoint::identity();
        let expected_delta =
            RISTRETTO_BASEPOINT_POINT * delta_scalar + pedersen_generator_h() * blind_sum;
        let c_res = c_init + expected_delta;
        let initial_commitment = OfflineAllowanceCommitment::new(
            asset_id,
            Numeric::new(100, 0),
            c_init.compress().to_bytes().to_vec(),
        );
        OfflineProofRequestSum {
            header,
            initial_commitment,
            resulting_commitment: c_res.compress().to_bytes().to_vec(),
            claimed_delta,
            receipt_amounts,
            blinding_seeds,
        }
    }

    fn sample_counter_request() -> OfflineProofRequestCounter {
        let header = sample_header(Hash::new(b"bundle-fastpq"), Hash::new(b"cert-fastpq"));
        OfflineProofRequestCounter {
            header,
            counter_checkpoint: 9,
            counters: vec![10, 11],
        }
    }

    fn sample_replay_request() -> OfflineProofRequestReplay {
        let header = sample_header(Hash::new(b"bundle-fastpq"), Hash::new(b"cert-fastpq"));
        let replay_log_head = Hash::new(b"replay-head");
        let tx_ids = vec![Hash::new(b"tx-a"), Hash::new(b"tx-b")];
        let replay_log_tail = replay_chain(replay_log_head, &tx_ids);
        OfflineProofRequestReplay {
            header,
            replay_log_head,
            replay_log_tail,
            tx_ids,
        }
    }

    fn call_proof_sum(json_bytes: &[u8]) -> Vec<u8> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let code = unsafe {
            connect_norito_offline_proof_sum(
                json_bytes.as_ptr(),
                json_bytes.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(code, 0, "sum proof should succeed");
        assert!(!out_ptr.is_null());
        let proof = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) }.to_vec();
        connect_norito_free(out_ptr);
        proof
    }

    #[test]
    fn sum_proof_rejects_mismatched_scales() {
        let _guard = super::test_support::chain_discriminant_guard();
        let mut request = sample_sum_request();
        request.receipt_amounts = vec![Numeric::new(10, 1), Numeric::new(15, 0)];
        let result = generate_offline_fastpq_sum_proof(&request);
        assert!(matches!(result, Err(BridgeError::Quantity)));
    }

    fn call_proof_counter(json_bytes: &[u8]) -> Vec<u8> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let code = unsafe {
            connect_norito_offline_proof_counter(
                json_bytes.as_ptr(),
                json_bytes.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(code, 0, "counter proof should succeed");
        assert!(!out_ptr.is_null());
        let proof = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) }.to_vec();
        connect_norito_free(out_ptr);
        proof
    }

    fn call_proof_replay(json_bytes: &[u8]) -> Vec<u8> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let code = unsafe {
            connect_norito_offline_proof_replay(
                json_bytes.as_ptr(),
                json_bytes.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(code, 0, "replay proof should succeed");
        assert!(!out_ptr.is_null());
        let proof = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) }.to_vec();
        connect_norito_free(out_ptr);
        proof
    }

    #[test]
    fn offline_fastpq_sum_proof_matches_generator() {
        let _guard = super::test_support::chain_discriminant_guard();
        let request = sample_sum_request();
        let sum_json = json::to_vec(&request).expect("sum json");
        let parsed: OfflineProofRequestSum = json::from_slice(&sum_json).expect("sum json parse");
        assert_eq!(parsed, request);
        let proof_a = call_proof_sum(&sum_json);
        let proof_b = call_proof_sum(&sum_json);
        assert_eq!(proof_a, proof_b);
        let expected = generate_offline_fastpq_sum_proof(&request).expect("sum proof");
        let expected_bytes = to_bytes(&expected).expect("sum proof bytes");
        assert_eq!(proof_a, expected_bytes);
    }

    #[test]
    fn offline_fastpq_counter_proof_matches_generator() {
        let _guard = super::test_support::chain_discriminant_guard();
        let request = sample_counter_request();
        let counter_json = json::to_vec(&request).expect("counter json");
        let proof = call_proof_counter(&counter_json);
        let expected = generate_offline_fastpq_counter_proof(&request).expect("counter proof");
        let expected_bytes = to_bytes(&expected).expect("counter proof bytes");
        assert_eq!(proof, expected_bytes);
    }

    #[test]
    fn offline_fastpq_replay_proof_matches_generator() {
        let _guard = super::test_support::chain_discriminant_guard();
        let request = sample_replay_request();
        let replay_json = json::to_vec(&request).expect("replay json");
        let proof = call_proof_replay(&replay_json);
        let expected = generate_offline_fastpq_replay_proof(&request).expect("replay proof");
        let expected_bytes = to_bytes(&expected).expect("replay proof bytes");
        assert_eq!(proof, expected_bytes);
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
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::new(asset_definition.clone(), destination.clone());
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
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::new(asset_definition.clone(), destination.clone());
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
        let account = parse_scoped_account_id(account_str)?;
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
                        account.account.clone(),
                        account.domain.clone(),
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
        let account = parse_scoped_account_id(account_str)?;
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
                        account.account.clone(),
                        account.domain.clone(),
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
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::new(asset_definition, destination.clone());
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
            destination,
            quantity,
            ttl,
            private_key,
        } = inputs;
        let nonce = parse_nonce(nonce, nonce_present != 0)?;

        let asset_id = AssetId::new(asset_definition, destination.clone());
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
        let json_bytes = match norito::json::to_vec(&tx) {
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
///   (`<base58-asset-definition-id>#<katakana-i105-account-id>`)
/// - `asset_definition_id`: canonical asset definition id (unprefixed Base58 address)
/// - `account_id`: canonical Katakana i105 account id (i105 literal)
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
        let asset =
            AssetId::parse_literal(&asset_literal).map_err(|_| BridgeError::OfflineAsset)?;
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
        let json_bytes =
            norito::json::to_vec(&payload).map_err(|_| BridgeError::OfflineSerialize)?;
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
fn as_jbytes(data: &[u8]) -> Vec<i8> {
    data.iter().map(|byte| *byte as i8).collect()
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
fn jstring_to_string(
    env: &mut jni::JNIEnv<'_>,
    value: jni::objects::JString<'_>,
) -> Result<String, String> {
    env.get_string(&value)
        .map(|s| s.into())
        .map_err(|err| err.to_string())
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
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_OfflineReceiptChallenge_nativeCompute(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    chain_id: jni::objects::JString<'_>,
    invoice: jni::objects::JString<'_>,
    receiver: jni::objects::JString<'_>,
    asset: jni::objects::JString<'_>,
    amount: jni::objects::JString<'_>,
    issued_at_ms: jni::sys::jlong,
    sender_certificate_id_hex: jni::objects::JString<'_>,
    nonce: jni::objects::JString<'_>,
    iroha_hash_out: jni::objects::JByteArray<'_>,
    client_hash_out: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let chain_str = jstring_to_string(&mut env, chain_id)?;
        let invoice_str = jstring_to_string(&mut env, invoice)?;
        let receiver_str = jstring_to_string(&mut env, receiver)?;
        let asset_str = jstring_to_string(&mut env, asset)?;
        let amount_str = jstring_to_string(&mut env, amount)?;
        let sender_certificate_id_str = jstring_to_string(&mut env, sender_certificate_id_hex)?;
        let nonce_str = jstring_to_string(&mut env, nonce)?;

        if env
            .get_array_length(&iroha_hash_out)
            .map_err(|err| err.to_string())?
            < Hash::LENGTH as i32
        {
            return Err(format!(
                "irohaHashOut must be at least {} bytes",
                Hash::LENGTH
            ));
        }
        if env
            .get_array_length(&client_hash_out)
            .map_err(|err| err.to_string())?
            < 32
        {
            return Err("clientHashOut must be at least 32 bytes".into());
        }

        let (preimage, iroha_hash, client_hash) = compute_offline_receipt_challenge(
            chain_str,
            invoice_str,
            receiver_str,
            asset_str,
            amount_str,
            issued_at_ms as u64,
            sender_certificate_id_str,
            nonce_str,
        )
        .map_err(|err| format!("offline challenge error {}", err.code()))?;

        let preimage_array = env
            .byte_array_from_slice(&preimage)
            .map_err(|err| err.to_string())?;
        let iroha_jbytes = as_jbytes(&iroha_hash);
        env.set_byte_array_region(iroha_hash_out, 0, &iroha_jbytes)
            .map_err(|err| err.to_string())?;
        let client_jbytes = as_jbytes(&client_hash);
        env.set_byte_array_region(client_hash_out, 0, &client_jbytes)
            .map_err(|err| err.to_string())?;
        Ok(preimage_array.into_raw())
    })();

    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(&mut env, message);
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
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_OfflineSpendReceiptPayloadEncoder_nativeEncode(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    tx_id_hex: jni::objects::JString<'_>,
    from_account: jni::objects::JString<'_>,
    to_account: jni::objects::JString<'_>,
    asset_id: jni::objects::JString<'_>,
    amount: jni::objects::JString<'_>,
    issued_at_ms: jni::sys::jlong,
    invoice_id: jni::objects::JString<'_>,
    platform_proof_json: jni::objects::JString<'_>,
    sender_certificate_id_hex: jni::objects::JString<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let tx_id_str = jstring_to_string(&mut env, tx_id_hex)?;
        let from_str = jstring_to_string(&mut env, from_account)?;
        let to_str = jstring_to_string(&mut env, to_account)?;
        let asset_str = jstring_to_string(&mut env, asset_id)?;
        let amount_str = jstring_to_string(&mut env, amount)?;
        let invoice_str = jstring_to_string(&mut env, invoice_id)?;
        let proof_json = jstring_to_string(&mut env, platform_proof_json)?;
        let certificate_id_hex = jstring_to_string(&mut env, sender_certificate_id_hex)?;

        let bytes = encode_offline_spend_receipt_payload(
            tx_id_str,
            from_str,
            to_str,
            asset_str,
            amount_str,
            issued_at_ms as u64,
            invoice_str,
            proof_json,
            certificate_id_hex,
        )
        .map_err(|e| format!("encode error: {}", e.code()))?;

        let array = env
            .byte_array_from_slice(&bytes)
            .map_err(|e| e.to_string())?;
        Ok(array.into_raw())
    })();

    match result {
        Ok(array) => array,
        Err(msg) => {
            throw_java_illegal_argument(&mut env, msg);
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
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_OfflineBuildClaimPayloadEncoder_nativeEncode(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    claim_id_hex: jni::objects::JString<'_>,
    platform: jni::objects::JString<'_>,
    app_id: jni::objects::JString<'_>,
    build_number: jni::sys::jlong,
    issued_at_ms: jni::sys::jlong,
    expires_at_ms: jni::sys::jlong,
    lineage_scope: jni::objects::JString<'_>,
    nonce_hex: jni::objects::JString<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let claim_id = jstring_to_string(&mut env, claim_id_hex)?;
        let platform_str = jstring_to_string(&mut env, platform)?;
        let app = jstring_to_string(&mut env, app_id)?;
        let scope = jstring_to_string(&mut env, lineage_scope)?;
        let nonce = jstring_to_string(&mut env, nonce_hex)?;

        let bytes = encode_offline_build_claim_payload(
            claim_id,
            platform_str,
            app,
            build_number as u64,
            issued_at_ms as u64,
            expires_at_ms as u64,
            scope,
            nonce,
        )
        .map_err(|e| format!("encode error: {}", e.code()))?;

        let array = env
            .byte_array_from_slice(&bytes)
            .map_err(|e| e.to_string())?;
        Ok(array.into_raw())
    })();

    match result {
        Ok(array) => array,
        Err(msg) => {
            throw_java_illegal_argument(&mut env, msg);
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

#[cfg(test)]
mod offline_receipt_challenge_tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
    };

    fn sample_account_id() -> AccountId {
        let keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn sample_asset_id(account: &AccountId) -> AssetId {
        let definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "xor".parse().unwrap(),
        );
        AssetId::new(definition, account.clone())
    }

    fn account_literal(account: &AccountId) -> String {
        account.to_string()
    }

    fn asset_literal(asset: &AssetId) -> String {
        asset.canonical_literal()
    }

    #[test]
    fn receipt_challenge_rejects_empty_chain_id() {
        let _guard = super::test_support::chain_discriminant_guard();
        let receiver = sample_account_id();
        let asset = sample_asset_id(&receiver);
        let asset_literal = asset_literal(&asset);
        let reparsed_asset = AssetId::parse_literal(&asset_literal).expect("asset literal parse");
        assert_eq!(reparsed_asset, asset);
        let sender_certificate_id = Hash::new(b"receipt-certificate").to_string();
        let nonce = Hash::new(b"receipt-nonce").to_string();
        let result = compute_offline_receipt_challenge(
            "".to_string(),
            "inv-1".to_string(),
            account_literal(&receiver),
            asset_literal,
            "1".to_string(),
            0,
            sender_certificate_id,
            nonce,
        );
        assert!(matches!(result, Err(BridgeError::ChainId)));
    }

    #[test]
    fn receipt_challenge_accepts_scaled_amount() {
        let _guard = super::test_support::chain_discriminant_guard();
        let receiver = sample_account_id();
        let asset = sample_asset_id(&receiver);
        let asset_literal = asset_literal(&asset);
        let reparsed_asset = AssetId::parse_literal(&asset_literal).expect("asset literal parse");
        assert_eq!(reparsed_asset, asset);
        let sender_certificate_id = Hash::new(b"receipt-certificate").to_string();
        let nonce = Hash::new(b"receipt-nonce").to_string();
        let result = compute_offline_receipt_challenge(
            "test-chain".to_string(),
            "inv-1".to_string(),
            account_literal(&receiver),
            asset_literal,
            "1.5".to_string(),
            0,
            sender_certificate_id,
            nonce,
        );
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }
}

#[cfg(test)]
mod offline_balance_proof_tests {
    use curve25519_dalek::{scalar::Scalar, traits::VartimeMultiscalarMul};
    use iroha_primitives::numeric::Numeric;

    use super::*;

    fn scalar_bytes(scalar: Scalar) -> [u8; 32] {
        scalar.to_bytes()
    }

    fn pedersen_commit(value: &Numeric, blinding: Scalar) -> RistrettoPoint {
        let scalar = numeric_to_scalar(value).expect("numeric to scalar");
        RISTRETTO_BASEPOINT_POINT * scalar + pedersen_generator_h() * blinding
    }

    fn verify_proof(
        chain_id: &ChainId,
        claimed_delta: &Numeric,
        c_init: &RistrettoPoint,
        c_res: &RistrettoPoint,
        proof: &[u8],
    ) -> bool {
        if proof.len() != OFFLINE_BALANCE_PROOF_BYTES {
            return false;
        }
        if proof[0] != OFFLINE_BALANCE_PROOF_VERSION {
            return false;
        }
        let delta_proof = &proof[1..1 + OFFLINE_DELTA_PROOF_BYTES];
        let range_proof = &proof[1 + OFFLINE_DELTA_PROOF_BYTES..];
        let r_point = match decode_commitment_point(&delta_proof[0..32]) {
            Ok(point) => point,
            Err(_) => return false,
        };
        let s_g = match decode_scalar_bytes(&delta_proof[32..64]) {
            Ok(scalar) => scalar,
            Err(_) => return false,
        };
        let s_h = match decode_scalar_bytes(&delta_proof[64..96]) {
            Ok(scalar) => scalar,
            Err(_) => return false,
        };
        let delta_bytes = match numeric_to_le_bytes(claimed_delta) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let context = transcript_context(chain_id);
        let u = c_res - c_init;
        let challenge = transcript_challenge(c_init, c_res, &delta_bytes, &context, &u, &r_point);
        let lhs = RistrettoPoint::vartime_multiscalar_mul(
            [s_g, s_h],
            [RISTRETTO_BASEPOINT_POINT, *pedersen_generator_h()],
        );
        let rhs = r_point + u * challenge;
        lhs == rhs && verify_range_proof(chain_id, c_res, range_proof)
    }

    fn verify_range_proof(chain_id: &ChainId, c_res: &RistrettoPoint, range_proof: &[u8]) -> bool {
        if range_proof.len() != OFFLINE_RANGE_PROOF_BYTES {
            return false;
        }
        let context = transcript_context(chain_id);
        let mut commitments = Vec::with_capacity(OFFLINE_RANGE_PROOF_BITS);
        for bit_index in 0..OFFLINE_RANGE_PROOF_BITS {
            let offset = bit_index * OFFLINE_RANGE_PROOF_PER_BIT_BYTES;
            let commitment = match decode_commitment_point(&range_proof[offset..offset + 32]) {
                Ok(point) => point,
                Err(_) => return false,
            };
            let a0 = match decode_commitment_point(&range_proof[offset + 32..offset + 64]) {
                Ok(point) => point,
                Err(_) => return false,
            };
            let a1 = match decode_commitment_point(&range_proof[offset + 64..offset + 96]) {
                Ok(point) => point,
                Err(_) => return false,
            };
            let e0 = match decode_scalar_bytes(&range_proof[offset + 96..offset + 128]) {
                Ok(scalar) => scalar,
                Err(_) => return false,
            };
            let s0 = match decode_scalar_bytes(&range_proof[offset + 128..offset + 160]) {
                Ok(scalar) => scalar,
                Err(_) => return false,
            };
            let s1 = match decode_scalar_bytes(&range_proof[offset + 160..offset + 192]) {
                Ok(scalar) => scalar,
                Err(_) => return false,
            };
            let challenge = range_proof_challenge(&context, bit_index as u8, &commitment, &a0, &a1);
            let e1 = challenge - e0;
            let lhs0 = RistrettoPoint::vartime_multiscalar_mul(
                [s0, -e0],
                [*pedersen_generator_h(), commitment],
            );
            if lhs0 != a0 {
                return false;
            }
            let commitment_minus_g = commitment - RISTRETTO_BASEPOINT_POINT;
            let lhs1 = RistrettoPoint::vartime_multiscalar_mul(
                [s1, -e1],
                [*pedersen_generator_h(), commitment_minus_g],
            );
            if lhs1 != a1 {
                return false;
            }
            commitments.push(commitment);
        }
        let scalars: Vec<Scalar> = (0..OFFLINE_RANGE_PROOF_BITS)
            .map(|index| Scalar::from(1u64 << index))
            .collect();
        let sum = RistrettoPoint::vartime_multiscalar_mul(scalars, commitments.iter());
        sum == *c_res
    }

    #[test]
    fn commitment_update_and_proof_roundtrip() {
        let chain_id = ChainId::from("iroha-sdk-tests");
        let initial_amount = Numeric::new(50, 2);
        let delta = Numeric::new(7, 2);
        let updated_amount = initial_amount
            .clone()
            .checked_add(delta.clone())
            .expect("sum");
        let blind_init = Scalar::from(5u64);
        let blind_res = Scalar::from(11u64);

        let c_init = pedersen_commit(&initial_amount, blind_init);
        let c_res_expected = pedersen_commit(&updated_amount, blind_res);
        let init_bytes = c_init.compress().as_bytes().to_vec();
        let blind_init_bytes = scalar_bytes(blind_init).to_vec();
        let blind_res_bytes = scalar_bytes(blind_res).to_vec();

        let updated_bytes =
            update_offline_commitment(&init_bytes, &delta, &blind_init_bytes, &blind_res_bytes)
                .expect("commitment update");
        assert_eq!(&updated_bytes[..], c_res_expected.compress().as_bytes());

        let proof = generate_offline_balance_proof(
            chain_id.clone(),
            &delta,
            &updated_amount,
            &init_bytes,
            &updated_bytes,
            &blind_init_bytes,
            &blind_res_bytes,
        )
        .expect("balance proof");

        assert!(
            verify_proof(&chain_id, &delta, &c_init, &c_res_expected, &proof),
            "proof must verify"
        );
    }

    #[test]
    fn commitment_update_accepts_scaled_delta() {
        let initial_amount = Numeric::new(50, 1);
        let delta = Numeric::new(5, 1);
        let updated_amount = Numeric::new(55, 1);
        let blind_init = Scalar::from(5u64);
        let blind_res = Scalar::from(11u64);
        let c_init = pedersen_commit(&initial_amount, blind_init);
        let c_res_expected = pedersen_commit(&updated_amount, blind_res);
        let init_bytes = c_init.compress().as_bytes().to_vec();
        let blind_init_bytes = scalar_bytes(blind_init).to_vec();
        let blind_res_bytes = scalar_bytes(blind_res).to_vec();

        let updated =
            update_offline_commitment(&init_bytes, &delta, &blind_init_bytes, &blind_res_bytes)
                .expect("commitment update");
        assert_eq!(&updated[..], c_res_expected.compress().as_bytes());
    }

    /// Simulates iOS offline payment flow:
    /// 1. topUp creates C(initial_amount, blind) via updateCommitment(zeros, amount, zeros, blind)
    /// 2. Spend delta: updateCommitment(c_init, delta, blind_init, blind_res)
    /// 3. generateProof must accept resultingValue = initial_amount + delta
    #[test]
    fn topup_then_spend_roundtrip() {
        let chain_id = ChainId::from("sora");
        let topup_amount = Numeric::new(4500, 2); // "45.00"
        let spend_delta = Numeric::new(500, 2); // "5.00"

        // Step 1: topUp — compute initial commitment C(45.00, blind_topup)
        let zero_commitment = [0u8; 32];
        let zero_blinding = [0u8; 32];
        let blind_topup = Scalar::from(42u64);
        let blind_topup_bytes = scalar_bytes(blind_topup).to_vec();

        let c_init_bytes = update_offline_commitment(
            &zero_commitment,
            &topup_amount,
            &zero_blinding,
            &blind_topup_bytes,
        )
        .expect("initial commitment");

        // Verify: c_init should be C(45.00, blind_topup)
        let c_init_expected = pedersen_commit(&topup_amount, blind_topup);
        assert_eq!(
            &c_init_bytes[..],
            c_init_expected.compress().as_bytes(),
            "initial commitment must equal C(topup_amount, blind_topup)"
        );

        // Step 2: spend 5.00 — resultingValue = topup_amount + spend_delta
        let resulting_amount = topup_amount
            .clone()
            .checked_add(spend_delta.clone())
            .expect("sum");
        let blind_spend = Scalar::from(99u64);
        let blind_spend_bytes = scalar_bytes(blind_spend).to_vec();

        let proof = generate_offline_balance_proof(
            chain_id.clone(),
            &spend_delta,
            &resulting_amount,
            &c_init_bytes,
            &update_offline_commitment(
                &c_init_bytes,
                &spend_delta,
                &blind_topup_bytes,
                &blind_spend_bytes,
            )
            .expect("updated commitment"),
            &blind_topup_bytes,
            &blind_spend_bytes,
        )
        .expect("balance proof for spend");

        // Verify proof
        let c_init = decode_commitment_point(&c_init_bytes).unwrap();
        let c_res = pedersen_commit(&resulting_amount, blind_spend);
        assert!(
            verify_proof(&chain_id, &spend_delta, &c_init, &c_res, &proof),
            "spend proof must verify"
        );
    }

    #[test]
    fn proof_rejects_scale_mismatch() {
        let chain_id = ChainId::from("iroha-sdk-tests");
        let initial_amount = Numeric::new(50, 0);
        let delta = Numeric::new(7, 0);
        let resulting_value = Numeric::new(575, 1);
        let blind_init = Scalar::from(5u64);
        let blind_res = Scalar::from(11u64);

        let c_init = pedersen_commit(&initial_amount, blind_init);
        let c_res_expected = pedersen_commit(&Numeric::new(57, 0), blind_res);
        let init_bytes = c_init.compress().as_bytes().to_vec();
        let updated_bytes = c_res_expected.compress().as_bytes().to_vec();
        let blind_init_bytes = scalar_bytes(blind_init).to_vec();
        let blind_res_bytes = scalar_bytes(blind_res).to_vec();

        let proof = generate_offline_balance_proof(
            chain_id,
            &delta,
            &resulting_value,
            &init_bytes,
            &updated_bytes,
            &blind_init_bytes,
            &blind_res_bytes,
        );
        assert!(matches!(proof, Err(BridgeError::Quantity)));
    }

    #[test]
    fn derive_offline_blinding_from_seed_matches_expected_sum() {
        let initial = Scalar::from(19u64);
        let certificate_id = Hash::new(b"offline-certificate");
        let counter = 11u64;
        let seed = OfflineProofBlindingSeed::derive(certificate_id, counter);
        let expected = (initial + blinding_scalar_from_seed(&seed)).to_bytes();

        let derived =
            derive_offline_blinding_from_seed(&initial.to_bytes(), certificate_id, counter)
                .expect("derive resulting blinding");

        assert_eq!(derived, expected);
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
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_OfflineBalanceProof_nativeUpdateCommitment(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    initial_commitment: jni::objects::JByteArray<'_>,
    claimed_delta: jni::objects::JString<'_>,
    initial_blinding: jni::objects::JByteArray<'_>,
    resulting_blinding: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let initial_commitment_vec = env
            .convert_byte_array(initial_commitment)
            .map_err(|err| err.to_string())?;
        let initial_blinding_vec = env
            .convert_byte_array(initial_blinding)
            .map_err(|err| err.to_string())?;
        let resulting_blinding_vec = env
            .convert_byte_array(resulting_blinding)
            .map_err(|err| err.to_string())?;
        if initial_commitment_vec.len() != 32 {
            return Err("initialCommitment must be 32 bytes".into());
        }
        if initial_blinding_vec.len() != 32 || resulting_blinding_vec.len() != 32 {
            return Err("blinding scalars must be 32 bytes".into());
        }
        let delta_str = jstring_to_string(&mut env, claimed_delta)?;
        let claimed_delta =
            Numeric::from_str(&delta_str).map_err(|_| "invalid delta value".to_owned())?;
        let updated = update_offline_commitment(
            &initial_commitment_vec,
            &claimed_delta,
            &initial_blinding_vec,
            &resulting_blinding_vec,
        )
        .map_err(|err| format!("offline commitment error {}", err.code()))?;
        let array = env
            .byte_array_from_slice(&updated)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();

    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(&mut env, message);
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
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_offline_OfflineBalanceProof_nativeGenerate(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    chain_id: jni::objects::JString<'_>,
    initial_commitment: jni::objects::JByteArray<'_>,
    resulting_commitment: jni::objects::JByteArray<'_>,
    claimed_delta: jni::objects::JString<'_>,
    resulting_value: jni::objects::JString<'_>,
    initial_blinding: jni::objects::JByteArray<'_>,
    resulting_blinding: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let chain_str = jstring_to_string(&mut env, chain_id)?;
        let delta_str = jstring_to_string(&mut env, claimed_delta)?;
        let resulting_value_str = jstring_to_string(&mut env, resulting_value)?;
        let initial_commitment_vec = env
            .convert_byte_array(initial_commitment)
            .map_err(|err| err.to_string())?;
        let resulting_commitment_vec = env
            .convert_byte_array(resulting_commitment)
            .map_err(|err| err.to_string())?;
        let initial_blinding_vec = env
            .convert_byte_array(initial_blinding)
            .map_err(|err| err.to_string())?;
        let resulting_blinding_vec = env
            .convert_byte_array(resulting_blinding)
            .map_err(|err| err.to_string())?;
        if initial_commitment_vec.len() != 32 || resulting_commitment_vec.len() != 32 {
            return Err("commitments must be 32 bytes".into());
        }
        if initial_blinding_vec.len() != 32 || resulting_blinding_vec.len() != 32 {
            return Err("blinding scalars must be 32 bytes".into());
        }
        let claimed_delta =
            Numeric::from_str(&delta_str).map_err(|_| "invalid delta value".to_owned())?;
        let resulting_value = Numeric::from_str(&resulting_value_str)
            .map_err(|_| "invalid resulting value".to_owned())?;
        let proof = generate_offline_balance_proof(
            ChainId::from(chain_str),
            &claimed_delta,
            &resulting_value,
            &initial_commitment_vec,
            &resulting_commitment_vec,
            &initial_blinding_vec,
            &resulting_blinding_vec,
        )
        .map_err(|err| format!("offline proof error {}", err.code()))?;
        let array = env
            .byte_array_from_slice(&proof)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();

    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(&mut env, message);
            std::ptr::null_mut()
        }
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

    use iroha_crypto::{Algorithm, KeyPair};
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
        IdentifierResolutionReceiptPayload {
            policy_id: "email#retail".parse().expect("valid policy id"),
            execution: iroha_data_model::ram_lfe::RamLfeExecutionReceiptPayload {
                program_id: "identifier_lookup_retail"
                    .parse()
                    .expect("valid program id"),
                program_digest: Hash::new(b"program"),
                backend: iroha_crypto::RamLfeBackend::BfvProgrammedSha3_256V1,
                verification_mode: iroha_crypto::RamLfeVerificationMode::Signed,
                output_hash: Hash::new(b"output"),
                associated_data_hash: Hash::new(b"associated-data"),
                executed_at_ms: 7,
                expires_at_ms: Some(107),
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

    fn sample_rwa_id_literal() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities".to_owned()
    }

    const LIVE_EMAIL_CLAIM_SIGNATURE_HEX: &str = "9262CA8C755D47207ED0CD2E19892DFAA4612701A36DCAF87173D42CC754DFB6A66158856FDFD25974C2A11E9FC32940CA0DF18CAC25A38CB5DEDC4625E67900";

    const LIVE_EMAIL_CLAIM_PAYLOAD_HEX: &str = "2B000000000000000D000000000000000500000000000000656D61696C0E00000000000000060000000000000072657461696CDD000000000000001C0000000000000014000000000000000C00000000000000656D61696C5F72657461696C200000000000000075522459A6B0039705A18CE5D21050F39454F440203D6041C454658735DA1D070400000000000000020000000400000000000000000000002000000000000000E12E5429C9C8B146C4EC6DB972DCDEBD6BC84257A4503C0A152E052D345B303D2000000000000000444180A5ECCBC236041F1DC4D5E3BD220B0656261EB55B9D9AE32AA729B5437F0800000000000000987ABF0A9D0100001100000000000000010800000000000000780EC40A9D01000028000000000000002000000000000000D82F9EAB952F7A5241BB2339C0095EBC61958428164AB820FAD85952F35745852000000000000000032DF7E7370E04DDBABF0CD40932935A1D2C77A9B8D723BBB9F1472F2791CC7128000000000000002000000000000000C60973F731CCB57008687F9BC38CC712E3BE7AB46D99A1BEFFD1C9FD61E60A875A00000000000000000000004E00000000000000460000000000000065643031323035363334453930373145383636323937344132324631333739373236363343343634344443333534364131393338453143414335384445344342413844393635";

    #[test]
    fn parse_identifier_receipt_accepts_torii_payload_hex() {
        let payload = sample_identifier_receipt_payload();
        let payload_hex = hex::encode(to_bytes(&payload).expect("encode payload"));
        let receipt = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(sample_identifier_signature_hex()),
            ),
            ("signature_payload_hex", JsonValue::from(payload_hex)),
        ]))
        .expect("parse torii payload hex receipt");

        assert_eq!(receipt.payload, payload);
        assert_eq!(
            hex::encode(receipt.signature.expect("signature").payload()),
            sample_identifier_signature_hex()
        );
    }

    #[test]
    fn parse_identifier_receipt_accepts_signature_payload_fallback() {
        let payload = sample_identifier_receipt_payload();
        let payload_value = norito::json::to_value(&payload).expect("payload json");
        let receipt = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(sample_identifier_signature_hex()),
            ),
            ("signature_payload_hex", JsonValue::from("01020304a0")),
            ("signature_payload", payload_value),
        ]))
        .expect("parse torii payload object receipt");

        assert_eq!(receipt.payload, payload);
        assert_eq!(
            hex::encode(receipt.signature.expect("signature").payload()),
            sample_identifier_signature_hex()
        );
    }

    #[test]
    fn parse_identifier_receipt_accepts_payload_envelope_fallback() {
        let payload = sample_identifier_receipt_payload();
        let payload_value = norito::json::to_value(&payload).expect("payload json");
        let receipt = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(sample_identifier_signature_hex()),
            ),
            ("payload", payload_value),
        ]))
        .expect("parse payload envelope receipt");

        assert_eq!(receipt.payload, payload);
        assert_eq!(
            hex::encode(receipt.signature.expect("signature").payload()),
            sample_identifier_signature_hex()
        );
    }

    #[test]
    fn parse_identifier_receipt_accepts_live_torii_payload_hex() {
        let receipt = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(LIVE_EMAIL_CLAIM_SIGNATURE_HEX.to_owned()),
            ),
            (
                "signature_payload_hex",
                JsonValue::from(LIVE_EMAIL_CLAIM_PAYLOAD_HEX.to_owned()),
            ),
        ]))
        .expect("parse live torii payload hex receipt");

        assert_eq!(
            receipt.payload.policy_id.to_string(),
            "email#retail",
            "live claim policy id should round-trip from payload hex"
        );
        assert_eq!(
            receipt.payload.receipt_hash.to_string(),
            "032df7e7370e04ddbabf0cd40932935a1d2c77a9b8d723bbb9f1472f2791cc71",
            "live claim receipt hash should round-trip from payload hex"
        );
        assert_eq!(
            hex::encode_upper(receipt.signature.expect("signature").payload()),
            LIVE_EMAIL_CLAIM_SIGNATURE_HEX
        );
    }

    #[test]
    fn parse_identifier_receipt_accepts_swift_normalized_payload_fallback() {
        let payload_bytes =
            hex::decode(LIVE_EMAIL_CLAIM_PAYLOAD_HEX).expect("hex decode live payload");
        let payload = IdentifierResolutionReceiptPayload::decode_all(&mut payload_bytes.as_slice())
            .expect("decode live payload bytes");
        let payload_value = json_object([
            ("policy_id", JsonValue::from(payload.policy_id.to_string())),
            ("opaque_id", JsonValue::from(payload.opaque_id.to_string())),
            (
                "receipt_hash",
                JsonValue::from(payload.receipt_hash.to_string()),
            ),
            ("uaid", JsonValue::from(payload.uaid.to_string())),
            (
                "account_id",
                JsonValue::from(payload.account_id.to_string()),
            ),
            (
                "execution",
                json_object([
                    (
                        "program_id",
                        JsonValue::from(payload.execution.program_id.name.to_string()),
                    ),
                    (
                        "program_digest",
                        JsonValue::from(payload.execution.program_digest.to_string()),
                    ),
                    ("backend", JsonValue::from("bfv-programmed-sha3-256-v1")),
                    ("verification_mode", JsonValue::from("signed")),
                    (
                        "output_hash",
                        JsonValue::from(payload.execution.output_hash.to_string()),
                    ),
                    (
                        "associated_data_hash",
                        JsonValue::from(payload.execution.associated_data_hash.to_string()),
                    ),
                    (
                        "executed_at_ms",
                        JsonValue::from(payload.execution.executed_at_ms),
                    ),
                    (
                        "expires_at_ms",
                        JsonValue::from(
                            payload
                                .execution
                                .expires_at_ms
                                .expect("live payload carries expiry"),
                        ),
                    ),
                ]),
            ),
        ]);
        let receipt = parse_identifier_receipt_value(json_object([
            (
                "signature",
                JsonValue::from(LIVE_EMAIL_CLAIM_SIGNATURE_HEX.to_owned()),
            ),
            (
                "signature_payload_hex",
                JsonValue::from(LIVE_EMAIL_CLAIM_PAYLOAD_HEX.to_owned()),
            ),
            ("signature_payload", payload_value),
        ]))
        .expect("parse swift-normalized claim receipt");

        assert_eq!(receipt.payload, payload);
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
        let domain: DomainId = "wonderland".parse().expect("domain");
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
            JsonValue::from(
                "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            ),
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
            JsonValue::from(
                "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            ),
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
        let env_bytes = norito::core::to_bytes(&env).expect("encode envelope");
        let view = norito::core::from_bytes_view(&env_bytes).expect("envelope view");
        assert_eq!(view.flags(), proto::CONNECT_LAYOUT_FLAGS);
        let decoded_env = decode_envelope(&env_bytes).expect("decode envelope");
        assert_eq!(decoded_env.seq, env.seq);
        assert_eq!(decoded_env.payload, env.payload);
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
    fn decode_envelope_accepts_legacy_schema_hash_fixture() {
        let hex = concat!(
            "4e52543000000b36414bbbba14690b36414bbbba1469008002000000000000e8ae6adadc072f3e0008",
            "0000000000000002000000000000006802000000000000030000005c0200000000000004000000000000",
            "00000000004802000000000000400000000000000001000000000000000001000000000000000101000000",
            "00000000020100000000000000030100000000000000040100000000000000050100000000000000060100",
            "0000000000000701000000000000000801000000000000000901000000000000000a01000000000000000b",
            "01000000000000000c01000000000000000d01000000000000000e01000000000000000f01000000000000",
            "00100100000000000000110100000000000000120100000000000000130100000000000000140100000000",
            "00000015010000000000000016010000000000000017010000000000000018010000000000000019010000",
            "00000000001a01000000000000001b01000000000000001c01000000000000001d01000000000000001e01",
            "000000000000001f0100000000000000200100000000000000210100000000000000220100000000000000",
            "23010000000000000024010000000000000025010000000000000026010000000000000027010000000000",
            "00002801000000000000002901000000000000002a01000000000000002b01000000000000002c01000000",
            "000000002d01000000000000002e01000000000000002f0100000000000000300100000000000000310100",
            "00000000000032010000000000000033010000000000000034010000000000000035010000000000000036",
            "01000000000000003701000000000000003801000000000000003901000000000000003a01000000000000",
            "003b01000000000000003c01000000000000003d01000000000000003e01000000000000003f"
        );
        let bytes = hex::decode(hex).expect("fixture hex");

        let view = norito::core::from_bytes_view(&bytes).expect("framed envelope view");
        assert!(
            matches!(
                view.decode::<proto::EnvelopeV1>(),
                Err(norito::core::Error::SchemaMismatch)
            ),
            "fixture should exercise schema fallback path"
        );

        let decoded = decode_envelope(&bytes).expect("decode legacy schema fixture");
        match decoded.payload {
            proto::ConnectPayloadV1::SignResultOk { signature } => {
                assert_eq!(decoded.seq, 2);
                assert_eq!(signature.bytes().len(), 64);
                assert_eq!(signature.bytes()[0], 0);
                assert_eq!(signature.bytes()[63], 63);
            }
            other => panic!("unexpected payload variant: {other:?}"),
        }
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
    use std::{fs, path::PathBuf, time::Duration};

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::{AccountId, address},
        transaction::{TransactionBuilder, signed::TransactionSignature},
    };
    use norito::{core::read_len_dyn_slice, json::Value};

    use super::decode_signed_transaction;

    // Matches account::address::DEFAULT_CHAIN_DISCRIMINANT (i105 discriminant) used by fixtures.
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
    fn signed_transaction_fixtures_decode() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let manifest = load_manifest();
        let names = [
            "register_asset_definition",
            "mint_asset",
            "grant_revoke_role_permission",
            "set_parameter_next_mode",
        ];
        for name in names {
            let bytes = signed_bytes_for(&manifest, name)
                .unwrap_or_else(|err| panic!("missing {name} signed payload: {err}"));
            let _ = decode_signed_transaction(&bytes);
        }
    }

    #[test]
    fn signed_transaction_norito_rpc_fixtures_decode() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let manifest = load_manifest_at(manifest_path_norito_rpc());
        let names = [
            "register_asset_definition",
            "mint_asset",
            "grant_revoke_role_permission",
            "set_parameter_next_mode",
        ];
        for name in names {
            let bytes = signed_bytes_for(&manifest, name)
                .unwrap_or_else(|err| panic!("missing {name} signed payload: {err}"));
            let _ = decode_signed_transaction(&bytes);
        }
    }

    #[test]
    fn signed_transaction_fixtures_decode_with_header() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let keypair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "00000004".parse().expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(1));
        let tx = builder.sign(keypair.private_key());
        let bytes = norito::codec::encode_adaptive(&tx);
        decode_signed_transaction(&bytes).expect("decode bare signed tx");
        let framed = norito::to_bytes(&tx).expect("encode framed signed tx");
        decode_signed_transaction(&framed).expect("decode framed signed tx");
    }

    #[test]
    fn signed_transaction_fixtures_reencode_match() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let manifest = load_manifest();
        let names = [
            "register_asset_definition",
            "mint_asset",
            "grant_revoke_role_permission",
            "set_parameter_next_mode",
        ];
        for name in names {
            let bytes = signed_bytes_for(&manifest, name)
                .unwrap_or_else(|err| panic!("missing {name} signed payload: {err}"));
            let Ok(signed) = decode_signed_transaction(&bytes) else {
                continue;
            };
            let reencoded_bytes = norito::codec::encode_adaptive(&signed);
            assert_eq!(
                reencoded_bytes, bytes,
                "re-encoded signed transaction differs for {name}"
            );
        }
    }

    #[test]
    fn signed_transaction_fixture_signature_prefix_matches_payload() {
        let _guard = ChainDiscriminantReset::new(FIXTURE_CHAIN_DISCRIMINANT);
        let manifest = load_manifest();
        let signed_bytes =
            signed_bytes_for(&manifest, "mint_asset").expect("mint_asset fixture present");
        let payload_bytes =
            payload_bytes_for(&manifest, "mint_asset").expect("mint_asset payload present");
        let payload_offset = signed_bytes
            .windows(payload_bytes.len())
            .position(|window| window == payload_bytes)
            .expect("payload bytes must appear in signed bytes");

        let (sig_len, sig_hdr) =
            read_len_dyn_slice(&signed_bytes).expect("signature length header");
        let sig_start = sig_hdr;
        let sig_end = sig_start
            .checked_add(sig_len)
            .expect("signature length overflow");
        let sig_slice = signed_bytes
            .get(sig_start..sig_end)
            .expect("signature slice within payload");
        let (_, used) = norito::core::decode_field_canonical::<TransactionSignature>(sig_slice)
            .expect("decode signature slice");
        assert_eq!(
            used, sig_len,
            "signature decode length must match prefix length"
        );

        let payload_len_offset = sig_end;
        let (payload_len, payload_hdr) = read_len_dyn_slice(
            signed_bytes
                .get(payload_len_offset..)
                .expect("payload len header slice"),
        )
        .expect("payload length header");
        let payload_start = payload_len_offset + payload_hdr;
        assert_eq!(
            payload_start, payload_offset,
            "payload must start after signature + payload length header"
        );
        assert_eq!(
            payload_len,
            payload_bytes.len(),
            "payload length header must match fixture payload length"
        );
    }

    fn load_manifest() -> Value {
        load_manifest_at(manifest_path())
    }

    fn load_manifest_at(path: PathBuf) -> Value {
        let bytes = fs::read(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        norito::json::from_slice(&bytes)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
    }

    fn manifest_path() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        root.join("../../IrohaSwift/Fixtures/transaction_fixtures.manifest.json")
    }

    fn manifest_path_norito_rpc() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        root.join("../../fixtures/norito_rpc/transaction_fixtures.manifest.json")
    }

    fn signed_bytes_for(manifest: &Value, name: &str) -> Result<Vec<u8>, String> {
        let fixtures = manifest
            .get("fixtures")
            .and_then(Value::as_array)
            .ok_or_else(|| "manifest missing fixtures array".to_string())?;
        for fixture in fixtures {
            let fixture_name = fixture.get("name").and_then(Value::as_str);
            if fixture_name == Some(name) {
                let signed = fixture
                    .get("signed_base64")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing signed_base64".to_string())?;
                return BASE64
                    .decode(signed)
                    .map_err(|err| format!("base64 decode failed: {err}"));
            }
        }
        Err(format!("fixture {name} not found"))
    }

    fn payload_bytes_for(manifest: &Value, name: &str) -> Result<Vec<u8>, String> {
        let fixtures = manifest
            .get("fixtures")
            .and_then(Value::as_array)
            .ok_or_else(|| "manifest missing fixtures array".to_string())?;
        for fixture in fixtures {
            let fixture_name = fixture.get("name").and_then(Value::as_str);
            if fixture_name == Some(name) {
                let payload = fixture
                    .get("payload_base64")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "missing payload_base64".to_string())?;
                return BASE64
                    .decode(payload)
                    .map_err(|err| format!("base64 decode failed: {err}"));
            }
        }
        Err(format!("fixture {name} not found"))
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
        let manifest_bytes = to_bytes(&manifest).expect("manifest encode");
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

    fn test_account_id(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (public_key, _) = keypair.into_parts();
        AccountId::new(public_key)
    }

    #[test]
    fn encode_offline_spend_receipt_payload_matches_native() {
        use base64::Engine as _;
        use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
        use iroha_data_model::offline::{
            AppleAppAttestProof, OfflineAllowanceCommitment, OfflinePlatformProof,
            OfflineSpendReceipt, OfflineWalletCertificate, OfflineWalletPolicy,
        };
        use iroha_data_model::{Metadata, asset::id::AssetId};
        use iroha_primitives::numeric::Numeric;

        let sender = test_account_id(1);
        let receiver = test_account_id(2);
        let asset_def = iroha_data_model::asset::id::AssetDefinitionId::new(
            "default".parse().unwrap(),
            "xor".parse().unwrap(),
        );
        let asset = AssetId::new(asset_def, sender.clone());
        let challenge_hash = Hash::new(vec![0x33; 32]);

        let certificate = OfflineWalletCertificate {
            controller: sender.clone(),
            operator: sender.clone(),
            allowance: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::from(500_u32),
                commitment: vec![0x42; 32],
            },
            spend_public_key: PublicKey::from_hex(
                Algorithm::Ed25519,
                "1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
            )
            .expect("public key"),
            attestation_report: vec![0x01, 0x02, 0x03],
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_800_000_000_000,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::from(1_000_u32),
                max_tx_value: Numeric::from(200_u32),
                expires_at_ms: 1_800_000_000_000,
            },
            operator_signature: Signature::from_bytes(&[0xAB; 64]),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        };

        let platform_proof = OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
            key_id: BASE64_STANDARD.encode(b"TEST_KEY"),
            counter: 42,
            assertion: vec![0xCA, 0xFE],
            challenge_hash,
        });

        let tx_id = Hash::new(vec![0x22; 32]);

        let receipt = OfflineSpendReceipt {
            tx_id,
            from: sender.clone(),
            to: receiver.clone(),
            asset: asset.clone(),
            amount: Numeric::from(75_u32),
            issued_at_ms: 1_700_000_500_000,
            invoice_id: "INV-42".into(),
            platform_proof: platform_proof.clone(),
            platform_snapshot: None,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature: Signature::from_bytes(&[0xCD; 64]),
            build_claim: None,
        };

        let native_bytes = receipt.signing_bytes().expect("native signing bytes");

        let platform_proof_json =
            norito::json::to_json(&platform_proof).expect("platform proof json");
        let certificate_id_hex = hex::encode(certificate.certificate_id().as_ref());
        let asset_literal = asset.canonical_literal();
        let reparsed_asset = AssetId::parse_literal(&asset_literal).expect("asset literal parse");
        assert_eq!(reparsed_asset, asset);

        let jni_bytes = encode_offline_spend_receipt_payload(
            hex::encode(tx_id.as_ref()),
            sender.to_string(),
            receiver.to_string(),
            asset_literal,
            "75".to_string(),
            1_700_000_500_000,
            "INV-42".to_string(),
            platform_proof_json,
            certificate_id_hex,
        )
        .expect("JNI encoding");

        assert_eq!(
            native_bytes, jni_bytes,
            "JNI encoding must match native signing_bytes"
        );
    }

    #[test]
    fn encode_offline_build_claim_payload_matches_native() {
        use iroha_data_model::offline::{OfflineBuildClaim, OfflineBuildClaimPlatform};

        let claim = OfflineBuildClaim {
            claim_id: Hash::new(b"test-claim-id"),
            platform: OfflineBuildClaimPlatform::Android,
            app_id: "jp.co.soramitsu.cbdc.pkr".to_owned(),
            build_number: 42,
            issued_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_086_400_000,
            lineage_scope: "test-scope".to_owned(),
            nonce: Hash::new(b"test-nonce"),
            operator_signature: Signature::from_bytes(&[0; 64]),
        };

        let native_bytes = claim.signing_bytes().expect("signing bytes");

        let jni_bytes = encode_offline_build_claim_payload(
            hex::encode(claim.claim_id.as_ref()),
            "Android".to_owned(),
            claim.app_id.clone(),
            claim.build_number,
            claim.issued_at_ms,
            claim.expires_at_ms,
            claim.lineage_scope.clone(),
            hex::encode(claim.nonce.as_ref()),
        )
        .expect("jni encode");

        assert_eq!(
            jni_bytes, native_bytes,
            "JNI encoding must match native signing_bytes()"
        );
    }
}

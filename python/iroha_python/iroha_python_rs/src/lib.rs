//! Python bindings exposing a growing subset of the Iroha SDK surface.

#![deny(unsafe_code)]
#![allow(unsafe_op_in_unsafe_fn)] // PyO3 generates historical wrappers that require this on edition 2024

use core::{
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    time::Duration,
};
use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use blake3::hash as blake3_hash;
use futures::executor::block_on;
use hex::{encode as hex_encode, encode_upper as hex_encode_upper};
use iroha_config::parameters::defaults;
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, Hash, HashOf, KeyGenOption, KeyPair, LaneCommitmentId,
    PrivateKey, PublicKey, Signature, derive_keyset_from_slice,
    error::ParseError,
    kex::{KeyExchangeScheme, X25519Sha256},
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature, encode_sm2_public_key_payload},
};
use iroha_data_model::{
    account::Account,
    asset::{
        alias::AssetDefinitionAlias,
        definition::{AssetBalancePolicy, AssetConfidentialPolicy},
        prelude::{AssetDefinition, AssetDefinitionId, AssetId, Mintable},
    },
    block::{
        BlockHeader,
        consensus::{LaneBlockCommitment, PERMISSIONED_TAG},
    },
    confidential::ConfidentialEncryptedPayload,
    consensus::{
        CertPhase, Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1, default_chain_order_hash,
    },
    domain::prelude::{Domain, DomainId},
    events::time::{ExecutionTime, Schedule as TimeSchedule, TimeEventFilter},
    isi::{
        Burn, ExecuteTrigger, Grant, InstructionBox, Mint, Register, RemoveKeyValue, Revoke,
        SetKeyValue, Transfer, Unregister,
        repo::{RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementId,
            SettlementLeg, SettlementPlan,
        },
        zk::{
            RegisterZkAceIdentityCommitment, RegisterZkAsset, RevokeZkAceIdentityCommitment,
            RotateZkAceIdentityCommitment, Shield, SubmitZkAceAuthorizedTransfer, Unshield,
            ZkAssetMode, ZkTransfer,
        },
    },
    metadata::Metadata,
    name::Name,
    nexus::{DataSpaceId, LaneId, LanePrivacyProof, LaneRelayEnvelope, compute_settlement_hash},
    nft::NftId,
    peer::PeerId,
    permission::Permission,
    prelude::{AccountId, ChainId},
    proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
    repo::prelude::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    rwa::{NewRwa, RwaControlPolicy, RwaId, RwaParentRef},
    transaction::{
        Executable, IvmBytecode, SignedTransaction, TransactionBuilder as ModelTransactionBuilder,
        TransactionSubmissionReceipt,
    },
    trigger::{
        Trigger, TriggerId,
        action::{Action as TriggerAction, Repeats},
    },
    zk::{
        ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER, ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
        ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG, ZkAcePublicInputsV1, ZkAceWitnessV1,
    },
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec},
};
use iroha_schema::Ident;
use iroha_torii_shared::{
    connect::{
        AppMeta, ConnectCiphertextV1, ConnectControlV1, ConnectFrameV1, ConnectPayloadV1,
        Constraints, ControlAfterKeyV1, Dir, FrameKind, PermissionsV1, Role, ServerEventV1,
        SignInProofV1, WalletSignatureV1,
    },
    connect_sdk,
};
use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
use norito::{codec, codec::DecodeAll, decode_from_bytes, json, json::JsonSerialize};
use pyo3::{
    Bound, FromPyObject, create_exception,
    exceptions::{PyException, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{
        PyAny, PyBytes, PyDict, PyDictMethods, PyList, PyModule, PyStringMethods, PyTuple, PyType,
    },
    wrap_pyfunction,
};
use rand_core_06::OsRng as OsRng06;
use sorafs_car::{
    CarBuildPlan, CarChunk, FilePlan,
    fetch_plan::chunk_fetch_specs_from_json,
    gateway::{GatewayFetchConfig, GatewayProviderInput},
    multi_fetch::{
        AttemptError, AttemptFailure, CapabilityMismatch, ChunkResponse, ChunkVerificationError,
        FetchOptions, FetchProvider, FetchRequest, MultiSourceError, ProviderMetadata,
        ProviderScoreContext, ProviderScoreDecision, RangeCapability, ScorePolicy, StreamBudget,
        TransportHint, fetch_plan_parallel,
    },
    scoreboard::{self, Eligibility, ProviderTelemetry, ScoreboardConfig, TelemetrySnapshot},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    alias_cache::{AliasCachePolicy, AliasProofState, decode_alias_proof, unix_now_secs},
    capacity::ReplicationOrderV1,
    pin_registry::{
        AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
    },
};
use sorafs_orchestrator::{
    AnonymityPolicy, OrchestratorConfig, RolloutPhase, TransportPolicy, fetch_via_gateway,
    proxy::{
        LocalQuicProxyConfig, ProxyCarBridgeConfig, ProxyKaigiBridgeConfig, ProxyMode,
        ProxyNoritoBridgeConfig,
    },
    taikai_cache::{
        EvictionStats, PromotionStats, QosConfig, QosStats, ReliabilityTuning, TaikaiCacheConfig,
        TaikaiCacheStatsSnapshot, TaikaiPullQueueStats, TierStats,
    },
};
use tokio::runtime::Runtime;
use x25519_dalek::StaticSecret;

/// Raised when a non-Ed25519 key is passed to an Ed25519-only helper.
const ERR_EXPECTED_ED25519: &str = "expected Ed25519 key material";
const ERR_SM2_SIGNATURE_LEN: &str = "sm2 signature must be 64 bytes";
const SM2_PRIVATE_KEY_LENGTH: usize = 32;
const SM2_PUBLIC_KEY_UNCOMPRESSED_LENGTH: usize = 65;
const SM2_SIGNATURE_LENGTH: usize = Sm2Signature::LENGTH;

create_exception!(_crypto, SorafsMultiFetchError, PyException);

fn algorithm_guard(algorithm: Algorithm) -> PyResult<()> {
    if algorithm != Algorithm::Ed25519 {
        Err(PyValueError::new_err(ERR_EXPECTED_ED25519))
    } else {
        Ok(())
    }
}

fn supported_crypto_algorithms() -> Vec<Algorithm> {
    let mut algorithms = vec![Algorithm::Ed25519, Algorithm::Secp256k1, Algorithm::MlDsa];
    algorithms.extend([
        Algorithm::Gost3410_2012_256ParamSetA,
        Algorithm::Gost3410_2012_256ParamSetB,
        Algorithm::Gost3410_2012_256ParamSetC,
        Algorithm::Gost3410_2012_512ParamSetA,
        Algorithm::Gost3410_2012_512ParamSetB,
    ]);
    algorithms.extend([Algorithm::BlsNormal, Algorithm::BlsSmall]);
    algorithms.push(Algorithm::Sm2);
    algorithms
}

fn parse_algorithm_arg(algorithm: &str) -> PyResult<Algorithm> {
    let normalized = algorithm.trim().to_ascii_lowercase();
    if let Ok(parsed) = Algorithm::from_str(&normalized) {
        return Ok(parsed);
    }

    let compact = normalized
        .chars()
        .map(|ch| match ch {
            '_' | ' ' | '.' => '-',
            _ => ch,
        })
        .collect::<String>();

    let parsed = match compact.as_str() {
        "ed-25519" | "ed25519" => Some(Algorithm::Ed25519),
        "ecdsa"
        | "ecdsa-secp256k1"
        | "ecdsa-secp256k1-sha256"
        | "secp-256-k1"
        | "secp-256k1"
        | "secp256k1" => Some(Algorithm::Secp256k1),
        "dilithium" | "dilithium3" | "ml-dsa" | "ml-dsa-65" | "mldsa" | "mldsa65" => {
            Some(Algorithm::MlDsa)
        }
        "gost-3410-2012-256-paramset-a" | "gost3410-2012-256-paramset-a" => {
            Some(Algorithm::Gost3410_2012_256ParamSetA)
        }
        "gost-3410-2012-256-paramset-b" | "gost3410-2012-256-paramset-b" => {
            Some(Algorithm::Gost3410_2012_256ParamSetB)
        }
        "gost-3410-2012-256-paramset-c" | "gost3410-2012-256-paramset-c" => {
            Some(Algorithm::Gost3410_2012_256ParamSetC)
        }
        "gost-3410-2012-512-paramset-a" | "gost3410-2012-512-paramset-a" => {
            Some(Algorithm::Gost3410_2012_512ParamSetA)
        }
        "gost-3410-2012-512-paramset-b" | "gost3410-2012-512-paramset-b" => {
            Some(Algorithm::Gost3410_2012_512ParamSetB)
        }
        "bls-normal" | "blsnormal" | "bls12-381-g1" | "bls12-381-normal" => {
            Some(Algorithm::BlsNormal)
        }
        "bls-small" | "blssmall" | "bls12-381-g2" | "bls12-381-small" => Some(Algorithm::BlsSmall),
        "sm2" => Some(Algorithm::Sm2),
        _ => None,
    };

    parsed
        .ok_or_else(|| PyValueError::new_err(format!("unsupported crypto algorithm `{algorithm}`")))
}

fn parse_err(kind: &str, err: ParseError) -> PyErr {
    PyValueError::new_err(format!("failed to parse {kind} key: {err}"))
}

fn parse_private_key(bytes: &[u8]) -> PyResult<PrivateKey> {
    PrivateKey::from_bytes(Algorithm::Ed25519, bytes).map_err(|err| parse_err("private", err))
}

fn parse_public_key(bytes: &[u8]) -> PyResult<PublicKey> {
    PublicKey::from_bytes(Algorithm::Ed25519, bytes).map_err(|err| parse_err("public", err))
}

fn parse_private_key_for_algorithm(algorithm: Algorithm, bytes: &[u8]) -> PyResult<PrivateKey> {
    PrivateKey::from_bytes(algorithm, bytes).map_err(|err| parse_err("private", err))
}

fn parse_public_key_for_algorithm(algorithm: Algorithm, bytes: &[u8]) -> PyResult<PublicKey> {
    PublicKey::from_bytes(algorithm, bytes).map_err(|err| parse_err("public", err))
}

fn proxy_mode_from_label_py(label: &str) -> PyResult<ProxyMode> {
    ProxyMode::parse(label.trim())
        .ok_or_else(|| PyValueError::new_err("proxy_mode must be 'bridge' or 'metadata-only'"))
}

fn sm2_distid_arg(distid: Option<&str>) -> String {
    distid
        .map(str::to_owned)
        .unwrap_or_else(Sm2PublicKey::default_distid)
}

trait IntoSm2Result {
    fn into_sm2_result(self) -> Result<Sm2PrivateKey, ParseError>;
}

impl IntoSm2Result for Sm2PrivateKey {
    fn into_sm2_result(self) -> Result<Sm2PrivateKey, ParseError> {
        Ok(self)
    }
}

impl IntoSm2Result for Result<Sm2PrivateKey, ParseError> {
    fn into_sm2_result(self) -> Result<Sm2PrivateKey, ParseError> {
        self
    }
}

fn parse_sm2_private_key(distid: Option<&str>, bytes: &[u8]) -> PyResult<Sm2PrivateKey> {
    if bytes.len() != SM2_PRIVATE_KEY_LENGTH {
        return Err(PyValueError::new_err(format!(
            "sm2 private key must be {SM2_PRIVATE_KEY_LENGTH} bytes, got {}",
            bytes.len()
        )));
    }
    let distid = sm2_distid_arg(distid);
    Sm2PrivateKey::from_bytes(distid, bytes)
        .map_err(|err| PyValueError::new_err(format!("failed to parse SM2 private key: {err}")))
}

fn parse_sm2_public_key(distid: Option<&str>, bytes: &[u8]) -> PyResult<Sm2PublicKey> {
    if bytes.len() != SM2_PUBLIC_KEY_UNCOMPRESSED_LENGTH {
        return Err(PyValueError::new_err(format!(
            "sm2 public key must be {SM2_PUBLIC_KEY_UNCOMPRESSED_LENGTH} bytes, got {}",
            bytes.len()
        )));
    }
    let distid = sm2_distid_arg(distid);
    Sm2PublicKey::from_sec1_bytes(distid, bytes)
        .map_err(|err| PyValueError::new_err(format!("failed to parse SM2 public key: {err}")))
}

fn parse_sm2_signature(bytes: &[u8]) -> PyResult<Sm2Signature> {
    let array: [u8; SM2_SIGNATURE_LENGTH] = bytes
        .try_into()
        .map_err(|_| PyValueError::new_err(ERR_SM2_SIGNATURE_LEN))?;
    Sm2Signature::from_bytes(&array)
        .map_err(|err| PyValueError::new_err(format!("invalid SM2 signature: {err}")))
}

fn public_key_to_bytes<'a>(
    public_key: &'a PublicKey,
    context: &'static str,
) -> PyResult<(Algorithm, &'a [u8])> {
    public_key
        .try_to_bytes()
        .map_err(|err| PyValueError::new_err(format!("{context} is malformed: {err}")))
}

fn public_key_multihash_string(
    public_key: &PublicKey,
    prefixed: bool,
    context: &str,
) -> PyResult<String> {
    if prefixed {
        public_key.try_to_prefixed_string()
    } else {
        public_key.try_to_multihash_string()
    }
    .map_err(|err| PyValueError::new_err(format!("failed to format {context}: {err}")))
}

fn private_key_multihash_string(
    private_key: &ExposedPrivateKey,
    prefixed: bool,
    context: &str,
) -> PyResult<String> {
    if prefixed {
        private_key.try_to_prefixed_string()
    } else {
        private_key.try_to_multihash_string()
    }
    .map_err(|err| PyValueError::new_err(format!("failed to format {context}: {err}")))
}

fn keypair_to_py(py: Python<'_>, key_pair: KeyPair) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let (_, public_bytes) = public_key_to_bytes(key_pair.public_key(), "public key")?;
    let (_, mut private_bytes) = key_pair.private_key().to_bytes();

    let public = Py::from(PyBytes::new(py, public_bytes));
    let private = Py::from(PyBytes::new(py, private_bytes.as_slice()));
    private_bytes.fill(0);
    Ok((private, public))
}

fn parse_chain_id(value: &str) -> PyResult<ChainId> {
    ChainId::from_str(value)
        .map_err(|err| PyValueError::new_err(format!("invalid chain id: {err}")))
}

fn parse_account_id(value: &str) -> PyResult<AccountId> {
    AccountId::parse_encoded(value)
        .map(|parsed| parsed.into_account_id())
        .map_err(|err| PyValueError::new_err(format!("invalid account id: {err}")))
}

fn ensure_ed25519_account(account: &AccountId) -> PyResult<()> {
    let (algorithm, _) = public_key_to_bytes(account.signatory(), "account signatory public key")?;
    algorithm_guard(algorithm)
}

fn ensure_allowed_kwargs<'py>(
    kwargs: &Bound<'py, PyDict>,
    allowed: &[&str],
    context: &str,
) -> PyResult<()> {
    for key in kwargs.keys().iter() {
        let key_str = key.extract::<String>().map_err(|_| {
            PyTypeError::new_err(format!("{context} keyword arguments must be strings"))
        })?;
        if !allowed.contains(&key_str.as_str()) {
            return Err(PyTypeError::new_err(format!(
                "{context} got an unexpected keyword argument `{key_str}`"
            )));
        }
    }
    Ok(())
}

fn dict_require<'py, F>(dict: &Bound<'py, PyDict>, key: &str, err: F) -> PyResult<Bound<'py, PyAny>>
where
    F: FnOnce() -> PyErr,
{
    dict.get_item(key)?.ok_or_else(err)
}

#[derive(Debug)]
struct TimeTriggerKwargsParsed<'py> {
    period_ms: Option<u64>,
    repeats: Option<u32>,
    metadata: Option<Bound<'py, PyAny>>,
}

fn parse_time_trigger_kwargs<'py>(
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<TimeTriggerKwargsParsed<'py>> {
    let Some(kwargs) = kwargs else {
        return Ok(TimeTriggerKwargsParsed {
            period_ms: None,
            repeats: None,
            metadata: None,
        });
    };

    ensure_allowed_kwargs(
        kwargs,
        &["period_ms", "repeats", "metadata"],
        "register_time_trigger()",
    )?;

    let period_ms = match kwargs.get_item("period_ms")? {
        Some(value) => Some(value.extract::<u64>()?),
        None => None,
    };
    let repeats = match kwargs.get_item("repeats")? {
        Some(value) => Some(value.extract::<u32>()?),
        None => None,
    };
    let metadata = kwargs.get_item("metadata")?;

    Ok(TimeTriggerKwargsParsed {
        period_ms,
        repeats,
        metadata,
    })
}

fn parse_connect_direction(value: &str) -> PyResult<Dir> {
    match value {
        "AppToWallet" => Ok(Dir::AppToWallet),
        "WalletToApp" => Ok(Dir::WalletToApp),
        other => Err(PyValueError::new_err(format!(
            "invalid connect direction `{other}`"
        ))),
    }
}

fn connect_direction_str(dir: Dir) -> &'static str {
    match dir {
        Dir::AppToWallet => "AppToWallet",
        Dir::WalletToApp => "WalletToApp",
    }
}

fn parse_connect_role(value: &str) -> PyResult<Role> {
    match value {
        "App" => Ok(Role::App),
        "Wallet" => Ok(Role::Wallet),
        other => Err(PyValueError::new_err(format!(
            "invalid connect role `{other}`"
        ))),
    }
}

fn connect_role_str(role: Role) -> &'static str {
    match role {
        Role::App => "App",
        Role::Wallet => "Wallet",
    }
}

fn fixed_array<const N: usize>(bytes: &[u8], context: &str) -> PyResult<[u8; N]> {
    if bytes.len() != N {
        return Err(PyValueError::new_err(format!(
            "{context} must be {N} bytes, got {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(bytes);
    Ok(arr)
}

fn py_text(value: &Bound<'_, PyAny>, context: &str) -> PyResult<String> {
    let text = value
        .extract::<String>()
        .map_err(|_| PyTypeError::new_err(format!("{context} must be a string")))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(PyValueError::new_err(format!(
            "{context} must be non-empty"
        )));
    }
    Ok(trimmed.to_owned())
}

fn py_account_id_list(value: &Bound<'_, PyAny>, context: &str) -> PyResult<Vec<AccountId>> {
    let items = if let Ok(items) = value.cast::<PyList>() {
        items.iter().collect::<Vec<_>>()
    } else if let Ok(items) = value.cast::<PyTuple>() {
        items.iter().collect::<Vec<_>>()
    } else {
        return Err(PyTypeError::new_err(format!(
            "{context} must be a list or tuple"
        )));
    };
    if items.is_empty() {
        return Err(PyValueError::new_err(format!(
            "{context} must be non-empty"
        )));
    }
    if items.len() > iroha_data_model::zk::ZK_ACE_MAX_ALLOWED_ACCOUNTS {
        return Err(PyValueError::new_err(format!(
            "{context} must contain at most {} accounts",
            iroha_data_model::zk::ZK_ACE_MAX_ALLOWED_ACCOUNTS
        )));
    }
    let mut accounts = Vec::with_capacity(items.len());
    let mut seen = HashSet::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let text = py_text(item, &format!("{context}[{index}]"))?;
        let account = parse_account_id(&text).map_err(|err| {
            PyValueError::new_err(format!("invalid {context}[{index}] `{text}`: {err}"))
        })?;
        if !seen.insert(account.clone()) {
            return Err(PyValueError::new_err(format!(
                "{context}[{index}] duplicates an earlier account"
            )));
        }
        accounts.push(account);
    }
    accounts.sort();
    Ok(accounts)
}

fn py_bytes_or_hex(value: &Bound<'_, PyAny>, context: &str) -> PyResult<Vec<u8>> {
    if let Ok(text) = value.extract::<String>() {
        return parse_hex_bytes_py(&text, context);
    }
    value
        .extract::<Vec<u8>>()
        .map_err(|_| PyTypeError::new_err(format!("{context} must be bytes or hex string")))
}

fn py_bytes_or_base64(value: &Bound<'_, PyAny>, context: &str) -> PyResult<Vec<u8>> {
    if let Ok(text) = value.extract::<String>() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(PyValueError::new_err(format!(
                "{context} must be non-empty"
            )));
        }
        return BASE64.decode(trimmed.as_bytes()).map_err(|err| {
            PyValueError::new_err(format!("failed to decode base64 {context}: {err}"))
        });
    }
    value
        .extract::<Vec<u8>>()
        .map_err(|_| PyTypeError::new_err(format!("{context} must be bytes or base64 string")))
}

fn py_fixed_array<const N: usize>(value: &Bound<'_, PyAny>, context: &str) -> PyResult<[u8; N]> {
    let bytes = py_bytes_or_hex(value, context)?;
    fixed_array::<N>(&bytes, context)
}

fn py_fixed_array_list(value: &Bound<'_, PyAny>, context: &str) -> PyResult<Vec<[u8; 32]>> {
    if let Ok(items) = value.cast::<PyList>() {
        let mut parsed = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            parsed.push(py_fixed_array::<32>(&item, &format!("{context}[{index}]"))?);
        }
        return Ok(parsed);
    }
    if let Ok(items) = value.cast::<PyTuple>() {
        let mut parsed = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            parsed.push(py_fixed_array::<32>(&item, &format!("{context}[{index}]"))?);
        }
        return Ok(parsed);
    }
    Err(PyTypeError::new_err(format!(
        "{context} must be a list or tuple"
    )))
}

fn ensure_unique_fixed_arrays(items: &[[u8; 32]], context: &str) -> PyResult<()> {
    let mut seen = HashSet::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if !seen.insert(*item) {
            return Err(PyValueError::new_err(format!(
                "{context}[{index}] duplicates an earlier value"
            )));
        }
    }
    Ok(())
}

fn ensure_non_zero_fixed_array<const N: usize>(item: [u8; N], context: &str) -> PyResult<[u8; N]> {
    if item.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(format!("{context} must be non-zero")));
    }
    Ok(item)
}

fn py_non_zero_fixed_array<const N: usize>(
    value: &Bound<'_, PyAny>,
    context: &str,
) -> PyResult<[u8; N]> {
    let item = py_fixed_array::<N>(value, context)?;
    ensure_non_zero_fixed_array(item, context)
}

fn parse_optional_fixed_array_py<const N: usize>(
    value: Option<&Bound<'_, PyAny>>,
    context: &str,
) -> PyResult<Option<[u8; N]>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    py_fixed_array::<N>(value, context).map(Some)
}

fn dict_get_alias<'py>(
    dict: &Bound<'py, PyDict>,
    aliases: &[&str],
) -> PyResult<Option<Bound<'py, PyAny>>> {
    for alias in aliases {
        if let Some(value) = dict.get_item(alias)?
            && !value.is_none()
        {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn parse_zk_asset_mode(value: Option<&str>) -> PyResult<ZkAssetMode> {
    match value.unwrap_or("Hybrid").trim() {
        "Hybrid" | "hybrid" => Ok(ZkAssetMode::Hybrid),
        "ZkNative" | "zk_native" | "zk-native" | "native" => Ok(ZkAssetMode::ZkNative),
        other => Err(PyValueError::new_err(format!(
            "invalid ZK asset mode `{other}`"
        ))),
    }
}

fn parse_zk_ace_action(value: Option<&str>, context: &str) -> PyResult<String> {
    let action = value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER);
    if action != ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER {
        return Err(PyValueError::new_err(format!(
            "{context} must be {ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER}"
        )));
    }
    Ok(action.to_owned())
}

fn parse_zk_ace_domain_tag(value: Option<&str>, context: &str) -> PyResult<String> {
    let domain_tag = value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG);
    if domain_tag != ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG {
        return Err(PyValueError::new_err(format!(
            "{context} must be {ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG}"
        )));
    }
    Ok(domain_tag.to_owned())
}

fn parse_zk_ace_verifying_key_id_py(
    value: &Bound<'_, PyAny>,
    context: &str,
) -> PyResult<VerifyingKeyId> {
    let vk = parse_required_verifying_key_id_py(Some(value), context)?;
    if vk.backend != ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND {
        return Err(PyValueError::new_err(format!(
            "{context}.backend must be {ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND}"
        )));
    }
    Ok(vk)
}

fn parse_optional_zk_ace_verifying_key_id_py(
    value: Option<&Bound<'_, PyAny>>,
    context: &str,
) -> PyResult<VerifyingKeyId> {
    match value {
        Some(value) if !value.is_none() => parse_zk_ace_verifying_key_id_py(value, context),
        _ => Ok(zk_ace_prover::zk_ace_verifier_key_id(
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        )),
    }
}

fn ensure_zk_ace_proof_attachment(
    proof: ProofAttachment,
    context: &str,
) -> PyResult<ProofAttachment> {
    if proof.backend != ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND {
        return Err(PyValueError::new_err(format!(
            "{context}.backend must be {ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND}"
        )));
    }
    if proof.vk_ref.backend != ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND {
        return Err(PyValueError::new_err(format!(
            "{context}.vk_ref.backend must be {ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND}"
        )));
    }
    Ok(proof)
}

fn zk_ace_proof_attachment_json_value(attachment: &ProofAttachment) -> PyResult<json::Value> {
    let mut vk_ref = json::Map::new();
    vk_ref.insert(
        "backend".to_owned(),
        json::Value::String(attachment.vk_ref.backend.as_str().to_owned()),
    );
    vk_ref.insert(
        "name".to_owned(),
        json::Value::String(attachment.vk_ref.name.clone()),
    );

    let mut proof = json::Map::new();
    proof.insert(
        "backend".to_owned(),
        json::Value::String(attachment.backend.as_str().to_owned()),
    );
    proof.insert("verifying_key_ref".to_owned(), json::Value::Object(vk_ref));
    proof.insert(
        "proof_b64".to_owned(),
        json::Value::String(BASE64.encode(&attachment.proof.bytes)),
    );
    if let Some(commitment) = attachment.vk_commitment {
        proof.insert(
            "verifying_key_commitment".to_owned(),
            json::Value::String(hex_encode(commitment)),
        );
    }
    if let Some(envelope_hash) = attachment.envelope_hash {
        proof.insert(
            "envelope_hash".to_owned(),
            json::Value::String(hex_encode(envelope_hash)),
        );
    }
    Ok(json::Value::Object(proof))
}

fn zk_ace_authorization_json(
    public_inputs: &ZkAcePublicInputsV1,
    proof: &ProofAttachment,
    public_inputs_bytes: &[u8],
) -> PyResult<String> {
    let mut root = json::Map::new();
    root.insert(
        "public_inputs".to_owned(),
        json::to_value(public_inputs)
            .map_err(|err| PyValueError::new_err(format!("serialize public inputs: {err}")))?,
    );
    root.insert(
        "proof".to_owned(),
        zk_ace_proof_attachment_json_value(proof)?,
    );
    root.insert(
        "identity_commitment".to_owned(),
        json::Value::String(hex_encode(public_inputs.identity_commitment)),
    );
    root.insert(
        "tx_digest".to_owned(),
        json::Value::String(hex_encode(public_inputs.tx_digest)),
    );
    root.insert(
        "replay_nullifier".to_owned(),
        json::Value::String(hex_encode(public_inputs.replay_nullifier)),
    );
    root.insert(
        "policy_hash".to_owned(),
        json::Value::String(hex_encode(public_inputs.policy_hash)),
    );
    root.insert(
        "verifier_key_id".to_owned(),
        json::Value::String(format!(
            "{}:{}",
            public_inputs.verifier_key_id.backend.as_str(),
            public_inputs.verifier_key_id.name
        )),
    );
    root.insert(
        "authorization_proof_bytes".to_owned(),
        json::to_value(&proof.proof.bytes.len())
            .map_err(|err| PyValueError::new_err(format!("serialize proof bytes: {err}")))?,
    );
    root.insert(
        "authorization_public_input_bytes".to_owned(),
        json::to_value(&public_inputs_bytes.len())
            .map_err(|err| PyValueError::new_err(format!("serialize public input bytes: {err}")))?,
    );
    root.insert(
        "replay_nullifier_bytes".to_owned(),
        json::to_value(&32usize)
            .map_err(|err| PyValueError::new_err(format!("serialize nullifier bytes: {err}")))?,
    );
    json::to_string(&json::Value::Object(root))
        .map_err(|err| PyValueError::new_err(format!("serialize ZK-ACE authorization: {err}")))
}

fn parse_u128_text(value: &str, context: &str) -> PyResult<u128> {
    value.trim().parse::<u128>().map_err(|err| {
        PyValueError::new_err(format!("{context} must be an unsigned integer: {err}"))
    })
}

fn parse_verifying_key_id_text(value: &str, context: &str) -> PyResult<VerifyingKeyId> {
    let trimmed = value.trim();
    let Some((backend, name)) = trimmed.split_once(':') else {
        return Err(PyValueError::new_err(format!(
            "{context} must use 'backend:name' format"
        )));
    };
    let backend = backend.trim();
    let name = name.trim();
    if backend.is_empty() || name.is_empty() {
        return Err(PyValueError::new_err(format!(
            "{context} must include both backend and name"
        )));
    }
    let backend = Ident::from_str(backend).map_err(|err| {
        PyValueError::new_err(format!("invalid {context} backend identifier: {err}"))
    })?;
    Ok(VerifyingKeyId::new(backend, name))
}

fn parse_verifying_key_id_py(
    value: Option<&Bound<'_, PyAny>>,
    context: &str,
) -> PyResult<Option<VerifyingKeyId>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(text) = value.extract::<String>() {
        return parse_verifying_key_id_text(&text, context).map(Some);
    }
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err(format!("{context} must be a string or mapping")))?;
    let backend = dict_get_alias(dict, &["backend", "backendId"])?
        .ok_or_else(|| PyValueError::new_err(format!("{context}.backend is required")))?;
    let name = dict_get_alias(dict, &["name", "id", "key"])?
        .ok_or_else(|| PyValueError::new_err(format!("{context}.name is required")))?;
    let backend_text = py_text(&backend, &format!("{context}.backend"))?;
    let name_text = py_text(&name, &format!("{context}.name"))?;
    let backend = Ident::from_str(&backend_text).map_err(|err| {
        PyValueError::new_err(format!("invalid {context} backend identifier: {err}"))
    })?;
    Ok(Some(VerifyingKeyId::new(backend, name_text)))
}

fn parse_required_verifying_key_id_py(
    value: Option<&Bound<'_, PyAny>>,
    context: &str,
) -> PyResult<VerifyingKeyId> {
    parse_verifying_key_id_py(value, context)?
        .ok_or_else(|| PyValueError::new_err(format!("{context} is required")))
}

fn parse_optional_root_hint(
    value: Option<&Bound<'_, PyAny>>,
    context: &str,
) -> PyResult<Option<[u8; 32]>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    py_fixed_array::<32>(value, context).map(Some)
}

fn parse_zk_proof_attachment(value: &Bound<'_, PyAny>, context: &str) -> PyResult<ProofAttachment> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err(format!("{context} must be a mapping")))?;
    let backend_value = dict_get_alias(dict, &["backend", "proof_backend", "proofBackend"])?
        .ok_or_else(|| PyValueError::new_err(format!("{context}.backend is required")))?;
    let backend_text = py_text(&backend_value, &format!("{context}.backend"))?;
    let backend = Ident::from_str(&backend_text).map_err(|err| {
        PyValueError::new_err(format!("invalid {context} backend identifier: {err}"))
    })?;
    let proof_value = dict_get_alias(
        dict,
        &[
            "proof_bytes",
            "proofBytes",
            "proof_b64",
            "proofB64",
            "proofBase64",
            "proof",
        ],
    )?
    .ok_or_else(|| PyValueError::new_err(format!("{context}.proof_bytes is required")))?;
    let proof_bytes = py_bytes_or_base64(&proof_value, &format!("{context}.proof_bytes"))?;
    if proof_bytes.is_empty() {
        return Err(PyValueError::new_err(format!(
            "{context}.proof_bytes must be non-empty"
        )));
    }
    let vk_value = dict_get_alias(
        dict,
        &[
            "verifying_key_ref",
            "verifyingKeyRef",
            "vk_ref",
            "vkRef",
            "verifying_key",
        ],
    )?;
    let vk_ref =
        parse_required_verifying_key_id_py(vk_value.as_ref(), &format!("{context}.vk_ref"))?;
    if vk_ref.backend != backend {
        return Err(PyValueError::new_err(format!(
            "{context}.vk_ref.backend must match {context}.backend"
        )));
    }
    let mut attachment =
        ProofAttachment::new_ref(backend.clone(), ProofBox::new(backend, proof_bytes), vk_ref);
    if let Some(commitment) = dict_get_alias(
        dict,
        &[
            "verifying_key_commitment",
            "verifyingKeyCommitment",
            "vk_commitment",
            "vkCommitment",
        ],
    )? && !commitment.is_none()
    {
        attachment.vk_commitment = Some(py_fixed_array::<32>(
            &commitment,
            &format!("{context}.verifying_key_commitment"),
        )?);
    }
    if let Some(envelope_hash) = dict_get_alias(dict, &["envelope_hash", "envelopeHash"])?
        && !envelope_hash.is_none()
    {
        attachment.envelope_hash = Some(py_fixed_array::<32>(
            &envelope_hash,
            &format!("{context}.envelope_hash"),
        )?);
    }
    Ok(attachment)
}

fn extract_optional_dict<'py>(
    value: Option<Bound<'py, PyAny>>,
    context: &str,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    match value {
        Some(obj) => {
            if obj.is_none() {
                Ok(None)
            } else {
                let dict = obj
                    .cast_into::<PyDict>()
                    .map_err(|_| PyTypeError::new_err(format!("{context} must be a mapping")))?;
                Ok(Some(dict))
            }
        }
        None => Ok(None),
    }
}

fn optional_string(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    match value {
        Some(obj) if !obj.is_none() => obj.extract::<String>().map(Some),
        _ => Ok(None),
    }
}

fn parse_string_list(value: Option<Bound<'_, PyAny>>, context: &str) -> PyResult<Vec<String>> {
    match value {
        Some(obj) if !obj.is_none() => obj
            .extract::<Vec<String>>()
            .map_err(|_| PyTypeError::new_err(format!("{context} must be a sequence of strings"))),
        _ => Ok(Vec::new()),
    }
}

fn parse_permissions(
    value: Option<Bound<'_, PyAny>>,
    context: &str,
) -> PyResult<Option<PermissionsV1>> {
    let Some(mapping) = extract_optional_dict(value, context)? else {
        return Ok(None);
    };
    let methods = parse_string_list(mapping.get_item("methods")?, "permissions.methods")?;
    let events = parse_string_list(mapping.get_item("events")?, "permissions.events")?;
    let resources = match mapping.get_item("resources")? {
        Some(obj) if !obj.is_none() => Some(obj.extract::<Vec<String>>().map_err(|_| {
            PyTypeError::new_err("permissions.resources must be a sequence of strings")
        })?),
        _ => None,
    };
    Ok(Some(PermissionsV1 {
        methods,
        events,
        resources,
    }))
}

fn parse_app_metadata(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<AppMeta>> {
    let Some(mapping) = extract_optional_dict(value, "metadata")? else {
        return Ok(None);
    };
    let name = dict_require(&mapping, "name", || {
        PyValueError::new_err("metadata.name is required")
    })?
    .extract::<String>()?;
    let url = optional_string(mapping.get_item("url")?)?;
    let icon_hash = optional_string(mapping.get_item("icon_hash")?)?;
    Ok(Some(AppMeta {
        name,
        url,
        icon_hash,
    }))
}

fn parse_sign_in_proof(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<SignInProofV1>> {
    let Some(mapping) = extract_optional_dict(value, "proof")? else {
        return Ok(None);
    };
    let domain = dict_require(&mapping, "domain", || {
        PyValueError::new_err("proof.domain is required")
    })?
    .extract::<String>()?;
    let uri = dict_require(&mapping, "uri", || {
        PyValueError::new_err("proof.uri is required")
    })?
    .extract::<String>()?;
    let statement = dict_require(&mapping, "statement", || {
        PyValueError::new_err("proof.statement is required")
    })?
    .extract::<String>()?;
    let issued_at = dict_require(&mapping, "issued_at", || {
        PyValueError::new_err("proof.issued_at is required")
    })?
    .extract::<String>()?;
    let nonce = dict_require(&mapping, "nonce", || {
        PyValueError::new_err("proof.nonce is required")
    })?
    .extract::<String>()?;
    Ok(Some(SignInProofV1 {
        domain,
        uri,
        statement,
        issued_at,
        nonce,
    }))
}

fn parse_wallet_signature(fields: &Bound<'_, PyDict>) -> PyResult<WalletSignatureV1> {
    let sig_bytes = dict_require(fields, "signature", || {
        PyValueError::new_err("approve.signature is required")
    })?
    .extract::<Vec<u8>>()?;
    let sig = fixed_array::<64>(&sig_bytes, "signature")?;
    let algorithm = match fields.get_item("algorithm")? {
        Some(value) if !value.is_none() => {
            let alg_str = value.extract::<String>()?;
            Algorithm::from_str(&alg_str).map_err(|_| {
                PyValueError::new_err(format!(
                    "unsupported connect signature algorithm `{alg_str}`"
                ))
            })?
        }
        _ => Algorithm::Ed25519,
    };
    Ok(WalletSignatureV1::new(
        algorithm,
        Signature::from_bytes(&sig),
    ))
}

fn parse_connect_control(fields: &Bound<'_, PyDict>) -> PyResult<ConnectControlV1> {
    let control_type = dict_require(fields, "control_type", || {
        PyValueError::new_err("connect frame control requires `control_type`")
    })?
    .extract::<String>()?;
    let fields_obj = dict_require(fields, "fields", || {
        PyValueError::new_err("connect frame control requires `fields`")
    })?;
    let payload = fields_obj
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("connect frame control `fields` must be a dict"))?;
    match control_type.as_str() {
        "Open" => {
            let pk_bytes = dict_require(payload, "app_public_key", || {
                PyValueError::new_err("open.app_public_key is required")
            })?
            .extract::<Vec<u8>>()?;
            let app_pk = fixed_array::<32>(&pk_bytes, "app_public_key")?;
            let chain_id = dict_require(payload, "chain_id", || {
                PyValueError::new_err("open.chain_id is required")
            })?
            .extract::<String>()?;
            let metadata = {
                let value = payload.get_item("metadata")?;
                parse_app_metadata(value)?
            };
            let permissions = {
                let value = payload.get_item("permissions")?;
                parse_permissions(value, "permissions")?
            };
            Ok(ConnectControlV1::Open {
                app_pk,
                app_meta: metadata,
                constraints: Constraints { chain_id },
                permissions,
            })
        }
        "Approve" => {
            let pk_bytes = dict_require(payload, "wallet_public_key", || {
                PyValueError::new_err("approve.wallet_public_key is required")
            })?
            .extract::<Vec<u8>>()?;
            let wallet_pk = fixed_array::<32>(&pk_bytes, "wallet_public_key")?;
            let account_id = dict_require(payload, "account_id", || {
                PyValueError::new_err("approve.account_id is required")
            })?
            .extract::<String>()?;
            let permissions = {
                let value = payload.get_item("permissions")?;
                parse_permissions(value, "permissions")?
            };
            let proof = {
                let value = payload.get_item("proof")?;
                parse_sign_in_proof(value)?
            };
            let sig_wallet = parse_wallet_signature(payload)?;
            Ok(ConnectControlV1::Approve {
                wallet_pk,
                account_id,
                permissions,
                proof,
                sig_wallet,
            })
        }
        "Reject" => {
            let code = dict_require(payload, "code", || {
                PyValueError::new_err("reject.code is required")
            })?
            .extract::<u16>()?;
            let code_id = dict_require(payload, "code_id", || {
                PyValueError::new_err("reject.code_id is required")
            })?
            .extract::<String>()?;
            let reason = dict_require(payload, "reason", || {
                PyValueError::new_err("reject.reason is required")
            })?
            .extract::<String>()?;
            Ok(ConnectControlV1::Reject {
                code,
                code_id,
                reason,
            })
        }
        "Close" => {
            let role = dict_require(payload, "role", || {
                PyValueError::new_err("close.role is required")
            })?
            .extract::<String>()?;
            let who = parse_connect_role(&role)?;
            let code = dict_require(payload, "code", || {
                PyValueError::new_err("close.code is required")
            })?
            .extract::<u16>()?;
            let reason = dict_require(payload, "reason", || {
                PyValueError::new_err("close.reason is required")
            })?
            .extract::<String>()?;
            let retryable = dict_require(payload, "retryable", || {
                PyValueError::new_err("close.retryable is required")
            })?
            .extract::<bool>()?;
            Ok(ConnectControlV1::Close {
                who,
                code,
                reason,
                retryable,
            })
        }
        "Ping" => {
            let nonce = dict_require(payload, "nonce", || {
                PyValueError::new_err("ping.nonce is required")
            })?
            .extract::<u64>()?;
            Ok(ConnectControlV1::Ping { nonce })
        }
        "Pong" => {
            let nonce = dict_require(payload, "nonce", || {
                PyValueError::new_err("pong.nonce is required")
            })?
            .extract::<u64>()?;
            Ok(ConnectControlV1::Pong { nonce })
        }
        other => Err(PyValueError::new_err(format!(
            "unsupported connect control variant `{other}`"
        ))),
    }
}

fn parse_frame_kind(kind: &Bound<'_, PyDict>) -> PyResult<FrameKind> {
    let kind_type = dict_require(kind, "type", || {
        PyValueError::new_err("connect frame kind requires `type`")
    })?
    .extract::<String>()?;
    match kind_type.as_str() {
        "Control" => Ok(FrameKind::Control(parse_connect_control(kind)?)),
        "Ciphertext" => {
            let fields_obj = dict_require(kind, "fields", || {
                PyValueError::new_err("ciphertext frame requires `fields`")
            })?;
            let payload = fields_obj
                .cast::<PyDict>()
                .map_err(|_| PyTypeError::new_err("ciphertext fields must be a dict"))?;
            let direction = dict_require(payload, "direction", || {
                PyValueError::new_err("ciphertext.direction is required")
            })?
            .extract::<String>()?;
            let dir = parse_connect_direction(&direction)?;
            let aead = dict_require(payload, "aead", || {
                PyValueError::new_err("ciphertext.aead is required")
            })?
            .extract::<Vec<u8>>()?;
            Ok(FrameKind::Ciphertext(ConnectCiphertextV1 { dir, aead }))
        }
        other => Err(PyValueError::new_err(format!(
            "unsupported connect frame kind `{other}`"
        ))),
    }
}

fn parse_control_after_key(
    fields: &Bound<'_, PyDict>,
    variant: &str,
) -> PyResult<ControlAfterKeyV1> {
    match variant {
        "Close" => {
            let who_str = dict_require(fields, "who", || {
                PyValueError::new_err("close.who is required")
            })?
            .extract::<String>()?;
            let who = parse_connect_role(&who_str)?;
            let code = dict_require(fields, "code", || {
                PyValueError::new_err("close.code is required")
            })?
            .extract::<u16>()?;
            let reason = dict_require(fields, "reason", || {
                PyValueError::new_err("close.reason is required")
            })?
            .extract::<String>()?;
            let retryable = dict_require(fields, "retryable", || {
                PyValueError::new_err("close.retryable is required")
            })?
            .extract::<bool>()?;
            Ok(ControlAfterKeyV1::Close {
                who,
                code,
                reason,
                retryable,
            })
        }
        "Reject" => {
            let code = dict_require(fields, "code", || {
                PyValueError::new_err("reject.code is required")
            })?
            .extract::<u16>()?;
            let code_id = dict_require(fields, "code_id", || {
                PyValueError::new_err("reject.code_id is required")
            })?
            .extract::<String>()?;
            let reason = dict_require(fields, "reason", || {
                PyValueError::new_err("reject.reason is required")
            })?
            .extract::<String>()?;
            Ok(ControlAfterKeyV1::Reject {
                code,
                code_id,
                reason,
            })
        }
        other => Err(PyValueError::new_err(format!(
            "unsupported encrypted control variant `{other}`"
        ))),
    }
}

fn encode_wallet_signature_dict<'py>(
    py: Python<'py>,
    sig: &WalletSignatureV1,
) -> PyResult<Py<PyAny>> {
    let mapping = PyDict::new(py);
    mapping.set_item("algorithm", sig.algorithm.to_string())?;
    mapping.set_item("signature", PyBytes::new(py, sig.signature.payload()))?;
    Ok(mapping.into_any().unbind())
}

fn parse_connect_payload(payload: &Bound<'_, PyDict>) -> PyResult<ConnectPayloadV1> {
    let payload_type = dict_require(payload, "type", || {
        PyValueError::new_err("connect payload requires `type`")
    })?
    .extract::<String>()?;
    match payload_type.as_str() {
        "Control" => {
            let variant = dict_require(payload, "variant", || {
                PyValueError::new_err("control payload requires `variant`")
            })?
            .extract::<String>()?;
            let fields_obj = dict_require(payload, "fields", || {
                PyValueError::new_err("control payload requires `fields`")
            })?;
            let fields = fields_obj
                .cast::<PyDict>()
                .map_err(|_| PyTypeError::new_err("control payload `fields` must be a dict"))?;
            let control = parse_control_after_key(fields, &variant)?;
            Ok(ConnectPayloadV1::Control(control))
        }
        "SignRequestRaw" => {
            let domain_tag = dict_require(payload, "domain_tag", || {
                PyValueError::new_err("SignRequestRaw.domain_tag is required")
            })?
            .extract::<String>()?;
            let bytes = dict_require(payload, "bytes", || {
                PyValueError::new_err("SignRequestRaw.bytes is required")
            })?
            .extract::<Vec<u8>>()?;
            Ok(ConnectPayloadV1::SignRequestRaw { domain_tag, bytes })
        }
        "SignRequestTx" => {
            let tx_bytes = dict_require(payload, "tx_bytes", || {
                PyValueError::new_err("SignRequestTx.tx_bytes is required")
            })?
            .extract::<Vec<u8>>()?;
            Ok(ConnectPayloadV1::SignRequestTx { tx_bytes })
        }
        "SignResultOk" => {
            let signature_obj = dict_require(payload, "signature", || {
                PyValueError::new_err("SignResultOk.signature is required")
            })?;
            let signature_dict = signature_obj
                .cast::<PyDict>()
                .map_err(|_| PyTypeError::new_err("SignResultOk.signature must be a dict"))?;
            let signature = parse_wallet_signature(signature_dict)?;
            Ok(ConnectPayloadV1::SignResultOk { signature })
        }
        "SignResultErr" => {
            let code = dict_require(payload, "code", || {
                PyValueError::new_err("SignResultErr.code is required")
            })?
            .extract::<String>()?;
            let message = dict_require(payload, "message", || {
                PyValueError::new_err("SignResultErr.message is required")
            })?
            .extract::<String>()?;
            Ok(ConnectPayloadV1::SignResultErr { code, message })
        }
        "DisplayRequest" => {
            let title = dict_require(payload, "title", || {
                PyValueError::new_err("DisplayRequest.title is required")
            })?
            .extract::<String>()?;
            let body = dict_require(payload, "body", || {
                PyValueError::new_err("DisplayRequest.body is required")
            })?
            .extract::<String>()?;
            Ok(ConnectPayloadV1::DisplayRequest { title, body })
        }
        other => Err(PyValueError::new_err(format!(
            "unsupported connect payload type `{other}`"
        ))),
    }
}

fn encode_connect_payload<'py>(py: Python<'py>, payload: &ConnectPayloadV1) -> PyResult<Py<PyAny>> {
    let mapping = PyDict::new(py);
    match payload {
        ConnectPayloadV1::Control(control) => {
            mapping.set_item("type", "Control")?;
            match control {
                ControlAfterKeyV1::Close {
                    who,
                    code,
                    reason,
                    retryable,
                } => {
                    mapping.set_item("variant", "Close")?;
                    let fields = PyDict::new(py);
                    fields.set_item("who", connect_role_str(*who))?;
                    fields.set_item("code", code)?;
                    fields.set_item("reason", reason)?;
                    fields.set_item("retryable", retryable)?;
                    mapping.set_item("fields", fields)?;
                }
                ControlAfterKeyV1::Reject {
                    code,
                    code_id,
                    reason,
                } => {
                    mapping.set_item("variant", "Reject")?;
                    let fields = PyDict::new(py);
                    fields.set_item("code", code)?;
                    fields.set_item("code_id", code_id)?;
                    fields.set_item("reason", reason)?;
                    mapping.set_item("fields", fields)?;
                }
            }
        }
        ConnectPayloadV1::SignRequestRaw { domain_tag, bytes } => {
            mapping.set_item("type", "SignRequestRaw")?;
            mapping.set_item("domain_tag", domain_tag)?;
            mapping.set_item("bytes", PyBytes::new(py, bytes))?;
        }
        ConnectPayloadV1::SignRequestTx { tx_bytes } => {
            mapping.set_item("type", "SignRequestTx")?;
            mapping.set_item("tx_bytes", PyBytes::new(py, tx_bytes))?;
        }
        ConnectPayloadV1::SignResultOk { signature } => {
            mapping.set_item("type", "SignResultOk")?;
            let sig_dict = encode_wallet_signature_dict(py, signature)?;
            mapping.set_item("signature", sig_dict)?;
        }
        ConnectPayloadV1::SignResultErr { code, message } => {
            mapping.set_item("type", "SignResultErr")?;
            mapping.set_item("code", code)?;
            mapping.set_item("message", message)?;
        }
        ConnectPayloadV1::DisplayRequest { title, body } => {
            mapping.set_item("type", "DisplayRequest")?;
            mapping.set_item("title", title)?;
            mapping.set_item("body", body)?;
        }
    }
    Ok(mapping.into_any().unbind())
}

fn encode_permissions_dict<'py>(py: Python<'py>, perms: &Option<PermissionsV1>) -> Py<PyAny> {
    match perms {
        Some(p) => {
            let mapping = PyDict::new(py);
            mapping
                .set_item("methods", &p.methods)
                .expect("set methods");
            mapping.set_item("events", &p.events).expect("set events");
            match &p.resources {
                Some(resources) => {
                    mapping
                        .set_item("resources", resources)
                        .expect("set resources");
                }
                None => {
                    mapping.set_item("resources", py.None()).expect("set none");
                }
            }
            mapping.into_any().unbind()
        }
        None => py.None(),
    }
}

fn encode_app_meta_dict<'py>(py: Python<'py>, meta: &Option<AppMeta>) -> Py<PyAny> {
    match meta {
        Some(value) => {
            let mapping = PyDict::new(py);
            mapping.set_item("name", &value.name).expect("set");
            match &value.url {
                Some(url) => {
                    mapping.set_item("url", url).expect("set");
                }
                None => {
                    mapping.set_item("url", py.None()).expect("set none");
                }
            }
            match &value.icon_hash {
                Some(hash) => {
                    mapping.set_item("icon_hash", hash).expect("set");
                }
                None => {
                    mapping.set_item("icon_hash", py.None()).expect("set none");
                }
            }
            mapping.into_any().unbind()
        }
        None => py.None(),
    }
}

fn encode_proof_dict<'py>(py: Python<'py>, proof: &Option<SignInProofV1>) -> Py<PyAny> {
    match proof {
        Some(value) => {
            let mapping = PyDict::new(py);
            mapping.set_item("domain", &value.domain).expect("set");
            mapping.set_item("uri", &value.uri).expect("set");
            mapping
                .set_item("statement", &value.statement)
                .expect("set");
            mapping
                .set_item("issued_at", &value.issued_at)
                .expect("set");
            mapping.set_item("nonce", &value.nonce).expect("set");
            mapping.into_any().unbind()
        }
        None => py.None(),
    }
}

fn encode_frame_kind(py: Python<'_>, kind: &FrameKind) -> PyResult<Py<PyDict>> {
    let mapping = PyDict::new(py);
    match kind {
        FrameKind::Control(control) => {
            mapping.set_item("type", "Control")?;
            match control {
                ConnectControlV1::Open {
                    app_pk,
                    app_meta,
                    constraints,
                    permissions,
                } => {
                    mapping.set_item("control_type", "Open")?;
                    let fields = PyDict::new(py);
                    fields.set_item("app_public_key", PyBytes::new(py, app_pk))?;
                    fields.set_item("chain_id", &constraints.chain_id)?;
                    fields.set_item("metadata", encode_app_meta_dict(py, app_meta))?;
                    fields.set_item("permissions", encode_permissions_dict(py, permissions))?;
                    mapping.set_item("fields", fields)?;
                }
                ConnectControlV1::Approve {
                    wallet_pk,
                    account_id,
                    permissions,
                    proof,
                    sig_wallet,
                } => {
                    mapping.set_item("control_type", "Approve")?;
                    let fields = PyDict::new(py);
                    fields.set_item("wallet_public_key", PyBytes::new(py, wallet_pk))?;
                    fields.set_item("account_id", account_id)?;
                    fields.set_item("permissions", encode_permissions_dict(py, permissions))?;
                    fields.set_item("proof", encode_proof_dict(py, proof))?;
                    fields.set_item(
                        "signature",
                        PyBytes::new(py, sig_wallet.signature.payload()),
                    )?;
                    fields.set_item("algorithm", sig_wallet.algorithm.as_static_str())?;
                    mapping.set_item("fields", fields)?;
                }
                ConnectControlV1::Reject {
                    code,
                    code_id,
                    reason,
                } => {
                    mapping.set_item("control_type", "Reject")?;
                    let fields = PyDict::new(py);
                    fields.set_item("code", code)?;
                    fields.set_item("code_id", code_id)?;
                    fields.set_item("reason", reason)?;
                    mapping.set_item("fields", fields)?;
                }
                ConnectControlV1::Close {
                    who,
                    code,
                    reason,
                    retryable,
                } => {
                    mapping.set_item("control_type", "Close")?;
                    let fields = PyDict::new(py);
                    fields.set_item("role", connect_role_str(*who))?;
                    fields.set_item("code", code)?;
                    fields.set_item("reason", reason)?;
                    fields.set_item("retryable", retryable)?;
                    mapping.set_item("fields", fields)?;
                }
                ConnectControlV1::Ping { nonce } => {
                    mapping.set_item("control_type", "Ping")?;
                    let fields = PyDict::new(py);
                    fields.set_item("nonce", nonce)?;
                    mapping.set_item("fields", fields)?;
                }
                ConnectControlV1::Pong { nonce } => {
                    mapping.set_item("control_type", "Pong")?;
                    let fields = PyDict::new(py);
                    fields.set_item("nonce", nonce)?;
                    mapping.set_item("fields", fields)?;
                }
                ConnectControlV1::ServerEvent { event } => {
                    mapping.set_item("control_type", "ServerEvent")?;
                    let fields = PyDict::new(py);
                    match event {
                        ServerEventV1::BlockProofs {
                            height,
                            entry_hash,
                            proofs_json,
                        } => {
                            fields.set_item("event_type", "BlockProofs")?;
                            fields.set_item("height", height)?;
                            fields.set_item("entry_hash", entry_hash)?;
                            fields.set_item("proofs_json", proofs_json)?;
                        }
                    }
                    mapping.set_item("fields", fields)?;
                }
            }
        }
        FrameKind::Ciphertext(ct) => {
            mapping.set_item("type", "Ciphertext")?;
            let fields = PyDict::new(py);
            fields.set_item("direction", connect_direction_str(ct.dir))?;
            fields.set_item("aead", PyBytes::new(py, &ct.aead))?;
            mapping.set_item("fields", fields)?;
        }
    }
    Ok(mapping.unbind())
}

fn decode_connect_frame_bytes(bytes: &[u8]) -> PyResult<ConnectFrameV1> {
    let (frame, used) = norito::core::decode_field_canonical::<ConnectFrameV1>(bytes)
        .map_err(|err| PyValueError::new_err(format!("failed to decode connect frame: {err}")))?;
    if used != bytes.len() {
        return Err(PyValueError::new_err(
            "connect frame payload contains trailing bytes",
        ));
    }
    Ok(frame)
}

fn sorafs_default_policy() -> AliasCachePolicy {
    AliasCachePolicy::new(
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS),
        Duration::from_secs(defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS),
    )
}

fn policy_override_u64<'py>(
    overrides: &Bound<'py, PyDict>,
    keys: &[&str],
    context: &str,
) -> PyResult<Option<u64>> {
    for key in keys {
        if let Some(value) = overrides.get_item(*key)? {
            if value.is_none() {
                return Ok(None);
            }
            let secs: u64 = value.extract().map_err(|_| {
                PyValueError::new_err(format!("{context} must be a positive integer"))
            })?;
            if secs == 0 {
                return Err(PyValueError::new_err(format!(
                    "{context} must be greater than zero"
                )));
            }
            return Ok(Some(secs));
        }
    }
    Ok(None)
}

fn alias_policy_from_py(overrides: Option<&Bound<'_, PyDict>>) -> PyResult<AliasCachePolicy> {
    let defaults = sorafs_default_policy();
    let mut positive = defaults.positive_ttl().as_secs();
    let mut refresh = defaults.refresh_window().as_secs();
    let mut hard = defaults.hard_expiry().as_secs();
    let mut negative = defaults.negative_ttl().as_secs();
    let mut revocation = defaults.revocation_ttl().as_secs();
    let mut rotation = defaults.rotation_max_age().as_secs();
    let mut successor = defaults.successor_grace().as_secs();
    let mut governance = defaults.governance_grace().as_secs();

    if let Some(mapping) = overrides {
        if let Some(value) = policy_override_u64(
            mapping,
            &["positive_ttl_secs", "positiveTtlSecs"],
            "positive_ttl_secs",
        )? {
            positive = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["refresh_window_secs", "refreshWindowSecs"],
            "refresh_window_secs",
        )? {
            refresh = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["hard_expiry_secs", "hardExpirySecs"],
            "hard_expiry_secs",
        )? {
            hard = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["negative_ttl_secs", "negativeTtlSecs"],
            "negative_ttl_secs",
        )? {
            negative = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["revocation_ttl_secs", "revocationTtlSecs"],
            "revocation_ttl_secs",
        )? {
            revocation = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["rotation_max_age_secs", "rotationMaxAgeSecs"],
            "rotation_max_age_secs",
        )? {
            rotation = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["successor_grace_secs", "successorGraceSecs"],
            "successor_grace_secs",
        )? {
            successor = value;
        }
        if let Some(value) = policy_override_u64(
            mapping,
            &["governance_grace_secs", "governanceGraceSecs"],
            "governance_grace_secs",
        )? {
            governance = value;
        }
    }

    if refresh > positive {
        return Err(PyValueError::new_err(
            "refresh_window_secs must not exceed positive_ttl_secs",
        ));
    }
    if hard < positive {
        return Err(PyValueError::new_err(
            "hard_expiry_secs must be greater than or equal to positive_ttl_secs",
        ));
    }

    Ok(AliasCachePolicy::new(
        Duration::from_secs(positive),
        Duration::from_secs(refresh),
        Duration::from_secs(hard),
        Duration::from_secs(negative),
        Duration::from_secs(revocation),
        Duration::from_secs(rotation),
        Duration::from_secs(successor),
        Duration::from_secs(governance),
    ))
}

fn alias_policy_to_dict(py: Python<'_>, policy: &AliasCachePolicy) -> PyResult<Py<PyDict>> {
    let mapping = PyDict::new(py);
    mapping.set_item("positive_ttl_secs", policy.positive_ttl().as_secs())?;
    mapping.set_item("refresh_window_secs", policy.refresh_window().as_secs())?;
    mapping.set_item("hard_expiry_secs", policy.hard_expiry().as_secs())?;
    mapping.set_item("negative_ttl_secs", policy.negative_ttl().as_secs())?;
    mapping.set_item("revocation_ttl_secs", policy.revocation_ttl().as_secs())?;
    mapping.set_item("rotation_max_age_secs", policy.rotation_max_age().as_secs())?;
    mapping.set_item("successor_grace_secs", policy.successor_grace().as_secs())?;
    mapping.set_item("governance_grace_secs", policy.governance_grace().as_secs())?;
    Ok(mapping.unbind())
}

fn parse_hex_bytes_py(input: &str, context: &str) -> PyResult<Vec<u8>> {
    let trimmed = input.trim_start_matches("0x");
    if !trimmed.len().is_multiple_of(2) {
        return Err(PyValueError::new_err(format!(
            "{context} must contain an even number of hex characters"
        )));
    }
    hex::decode(trimmed)
        .map_err(|err| PyValueError::new_err(format!("failed to decode {context}: {err}")))
}

#[pyfunction]
#[pyo3(name = "sorafs_alias_policy_defaults")]
fn sorafs_alias_policy_defaults_py(py: Python<'_>) -> PyResult<Py<PyDict>> {
    let policy = sorafs_default_policy();
    alias_policy_to_dict(py, &policy)
}

#[pyfunction]
#[pyo3(
    name = "sorafs_evaluate_alias_proof",
    signature = (proof_b64, policy_overrides=None, now_secs=None)
)]
fn sorafs_evaluate_alias_proof_py(
    py: Python<'_>,
    proof_b64: &str,
    policy_overrides: Option<&Bound<'_, PyDict>>,
    now_secs: Option<u64>,
) -> PyResult<Py<PyDict>> {
    let trimmed = proof_b64.trim();
    if trimmed.is_empty() {
        return Err(PyValueError::new_err("proof must not be empty"));
    }

    let policy = alias_policy_from_py(policy_overrides)?;
    let now = now_secs.unwrap_or_else(unix_now_secs);
    let proof_bytes = BASE64.decode(trimmed.as_bytes()).map_err(|err| {
        PyValueError::new_err(format!("failed to decode base64 alias proof: {err}"))
    })?;
    let bundle = decode_alias_proof(&proof_bytes)
        .map_err(|err| PyValueError::new_err(format!("invalid alias proof bundle: {err}")))?;
    let evaluation = policy.evaluate(&bundle, now);
    let state_label = match evaluation.state {
        AliasProofState::Fresh => "fresh",
        AliasProofState::RefreshWindow => "refresh_window",
        AliasProofState::Expired => "expired",
        AliasProofState::HardExpired => "hard_expired",
    };

    let dict = PyDict::new(py);
    dict.set_item("state", state_label)?;
    dict.set_item("status_label", evaluation.status_label())?;
    dict.set_item("rotation_due", evaluation.rotation_due)?;
    dict.set_item("age_seconds", evaluation.age.as_secs())?;
    dict.set_item("generated_at_unix", evaluation.generated_at_unix)?;
    dict.set_item("expires_at_unix", evaluation.expires_at_unix)?;
    if let Some(remain) = evaluation.expires_in {
        dict.set_item("expires_in_seconds", Some(remain.as_secs()))?;
    } else {
        dict.set_item("expires_in_seconds", Option::<u64>::None)?;
    }
    dict.set_item("servable", evaluation.state.is_servable())?;
    Ok(dict.unbind())
}

#[pyfunction]
#[pyo3(name = "sorafs_alias_proof_fixture", signature = (options=None))]
fn sorafs_alias_proof_fixture_py(
    py: Python<'_>,
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyDict>> {
    let mapping = options;
    let alias = if let Some(opts) = mapping {
        if let Some(value) = opts.get_item("alias")? {
            value
                .extract::<String>()
                .map_err(|_| PyValueError::new_err("alias must be a string when provided"))?
        } else {
            "docs/sora".to_owned()
        }
    } else {
        "docs/sora".to_owned()
    };

    let manifest_cid = if let Some(opts) = mapping {
        if let Some(value) = opts.get_item("manifest_cid_hex")? {
            let hex_str = value.extract::<String>().map_err(|_| {
                PyValueError::new_err("manifest_cid_hex must be a string when provided")
            })?;
            parse_hex_bytes_py(&hex_str, "manifest_cid_hex")?
        } else {
            vec![0xAA, 0xBB]
        }
    } else {
        vec![0xAA, 0xBB]
    };
    if manifest_cid.is_empty() {
        return Err(PyValueError::new_err(
            "manifest_cid_hex must not decode to an empty value",
        ));
    }

    let now = unix_now_secs();
    let generated = if let Some(opts) = mapping {
        if let Some(value) = opts.get_item("generated_at_unix")? {
            let secs: u64 = value.extract().map_err(|_| {
                PyValueError::new_err("generated_at_unix must be a non-negative integer")
            })?;
            secs
        } else {
            now.saturating_sub(60)
        }
    } else {
        now.saturating_sub(60)
    };

    let expires_default = generated + defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS;
    let expires = if let Some(opts) = mapping {
        if let Some(value) = opts.get_item("expires_at_unix")? {
            let secs: u64 = value.extract().map_err(|_| {
                PyValueError::new_err("expires_at_unix must be a non-negative integer")
            })?;
            secs
        } else {
            expires_default
        }
    } else {
        expires_default
    };

    if expires <= generated {
        return Err(PyValueError::new_err(
            "expires_at_unix must be greater than generated_at_unix",
        ));
    }

    let bound_at = if let Some(opts) = mapping {
        if let Some(value) = opts.get_item("bound_at_epoch")? {
            value.extract::<u64>().map_err(|_| {
                PyValueError::new_err("bound_at_epoch must be a non-negative integer")
            })?
        } else {
            1
        }
    } else {
        1
    };

    let expiry_epoch = if let Some(opts) = mapping {
        if let Some(value) = opts.get_item("expiry_epoch")? {
            value
                .extract::<u64>()
                .map_err(|_| PyValueError::new_err("expiry_epoch must be a non-negative integer"))?
        } else {
            bound_at + 100
        }
    } else {
        bound_at + 100
    };

    let binding = AliasBindingV1 {
        alias: alias.clone(),
        manifest_cid,
        bound_at,
        expiry_epoch,
    };

    let mut bundle = AliasProofBundleV1 {
        binding,
        registry_root: [0u8; 32],
        registry_height: 1,
        generated_at_unix: generated,
        expires_at_unix: expires,
        merkle_path: Vec::new(),
        council_signatures: Vec::new(),
    };

    let root = alias_merkle_root(&bundle.binding, &bundle.merkle_path)
        .map_err(|err| PyValueError::new_err(format!("invalid alias proof bundle: {err}")))?;
    bundle.registry_root = root;
    let digest = alias_proof_signature_digest(&bundle);
    let keypair = KeyPair::from_private_key(
        PrivateKey::from_bytes(Algorithm::Ed25519, &[0x66; 32]).expect("seeded key"),
    )
    .expect("derive keypair");
    let signature = Signature::new(keypair.private_key(), digest.as_ref());
    let (_, signer_bytes) = public_key_to_bytes(keypair.public_key(), "alias proof signer")?;
    let signer: [u8; 32] = signer_bytes.try_into().map_err(|_| {
        PyValueError::new_err(format!(
            "alias proof signer must be 32 bytes, got {}",
            signer_bytes.len()
        ))
    })?;
    bundle
        .council_signatures
        .push(sorafs_manifest::CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        });
    bundle
        .validate()
        .map_err(|err| PyValueError::new_err(format!("invalid alias proof bundle: {err}")))?;

    let proof_bytes = norito::to_bytes(&bundle)
        .map_err(|err| PyValueError::new_err(format!("failed to encode alias proof: {err}")))?;
    let proof_b64 = BASE64.encode(proof_bytes);
    let dict = PyDict::new(py);
    dict.set_item("alias", alias)?;
    dict.set_item("proof_b64", proof_b64)?;
    dict.set_item("generated_at_unix", generated)?;
    dict.set_item("expires_at_unix", expires)?;
    dict.set_item("registry_height", bundle.registry_height)?;
    dict.set_item("registry_root_hex", hex::encode(bundle.registry_root))?;
    Ok(dict.unbind())
}

#[derive(Clone, FromPyObject)]
struct PyRangeCapability {
    max_chunk_span: u32,
    min_granularity: u32,
    supports_sparse_offsets: Option<bool>,
    requires_alignment: Option<bool>,
    supports_merkle_proof: Option<bool>,
}

#[derive(Clone, FromPyObject)]
struct PyStreamBudget {
    max_in_flight: u16,
    max_bytes_per_sec: u64,
    burst_bytes: Option<u64>,
}

#[derive(Clone, FromPyObject)]
struct PyTransportHint {
    protocol: String,
    protocol_id: u8,
    priority: u8,
}

#[derive(Clone, FromPyObject)]
struct PyProviderMetadata {
    provider_id: Option<String>,
    profile_id: Option<String>,
    profile_aliases: Option<Vec<String>>,
    availability: Option<String>,
    stake_amount: Option<String>,
    max_streams: Option<u32>,
    refresh_deadline: Option<u64>,
    expires_at: Option<u64>,
    ttl_secs: Option<u64>,
    allow_unknown_capabilities: Option<bool>,
    capability_names: Option<Vec<String>>,
    rendezvous_topics: Option<Vec<String>>,
    notes: Option<String>,
    range_capability: Option<PyRangeCapability>,
    stream_budget: Option<PyStreamBudget>,
    transport_hints: Option<Vec<PyTransportHint>>,
}

#[derive(Clone, FromPyObject)]
struct PyTelemetryEntry {
    provider_id: String,
    qos_score: Option<f64>,
    latency_p95_ms: Option<f64>,
    failure_rate_ewma: Option<f64>,
    token_health: Option<f64>,
    staking_weight: Option<f64>,
    penalty: Option<bool>,
    last_updated_unix: Option<u64>,
}

#[derive(Clone, FromPyObject)]
struct PyProviderBoost {
    provider: String,
    delta: i64,
}

struct PyScorePolicy {
    deny: HashSet<String>,
    boosts: HashMap<String, i64>,
}

impl PyScorePolicy {
    fn new(deny: HashSet<String>, boosts: HashMap<String, i64>) -> Self {
        Self { deny, boosts }
    }
}

impl ScorePolicy for PyScorePolicy {
    fn score(&self, ctx: ProviderScoreContext<'_>) -> ProviderScoreDecision {
        let provider = ctx.provider.id().as_str();
        if self.deny.contains(provider) {
            return ProviderScoreDecision {
                priority_delta: 0,
                allow: false,
            };
        }
        let delta = self.boosts.get(provider).copied().unwrap_or(0);
        ProviderScoreDecision {
            priority_delta: delta,
            allow: true,
        }
    }
}

struct ProcessedPyProvider {
    name: String,
    max_concurrent: NonZeroUsize,
    weight: Option<NonZeroU32>,
    metadata: Option<ProviderMetadata>,
}

fn py_range_capability_to_internal(range: &PyRangeCapability) -> RangeCapability {
    RangeCapability {
        max_chunk_span: range.max_chunk_span,
        min_granularity: range.min_granularity,
        supports_sparse_offsets: range.supports_sparse_offsets.unwrap_or(true),
        requires_alignment: range.requires_alignment.unwrap_or(false),
        supports_merkle_proof: range.supports_merkle_proof.unwrap_or(true),
    }
}

fn py_stream_budget_to_internal(budget: &PyStreamBudget) -> StreamBudget {
    StreamBudget {
        max_in_flight: budget.max_in_flight,
        max_bytes_per_sec: budget.max_bytes_per_sec,
        burst_bytes: budget.burst_bytes,
    }
}

fn ensure_positive_u64(value: u64, context: &str) -> PyResult<u64> {
    if value == 0 {
        Err(PyValueError::new_err(format!(
            "{context} must be greater than zero"
        )))
    } else {
        Ok(value)
    }
}

fn ensure_positive_u32(value: u32, context: &str) -> PyResult<u32> {
    if value == 0 {
        Err(PyValueError::new_err(format!(
            "{context} must be greater than zero"
        )))
    } else {
        Ok(value)
    }
}

fn py_taikai_cache_to_internal(cfg: &PyTaikaiCacheOptions) -> PyResult<TaikaiCacheConfig> {
    let qos = &cfg.qos;
    let qos_config = QosConfig {
        priority_rate_bps: ensure_positive_u64(
            qos.priority_rate_bps,
            "taikai_cache.qos.priority_rate_bps",
        )?,
        standard_rate_bps: ensure_positive_u64(
            qos.standard_rate_bps,
            "taikai_cache.qos.standard_rate_bps",
        )?,
        bulk_rate_bps: ensure_positive_u64(qos.bulk_rate_bps, "taikai_cache.qos.bulk_rate_bps")?,
        burst_multiplier: ensure_positive_u32(
            qos.burst_multiplier,
            "taikai_cache.qos.burst_multiplier",
        )?,
    };
    Ok(TaikaiCacheConfig {
        hot_capacity_bytes: ensure_positive_u64(
            cfg.hot_capacity_bytes,
            "taikai_cache.hot_capacity_bytes",
        )?,
        hot_retention: Duration::from_secs(ensure_positive_u64(
            cfg.hot_retention_secs,
            "taikai_cache.hot_retention_secs",
        )?),
        warm_capacity_bytes: ensure_positive_u64(
            cfg.warm_capacity_bytes,
            "taikai_cache.warm_capacity_bytes",
        )?,
        warm_retention: Duration::from_secs(ensure_positive_u64(
            cfg.warm_retention_secs,
            "taikai_cache.warm_retention_secs",
        )?),
        cold_capacity_bytes: ensure_positive_u64(
            cfg.cold_capacity_bytes,
            "taikai_cache.cold_capacity_bytes",
        )?,
        cold_retention: Duration::from_secs(ensure_positive_u64(
            cfg.cold_retention_secs,
            "taikai_cache.cold_retention_secs",
        )?),
        qos: qos_config,
        reliability: {
            let defaults = ReliabilityTuning::default();
            let reliability = cfg.reliability.clone().unwrap_or_default();
            let failures_to_trip = reliability
                .failures_to_trip
                .unwrap_or(defaults.failures_to_trip)
                .max(1);
            let open_secs = reliability.open_secs.unwrap_or(defaults.open_secs).max(1);
            ReliabilityTuning {
                failures_to_trip,
                open_secs,
            }
        },
    })
}

fn py_transport_hints_to_internal(hints: &[PyTransportHint]) -> Vec<TransportHint> {
    hints
        .iter()
        .map(|hint| TransportHint {
            protocol: hint.protocol.clone(),
            protocol_id: hint.protocol_id,
            priority: hint.priority,
        })
        .collect()
}

fn py_provider_metadata_to_internal(
    metadata: PyProviderMetadata,
    alias: &str,
) -> PyResult<ProviderMetadata> {
    let mut provider_metadata = ProviderMetadata::new();
    provider_metadata.provider_id = Some(metadata.provider_id.unwrap_or_else(|| alias.to_string()));
    provider_metadata.profile_id = metadata.profile_id;
    if let Some(aliases) = metadata.profile_aliases {
        provider_metadata.profile_aliases = aliases;
    }
    provider_metadata.availability = metadata.availability;
    provider_metadata.stake_amount = metadata.stake_amount;
    provider_metadata.max_streams = metadata.max_streams.map(|value| value as u16);
    provider_metadata.refresh_deadline = metadata.refresh_deadline;
    provider_metadata.expires_at = metadata.expires_at;
    provider_metadata.ttl_secs = metadata.ttl_secs;
    provider_metadata.allow_unknown_capabilities =
        metadata.allow_unknown_capabilities.unwrap_or(false);
    if let Some(names) = metadata.capability_names {
        provider_metadata.capability_names = names;
    }
    if let Some(notes) = metadata.notes {
        provider_metadata.notes = Some(notes);
    }
    if let Some(topics) = metadata.rendezvous_topics {
        provider_metadata.rendezvous_topics = topics;
    }
    if let Some(range) = metadata.range_capability {
        provider_metadata.range_capability = Some(py_range_capability_to_internal(&range));
    }
    if let Some(budget) = metadata.stream_budget {
        provider_metadata.stream_budget = Some(py_stream_budget_to_internal(&budget));
    }
    if let Some(hints) = metadata.transport_hints {
        provider_metadata.transport_hints = py_transport_hints_to_internal(&hints);
    }
    Ok(provider_metadata)
}

fn telemetry_snapshot_from_py(entries: &[PyTelemetryEntry]) -> TelemetrySnapshot {
    let records = entries
        .iter()
        .map(|entry| ProviderTelemetry {
            provider_id: entry.provider_id.clone(),
            qos_score: entry.qos_score,
            latency_p95_ms: entry.latency_p95_ms,
            failure_rate_ewma: entry.failure_rate_ewma,
            token_health: entry.token_health,
            staking_weight: entry.staking_weight,
            penalty: entry.penalty.unwrap_or(false),
            last_updated_unix: entry.last_updated_unix,
        })
        .collect::<Vec<_>>();
    TelemetrySnapshot::from_records(records)
}

#[derive(Clone, FromPyObject)]
struct PyLocalProviderSpec {
    name: String,
    path: String,
    max_concurrent: Option<usize>,
    weight: Option<u32>,
    metadata: Option<PyProviderMetadata>,
}

#[derive(Clone, Default, FromPyObject)]
struct PyMultiFetchOptions {
    verify_digests: Option<bool>,
    verify_lengths: Option<bool>,
    retry_budget: Option<usize>,
    provider_failure_threshold: Option<usize>,
    max_parallel: Option<usize>,
    max_peers: Option<usize>,
    chunker_handle: Option<String>,
    telemetry_region: Option<String>,
    telemetry: Option<Vec<PyTelemetryEntry>>,
    use_scoreboard: Option<bool>,
    deny_providers: Option<Vec<String>>,
    boost_providers: Option<Vec<PyProviderBoost>>,
    return_scoreboard: Option<bool>,
    scoreboard_out_path: Option<String>,
    scoreboard_now_unix_secs: Option<u64>,
    scoreboard_telemetry_label: Option<String>,
}

fn derive_python_scoreboard_label(
    options: &PyMultiFetchOptions,
    persist_path: bool,
) -> Option<String> {
    if let Some(label) = options
        .scoreboard_telemetry_label
        .as_deref()
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
    {
        return Some(label.to_string());
    }
    if persist_path {
        Some("sdk:python".to_string())
    } else {
        None
    }
}

fn option_usize_to_json_value(value: Option<usize>) -> json::Value {
    value
        .and_then(|val| u64::try_from(val).ok())
        .map_or(json::Value::Null, json::Value::from)
}

fn transport_policy_labels(
    requested: TransportPolicy,
    override_policy: Option<TransportPolicy>,
) -> (&'static str, bool, Option<&'static str>) {
    match override_policy {
        Some(policy) => (policy.label(), true, Some(policy.label())),
        None => (requested.label(), false, None),
    }
}

fn anonymity_policy_labels(
    requested: AnonymityPolicy,
    override_policy: Option<AnonymityPolicy>,
) -> (&'static str, bool, Option<&'static str>) {
    match override_policy {
        Some(policy) => (policy.label(), true, Some(policy.label())),
        None => (requested.label(), false, None),
    }
}

#[allow(clippy::too_many_arguments)]
fn python_scoreboard_metadata(
    provider_count: usize,
    gateway_provider_count: usize,
    gateway_manifest_provided: bool,
    assume_now: u64,
    telemetry_label: &str,
    telemetry_region: Option<&str>,
    max_parallel: Option<usize>,
    max_peers: Option<usize>,
    retry_budget: Option<usize>,
    provider_failure_threshold: Option<usize>,
    transport_policy: TransportPolicy,
    transport_override: Option<TransportPolicy>,
    anonymity_policy: AnonymityPolicy,
    anonymity_override: Option<AnonymityPolicy>,
) -> json::Value {
    let mut map = json::Map::new();
    map.insert(
        "version".into(),
        json::Value::from(env!("CARGO_PKG_VERSION")),
    );
    map.insert("use_scoreboard".into(), json::Value::from(true));
    map.insert("allow_implicit_metadata".into(), json::Value::from(false));
    map.insert(
        "provider_count".into(),
        json::Value::from(u64::try_from(provider_count).unwrap_or(u64::MAX)),
    );
    map.insert(
        "gateway_provider_count".into(),
        json::Value::from(u64::try_from(gateway_provider_count).unwrap_or(u64::MAX)),
    );
    map.insert(
        "max_parallel".into(),
        option_usize_to_json_value(max_parallel),
    );
    map.insert("max_peers".into(), option_usize_to_json_value(max_peers));
    map.insert(
        "retry_budget".into(),
        option_usize_to_json_value(retry_budget),
    );
    map.insert(
        "provider_failure_threshold".into(),
        option_usize_to_json_value(provider_failure_threshold),
    );
    map.insert("assume_now".into(), json::Value::from(assume_now));
    map.insert(
        "telemetry_source".into(),
        json::Value::from(telemetry_label.to_string()),
    );
    map.insert(
        "telemetry_region".into(),
        telemetry_region
            .map(|label| json::Value::from(label.to_string()))
            .unwrap_or(json::Value::Null),
    );
    map.insert(
        "gateway_manifest_provided".into(),
        json::Value::from(gateway_manifest_provided),
    );
    let (transport_label, transport_override_flag, transport_override_label) =
        transport_policy_labels(transport_policy, transport_override);
    map.insert(
        "transport_policy".into(),
        json::Value::from(transport_label),
    );
    map.insert(
        "transport_policy_override".into(),
        json::Value::from(transport_override_flag),
    );
    map.insert(
        "transport_policy_override_label".into(),
        transport_override_label
            .map(json::Value::from)
            .unwrap_or(json::Value::Null),
    );
    let (anonymity_label, anonymity_override_flag, anonymity_override_label) =
        anonymity_policy_labels(anonymity_policy, anonymity_override);
    map.insert(
        "anonymity_policy".into(),
        json::Value::from(anonymity_label),
    );
    map.insert(
        "anonymity_policy_override".into(),
        json::Value::from(anonymity_override_flag),
    );
    map.insert(
        "anonymity_policy_override_label".into(),
        anonymity_override_label
            .map(json::Value::from)
            .unwrap_or(json::Value::Null),
    );
    json::Value::Object(map)
}

fn provider_mix_label_from_counts(direct: u64, gateway: u64) -> &'static str {
    match (direct > 0, gateway > 0) {
        (true, true) => "mixed",
        (true, false) => "direct-only",
        (false, true) => "gateway-only",
        (false, false) => "none",
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_gateway_metadata_dict(
    py: Python<'_>,
    provider_count: usize,
    gateway_provider_count: usize,
    config: &OrchestratorConfig,
    telemetry_label: Option<&str>,
    telemetry_region: Option<&str>,
    manifest_id: &str,
    manifest_cid_hex: Option<&str>,
    manifest_envelope_present: bool,
    allow_implicit_metadata: bool,
) -> PyResult<Py<PyDict>> {
    let metadata = PyDict::new(py);
    let direct_count = u64::try_from(provider_count).unwrap_or(u64::MAX);
    let gateway_count = u64::try_from(gateway_provider_count).unwrap_or(u64::MAX);
    metadata.set_item("provider_count", direct_count)?;
    metadata.set_item("gateway_provider_count", gateway_count)?;
    metadata.set_item(
        "provider_mix",
        provider_mix_label_from_counts(direct_count, gateway_count),
    )?;

    let (transport_label, transport_override_flag, transport_override_label) =
        transport_policy_labels(
            config.transport_policy,
            config.policy_override.transport_policy,
        );
    metadata.set_item("transport_policy", transport_label)?;
    metadata.set_item("transport_policy_override", transport_override_flag)?;
    if let Some(label) = transport_override_label {
        metadata.set_item("transport_policy_override_label", label)?;
    } else {
        metadata.set_item("transport_policy_override_label", py.None())?;
    }

    let (anonymity_label, anonymity_override_flag, anonymity_override_label) =
        anonymity_policy_labels(
            config.anonymity_policy,
            config.policy_override.anonymity_policy,
        );
    metadata.set_item("anonymity_policy", anonymity_label)?;
    metadata.set_item("anonymity_policy_override", anonymity_override_flag)?;
    if let Some(label) = anonymity_override_label {
        metadata.set_item("anonymity_policy_override_label", label)?;
    } else {
        metadata.set_item("anonymity_policy_override_label", py.None())?;
    }

    if let Some(limit) = config.fetch.global_parallel_limit {
        let converted = u64::try_from(limit).unwrap_or(u64::MAX);
        metadata.set_item("max_parallel", converted)?;
    } else {
        metadata.set_item("max_parallel", py.None())?;
    }

    if let Some(limit) = config.max_providers {
        let converted = u64::try_from(limit.get()).unwrap_or(u64::MAX);
        metadata.set_item("max_peers", converted)?;
    } else {
        metadata.set_item("max_peers", py.None())?;
    }

    if let Some(limit) = config.fetch.per_chunk_retry_limit {
        let converted = u64::try_from(limit).unwrap_or(u64::MAX);
        metadata.set_item("retry_budget", converted)?;
    } else {
        metadata.set_item("retry_budget", py.None())?;
    }

    let provider_failure_threshold =
        u64::try_from(config.fetch.provider_failure_threshold).unwrap_or(u64::MAX);
    metadata.set_item("provider_failure_threshold", provider_failure_threshold)?;
    metadata.set_item("assume_now_unix", config.scoreboard.now_unix_secs)?;

    if let Some(label) = telemetry_label {
        metadata.set_item("telemetry_source_label", label)?;
    } else {
        metadata.set_item("telemetry_source_label", py.None())?;
    }
    if let Some(region) = telemetry_region {
        metadata.set_item("telemetry_region", region)?;
    } else {
        metadata.set_item("telemetry_region", py.None())?;
    }

    metadata.set_item("gateway_manifest_provided", manifest_envelope_present)?;
    metadata.set_item("gateway_manifest_id", manifest_id)?;
    if let Some(cid) = manifest_cid_hex {
        metadata.set_item("gateway_manifest_cid", cid)?;
    } else {
        metadata.set_item("gateway_manifest_cid", py.None())?;
    }
    metadata.set_item("allow_implicit_metadata", allow_implicit_metadata)?;

    Ok(metadata.unbind())
}

#[derive(Clone, FromPyObject)]
struct PyGatewayProviderSpec {
    name: String,
    provider_id_hex: String,
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
}

#[derive(Clone, Default, FromPyObject)]
struct PyGatewayFetchOptions {
    manifest_envelope_b64: Option<String>,
    manifest_cid_hex: Option<String>,
    expected_cache_version: Option<String>,
    moderation_token_key_b64: Option<String>,
    client_id: Option<String>,
    telemetry_region: Option<String>,
    scoreboard_telemetry_label: Option<String>,
    rollout_phase: Option<String>,
    max_peers: Option<usize>,
    retry_budget: Option<usize>,
    transport_policy: Option<String>,
    anonymity_policy: Option<String>,
    local_proxy: Option<PyLocalProxyOptions>,
    taikai_cache: Option<PyTaikaiCacheOptions>,
}

#[derive(Clone, Default, FromPyObject)]
struct PyLocalProxyOptions {
    bind_addr: Option<String>,
    telemetry_label: Option<String>,
    guard_cache_key_hex: Option<String>,
    emit_browser_manifest: Option<bool>,
    proxy_mode: Option<String>,
    prewarm_circuits: Option<bool>,
    max_streams_per_circuit: Option<u32>,
    circuit_ttl_hint_secs: Option<u32>,
    norito_bridge: Option<PyLocalProxyNoritoBridgeOptions>,
    car_bridge: Option<PyLocalProxyCarBridgeOptions>,
    kaigi_bridge: Option<PyLocalProxyKaigiBridgeOptions>,
}

#[derive(Clone, Default, FromPyObject)]
struct PyLocalProxyNoritoBridgeOptions {
    spool_dir: String,
    extension: Option<String>,
}

#[derive(Clone, Default, FromPyObject)]
struct PyLocalProxyCarBridgeOptions {
    cache_dir: String,
    extension: Option<String>,
    allow_zst: Option<bool>,
}

#[derive(Clone, Default, FromPyObject)]
struct PyLocalProxyKaigiBridgeOptions {
    spool_dir: String,
    extension: Option<String>,
    room_policy: Option<String>,
}

#[derive(Clone, FromPyObject)]
struct PyTaikaiQosOptions {
    priority_rate_bps: u64,
    standard_rate_bps: u64,
    bulk_rate_bps: u64,
    burst_multiplier: u32,
}

#[derive(Clone, Default, FromPyObject)]
struct PyTaikaiReliabilityOptions {
    failures_to_trip: Option<u32>,
    open_secs: Option<u64>,
}

#[derive(Clone, FromPyObject)]
struct PyTaikaiCacheOptions {
    hot_capacity_bytes: u64,
    hot_retention_secs: u64,
    warm_capacity_bytes: u64,
    warm_retention_secs: u64,
    cold_capacity_bytes: u64,
    cold_retention_secs: u64,
    qos: PyTaikaiQosOptions,
    reliability: Option<PyTaikaiReliabilityOptions>,
}

fn chunk_verification_error_payload(
    py: Python<'_>,
    error: ChunkVerificationError,
) -> PyResult<Py<PyDict>> {
    let payload = PyDict::new(py);
    match error {
        ChunkVerificationError::LengthMismatch { expected, actual } => {
            payload.set_item("type", "length_mismatch")?;
            payload.set_item("expected", expected)?;
            payload.set_item("actual", actual)?;
        }
        ChunkVerificationError::DigestMismatch { expected, actual } => {
            payload.set_item("type", "digest_mismatch")?;
            payload.set_item("expected", hex_encode(expected))?;
            payload.set_item("actual", hex_encode(actual))?;
        }
    }
    Ok(payload.into())
}

fn capability_mismatch_payload(
    py: Python<'_>,
    mismatch: CapabilityMismatch,
) -> PyResult<Py<PyDict>> {
    let payload = PyDict::new(py);
    match mismatch {
        CapabilityMismatch::MissingRangeCapability => {
            payload.set_item("type", "missing_range_capability")?;
        }
        CapabilityMismatch::ChunkTooLarge {
            chunk_length,
            max_span,
        } => {
            payload.set_item("type", "chunk_too_large")?;
            payload.set_item("chunk_length", chunk_length)?;
            payload.set_item("max_span", max_span)?;
        }
        CapabilityMismatch::OffsetMisaligned {
            offset,
            required_alignment,
        } => {
            payload.set_item("type", "offset_misaligned")?;
            payload.set_item("offset", offset)?;
            payload.set_item("required_alignment", required_alignment)?;
        }
        CapabilityMismatch::LengthMisaligned {
            length,
            required_alignment,
        } => {
            payload.set_item("type", "length_misaligned")?;
            payload.set_item("length", length)?;
            payload.set_item("required_alignment", required_alignment)?;
        }
        CapabilityMismatch::StreamBurstTooSmall {
            chunk_length,
            burst_limit,
        } => {
            payload.set_item("type", "stream_burst_too_small")?;
            payload.set_item("chunk_length", chunk_length)?;
            payload.set_item("burst_limit", burst_limit)?;
        }
    }
    Ok(payload.into())
}

fn attempt_failure_payload(py: Python<'_>, failure: AttemptFailure) -> PyResult<Py<PyDict>> {
    let payload = PyDict::new(py);
    match failure {
        AttemptFailure::Provider {
            message,
            policy_block,
        } => {
            payload.set_item("type", "provider")?;
            payload.set_item("message", message)?;
            if let Some(policy) = policy_block {
                let policy_dict = PyDict::new(py);
                policy_dict.set_item("observed_status", policy.observed_status.as_u16())?;
                policy_dict.set_item("canonical_status", policy.canonical_status.as_u16())?;
                if let Some(code) = policy.code {
                    policy_dict.set_item("code", code)?;
                }
                if let Some(cache) = policy.cache_version {
                    policy_dict.set_item("cache_version", cache)?;
                }
                if let Some(denylist) = policy.denylist_version {
                    policy_dict.set_item("denylist_version", denylist)?;
                }
                policy_dict.set_item("proof_token_present", policy.proof_token_present)?;
                if let Some(message) = policy.message {
                    policy_dict.set_item("message", message)?;
                }
                payload.set_item("policy_block", policy_dict)?;
            }
        }
        AttemptFailure::InvalidChunk(error) => {
            payload.set_item("type", "invalid_chunk")?;
            payload.set_item("error", chunk_verification_error_payload(py, error)?)?;
        }
    }
    Ok(payload.into())
}

fn tier_counts_payload(py: Python<'_>, counts: TierStats) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("hot", counts.hot)?;
    dict.set_item("warm", counts.warm)?;
    dict.set_item("cold", counts.cold)?;
    Ok(dict.into())
}

fn eviction_counts_payload(py: Python<'_>, counts: EvictionStats) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    let hot = PyDict::new(py);
    hot.set_item("expired", counts.hot.expired)?;
    hot.set_item("capacity", counts.hot.capacity)?;
    let warm = PyDict::new(py);
    warm.set_item("expired", counts.warm.expired)?;
    warm.set_item("capacity", counts.warm.capacity)?;
    let cold = PyDict::new(py);
    cold.set_item("expired", counts.cold.expired)?;
    cold.set_item("capacity", counts.cold.capacity)?;
    dict.set_item("hot", hot)?;
    dict.set_item("warm", warm)?;
    dict.set_item("cold", cold)?;
    Ok(dict.into())
}

fn promotions_payload(py: Python<'_>, promotions: PromotionStats) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("warm_to_hot", promotions.warm_to_hot)?;
    dict.set_item("cold_to_warm", promotions.cold_to_warm)?;
    dict.set_item("cold_to_hot", promotions.cold_to_hot)?;
    Ok(dict.into())
}

fn qos_counts_payload(py: Python<'_>, counts: QosStats) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("priority", counts.priority)?;
    dict.set_item("standard", counts.standard)?;
    dict.set_item("bulk", counts.bulk)?;
    Ok(dict.into())
}

fn taikai_cache_stats_payload(
    py: Python<'_>,
    stats: TaikaiCacheStatsSnapshot,
) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("hits", tier_counts_payload(py, stats.hits)?)?;
    dict.set_item("misses", stats.misses)?;
    dict.set_item("inserts", tier_counts_payload(py, stats.inserts)?)?;
    dict.set_item("evictions", eviction_counts_payload(py, stats.evictions)?)?;
    dict.set_item("promotions", promotions_payload(py, stats.promotions)?)?;
    dict.set_item("qos_denials", qos_counts_payload(py, stats.qos_denials)?)?;
    Ok(dict.into())
}

fn taikai_queue_stats_payload(py: Python<'_>, stats: TaikaiPullQueueStats) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("pending_segments", stats.pending_segments)?;
    dict.set_item("pending_bytes", stats.pending_bytes)?;
    dict.set_item("pending_batches", stats.pending_batches)?;
    dict.set_item("in_flight_batches", stats.in_flight_batches)?;
    dict.set_item("hedged_batches", stats.hedged_batches)?;
    dict.set_item(
        "shaper_denials",
        qos_counts_payload(py, stats.shaper_denials)?,
    )?;
    dict.set_item("dropped_segments", stats.dropped_segments)?;
    dict.set_item("failovers", stats.failovers)?;
    dict.set_item("open_circuits", stats.open_circuits)?;
    Ok(dict.into())
}

fn attempt_error_payload(py: Python<'_>, error: AttemptError) -> PyResult<Py<PyDict>> {
    let AttemptError { provider, failure } = error;
    let provider_name = provider.as_str().to_owned();
    let payload = PyDict::new(py);
    payload.set_item("provider", provider_name)?;
    payload.set_item("failure", attempt_failure_payload(py, failure)?)?;
    Ok(payload.into())
}

fn sorafs_multi_fetch_error(py: Python<'_>, err: MultiSourceError) -> PyErr {
    match build_multi_fetch_error_payload(py, err) {
        Ok(payload) => SorafsMultiFetchError::new_err(payload),
        Err(py_err) => py_err,
    }
}

fn build_multi_fetch_error_payload(py: Python<'_>, err: MultiSourceError) -> PyResult<Py<PyDict>> {
    let payload = PyDict::new(py);
    payload.set_item("message", err.to_string())?;
    match err {
        MultiSourceError::NoProviders => {
            payload.set_item("kind", "no_providers")?;
        }
        MultiSourceError::NoHealthyProviders {
            chunk_index,
            attempts,
            last_error,
        } => {
            payload.set_item("kind", "no_healthy_providers")?;
            payload.set_item("chunk_index", chunk_index)?;
            payload.set_item("attempts", attempts)?;
            if let Some(error) = last_error {
                payload.set_item("last_error", attempt_error_payload(py, *error)?)?;
            } else {
                payload.set_item("last_error", py.None())?;
            }
        }
        MultiSourceError::NoCompatibleProviders {
            chunk_index,
            providers,
        } => {
            payload.set_item("kind", "no_compatible_providers")?;
            payload.set_item("chunk_index", chunk_index)?;
            let entries = PyList::empty(py);
            for (provider, reason) in providers {
                let entry = PyDict::new(py);
                entry.set_item("provider", provider.as_str())?;
                entry.set_item("reason", capability_mismatch_payload(py, reason)?)?;
                entries.append(entry)?;
            }
            payload.set_item("providers", entries)?;
        }
        MultiSourceError::ExhaustedRetries {
            chunk_index,
            attempts,
            last_error,
        } => {
            payload.set_item("kind", "exhausted_retries")?;
            payload.set_item("chunk_index", chunk_index)?;
            payload.set_item("attempts", attempts)?;
            payload.set_item("last_error", attempt_error_payload(py, *last_error)?)?;
        }
        MultiSourceError::ObserverFailed {
            chunk_index,
            source,
        } => {
            payload.set_item("kind", "observer_failed")?;
            payload.set_item("chunk_index", chunk_index)?;
            payload.set_item("observer_error", source.to_string())?;
        }
        MultiSourceError::InternalInvariant(reason) => {
            payload.set_item("kind", "internal_invariant")?;
            payload.set_item("reason", reason)?;
        }
    }
    Ok(payload.into())
}

#[pyfunction(
    name = "sorafs_multi_fetch_local",
    signature = (plan_json, providers, *, options=None)
)]
fn sorafs_multi_fetch_local_py(
    py: Python<'_>,
    plan_json: &str,
    providers: Vec<PyLocalProviderSpec>,
    options: Option<PyMultiFetchOptions>,
) -> PyResult<Py<PyDict>> {
    if providers.is_empty() {
        return Err(PyValueError::new_err(
            "providers list must contain at least one entry",
        ));
    }

    let options = options.unwrap_or_default();

    let plan_value: json::Value = json::from_str(plan_json)
        .map_err(|err| PyValueError::new_err(format!("failed to parse plan JSON: {err}")))?;
    let chunk_specs = chunk_fetch_specs_from_json(&plan_value)
        .map_err(|err| PyValueError::new_err(format!("invalid chunk fetch plan: {err}")))?;
    if chunk_specs.is_empty() {
        return Err(PyValueError::new_err(
            "chunk fetch plan must contain at least one chunk",
        ));
    }

    let content_length = chunk_specs
        .iter()
        .map(|spec| spec.offset + u64::from(spec.length))
        .max()
        .unwrap_or(0);

    let chunk_profile = if let Some(handle) = options.chunker_handle.as_deref() {
        let descriptor = sorafs_car::chunker_registry::lookup_by_handle(handle)
            .ok_or_else(|| PyValueError::new_err(format!("unknown chunker handle '{handle}'")))?;
        descriptor.profile
    } else {
        ChunkProfile::DEFAULT
    };

    let plan = CarBuildPlan {
        chunk_profile,
        payload_digest: blake3_hash(&[]),
        content_length,
        chunks: chunk_specs
            .iter()
            .map(|spec| CarChunk {
                offset: spec.offset,
                length: spec.length,
                digest: spec.digest,
                taikai_segment_hint: spec.taikai_segment_hint.clone(),
            })
            .collect(),
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: chunk_specs.len(),
            size: content_length,
        }],
    };

    let mut processed = Vec::with_capacity(providers.len());
    let mut provider_names = HashSet::new();
    let mut raw_path_lookup: HashMap<String, PathBuf> = HashMap::new();

    for spec in providers {
        if !provider_names.insert(spec.name.clone()) {
            return Err(PyValueError::new_err(format!(
                "duplicate provider '{}'",
                spec.name
            )));
        }
        let path = PathBuf::from(&spec.path);
        if !path.exists() {
            return Err(PyValueError::new_err(format!(
                "provider payload '{}' does not exist",
                spec.path
            )));
        }
        if !path.is_file() {
            return Err(PyValueError::new_err(format!(
                "provider payload '{}' is not a regular file",
                spec.path
            )));
        }

        raw_path_lookup.insert(spec.name.clone(), path.clone());

        let max_concurrent = spec
            .max_concurrent
            .and_then(NonZeroUsize::new)
            .unwrap_or_else(|| NonZeroUsize::new(2).expect("constant non-zero"));

        let weight = match spec.weight {
            Some(value) => Some(NonZeroU32::new(value).ok_or_else(|| {
                PyValueError::new_err("provider weight must be greater than zero")
            })?),
            None => None,
        };

        let metadata = match spec.metadata {
            Some(meta) => {
                let provider_metadata = py_provider_metadata_to_internal(meta, &spec.name)?;
                if let Some(provider_id) = provider_metadata.provider_id.clone() {
                    raw_path_lookup.insert(provider_id, path.clone());
                }
                Some(provider_metadata)
            }
            None => None,
        };

        processed.push(ProcessedPyProvider {
            name: spec.name,
            max_concurrent,
            weight,
            metadata,
        });
    }

    if options.scoreboard_telemetry_label.is_some() && options.scoreboard_out_path.is_none() {
        return Err(PyValueError::new_err(
            "scoreboard_telemetry_label requires scoreboard_out_path to be set",
        ));
    }

    let telemetry_region = if let Some(raw) = options.telemetry_region.as_ref() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PyValueError::new_err("telemetry_region must not be empty"));
        }
        Some(trimmed.to_string())
    } else {
        None
    };

    let provider_count = processed.len();
    let mut scoreboard_config = ScoreboardConfig::default();
    if let Some(now) = options.scoreboard_now_unix_secs {
        scoreboard_config.now_unix_secs = now;
    }
    let scoreboard_out_raw = options
        .scoreboard_out_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(path_str) = scoreboard_out_raw {
        let path = PathBuf::from(path_str);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                PyValueError::new_err(format!(
                    "failed to create scoreboard directory `{}`: {err}",
                    parent.display()
                ))
            })?;
        }
        scoreboard_config.persist_path = Some(path);
        if let Some(label) = derive_python_scoreboard_label(&options, true) {
            scoreboard_config.persist_metadata = Some(python_scoreboard_metadata(
                provider_count,
                0,
                false,
                scoreboard_config.now_unix_secs,
                &label,
                telemetry_region.as_deref(),
                options.max_parallel,
                options.max_peers,
                options.retry_budget,
                options.provider_failure_threshold,
                TransportPolicy::SoranetPreferred,
                None,
                AnonymityPolicy::GuardPq,
                None,
            ));
        }
    }

    let telemetry_snapshot = options
        .telemetry
        .as_ref()
        .map_or_else(TelemetrySnapshot::default, |entries| {
            telemetry_snapshot_from_py(entries)
        });
    let telemetry_provided = options
        .telemetry
        .as_ref()
        .is_some_and(|entries| !entries.is_empty());
    let scoreboard_requested = options.use_scoreboard.unwrap_or(false)
        || telemetry_provided
        || scoreboard_config.persist_path.is_some();

    let mut alias_by_provider_id: HashMap<String, String> = HashMap::new();
    let scoreboard_metadata = if scoreboard_requested {
        let mut list = Vec::with_capacity(processed.len());
        for provider in processed.iter_mut() {
            let mut metadata = provider.metadata.clone().ok_or_else(|| {
                PyValueError::new_err(format!(
                    "scoreboard requires metadata for provider '{}' (provide advert metadata or disable use_scoreboard)",
                    provider.name
                ))
            })?;
            let provider_id = metadata
                .provider_id
                .clone()
                .unwrap_or_else(|| provider.name.clone());
            alias_by_provider_id.insert(provider_id.clone(), provider.name.clone());
            if !raw_path_lookup.contains_key(&provider_id)
                && let Some(path) = raw_path_lookup.get(&provider.name)
            {
                raw_path_lookup.insert(provider_id.clone(), path.clone());
            }
            metadata.provider_id = Some(provider_id);
            provider.metadata = Some(metadata.clone());
            list.push(metadata);
        }
        list
    } else {
        Vec::new()
    };

    let scoreboard = if scoreboard_requested {
        Some(
            scoreboard::build_scoreboard(
                &plan,
                &scoreboard_metadata,
                &telemetry_snapshot,
                &scoreboard_config,
            )
            .map_err(|err| PyValueError::new_err(format!("failed to build scoreboard: {err}")))?,
        )
    } else {
        None
    };

    let include_scoreboard = options.return_scoreboard.unwrap_or(scoreboard_requested);

    let mut eligible_aliases: HashSet<String> = HashSet::new();
    let mut weight_by_alias: HashMap<String, NonZeroU32> = HashMap::new();
    let scoreboard_export = if let Some(scoreboard) = scoreboard.as_ref() {
        let mut entries: Vec<(String, String, f64, f64, String)> =
            Vec::with_capacity(scoreboard.entries().len());
        for entry in scoreboard.entries() {
            let provider_id = entry.provider.id().as_str();
            let alias = alias_by_provider_id
                .get(provider_id)
                .cloned()
                .unwrap_or_else(|| provider_id.to_string());
            match &entry.eligibility {
                Eligibility::Eligible => {
                    eligible_aliases.insert(alias.clone());
                    weight_by_alias.insert(alias.clone(), entry.provider.weight());
                    entries.push((
                        provider_id.to_string(),
                        alias,
                        entry.raw_score,
                        entry.normalised_weight,
                        "eligible".to_string(),
                    ));
                }
                Eligibility::Ineligible(reason) => {
                    entries.push((
                        provider_id.to_string(),
                        alias,
                        entry.raw_score,
                        entry.normalised_weight,
                        reason.to_string(),
                    ));
                }
            }
        }
        if scoreboard_requested && eligible_aliases.is_empty() {
            return Err(PyValueError::new_err("scoreboard excluded all providers"));
        }
        if include_scoreboard {
            Some(entries)
        } else {
            None
        }
    } else {
        None
    };

    let mut fetch_providers = Vec::with_capacity(processed.len());
    for provider in &processed {
        if scoreboard_requested && !eligible_aliases.contains(&provider.name) {
            continue;
        }
        let mut fetch_provider = FetchProvider::new(provider.name.clone())
            .with_max_concurrent_chunks(provider.max_concurrent);
        if let Some(weight) = weight_by_alias
            .get(&provider.name)
            .copied()
            .or(provider.weight)
        {
            fetch_provider = fetch_provider.with_weight(weight);
        }
        if let Some(metadata) = &provider.metadata {
            fetch_provider = fetch_provider.with_metadata(metadata.clone());
        }
        fetch_providers.push(fetch_provider);
    }

    if fetch_providers.is_empty() {
        return Err(PyValueError::new_err(
            "no providers available after applying scoreboard filters",
        ));
    }

    if let Some(limit) = options.max_peers {
        let limit = limit.max(1);
        if fetch_providers.len() > limit {
            fetch_providers.truncate(limit);
        }
    }

    let mut provider_paths: HashMap<String, PathBuf> = HashMap::new();
    for provider in &fetch_providers {
        if let Some(path) = raw_path_lookup.get(provider.id().as_str()) {
            provider_paths.insert(provider.id().as_str().to_string(), path.clone());
        }
        if let Some(metadata) = provider.metadata()
            && let Some(provider_id) = metadata.provider_id.as_ref()
            && let Some(path) = raw_path_lookup.get(provider_id)
        {
            provider_paths.insert(provider_id.clone(), path.clone());
        }
    }

    let path_map = Arc::new(provider_paths);
    let fetcher = move |request: FetchRequest| {
        let map = Arc::clone(&path_map);
        async move {
            let provider_name = request.provider.id().as_str();
            let path = map.get(provider_name).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("unknown provider '{provider_name}'"),
                )
            })?;
            let mut file = File::open(path)?;
            file.seek(SeekFrom::Start(request.spec.offset))?;
            let mut buf = vec![0u8; request.spec.length as usize];
            file.read_exact(&mut buf)?;
            Ok::<ChunkResponse, std::io::Error>(ChunkResponse::new(buf))
        }
    };

    let mut fetch_options = FetchOptions::default();
    if let Some(flag) = options.verify_digests {
        fetch_options.verify_digests = flag;
    }
    if let Some(flag) = options.verify_lengths {
        fetch_options.verify_lengths = flag;
    }
    if let Some(limit) = options.retry_budget {
        fetch_options.per_chunk_retry_limit = Some(limit.max(1));
    }
    if let Some(threshold) = options.provider_failure_threshold {
        fetch_options.provider_failure_threshold = threshold;
    }
    if let Some(limit) = options.max_parallel {
        fetch_options.global_parallel_limit = Some(limit.max(1));
    }

    let policy_requested = options
        .deny_providers
        .as_ref()
        .is_some_and(|deny| !deny.is_empty())
        || options
            .boost_providers
            .as_ref()
            .is_some_and(|boosts| !boosts.is_empty());
    if policy_requested {
        let mut deny = HashSet::new();
        if let Some(entries) = options.deny_providers.as_ref() {
            for provider in entries {
                deny.insert(provider.clone());
            }
        }
        let mut boosts = HashMap::new();
        if let Some(entries) = options.boost_providers.as_ref() {
            for boost in entries {
                boosts.insert(boost.provider.clone(), boost.delta);
            }
        }
        fetch_options.score_policy = Some(Arc::new(PyScorePolicy::new(deny, boosts)));
    }

    let outcome = block_on(fetch_plan_parallel(
        &plan,
        fetch_providers,
        fetcher,
        fetch_options,
    ))
    .map_err(|err| sorafs_multi_fetch_error(py, err))?;

    let payload_bytes = outcome.assemble_payload();
    let result = PyDict::new(py);
    result.set_item("chunk_count", outcome.chunks.len())?;
    result.set_item("payload", PyBytes::new(py, &payload_bytes))?;

    let provider_list = PyList::empty(py);
    for report in &outcome.provider_reports {
        let entry = PyDict::new(py);
        entry.set_item("provider", report.provider.id().as_str())?;
        entry.set_item("successes", report.successes)?;
        entry.set_item("failures", report.failures)?;
        entry.set_item("disabled", report.disabled)?;
        provider_list.append(entry)?;
    }
    result.set_item("provider_reports", provider_list)?;

    let receipts_list = PyList::empty(py);
    for receipt in &outcome.chunk_receipts {
        let entry = PyDict::new(py);
        entry.set_item("chunk_index", receipt.chunk_index)?;
        entry.set_item("provider", receipt.provider.as_str())?;
        entry.set_item("attempts", receipt.attempts)?;
        entry.set_item("latency_ms", receipt.latency_ms)?;
        entry.set_item("bytes", receipt.bytes)?;
        receipts_list.append(entry)?;
    }
    result.set_item("chunk_receipts", receipts_list)?;

    if let Some(entries) = scoreboard_export {
        let scoreboard_list = PyList::empty(py);
        for (provider_id, alias, raw_score, normalised_weight, eligibility) in entries {
            let row = PyDict::new(py);
            row.set_item("provider_id", provider_id)?;
            row.set_item("alias", alias)?;
            row.set_item("raw_score", raw_score)?;
            row.set_item("normalized_weight", normalised_weight)?;
            row.set_item("eligibility", eligibility)?;
            scoreboard_list.append(row)?;
        }
        result.set_item("scoreboard", scoreboard_list)?;
    } else if include_scoreboard {
        result.set_item("scoreboard", py.None())?;
    }

    Ok(result.unbind())
}

#[pyfunction]
#[pyo3(
    name = "sorafs_gateway_fetch",
    signature = (manifest_id_hex, chunker_handle, plan_json, providers, *, options=None)
)]
fn sorafs_gateway_fetch_py(
    py: Python<'_>,
    manifest_id_hex: &str,
    chunker_handle: &str,
    plan_json: &str,
    providers: Vec<PyGatewayProviderSpec>,
    options: Option<PyGatewayFetchOptions>,
) -> PyResult<Py<PyDict>> {
    if providers.is_empty() {
        return Err(PyValueError::new_err(
            "providers list must contain at least one entry",
        ));
    }

    let manifest_id = manifest_id_hex.trim().to_ascii_lowercase();
    if manifest_id.len() != 64 || !manifest_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(PyValueError::new_err(
            "manifest_id_hex must be a 32-byte hex string",
        ));
    }

    let options = options.unwrap_or_default();
    let scoreboard_telemetry_label = options
        .scoreboard_telemetry_label
        .as_ref()
        .map(|label| {
            let trimmed = label.trim();
            if trimmed.is_empty() {
                return Err(PyValueError::new_err(
                    "scoreboard_telemetry_label must not be empty when provided",
                ));
            }
            Ok(trimmed.to_string())
        })
        .transpose()?;

    let plan_value: json::Value = json::from_str(plan_json)
        .map_err(|err| PyValueError::new_err(format!("failed to parse plan JSON: {err}")))?;
    let mut chunk_specs = chunk_fetch_specs_from_json(&plan_value)
        .map_err(|err| PyValueError::new_err(format!("invalid chunk fetch plan: {err}")))?;
    if chunk_specs.is_empty() {
        return Err(PyValueError::new_err(
            "chunk fetch plan must contain at least one chunk",
        ));
    }
    chunk_specs.sort_by_key(|spec| spec.chunk_index);
    for (idx, spec) in chunk_specs.iter().enumerate() {
        if spec.chunk_index != idx {
            return Err(PyValueError::new_err(format!(
                "chunk fetch plan missing chunk index {idx}"
            )));
        }
    }

    let content_length = chunk_specs
        .iter()
        .map(|spec| spec.offset + u64::from(spec.length))
        .max()
        .unwrap_or(0);

    let chunker_handle_trimmed = chunker_handle.trim();
    if chunker_handle_trimmed.is_empty() {
        return Err(PyValueError::new_err("chunker_handle must not be empty"));
    }
    let descriptor = sorafs_car::chunker_registry::lookup_by_handle(chunker_handle_trimmed)
        .ok_or_else(|| {
            PyValueError::new_err(format!("unknown chunker handle '{chunker_handle_trimmed}'"))
        })?;

    let plan = CarBuildPlan {
        chunk_profile: descriptor.profile,
        payload_digest: blake3_hash(&[]),
        content_length,
        chunks: chunk_specs
            .iter()
            .map(|spec| CarChunk {
                offset: spec.offset,
                length: spec.length,
                digest: spec.digest,
                taikai_segment_hint: spec.taikai_segment_hint.clone(),
            })
            .collect(),
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: chunk_specs.len(),
            size: content_length,
        }],
    };

    let manifest_envelope_b64 = options
        .manifest_envelope_b64
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let manifest_envelope_present = manifest_envelope_b64.is_some();
    let manifest_cid_hex = options
        .manifest_cid_hex
        .as_ref()
        .map(|cid| cid.trim().to_ascii_lowercase());
    if let Some(cid) = &manifest_cid_hex
        && (cid.len() != 64 || !cid.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(PyValueError::new_err(
            "manifest_cid_hex must be a 32-byte hex string",
        ));
    }
    let expected_cache_version = options
        .expected_cache_version
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let moderation_token_key_b64 = options
        .moderation_token_key_b64
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let client_id = options
        .client_id
        .as_ref()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .map(str::to_owned);
    let telemetry_region = if let Some(raw) = options.telemetry_region.as_ref() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PyValueError::new_err("telemetry_region must not be empty"));
        }
        Some(trimmed.to_string())
    } else {
        None
    };
    let manifest_cid_metadata = manifest_cid_hex.clone();

    let provider_inputs: Vec<GatewayProviderInput> = providers
        .into_iter()
        .map(|spec| {
            if spec.name.trim().is_empty() {
                return Err(PyValueError::new_err("provider name must not be empty"));
            }
            let provider_id = spec.provider_id_hex.trim().to_ascii_lowercase();
            if provider_id.len() != 64 || !provider_id.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(PyValueError::new_err(format!(
                    "provider '{}' has invalid provider_id_hex; expected 32-byte hex",
                    spec.name
                )));
            }
            if spec.base_url.trim().is_empty() {
                return Err(PyValueError::new_err(format!(
                    "provider '{}' base_url must not be empty",
                    spec.name
                )));
            }
            if spec.stream_token_b64.trim().is_empty() {
                return Err(PyValueError::new_err(format!(
                    "provider '{}' stream_token must not be empty",
                    spec.name
                )));
            }
            Ok(GatewayProviderInput {
                name: spec.name,
                provider_id_hex: provider_id,
                base_url: spec.base_url,
                stream_token_b64: spec.stream_token_b64,
                privacy_events_url: spec.privacy_events_url,
            })
        })
        .collect::<PyResult<_>>()?;
    let unique_gateway_providers = provider_inputs
        .iter()
        .map(|provider| provider.provider_id_hex.as_str())
        .collect::<HashSet<_>>()
        .len();
    if unique_gateway_providers < 2 {
        return Err(PyValueError::new_err(
            "sorafs_gateway_fetch requires at least two unique gateway providers",
        ));
    }

    let mut orchestrator_config = OrchestratorConfig::default();
    if let Some(region) = telemetry_region.as_ref() {
        orchestrator_config = orchestrator_config.with_telemetry_region(region.clone());
    }
    if let Some(limit) = options.retry_budget {
        let limit = limit.max(1);
        orchestrator_config.fetch.per_chunk_retry_limit = Some(limit);
    }
    let max_peers = options.max_peers.map(|limit| limit.max(1));
    if let Some(limit) = max_peers {
        orchestrator_config.fetch.global_parallel_limit = Some(
            orchestrator_config
                .fetch
                .global_parallel_limit
                .map_or(limit, |existing| existing.min(limit)),
        );
    }
    if let Some(raw) = options.transport_policy.as_ref() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PyValueError::new_err("transport_policy must not be empty"));
        }
        let policy = TransportPolicy::parse(trimmed).ok_or_else(|| {
            PyValueError::new_err(
                "transport_policy must be one of 'soranet-first', 'soranet-strict', or 'direct-only'",
            )
        })?;
        orchestrator_config.transport_policy = policy;
    }
    if let Some(raw) = options.rollout_phase.as_ref() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PyValueError::new_err("rollout_phase must not be empty"));
        }
        let phase = RolloutPhase::parse(trimmed).ok_or_else(|| {
            PyValueError::new_err(
                "rollout_phase must be one of 'canary', 'ramp', 'default', or stage_a/stage_b/stage_c aliases",
            )
        })?;
        orchestrator_config = orchestrator_config.with_rollout_phase(phase);
    }
    if let Some(raw) = options.anonymity_policy.as_ref() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PyValueError::new_err("anonymity_policy must not be empty"));
        }
        let policy = AnonymityPolicy::parse(trimmed).ok_or_else(|| {
            PyValueError::new_err(
                "anonymity_policy must be one of 'stage-a', 'anon-guard-pq', 'stage-b', 'anon-majority-pq', 'stage-c', or 'anon-strict-pq'",
            )
        })?;
        orchestrator_config.anonymity_policy = policy;
        orchestrator_config.anonymity_policy_override = Some(policy);
    }
    if let Some(proxy_opts) = options.local_proxy.as_ref() {
        let mut proxy_cfg = LocalQuicProxyConfig::default();
        if let Some(bind) = proxy_opts.bind_addr.as_ref() {
            let trimmed = bind.trim();
            if trimmed.is_empty() {
                return Err(PyValueError::new_err(
                    "local_proxy.bind_addr must not be empty when provided",
                ));
            }
            proxy_cfg.bind_addr = trimmed.to_string();
        }
        proxy_cfg.telemetry_label = proxy_opts
            .telemetry_label
            .as_ref()
            .map(|label| label.trim().to_string())
            .filter(|label| !label.is_empty());
        proxy_cfg.guard_cache_key_hex = proxy_opts
            .guard_cache_key_hex
            .as_ref()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        if let Some(flag) = proxy_opts.emit_browser_manifest {
            proxy_cfg.emit_browser_manifest = flag;
        }
        if let Some(mode) = proxy_opts.proxy_mode.as_ref() {
            proxy_cfg.proxy_mode = proxy_mode_from_label_py(mode)?;
        }
        if let Some(flag) = proxy_opts.prewarm_circuits {
            proxy_cfg.prewarm_circuits = flag;
        }
        proxy_cfg.max_streams_per_circuit = proxy_opts.max_streams_per_circuit;
        proxy_cfg.circuit_ttl_hint_secs = proxy_opts.circuit_ttl_hint_secs;

        if let Some(norito_cfg) = proxy_opts.norito_bridge.as_ref() {
            let trimmed = norito_cfg.spool_dir.trim();
            if trimmed.is_empty() {
                return Err(PyValueError::new_err(
                    "local_proxy.norito_bridge.spool_dir must not be empty when provided",
                ));
            }
            proxy_cfg.norito_bridge = Some(ProxyNoritoBridgeConfig {
                spool_dir: trimmed.to_string(),
                extension: norito_cfg
                    .extension
                    .as_ref()
                    .map(|ext| ext.trim().to_string())
                    .filter(|ext| !ext.is_empty()),
            });
        }
        if let Some(car_cfg) = proxy_opts.car_bridge.as_ref() {
            let trimmed = car_cfg.cache_dir.trim();
            if trimmed.is_empty() {
                return Err(PyValueError::new_err(
                    "local_proxy.car_bridge.cache_dir must not be empty when provided",
                ));
            }
            proxy_cfg.car_bridge = Some(ProxyCarBridgeConfig {
                cache_dir: trimmed.to_string(),
                extension: car_cfg
                    .extension
                    .as_ref()
                    .map(|ext| ext.trim().to_string())
                    .filter(|ext| !ext.is_empty()),
                allow_zst: car_cfg.allow_zst.unwrap_or(false),
            });
        }
        if let Some(kaigi_cfg) = proxy_opts.kaigi_bridge.as_ref() {
            let trimmed = kaigi_cfg.spool_dir.trim();
            if trimmed.is_empty() {
                return Err(PyValueError::new_err(
                    "local_proxy.kaigi_bridge.spool_dir must not be empty when provided",
                ));
            }
            let room_policy = if let Some(policy) = kaigi_cfg.room_policy.as_ref() {
                let normalized = policy.trim().to_ascii_lowercase();
                match normalized.as_str() {
                    "public" | "authenticated" => Some(normalized),
                    _ => {
                        return Err(PyValueError::new_err(
                            "local_proxy.kaigi_bridge.room_policy must be 'public' or 'authenticated'",
                        ));
                    }
                }
            } else {
                None
            };
            proxy_cfg.kaigi_bridge = Some(ProxyKaigiBridgeConfig {
                spool_dir: trimmed.to_string(),
                extension: kaigi_cfg
                    .extension
                    .as_ref()
                    .map(|ext| ext.trim().to_string())
                    .filter(|ext| !ext.is_empty()),
                room_policy,
            });
        }

        if matches!(proxy_cfg.proxy_mode, ProxyMode::Bridge) && proxy_cfg.norito_bridge.is_none() {
            proxy_cfg.norito_bridge = Some(ProxyNoritoBridgeConfig {
                spool_dir: defaults::streaming::soranet::PROVISION_SPOOL_DIR.to_string(),
                extension: Some("norito".to_string()),
            });
        }
        if matches!(proxy_cfg.proxy_mode, ProxyMode::Bridge) && proxy_cfg.kaigi_bridge.is_none() {
            proxy_cfg.kaigi_bridge = Some(ProxyKaigiBridgeConfig {
                spool_dir: defaults::streaming::soranet::PROVISION_SPOOL_DIR.to_string(),
                extension: Some("norito".to_string()),
                room_policy: Some("public".to_string()),
            });
        }

        orchestrator_config.local_proxy = Some(proxy_cfg);
    }
    if let Some(cache_opts) = options.taikai_cache.as_ref() {
        let cache_cfg = py_taikai_cache_to_internal(cache_opts)?;
        orchestrator_config.taikai_cache = Some(cache_cfg);
    }
    let local_proxy_snapshot = orchestrator_config.local_proxy.clone();

    let gateway_provider_count = provider_inputs.len();
    let gateway_config = GatewayFetchConfig {
        manifest_id_hex: manifest_id.clone(),
        chunker_handle: chunker_handle_trimmed.to_string(),
        manifest_envelope_b64,
        client_id,
        expected_manifest_cid_hex: manifest_cid_hex,
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version,
        moderation_token_key_b64,
    };
    let metadata = build_gateway_metadata_dict(
        py,
        0,
        gateway_provider_count,
        &orchestrator_config,
        scoreboard_telemetry_label.as_deref(),
        telemetry_region.as_deref(),
        manifest_id.as_str(),
        manifest_cid_metadata.as_deref(),
        manifest_envelope_present,
        false,
    )?;

    let runtime = Runtime::new().map_err(|err| {
        PyValueError::new_err(format!("failed to initialise Tokio runtime: {err}"))
    })?;
    let session = runtime
        .block_on(fetch_via_gateway(
            orchestrator_config,
            &plan,
            gateway_config,
            provider_inputs,
            None::<&TelemetrySnapshot>,
            max_peers,
        ))
        .map_err(|err| PyValueError::new_err(format!("sorafs gateway fetch failed: {err}")))?;

    let outcome = &session.outcome;
    let policy_report = &session.policy_report;

    let payload_bytes = outcome.assemble_payload();
    let result = PyDict::new(py);
    result.set_item("manifest_id_hex", manifest_id)?;
    result.set_item("chunker_handle", chunker_handle_trimmed)?;
    result.set_item("chunk_count", outcome.chunks.len())?;
    result.set_item("assembled_bytes", payload_bytes.len())?;
    result.set_item("payload", PyBytes::new(py, &payload_bytes))?;
    result.set_item("metadata", metadata.clone_ref(py))?;
    if let Some(region) = telemetry_region.as_ref() {
        result.set_item("telemetry_region", region)?;
    }
    result.set_item(
        "anonymity_policy",
        anonymity_policy_label(policy_report.policy),
    )?;
    result.set_item("anonymity_status", policy_report.status_label())?;
    result.set_item("anonymity_reason", policy_report.reason_label())?;
    result.set_item(
        "anonymity_soranet_selected",
        policy_report.selected_soranet_total,
    )?;
    result.set_item("anonymity_pq_selected", policy_report.selected_pq)?;
    result.set_item(
        "anonymity_classical_selected",
        policy_report.selected_classical(),
    )?;
    result.set_item("anonymity_classical_ratio", policy_report.classical_ratio())?;
    result.set_item("anonymity_pq_ratio", policy_report.pq_ratio())?;
    result.set_item("anonymity_candidate_ratio", policy_report.candidate_ratio())?;
    result.set_item("anonymity_deficit_ratio", policy_report.deficit_ratio())?;
    result.set_item("anonymity_supply_delta", policy_report.supply_delta_ratio())?;
    result.set_item("anonymity_brownout", policy_report.is_brownout())?;
    result.set_item(
        "anonymity_brownout_effective",
        policy_report.should_flag_brownout(),
    )?;
    result.set_item("anonymity_uses_classical", policy_report.uses_classical())?;

    let provider_reports = PyList::empty(py);
    for report in &outcome.provider_reports {
        let entry = PyDict::new(py);
        entry.set_item("provider", report.provider.id().as_str())?;
        entry.set_item("successes", report.successes)?;
        entry.set_item("failures", report.failures)?;
        entry.set_item("disabled", report.disabled)?;
        provider_reports.append(entry)?;
    }
    result.set_item("provider_reports", provider_reports)?;

    let receipts = PyList::empty(py);
    for receipt in &outcome.chunk_receipts {
        let entry = PyDict::new(py);
        entry.set_item("chunk_index", receipt.chunk_index)?;
        entry.set_item("provider", receipt.provider.as_str())?;
        entry.set_item("attempts", receipt.attempts)?;
        entry.set_item("latency_ms", receipt.latency_ms)?;
        entry.set_item("bytes", receipt.bytes)?;
        receipts.append(entry)?;
    }
    result.set_item("chunk_receipts", receipts)?;
    if let Some(manifest) = &session.local_proxy_manifest {
        let manifest_value = json::to_value(manifest).map_err(|err| {
            PyValueError::new_err(format!("failed to serialise local proxy manifest: {err}"))
        })?;
        let manifest_json = json::to_string(&manifest_value).map_err(|err| {
            PyValueError::new_err(format!("failed to serialise local proxy manifest: {err}"))
        })?;
        let json_module = py.import("json")?;
        let parsed_manifest = json_module.getattr("loads")?.call1((manifest_json,))?;
        result.set_item("local_proxy_manifest", parsed_manifest)?;
    } else {
        result.set_item("local_proxy_manifest", py.None())?;
    }
    if let Some(proxy_cfg) = local_proxy_snapshot.as_ref() {
        result.set_item("local_proxy_mode", proxy_cfg.proxy_mode.as_str())?;
        if let Some(bridge) = proxy_cfg.norito_bridge.as_ref() {
            result.set_item("local_proxy_norito_spool", bridge.spool_dir.clone())?;
        } else {
            result.set_item("local_proxy_norito_spool", py.None())?;
        }
        if let Some(bridge) = proxy_cfg.kaigi_bridge.as_ref() {
            result.set_item("local_proxy_kaigi_spool", bridge.spool_dir.clone())?;
            let policy = bridge
                .room_policy
                .clone()
                .unwrap_or_else(|| "public".to_string());
            result.set_item("local_proxy_kaigi_policy", policy)?;
        } else {
            result.set_item("local_proxy_kaigi_spool", py.None())?;
            result.set_item("local_proxy_kaigi_policy", py.None())?;
        }
    } else {
        result.set_item("local_proxy_mode", py.None())?;
        result.set_item("local_proxy_norito_spool", py.None())?;
        result.set_item("local_proxy_kaigi_spool", py.None())?;
        result.set_item("local_proxy_kaigi_policy", py.None())?;
    }
    if let Some(verification) = &session.car_verification {
        let car_dict = PyDict::new(py);
        car_dict.set_item(
            "manifest_digest_hex",
            hex_encode(verification.manifest_digest.as_bytes()),
        )?;
        car_dict.set_item(
            "manifest_payload_digest_hex",
            hex_encode(verification.manifest_payload_digest.as_bytes()),
        )?;
        car_dict.set_item(
            "manifest_car_digest_hex",
            hex_encode(verification.manifest_car_digest),
        )?;
        car_dict.set_item(
            "manifest_content_length",
            verification.manifest_content_length,
        )?;
        car_dict.set_item("manifest_chunk_count", verification.manifest_chunk_count)?;
        car_dict.set_item(
            "manifest_chunk_profile_handle",
            verification.chunk_profile_handle.clone(),
        )?;
        let signatures = PyList::empty(py);
        for signature in &verification.manifest_governance.council_signatures {
            let entry = PyDict::new(py);
            entry.set_item("signer_hex", hex_encode(signature.signer))?;
            entry.set_item("signature_hex", hex_encode(&signature.signature))?;
            signatures.append(entry)?;
        }
        let governance_obj = PyDict::new(py);
        governance_obj.set_item("council_signatures", signatures)?;
        car_dict.set_item("manifest_governance", governance_obj)?;

        let car_obj = PyDict::new(py);
        car_obj.set_item("size", verification.car_stats.car_size)?;
        car_obj.set_item(
            "payload_digest_hex",
            hex_encode(verification.car_stats.car_payload_digest.as_bytes()),
        )?;
        car_obj.set_item(
            "archive_digest_hex",
            hex_encode(verification.car_stats.car_archive_digest.as_bytes()),
        )?;
        car_obj.set_item("cid_hex", hex_encode(&verification.car_stats.car_cid))?;
        let roots = PyList::empty(py);
        for cid in &verification.car_stats.root_cids {
            roots.append(hex_encode(cid))?;
        }
        car_obj.set_item("root_cids_hex", roots)?;
        car_obj.set_item("verified", true)?;
        car_obj.set_item("por_leaf_count", verification.por_leaf_count)?;
        car_dict.set_item("car_archive", car_obj)?;

        result.set_item("car_verification", car_dict)?;
    } else {
        result.set_item("car_verification", py.None())?;
    }

    if let Some(cache_stats) = session.taikai_cache_stats {
        let summary = taikai_cache_stats_payload(py, cache_stats)?;
        result.set_item("taikai_cache_summary", summary)?;
    } else {
        result.set_item("taikai_cache_summary", py.None())?;
    }
    if let Some(queue_stats) = session.taikai_cache_queue {
        let queue = taikai_queue_stats_payload(py, queue_stats)?;
        result.set_item("taikai_cache_queue", queue)?;
    } else {
        result.set_item("taikai_cache_queue", py.None())?;
    }

    Ok(result.unbind())
}

fn anonymity_policy_label(policy: AnonymityPolicy) -> &'static str {
    match policy {
        AnonymityPolicy::GuardPq => "anon-guard-pq",
        AnonymityPolicy::MajorityPq => "anon-majority-pq",
        AnonymityPolicy::StrictPq => "anon-strict-pq",
    }
}

#[pyfunction]
#[pyo3(name = "sorafs_decode_replication_order")]
fn sorafs_decode_replication_order_py(py: Python<'_>, norito_bytes: &[u8]) -> PyResult<Py<PyDict>> {
    let order: ReplicationOrderV1 = decode_from_bytes(norito_bytes).map_err(|err| {
        PyValueError::new_err(format!("failed to decode replication order: {err}"))
    })?;
    order
        .validate()
        .map_err(|err| PyValueError::new_err(format!("invalid replication order: {err}")))?;

    let ReplicationOrderV1 {
        version,
        order_id,
        manifest_cid,
        manifest_digest,
        chunking_profile,
        target_replicas,
        assignments,
        issued_at,
        deadline_at,
        sla,
        metadata,
    } = order;

    let dict = PyDict::new(py);
    dict.set_item("schema_version", version)?;
    dict.set_item("order_id_hex", hex::encode(order_id))?;

    let manifest_cid_base64 = BASE64.encode(&manifest_cid);
    dict.set_item("manifest_cid_base64", manifest_cid_base64)?;
    match String::from_utf8(manifest_cid) {
        Ok(value) => dict.set_item("manifest_cid_utf8", value)?,
        Err(_) => dict.set_item("manifest_cid_utf8", py.None())?,
    }

    dict.set_item("manifest_digest_hex", hex::encode(manifest_digest))?;
    dict.set_item("chunking_profile", chunking_profile)?;
    dict.set_item("target_replicas", u32::from(target_replicas))?;
    dict.set_item("issued_at_unix", issued_at)?;
    dict.set_item("deadline_at_unix", deadline_at)?;

    let sla_dict = PyDict::new(py);
    sla_dict.set_item("ingest_deadline_secs", sla.ingest_deadline_secs)?;
    sla_dict.set_item(
        "min_availability_percent_milli",
        sla.min_availability_percent_milli,
    )?;
    sla_dict.set_item(
        "min_por_success_percent_milli",
        sla.min_por_success_percent_milli,
    )?;
    dict.set_item("sla", sla_dict)?;

    let assignments_list = PyList::empty(py);
    for assignment in assignments {
        let entry = PyDict::new(py);
        entry.set_item("provider_id_hex", hex::encode(assignment.provider_id))?;
        entry.set_item("slice_gib", assignment.slice_gib)?;
        match assignment.lane {
            Some(lane) => entry.set_item("lane", lane)?,
            None => entry.set_item("lane", py.None())?,
        }
        assignments_list.append(entry)?;
    }
    dict.set_item("assignments", assignments_list)?;

    let metadata_list = PyList::empty(py);
    for entry in metadata {
        let meta = PyDict::new(py);
        meta.set_item("key", entry.key)?;
        meta.set_item("value", entry.value)?;
        metadata_list.append(meta)?;
    }
    dict.set_item("metadata", metadata_list)?;

    Ok(dict.unbind())
}

#[pyfunction]
#[pyo3(name = "decode_transaction_receipt_json")]
fn decode_transaction_receipt_json_py(receipt_bytes: &[u8]) -> PyResult<String> {
    let receipt: TransactionSubmissionReceipt =
        decode_from_bytes(receipt_bytes).map_err(|err| {
            PyValueError::new_err(format!("failed to decode transaction receipt: {err}"))
        })?;
    json::to_json(&receipt)
        .map_err(|err| PyValueError::new_err(format!("failed to serialize receipt: {err}")))
}

#[pyfunction]
#[pyo3(name = "zk_ace_build_transfer_authorization_v1", signature = (
    from_account_id,
    to_account_id,
    asset_definition_id,
    amount,
    chain_id,
    identity_root,
    identity_blinding,
    replay_secret,
    policy_hash,
    verifier_key_id = None,
    vk_commitment = None
))]
#[allow(clippy::too_many_arguments)]
fn zk_ace_build_transfer_authorization_v1_py(
    from_account_id: &str,
    to_account_id: &str,
    asset_definition_id: &str,
    amount: &str,
    chain_id: &str,
    identity_root: &Bound<'_, PyAny>,
    identity_blinding: &Bound<'_, PyAny>,
    replay_secret: &Bound<'_, PyAny>,
    policy_hash: &Bound<'_, PyAny>,
    verifier_key_id: Option<&Bound<'_, PyAny>>,
    vk_commitment: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let from = parse_account_id(from_account_id)?;
    let to = parse_account_id(to_account_id)?;
    let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        PyValueError::new_err(format!(
            "invalid asset definition id `{asset_definition_id}`: {err}"
        ))
    })?;
    let amount = parse_u128_text(amount, "amount")?;
    let chain_id = parse_chain_id(chain_id)?;
    let witness = ZkAceWitnessV1 {
        identity_root: py_non_zero_fixed_array::<32>(identity_root, "identity_root")?,
        identity_blinding: py_non_zero_fixed_array::<32>(identity_blinding, "identity_blinding")?,
        replay_secret: py_non_zero_fixed_array::<32>(replay_secret, "replay_secret")?,
    };
    let policy_hash = py_non_zero_fixed_array::<32>(policy_hash, "policy_hash")?;
    let verifier_key_id =
        parse_optional_zk_ace_verifying_key_id_py(verifier_key_id, "verifier_key_id")?;
    let vk_commitment = match vk_commitment {
        Some(value) if !value.is_none() => py_fixed_array::<32>(value, "vk_commitment")?,
        _ => zk_ace_prover::zk_ace_verifying_key_commitment_v1().map_err(|err| {
            PyValueError::new_err(format!("failed to build ZK-ACE verifier commitment: {err}"))
        })?,
    };
    let authorization = zk_ace_prover::build_zk_ace_transfer_authorization_v1(
        from,
        to,
        asset,
        amount,
        chain_id,
        witness,
        policy_hash,
        verifier_key_id,
        vk_commitment,
    )
    .map_err(|err| PyValueError::new_err(format!("failed to build ZK-ACE proof: {err}")))?;
    zk_ace_authorization_json(
        &authorization.public_inputs,
        &authorization.proof,
        &authorization.public_inputs_bytes,
    )
}

fn decode_kagemusha_recursive_archive<T>(archive: &[u8], context: &str) -> PyResult<T>
where
    T: for<'de> norito::core::NoritoDeserialize<'de>,
{
    if archive.is_empty() {
        return Err(PyValueError::new_err(format!(
            "{context} archive must not be empty"
        )));
    }
    decode_from_bytes(archive)
        .map_err(|err| PyValueError::new_err(format!("invalid {context} archive: {err}")))
}

fn encode_kagemusha_recursive_archive<T>(
    py: Python<'_>,
    value: &T,
    context: &str,
) -> PyResult<Py<PyBytes>>
where
    T: norito::core::NoritoSerialize,
{
    let bytes = norito::to_bytes(value)
        .map_err(|err| PyRuntimeError::new_err(format!("{context}: {err}")))?;
    Ok(Py::from(PyBytes::new(py, &bytes)))
}

fn is_kagemusha_recursive_compact_unavailable_error(err: &str) -> bool {
    matches!(
        err,
        iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE
            | iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_prove_verified_compact_payment_token_with_records")]
fn kagemusha_prove_verified_compact_payment_token_with_records_py(
    py: Python<'_>,
    record_bundle_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle =
        decode_kagemusha_recursive_archive(
            record_bundle_archive,
            "Kagemusha verified compact-token record bundle",
        )?;
    let vk_box = iroha_core::zk::kagemusha_folded_vk_box().map_err(PyRuntimeError::new_err)?;
    let token = iroha_core::zk::prove_verified_kagemusha_compact_payment_token_from_record_bundle(
        &record_bundle,
        iroha_core::zk::KAGEMUSHA_FOLDED_CIRCUIT_ID,
        &vk_box,
        None,
    )
    .map_err(PyRuntimeError::new_err)?;
    encode_kagemusha_recursive_archive(
        py,
        &token,
        "failed to encode Kagemusha compact payment token",
    )
}

#[pyfunction]
#[pyo3(
    name = "kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes"
)]
fn kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes_py(
    py: Python<'_>,
    record_bundle_archive: &[u8],
    pallas_open_envelopes_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle =
        decode_kagemusha_recursive_archive(
            record_bundle_archive,
            "Kagemusha recursive aggregation record bundle",
        )?;
    if pallas_open_envelopes_archive.is_empty() {
        return Err(PyValueError::new_err(
            "Kagemusha recursive aggregation Pallas open-envelope archive must not be empty",
        ));
    }
    let vk_box = iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
        .map_err(PyRuntimeError::new_err)?;
    let bundle =
        iroha_core::zk::prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive(
            &record_bundle,
            pallas_open_envelopes_archive,
            iroha_core::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
            &vk_box,
            None,
        )
        .map_err(PyRuntimeError::new_err)?;
    encode_kagemusha_recursive_archive(
        py,
        &bundle,
        "failed to encode Kagemusha recursive aggregation proof bundle",
    )
}

#[pyfunction]
#[pyo3(
    name = "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
)]
fn kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
    py: Python<'_>,
    record_bundle_archive: &[u8],
    pallas_open_envelopes_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle =
        decode_kagemusha_recursive_archive(
            record_bundle_archive,
            "Kagemusha recursive compact record bundle",
        )?;
    if pallas_open_envelopes_archive.is_empty() {
        return Err(PyValueError::new_err(
            "pallas_open_envelopes_archive must not be empty",
        ));
    }
    let token =
        iroha_core::zk::prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive(
            &record_bundle,
            pallas_open_envelopes_archive,
            None,
        )
        .map_err(|err| {
            if err.starts_with(
                "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
            ) || err.starts_with(
                "invalid Kagemusha recursive compact Pallas open-envelope archive",
            ) || err.starts_with(
                "invalid Kagemusha recursive compact record-backed Pallas preflight",
            ) {
                return PyValueError::new_err(err.replacen("failed to decode", "invalid", 1));
            }
            PyRuntimeError::new_err(err)
        })?;
    encode_kagemusha_recursive_archive(
        py,
        &token,
        "failed to encode Kagemusha recursive compact payment token",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_verify_recursive_compact_payment_token")]
fn kagemusha_verify_recursive_compact_payment_token_py(
    compact_token_archive: &[u8],
) -> PyResult<bool> {
    let token: iroha_data_model::offline::KagemushaCompactPaymentToken =
        decode_kagemusha_recursive_archive(
            compact_token_archive,
            "Kagemusha recursive compact payment token",
        )?;
    let vk_box = iroha_core::zk::kagemusha_recursive_compact_payment_token_vk_box()
        .map_err(PyRuntimeError::new_err)?;
    match iroha_core::zk::preverify_kagemusha_recursive_compact_payment_token(&token, &vk_box) {
        Err(err) if is_kagemusha_recursive_compact_unavailable_error(&err) => {
            return Ok(false);
        }
        Err(err) => return Err(PyValueError::new_err(err)),
        Ok(()) => {}
    }
    if iroha_core::zk::verify_kagemusha_recursive_compact_payment_token(&token, &vk_box) {
        return Ok(true);
    }
    Ok(false)
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_bridge_abi_version")]
fn kagemusha_recursive_spend_bridge_abi_version_py() -> u32 {
    7
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_init")]
fn kagemusha_recursive_spend_init_py(
    py: Python<'_>,
    request_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 =
        decode_kagemusha_recursive_archive(request_archive, "Kagemusha recursive spend init")?;
    request
        .validate_public_binding()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    let lineage_verifier_key = request.lineage_verifier_key.as_ref().ok_or_else(|| {
        PyValueError::new_err("Kagemusha Reserved-lineage init requires lineage_verifier_key")
    })?;
    let lineage_proving_key_archive =
        request
            .lineage_proving_key_archive
            .as_deref()
            .ok_or_else(|| {
                PyValueError::new_err(
                    "Kagemusha Reserved-lineage init requires lineage_proving_key_archive",
                )
            })?;
    let bundle = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::prove_kagemusha_recursive_spend_lineage_init_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts_at_height(
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
                request.current_note,
                lineage_verifier_key,
                lineage_proving_key_archive,
                block_height,
            )
        }
        None => {
            iroha_core::zk::prove_kagemusha_recursive_spend_lineage_init_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts(
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
                request.current_note,
                lineage_verifier_key,
                lineage_proving_key_archive,
            )
        }
    }
    .map_err(PyRuntimeError::new_err)?;
    encode_kagemusha_recursive_archive(
        py,
        &bundle,
        "failed to encode Kagemusha recursive spend init bundle",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_append")]
fn kagemusha_recursive_spend_append_py(
    py: Python<'_>,
    request_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 =
        decode_kagemusha_recursive_archive(request_archive, "Kagemusha recursive spend append")?;
    request
        .validate_public_binding()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    let output_proof_circuit_id = request.output_proof_circuit_id().to_owned();
    let output_append_is_currently_provable =
        iroha_data_model::offline::can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
            output_proof_circuit_id.as_str(),
            request.previous_bundle.accumulator.hop_count,
        );
    if !output_append_is_currently_provable {
        return Err(PyRuntimeError::new_err(format!(
            "Kagemusha recursive spend append cannot prove output proof circuit `{}` at previous hop {}",
            output_proof_circuit_id, request.previous_bundle.accumulator.hop_count,
        )));
    }
    let mut lineage_proving_key_archive = None;
    let vk_box = match output_proof_circuit_id.as_str() {
        iroha_core::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID => {
            iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
                .map_err(PyRuntimeError::new_err)?
        }
        output_circuit
            if iroha_data_model::offline::is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
                output_circuit,
            ) =>
        {
            lineage_proving_key_archive =
                Some(request.lineage_proving_key_archive.as_deref().ok_or_else(|| {
                    PyValueError::new_err(
                        "Kagemusha Reserved-lineage append requires lineage_proving_key_archive",
                    )
                })?);
            request.lineage_verifier_key.clone().ok_or_else(|| {
                PyValueError::new_err(
                    "Kagemusha Reserved-lineage append requires lineage_verifier_key",
                )
            })?
        }
        other => {
            return Err(PyRuntimeError::new_err(format!(
                "Kagemusha recursive spend append requires a supported output proof circuit id (found `{other}`)"
            )));
        }
    };
    let bundle = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::prove_kagemusha_recursive_spend_append_from_record_bundle_and_pallas_open_envelope_archive_at_height(
                &request.previous_bundle,
                request.previous_lineage_verifier_record.as_ref(),
                &request.previous_recursive_proof_open_envelopes_archive,
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
                request.current_note,
                output_proof_circuit_id.as_str(),
                &vk_box,
                lineage_proving_key_archive,
                block_height,
            )
        }
        None => {
            iroha_core::zk::prove_kagemusha_recursive_spend_append_from_record_bundle_and_pallas_open_envelope_archive(
                &request.previous_bundle,
                request.previous_lineage_verifier_record.as_ref(),
                &request.previous_recursive_proof_open_envelopes_archive,
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
                request.current_note,
                output_proof_circuit_id.as_str(),
                &vk_box,
                lineage_proving_key_archive,
            )
        }
    }
    .map_err(PyRuntimeError::new_err)?;
    encode_kagemusha_recursive_archive(
        py,
        &bundle,
        "failed to encode Kagemusha recursive spend append bundle",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_transition_profile_init")]
fn kagemusha_recursive_spend_transition_profile_init_py(
    py: Python<'_>,
    request_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 =
        decode_kagemusha_recursive_archive(
            request_archive,
            "Kagemusha recursive spend transition profile init",
        )?;
    request
        .validate_public_binding()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
        norito::decode_from_bytes(&request.pallas_open_envelopes_archive).map_err(|err| {
            PyValueError::new_err(format!(
                "invalid Kagemusha recursive spend Pallas open-envelope archive: {err}"
            ))
        })?;
    let evidence = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes_at_height(
                &request.record_bundle,
                &envelopes,
                block_height,
            )
        }
        None => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes(
                &request.record_bundle,
                &envelopes,
            )
        }
    }
    .map_err(PyRuntimeError::new_err)?;
    let profile =
        iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_from_initial_evidence(
            &evidence,
            &request.current_note,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    encode_kagemusha_recursive_archive(
        py,
        &profile,
        "failed to encode Kagemusha recursive spend transition profile",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_transition_profile_append")]
fn kagemusha_recursive_spend_transition_profile_append_py(
    py: Python<'_>,
    request_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 =
        decode_kagemusha_recursive_archive(
            request_archive,
            "Kagemusha recursive spend transition profile append",
        )?;
    request
        .validate_public_binding()
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
        norito::decode_from_bytes(&request.pallas_open_envelopes_archive).map_err(|err| {
            PyValueError::new_err(format!(
                "invalid Kagemusha recursive spend Pallas open-envelope archive: {err}"
            ))
        })?;
    let evidence = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes_at_height(
                &request.record_bundle,
                &envelopes,
                block_height,
            )
        }
        None => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes(
                &request.record_bundle,
                &envelopes,
            )
        }
    }
    .map_err(PyRuntimeError::new_err)?;
    let profile = if request
        .previous_recursive_proof_open_envelopes_archive
        .is_empty()
    {
        iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
            &request.previous_bundle.accumulator,
            &request.previous_bundle.recursive_proof,
            &request.previous_recursive_proof_open_envelopes_archive,
            &evidence,
            &request.current_note,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
    } else {
        let hop = request.record_bundle.bundle.steps.first().ok_or_else(|| {
            PyValueError::new_err("Kagemusha recursive spend append request has no current hop")
        })?;
        let current_hop_proof_hash =
            iroha_core::zk::kagemusha_fold_step_proof_hash(&hop.attachment.proof)
                .map_err(PyRuntimeError::new_err)?;
        let append_opening_preflight =
            iroha_core::zk::kagemusha_recursive_spend_lineage_append_opening_preflight_from_archives(
                &request.previous_bundle,
                &request.previous_recursive_proof_open_envelopes_archive,
                &current_hop_proof_hash,
                &request.pallas_open_envelopes_archive,
            )
            .map_err(PyRuntimeError::new_err)?;
        iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
            &request.previous_bundle.accumulator,
            &request.previous_bundle.recursive_proof,
            &request.previous_recursive_proof_open_envelopes_archive,
            append_opening_preflight.contract,
            &evidence,
            &request.current_note,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
    };
    encode_kagemusha_recursive_archive(
        py,
        &profile,
        "failed to encode Kagemusha recursive spend transition profile",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_lineage_append_boundary")]
fn kagemusha_recursive_spend_lineage_append_boundary_py(
    py: Python<'_>,
    profile_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
        decode_kagemusha_recursive_archive(
            profile_archive,
            "Kagemusha recursive spend lineage append boundary",
        )?;
    let boundary =
        iroha_data_model::offline::kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
            &profile,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    boundary
        .validate_against_transition_profile(&profile)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    encode_kagemusha_recursive_archive(
        py,
        &boundary,
        "failed to encode Kagemusha recursive spend lineage append boundary",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_lineage_witness_from_init_result")]
fn kagemusha_recursive_spend_lineage_witness_from_init_result_py(
    py: Python<'_>,
    request_archive: &[u8],
    bundle_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 =
        decode_kagemusha_recursive_archive(
            request_archive,
            "Kagemusha recursive spend lineage witness init request",
        )?;
    let bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 =
        decode_kagemusha_recursive_archive(
            bundle_archive,
            "Kagemusha recursive spend lineage witness init bundle",
        )?;
    let witness =
        iroha_data_model::offline::kagemusha_recursive_spend_lineage_witness_from_init_result(
            &request, &bundle,
        )
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    encode_kagemusha_recursive_archive(
        py,
        &witness,
        "failed to encode Kagemusha recursive spend lineage witness",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_lineage_witness_append_result")]
fn kagemusha_recursive_spend_lineage_witness_append_result_py(
    py: Python<'_>,
    previous_witness_archive: &[u8],
    request_archive: &[u8],
    bundle_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let previous_witness: iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 =
        decode_kagemusha_recursive_archive(
            previous_witness_archive,
            "Kagemusha recursive spend previous lineage witness",
        )?;
    let request: iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 =
        decode_kagemusha_recursive_archive(
            request_archive,
            "Kagemusha recursive spend lineage witness append request",
        )?;
    let bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 =
        decode_kagemusha_recursive_archive(
            bundle_archive,
            "Kagemusha recursive spend lineage witness append bundle",
        )?;
    let witness =
        iroha_data_model::offline::kagemusha_recursive_spend_lineage_witness_append_result(
            &previous_witness,
            &request,
            &bundle,
        )
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    encode_kagemusha_recursive_archive(
        py,
        &witness,
        "failed to encode Kagemusha recursive spend lineage witness",
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_verify")]
fn kagemusha_recursive_spend_verify_py(
    py: Python<'_>,
    request_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1 =
        decode_kagemusha_recursive_archive(request_archive, "Kagemusha recursive spend verify")?;
    request
        .validate_public_binding()
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    let result = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::kagemusha_recursive_spend_verify_result_with_lineage_record_at_height(
                &request.bundle,
                request.lineage_verifier_record.as_ref(),
                block_height,
            )
        }
        None => iroha_core::zk::kagemusha_recursive_spend_verify_result_with_lineage_record(
            &request.bundle,
            request.lineage_verifier_record.as_ref(),
        ),
    }
    .map_err(PyRuntimeError::new_err)?;
    encode_kagemusha_recursive_archive(
        py,
        &result,
        "failed to encode Kagemusha recursive spend verify result",
    )
}

fn kagemusha_recursive_spend_redeem_instruction_from_request(
    request: iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1,
) -> Result<iroha_data_model::isi::offline::RedeemKagemushaRecursive, String> {
    request
        .validate_public_binding()
        .map_err(|err| err.to_string())?;
    if let Some(lineage_witness) = &request.lineage_witness {
        match request.bundle.recursive_proof.verifier_key_id.name.as_str() {
            iroha_core::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID => {
                let vk_box = iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()?;
                if let Some(record) = request.lineage_verifier_record.as_ref() {
                    let resolver = |id: &iroha_data_model::proof::VerifyingKeyId| {
                        if id.backend == iroha_core::zk::ZK_BACKEND_HALO2_IPA
                            && id.name == record.circuit_id
                        {
                            Some(record)
                        } else {
                            None
                        }
                    };
                    match request.block_height {
                        Some(block_height) => {
                            iroha_core::zk::verify_kagemusha_recursive_spend_lineage_witness_with_record_resolver_at_height(
                                &request.bundle,
                                lineage_witness,
                                block_height,
                                resolver,
                            )
                        }
                        None => {
                            iroha_core::zk::verify_kagemusha_recursive_spend_lineage_witness_with_record_resolver(
                                &request.bundle,
                                lineage_witness,
                                resolver,
                            )
                        }
                    }?;
                    if !iroha_core::zk::verify_kagemusha_recursive_spend_bundle(
                        &request.bundle,
                        &vk_box,
                    ) {
                        return Err(
                            "record-backed recursive Kagemusha lineage final proof did not verify"
                                .to_owned(),
                        );
                    }
                } else {
                    iroha_core::zk::verify_kagemusha_recursive_spend_lineage_witness_and_bundle_with_vk_box(
                        &request.bundle,
                        lineage_witness,
                        &vk_box,
                    )?;
                }
            }
            iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_CIRCUIT_ID => {
                let record = request.lineage_verifier_record.as_ref().ok_or_else(|| {
                    "reserved-lineage Kagemusha recursive spend redeem requires a lineage verifier record"
                        .to_owned()
                })?;
                let resolver = |id: &iroha_data_model::proof::VerifyingKeyId| {
                    if id.backend == iroha_core::zk::ZK_BACKEND_HALO2_IPA
                        && id.name == record.circuit_id
                    {
                        Some(record)
                    } else {
                        None
                    }
                };
                match request.block_height {
                    Some(block_height) => {
                        iroha_core::zk::verify_kagemusha_recursive_spend_lineage_witness_and_bundle_with_record_resolver_at_height(
                            &request.bundle,
                            lineage_witness,
                            record,
                            block_height,
                            resolver,
                        )
                    }
                    None => {
                        iroha_core::zk::verify_kagemusha_recursive_spend_lineage_witness_and_bundle_with_record_resolver(
                            &request.bundle,
                            lineage_witness,
                            record,
                            resolver,
                        )
                    }
                }?;
            }
            other => {
                return Err(format!(
                    "Kagemusha recursive spend redeem requires a supported proof circuit id (found `{other}`)"
                ));
            }
        }
    } else {
        iroha_core::zk::ensure_kagemusha_recursive_spend_chain_admission_proves_lineage(
            &request.bundle,
        )?;
        if request.bundle.recursive_proof.verifier_key_id.name
            == iroha_core::zk::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_CIRCUIT_ID
        {
            let record = request.lineage_verifier_record.as_ref().ok_or_else(|| {
                "reserved-lineage Kagemusha recursive spend redeem requires a lineage verifier record"
                    .to_owned()
            })?;
            match request.block_height {
                Some(block_height) => {
                    iroha_core::zk::preverify_kagemusha_recursive_spend_bundle_with_record_at_height(
                        &request.bundle,
                        record,
                        block_height,
                    )
                }
                None => iroha_core::zk::preverify_kagemusha_recursive_spend_bundle_with_record(
                    &request.bundle,
                    record,
                ),
            }?;
            let verified = match request.block_height {
                Some(block_height) => {
                    iroha_core::zk::verify_kagemusha_recursive_spend_bundle_with_record_at_height(
                        &request.bundle,
                        record,
                        block_height,
                    )
                }
                None => iroha_core::zk::verify_kagemusha_recursive_spend_bundle_with_record(
                    &request.bundle,
                    record,
                ),
            };
            if !verified {
                return Err(
                    "reserved-lineage Kagemusha recursive spend proof did not verify".to_owned(),
                );
            }
        }
    }
    Ok(
        iroha_data_model::isi::offline::RedeemKagemushaRecursive::new_with_lineage_witness(
            request.bundle,
            request.recipient,
            request.public_amount,
            request.redeem_proof,
            request.lineage_witness,
        ),
    )
}

#[pyfunction]
#[pyo3(name = "kagemusha_recursive_spend_redeem")]
fn kagemusha_recursive_spend_redeem_py(
    py: Python<'_>,
    request_archive: &[u8],
) -> PyResult<Py<PyBytes>> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1 =
        decode_kagemusha_recursive_archive(request_archive, "Kagemusha recursive spend redeem")?;
    let instruction =
        kagemusha_recursive_spend_redeem_instruction_from_request(request).map_err(|err| {
            PyValueError::new_err(format!(
                "invalid Kagemusha recursive spend redeem request: {err}"
            ))
        })?;
    encode_kagemusha_recursive_archive(
        py,
        &instruction,
        "failed to encode Kagemusha recursive spend redeem instruction",
    )
}

#[pyfunction]
#[pyo3(name = "generate_connect_keypair")]
/// Generate an X25519 keypair for Connect.
fn generate_connect_keypair_py(py: Python<'_>) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let scheme = X25519Sha256::new();
    let (public, secret) = scheme.try_keypair(KeyGenOption::Random).map_err(|err| {
        PyRuntimeError::new_err(format!("failed to generate X25519 keypair: {err}"))
    })?;
    let public_bytes = Py::from(PyBytes::new(py, public.as_bytes()));
    let private_bytes = Py::from(PyBytes::new(py, secret.to_bytes().as_ref()));
    Ok((private_bytes, public_bytes))
}

#[pyfunction]
#[pyo3(name = "connect_public_key_from_private")]
/// Derive the public key corresponding to an X25519 private key.
fn connect_public_key_from_private_py(py: Python<'_>, private_key: &[u8]) -> PyResult<Py<PyBytes>> {
    let secret_bytes = fixed_array::<32>(private_key, "private_key")?;
    let scheme = X25519Sha256::new();
    let static_secret = StaticSecret::from(secret_bytes);
    let (public, _) = scheme.keypair(KeyGenOption::FromPrivateKey(static_secret));
    Ok(Py::from(PyBytes::new(py, public.as_bytes())))
}

#[pyfunction]
#[pyo3(name = "derive_connect_direction_keys")]
/// Derive per-direction ChaCha20-Poly1305 keys from X25519 session material.
fn derive_connect_direction_keys_py(
    py: Python<'_>,
    local_private_key: &[u8],
    peer_public_key: &[u8],
    sid: &[u8],
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let local_sk = fixed_array::<32>(local_private_key, "local_private_key")?;
    let peer_pk = fixed_array::<32>(peer_public_key, "peer_public_key")?;
    let sid_arr = fixed_array::<32>(sid, "sid")?;
    let (k_app, k_wallet) = connect_sdk::x25519_derive_keys(&local_sk, &peer_pk, &sid_arr)
        .map_err(|err| PyValueError::new_err(format!("x25519 derive keys failed: {err}")))?;
    let app_bytes = Py::from(PyBytes::new(py, &k_app));
    let wallet_bytes = Py::from(PyBytes::new(py, &k_wallet));
    Ok((app_bytes, wallet_bytes))
}

#[pyfunction]
#[pyo3(name = "build_connect_approve_preimage")]
/// Build the canonical approval preimage for wallet signatures.
fn build_connect_approve_preimage_py(
    py: Python<'_>,
    sid: &[u8],
    app_public_key: &[u8],
    wallet_public_key: &[u8],
    account_id: &str,
    permissions: Option<&Bound<'_, PyAny>>,
    proof: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyBytes>> {
    let sid_arr = fixed_array::<32>(sid, "sid")?;
    let app_pk = fixed_array::<32>(app_public_key, "app_public_key")?;
    let wallet_pk = fixed_array::<32>(wallet_public_key, "wallet_public_key")?;

    let permissions_parsed = parse_permissions(permissions.cloned(), "permissions")?;
    let proof_parsed = parse_sign_in_proof(proof.cloned())?;

    let preimage = connect_sdk::build_approve_preimage(
        &sid_arr,
        &app_pk,
        &wallet_pk,
        account_id,
        permissions_parsed.as_ref(),
        proof_parsed.as_ref(),
    );
    Ok(Py::from(PyBytes::new(py, &preimage)))
}

#[pyfunction]
#[pyo3(name = "seal_connect_payload")]
fn seal_connect_payload_py(
    py: Python<'_>,
    key: &[u8],
    sid: &[u8],
    direction: &str,
    sequence: u64,
    payload: &Bound<'_, PyDict>,
) -> PyResult<Py<PyBytes>> {
    let key_arr = fixed_array::<32>(key, "key")?;
    let sid_arr = fixed_array::<32>(sid, "sid")?;
    let dir = parse_connect_direction(direction)?;
    let payload = parse_connect_payload(payload)?;
    let frame = connect_sdk::seal_envelope(&key_arr, &sid_arr, dir, sequence, payload);
    let encoded = codec::Encode::encode(&frame);
    Ok(Py::from(PyBytes::new(py, encoded.as_slice())))
}

#[pyfunction]
#[pyo3(name = "open_connect_payload")]
fn open_connect_payload_py(py: Python<'_>, key: &[u8], frame_bytes: &[u8]) -> PyResult<Py<PyDict>> {
    let key_arr = fixed_array::<32>(key, "key")?;
    let frame = decode_connect_frame_bytes(frame_bytes)?;
    let envelope = connect_sdk::open_envelope(&key_arr, &frame).map_err(|err| {
        PyValueError::new_err(format!("failed to decrypt connect payload: {err}"))
    })?;
    let mapping = PyDict::new(py);
    mapping.set_item("seq", envelope.seq)?;
    let payload_dict = encode_connect_payload(py, &envelope.payload)?;
    mapping.set_item("payload", payload_dict)?;
    Ok(mapping.unbind())
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::OnceLock};

    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::SigningKey;
    use http::StatusCode;
    use httpmock::{MockServer, prelude::*};
    use iroha_core::zk::{ZK_BACKEND_HALO2_IPA, kagemusha_recursive_spend_bundle_instance_values};
    use iroha_data_model::offline::{
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, KagemushaRecursiveAggregationProof,
        KagemushaRecursiveSpendAccumulatorV1, KagemushaRecursiveSpendAppendRequestV1,
        KagemushaRecursiveSpendBundleV1, KagemushaRecursiveSpendInitRequestV1,
        KagemushaRecursiveSpendLineageWitnessV1, KagemushaRecursiveSpendRedeemRequestV1,
        KagemushaRecursiveSpendTransitionProfileV1, KagemushaRecursiveSpendVerifyRequestV1,
        KagemushaRecursiveSpendVerifyResultV1, KagemushaSpendableNoteDescriptorV1,
        KagemushaVerifiedFoldBundle, KagemushaVerifiedFoldRecordBundle, KagemushaVerifiedFoldStep,
        KagemushaVerifiedFoldVerifierRecord,
        kagemusha_recursive_spend_public_inputs_from_accumulator,
    };
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        proof::{VerifyingKeyBox, VerifyingKeyId},
        zk::{BackendTag, OpenVerifyEnvelope},
    };
    use ivm::bn254_vec::{self, FieldElem};
    use norito::to_bytes;
    use once_cell::sync::OnceCell;
    use pyo3::{
        Python,
        types::{PyBytes, PyDict, PyList},
    };
    use sorafs_car::{CarWriter, multi_fetch::PolicyBlockEvidence};
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, CouncilSignature, DagCodecId, GovernanceProofs, ManifestBuilder,
        PinPolicy, StreamTokenBodyV1, StreamTokenV1,
    };
    use tempfile::tempdir;

    use super::*;

    const SAMPLE_RWA_ID: &str =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities.universal";

    fn ensure_python() {
        static INIT: OnceCell<()> = OnceCell::new();
        INIT.get_or_init(|| {
            Python::initialize();
        });
    }

    fn canonical_i105_from_seed(seed: u8) -> String {
        AccountId::new(PublicKey::from(parse_private_key(&[seed; 32]).unwrap()))
            .canonical_i105()
            .expect("canonical I105")
    }

    fn sample_account(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn fixed_bytes(label: &[u8]) -> [u8; Hash::LENGTH] {
        Hash::new(label).into()
    }

    fn recursive_spend_lineage_scalar_projection(seed: u8) -> [u8; Hash::LENGTH] {
        let mut bytes = [seed; Hash::LENGTH];
        bytes[Hash::LENGTH - 1] &= 0x1f;
        bytes
    }

    fn append_zk1_tlv(buf: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        buf.extend_from_slice(&tag);
        buf.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("ZK1 TLV payload length fits u32")
                .to_le_bytes(),
        );
        buf.extend_from_slice(payload);
    }

    fn append_zk1_raw_instance_columns(buf: &mut Vec<u8>, columns: Vec<Vec<[u8; 32]>>) {
        let rows = columns
            .first()
            .map(Vec::len)
            .expect("recursive spend public instances are non-empty");
        assert!(
            columns.iter().all(|column| column.len() == rows),
            "recursive spend public instance columns have equal row counts"
        );
        let mut payload = Vec::with_capacity(8 + rows * columns.len() * Hash::LENGTH);
        payload.extend_from_slice(
            &u32::try_from(columns.len())
                .expect("recursive spend public instance column count fits u32")
                .to_le_bytes(),
        );
        payload.extend_from_slice(
            &u32::try_from(rows)
                .expect("recursive spend public instance row count fits u32")
                .to_le_bytes(),
        );
        for row in 0..rows {
            for column in &columns {
                payload.extend_from_slice(&column[row]);
            }
        }
        append_zk1_tlv(buf, *b"I10P", &payload);
    }

    fn privacy_request(
        algorithm_id: &str,
        entrypoint: &str,
        proof: Vec<u8>,
    ) -> PrivacyProofRequestV1 {
        let vk_backend = privacy_algorithm_entry(algorithm_id)
            .map(|entry| entry.backend_family)
            .unwrap_or("unknown");
        let vk_name = privacy_algorithm_entry(algorithm_id)
            .map(privacy_canonical_vk_ref_name)
            .unwrap_or_else(|| "vk_unknown".to_owned());
        PrivacyProofRequestV1 {
            algorithm_id: algorithm_id.to_owned(),
            entrypoint: entrypoint.to_owned(),
            vk_ref: format!("{vk_backend}:{vk_name}"),
            public_inputs: b"public-inputs".to_vec(),
            witness: b"secret-witness".to_vec(),
            proof,
        }
    }

    fn public_privacy_request_archive(request: &PrivacyProofRequestV1) -> Vec<u8> {
        let mut archive = norito::to_bytes(request).expect("encode privacy request");
        assert!(
            privacy_patch_archive_repeated_schema_byte(&mut archive, PRIVACY_REQUEST_SCHEMA_BYTE),
            "privacy request archive must carry a complete Norito schema hash slot"
        );
        archive
    }

    fn normalize_privacy_public_archive_for_decode<T>(bytes: &mut [u8])
    where
        T: norito::NoritoSerialize,
    {
        if [
            PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE,
            PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE,
            PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE,
        ]
        .into_iter()
        .any(|schema_byte| privacy_archive_has_repeated_schema_byte(bytes, schema_byte))
        {
            assert!(
                privacy_patch_archive_schema_hash(
                    bytes,
                    <T as norito::NoritoSerialize>::schema_hash()
                ),
                "privacy archive must carry a complete Norito schema hash slot"
            );
        }
    }

    fn adversarial_privacy_request_archives() -> Vec<(&'static str, Vec<u8>)> {
        let request = privacy_request(
            "orchard-halo2-actions-v1",
            "buildOrchardActionBundleProofV1",
            b"candidate-proof".to_vec(),
        );
        let valid = public_privacy_request_archive(&request);
        assert!(
            valid.len() > 40,
            "privacy request fixture must include a Norito V1 frame header"
        );

        let mut bad_magic = valid.clone();
        bad_magic[0] ^= 0x80;
        let mut bad_version = valid.clone();
        bad_version[4] ^= 0x01;
        let mut bad_schema = valid.clone();
        bad_schema[6] ^= 0x01;
        let mut bad_compression = valid.clone();
        bad_compression[22] ^= 0x01;
        let mut bad_payload_length = valid.clone();
        bad_payload_length[30] ^= 0x40;
        let mut bad_crc = valid.clone();
        bad_crc[31] ^= 0x01;
        let mut bad_flags = valid.clone();
        bad_flags[39] |= 0x80;
        let mut payload_tamper = valid.clone();
        let payload_last = payload_tamper
            .len()
            .checked_sub(1)
            .expect("non-empty privacy request archive");
        payload_tamper[payload_last] ^= 0x01;

        vec![
            ("truncated-header", valid[..39].to_vec()),
            ("truncated-payload", valid[..valid.len() - 1].to_vec()),
            ("bad-magic", bad_magic),
            ("bad-version", bad_version),
            ("bad-schema", bad_schema),
            ("bad-compression", bad_compression),
            ("bad-payload-length", bad_payload_length),
            ("bad-crc", bad_crc),
            ("bad-flags", bad_flags),
            ("payload-tamper", payload_tamper),
        ]
    }

    fn assert_malformed_privacy_request_result(result: &PrivacyProofResultV1, case: &str) {
        assert_eq!(result.version, PRIVACY_FFI_VERSION_V1, "{case}");
        assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR, "{case}");
        assert_eq!(
            result.error_code, PRIVACY_FFI_ERROR_MALFORMED_NORITO,
            "{case}"
        );
        assert_eq!(result.message, "malformed Norito V1 privacy proof request");
        assert!(result.algorithm_id.is_empty(), "{case}");
        assert!(result.entrypoint.is_empty(), "{case}");
        assert!(result.vk_ref.is_empty(), "{case}");
        assert!(result.public_inputs.is_empty(), "{case}");
        assert!(result.proof.is_empty(), "{case}");
        assert!(!result.verified, "{case}");
    }

    fn assert_unreflected_invalid_privacy_request_result(
        result: &PrivacyProofResultV1,
        message_fragment: &str,
        case: &str,
    ) {
        assert_eq!(result.version, PRIVACY_FFI_VERSION_V1, "{case}");
        assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR, "{case}");
        assert_eq!(
            result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "{case}"
        );
        assert!(
            result.message.contains(message_fragment),
            "{case}: {}",
            result.message
        );
        assert!(result.algorithm_id.is_empty(), "{case}");
        assert!(result.entrypoint.is_empty(), "{case}");
        assert!(result.vk_ref.is_empty(), "{case}");
        assert!(result.public_inputs.is_empty(), "{case}");
        assert!(result.proof.is_empty(), "{case}");
        assert!(!result.verified, "{case}");
    }

    fn assert_subslice_absent(haystack: &[u8], needle: &[u8], context: &str) {
        assert!(
            !needle.is_empty(),
            "privacy witness marker must be non-empty"
        );
        assert!(
            !haystack
                .windows(needle.len())
                .any(|window| window == needle),
            "{context} leaked privacy witness bytes",
        );
    }

    fn assert_privacy_result_does_not_serialize_witness(
        result: &PrivacyProofResultV1,
        witness: &[u8],
    ) {
        assert!(
            result.proof.is_empty(),
            "failed privacy result must not carry a proof"
        );
        assert_subslice_absent(result.message.as_bytes(), witness, "privacy result message");
        let encoded = norito::to_bytes(result).expect("encode privacy result");
        assert_subslice_absent(&encoded, witness, "Norito privacy result archive");
    }

    fn privacy_catalog_entry_for_test(
        id: &'static str,
        proof_family: &'static str,
        backend_family: &'static str,
        sdk_entrypoints: &'static [&'static str],
        planned_entrypoints: &'static [&'static str],
    ) -> PrivacyAlgorithmEntry {
        PrivacyAlgorithmEntry {
            id,
            proof_family,
            backend_family,
            sdk_entrypoints,
            planned_entrypoints,
        }
    }

    #[test]
    fn privacy_algorithm_catalog_is_unique_portable_and_disjoint() {
        assert!(privacy_algorithm_catalog_invariants_hold());
        assert!(
            PRIVACY_ALGORITHM_ENTRIES
                .iter()
                .any(|entry| !entry.planned_entrypoints.is_empty()),
            "catalog invariant test must cover planned entrypoint rows",
        );
        assert!(
            PRIVACY_ALGORITHM_ENTRIES.iter().any(|entry| entry
                .sdk_entrypoints
                .iter()
                .any(|entrypoint| privacy_entrypoint_is_explicit_dev_fixture(entrypoint))),
            "catalog invariant test must cover explicit DevFixture rows",
        );
        assert!(privacy_required_production_plan_rows_are_present(
            PRIVACY_ALGORITHM_ENTRIES
        ));
        assert_eq!(PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS.len(), 16);
        assert_eq!(PRIVACY_RESEARCH_TARGET_ALGORITHM_IDS.len(), 6);
        assert!(
            PRIVACY_ALGORITHM_ENTRIES
                .iter()
                .filter(|entry| privacy_algorithm_entry_is_component(entry))
                .all(
                    |entry| !privacy_entrypoints_include_ledger_mutation(entry.sdk_entrypoints)
                        && !privacy_entrypoints_include_ledger_mutation(entry.planned_entrypoints)
                ),
            "component privacy rows must remain proof-only",
        );
        assert!(
            PRIVACY_ALGORITHM_ENTRIES
                .iter()
                .filter(|entry| privacy_algorithm_entry_is_research_target(entry))
                .all(|entry| entry.sdk_entrypoints.is_empty()
                    && !entry.planned_entrypoints.is_empty()),
            "research privacy rows must keep executable SDK entrypoints planned-only",
        );
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_missing_verifier_key_name_mappings() {
        assert!(
            PRIVACY_ALGORITHM_ENTRIES
                .iter()
                .all(privacy_catalog_vk_ref_name_is_registered),
            "every catalog row must have an explicit verifier-key name mapping",
        );
        assert!(
            !privacy_algorithm_catalog_vk_ref_names_have_duplicates(PRIVACY_ALGORITHM_ENTRIES),
            "catalog verifier-key names must be unique",
        );

        const EMPTY: &[&str] = &[];
        const PLANNED_PROOF: &[&str] = &["buildUnmappedPrivacyProofV1"];
        let unmapped = privacy_catalog_entry_for_test(
            "unmapped-mainnet-privacy-row-v1",
            "halo2-ipa-pasta",
            "halo2-ipa-pasta",
            EMPTY,
            PLANNED_PROOF,
        );

        assert!(!privacy_catalog_vk_ref_name_is_registered(&unmapped));
        assert!(
            !privacy_algorithm_catalog_entries_are_valid(&[unmapped]),
            "unmapped verifier-key names must fail catalog admission",
        );
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_adversarial_duplicates_and_unportable_labels() {
        const SDK_ALPHA: &[&str] = &["buildAlphaProof"];
        const SDK_BETA: &[&str] = &["buildBetaProof"];
        const SDK_DUPLICATE: &[&str] = &["buildAlphaProof", "buildAlphaProof"];
        const SDK_EMPTY: &[&str] = &[""];
        const SDK_DELIMITED: &[&str] = &["buildAlphaProof:shadow"];
        const SDK_HYPHENATED: &[&str] = &["build-AlphaProof"];
        const SDK_LEADING_UNDERSCORE: &[&str] = &["_buildAlphaProof"];
        const SDK_TRAILING_UNDERSCORE: &[&str] = &["buildAlphaProof_"];
        const SDK_DOTTED_LEADING_UNDERSCORE: &[&str] = &["Iroha._Privacy.buildAlphaProof"];
        const SDK_DOTTED_TRAILING_UNDERSCORE: &[&str] = &["Iroha.Privacy_.buildAlphaProof"];
        const SDK_DOLLAR: &[&str] = &["build$AlphaProof"];
        const SDK_UNPORTABLE: &[&str] = &["build Alpha Proof"];
        const SDK_MAINNET_READY: &[&str] = &["buildMainnetReadyProof"];
        const PLANNED_ALPHA: &[&str] = &["buildAlphaProof"];
        const PLANNED_BETA: &[&str] = &["verifyBetaProof"];
        const PLANNED_EMPTY: &[&str] = &[""];
        const PLANNED_AUDIT_SIGNOFF: &[&str] = &["buildAuditSignoffProof"];

        let duplicate_ids = [
            privacy_catalog_entry_for_test(
                "confidential-transfer-v2",
                "halo2-ipa-pasta",
                "halo2-ipa-pasta",
                SDK_ALPHA,
                PLANNED_BETA,
            ),
            privacy_catalog_entry_for_test(
                "confidential-transfer-v2",
                "stark-fri",
                "stark-fri",
                SDK_BETA,
                PLANNED_BETA,
            ),
        ];
        assert!(
            !privacy_algorithm_catalog_entries_are_valid(&duplicate_ids),
            "duplicate algorithm IDs must be rejected",
        );
        assert!(
            PRIVACY_ALGORITHM_ENTRIES.iter().all(|entry| {
                !privacy_exposed_label_claims_production_readiness(entry.id)
                    && !privacy_exposed_label_claims_production_readiness(entry.proof_family)
                    && !privacy_exposed_label_claims_production_readiness(entry.backend_family)
                    && entry.sdk_entrypoints.iter().all(|entrypoint| {
                        !privacy_exposed_label_claims_production_readiness(entrypoint)
                    })
                    && entry.planned_entrypoints.iter().all(|entrypoint| {
                        !privacy_exposed_label_claims_production_readiness(entrypoint)
                    })
            }),
            "native privacy catalog must not expose production-ready/mainnet/audit claim labels",
        );
        for label in [
            "mainnet-ready-row",
            "claimed-production",
            "audited-production",
            "externally-audited",
            "buildAuditSignoffProof",
            "buildS.e.c.u.r.i.t.yReviewPassedProof",
        ] {
            assert!(
                privacy_exposed_label_claims_production_readiness(label),
                "{label} must be treated as a production/audit readiness claim",
            );
        }

        for (case, entry) in [
            (
                "unportable algorithm id",
                privacy_catalog_entry_for_test(
                    "bad/../algorithm",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "delimited algorithm id",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2:shadow",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "uppercase algorithm id",
                privacy_catalog_entry_for_test(
                    "Confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "leading underscore algorithm id",
                privacy_catalog_entry_for_test(
                    "_confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "leading hyphen algorithm id",
                privacy_catalog_entry_for_test(
                    "-confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "trailing underscore algorithm id",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2_",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "trailing hyphen algorithm id",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2-",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "unportable proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2 ipa pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "uppercase proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "Halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "delimited proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2:ipa:pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "empty proof-family segment",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2//ipa",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "leading slash proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "/halo2",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "leading hyphen proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "-halo2",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "trailing slash proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2/",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "trailing hyphen proof family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "unportable backend family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2/ipa/pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "delimited backend family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2:ipa:pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "uppercase backend family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "Halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "leading separator backend family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "-halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "trailing separator backend family",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta.",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "duplicate sdk entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DUPLICATE,
                    PLANNED_BETA,
                ),
            ),
            (
                "sdk planned overlap",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_ALPHA,
                ),
            ),
            (
                "unportable entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_UNPORTABLE,
                    PLANNED_BETA,
                ),
            ),
            (
                "delimited entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DELIMITED,
                    PLANNED_BETA,
                ),
            ),
            (
                "hyphenated entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_HYPHENATED,
                    PLANNED_BETA,
                ),
            ),
            (
                "leading underscore entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_LEADING_UNDERSCORE,
                    PLANNED_BETA,
                ),
            ),
            (
                "trailing underscore entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_TRAILING_UNDERSCORE,
                    PLANNED_BETA,
                ),
            ),
            (
                "dotted leading underscore entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DOTTED_LEADING_UNDERSCORE,
                    PLANNED_BETA,
                ),
            ),
            (
                "dotted trailing underscore entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DOTTED_TRAILING_UNDERSCORE,
                    PLANNED_BETA,
                ),
            ),
            (
                "dollar entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DOLLAR,
                    PLANNED_BETA,
                ),
            ),
            (
                "empty sdk entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_EMPTY,
                    PLANNED_BETA,
                ),
            ),
            (
                "empty planned entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_EMPTY,
                ),
            ),
            (
                "proof-family production-ready claim",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-production-ready",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "backend-family audit-signoff claim",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "audit-signoff-pasta",
                    SDK_ALPHA,
                    PLANNED_BETA,
                ),
            ),
            (
                "sdk entrypoint mainnet-ready claim",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_MAINNET_READY,
                    PLANNED_BETA,
                ),
            ),
            (
                "planned entrypoint audit-signoff claim",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_AUDIT_SIGNOFF,
                ),
            ),
        ] {
            assert!(
                !privacy_algorithm_catalog_entries_are_valid(&[entry]),
                "{case} must be rejected",
            );
        }
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_missing_or_misregistered_required_plan_rows() {
        const HELPER_ONLY_PLANNED: &[&str] = &["deriveOrchardWitness"];
        const PROOF_HELPER_ONLY_PLANNED: &[&str] = &["buildAnonymousPgcProofEnvelope"];

        let missing_required: Vec<PrivacyAlgorithmEntry> = PRIVACY_ALGORITHM_ENTRIES
            .iter()
            .copied()
            .filter(|entry| entry.id != "anonymous-pgc-k-out-of-n-v1")
            .collect();
        assert!(
            !privacy_required_production_plan_rows_are_present(&missing_required),
            "missing required production plan rows must be rejected",
        );

        let mut duplicate_required: Vec<PrivacyAlgorithmEntry> = PRIVACY_ALGORITHM_ENTRIES.to_vec();
        let duplicate = *PRIVACY_ALGORITHM_ENTRIES
            .iter()
            .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
            .expect("required production plan row");
        duplicate_required.push(duplicate);
        assert!(
            !privacy_required_production_plan_rows_are_present(&duplicate_required),
            "duplicate required production plan rows must be rejected",
        );

        let mut wrong_backend: Vec<PrivacyAlgorithmEntry> = PRIVACY_ALGORITHM_ENTRIES.to_vec();
        wrong_backend
            .iter_mut()
            .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
            .expect("required production plan row")
            .backend_family = "wrong-backend";
        assert!(
            !privacy_required_production_plan_rows_are_present(&wrong_backend),
            "required production plan backend drift must be rejected",
        );

        let mut wrong_proof: Vec<PrivacyAlgorithmEntry> = PRIVACY_ALGORITHM_ENTRIES.to_vec();
        wrong_proof
            .iter_mut()
            .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
            .expect("required production plan row")
            .proof_family = "wrong-proof";
        assert!(
            !privacy_required_production_plan_rows_are_present(&wrong_proof),
            "required production plan proof-family drift must be rejected",
        );

        let mut missing_planned: Vec<PrivacyAlgorithmEntry> = PRIVACY_ALGORITHM_ENTRIES.to_vec();
        missing_planned
            .iter_mut()
            .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
            .expect("required production plan row")
            .planned_entrypoints = &[];
        assert!(
            !privacy_required_production_plan_rows_are_present(&missing_planned),
            "required production plan rows must keep planned production proof builders until gates pass",
        );

        let mut helper_only_planned: Vec<PrivacyAlgorithmEntry> =
            PRIVACY_ALGORITHM_ENTRIES.to_vec();
        helper_only_planned
            .iter_mut()
            .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
            .expect("required production plan row")
            .planned_entrypoints = HELPER_ONLY_PLANNED;
        assert!(
            !privacy_required_production_plan_rows_are_present(&helper_only_planned),
            "required production plan rows must reject helper-only planned entrypoints",
        );

        let mut proof_helper_only_planned: Vec<PrivacyAlgorithmEntry> =
            PRIVACY_ALGORITHM_ENTRIES.to_vec();
        proof_helper_only_planned
            .iter_mut()
            .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
            .expect("required production plan row")
            .planned_entrypoints = PROOF_HELPER_ONLY_PLANNED;
        assert!(
            !privacy_required_production_plan_rows_are_present(&proof_helper_only_planned),
            "required production plan rows must reject proof-helper planned entrypoints",
        );
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_adversarial_fixture_and_local_verifier_entrypoints() {
        const SDK_ALPHA: &[&str] = &["buildShapeCommitment"];
        const SDK_IMPLICIT_FIXTURE: &[&str] = &["buildMockProof"];
        const SDK_LOCAL_ONLY: &[&str] = &["verifyShapeProofLocally"];
        const SDK_LOCAL_SUFFIX: &[&str] = &["verifyShapeProofLocal"];
        const SDK_LOCAL_VERIFIER_SUFFIX: &[&str] = &["verifyShapeProofLocalVerifier"];
        const SDK_DEV_ONLY: &[&str] = &["buildShapeDevProofFixture"];
        const SDK_DEV_AND_LOCAL: &[&str] =
            &["buildShapeDevProofFixture", "verifyShapeProofLocally"];
        const PLANNED_PROOF: &[&str] = &["buildShapeProductionProofV1"];
        const PLANNED_FIXTURE: &[&str] = &["buildShapeDevProofFixture"];
        const PLANNED_LOCAL: &[&str] = &["verifyShapeProofLocally"];
        const PLANNED_LOCAL_SUFFIX: &[&str] = &["verifyShapeProofLocal"];
        const PLANNED_LOCAL_VERIFIER_SUFFIX: &[&str] = &["verifyShapeProofLocalVerifier"];
        const PLANNED_INSTRUCTION: &[&str] = &["buildShapeProductionInstruction"];
        const PLANNED_PROOF_HELPER: &[&str] = &["buildShapeProofEnvelope"];
        const RESEARCH_SDK: &[&str] = &["verifySharedResearchProof"];

        for (case, entry) in [
            (
                "research target executable entrypoint",
                privacy_catalog_entry_for_test(
                    "pq-masp-stark-v0",
                    "stark-fri",
                    "pq-masp-stark-fri",
                    RESEARCH_SDK,
                    PLANNED_PROOF,
                ),
            ),
            (
                "planned fixture entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_FIXTURE,
                ),
            ),
            (
                "planned local verifier",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_LOCAL,
                ),
            ),
            (
                "planned local verifier short suffix",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_LOCAL_SUFFIX,
                ),
            ),
            (
                "planned local verifier alias suffix",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_LOCAL_VERIFIER_SUFFIX,
                ),
            ),
            (
                "implicit fixture sdk entrypoint",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_IMPLICIT_FIXTURE,
                    PLANNED_PROOF,
                ),
            ),
            (
                "local verifier without DevFixture",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_LOCAL_ONLY,
                    PLANNED_PROOF,
                ),
            ),
            (
                "local verifier short suffix without DevFixture",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_LOCAL_SUFFIX,
                    PLANNED_PROOF,
                ),
            ),
            (
                "local verifier alias suffix without DevFixture",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_LOCAL_VERIFIER_SUFFIX,
                    PLANNED_PROOF,
                ),
            ),
            (
                "DevFixture without local verifier",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DEV_ONLY,
                    PLANNED_PROOF,
                ),
            ),
            (
                "DevFixture without planned production proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DEV_AND_LOCAL,
                    PLANNED_INSTRUCTION,
                ),
            ),
            (
                "DevFixture with proof helper only",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DEV_AND_LOCAL,
                    PLANNED_PROOF_HELPER,
                ),
            ),
        ] {
            assert!(
                !privacy_algorithm_catalog_entries_are_valid(&[entry]),
                "{case} must be rejected",
            );
        }
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_component_ledger_mutation_entrypoints() {
        const SDK_PROOF_COMPONENT: &[&str] = &[
            "buildVeRangeDevProofFixture",
            "buildVeRangeProofEnvelope",
            "verifyVeRangeProofLocally",
        ];
        const SDK_INSTRUCTION: &[&str] = &["buildVeRangeInstruction"];
        const SDK_QUALIFIED_INSTRUCTION: &[&str] = &["Iroha.Privacy.buildVeRangeInstruction"];
        const PLANNED_PROOF: &[&str] = &["buildVeRangeProofV1"];
        const PLANNED_TRANSACTION: &[&str] = &["buildVeRangeTransaction"];
        const PLANNED_SUBMIT: &[&str] = &["buildSubmitVeRangeProof"];

        for (case, entry) in [
            (
                "component sdk instruction",
                privacy_catalog_entry_for_test(
                    "verange-transparent-range-v1",
                    "verange-transparent-range",
                    "verange",
                    SDK_INSTRUCTION,
                    PLANNED_PROOF,
                ),
            ),
            (
                "component qualified sdk instruction",
                privacy_catalog_entry_for_test(
                    "verange-transparent-range-v1",
                    "verange-transparent-range",
                    "verange",
                    SDK_QUALIFIED_INSTRUCTION,
                    PLANNED_PROOF,
                ),
            ),
            (
                "component planned transaction",
                privacy_catalog_entry_for_test(
                    "verange-transparent-range-v1",
                    "verange-transparent-range",
                    "verange",
                    SDK_PROOF_COMPONENT,
                    PLANNED_TRANSACTION,
                ),
            ),
            (
                "component planned submit",
                privacy_catalog_entry_for_test(
                    "verange-transparent-range-v1",
                    "verange-transparent-range",
                    "verange",
                    SDK_PROOF_COMPONENT,
                    PLANNED_SUBMIT,
                ),
            ),
        ] {
            assert!(
                !privacy_algorithm_catalog_entries_are_valid(&[entry]),
                "{case} must be rejected",
            );
        }
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_planned_ledger_mutation_without_production_proof_builder()
    {
        const SDK_ALPHA: &[&str] = &["buildShapeCommitment"];
        const SDK_DEV_AND_LOCAL: &[&str] =
            &["buildShapeDevProofFixture", "verifyShapeProofLocally"];
        const PLANNED_INSTRUCTION: &[&str] = &["buildShapeTransferInstruction"];
        const PLANNED_TRANSACTION: &[&str] = &["buildShapeAuthorizedTransaction"];
        const PLANNED_SUBMIT_PROOF: &[&str] = &["buildSubmitShapeProof"];
        const PLANNED_PROOF_AND_INSTRUCTION: &[&str] =
            &["buildShapeProofV1", "buildShapeTransferInstruction"];

        assert!(privacy_algorithm_catalog_entries_are_valid(&[
            privacy_catalog_entry_for_test(
                "confidential-transfer-v2",
                "halo2-ipa-pasta",
                "halo2-ipa-pasta",
                SDK_ALPHA,
                PLANNED_PROOF_AND_INSTRUCTION,
            )
        ]));

        for (case, entry) in [
            (
                "planned instruction without production proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_INSTRUCTION,
                ),
            ),
            (
                "planned transaction without production proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_TRANSACTION,
                ),
            ),
            (
                "submit proof name without separate production proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_ALPHA,
                    PLANNED_SUBMIT_PROOF,
                ),
            ),
            (
                "dev fixture and local verifier without planned production proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_DEV_AND_LOCAL,
                    PLANNED_INSTRUCTION,
                ),
            ),
        ] {
            assert!(
                !privacy_algorithm_catalog_entries_are_valid(&[entry]),
                "{case} must be rejected",
            );
        }
    }

    #[test]
    fn privacy_algorithm_catalog_rejects_unpaired_or_generic_sdk_ledger_mutations() {
        const EMPTY: &[&str] = &[];
        const SDK_GENERIC_TRANSACTION: &[&str] = &["buildTransaction"];
        const SDK_GENERIC_SUBMIT_QUALIFIED: &[&str] = &["Iroha.Privacy.submitSignedTransaction"];
        const SDK_TYPED_INSTRUCTION: &[&str] = &["buildShapeTransferInstruction"];
        const SDK_TYPED_INSTRUCTION_WITH_PROOF: &[&str] =
            &["buildShapeProofV1", "buildShapeTransferInstruction"];
        const SDK_UNTYPED_SUBMIT_PROOF: &[&str] = &["buildSubmitShapeProof"];
        const SDK_PROOF: &[&str] = &["buildShapeProofV1"];
        const PLANNED_PROOF: &[&str] = &["buildShapeProofV1"];
        const PLANNED_GENERIC_SUBMIT: &[&str] = &["submitSignedTransaction"];
        const PLANNED_UNTYPED_SUBMIT_PROOF: &[&str] = &["buildSubmitShapeProof"];

        assert!(privacy_algorithm_catalog_entries_are_valid(&[
            privacy_catalog_entry_for_test(
                "transparent-transfer",
                "none",
                "none",
                SDK_GENERIC_TRANSACTION,
                EMPTY,
            )
        ]));
        assert!(privacy_algorithm_catalog_entries_are_valid(&[
            privacy_catalog_entry_for_test(
                "confidential-transfer-v2",
                "halo2-ipa-pasta",
                "halo2-ipa-pasta",
                SDK_TYPED_INSTRUCTION_WITH_PROOF,
                EMPTY,
            )
        ]));
        assert!(privacy_algorithm_catalog_entries_are_valid(&[
            privacy_catalog_entry_for_test(
                "confidential-transfer-v2",
                "halo2-ipa-pasta",
                "halo2-ipa-pasta",
                SDK_TYPED_INSTRUCTION,
                PLANNED_PROOF,
            )
        ]));

        for (case, entry) in [
            (
                "proofed sdk instruction without proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_TYPED_INSTRUCTION,
                    EMPTY,
                ),
            ),
            (
                "proofed sdk generic transaction",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_GENERIC_TRANSACTION,
                    PLANNED_PROOF,
                ),
            ),
            (
                "proofed qualified sdk generic submit",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_GENERIC_SUBMIT_QUALIFIED,
                    PLANNED_PROOF,
                ),
            ),
            (
                "proofed sdk untyped submit proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_UNTYPED_SUBMIT_PROOF,
                    PLANNED_PROOF,
                ),
            ),
            (
                "proofed planned generic submit",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_PROOF,
                    PLANNED_GENERIC_SUBMIT,
                ),
            ),
            (
                "proofed planned untyped submit proof",
                privacy_catalog_entry_for_test(
                    "confidential-transfer-v2",
                    "halo2-ipa-pasta",
                    "halo2-ipa-pasta",
                    SDK_PROOF,
                    PLANNED_UNTYPED_SUBMIT_PROOF,
                ),
            ),
        ] {
            assert!(
                !privacy_algorithm_catalog_entries_are_valid(&[entry]),
                "{case} must be rejected",
            );
        }
    }

    #[test]
    fn privacy_capabilities_are_norito_v1_and_fail_closed() {
        let capabilities = privacy_capabilities();
        let encoded = norito::to_bytes(&capabilities).expect("encode capabilities");
        let decoded: PrivacyCapabilitiesV1 =
            norito::decode_from_bytes(&encoded).expect("decode capabilities");

        assert!(privacy_capabilities_invariants_hold(&decoded));
        assert_eq!(decoded.version, PRIVACY_FFI_VERSION_V1);
        assert_eq!(decoded.gate_version, PRIVACY_PRODUCTION_GATE_VERSION);
        assert_eq!(
            decoded.algorithms.len(),
            PRIVACY_ALGORITHM_ENTRIES.len(),
            "all cataloged privacy algorithms must be represented",
        );
        assert!(
            decoded
                .algorithms
                .iter()
                .any(|entry| entry.algorithm_id == "orchard-halo2-actions-v1"),
        );
        let zk_ace = decoded
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == "zk-ace-pq-authorization-v0")
            .expect("ZK-ACE native capability must be advertised");
        assert_eq!(zk_ace.proof_family, "stark/fri/sha256-goldilocks");
        assert_eq!(zk_ace.backend_family, "stark-fri");
        assert!(
            !zk_ace.production_ready,
            "ZK-ACE native capability must not become production-ready only because its verifier backend is allowlisted",
        );
        assert!(!zk_ace.production_gate.ready);
        assert!(zk_ace.production_gate.audit_references.is_empty());
        assert!(zk_ace.production_gate.gates.iter().all(|gate| !gate.passed));
        assert!(
            zk_ace
                .production_gate
                .missing
                .iter()
                .any(|missing| missing == PRIVACY_PRODUCTION_GATE_MISSING_ENGINE)
        );
        assert!(
            zk_ace
                .production_gate
                .missing
                .iter()
                .any(|missing| missing == PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST)
        );
        for algorithm in decoded.algorithms {
            assert!(!algorithm.production_ready);
            assert!(!algorithm.production_gate.ready);
            assert!(algorithm.production_gate.audit_references.is_empty());
            assert!(
                algorithm
                    .production_gate
                    .missing
                    .iter()
                    .any(|missing| missing.contains("external audit")),
            );
            assert!(
                algorithm
                    .production_gate
                    .gates
                    .iter()
                    .all(|gate| !gate.passed),
            );
        }
    }

    #[test]
    fn privacy_native_archives_use_public_schema_hashes() {
        let mut capabilities_archive = Python::attach(|py| {
            let output = privacy_capabilities_v1_py(py).expect("encode privacy capabilities");
            output.bind(py).as_bytes().to_vec()
        });
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &capabilities_archive,
                PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE,
            ),
            "capabilities output must use the public privacy capabilities schema"
        );
        normalize_privacy_public_archive_for_decode::<PrivacyCapabilitiesV1>(
            &mut capabilities_archive,
        );
        let capabilities: PrivacyCapabilitiesV1 =
            norito::decode_from_bytes(&capabilities_archive).expect("decode capabilities");
        assert!(privacy_capabilities_invariants_hold(&capabilities));

        let build_request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"secret witness".to_vec(),
            proof: Vec::new(),
        };
        let build_request_archive = public_privacy_request_archive(&build_request);
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &build_request_archive,
                PRIVACY_REQUEST_SCHEMA_BYTE,
            ),
            "build request must use the public privacy request schema"
        );
        let mut build_archive = Python::attach(|py| {
            let output =
                privacy_build_proof_v1_py(py, &build_request_archive).expect("encode build result");
            output.bind(py).as_bytes().to_vec()
        });
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &build_archive,
                PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE,
            ),
            "build output must use the public privacy build-result schema"
        );
        normalize_privacy_public_archive_for_decode::<PrivacyProofResultV1>(&mut build_archive);
        let build_result: PrivacyProofResultV1 =
            norito::decode_from_bytes(&build_archive).expect("decode build result");
        assert_eq!(
            build_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED
        );

        let verify_request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: Vec::new(),
            proof: b"secret proof".to_vec(),
        };
        let verify_request_archive = public_privacy_request_archive(&verify_request);
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &verify_request_archive,
                PRIVACY_REQUEST_SCHEMA_BYTE,
            ),
            "verify request must use the public privacy request schema"
        );
        let mut verify_archive = Python::attach(|py| {
            let output = privacy_verify_proof_v1_py(py, &verify_request_archive)
                .expect("encode verify result");
            output.bind(py).as_bytes().to_vec()
        });
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &verify_archive,
                PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE,
            ),
            "verify output must use the public privacy verify-result schema"
        );
        normalize_privacy_public_archive_for_decode::<PrivacyProofResultV1>(&mut verify_archive);
        let verify_result: PrivacyProofResultV1 =
            norito::decode_from_bytes(&verify_archive).expect("decode verify result");
        assert_eq!(
            verify_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED
        );
    }

    #[test]
    fn privacy_public_schema_request_archives_reject_operation_confusion() {
        let proof_marker = b"forged-public-build-proof-shadow";
        let build_shadow = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            proof_marker.to_vec(),
        );
        let build_archive = public_privacy_request_archive(&build_shadow);
        assert!(
            privacy_archive_has_repeated_schema_byte(&build_archive, PRIVACY_REQUEST_SCHEMA_BYTE),
            "build-shadow request must use the public privacy request schema"
        );

        let build_result =
            privacy_result_for_request_archive(&build_archive, PrivacyProofOperationV1::Build);

        assert_eq!(build_result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(build_result.message.contains("build"));
        assert!(build_result.message.contains("proof"));
        assert!(build_result.message.contains("must not include"));
        assert!(build_result.proof.is_empty());
        assert!(!build_result.verified);
        assert_subslice_absent(
            build_result.message.as_bytes(),
            proof_marker,
            "public-schema build-shadow result message",
        );

        let witness_marker = b"forged-public-verify-witness-shadow";
        let mut verify_shadow = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            b"candidate-proof".to_vec(),
        );
        verify_shadow.witness = witness_marker.to_vec();
        let verify_archive = public_privacy_request_archive(&verify_shadow);
        assert!(
            privacy_archive_has_repeated_schema_byte(&verify_archive, PRIVACY_REQUEST_SCHEMA_BYTE),
            "verify-shadow request must use the public privacy request schema"
        );

        let verify_result =
            privacy_result_for_request_archive(&verify_archive, PrivacyProofOperationV1::Verify);

        assert_eq!(verify_result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(verify_result.message.contains("verify"));
        assert!(verify_result.message.contains("witness"));
        assert!(verify_result.message.contains("must not include"));
        assert_privacy_result_does_not_serialize_witness(&verify_result, witness_marker);

        let mut missing_witness = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            Vec::new(),
        );
        missing_witness.witness.clear();
        let missing_witness_archive = public_privacy_request_archive(&missing_witness);
        let missing_witness_result = privacy_result_for_request_archive(
            &missing_witness_archive,
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            missing_witness_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST
        );
        assert!(missing_witness_result.message.contains("witness"));
        assert!(missing_witness_result.message.contains("must include"));

        let mut missing_proof = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            Vec::new(),
        );
        missing_proof.witness.clear();
        let missing_proof_archive = public_privacy_request_archive(&missing_proof);
        let missing_proof_result = privacy_result_for_request_archive(
            &missing_proof_archive,
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(
            missing_proof_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST
        );
        assert!(missing_proof_result.message.contains("proof"));
        assert!(missing_proof_result.message.contains("must include"));
    }

    #[test]
    fn privacy_request_archives_reject_private_rust_schema_hashes() {
        let request = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            Vec::new(),
        );
        let private_archive = norito::to_bytes(&request).expect("encode private privacy request");
        assert!(
            !privacy_archive_has_repeated_schema_byte(
                &private_archive,
                PRIVACY_REQUEST_SCHEMA_BYTE
            ),
            "private Rust request schema must not masquerade as the public FFI request schema",
        );

        for operation in [
            PrivacyProofOperationV1::Build,
            PrivacyProofOperationV1::Verify,
        ] {
            let result = privacy_result_for_request_archive(&private_archive, operation);
            assert_malformed_privacy_request_result(&result, "private-rust-schema");
        }
    }

    #[test]
    fn privacy_capabilities_result_invariants_are_fail_closed() {
        let capabilities = privacy_capabilities();

        assert!(privacy_capabilities_invariants_hold(&capabilities));
        assert!(capabilities.algorithms.iter().all(|algorithm| {
            !algorithm.production_ready
                && privacy_production_gate_invariants_hold(&algorithm.production_gate)
        }));
    }

    #[test]
    fn privacy_capability_invariants_reject_forged_production_readiness() {
        let base = privacy_capabilities()
            .algorithms
            .into_iter()
            .next()
            .expect("privacy capabilities include algorithms");

        let mut production_ready = base.clone();
        production_ready.production_ready = true;
        assert!(
            !privacy_capability_invariants_hold(&production_ready),
            "production_ready = true must be rejected",
        );

        let mut gate_ready = base.clone();
        gate_ready.production_gate.ready = true;
        assert!(
            !privacy_capability_invariants_hold(&gate_ready),
            "production_gate.ready = true must be rejected",
        );

        let mut passed_gate = base.clone();
        passed_gate.production_gate.gates[0].passed = true;
        assert!(
            !privacy_capability_invariants_hold(&passed_gate),
            "passed production gate status must be rejected",
        );

        let mut unknown_gate = base.clone();
        unknown_gate.production_gate.gates[0].key = "shadow_gate".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&unknown_gate),
            "unknown production gate keys must be rejected",
        );

        let mut unportable_gate = base.clone();
        unportable_gate.production_gate.gates[0].key = "shadow gate".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&unportable_gate),
            "unportable production gate keys must be rejected",
        );

        let mut shuffled_gate_order = base.clone();
        shuffled_gate_order.production_gate.gates.swap(0, 1);
        assert!(
            !privacy_capability_invariants_hold(&shuffled_gate_order),
            "shuffled production gate key order must be rejected",
        );

        let mut forged_audit = base.clone();
        forged_audit
            .production_gate
            .audit_references
            .push("audit://forged".to_owned());
        assert!(
            !privacy_capability_invariants_hold(&forged_audit),
            "forged audit references must be rejected",
        );

        let mut missing_audit = base.clone();
        missing_audit
            .production_gate
            .missing
            .retain(|missing| missing != "external audit signoff is missing");
        assert!(
            !privacy_capability_invariants_hold(&missing_audit),
            "removed external-audit evidence must be rejected",
        );

        let mut missing_engine = base.clone();
        missing_engine
            .production_gate
            .missing
            .retain(|missing| missing != PRIVACY_PRODUCTION_GATE_MISSING_ENGINE);
        assert!(
            !privacy_capability_invariants_hold(&missing_engine),
            "removed production-engine evidence must be rejected",
        );

        let mut missing_allowlist = base.clone();
        missing_allowlist
            .production_gate
            .missing
            .retain(|missing| missing != PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST);
        assert!(
            !privacy_capability_invariants_hold(&missing_allowlist),
            "removed allowlist evidence must be rejected",
        );

        let mut shuffled_missing_reasons = base.clone();
        shuffled_missing_reasons.production_gate.missing.swap(0, 1);
        assert!(
            !privacy_capability_invariants_hold(&shuffled_missing_reasons),
            "shuffled production-gate missing reasons must be rejected",
        );

        let mut forged_missing_reason = base.clone();
        forged_missing_reason
            .production_gate
            .missing
            .push("external audit signoff passed without evidence".to_owned());
        assert!(
            !privacy_capability_invariants_hold(&forged_missing_reason),
            "unknown production-gate missing reasons must be rejected",
        );

        let mut duplicate_gate = base.clone();
        duplicate_gate
            .production_gate
            .gates
            .push(duplicate_gate.production_gate.gates[0].clone());
        assert!(
            !privacy_capability_invariants_hold(&duplicate_gate),
            "duplicate production gate keys must be rejected",
        );

        let mut extra_entrypoint = base.clone();
        extra_entrypoint
            .sdk_entrypoints
            .push("buildShadowProductionProof".to_owned());
        assert!(
            !privacy_capability_invariants_hold(&extra_entrypoint),
            "forged SDK entrypoints must be rejected",
        );

        let mut production_ready_proof_family = base.clone();
        production_ready_proof_family.proof_family = "halo2-production-ready".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&production_ready_proof_family),
            "production-ready proof-family labels must be rejected",
        );

        for (case, proof_family) in [
            ("uppercase proof family", "Halo2-ipa-pasta"),
            ("delimited proof family", "halo2:ipa:pasta"),
            ("empty proof-family segment", "halo2//ipa"),
            ("leading slash proof family", "/halo2"),
            ("leading hyphen proof family", "-halo2"),
            ("trailing slash proof family", "halo2/"),
            ("trailing hyphen proof family", "halo2-"),
        ] {
            let mut forged_proof_family = base.clone();
            forged_proof_family.proof_family = proof_family.to_owned();
            assert!(
                !privacy_capability_invariants_hold(&forged_proof_family),
                "{case} must be rejected"
            );
        }

        let mut audit_signoff_backend = base.clone();
        audit_signoff_backend.backend_family = "audit-signoff-pasta".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&audit_signoff_backend),
            "audit-signoff backend labels must be rejected",
        );

        let mut mainnet_ready_entrypoint = base.clone();
        mainnet_ready_entrypoint
            .sdk_entrypoints
            .push("buildMainnetReadyProof".to_owned());
        assert!(
            !privacy_capability_invariants_hold(&mainnet_ready_entrypoint),
            "mainnet-ready executable entrypoints must be rejected",
        );

        let mut claimed_mainnet_algorithm = base.clone();
        claimed_mainnet_algorithm.algorithm_id = "claimed-mainnet-row".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&claimed_mainnet_algorithm),
            "claimed-mainnet algorithm labels must be rejected",
        );

        let mut audit_claim_planned_entrypoint = base.clone();
        audit_claim_planned_entrypoint
            .planned_entrypoints
            .push("buildClaimedAuditProof".to_owned());
        assert!(
            !privacy_capability_invariants_hold(&audit_claim_planned_entrypoint),
            "claimed-audit planned entrypoints must be rejected",
        );
    }

    #[test]
    fn privacy_capabilities_invariants_reject_bad_versions_and_duplicate_rows() {
        let base = privacy_capabilities();

        let mut bad_version = base.clone();
        bad_version.version = PRIVACY_FFI_VERSION_V1 + 1;
        assert!(
            !privacy_capabilities_invariants_hold(&bad_version),
            "bad capabilities version must be rejected",
        );

        let mut bad_gate_version = base.clone();
        bad_gate_version.gate_version = "privacy-production-gate-v2".to_owned();
        assert!(
            !privacy_capabilities_invariants_hold(&bad_gate_version),
            "bad production gate version must be rejected",
        );

        let mut shuffled_row_order = base.clone();
        shuffled_row_order.algorithms.swap(0, 1);
        assert!(
            !privacy_capabilities_invariants_hold(&shuffled_row_order),
            "shuffled algorithm capability rows must be rejected",
        );

        let mut duplicate_row = base.clone();
        let duplicate = duplicate_row.algorithms[0].clone();
        duplicate_row.algorithms.push(duplicate);
        assert!(
            !privacy_capabilities_invariants_hold(&duplicate_row),
            "duplicate algorithm capability rows must be rejected",
        );
    }

    #[test]
    fn privacy_request_archive_size_boundaries_are_fail_closed() {
        assert!(privacy_request_archive_out_of_bounds(0));
        assert!(!privacy_request_archive_out_of_bounds(1));
        assert!(!privacy_request_archive_out_of_bounds(
            PRIVACY_NATIVE_ARCHIVE_MAX_BYTES
        ));
        assert!(privacy_request_archive_out_of_bounds(
            PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1
        ));
    }

    #[test]
    fn privacy_request_rejects_oversized_text_fields_without_reflection() {
        let oversized = "x".repeat(PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES + 1);
        for field in ["algorithm_id", "entrypoint", "vk_ref"] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            match field {
                "algorithm_id" => request.algorithm_id = oversized.clone(),
                "entrypoint" => request.entrypoint = oversized.clone(),
                "vk_ref" => request.vk_ref = oversized.clone(),
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(&result, "maximum length", field);
            assert!(
                !norito::to_bytes(&result)
                    .expect("encode privacy result")
                    .windows(oversized.len())
                    .any(|window| window == oversized.as_bytes()),
                "{field} was reflected in the encoded privacy result",
            );
        }
    }

    #[test]
    fn privacy_request_rejects_control_text_fields_without_reflection() {
        for (field, value) in [
            ("algorithm_id", "confidential-transfer-v2\nforged"),
            ("entrypoint", "buildConfidentialTransferProofV2\rforged"),
            ("vk_ref", "vk:test\tforged"),
        ] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            match field {
                "algorithm_id" => request.algorithm_id = value.to_owned(),
                "entrypoint" => request.entrypoint = value.to_owned(),
                "vk_ref" => request.vk_ref = value.to_owned(),
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(&result, "control characters", field);
        }
    }

    #[test]
    fn privacy_request_rejects_non_ascii_text_fields_without_reflection() {
        let marker = "unicode-text-never-echo";
        for (field, value) in [
            (
                "algorithm_id",
                format!("confidential-transfer-v2{marker}\u{200B}"),
            ),
            (
                "entrypoint",
                format!("buildConfidentialTransferProofV2{marker}\u{2060}"),
            ),
            ("vk_ref", format!("vk:test{marker}\u{FF1A}spoof")),
        ] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            match field {
                "algorithm_id" => request.algorithm_id = value,
                "entrypoint" => request.entrypoint = value,
                "vk_ref" => request.vk_ref = value,
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(&result, "printable ASCII", field);
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert!(
                !encoded
                    .windows(marker.len())
                    .any(|window| window == marker.as_bytes()),
                "{field} was reflected in the encoded privacy result",
            );
        }
    }

    #[test]
    fn privacy_request_rejects_unportable_text_fields_without_reflection() {
        let marker = "punctuation-text-never-echo";
        for (field, value) in [
            ("algorithm_id", format!("confidential-transfer-v2 {marker}")),
            (
                "entrypoint",
                format!("buildConfidentialTransferProofV2\"{marker}\""),
            ),
            ("vk_ref", format!("vk:test/../{marker}")),
        ] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            match field {
                "algorithm_id" => request.algorithm_id = value,
                "entrypoint" => request.entrypoint = value,
                "vk_ref" => request.vk_ref = value,
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(
                &result,
                "portable identifier",
                field,
            );
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert!(
                !encoded
                    .windows(marker.len())
                    .any(|window| window == marker.as_bytes()),
                "{field} was reflected in the encoded privacy result",
            );
        }
    }

    #[test]
    fn privacy_request_rejects_invalid_catalog_shapes_without_reflection() {
        let marker = "catalog-shape-text-never-echo";
        for (field, value) in [
            ("algorithm_id", format!("confidential-transfer-v2:{marker}")),
            ("algorithm_id", format!("Confidential-transfer-v2{marker}")),
            ("algorithm_id", format!("confidential.transfer.v2{marker}")),
            ("algorithm_id", format!("_confidential-transfer-v2{marker}")),
            ("algorithm_id", format!("-confidential-transfer-v2{marker}")),
            (
                "algorithm_id",
                format!("confidential-transfer-v2-{marker}_"),
            ),
            (
                "algorithm_id",
                format!("confidential-transfer-v2-{marker}-"),
            ),
            (
                "entrypoint",
                format!("buildConfidentialTransferProofV2:{marker}"),
            ),
            (
                "entrypoint",
                format!("build-ConfidentialTransferProofV2{marker}"),
            ),
            (
                "entrypoint",
                format!("_buildConfidentialTransferProofV2{marker}"),
            ),
            (
                "entrypoint",
                format!("buildConfidentialTransferProofV2_{marker}"),
            ),
            (
                "entrypoint",
                format!("Iroha._Privacy.buildConfidentialTransferProofV2{marker}"),
            ),
            (
                "entrypoint",
                format!("Iroha.Privacy_.buildConfidentialTransferProofV2{marker}"),
            ),
        ] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            match field {
                "algorithm_id" => request.algorithm_id = value,
                "entrypoint" => request.entrypoint = value,
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(
                &result,
                "catalog identifier shapes",
                field,
            );
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(&encoded, marker.as_bytes(), "catalog-shape failure result");
        }
    }

    #[test]
    fn privacy_request_rejects_empty_required_text_fields_without_reflection() {
        let marker = b"required-text-field-never-echo";
        for field in ["algorithm_id", "entrypoint", "vk_ref"] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            request.public_inputs = marker.to_vec();
            match field {
                "algorithm_id" => request.algorithm_id.clear(),
                "entrypoint" => request.entrypoint.clear(),
                "vk_ref" => request.vk_ref.clear(),
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);
            let message_fragment = match field {
                "vk_ref" => "non-empty vk_ref",
                _ => "non-empty algorithm_id and entrypoint",
            };

            assert_unreflected_invalid_privacy_request_result(&result, message_fragment, field);
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(&encoded, marker, "empty required field failure result");
        }
    }

    #[test]
    fn privacy_request_rejects_exposed_production_claims_without_reflection() {
        for (field, value) in [
            ("algorithm_id", "forged-mainnet-ready-algorithm"),
            ("algorithm_id", "claimed-mainnet-algorithm"),
            ("entrypoint", "buildAuditSignoffProof"),
            ("entrypoint", "buildClaimedAuditProof"),
            ("entrypoint", "buildS.e.c.u.r.i.t.yReviewPassedProof"),
            (
                "vk_ref",
                "halo2-ipa-pasta:externally-audited-confidential-transfer",
            ),
            (
                "vk_ref",
                "halo2-ipa-pasta:audit-claim-confidential-transfer",
            ),
        ] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            match field {
                "algorithm_id" => request.algorithm_id = value.to_owned(),
                "entrypoint" => request.entrypoint = value.to_owned(),
                "vk_ref" => request.vk_ref = value.to_owned(),
                _ => unreachable!("unexpected privacy request field"),
            }

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(
                &result,
                "production/mainnet/audit readiness",
                field,
            );
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert!(
                !encoded
                    .windows(value.len())
                    .any(|window| window == value.as_bytes()),
                "{field} was reflected in the encoded privacy result",
            );
        }
    }

    #[test]
    fn privacy_request_rejects_oversized_public_inputs_without_reflection() {
        let mut request = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            Vec::new(),
        );
        request.public_inputs = vec![0xA5; PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES + 1];

        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_unreflected_invalid_privacy_request_result(&result, "public_inputs", "public");
    }

    #[test]
    fn privacy_request_rejects_oversized_witness_without_reflection() {
        let marker = b"oversized-witness-never-echo";
        let mut oversized = vec![0xA5; PRIVACY_REQUEST_WITNESS_MAX_BYTES + 1];
        oversized[..marker.len()].copy_from_slice(marker);
        let mut request = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            Vec::new(),
        );
        request.witness = oversized;

        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_unreflected_invalid_privacy_request_result(&result, "witness", "witness");
        let encoded = norito::to_bytes(&result).expect("encode privacy result");
        assert!(
            !encoded.windows(marker.len()).any(|window| window == marker),
            "oversized witness marker was reflected in the encoded privacy result",
        );
    }

    #[test]
    fn privacy_request_rejects_oversized_proof_without_reflection() {
        let marker = b"oversized-proof-never-echo";
        let mut oversized = vec![0xA7; PRIVACY_REQUEST_PROOF_MAX_BYTES + 1];
        oversized[..marker.len()].copy_from_slice(marker);
        let mut request = privacy_request(
            "confidential-transfer-v2",
            "buildConfidentialTransferProofV2",
            oversized,
        );
        request.witness.clear();

        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Verify);

        assert_unreflected_invalid_privacy_request_result(&result, "proof", "proof");
        let encoded = norito::to_bytes(&result).expect("encode privacy result");
        assert!(
            !encoded.windows(marker.len()).any(|window| window == marker),
            "oversized proof marker was reflected in the encoded privacy result",
        );
    }

    #[test]
    fn privacy_native_availability_probe_rejects_with_malformed_error() {
        for operation in [
            PrivacyProofOperationV1::Build,
            PrivacyProofOperationV1::Verify,
        ] {
            let result = privacy_result_for_request_archive(
                PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE,
                operation,
            );

            assert_eq!(result.version, PRIVACY_FFI_VERSION_V1);
            assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR);
            assert_eq!(result.error_code, PRIVACY_FFI_ERROR_MALFORMED_NORITO);
            assert_eq!(result.message, "malformed Norito V1 privacy proof request");
            assert!(result.algorithm_id.is_empty());
            assert!(result.entrypoint.is_empty());
            assert!(result.vk_ref.is_empty());
            assert!(result.public_inputs.is_empty());
            assert!(result.proof.is_empty());
            assert!(!result.verified);
        }
    }

    #[test]
    fn privacy_build_proof_rejects_malformed_norito() {
        let result =
            privacy_result_for_request_archive(b"not norito", PrivacyProofOperationV1::Build);

        assert_eq!(result.version, PRIVACY_FFI_VERSION_V1);
        assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_MALFORMED_NORITO);
        assert!(!result.verified);
        assert!(result.proof.is_empty());
        assert!(result.public_inputs.is_empty());
    }

    #[test]
    fn privacy_proof_entrypoints_reject_adversarial_norito_frames() {
        for (case, malformed) in adversarial_privacy_request_archives() {
            for operation in [
                PrivacyProofOperationV1::Build,
                PrivacyProofOperationV1::Verify,
            ] {
                let result = privacy_result_for_request_archive(&malformed, operation);
                assert_malformed_privacy_request_result(&result, case);
            }
        }
    }

    #[test]
    fn privacy_build_proof_rejects_missing_request_fields() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: String::new(),
            entrypoint: String::new(),
            vk_ref: String::new(),
            public_inputs: b"public".to_vec(),
            witness: b"secret".to_vec(),
            proof: b"proof".to_vec(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.public_inputs, b"public");
        assert!(result.proof.is_empty());
        assert!(!result.message.contains("secret"));
    }

    #[test]
    fn privacy_build_proof_rejects_unknown_algorithm() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "adversarial-shadow-row".to_owned(),
            entrypoint: "buildAdversarialShadowProof".to_owned(),
            vk_ref: "vk:test".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"secret".to_vec(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM);
        assert_eq!(result.algorithm_id, "adversarial-shadow-row");
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_failure_results_never_serialize_witness_material() {
        let witness = b"python-host-witness-never-echo-5a7c91";

        let unsupported = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "adversarial-shadow-row".to_owned(),
                entrypoint: "buildAdversarialShadowProof".to_owned(),
                vk_ref: "vk:test".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            unsupported.error_code,
            PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
        );
        assert_privacy_result_does_not_serialize_witness(&unsupported, witness);

        let bad_entrypoint = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "orchard-halo2-actions-v1".to_owned(),
                entrypoint: "buildAdversarialProof".to_owned(),
                vk_ref: "halo2-ipa-orchard:vk_orchard_actions_v1".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(bad_entrypoint.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,);
        assert_privacy_result_does_not_serialize_witness(&bad_entrypoint, witness);

        let missing_vk = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: String::new(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(missing_vk.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_privacy_result_does_not_serialize_witness(&missing_vk, witness);

        let wrong_vk_backend = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "groth16-bls12-377:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            wrong_vk_backend.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&wrong_vk_backend, witness);

        let wrong_vk_name = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:vk_test".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(wrong_vk_name.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,);
        assert_privacy_result_does_not_serialize_witness(&wrong_vk_name, witness);

        let empty_public_inputs = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: Vec::new(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            empty_public_inputs.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&empty_public_inputs, witness);

        let disabled_build = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            disabled_build.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
        );
        assert_privacy_result_does_not_serialize_witness(&disabled_build, witness);

        let disabled_verify = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: Vec::new(),
                proof: b"candidate proof".to_vec(),
            },
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(
            disabled_verify.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
        );
        assert_privacy_result_does_not_serialize_witness(&disabled_verify, witness);

        let witness_shadow_verify = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: b"candidate proof".to_vec(),
            },
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(
            witness_shadow_verify.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&witness_shadow_verify, witness);
    }

    #[test]
    fn privacy_failure_results_preserve_error_invariants_without_proof_reflection() {
        let proof_marker = b"python-host-proof-never-echo-c61e";

        let build_shadow_result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: proof_marker.to_vec(),
            },
            PrivacyProofOperationV1::Build,
        );

        let disabled_verify_result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: Vec::new(),
                proof: proof_marker.to_vec(),
            },
            PrivacyProofOperationV1::Verify,
        );

        for (case, result) in [
            ("build-proof-shadow", build_shadow_result),
            ("disabled-verify-proof", disabled_verify_result),
        ] {
            assert!(
                privacy_failure_result_invariants_hold(&result),
                "{case}: {result:?}",
            );
            assert!(
                !result
                    .message
                    .as_bytes()
                    .windows(proof_marker.len())
                    .any(|window| window == proof_marker),
                "{case} reflected proof bytes in the privacy result message",
            );
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert!(
                !encoded
                    .windows(proof_marker.len())
                    .any(|window| window == proof_marker),
                "{case} reflected proof bytes in the encoded privacy result",
            );
        }
    }

    #[test]
    fn privacy_build_proof_rejects_unknown_entrypoint_for_known_algorithm() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "orchard-halo2-actions-v1".to_owned(),
            entrypoint: "buildAdversarialProof".to_owned(),
            vk_ref: "halo2-ipa-orchard:vk_orchard_actions_v1".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"secret".to_vec(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "orchard-halo2-actions-v1");
        assert_eq!(result.entrypoint, "buildAdversarialProof");
        assert!(result.message.contains("entrypoint"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_planned_entrypoint_before_request_validation() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "orchard-halo2-actions-v1".to_owned(),
            entrypoint: "buildOrchardActionBundleProofV1".to_owned(),
            vk_ref: String::new(),
            public_inputs: b"public".to_vec(),
            witness: b"planned-entrypoint-witness-must-not-echo".to_vec(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "orchard-halo2-actions-v1");
        assert_eq!(result.entrypoint, "buildOrchardActionBundleProofV1");
        assert!(result.message.contains("planned"));
        assert!(result.message.contains("not executable"));
        assert!(!result.message.contains("vk_ref"));
        assert!(!result.message.contains("witness"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_empty_vk_ref() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: String::new(),
            public_inputs: b"public".to_vec(),
            witness: b"secret".to_vec(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.message.contains("vk_ref"));
        assert!(result.proof.is_empty());
        assert!(!result.message.contains("secret"));
    }

    #[test]
    fn privacy_proof_ffi_rejects_malformed_vk_ref_without_reflection() {
        let marker = "vkrefshapeneverecho";
        for (case, vk_ref) in [
            (
                "missing-separator",
                format!("halo2-ipa-pasta-confidential_transfer_v2-{marker}"),
            ),
            ("empty-vk-name", "halo2-ipa-pasta:".to_owned()),
            (
                "extra-separator",
                format!("halo2-ipa-pasta:confidential_transfer_v2:{marker}"),
            ),
            (
                "delimited-backend",
                format!("halo2:ipa:confidential_transfer_v2_{marker}"),
            ),
            (
                "uppercase-backend",
                format!("Halo2-ipa-pasta:confidential_transfer_v2_{marker}"),
            ),
            (
                "leading-separator-backend",
                format!("-halo2-ipa-pasta:confidential_transfer_v2_{marker}"),
            ),
            (
                "trailing-separator-backend",
                format!("halo2-ipa-pasta.:confidential_transfer_v2_{marker}"),
            ),
            (
                "dotted-backend-alias",
                format!("halo2.ipa.pasta-{marker}:confidential_transfer_v2"),
            ),
            (
                "underscored-backend-alias",
                format!("halo2_ipa_pasta_{marker}:confidential_transfer_v2"),
            ),
            (
                "repeated-backend-separator",
                format!("halo2--ipa-pasta-{marker}:confidential_transfer_v2"),
            ),
            (
                "uppercase-vk-name",
                format!("halo2-ipa-pasta:Confidential_transfer_v2_{marker}"),
            ),
            (
                "dotted-vk-name",
                format!("halo2-ipa-pasta:confidential.transfer.v2_{marker}"),
            ),
            (
                "dashed-vk-name",
                format!("halo2-ipa-pasta:confidential-transfer-v2-{marker}"),
            ),
            (
                "leading-underscore-vk-name",
                format!("halo2-ipa-pasta:_confidential_transfer_v2_{marker}"),
            ),
            (
                "trailing-underscore-vk-name",
                format!("halo2-ipa-pasta:confidential_transfer_v2_{marker}_"),
            ),
            (
                "repeated-underscore-vk-name",
                format!("halo2-ipa-pasta:confidential_transfer__v2_{marker}"),
            ),
        ] {
            let request = PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref,
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: Vec::new(),
            };
            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(&result, "backend:name", case);
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(
                &encoded,
                marker.as_bytes(),
                "malformed vk_ref failure result",
            );
        }
    }

    #[test]
    fn privacy_proof_ffi_rejects_malformed_vk_ref_before_catalog_binding_without_reflection() {
        let marker = "vk-ref-order-never-echo";
        for (case, algorithm_id, entrypoint, vk_ref) in [
            (
                "unsupported-algorithm",
                "future-privacy-row",
                "buildFuturePrivacyProof",
                format!("halo2-ipa-pasta:Bad_vk_name_{marker}"),
            ),
            (
                "planned-entrypoint",
                "asset-hidden-confidential-transfer-v1",
                "buildConfidentialAssetHiddenTransferProofV1",
                format!("halo2-ipa-pasta:bad.vk.name_{marker}"),
            ),
            (
                "unregistered-entrypoint",
                "confidential-transfer-v2",
                "buildUnregisteredPrivacyProof",
                format!("halo2-ipa-pasta:bad-vk-name-{marker}"),
            ),
            (
                "non-proof-entrypoint",
                "verange-transparent-range-v1",
                "buildRangeCommitment",
                format!("verange:_bad_vk_name_{marker}"),
            ),
        ] {
            let mut request = privacy_request(algorithm_id, entrypoint, Vec::new());
            request.vk_ref = vk_ref;

            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_unreflected_invalid_privacy_request_result(&result, "backend:name", case);
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(
                &encoded,
                marker.as_bytes(),
                "vk_ref validation-order failure result",
            );
        }
    }

    #[test]
    fn privacy_proof_ffi_rejects_wrong_backend_vk_ref_before_production_gate() {
        let (case, vk_ref) = (
            "wrong-backend",
            "groth16-bls12-377:confidential_transfer_v2",
        );
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: vk_ref.to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"secret witness".to_vec(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(
            result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "{case}",
        );
        assert!(
            result.message.contains("vk_ref backend"),
            "{case}: {}",
            result.message,
        );
        assert!(result.message.contains("backend family"), "{case}");
        assert_eq!(result.algorithm_id, "confidential-transfer-v2", "{case}");
        assert_eq!(result.vk_ref, vk_ref, "{case}");
        assert_eq!(result.public_inputs, b"public", "{case}");
        assert!(result.proof.is_empty(), "{case}");
        assert!(!result.verified, "{case}");
    }

    #[test]
    fn privacy_proof_ffi_rejects_wrong_vk_ref_name_before_production_gate() {
        for (case, vk_ref) in [
            ("generic-vk-name", "halo2-ipa-pasta:vk_test"),
            (
                "foreign-algorithm-vk-name",
                "halo2-ipa-pasta:confidential_unshield_v3",
            ),
            (
                "legacy-vk-prefix",
                "halo2-ipa-pasta:vk_confidential_transfer_v2",
            ),
        ] {
            let request = PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: vk_ref.to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: Vec::new(),
            };
            let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

            assert_eq!(
                result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "{case}",
            );
            assert!(
                result.message.contains("vk_ref name"),
                "{case}: {}",
                result.message,
            );
            assert!(result.message.contains("algorithm verifier key"), "{case}");
            assert_eq!(result.algorithm_id, "confidential-transfer-v2", "{case}");
            assert_eq!(result.vk_ref, vk_ref, "{case}");
            assert_eq!(result.public_inputs, b"public", "{case}");
            assert!(result.proof.is_empty(), "{case}");
            assert!(!result.verified, "{case}");
        }
    }

    #[test]
    fn privacy_build_proof_rejects_empty_public_inputs_before_production_gate() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: Vec::new(),
            witness: b"secret witness".to_vec(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert_eq!(result.entrypoint, "buildConfidentialTransferProofV2");
        assert!(result.message.contains("public_inputs"));
        assert!(result.message.contains("non-empty"));
        assert!(result.public_inputs.is_empty());
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_verify_proof_rejects_empty_public_inputs_before_production_gate() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: Vec::new(),
            witness: Vec::new(),
            proof: b"proof bytes".to_vec(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Verify);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert_eq!(result.entrypoint, "buildConfidentialTransferProofV2");
        assert!(result.message.contains("public_inputs"));
        assert!(result.message.contains("non-empty"));
        assert!(result.public_inputs.is_empty());
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_missing_witness_before_production_gate() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: Vec::new(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.message.contains("witness"));
        assert!(result.message.contains("build"));
        assert!(result.message.contains("must include"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_proof_shadow_before_production_gate() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"secret witness".to_vec(),
            proof: b"forged-build-proof-shadow".to_vec(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.message.contains("build"));
        assert!(result.message.contains("proof"));
        assert!(result.message.contains("must not include"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_proof_ffi_rejects_non_proof_sdk_entrypoints_before_production_gate() {
        for (case, operation, mut request) in [
            (
                "build-commitment-helper",
                PrivacyProofOperationV1::Build,
                privacy_request(
                    "verange-transparent-range-v1",
                    "buildRangeCommitment",
                    Vec::new(),
                ),
            ),
            (
                "build-dev-fixture-helper",
                PrivacyProofOperationV1::Build,
                privacy_request(
                    "verange-transparent-range-v1",
                    "buildVeRangeDevProofFixture",
                    Vec::new(),
                ),
            ),
            (
                "verify-local-helper",
                PrivacyProofOperationV1::Verify,
                privacy_request(
                    "verange-transparent-range-v1",
                    "verifyVeRangeProofLocally",
                    b"candidate-proof".to_vec(),
                ),
            ),
            (
                "build-instruction-helper",
                PrivacyProofOperationV1::Build,
                privacy_request(
                    "confidential-transfer-v2",
                    "buildZkTransferInstruction",
                    Vec::new(),
                ),
            ),
        ] {
            if operation == PrivacyProofOperationV1::Verify {
                request.witness.clear();
            }
            let result = privacy_result_for_request(request, operation);

            assert_eq!(
                result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "{case}",
            );
            assert!(
                result.message.contains("production proof builder"),
                "{case}: {}",
                result.message,
            );
            assert_eq!(result.public_inputs, b"public-inputs", "{case}");
            assert!(result.proof.is_empty(), "{case}");
            assert!(!result.verified, "{case}");
        }
    }

    #[test]
    fn privacy_verify_proof_rejects_missing_proof_before_production_gate() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: Vec::new(),
            proof: Vec::new(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Verify);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.message.contains("proof"));
        assert!(result.message.contains("verify"));
        assert!(result.message.contains("must include"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_verify_proof_rejects_witness_shadow_before_production_gate() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"forged-verify-witness-shadow".to_vec(),
            proof: b"candidate proof".to_vec(),
        };
        let result = privacy_result_for_request(request, PrivacyProofOperationV1::Verify);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.message.contains("verify"));
        assert!(result.message.contains("witness"));
        assert!(result.message.contains("must not include"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_supported_algorithm_until_gate_passes() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: b"secret witness".to_vec(),
            proof: Vec::new(),
        };
        let archive = public_privacy_request_archive(&request);
        let result = privacy_result_for_request_archive(&archive, PrivacyProofOperationV1::Build);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_PRODUCTION_DISABLED);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert_eq!(result.entrypoint, "buildConfidentialTransferProofV2");
        assert_eq!(result.vk_ref, "halo2-ipa-pasta:confidential_transfer_v2");
        assert_eq!(result.public_inputs, b"public");
        for fragment in [
            "exact protocol implementation",
            "real proving",
            "real verification",
            "chain admission",
            "cross-SDK parity",
            "wallet/state support",
            "deterministic tests",
            "fuzzing",
            "performance gates",
            "external audit",
            "real protocol engine",
            "Iroha production allowlist",
        ] {
            assert!(
                result.message.contains(fragment),
                "production-disabled message missing {fragment}: {}",
                result.message
            );
        }
        assert!(result.proof.is_empty());
        assert!(!result.verified);
        assert!(!result.message.contains("secret"));

        let zk_ace_request = privacy_request(
            "zk-ace-pq-authorization-v0",
            "buildZkAceAuthorizationProofV1",
            Vec::new(),
        );
        let zk_ace_archive = public_privacy_request_archive(&zk_ace_request);
        let zk_ace_result =
            privacy_result_for_request_archive(&zk_ace_archive, PrivacyProofOperationV1::Build);

        assert_eq!(
            zk_ace_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED
        );
        assert_eq!(zk_ace_result.algorithm_id, "zk-ace-pq-authorization-v0");
        assert_eq!(zk_ace_result.entrypoint, "buildZkAceAuthorizationProofV1");
        assert_eq!(zk_ace_result.vk_ref, "stark-fri:zk_ace_pq_authorization_v0");
        assert_eq!(zk_ace_result.public_inputs, b"public-inputs");
        assert!(zk_ace_result.message.contains("Iroha production allowlist"));
        assert!(zk_ace_result.proof.is_empty());
        assert!(!zk_ace_result.verified);
        assert!(!zk_ace_result.message.contains("secret-witness"));
    }

    #[test]
    fn privacy_verify_proof_rejects_supported_algorithm_until_gate_passes() {
        let request = PrivacyProofRequestV1 {
            algorithm_id: "confidential-transfer-v2".to_owned(),
            entrypoint: "buildConfidentialTransferProofV2".to_owned(),
            vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
            public_inputs: b"public".to_vec(),
            witness: Vec::new(),
            proof: b"candidate proof".to_vec(),
        };
        let archive = public_privacy_request_archive(&request);
        let result = privacy_result_for_request_archive(&archive, PrivacyProofOperationV1::Verify);

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_PRODUCTION_DISABLED);
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert_eq!(result.entrypoint, "buildConfidentialTransferProofV2");
        assert_eq!(result.public_inputs, b"public");
        for fragment in [
            "exact protocol implementation",
            "real proving",
            "real verification",
            "chain admission",
            "cross-SDK parity",
            "wallet/state support",
            "deterministic tests",
            "fuzzing",
            "performance gates",
            "external audit",
            "real protocol engine",
            "Iroha production allowlist",
        ] {
            assert!(
                result.message.contains(fragment),
                "production-disabled message missing {fragment}: {}",
                result.message
            );
        }
        assert!(result.proof.is_empty());
        assert!(!result.verified);
        assert!(!result.message.contains("secret"));

        let mut zk_ace_request = privacy_request(
            "zk-ace-pq-authorization-v0",
            "buildZkAceAuthorizationProofV1",
            b"candidate-zk-ace-proof".to_vec(),
        );
        zk_ace_request.witness.clear();
        let zk_ace_archive = public_privacy_request_archive(&zk_ace_request);
        let zk_ace_result =
            privacy_result_for_request_archive(&zk_ace_archive, PrivacyProofOperationV1::Verify);

        assert_eq!(
            zk_ace_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED
        );
        assert_eq!(zk_ace_result.algorithm_id, "zk-ace-pq-authorization-v0");
        assert_eq!(zk_ace_result.entrypoint, "buildZkAceAuthorizationProofV1");
        assert_eq!(zk_ace_result.vk_ref, "stark-fri:zk_ace_pq_authorization_v0");
        assert_eq!(zk_ace_result.public_inputs, b"public-inputs");
        assert!(zk_ace_result.message.contains("Iroha production allowlist"));
        assert!(zk_ace_result.proof.is_empty());
        assert!(!zk_ace_result.verified);
        assert!(!zk_ace_result.message.contains("candidate-zk-ace-proof"));
    }

    fn sample_kagemusha_recursive_spend_bundle() -> KagemushaRecursiveSpendBundleV1 {
        let chain_id: ChainId = "kagemusha-recursive-spend-python"
            .parse()
            .expect("chain id");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "kgmpy".parse().expect("asset definition name"),
        );
        let current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_bytes(b"python-recursive-current-note"),
            spend_nullifier: fixed_bytes(b"python-recursive-current-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let mut topup_anchor_nullifiers = vec![
            fixed_bytes(b"python-recursive-topup-anchor-0"),
            fixed_bytes(b"python-recursive-topup-anchor-1"),
        ];
        topup_anchor_nullifiers.sort_unstable();
        let accumulator = KagemushaRecursiveSpendAccumulatorV1 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN.to_owned(),
            chain_id,
            asset,
            initial_root: fixed_bytes(b"python-recursive-initial-root"),
            final_root: fixed_bytes(b"python-recursive-final-root"),
            topup_anchor_nullifiers,
            hop_count: 2,
            lineage_digest: fixed_bytes(b"python-recursive-lineage"),
            aggregation_transcript_digest: fixed_bytes(b"python-recursive-lineage"),
            nullifier_digest: Hash::new(b"python-recursive-nullifier-digest"),
            output_commitment_digest: Hash::new(b"python-recursive-output-digest"),
            fold_digest: Hash::new(b"python-recursive-fold-digest"),
            recursive_proof_chain_digest: fixed_bytes(b"python-recursive-proof-chain"),
            transition_profile_binding_digest: fixed_bytes(b"python-recursive-transition-binding"),
            append_opening_preflight_digest: [0u8; 32],
            append_boundary_digest: [0u8; 32],
            verifier_params_fingerprint: fixed_bytes(b"python-recursive-params"),
            fixed_window_table_schedule_digest: fixed_bytes(b"python-recursive-schedule"),
            fixed_window_shared_table_manifest_digest: fixed_bytes(b"python-recursive-manifest"),
            fixed_window_table_base_digest: fixed_bytes(b"python-recursive-table-base"),
            verifier_witness_batch_digest: fixed_bytes(b"python-recursive-witness-batch"),
            verifier_opening_len: 4,
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

    fn attach_recursive_spend_previous_proof_open_verify_envelope(
        bundle: &mut KagemushaRecursiveSpendBundleV1,
        vk_hash: [u8; Hash::LENGTH],
    ) {
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: bundle.recursive_proof.verifier_key_id.name.clone(),
            vk_hash,
            public_inputs:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
                    .to_vec(),
            proof_bytes: vec![0xA5; 64],
            aux: Vec::new(),
        };
        bundle.recursive_proof.proof = ProofBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            norito::to_bytes(&envelope).expect("encode previous recursive proof envelope"),
        );
    }

    fn sample_reserved_lineage_previous_bundle() -> KagemushaRecursiveSpendBundleV1 {
        let mut previous_bundle = sample_kagemusha_recursive_spend_bundle();
        previous_bundle.recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        previous_bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            recursive_spend_lineage_scalar_projection(0x6C);
        previous_bundle.recursive_proof.public_inputs_hash = previous_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("Python reserved lineage previous public-input hash");
        attach_recursive_spend_previous_proof_open_verify_envelope(
            &mut previous_bundle,
            fixed_bytes(b"python-recursive-spend-append-previous-proof-vk"),
        );
        previous_bundle
    }

    fn sample_reserved_lineage_append_request_missing_key_artifacts()
    -> KagemushaRecursiveSpendAppendRequestV1 {
        let init_request = sample_recursive_spend_init_request();
        let mut previous_bundle = sample_reserved_lineage_previous_bundle();
        let step = init_request
            .record_bundle
            .bundle
            .steps
            .first()
            .expect("Python append record bundle has one hop");
        let root_before = step.root_before;
        let previous_note_nullifier = step.input_nullifiers[0];
        let output_commitment = step.output_commitments[0];
        previous_bundle.accumulator.chain_id = init_request.record_bundle.bundle.chain_id.clone();
        previous_bundle.accumulator.asset = init_request.record_bundle.bundle.asset.clone();
        if previous_bundle.accumulator.initial_root == root_before {
            previous_bundle.accumulator.initial_root =
                fixed_bytes(b"python-lineage-append-distinct-initial-root");
        }
        previous_bundle.accumulator.topup_anchor_nullifiers =
            vec![fixed_bytes(b"python-lineage-append-distinct-topup-anchor")];
        previous_bundle.accumulator.final_root = root_before;
        previous_bundle.accumulator.current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_bytes(b"python-lineage-append-previous-note"),
            spend_nullifier: previous_note_nullifier,
            amount: Numeric::new(42, 0),
        };
        refresh_recursive_spend_bundle_public_inputs(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope(
            &mut previous_bundle,
            fixed_bytes(b"python-lineage-append-missing-artifact-previous-vk"),
        );
        let previous_recursive_proof_open_envelopes_archive =
            sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle);

        let request = KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            previous_lineage_verifier_record: Some(sample_recursive_spend_lineage_verifier_record()),
            record_bundle: init_request.record_bundle,
            pallas_open_envelopes_archive: init_request.pallas_open_envelopes_archive,
            current_note: KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: fixed_bytes(b"python-lineage-append-current-nullifier"),
                amount: Numeric::new(42, 0),
            },
            output_proof_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                .to_owned(),
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            previous_recursive_proof_open_envelopes_archive,
            block_height: None,
        };
        request
            .validate_public_binding()
            .expect("Python missing-key-artifact append request is otherwise well formed");
        request
    }

    fn sample_recursive_spend_transition_profile_init_request()
    -> KagemushaRecursiveSpendInitRequestV1 {
        let (_, witness) = sample_verifying_semantic_recursive_spend_lineage_fixture();
        KagemushaRecursiveSpendInitRequestV1 {
            record_bundle: witness.record_bundle.clone(),
            pallas_open_envelopes_archive: witness.pallas_open_envelopes_archive.clone(),
            current_note: witness
                .current_notes
                .first()
                .expect("semantic lineage witness has a current note")
                .clone(),
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        }
    }

    fn sample_recursive_spend_transition_profile_append_request()
    -> KagemushaRecursiveSpendAppendRequestV1 {
        let (mut previous_bundle, witness) =
            sample_verifying_semantic_recursive_spend_lineage_fixture();
        let record_bundle = witness.record_bundle.clone();
        let step = record_bundle
            .bundle
            .steps
            .first()
            .expect("sample append record bundle has one hop");
        let root_before = step.root_before;
        let previous_note_nullifier = step.input_nullifiers[0];
        let output_commitment = step.output_commitments[0];
        if previous_bundle.accumulator.initial_root == root_before {
            previous_bundle.accumulator.initial_root =
                fixed_bytes(b"python-height-profile-distinct-initial-root");
        }
        previous_bundle.accumulator.topup_anchor_nullifiers =
            vec![fixed_bytes(b"python-height-profile-distinct-topup")];
        previous_bundle.accumulator.final_root = root_before;
        previous_bundle.accumulator.current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_bytes(b"python-height-profile-previous-note"),
            spend_nullifier: previous_note_nullifier,
            amount: Numeric::new(7, 0),
        };
        refresh_recursive_spend_bundle_public_inputs(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope(
            &mut previous_bundle,
            fixed_bytes(b"python-height-profile-previous-proof-vk"),
        );

        let previous_proof_open_archive =
            sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle);
        let request = KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            previous_lineage_verifier_record: None,
            record_bundle,
            pallas_open_envelopes_archive: witness.pallas_open_envelopes_archive.clone(),
            current_note: KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: fixed_bytes(b"python-height-profile-current-nullifier"),
                amount: Numeric::new(7, 0),
            },
            output_proof_circuit_id: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.to_owned(),
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
            block_height: None,
        };
        request
            .validate_public_binding()
            .expect("Python append transition-profile request is well formed");
        request
    }

    fn window_first_recursive_spend_hop_record(
        record_bundle: &mut KagemushaVerifiedFoldRecordBundle,
    ) {
        let record = &mut record_bundle
            .verifier_records
            .first_mut()
            .expect("sample record bundle has a verifier record")
            .record;
        record.activation_height = Some(2);
        record.withdraw_height = Some(4);
    }

    fn refresh_recursive_spend_bundle_public_inputs(bundle: &mut KagemushaRecursiveSpendBundleV1) {
        let mut public_inputs =
            kagemusha_recursive_spend_public_inputs_from_accumulator(&bundle.accumulator)
                .expect("mutated recursive spend accumulator public inputs");
        if bundle.recursive_proof.verifier_key_id.name
            == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
        {
            public_inputs.recursive_verifier_scalar_projection_digest =
                recursive_spend_lineage_scalar_projection(0x6C);
        }
        bundle.recursive_proof.public_inputs = public_inputs;
        bundle.recursive_proof.public_inputs_hash = bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("mutated recursive spend public-input hash");
    }

    fn sample_recursive_spend_redeem_request(
        public_amount: u128,
    ) -> KagemushaRecursiveSpendRedeemRequestV1 {
        let mut redeem_proof = ProofAttachment::new_ref(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![0x5A; 64]),
            VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "python-recursive-unshield"),
        );
        redeem_proof.vk_commitment = Some(fixed_bytes(b"python-recursive-unshield-vk"));
        KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: sample_kagemusha_recursive_spend_bundle(),
            recipient: sample_account(0xB8),
            public_amount,
            redeem_proof,
            lineage_witness: None,
            lineage_verifier_record: None,
            block_height: None,
        }
    }

    fn sample_recursive_spend_init_request() -> KagemushaRecursiveSpendInitRequestV1 {
        let chain_id: ChainId = "kagemusha-python-current-hop".parse().expect("chain id");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "kgmpyhop".parse().expect("asset definition name"),
        );
        let vk_id = VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "python-current-hop");
        let verifier_key = iroha_data_model::proof::VerifyingKeyBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            vec![0xC4; 48],
        );
        let vk_commitment = iroha_core::zk::hash_vk(&verifier_key);
        let proof_schema = b"python-current-hop-public-inputs-v1".to_vec();
        let proof_envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "python-current-hop-circuit".to_owned(),
            vk_hash: vk_commitment,
            public_inputs: proof_schema.clone(),
            proof_bytes: vec![0xA1; 16],
            aux: Vec::new(),
        };
        let mut attachment = ProofAttachment::new_ref(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            ProofBox::new(
                ZK_BACKEND_HALO2_IPA.to_owned(),
                norito::to_bytes(&proof_envelope).expect("encode Python hop proof envelope"),
            ),
            vk_id.clone(),
        );
        attachment.vk_commitment = Some(vk_commitment);
        let output_commitment = fixed_bytes(b"python-current-hop-output");
        let step = KagemushaVerifiedFoldStep {
            root_before: fixed_bytes(b"python-current-hop-root-before"),
            input_nullifiers: vec![fixed_bytes(b"python-current-hop-input")],
            output_commitments: vec![output_commitment],
            root_after: fixed_bytes(b"python-current-hop-root-after"),
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            "python-current-hop-circuit",
            BackendTag::Halo2IpaPasta,
            "pallas",
            Hash::new(proof_schema.as_slice()).into(),
            vk_commitment,
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = ConfidentialStatus::Active;
        record.max_proof_bytes = 4096;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.key = Some(verifier_key);
        let record_bundle = KagemushaVerifiedFoldRecordBundle {
            bundle: KagemushaVerifiedFoldBundle {
                chain_id,
                asset,
                steps: vec![step],
            },
            verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
        };
        let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&sample_recursive_spend_pallas_archive(1))
                .expect("decode Python current-hop Pallas archive");
        let envelope = envelopes
            .first_mut()
            .expect("Pallas archive contains one envelope");
        envelope.vk_commitment = Some(vk_commitment);
        envelope.public_inputs_schema_hash = Some(Hash::new(proof_schema.as_slice()).into());
        KagemushaRecursiveSpendInitRequestV1 {
            record_bundle,
            pallas_open_envelopes_archive: norito::to_bytes(&envelopes)
                .expect("encode Python current-hop Pallas archive"),
            current_note: KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: fixed_bytes(b"python-current-hop-note-nullifier"),
                amount: Numeric::new(42, 0),
            },
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        }
    }

    fn sample_recursive_spend_lineage_verifier_record()
    -> iroha_data_model::proof::VerifyingKeyRecord {
        let verifier_key = iroha_data_model::proof::VerifyingKeyBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            vec![0xC7; 48],
        );
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            BackendTag::Halo2IpaPasta,
            "pallas",
            iroha_data_model::offline::kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
            iroha_core::zk::hash_vk(&verifier_key),
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = ConfidentialStatus::Active;
        record.max_proof_bytes = 4096;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.key = Some(verifier_key);
        record
    }

    fn sample_previous_recursive_proof_open_envelopes_archive(
        previous_bundle: &KagemushaRecursiveSpendBundleV1,
    ) -> Vec<u8> {
        let expected =
            iroha_data_model::offline::kagemusha_recursive_previous_proof_open_envelope_metadata(
                previous_bundle,
            )
            .expect("Python previous proof opening metadata");
        let envelope = sample_pallas_open_envelope_with_metadata(
            4,
            "python-recursive-spend-previous-proof-open-envelope",
            expected,
        );
        norito::to_bytes(&vec![envelope])
            .expect("encode Python previous proof open-envelope archive")
    }

    fn sample_recursive_spend_pallas_archive(hop_count: usize) -> Vec<u8> {
        let envelopes = (0..hop_count)
            .map(|hop_index| {
                let label =
                    0x90_u8.wrapping_add(u8::try_from(hop_index).expect("hop index fits u8"));
                iroha_zkp_halo2::OpenVerifyEnvelope {
                    params: iroha_zkp_halo2::IpaParams {
                        version: 1,
                        curve_id: 1,
                        n: 2,
                        g: vec![[label; Hash::LENGTH], [label.wrapping_add(1); Hash::LENGTH]],
                        h: vec![
                            [label.wrapping_add(2); Hash::LENGTH],
                            [label.wrapping_add(3); Hash::LENGTH],
                        ],
                        u: [label.wrapping_add(4); Hash::LENGTH],
                    },
                    public: iroha_zkp_halo2::PolyOpenPublic {
                        version: 1,
                        curve_id: 1,
                        n: 2,
                        z: [label.wrapping_add(5); Hash::LENGTH],
                        t: [label.wrapping_add(6); Hash::LENGTH],
                        p_g: [label.wrapping_add(7); Hash::LENGTH],
                    },
                    proof: iroha_zkp_halo2::IpaProofData {
                        version: 1,
                        l: vec![[label.wrapping_add(8); Hash::LENGTH]],
                        r: vec![[label.wrapping_add(9); Hash::LENGTH]],
                        a_final: [label.wrapping_add(10); Hash::LENGTH],
                        b_final: [label.wrapping_add(11); Hash::LENGTH],
                    },
                    transcript_label: format!("python-mixed-lineage-open-envelope-{hop_index}"),
                    vk_commitment: Some([label.wrapping_add(12); Hash::LENGTH]),
                    public_inputs_schema_hash: Some([label.wrapping_add(13); Hash::LENGTH]),
                    domain_tag: Some([label.wrapping_add(14); Hash::LENGTH]),
                }
            })
            .collect::<Vec<_>>();
        norito::to_bytes(&envelopes).expect("encode Python Pallas envelope archive")
    }

    fn sample_semantic_redeem_request_with_reserved_previous_lineage()
    -> KagemushaRecursiveSpendRedeemRequestV1 {
        let mut request = sample_recursive_spend_redeem_request(42);
        let vk_id = VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "python-mixed-lineage-hop");
        let verifier_key = iroha_data_model::proof::VerifyingKeyBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            vec![0xE7; 32],
        );
        let vk_commitment = iroha_core::zk::hash_vk(&verifier_key);
        let proof_schema = b"python-mixed-lineage-hop-public-inputs-v1".to_vec();
        let proof_schema_hash: [u8; Hash::LENGTH] = Hash::new(proof_schema.as_slice()).into();
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            BackendTag::Halo2IpaPasta,
            "pallas",
            proof_schema_hash,
            vk_commitment,
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = ConfidentialStatus::Active;
        record.max_proof_bytes = 4096;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.key = Some(verifier_key.clone());

        let intermediate_root = fixed_bytes(b"python-mixed-lineage-intermediate-root");
        let intermediate_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_bytes(b"python-mixed-lineage-note-0"),
            spend_nullifier: fixed_bytes(b"python-mixed-lineage-nullifier-0"),
            amount: request.bundle.accumulator.current_note.amount.clone(),
        };
        let step0 = KagemushaVerifiedFoldStep {
            root_before: request.bundle.accumulator.initial_root,
            input_nullifiers: request.bundle.accumulator.topup_anchor_nullifiers.clone(),
            output_commitments: vec![intermediate_note.note_commitment],
            root_after: intermediate_root,
            attachment: {
                let proof_envelope = OpenVerifyEnvelope {
                    backend: BackendTag::Halo2IpaPasta,
                    circuit_id:
                        iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
                            .to_owned(),
                    vk_hash: vk_commitment,
                    public_inputs: proof_schema.clone(),
                    proof_bytes: vec![0xA1; 16],
                    aux: Vec::new(),
                };
                let mut attachment = ProofAttachment::new_ref(
                    ZK_BACKEND_HALO2_IPA.to_owned(),
                    ProofBox::new(
                        ZK_BACKEND_HALO2_IPA.to_owned(),
                        norito::to_bytes(&proof_envelope)
                            .expect("encode first Python mixed lineage hop proof"),
                    ),
                    vk_id.clone(),
                );
                attachment.vk_commitment = Some(vk_commitment);
                attachment
            },
            verifier_key: verifier_key.clone(),
        };
        let step1 = KagemushaVerifiedFoldStep {
            root_before: intermediate_root,
            input_nullifiers: vec![intermediate_note.spend_nullifier],
            output_commitments: vec![request.bundle.accumulator.current_note.note_commitment],
            root_after: request.bundle.accumulator.final_root,
            attachment: {
                let proof_envelope = OpenVerifyEnvelope {
                    backend: BackendTag::Halo2IpaPasta,
                    circuit_id:
                        iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
                            .to_owned(),
                    vk_hash: vk_commitment,
                    public_inputs: proof_schema,
                    proof_bytes: vec![0xA2; 16],
                    aux: Vec::new(),
                };
                let mut attachment = ProofAttachment::new_ref(
                    ZK_BACKEND_HALO2_IPA.to_owned(),
                    ProofBox::new(
                        ZK_BACKEND_HALO2_IPA.to_owned(),
                        norito::to_bytes(&proof_envelope)
                            .expect("encode second Python mixed lineage hop proof"),
                    ),
                    vk_id.clone(),
                );
                attachment.vk_commitment = Some(vk_commitment);
                attachment
            },
            verifier_key,
        };
        let previous_accumulator = KagemushaRecursiveSpendAccumulatorV1 {
            domain: request.bundle.accumulator.domain.clone(),
            chain_id: request.bundle.accumulator.chain_id.clone(),
            asset: request.bundle.accumulator.asset.clone(),
            initial_root: request.bundle.accumulator.initial_root,
            final_root: intermediate_root,
            topup_anchor_nullifiers: request.bundle.accumulator.topup_anchor_nullifiers.clone(),
            hop_count: 1,
            lineage_digest: fixed_bytes(b"python-mixed-lineage-digest-0"),
            aggregation_transcript_digest: fixed_bytes(b"python-mixed-lineage-digest-0"),
            nullifier_digest: Hash::new(b"python-mixed-lineage-nullifier-digest"),
            output_commitment_digest: Hash::new(b"python-mixed-lineage-output-digest"),
            fold_digest: Hash::new(b"python-mixed-lineage-fold-digest"),
            recursive_proof_chain_digest: fixed_bytes(b"python-mixed-lineage-proof-chain"),
            transition_profile_binding_digest: fixed_bytes(
                b"python-mixed-lineage-transition-binding",
            ),
            append_opening_preflight_digest: [0u8; 32],
            append_boundary_digest: [0u8; 32],
            verifier_params_fingerprint: request.bundle.accumulator.verifier_params_fingerprint,
            fixed_window_table_schedule_digest: request
                .bundle
                .accumulator
                .fixed_window_table_schedule_digest,
            fixed_window_shared_table_manifest_digest: request
                .bundle
                .accumulator
                .fixed_window_shared_table_manifest_digest,
            fixed_window_table_base_digest: fixed_bytes(b"python-mixed-lineage-table-base"),
            verifier_witness_batch_digest: fixed_bytes(b"python-mixed-lineage-witness-batch"),
            verifier_opening_len: request.bundle.accumulator.verifier_opening_len,
            current_note: intermediate_note.clone(),
        };
        let mut previous_public_inputs =
            kagemusha_recursive_spend_public_inputs_from_accumulator(&previous_accumulator)
                .expect("Python reserved previous public inputs");
        previous_public_inputs.recursive_verifier_scalar_projection_digest =
            recursive_spend_lineage_scalar_projection(0xF1);
        let previous_public_inputs_hash = previous_public_inputs
            .public_inputs_hash()
            .expect("Python reserved previous public-input hash");
        let previous_recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                ZK_BACKEND_HALO2_IPA,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs: previous_public_inputs,
            public_inputs_hash: previous_public_inputs_hash,
            proof: ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xA3; 64]),
        };
        let mut pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&sample_recursive_spend_pallas_archive(2))
                .expect("decode Python mixed lineage Pallas archive");
        for envelope in &mut pallas_open_envelopes {
            envelope.vk_commitment = Some(vk_commitment);
            envelope.public_inputs_schema_hash = Some(proof_schema_hash);
        }
        request.lineage_witness = Some(KagemushaRecursiveSpendLineageWitnessV1 {
            record_bundle: KagemushaVerifiedFoldRecordBundle {
                bundle: KagemushaVerifiedFoldBundle {
                    chain_id: request.bundle.accumulator.chain_id.clone(),
                    asset: request.bundle.accumulator.asset.clone(),
                    steps: vec![step0, step1],
                },
                verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
            },
            pallas_open_envelopes_archive: norito::to_bytes(&pallas_open_envelopes)
                .expect("encode Python mixed lineage Pallas archive"),
            current_notes: vec![
                intermediate_note,
                request.bundle.accumulator.current_note.clone(),
            ],
            previous_recursive_proofs: vec![previous_recursive_proof],
        });
        request
    }

    fn attach_reserved_lineage_envelope(
        request: &mut KagemushaRecursiveSpendRedeemRequestV1,
        include_lineage_slice: bool,
    ) {
        request.bundle.recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        request
            .bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            recursive_spend_lineage_scalar_projection(0x4D);
        request.bundle.recursive_proof.public_inputs_hash = request
            .bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("lineage recursive spend public-input hash");

        let mut proof_bytes = b"ZK1\0".to_vec();
        append_zk1_tlv(&mut proof_bytes, *b"PROF", &[0xB1; 64]);
        let mut instance_columns =
            kagemusha_recursive_spend_bundle_instance_values(&request.bundle)
                .expect("recursive spend public instance values")
                .public_instance_columns();
        if include_lineage_slice {
            instance_columns.push(vec![
                request
                    .bundle
                    .recursive_proof
                    .public_inputs
                    .recursive_verifier_scalar_projection_digest,
            ]);
        }
        append_zk1_raw_instance_columns(&mut proof_bytes, instance_columns);
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned(),
            vk_hash: fixed_bytes(b"python-recursive-lineage-envelope-vk"),
            public_inputs:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
                    .to_vec(),
            proof_bytes,
            aux: Vec::new(),
        };
        request.bundle.recursive_proof.proof = ProofBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            norito::to_bytes(&envelope).expect("encode recursive spend lineage envelope"),
        );
    }

    fn attach_strict_reserved_lineage_envelope(
        request: &mut KagemushaRecursiveSpendRedeemRequestV1,
    ) {
        attach_reserved_lineage_envelope(request, true);
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

    fn pallas_open_envelopes_archive_for_record_bundle_python(
        record_bundle: &KagemushaVerifiedFoldRecordBundle,
        label: &str,
    ) -> Vec<u8> {
        let hop_count = record_bundle.bundle.steps.len();
        let envelopes = record_bundle
            .bundle
            .steps
            .iter()
            .enumerate()
            .map(|(hop_index, step)| {
                let metadata =
                    iroha_core::zk::kagemusha_pallas_open_envelope_metadata_for_verified_hop(
                        &record_bundle.bundle.chain_id,
                        &record_bundle.bundle.asset,
                        hop_index,
                        step,
                    )
                    .expect("Pallas open-envelope hop metadata");
                let envelope_label = if hop_count == 1 {
                    label.to_owned()
                } else {
                    format!("{label}-{hop_index}")
                };
                sample_pallas_open_envelope_with_metadata(4, &envelope_label, metadata)
            })
            .collect::<Vec<_>>();
        norito::to_bytes(&envelopes).expect("encode Python Pallas envelope archive")
    }

    fn sample_one_hop_recursive_compact_record_bundle_for_python()
    -> KagemushaVerifiedFoldRecordBundle {
        static FIXTURE: OnceLock<KagemushaVerifiedFoldRecordBundle> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let chain_id: ChainId = "kagemusha-recursive-compact-python-real"
                    .parse()
                    .expect("chain id");
                let asset = AssetDefinitionId::new(
                    DomainId::try_new("offline", "universal").expect("domain id"),
                    "kgmpycompactreal"
                        .parse()
                        .expect("asset definition name"),
                );
                let record = iroha_core::zk::confidential_v2::confidential_transfer_v2_vk_record(
                    iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                    3,
                )
                .expect("confidential transfer v2 verifier record");
                let verifier_key = record.key.clone().expect("inline transfer verifier key");
                let spend_key = [0x11_u8; Hash::LENGTH];
                let input_rho = [0x21_u8; Hash::LENGTH];
                let input_diversifier =
                    iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                        b"kagemusha-python-compact-input",
                    );
                let input_owner_tag =
                    iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &spend_key,
                        input_diversifier,
                    )
                    .expect("input owner tag");
                let input_commitment =
                    iroha_core::zk::confidential_v2::derive_confidential_note_v2(
                        &asset.to_string(),
                        7,
                        input_rho,
                        input_owner_tag,
                    )
                    .expect("input commitment");
                let tree_commitments = vec![input_commitment];
                let root_before =
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(
                        &tree_commitments,
                    )
                    .expect("root before");
                let output_owner_tag =
                    iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &[0x41_u8; Hash::LENGTH],
                        iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                            b"kagemusha-python-compact-output",
                        ),
                    )
                    .expect("output owner tag");
                let proof =
                    iroha_core::zk::confidential_v2::build_confidential_transfer_proof_v2(
                        &chain_id,
                        &asset.to_string(),
                        &spend_key,
                        &tree_commitments,
                        &[iroha_core::zk::confidential_v2::ConfidentialTransferInputV2 {
                            amount: 7,
                            rho: input_rho,
                            diversifier: input_diversifier,
                            leaf_index: 0,
                        }],
                        &[iroha_core::zk::confidential_v2::ConfidentialTransferOutputV2 {
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
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(&next_tree)
                        .expect("root after");
                let mut attachment = ProofAttachment::new_ref(
                    ZK_BACKEND_HALO2_IPA.into(),
                    proof.proof,
                    VerifyingKeyId::new(
                        ZK_BACKEND_HALO2_IPA,
                        "kagemusha-python-compact-confidential-transfer-v2",
                    ),
                );
                attachment.vk_commitment = Some(iroha_core::zk::hash_vk(&verifier_key));
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

    fn sample_two_hop_recursive_compact_record_bundle_for_python()
    -> (KagemushaVerifiedFoldRecordBundle, Vec<u8>) {
        static FIXTURE: OnceLock<(KagemushaVerifiedFoldRecordBundle, Vec<u8>)> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let mut record_bundle = sample_one_hop_recursive_compact_record_bundle_for_python();
                let chain_id = record_bundle.bundle.chain_id.clone();
                let asset = record_bundle.bundle.asset.clone();
                let record = record_bundle
                    .verifier_records
                    .first()
                    .expect("one-hop Python compact fixture has verifier record")
                    .record
                    .clone();
                let verifier_key = record_bundle.bundle.steps[0].verifier_key.clone();
                let input_diversifier =
                    iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                        b"kagemusha-python-compact-input",
                    );
                let input_owner_tag =
                    iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &[0x11_u8; Hash::LENGTH],
                        input_diversifier,
                    )
                    .expect("input owner tag");
                let input_commitment =
                    iroha_core::zk::confidential_v2::derive_confidential_note_v2(
                        &asset.to_string(),
                        7,
                        [0x21_u8; Hash::LENGTH],
                        input_owner_tag,
                    )
                    .expect("input commitment");
                let mut tree_commitments = vec![input_commitment];
                assert_eq!(
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(
                        &tree_commitments,
                    )
                    .expect("first root"),
                    record_bundle.bundle.steps[0].root_before
                );
                tree_commitments.extend(
                    record_bundle.bundle.steps[0]
                        .output_commitments
                        .iter()
                        .copied(),
                );
                let second_input_diversifier =
                    iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                        b"kagemusha-python-compact-output",
                    );
                let second_root_before =
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(
                        &tree_commitments,
                    )
                    .expect("second root before");
                let second_owner_tag =
                    iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &[0x51_u8; Hash::LENGTH],
                        iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                            b"kagemusha-python-compact-second-output",
                        ),
                    )
                    .expect("second owner tag");
                let second_proof =
                    iroha_core::zk::confidential_v2::build_confidential_transfer_proof_v2(
                        &chain_id,
                        &asset.to_string(),
                        &[0x41_u8; Hash::LENGTH],
                        &tree_commitments,
                        &[iroha_core::zk::confidential_v2::ConfidentialTransferInputV2 {
                            amount: 7,
                            rho: [0x31_u8; Hash::LENGTH],
                            diversifier: second_input_diversifier,
                            leaf_index: 1,
                        }],
                        &[iroha_core::zk::confidential_v2::ConfidentialTransferOutputV2 {
                            amount: 7,
                            rho: [0x61_u8; Hash::LENGTH],
                            owner_tag: second_owner_tag,
                        }],
                        second_root_before,
                        &record.circuit_id,
                        &verifier_key,
                    )
                    .expect("second confidential transfer v2 proof");
                let mut final_tree = tree_commitments;
                final_tree.extend(second_proof.output_commitments.iter().copied());
                let second_root_after =
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(&final_tree)
                        .expect("second root after");
                let mut attachment = ProofAttachment::new_ref(
                    ZK_BACKEND_HALO2_IPA.into(),
                    second_proof.proof,
                    VerifyingKeyId::new(
                        ZK_BACKEND_HALO2_IPA,
                        "kagemusha-python-compact-confidential-transfer-v2",
                    ),
                );
                attachment.vk_commitment = Some(iroha_core::zk::hash_vk(&verifier_key));
                record_bundle.bundle.steps.push(KagemushaVerifiedFoldStep {
                    root_before: second_root_before,
                    input_nullifiers: second_proof.nullifiers,
                    output_commitments: second_proof.output_commitments,
                    root_after: second_root_after,
                    attachment,
                    verifier_key,
                });
                let archive = pallas_open_envelopes_archive_for_record_bundle_python(
                    &record_bundle,
                    "python-recursive-compact-multi-hop-open",
                );
                (record_bundle, archive)
            })
            .clone()
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
                let chain_id: ChainId = "kagemusha-recursive-spend-python-real"
                    .parse()
                    .expect("chain id");
                let asset = AssetDefinitionId::new(
                    DomainId::try_new("offline", "universal").expect("domain id"),
                    "kgmpyreal".parse().expect("asset definition name"),
                );
                let record = iroha_core::zk::confidential_v2::confidential_transfer_v2_vk_record(
                    iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                    3,
                )
                .expect("confidential transfer v2 verifier record");
                let verifier_key = record.key.clone().expect("inline transfer verifier key");
                let spend_key = [0x11_u8; Hash::LENGTH];
                let input_rho = [0x21_u8; Hash::LENGTH];
                let input_diversifier =
                    iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                        b"kagemusha-python-real-input",
                    );
                let input_owner_tag =
                    iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &spend_key,
                        input_diversifier,
                    )
                    .expect("input owner tag");
                let input_commitment =
                    iroha_core::zk::confidential_v2::derive_confidential_note_v2(
                        &asset.to_string(),
                        7,
                        input_rho,
                        input_owner_tag,
                    )
                    .expect("input commitment");
                let tree_commitments = vec![input_commitment];
                let root_before =
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(
                        &tree_commitments,
                    )
                    .expect("root before");
                let output_owner_tag =
                    iroha_core::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                        &[0x41_u8; Hash::LENGTH],
                        iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                            b"kagemusha-python-real-output",
                        ),
                    )
                    .expect("output owner tag");
                let proof =
                    iroha_core::zk::confidential_v2::build_confidential_transfer_proof_v2(
                        &chain_id,
                        &asset.to_string(),
                        &spend_key,
                        &tree_commitments,
                        &[iroha_core::zk::confidential_v2::ConfidentialTransferInputV2 {
                            amount: 7,
                            rho: input_rho,
                            diversifier: input_diversifier,
                            leaf_index: 0,
                        }],
                        &[iroha_core::zk::confidential_v2::ConfidentialTransferOutputV2 {
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
                    iroha_core::zk::confidential_v2::compute_confidential_root_v2(&next_tree)
                        .expect("root after");
                let mut attachment = ProofAttachment::new_ref(
                    ZK_BACKEND_HALO2_IPA.into(),
                    proof.proof,
                    VerifyingKeyId::new(
                        ZK_BACKEND_HALO2_IPA,
                        "kagemusha-python-confidential-transfer-v2",
                    ),
                );
                attachment.vk_commitment = Some(iroha_core::zk::hash_vk(&verifier_key));
                let step = KagemushaVerifiedFoldStep {
                    root_before,
                    input_nullifiers: proof.nullifiers,
                    output_commitments: proof.output_commitments,
                    root_after,
                    attachment,
                    verifier_key,
                };
                let id = step.attachment.vk_ref.clone();
                let record_bundle = KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id,
                        asset,
                        steps: vec![step],
                    },
                    verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id, record }],
                };
                let metadata =
                    iroha_core::zk::kagemusha_pallas_open_envelope_metadata_for_verified_hop(
                        &record_bundle.bundle.chain_id,
                        &record_bundle.bundle.asset,
                        0,
                        &record_bundle.bundle.steps[0],
                    )
                    .expect("Pallas open-envelope hop metadata");
                let envelope = sample_pallas_open_envelope_with_metadata(
                    4,
                    "python-recursive-spend-verify-open-envelope",
                    metadata,
                );
                let envelope_archive =
                    norito::to_bytes(&vec![envelope]).expect("encode Pallas envelope archive");
                let current_note = KagemushaSpendableNoteDescriptorV1 {
                    note_commitment: record_bundle.bundle.steps[0].output_commitments[0],
                    spend_nullifier: fixed_bytes(
                        b"python-recursive-spend-verify-current-nullifier",
                    ),
                    amount: Numeric::new(7, 0),
                };
                let vk_box = iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
                    .expect("recursive spend verifier key");
                iroha_core::zk::prove_kagemusha_recursive_spend_init_from_record_bundle_and_pallas_open_envelope_archive(
                    &record_bundle,
                    &envelope_archive,
                    current_note.clone(),
                    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                    &vk_box,
                    None,
                )
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

    fn empty_kagemusha_record_bundle_archive() -> Vec<u8> {
        let record_bundle = iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
            bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                chain_id: "kagemusha-python-empty-record-bundle"
                    .parse()
                    .expect("chain id"),
                asset: AssetDefinitionId::new(
                    DomainId::try_new("offline", "universal").expect("domain id"),
                    "kgmpy".parse().expect("asset definition name"),
                ),
                steps: Vec::new(),
            },
            verifier_records: Vec::new(),
        };
        norito::to_bytes(&record_bundle).expect("encode empty Kagemusha record bundle")
    }

    fn recursive_compact_token_archive_for_python(
        verifier_key_name: String,
        bind_public_inputs_hash: bool,
    ) -> Vec<u8> {
        let public_inputs = iroha_data_model::offline::KagemushaFoldedPublicInputs {
            domain: iroha_data_model::offline::KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
            aggregation_mode:
                iroha_data_model::offline::KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
            chain_id: "python-recursive-compact-malformed"
                .parse()
                .expect("chain id"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("offline", "universal").expect("domain id"),
                "kgmpycompact".parse().expect("asset definition name"),
            ),
            initial_root: [0x11; 32],
            final_root: [0x22; 32],
            hop_count: 1,
            nullifier_digest: Hash::new(b"python-recursive-compact-nullifiers"),
            output_commitment_digest: Hash::new(b"python-recursive-compact-outputs"),
            fold_digest: Hash::new(b"python-recursive-compact-fold"),
            aggregation_transcript_digest: [0x33; 32],
        };
        let public_inputs_hash = if bind_public_inputs_hash {
            public_inputs
                .public_inputs_hash()
                .expect("Python recursive compact public-input hash")
        } else {
            Hash::new(b"forged-python-recursive-compact-hash")
        };
        let token = iroha_data_model::offline::KagemushaCompactPaymentToken {
            public_inputs,
            folded_proof: iroha_data_model::offline::KagemushaFoldedProof {
                verifier_key_id: VerifyingKeyId::new(
                    iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                    verifier_key_name,
                ),
                public_inputs_hash,
                proof: ProofBox::new(iroha_core::zk::ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xC7]),
            },
        };
        norito::to_bytes(&token).expect("encode malformed recursive compact token")
    }

    fn malformed_recursive_compact_token_archive_for_python() -> Vec<u8> {
        recursive_compact_token_archive_for_python(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1.to_owned(),
            false,
        )
    }

    fn sentinel_spoofed_recursive_compact_token_archive_for_python() -> Vec<u8> {
        recursive_compact_token_archive_for_python(
            format!(
                "forged::{}",
                iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE
            ),
            true,
        )
    }

    fn recursive_compact_multi_row_token_archive_for_python(
        record_bundle: &KagemushaVerifiedFoldRecordBundle,
    ) -> Vec<u8> {
        recursive_compact_shape_token_archive_for_python(record_bundle, true, None)
    }

    fn recursive_compact_forged_vk_hash_token_archive_for_python(
        record_bundle: &KagemushaVerifiedFoldRecordBundle,
    ) -> Vec<u8> {
        recursive_compact_shape_token_archive_for_python(
            record_bundle,
            false,
            Some(fixed_bytes(b"python-recursive-compact-forged-vk-hash")),
        )
    }

    fn recursive_compact_shape_token_archive_for_python(
        record_bundle: &KagemushaVerifiedFoldRecordBundle,
        multi_row_instances: bool,
        envelope_vk_hash: Option<[u8; Hash::LENGTH]>,
    ) -> Vec<u8> {
        let verified_steps = record_bundle
            .bundle
            .steps
            .iter()
            .map(|step| iroha_data_model::offline::KagemushaFoldStep {
                root_before: step.root_before,
                input_nullifiers: step.input_nullifiers.clone(),
                output_commitments: step.output_commitments.clone(),
                root_after: step.root_after,
                proof_hash: iroha_core::zk::kagemusha_fold_step_proof_hash(&step.attachment.proof)
                    .expect("Python Kagemusha hop proof hash"),
                proof_public_inputs_digest:
                    iroha_core::zk::kagemusha_fold_step_public_inputs_digest(&step.attachment.proof)
                        .expect("Python Kagemusha hop public-input digest"),
                verifier_key_id: step.attachment.vk_ref.clone(),
                verifier_key_commitment: step
                    .attachment
                    .vk_commitment
                    .expect("Python sample hop has verifier-key commitment"),
                verifier_key_poseidon_digest:
                    iroha_data_model::offline::kagemusha_verifier_key_poseidon_digest(
                        step.verifier_key.backend.as_str(),
                        &step.verifier_key.bytes,
                    )
                    .expect("Python Kagemusha verifier key poseidon digest"),
            })
            .collect::<Vec<_>>();
        let evidence =
            iroha_data_model::offline::kagemusha_recursive_aggregation_evidence_from_steps(
                &record_bundle.bundle.chain_id,
                &record_bundle.bundle.asset,
                &verified_steps,
                4,
                fixed_bytes(b"python-recursive-compact-shape-verifier-params"),
                iroha_core::zk::kagemusha_recursive_fixed_window_table_schedule_digest(4)
                    .expect("Python recursive compact fixed-window schedule digest"),
                iroha_core::zk::kagemusha_recursive_fixed_window_shared_table_manifest_digest(4)
                    .expect("Python recursive compact shared-table manifest digest"),
                fixed_bytes(b"python-recursive-compact-shape-table-base"),
                fixed_bytes(b"python-recursive-compact-shape-witness-batch"),
            )
            .expect("Python recursive compact shape evidence");
        let public_inputs =
            iroha_data_model::offline::kagemusha_folded_public_inputs_from_aggregation_statement(
                &evidence.aggregation_statement,
            )
            .expect("Python recursive compact folded public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("Python recursive compact public-input hash");
        let mut recursive_public_inputs =
            iroha_data_model::offline::kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(
                &evidence,
            )
            .expect("Python recursive compact proof public inputs");
        recursive_public_inputs.recursive_verifier_scalar_projection_digest =
            fixed_bytes(b"python-recursive-compact-shape-scalar-projection");

        let compact_vk = iroha_core::zk::kagemusha_recursive_compact_payment_token_vk_box()
            .expect("Python recursive compact vk");
        let mut proof_bytes = b"ZK1\0".to_vec();
        append_zk1_tlv(&mut proof_bytes, *b"PROF", &[0xC7; 64]);
        let mut instance_columns =
            iroha_core::zk::kagemusha_recursive_aggregation_proof_public_input_instance_values(
                &recursive_public_inputs,
            )
            .expect("Python recursive compact public instance values")
            .public_instance_columns();
        if multi_row_instances {
            for (index, column) in instance_columns.iter_mut().enumerate() {
                let mut row = [0_u8; Hash::LENGTH];
                row[..8].copy_from_slice(
                    &(u64::try_from(index).expect("Python test index fits u64") + 1).to_le_bytes(),
                );
                column.push(row);
            }
        }
        append_zk1_raw_instance_columns(&mut proof_bytes, instance_columns);
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1
                .to_owned(),
            vk_hash: envelope_vk_hash.unwrap_or_else(|| iroha_core::zk::hash_vk(&compact_vk)),
            public_inputs:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
                    .to_vec(),
            proof_bytes,
            aux: Vec::new(),
        };
        let token = iroha_data_model::offline::KagemushaCompactPaymentToken {
            public_inputs,
            folded_proof: iroha_data_model::offline::KagemushaFoldedProof {
                verifier_key_id: VerifyingKeyId::new(
                    ZK_BACKEND_HALO2_IPA,
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
                ),
                public_inputs_hash,
                proof: iroha_data_model::proof::ProofBox::new(
                    ZK_BACKEND_HALO2_IPA.to_owned(),
                    norito::to_bytes(&envelope).expect("encode Python multi-row compact envelope"),
                ),
            },
        };
        norito::to_bytes(&token).expect("encode Python multi-row recursive compact token")
    }

    fn provider_metadata(provider_id: &str) -> PyProviderMetadata {
        PyProviderMetadata {
            provider_id: Some(provider_id.to_string()),
            profile_id: None,
            profile_aliases: None,
            availability: None,
            stake_amount: None,
            max_streams: Some(2),
            refresh_deadline: None,
            expires_at: None,
            ttl_secs: None,
            allow_unknown_capabilities: Some(true),
            capability_names: None,
            rendezvous_topics: None,
            notes: None,
            range_capability: Some(PyRangeCapability {
                max_chunk_span: u32::MAX,
                min_granularity: 1,
                supports_sparse_offsets: Some(true),
                requires_alignment: Some(false),
                supports_merkle_proof: Some(true),
            }),
            stream_budget: Some(PyStreamBudget {
                max_in_flight: 4,
                max_bytes_per_sec: 8 * 1024 * 1024,
                burst_bytes: Some(8 * 1024 * 1024),
            }),
            transport_hints: None,
        }
    }

    #[test]
    fn generate_sm2_keypair_roundtrip() {
        ensure_python();
        Python::attach(|py| {
            let (private_py, public_py) =
                generate_sm2_keypair_py(py, None).expect("generate SM2 keypair");
            let private_bytes = private_py.bind(py).as_bytes();
            let public_bytes = public_py.bind(py).as_bytes();
            assert_eq!(private_bytes.len(), SM2_PRIVATE_KEY_LENGTH);
            assert_eq!(public_bytes.len(), SM2_PUBLIC_KEY_UNCOMPRESSED_LENGTH);
            let private =
                parse_sm2_private_key(None, private_bytes).expect("parse SM2 private key");
            let derived_public = private.public_key().to_sec1_bytes(false);
            assert_eq!(derived_public.as_slice(), public_bytes);
        });
    }

    #[test]
    fn parse_public_key_multihash_returns_checked_payload() {
        ensure_python();
        let key_pair =
            KeyPair::from_seed(b"python-public-key-multihash".to_vec(), Algorithm::Ed25519);
        let (algorithm, expected_payload) =
            public_key_to_bytes(key_pair.public_key(), "fixture public key")
                .expect("fixture public key is well-formed");
        let encoded = key_pair
            .public_key()
            .try_to_prefixed_string()
            .expect("fixture public key prefixed multihash formats");

        Python::attach(|py| {
            let (parsed_algorithm, parsed_payload) =
                parse_public_key_multihash_py(py, &encoded).expect("public key multihash parses");
            assert_eq!(parsed_algorithm, algorithm.as_static_str());
            assert_eq!(parsed_payload.bind(py).as_bytes(), expected_payload);
        });
    }

    #[test]
    fn multihash_helpers_use_checked_formatters() {
        ensure_python();
        let key_pair = KeyPair::from_seed(b"python-multihash-helper".to_vec(), Algorithm::Ed25519);
        let (_, public_payload) = public_key_to_bytes(key_pair.public_key(), "fixture public key")
            .expect("fixture public key is well-formed");
        let public_payload = public_payload.to_vec();
        let (private_algorithm, private_payload) = key_pair.private_key().to_bytes();
        assert_eq!(private_algorithm, Algorithm::Ed25519);
        let exposed_private = ExposedPrivateKey(key_pair.private_key().clone());

        assert_eq!(
            public_key_multihash_py(Algorithm::Ed25519.as_static_str(), &public_payload, false)
                .expect("public key multihash formats"),
            public_key_multihash_string(key_pair.public_key(), false, "expected public key")
                .expect("expected public key multihash formats")
        );
        assert_eq!(
            public_key_multihash_py(Algorithm::Ed25519.as_static_str(), &public_payload, true)
                .expect("prefixed public key multihash formats"),
            public_key_multihash_string(key_pair.public_key(), true, "expected public key")
                .expect("expected prefixed public key multihash formats")
        );
        assert_eq!(
            private_key_multihash_py(
                Algorithm::Ed25519.as_static_str(),
                private_payload.as_slice(),
                false,
            )
            .expect("private key multihash formats"),
            private_key_multihash_string(&exposed_private, false, "expected private key")
                .expect("expected private key multihash formats")
        );
        assert_eq!(
            private_key_multihash_py(
                Algorithm::Ed25519.as_static_str(),
                private_payload.as_slice(),
                true,
            )
            .expect("prefixed private key multihash formats"),
            private_key_multihash_string(&exposed_private, true, "expected private key")
                .expect("expected prefixed private key multihash formats")
        );
    }

    #[test]
    fn sm2_fixture_from_seed_uses_checked_public_key_formatters() {
        ensure_python();
        let distid = "1234567812345678";
        let seed = [0x42_u8; SM2_PRIVATE_KEY_LENGTH];
        let message = b"python sm2 fixture checked multihash";

        Python::attach(|py| {
            let fixture = sm2_fixture_from_seed_py(py, distid, &seed, message)
                .expect("SM2 fixture generates");
            let fixture = fixture.bind(py);
            let public_key_sec1_hex = fixture
                .get_item("public_key_sec1_hex")
                .expect("SEC1 public key item lookup succeeds")
                .expect("SEC1 public key item exists")
                .extract::<String>()
                .expect("SEC1 public key is string");
            let public_key_multihash = fixture
                .get_item("public_key_multihash")
                .expect("multihash item lookup succeeds")
                .expect("multihash item exists")
                .extract::<String>()
                .expect("multihash is string");
            let public_key_prefixed = fixture
                .get_item("public_key_prefixed")
                .expect("prefixed item lookup succeeds")
                .expect("prefixed item exists")
                .extract::<String>()
                .expect("prefixed multihash is string");
            let public_key_sec1 =
                hex::decode(public_key_sec1_hex).expect("fixture SEC1 public key hex decodes");
            let payload = encode_sm2_public_key_payload(distid, &public_key_sec1)
                .expect("fixture SM2 public key payload encodes");
            let public_key = PublicKey::from_bytes(Algorithm::Sm2, &payload)
                .expect("fixture SM2 public key constructs");

            assert_eq!(
                sm2_public_key_multihash_py(&public_key_sec1, Some(distid))
                    .expect("SM2 public key multihash formats"),
                public_key_multihash
            );
            assert_eq!(
                sm2_fixture_public_key_multihashes(&public_key)
                    .expect("fixture public key multihashes format")
                    .1,
                public_key_prefixed
            );
        });
    }

    #[test]
    fn keypair_and_account_public_exports_use_checked_payloads() {
        ensure_python();
        let key_pair = KeyPair::from_seed(b"python-keypair-export".to_vec(), Algorithm::Ed25519);
        let (_, expected_public) = public_key_to_bytes(key_pair.public_key(), "fixture public key")
            .expect("fixture public key is well-formed");
        let expected_public = expected_public.to_vec();
        let (_, expected_private) = key_pair.private_key().to_bytes();
        let authority = AccountId::new(key_pair.public_key().clone())
            .canonical_i105()
            .expect("canonical authority");

        Python::attach(|py| {
            let (private_py, public_py) =
                keypair_to_py(py, key_pair.clone()).expect("keypair exports");
            assert_eq!(public_py.bind(py).as_bytes(), expected_public.as_slice());
            assert_eq!(private_py.bind(py).as_bytes(), expected_private.as_slice());
        });

        let account = PyAccountId::new(&authority).expect("account id parses");
        assert_eq!(
            account.public_key_hex().expect("public key hex"),
            hex::encode(expected_public)
        );
    }

    #[test]
    fn sorafs_alias_proof_fixture_generates_servable_checked_signer() {
        ensure_python();
        Python::attach(|py| {
            let fixture =
                sorafs_alias_proof_fixture_py(py, None).expect("alias proof fixture generates");
            let fixture = fixture.bind(py);
            let proof_b64 = fixture
                .get_item("proof_b64")
                .expect("proof item lookup succeeds")
                .expect("proof item exists")
                .extract::<String>()
                .expect("proof is string");
            let generated_at_unix = fixture
                .get_item("generated_at_unix")
                .expect("generated item lookup succeeds")
                .expect("generated item exists")
                .extract::<u64>()
                .expect("generated timestamp is integer");

            let evaluation =
                sorafs_evaluate_alias_proof_py(py, &proof_b64, None, Some(generated_at_unix))
                    .expect("alias proof evaluates");
            let evaluation = evaluation.bind(py);
            let state = evaluation
                .get_item("state")
                .expect("state item lookup succeeds")
                .expect("state item exists")
                .extract::<String>()
                .expect("state is string");
            let servable = evaluation
                .get_item("servable")
                .expect("servable item lookup succeeds")
                .expect("servable item exists")
                .extract::<bool>()
                .expect("servable is boolean");

            assert_eq!(state, "fresh");
            assert!(servable);
        });
    }

    #[test]
    fn decode_transaction_receipt_json_roundtrip() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let payload = iroha_data_model::transaction::TransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
            signed_transaction_hash: Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0xB6; 32],
            ))),
            submitted_at_ms: 42,
            submitted_at_height: 7,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let decoded = decode_transaction_receipt_json_py(&bytes).expect("decode receipt json");
        let expected = json::to_json(&receipt).expect("serialize receipt");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn kagemusha_python_native_provers_reject_malformed_archives() {
        ensure_python();
        Python::attach(|py| {
            let err =
                kagemusha_prove_verified_compact_payment_token_with_records_py(py, &[0x01, 0x02])
                    .expect_err("compact-token prover must reject malformed record bundle archives")
                    .to_string();
            assert!(err.contains("invalid Kagemusha verified compact-token record bundle"));

            let err =
                kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes_py(
                    py,
                    &[0x01, 0x02],
                    &[0x03, 0x04],
                )
                .expect_err("recursive aggregation prover must reject malformed record bundle archives")
                .to_string();
            assert!(err.contains("invalid Kagemusha recursive aggregation record bundle"));

            let record_archive = empty_kagemusha_record_bundle_archive();
            let err =
                kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes_py(
                    py,
                    &record_archive,
                    &[],
                )
                .expect_err("recursive aggregation prover must reject empty Pallas open-envelope archive")
                .to_string();
            assert!(
                err.contains("Pallas open-envelope archive must not be empty"),
                "unexpected error: {err}"
            );

            for (label, call) in [
                (
                    "init",
                    kagemusha_recursive_spend_init_py
                        as fn(Python<'_>, &[u8]) -> PyResult<Py<PyBytes>>,
                ),
                (
                    "append",
                    kagemusha_recursive_spend_append_py
                        as fn(Python<'_>, &[u8]) -> PyResult<Py<PyBytes>>,
                ),
                (
                    "verify",
                    kagemusha_recursive_spend_verify_py
                        as fn(Python<'_>, &[u8]) -> PyResult<Py<PyBytes>>,
                ),
                (
                    "redeem",
                    kagemusha_recursive_spend_redeem_py
                        as fn(Python<'_>, &[u8]) -> PyResult<Py<PyBytes>>,
                ),
            ] {
                let err = call(py, &[])
                    .expect_err("recursive spend native helper must reject empty request archives")
                    .to_string();
                assert!(
                    err.contains("archive must not be empty"),
                    "{label} empty-archive error lost context: {err}"
                );

                let err = call(py, &[0x01, 0x02])
                    .expect_err(
                        "recursive spend native helper must reject malformed request archives",
                    )
                    .to_string();
                assert!(
                    err.contains("invalid Kagemusha recursive spend"),
                    "{label} malformed-archive error lost context: {err}"
                );
            }
        });
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_python_functions_validate_archives() {
        ensure_python();
        Python::attach(|py| {
            let err =
                kagemusha_recursive_spend_lineage_witness_from_init_result_py(py, &[], &[0x01])
                    .expect_err("init lineage helper must reject empty request archive")
                    .to_string();
            assert!(
                err.contains("archive must not be empty"),
                "unexpected empty request error: {err}"
            );

            let err =
                kagemusha_recursive_spend_lineage_witness_from_init_result_py(py, &[0x01], &[])
                    .expect_err("init lineage helper must reject empty bundle archive")
                    .to_string();
            assert!(
                err.contains("failed to fill whole buffer"),
                "unexpected empty bundle error: {err}"
            );

            let err = kagemusha_recursive_spend_lineage_witness_append_result_py(
                py,
                &[],
                &[0x01],
                &[0x02],
            )
            .expect_err("append lineage helper must reject empty witness archive")
            .to_string();
            assert!(
                err.contains("archive must not be empty"),
                "unexpected empty witness error: {err}"
            );

            let err = kagemusha_recursive_spend_lineage_witness_append_result_py(
                py,
                &[0x01],
                &[],
                &[0x02],
            )
            .expect_err("append lineage helper must reject empty request archive")
            .to_string();
            assert!(
                err.contains("failed to fill whole buffer"),
                "unexpected empty append request error: {err}"
            );

            let err = kagemusha_recursive_spend_lineage_witness_append_result_py(
                py,
                &[0x01],
                &[0x02],
                &[],
            )
            .expect_err("append lineage helper must reject empty bundle archive")
            .to_string();
            assert!(
                err.contains("failed to fill whole buffer"),
                "unexpected empty append bundle error: {err}"
            );

            let err = kagemusha_recursive_spend_lineage_witness_from_init_result_py(
                py,
                &[0x01, 0x02],
                &[0x03, 0x04],
            )
            .expect_err("init lineage helper must reject malformed archives")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive spend"),
                "unexpected malformed init helper error: {err}"
            );

            let err = kagemusha_recursive_spend_lineage_witness_append_result_py(
                py,
                &[0x01, 0x02],
                &[0x03, 0x04],
                &[0x05, 0x06],
            )
            .expect_err("append lineage helper must reject malformed archives")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive spend"),
                "unexpected malformed append helper error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_bridge_abi_version_python_function_is_additive_seven() {
        assert_eq!(kagemusha_recursive_spend_bridge_abi_version_py(), 7);
    }

    #[test]
    fn kagemusha_recursive_compact_python_function_rejects_malformed_record_bundle() {
        ensure_python();
        Python::attach(|py| {
            let err =
            kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
                py,
                &[1],
                &[2],
            )
            .expect_err("recursive compact prover must reject malformed record bundle")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive compact record bundle archive"),
                "unexpected recursive compact malformed-input error: {err}"
            );

            let record_archive = empty_kagemusha_record_bundle_archive();
            let err =
            kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
                py,
                &record_archive,
                &[2],
            )
            .expect_err("recursive compact prover must reject malformed Pallas archive")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive compact Pallas open-envelope archive"),
                "unexpected recursive compact malformed-Pallas error: {err}"
            );

            let detached_pallas_archive =
                norito::to_bytes(&vec![sample_pallas_open_envelope_with_metadata(
                    4,
                    "python-recursive-compact-detached-pallas",
                    iroha_zkp_halo2::PolyOpenTranscriptMetadata {
                        vk_commitment: Some(fixed_bytes(b"python-recursive-compact-detached-vk")),
                        public_inputs_schema_hash: Some(fixed_bytes(
                            b"python-recursive-compact-detached-schema",
                        )),
                        domain_tag: Some(fixed_bytes(b"python-recursive-compact-detached-domain")),
                    },
                )])
                .expect("encode detached recursive compact Pallas archive");
            let err =
            kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
                py,
                &record_archive,
                &detached_pallas_archive,
            )
            .expect_err("recursive compact prover must reject detached valid Pallas archive")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive compact record-backed Pallas preflight"),
                "unexpected recursive compact detached-Pallas error: {err}"
            );

            let one_hop_record_bundle = sample_one_hop_recursive_compact_record_bundle_for_python();
            let one_hop_record_archive = norito::to_bytes(&one_hop_record_bundle)
                .expect("encode Python one-hop compact record bundle");
            let one_hop_pallas_archive = pallas_open_envelopes_archive_for_record_bundle_python(
                &one_hop_record_bundle,
                "python-recursive-compact-one-hop-open",
            );
            let mut extra_pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(&one_hop_pallas_archive)
                    .expect("decode Python one-hop Pallas archive");
            extra_pallas_open_envelopes.push(
                extra_pallas_open_envelopes
                    .first()
                    .expect("one-hop Pallas archive contains one envelope")
                    .clone(),
            );
            let extra_pallas_archive = norito::to_bytes(&extra_pallas_open_envelopes)
                .expect("encode Python extra Pallas archive");
            let err =
            kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
                py,
                &one_hop_record_archive,
                &extra_pallas_archive,
            )
            .expect_err("recursive compact prover must reject extra valid Pallas opening archive")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                    && err.contains("witness"),
                "unexpected recursive compact extra-Pallas error: {err}"
            );

            let (multi_hop_record_bundle, multi_hop_pallas_archive) =
                sample_two_hop_recursive_compact_record_bundle_for_python();
            let multi_hop_record_archive = norito::to_bytes(&multi_hop_record_bundle)
                .expect("encode Python multi-hop compact record bundle");
            let mut missing_pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(&multi_hop_pallas_archive)
                    .expect("decode Python multi-hop Pallas archive");
            missing_pallas_open_envelopes.pop();
            let missing_pallas_archive = norito::to_bytes(&missing_pallas_open_envelopes)
                .expect("encode Python missing Pallas archive");
            let err =
            kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
                py,
                &multi_hop_record_archive,
                &missing_pallas_archive,
            )
            .expect_err("recursive compact prover must reject missing valid Pallas opening archive")
            .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                    && err.contains("witness"),
                "unexpected recursive compact missing-Pallas error: {err}"
            );
            let err =
            kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py(
                py,
                &multi_hop_record_archive,
                &multi_hop_pallas_archive,
            )
            .expect_err("valid multi-hop recursive compact archive must remain unavailable")
            .to_string();
            assert!(
                err.contains(
                    iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
                ),
                "unexpected recursive compact multi-hop error: {err}"
            );

            let err = kagemusha_verify_recursive_compact_payment_token_py(&[1])
                .expect_err("recursive compact verifier must reject malformed payment token")
                .to_string();
            assert!(
                err.contains("invalid Kagemusha recursive compact payment token archive"),
                "unexpected recursive compact verifier malformed-input error: {err}"
            );

            let malformed_token = malformed_recursive_compact_token_archive_for_python();
            let err = kagemusha_verify_recursive_compact_payment_token_py(&malformed_token)
                .expect_err("recursive compact verifier must reject malformed token binding")
                .to_string();
            assert!(
                err.contains("public-input hash mismatch"),
                "unexpected recursive compact malformed-binding error: {err}"
            );

            let one_hop_record_bundle = sample_one_hop_recursive_compact_record_bundle_for_python();
            let forged_vk_hash_token =
                recursive_compact_forged_vk_hash_token_archive_for_python(&one_hop_record_bundle);
            let err = kagemusha_verify_recursive_compact_payment_token_py(&forged_vk_hash_token)
                .expect_err("recursive compact token with forged verifier-key hash must reject")
                .to_string();
            assert!(
                err.contains("envelope verifier-key hash mismatch"),
                "unexpected recursive compact forged verifier-key hash error: {err}"
            );

            let multi_row_token =
                recursive_compact_multi_row_token_archive_for_python(&one_hop_record_bundle);
            let err = kagemusha_verify_recursive_compact_payment_token_py(&multi_row_token)
                .expect_err(
                    "Python recursive compact verifier must reject multi-row public instances",
                )
                .to_string();
            assert!(
                err.contains("exactly one row"),
                "unexpected Python recursive compact multi-row error: {err}"
            );

            let sentinel_spoofed_token =
                sentinel_spoofed_recursive_compact_token_archive_for_python();
            let err = kagemusha_verify_recursive_compact_payment_token_py(&sentinel_spoofed_token)
                .expect_err("sentinel-spoofed recursive compact token must reject")
                .to_string();
            assert!(
                err.contains("circuit id `forged::"),
                "unexpected recursive compact sentinel-spoofed error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_transition_profile_init_python_enforces_request_block_height() {
        ensure_python();
        Python::attach(|py| {
            let mut base = sample_recursive_spend_transition_profile_init_request();
            window_first_recursive_spend_hop_record(&mut base.record_bundle);

            let err = kagemusha_recursive_spend_transition_profile_init_py(
                py,
                &norito::to_bytes(&base).expect("encode no-height init transition request"),
            )
            .expect_err("height-unbound current-hop record must reject")
            .to_string();
            assert!(
                err.contains("chain height"),
                "unexpected no-height init transition error: {err}"
            );

            let mut future = base.clone();
            future.block_height = Some(1);
            let err = kagemusha_recursive_spend_transition_profile_init_py(
                py,
                &norito::to_bytes(&future).expect("encode future init transition request"),
            )
            .expect_err("future current-hop record must reject")
            .to_string();
            assert!(
                err.contains("not active"),
                "unexpected future init transition error: {err}"
            );

            let mut in_window = base.clone();
            in_window.block_height = Some(2);
            let profile_archive = kagemusha_recursive_spend_transition_profile_init_py(
                py,
                &norito::to_bytes(&in_window).expect("encode in-window init transition request"),
            )
            .expect("in-window current-hop record should build a transition profile");
            let profile: KagemushaRecursiveSpendTransitionProfileV1 =
                decode_from_bytes(profile_archive.bind(py).as_bytes())
                    .expect("decode Python init transition profile");
            assert_eq!(profile.hop_count, 1);

            let mut withdrawn = base;
            withdrawn.block_height = Some(4);
            let err = kagemusha_recursive_spend_transition_profile_init_py(
                py,
                &norito::to_bytes(&withdrawn).expect("encode withdrawn init transition request"),
            )
            .expect_err("withdrawn current-hop record must reject")
            .to_string();
            assert!(
                err.contains("not active"),
                "unexpected withdrawn init transition error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_transition_profile_append_python_enforces_request_block_height() {
        ensure_python();
        Python::attach(|py| {
            let mut base = sample_recursive_spend_transition_profile_append_request();
            window_first_recursive_spend_hop_record(&mut base.record_bundle);

            let err = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&base).expect("encode no-height append transition request"),
            )
            .expect_err("height-unbound current-hop record must reject")
            .to_string();
            assert!(
                err.contains("chain height"),
                "unexpected no-height append transition error: {err}"
            );

            let mut future = base.clone();
            future.block_height = Some(1);
            let err = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&future).expect("encode future append transition request"),
            )
            .expect_err("future current-hop record must reject")
            .to_string();
            assert!(
                err.contains("not active"),
                "unexpected future append transition error: {err}"
            );

            let mut in_window = base.clone();
            in_window.block_height = Some(2);
            let profile_archive = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&in_window).expect("encode in-window append transition request"),
            )
            .expect("in-window current-hop record should build an append transition profile");
            let profile: KagemushaRecursiveSpendTransitionProfileV1 =
                decode_from_bytes(profile_archive.bind(py).as_bytes())
                    .expect("decode Python append transition profile");
            assert_eq!(
                profile.hop_count,
                in_window.previous_bundle.accumulator.hop_count + 1
            );

            let mut withdrawn = base;
            withdrawn.block_height = Some(4);
            let err = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&withdrawn).expect("encode withdrawn append transition request"),
            )
            .expect_err("withdrawn current-hop record must reject")
            .to_string();
            assert!(
                err.contains("not active"),
                "unexpected withdrawn append transition error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_transition_profile_append_python_binds_append_opening_preflight() {
        ensure_python();
        let (mut previous_bundle, witness) =
            sample_verifying_semantic_recursive_spend_lineage_fixture();
        let record_bundle = witness.record_bundle.clone();
        let step = record_bundle
            .bundle
            .steps
            .first()
            .expect("sample append record bundle has one hop");
        if previous_bundle.accumulator.initial_root == step.root_before {
            previous_bundle.accumulator.initial_root =
                fixed_bytes(b"python-transition-profile-distinct-initial-root");
        }
        previous_bundle.accumulator.topup_anchor_nullifiers =
            vec![fixed_bytes(b"python-transition-profile-distinct-topup")];
        previous_bundle.accumulator.final_root = step.root_before;
        previous_bundle.accumulator.current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_bytes(b"python-transition-profile-previous-note"),
            spend_nullifier: step.input_nullifiers[0],
            amount: Numeric::new(7, 0),
        };
        refresh_recursive_spend_bundle_public_inputs(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope(
            &mut previous_bundle,
            fixed_bytes(b"python-transition-profile-previous-proof-envelope-vk"),
        );

        let previous_proof_open_archive =
            sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle);
        let current_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step.output_commitments[0],
            spend_nullifier: fixed_bytes(b"python-transition-profile-current-nullifier"),
            amount: Numeric::new(7, 0),
        };
        let mut request = KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            previous_lineage_verifier_record: None,
            record_bundle,
            pallas_open_envelopes_archive: witness.pallas_open_envelopes_archive.clone(),
            current_note,
            output_proof_circuit_id: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.to_owned(),
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
            block_height: None,
        };
        request
            .validate_public_binding()
            .expect("Python append request with previous proof openings is well formed");

        Python::attach(|py| {
            let profile_archive = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&request)
                    .expect("encode Python append transition-profile request"),
            )
            .expect("Python append transition profile with previous proof openings");
            let profile: KagemushaRecursiveSpendTransitionProfileV1 =
                decode_from_bytes(profile_archive.bind(py).as_bytes())
                    .expect("decode Python append transition profile");
            let append_opening_preflight_digest = profile
                .append_opening_preflight_digest
                .expect("Python append profile binds append opening preflight digest");
            assert_ne!(
                append_opening_preflight_digest,
                [0u8; Hash::LENGTH],
                "Python append opening preflight digest must be non-zero"
            );
            assert!(
                profile
                    .previous_recursive_proof_open_envelopes_archive_digest
                    .is_some(),
                "Python append profile must retain the previous-proof opening archive digest"
            );
            let append_opening_preflight = profile
                .append_opening_preflight
                .as_ref()
                .expect("Python append profile binds full append opening preflight contract");
            assert_eq!(
                append_opening_preflight.append_opening_preflight_digest,
                append_opening_preflight_digest,
                "Python append profile contract digest must match the profile digest field"
            );
            assert_eq!(
                Some(
                    append_opening_preflight.previous_recursive_proof_open_envelopes_archive_digest
                ),
                profile.previous_recursive_proof_open_envelopes_archive_digest,
                "Python append profile contract must bind the previous opening archive digest"
            );
            assert_eq!(
                append_opening_preflight.current_hop_proof_hash,
                profile.current_hop_statement.proof_hash,
                "Python append profile contract must bind the current-hop proof hash"
            );

            let mut forged_current_hop_opening = request.clone();
            let mut forged_current_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(
                    &forged_current_hop_opening.pallas_open_envelopes_archive,
                )
                .expect("decode Python current-hop Pallas archive");
            forged_current_envelopes[0].domain_tag = Some(fixed_bytes(
                b"python-transition-profile-forged-current-domain",
            ));
            forged_current_hop_opening.pallas_open_envelopes_archive =
                norito::to_bytes(&forged_current_envelopes)
                    .expect("encode Python forged current-hop Pallas archive");
            let err = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&forged_current_hop_opening)
                    .expect("encode Python forged current-hop transition request"),
            )
            .expect_err("Python append profile must reject forged current-hop opening metadata")
            .to_string();
            assert!(
                err.contains("hop domain metadata mismatch"),
                "Python forged current-hop opening returned unexpected error: {err}"
            );

            request
                .previous_recursive_proof_open_envelopes_archive
                .clear();
            let legacy_profile_archive = kagemusha_recursive_spend_transition_profile_append_py(
                py,
                &norito::to_bytes(&request)
                    .expect("encode Python legacy append transition-profile request"),
            )
            .expect("Python legacy append transition profile without previous proof openings");
            let legacy_profile: KagemushaRecursiveSpendTransitionProfileV1 =
                decode_from_bytes(legacy_profile_archive.bind(py).as_bytes())
                    .expect("decode Python legacy append transition profile");
            assert_eq!(
                legacy_profile.append_opening_preflight_digest, None,
                "Python legacy append profiles must not synthesize append opening preflight bytes"
            );
            assert_eq!(
                legacy_profile.append_opening_preflight, None,
                "Python legacy append profiles must not synthesize append opening preflight contracts"
            );
            assert_eq!(
                legacy_profile.previous_recursive_proof_open_envelopes_archive_digest, None,
                "Python legacy append profiles must not bind absent previous proof opening bytes"
            );
            let profile_digest =
                iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_digest(
                    &profile,
                )
                .expect("Python append opening profile digest");
            let legacy_profile_digest =
                iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_digest(
                    &legacy_profile,
                )
                .expect("Python legacy append profile digest");
            assert_ne!(
                profile_digest, legacy_profile_digest,
                "binding append opening preflight bytes must change the Python transition profile digest"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_append_python_rejects_forged_previous_proof_opening_metadata() {
        ensure_python();
        Python::attach(|py| {
            for (case, expected_field) in [
                (
                    "vk_commitment",
                    "previous_recursive_proof_open_envelopes_archive.vk_commitment",
                ),
                (
                    "public_inputs_schema_hash",
                    "previous_recursive_proof_open_envelopes_archive.public_inputs_schema_hash",
                ),
                (
                    "domain_tag",
                    "previous_recursive_proof_open_envelopes_archive.domain_tag",
                ),
            ] {
                let previous_bundle = sample_reserved_lineage_previous_bundle();
                let mut previous_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                    norito::decode_from_bytes(
                        &sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle),
                    )
                    .expect("decode Python previous proof open envelopes");
                let envelope = previous_open_envelopes
                    .first_mut()
                    .expect("previous proof archive contains one envelope");
                match case {
                    "vk_commitment" => {
                        envelope.vk_commitment =
                            Some(fixed_bytes(b"python-forged-previous-proof-vk"));
                    }
                    "public_inputs_schema_hash" => {
                        envelope.public_inputs_schema_hash =
                            Some(fixed_bytes(b"python-forged-previous-proof-schema"));
                    }
                    "domain_tag" => {
                        envelope.domain_tag =
                            Some(fixed_bytes(b"python-forged-previous-proof-domain"));
                    }
                    _ => unreachable!("covered previous-proof opening metadata case"),
                }

                let request = KagemushaRecursiveSpendAppendRequestV1 {
                    previous_bundle: previous_bundle.clone(),
                    previous_lineage_verifier_record: Some(
                        sample_recursive_spend_lineage_verifier_record(),
                    ),
                    record_bundle: KagemushaVerifiedFoldRecordBundle {
                        bundle: KagemushaVerifiedFoldBundle {
                            chain_id: previous_bundle.accumulator.chain_id.clone(),
                            asset: previous_bundle.accumulator.asset.clone(),
                            steps: Vec::new(),
                        },
                        verifier_records: Vec::new(),
                    },
                    pallas_open_envelopes_archive: Vec::new(),
                    current_note: previous_bundle.accumulator.current_note.clone(),
                    output_proof_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                        .to_owned(),
                    lineage_verifier_key: None,
                    lineage_proving_key_archive: None,
                    previous_recursive_proof_open_envelopes_archive: norito::to_bytes(
                        &previous_open_envelopes,
                    )
                    .expect("encode forged Python previous proof open archive"),
                    block_height: None,
                };
                let archive =
                    norito::to_bytes(&request).expect("encode Python append request archive");

                let err = kagemusha_recursive_spend_append_py(py, &archive)
                    .expect_err("Python host must reject forged previous-proof opening metadata")
                    .to_string();
                assert!(
                    err.contains(expected_field),
                    "{case} metadata splice returned unexpected error: {err}"
                );
            }
        });
    }

    #[test]
    fn kagemusha_recursive_spend_init_python_rejects_forged_current_hop_pallas_metadata() {
        ensure_python();
        Python::attach(|py| {
            for (case, expected_field) in [
                (
                    "vk_commitment",
                    "lineage_witness.pallas_open_envelopes_archive.vk_commitment",
                ),
                (
                    "public_inputs_schema_hash",
                    "lineage_witness.pallas_open_envelopes_archive.public_inputs_schema_hash",
                ),
            ] {
                let mut request = sample_recursive_spend_init_request();
                let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                    norito::decode_from_bytes(&request.pallas_open_envelopes_archive)
                        .expect("decode Python current-hop Pallas archive");
                let envelope = envelopes
                    .first_mut()
                    .expect("current-hop Pallas archive contains one envelope");
                match case {
                    "vk_commitment" => {
                        envelope.vk_commitment = Some(fixed_bytes(b"python-forged-current-vk"));
                    }
                    "public_inputs_schema_hash" => {
                        envelope.public_inputs_schema_hash =
                            Some(fixed_bytes(b"python-forged-current-schema"));
                    }
                    _ => unreachable!("covered current-hop Pallas metadata case"),
                }
                request.pallas_open_envelopes_archive =
                    norito::to_bytes(&envelopes).expect("encode forged Python Pallas archive");
                let archive =
                    norito::to_bytes(&request).expect("encode Python init request archive");

                let err = kagemusha_recursive_spend_init_py(py, &archive)
                    .expect_err("Python host must reject forged current-hop Pallas metadata")
                    .to_string();
                assert!(
                    err.contains(expected_field),
                    "{case} current-hop metadata splice returned unexpected error: {err}"
                );
            }
        });
    }

    #[test]
    fn kagemusha_recursive_spend_init_python_rejects_forged_current_hop_proof_circuit_id() {
        ensure_python();
        Python::attach(|py| {
            let mut request = sample_recursive_spend_init_request();
            let mut envelope: OpenVerifyEnvelope = norito::decode_from_bytes(
                &request.record_bundle.bundle.steps[0].attachment.proof.bytes,
            )
            .expect("decode Python current-hop proof envelope");
            envelope.circuit_id = "forged-python-current-hop-proof-circuit-id".to_owned();
            request.record_bundle.bundle.steps[0].attachment.proof.bytes =
                norito::to_bytes(&envelope)
                    .expect("encode forged Python current-hop proof envelope");
            let archive = norito::to_bytes(&request).expect("encode Python init request archive");

            let err = kagemusha_recursive_spend_init_py(py, &archive)
                .expect_err("Python host must reject forged current-hop proof circuit id")
                .to_string();
            assert!(
                err.contains(
                    "lineage_witness.record_bundle.bundle.steps.attachment.proof.circuit_id"
                ),
                "current-hop proof circuit-id splice returned unexpected error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_init_python_rejects_missing_lineage_key_artifacts() {
        ensure_python();
        Python::attach(|py| {
            let request = sample_recursive_spend_init_request();
            let archive = norito::to_bytes(&request).expect("encode Python init request archive");

            let err = kagemusha_recursive_spend_init_py(py, &archive)
                .expect_err("Python host must reject missing Reserved-lineage key artifacts")
                .to_string();
            assert!(
                err.contains("lineage_verifier_key"),
                "missing lineage key artifacts returned unexpected error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_append_python_rejects_missing_lineage_key_artifacts() {
        ensure_python();
        Python::attach(|py| {
            for (case, expected_field, verifier_key, proving_key_archive) in [
                (
                    "missing both artifacts",
                    "lineage_proving_key_archive",
                    None,
                    None,
                ),
                (
                    "missing proving key archive",
                    "lineage_proving_key_archive",
                    Some(VerifyingKeyBox::new("halo2/ipa".to_owned(), vec![0xAB; 32])),
                    None,
                ),
                (
                    "missing verifier key",
                    "lineage_verifier_key",
                    None,
                    Some(vec![0xAC; 32]),
                ),
            ] {
                let mut request = sample_reserved_lineage_append_request_missing_key_artifacts();
                request.lineage_verifier_key = verifier_key;
                request.lineage_proving_key_archive = proving_key_archive;
                let archive =
                    norito::to_bytes(&request).expect("encode Python append request archive");

                let err = kagemusha_recursive_spend_append_py(py, &archive)
                    .expect_err("Python host must reject missing Reserved-lineage key artifacts")
                    .to_string();
                assert!(
                    err.contains(expected_field),
                    "{case} returned unexpected error: {err}"
                );
            }
        });
    }

    #[test]
    fn kagemusha_recursive_spend_append_python_rejects_malformed_previous_proof_opening_archives() {
        ensure_python();
        Python::attach(|py| {
            let previous_bundle = sample_reserved_lineage_previous_bundle();
            let canonical_archive =
                sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle);
            let previous_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(&canonical_archive)
                    .expect("decode Python previous proof open envelopes");
            let previous_open_envelope = previous_open_envelopes
                .first()
                .expect("previous proof archive contains one envelope")
                .clone();

            for (case, previous_proof_open_archive) in [
                (
                    "malformed previous-proof opening archive",
                    vec![0x00, 0xFF, 0x01],
                ),
                (
                    "empty previous-proof opening vector",
                    norito::to_bytes::<Vec<iroha_zkp_halo2::OpenVerifyEnvelope>>(&Vec::new())
                        .expect("encode Python empty previous proof open archive"),
                ),
                (
                    "over-count previous-proof opening vector",
                    norito::to_bytes(&vec![
                        previous_open_envelope.clone(),
                        previous_open_envelope.clone(),
                    ])
                    .expect("encode Python over-count previous proof open archive"),
                ),
            ] {
                let request = KagemushaRecursiveSpendAppendRequestV1 {
                    previous_bundle: previous_bundle.clone(),
                    previous_lineage_verifier_record: Some(
                        sample_recursive_spend_lineage_verifier_record(),
                    ),
                    record_bundle: KagemushaVerifiedFoldRecordBundle {
                        bundle: KagemushaVerifiedFoldBundle {
                            chain_id: previous_bundle.accumulator.chain_id.clone(),
                            asset: previous_bundle.accumulator.asset.clone(),
                            steps: Vec::new(),
                        },
                        verifier_records: Vec::new(),
                    },
                    pallas_open_envelopes_archive: Vec::new(),
                    current_note: previous_bundle.accumulator.current_note.clone(),
                    output_proof_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                        .to_owned(),
                    lineage_verifier_key: None,
                    lineage_proving_key_archive: None,
                    previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
                    block_height: None,
                };
                let archive =
                    norito::to_bytes(&request).expect("encode Python append request archive");

                let err = match kagemusha_recursive_spend_append_py(py, &archive) {
                    Ok(_) => panic!("Python host must reject {case}"),
                    Err(err) => err.to_string(),
                };
                assert!(
                    err.contains("previous_recursive_proof_open_envelopes_archive"),
                    "{case} returned unexpected error: {err}"
                );
            }
        });
    }

    #[test]
    fn kagemusha_recursive_spend_append_python_rejects_stale_previous_proof_payload_opening() {
        ensure_python();
        Python::attach(|py| {
            let mut previous_bundle = sample_reserved_lineage_previous_bundle();
            let previous_proof_open_archive =
                sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle);
            let mut previous_proof_envelope: iroha_data_model::zk::OpenVerifyEnvelope =
                norito::decode_from_bytes(&previous_bundle.recursive_proof.proof.bytes)
                    .expect("decode Python previous recursive proof envelope");
            previous_proof_envelope.proof_bytes.push(0x42);
            previous_bundle.recursive_proof.proof.bytes =
                norito::to_bytes(&previous_proof_envelope)
                    .expect("encode Python stale previous recursive proof envelope");

            let request = KagemushaRecursiveSpendAppendRequestV1 {
                previous_bundle: previous_bundle.clone(),
                previous_lineage_verifier_record: Some(
                    sample_recursive_spend_lineage_verifier_record(),
                ),
                record_bundle: KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id: previous_bundle.accumulator.chain_id.clone(),
                        asset: previous_bundle.accumulator.asset.clone(),
                        steps: Vec::new(),
                    },
                    verifier_records: Vec::new(),
                },
                pallas_open_envelopes_archive: Vec::new(),
                current_note: previous_bundle.accumulator.current_note.clone(),
                output_proof_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
                lineage_verifier_key: None,
                lineage_proving_key_archive: None,
                previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
                block_height: None,
            };
            let archive = norito::to_bytes(&request).expect("encode Python append request archive");

            let err = match kagemusha_recursive_spend_append_py(py, &archive) {
                Ok(_) => panic!("Python host must reject stale previous-proof payload opening"),
                Err(err) => err.to_string(),
            };
            assert!(
                err.contains("previous_recursive_proof_open_envelopes_archive.domain_tag"),
                "stale previous-proof payload returned unexpected error: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_append_python_rejects_forged_previous_proof_circuit_id() {
        ensure_python();
        Python::attach(|py| {
            let mut previous_bundle = sample_reserved_lineage_previous_bundle();
            let previous_proof_open_archive =
                sample_previous_recursive_proof_open_envelopes_archive(&previous_bundle);
            let mut previous_proof_envelope: OpenVerifyEnvelope =
                norito::decode_from_bytes(&previous_bundle.recursive_proof.proof.bytes)
                    .expect("decode Python previous recursive proof envelope");
            previous_proof_envelope.circuit_id =
                "forged-python-previous-recursive-proof-circuit-id".to_owned();
            previous_bundle.recursive_proof.proof.bytes = norito::to_bytes(
                &previous_proof_envelope,
            )
            .expect("encode Python previous recursive proof envelope with forged circuit id");

            let request = KagemushaRecursiveSpendAppendRequestV1 {
                previous_bundle: previous_bundle.clone(),
                previous_lineage_verifier_record: Some(
                    sample_recursive_spend_lineage_verifier_record(),
                ),
                record_bundle: KagemushaVerifiedFoldRecordBundle {
                    bundle: KagemushaVerifiedFoldBundle {
                        chain_id: previous_bundle.accumulator.chain_id.clone(),
                        asset: previous_bundle.accumulator.asset.clone(),
                        steps: Vec::new(),
                    },
                    verifier_records: Vec::new(),
                },
                pallas_open_envelopes_archive: Vec::new(),
                current_note: previous_bundle.accumulator.current_note.clone(),
                output_proof_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
                lineage_verifier_key: None,
                lineage_proving_key_archive: None,
                previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
                block_height: None,
            };
            let archive = norito::to_bytes(&request).expect("encode Python append request archive");

            let err = match kagemusha_recursive_spend_append_py(py, &archive) {
                Ok(_) => {
                    panic!("Python host must reject forged previous recursive proof circuit id")
                }
                Err(err) => err.to_string(),
            };
            assert!(
                err.contains("previous_bundle.recursive_proof.proof.circuit_id"),
                "forged previous proof circuit-id returned unexpected error: {err}"
            );
        });
    }

    #[test]
    fn privacy_bridge_abi_version_python_function_is_additive_seven() {
        assert_eq!(privacy_bridge_abi_version_py(), 7);
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_python_function_rebuilds_init_witness() {
        ensure_python();
        Python::attach(|py| {
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
                lineage_verifier_key: None,
                lineage_proving_key_archive: None,
                block_height: None,
            };
            let request_archive = norito::to_bytes(&request).expect("encode init request");
            let bundle_archive = norito::to_bytes(&bundle).expect("encode init bundle");

            let output = kagemusha_recursive_spend_lineage_witness_from_init_result_py(
                py,
                &request_archive,
                &bundle_archive,
            )
            .expect("init lineage helper rebuilds witness archive");
            let decoded: KagemushaRecursiveSpendLineageWitnessV1 =
                decode_from_bytes(output.bind(py).as_bytes())
                    .expect("decode rebuilt lineage witness");
            assert_eq!(decoded, witness);

            let wrong_bundle_archive = norito::to_bytes(&sample_kagemusha_recursive_spend_bundle())
                .expect("encode mismatched bundle");
            let err = kagemusha_recursive_spend_lineage_witness_from_init_result_py(
                py,
                &request_archive,
                &wrong_bundle_archive,
            )
            .expect_err("lineage helper must reject mismatched init bundle")
            .to_string();
            assert!(
                err.contains("changed chain id"),
                "mismatched init bundle error lost continuity context: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_verify_python_function_reports_malformed_proof_material() {
        fn verify_result(
            py: Python<'_>,
            request: &KagemushaRecursiveSpendVerifyRequestV1,
        ) -> KagemushaRecursiveSpendVerifyResultV1 {
            let archive = norito::to_bytes(request).expect("encode recursive spend verify request");
            let output = kagemusha_recursive_spend_verify_py(py, &archive)
                .expect("verify function returns a diagnostic result archive");
            decode_from_bytes(output.bind(py).as_bytes())
                .expect("decode recursive spend verify result")
        }
        fn expect_request_error(
            py: Python<'_>,
            request: &KagemushaRecursiveSpendVerifyRequestV1,
            expected: &str,
        ) {
            let archive = norito::to_bytes(request).expect("encode recursive spend verify request");
            let err = kagemusha_recursive_spend_verify_py(py, &archive)
                .expect_err("malformed verify request must reject")
                .to_string();
            assert!(
                err.contains(expected),
                "malformed verify request error `{err}` did not contain `{expected}`"
            );
        }

        ensure_python();
        Python::attach(|py| {
            let request = KagemushaRecursiveSpendVerifyRequestV1 {
                bundle: sample_kagemusha_recursive_spend_bundle(),
                lineage_verifier_record: None,
                block_height: None,
            };
            let result = verify_result(py, &request);
            assert!(!result.valid);
            assert!(!result.chain_admissible);
            assert!(!result.witnessless_redeem_supported);
            assert!(result.lineage_witness_required_for_redeem);
            assert!(
                result.reason.contains("recursive spend proof envelope")
                    || result.reason.contains("fixed-window table schedule digest"),
                "malformed recursive proof envelope was not reported diagnostically: {}",
                result.reason
            );

            let mut trusted_setup_backend = request.clone();
            trusted_setup_backend.bundle.recursive_proof.proof =
                ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
            expect_request_error(py, &trusted_setup_backend, "proof.backend");

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
            expect_request_error(py, &stark_recursive_bundle, "proof.backend");

            let mut empty_recursive_proof = request;
            empty_recursive_proof.bundle.recursive_proof.proof =
                ProofBox::new(ZK_BACKEND_HALO2_IPA.into(), Vec::new());
            expect_request_error(py, &empty_recursive_proof, "proof.bytes");
        });
    }

    #[test]
    fn kagemusha_recursive_spend_verify_python_function_requires_lineage_record() {
        ensure_python();
        let mut bundle = sample_kagemusha_recursive_spend_bundle();
        bundle.recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_bytes(b"python-recursive-spend-lineage-verify-scalar");
        bundle.recursive_proof.public_inputs_hash = bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("lineage recursive spend public-input hash");
        let request = KagemushaRecursiveSpendVerifyRequestV1 {
            bundle,
            lineage_verifier_record: None,
            block_height: None,
        };
        let archive = norito::to_bytes(&request).expect("encode recursive spend verify request");

        Python::attach(|py| {
            let err = kagemusha_recursive_spend_verify_py(py, &archive)
                .expect_err("reserved lineage verify request without a record must reject")
                .to_string();
            assert!(
                err.contains("lineage_verifier_record"),
                "reserved lineage verification did not reject the malformed request: {err}"
            );

            let mut forged_record = sample_recursive_spend_lineage_verifier_record();
            forged_record.commitment = fixed_bytes(b"python-recursive-spend-forged-lineage-vk");
            let forged_request = KagemushaRecursiveSpendVerifyRequestV1 {
                bundle: request.bundle.clone(),
                lineage_verifier_record: Some(forged_record),
                block_height: None,
            };
            let archive =
                norito::to_bytes(&forged_request).expect("encode forged lineage verify request");
            let err = kagemusha_recursive_spend_verify_py(py, &archive)
                .expect_err("forged lineage verify request must reject")
                .to_string();
            assert!(
                err.contains("lineage_verifier_record.commitment"),
                "forged lineage verifier record was not rejected clearly: {err}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_verify_python_function_enforces_request_block_height() {
        ensure_python();
        Python::attach(|py| {
            let verify_result =
                |request: &KagemushaRecursiveSpendVerifyRequestV1| -> KagemushaRecursiveSpendVerifyResultV1 {
                    let archive =
                        norito::to_bytes(request).expect("encode recursive spend verify request");
                    let output = kagemusha_recursive_spend_verify_py(py, &archive)
                        .expect("verify function returns a diagnostic result archive");
                    decode_from_bytes(output.bind(py).as_bytes())
                        .expect("decode recursive spend verify result")
                };

            let mut record = sample_recursive_spend_lineage_verifier_record();
            record.activation_height = Some(2);
            record.withdraw_height = Some(4);
            let base = KagemushaRecursiveSpendVerifyRequestV1 {
                bundle: sample_reserved_lineage_previous_bundle(),
                lineage_verifier_record: Some(record),
                block_height: None,
            };

            let no_height = verify_result(&base);
            assert!(!no_height.valid);
            assert!(!no_height.chain_admissible);
            assert!(
                no_height.reason.contains("chain height"),
                "unexpected no-height reason: {}",
                no_height.reason
            );

            let mut future = base.clone();
            future.block_height = Some(1);
            let future = verify_result(&future);
            assert!(!future.valid);
            assert!(
                future.reason.contains("not active"),
                "unexpected future-height reason: {}",
                future.reason
            );

            let mut in_window = base.clone();
            in_window.block_height = Some(2);
            let in_window = verify_result(&in_window);
            assert!(!in_window.valid);
            assert!(
                !in_window.reason.contains("chain height")
                    && !in_window.reason.contains("not active"),
                "in-window request must reach proof validation: {}",
                in_window.reason
            );

            let mut withdrawn = base;
            withdrawn.block_height = Some(4);
            let withdrawn = verify_result(&withdrawn);
            assert!(!withdrawn.valid);
            assert!(
                withdrawn.reason.contains("not active"),
                "unexpected withdrawn-height reason: {}",
                withdrawn.reason
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_verify_python_function_reports_backend_valid_semantic_profile() {
        ensure_python();
        let bundle = sample_verifying_semantic_recursive_spend_bundle();
        let vk_box = iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
            .expect("recursive spend verifier key");
        assert!(
            iroha_core::zk::verify_kagemusha_recursive_spend_bundle(&bundle, &vk_box),
            "fixture must be backend-valid before chain-admission gating"
        );
        let request = KagemushaRecursiveSpendVerifyRequestV1 {
            bundle,
            lineage_verifier_record: None,
            block_height: None,
        };
        let archive = norito::to_bytes(&request).expect("encode recursive spend verify request");

        Python::attach(|py| {
            let output = kagemusha_recursive_spend_verify_py(py, &archive)
                .expect("verify function returns a diagnostic result archive");
            let result: KagemushaRecursiveSpendVerifyResultV1 =
                decode_from_bytes(output.bind(py).as_bytes())
                    .expect("decode recursive spend verify result");
            assert!(
                result.valid,
                "backend-valid semantic recursive spend proofs must be spendable offline"
            );
            assert!(
                !result.chain_admissible,
                "semantic recursive spend proofs without lineage witness are not directly redeemable"
            );
            assert!(!result.witnessless_redeem_supported);
            assert!(result.lineage_witness_required_for_redeem);
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
        });
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_native_rejects_semantic_profile() {
        let request = sample_recursive_spend_redeem_request(42);
        request
            .validate_public_binding()
            .expect("semantic recursive spend redeem request has valid public bindings");
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(request)
            .expect_err("Python native redeem builder must reject semantic profile");
        assert!(err.contains("private-hop lineage"));

        let wrong_amount = sample_recursive_spend_redeem_request(41);
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(wrong_amount)
            .expect_err("Python native redeem builder must reject wrong public amount");
        assert!(err.to_string().contains("public_amount"));

        let mut missing_anchor = sample_recursive_spend_redeem_request(42);
        missing_anchor
            .bundle
            .accumulator
            .topup_anchor_nullifiers
            .clear();
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(missing_anchor)
            .expect_err("Python native redeem builder must reject missing top-up anchors");
        assert!(err.to_string().contains("top-up anchor"));

        let mut missing_vk_commitment = sample_recursive_spend_redeem_request(42);
        missing_vk_commitment.redeem_proof.vk_commitment = None;
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(missing_vk_commitment)
            .expect_err("Python native redeem builder must reject missing redeem VK commitment");
        assert!(err.to_string().contains("vk_commitment"));

        let mut zero_vk_commitment = sample_recursive_spend_redeem_request(42);
        zero_vk_commitment.redeem_proof.vk_commitment = Some([0u8; Hash::LENGTH]);
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(zero_vk_commitment)
            .expect_err("Python native redeem builder must reject zero redeem VK commitment");
        assert!(err.to_string().contains("vk_commitment"));
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_requires_lineage_record_for_reserved_previous_proof()
    {
        let request = sample_semantic_redeem_request_with_reserved_previous_lineage();
        let err = request
            .validate_public_binding()
            .expect_err("semantic final redeem with reserved previous proof requires record");
        assert!(
            err.to_string().contains("lineage_verifier_record"),
            "unexpected missing lineage-record error: {err}"
        );
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(request.clone())
            .expect_err("Python native redeem builder must reject mixed lineage without record");
        assert!(
            err.contains("lineage_verifier_record"),
            "unexpected mixed-lineage rejection: {err}"
        );

        let mut with_record = request;
        with_record.lineage_verifier_record =
            Some(sample_recursive_spend_lineage_verifier_record());
        with_record
            .validate_public_binding()
            .expect("semantic final redeem accepts reserved previous proof with lineage record");

        let mut forged_record = with_record.clone();
        forged_record
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .commitment = fixed_bytes(b"python-recursive-spend-forged-mixed-lineage-vk");
        let err = forged_record
            .validate_public_binding()
            .expect_err("semantic final redeem must reject forged lineage verifier record");
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.commitment"),
            "unexpected forged lineage-record public-binding error: {err}"
        );
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(forged_record)
            .expect_err("Python native redeem builder must reject forged lineage verifier record");
        assert!(
            err.contains("lineage_verifier_record.commitment"),
            "unexpected forged lineage-record rejection: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_accepts_record_backed_lineage_witness() {
        ensure_python();
        let (bundle, witness) = sample_verifying_semantic_recursive_spend_lineage_fixture();
        let mut request = sample_recursive_spend_redeem_request(7);
        request.bundle = bundle.clone();
        request.lineage_witness = Some(witness.clone());
        request
            .validate_public_binding()
            .expect("record-backed recursive spend redeem request has valid public bindings");

        let instruction =
            kagemusha_recursive_spend_redeem_instruction_from_request(request.clone())
                .expect("Python native redeem builder accepts record-backed lineage witness");
        assert_eq!(instruction.bundle, bundle);
        assert_eq!(instruction.public_amount, 7);
        assert_eq!(instruction.lineage_witness.as_ref(), Some(&witness));

        let mut tampered_final_proof = request.clone();
        tampered_final_proof.bundle.recursive_proof.proof.bytes[0] ^= 0x01;
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(tampered_final_proof)
            .expect_err("Python native redeem builder must reject tampered final recursive proof");
        assert!(
            err.contains("final proof did not verify"),
            "unexpected tampered final recursive proof error: {err}"
        );

        let archive = norito::to_bytes(&request).expect("encode recursive spend redeem request");
        Python::attach(|py| {
            let output = kagemusha_recursive_spend_redeem_py(py, &archive)
                .expect("Python function accepts record-backed lineage witness");
            let decoded: iroha_data_model::isi::offline::RedeemKagemushaRecursive =
                decode_from_bytes(output.bind(py).as_bytes())
                    .expect("decode recursive redeem instruction");
            assert_eq!(decoded.bundle, bundle);
            assert_eq!(decoded.public_amount, 7);
            assert_eq!(decoded.lineage_witness.as_ref(), Some(&witness));
        });
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_rejects_adversarial_lineage_witnesses() {
        let (bundle, witness) = sample_verifying_semantic_recursive_spend_lineage_fixture();
        let base_request = {
            let mut request = sample_recursive_spend_redeem_request(7);
            request.bundle = bundle;
            request.lineage_witness = Some(witness);
            request
        };

        fn assert_rejects(request: KagemushaRecursiveSpendRedeemRequestV1, label: &str) {
            let err = match kagemusha_recursive_spend_redeem_instruction_from_request(request) {
                Ok(_) => panic!("Python native redeem builder must reject {label}"),
                Err(err) => err,
            };
            assert!(
                !err.is_empty(),
                "Python native redeem builder must report a reason for {label}"
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
        extra.id = VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "unused-python-lineage-hop");
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
        assert_rejects(malformed_pallas_archive, "malformed Pallas archive");

        let mut note_commitment_mismatch = base_request.clone();
        note_commitment_mismatch
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[0]
            .note_commitment = fixed_bytes(b"python-lineage-wrong-current-note");
        assert_rejects(note_commitment_mismatch, "current note commitment mismatch");

        let mut final_note_input_nullifier_collision = base_request.clone();
        let first_input = final_note_input_nullifier_collision
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[0]
            .input_nullifiers[0];
        final_note_input_nullifier_collision
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[0]
            .spend_nullifier = first_input;
        assert_rejects(
            final_note_input_nullifier_collision,
            "final note input-nullifier collision",
        );

        let mut final_note_output_commitment_collision = base_request.clone();
        let sibling_output = fixed_bytes(b"python-lineage-final-note-sibling-output");
        {
            let witness = final_note_output_commitment_collision
                .lineage_witness
                .as_mut()
                .expect("lineage witness");
            witness.record_bundle.bundle.steps[0]
                .output_commitments
                .push(sibling_output);
            witness.current_notes[0].spend_nullifier = sibling_output;
        }
        assert_rejects(
            final_note_output_commitment_collision,
            "final note output-commitment collision",
        );

        let mut reserved_lineage_with_record_witness = base_request.clone();
        attach_strict_reserved_lineage_envelope(&mut reserved_lineage_with_record_witness);
        reserved_lineage_with_record_witness.lineage_verifier_record =
            Some(sample_recursive_spend_lineage_verifier_record());
        assert_rejects(
            reserved_lineage_with_record_witness,
            "reserved lineage bundle with record-backed witness",
        );

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
    fn kagemusha_recursive_spend_redeem_python_native_accepts_witnessless_reserved_lineage_public_binding()
     {
        let mut request = sample_recursive_spend_redeem_request(42);
        attach_strict_reserved_lineage_envelope(&mut request);
        request.lineage_verifier_record = Some(sample_recursive_spend_lineage_verifier_record());
        request.validate_public_binding().expect(
            "witnessless reserved-lineage redeem validates before backend proof verification",
        );

        let err = kagemusha_recursive_spend_redeem_instruction_from_request(request.clone())
            .expect_err(
                "Python native redeem builder must reject backend-invalid reserved-lineage proof",
            );
        assert!(
            err.contains("missing verifier-slice public instance columns"),
            "unexpected structurally invalid reserved-lineage rejection: {err}"
        );

        let mut missing_lineage_slice = sample_recursive_spend_redeem_request(42);
        attach_reserved_lineage_envelope(&mut missing_lineage_slice, false);
        missing_lineage_slice.lineage_verifier_record =
            Some(sample_recursive_spend_lineage_verifier_record());
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(missing_lineage_slice)
            .expect_err("reserved-lineage redeem without verifier-slice columns must reject");
        assert!(
            err.contains("verifier-slice") || err.contains("public instance columns"),
            "unexpected missing-verifier-slice error: {err}"
        );

        let mut missing_scalar = request.clone();
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
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(missing_scalar)
            .expect_err("Python native redeem builder must reject zero lineage scalar projection");
        assert!(
            err.contains("recursive_verifier_scalar_projection_digest"),
            "unexpected zero-lineage-scalar error: {err}"
        );

        let mut malformed_envelope = request;
        malformed_envelope.bundle.recursive_proof.proof =
            ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![0xA5; 64]);
        let err = kagemusha_recursive_spend_redeem_instruction_from_request(malformed_envelope)
            .expect_err("malformed reserved lineage proof envelope must reject");
        assert!(
            err.contains("failed to decode recursive spend lineage proof envelope"),
            "unexpected malformed-lineage-envelope error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_function_rejects_semantic_profile() {
        ensure_python();
        let request = sample_recursive_spend_redeem_request(42);
        request
            .validate_public_binding()
            .expect("semantic recursive spend redeem request has valid public bindings");
        let archive = norito::to_bytes(&request).expect("encode recursive spend request");
        Python::attach(|py| {
            let err = kagemusha_recursive_spend_redeem_py(py, &archive)
                .expect_err("Python function must reject semantic profile");
            let message = err.to_string();
            assert!(message.contains("invalid Kagemusha recursive spend redeem request"));
            assert!(message.contains("private-hop lineage"));
        });
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_function_rejects_structurally_invalid_lineage() {
        ensure_python();
        let mut request = sample_recursive_spend_redeem_request(42);
        attach_strict_reserved_lineage_envelope(&mut request);
        let mut lineage_record = sample_recursive_spend_lineage_verifier_record();
        lineage_record.max_proof_bytes =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
                as u32;
        request.lineage_verifier_record = Some(lineage_record);
        let archive =
            norito::to_bytes(&request).expect("encode reserved lineage recursive spend request");
        Python::attach(|py| {
            let err = kagemusha_recursive_spend_redeem_py(py, &archive)
                .expect_err("Python function must reject backend-invalid reserved-lineage redeem");
            let message = err.to_string();
            assert!(message.contains("invalid Kagemusha recursive spend redeem request"));
            assert!(
                message.contains("missing verifier-slice public instance columns"),
                "unexpected structurally invalid lineage rejection: {message}"
            );
        });
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_python_function_rejects_amount_mismatch() {
        ensure_python();
        let request = sample_recursive_spend_redeem_request(41);
        let archive =
            norito::to_bytes(&request).expect("encode mismatched recursive spend request");
        Python::attach(|py| {
            let err = kagemusha_recursive_spend_redeem_py(py, &archive)
                .expect_err("Python function must reject amount mismatch");
            let message = err.to_string();
            assert!(message.contains("invalid Kagemusha recursive spend redeem request"));
            assert!(message.contains("public_amount"));
        });
    }

    #[test]
    fn attachments_json_decodes_versioned_signed_transaction() {
        ensure_python();
        let signing = SigningKey::from_bytes(&[0x11u8; 32]);
        let private_key =
            parse_private_key(signing.as_bytes()).expect("ed25519 private key parses");
        let public_key = PublicKey::from(private_key.clone());
        let authority = AccountId::new(public_key.clone())
            .canonical_i105()
            .expect("canonical I105 authority");

        let mut builder =
            TransactionBuilder::new("test-chain", &authority).expect("builder constructs");
        let envelope = builder.sign(signing.as_bytes()).expect("transaction signs");

        let attachments = envelope
            .attachments_json()
            .expect("attachments decode succeeds");
        assert!(attachments.is_none());
    }

    #[test]
    fn python_scoreboard_metadata_records_policy_labels() {
        let metadata = python_scoreboard_metadata(
            3,
            0,
            false,
            42,
            "sdk:python",
            Some("iad-prod"),
            Some(4),
            Some(2),
            Some(3),
            Some(5),
            TransportPolicy::SoranetPreferred,
            Some(TransportPolicy::DirectOnly),
            AnonymityPolicy::GuardPq,
            Some(AnonymityPolicy::StrictPq),
        );
        let map = metadata
            .as_object()
            .expect("scoreboard metadata should be an object");
        assert_eq!(
            map.get("gateway_provider_count")
                .and_then(json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            map.get("gateway_manifest_provided")
                .and_then(json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            map.get("transport_policy").and_then(json::Value::as_str),
            Some("direct-only")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("transport_policy_override_label")
                .and_then(json::Value::as_str),
            Some("direct-only")
        );
        assert_eq!(
            map.get("anonymity_policy").and_then(json::Value::as_str),
            Some("anon-strict-pq")
        );
        assert_eq!(
            map.get("anonymity_policy_override")
                .and_then(json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("telemetry_region").and_then(json::Value::as_str),
            Some("iad-prod")
        );
    }

    #[test]
    fn python_scoreboard_metadata_defaults_soranet_first() {
        let metadata = python_scoreboard_metadata(
            1,
            0,
            false,
            0,
            "sdk:python",
            None,
            None,
            None,
            None,
            None,
            TransportPolicy::SoranetPreferred,
            None,
            AnonymityPolicy::GuardPq,
            None,
        );
        let map = metadata
            .as_object()
            .expect("scoreboard metadata should be an object");
        assert_eq!(
            map.get("transport_policy").and_then(json::Value::as_str),
            Some("soranet-first")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(json::Value::as_bool),
            Some(false)
        );
        assert!(
            map.get("transport_policy_override_label")
                .is_some_and(json::Value::is_null)
        );
        assert_eq!(
            map.get("anonymity_policy").and_then(json::Value::as_str),
            Some("anon-guard-pq")
        );
        assert!(
            map.get("anonymity_policy_override_label")
                .is_some_and(json::Value::is_null)
        );
    }

    #[test]
    fn python_scoreboard_metadata_records_gateway_fields() {
        let metadata = python_scoreboard_metadata(
            0,
            2,
            true,
            0,
            "sdk:python",
            None,
            None,
            None,
            None,
            None,
            TransportPolicy::SoranetPreferred,
            None,
            AnonymityPolicy::GuardPq,
            None,
        );
        let map = metadata
            .as_object()
            .expect("scoreboard metadata should be an object");
        assert_eq!(
            map.get("gateway_provider_count")
                .and_then(json::Value::as_u64),
            Some(2)
        );
        assert_eq!(
            map.get("gateway_manifest_provided")
                .and_then(json::Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn transfer_rwa_instruction_classmethod_serializes_canonical_numeric_payload() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let quantity = pyo3::types::PyString::new(py, "1.2500");
            let source = canonical_i105_from_seed(0x11);
            let destination = canonical_i105_from_seed(0x22);
            let instruction = Instruction::transfer_rwa(
                &instruction_type,
                &source,
                SAMPLE_RWA_ID,
                quantity.as_any(),
                &destination,
            )
            .expect("transfer rwa builds");
            let decoded = json::from_str::<InstructionBox>(&instruction.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let Some(rwa_box) = instruction_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected TransferRwa instruction");
            };
            let iroha_data_model::isi::rwa::RwaInstructionBox::Transfer(transfer) = rwa_box else {
                panic!("expected TransferRwa instruction");
            };

            assert_eq!(
                transfer.source,
                parse_account_id(&source).expect("source parses")
            );
            assert_eq!(
                transfer.destination,
                parse_account_id(&destination).expect("destination parses")
            );
            assert_eq!(transfer.rwa, SAMPLE_RWA_ID.parse().expect("rwa id parses"));
            assert_eq!(
                transfer.quantity,
                Numeric::from_str("1.25").expect("numeric parses")
            );
        });
    }

    #[test]
    fn register_rwa_instruction_classmethod_serializes_payload() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let json_module = py.import("json").expect("json module");
            let payload = json_module
                .call_method1(
                    "loads",
                    (r#"{
                        "domain": "commodities.universal",
                        "quantity": "10.5",
                        "spec": {"scale": 1},
                        "primary_reference": "vault-cert-001",
                        "status": null,
                        "metadata": {"origin": "AE"},
                        "parents": [],
                        "controls": {
                            "controller_accounts": [],
                            "controller_roles": [],
                            "freeze_enabled": true,
                            "hold_enabled": false,
                            "force_transfer_enabled": false,
                            "redeem_enabled": false
                        }
                    }"#,),
                )
                .expect("register payload loads");
            let instruction =
                Instruction::register_rwa(&instruction_type, payload.as_any()).expect("builds");
            let decoded = json::from_str::<InstructionBox>(&instruction.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let Some(rwa_box) = instruction_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected RegisterRwa instruction");
            };
            let iroha_data_model::isi::rwa::RwaInstructionBox::Register(register) = rwa_box else {
                panic!("expected RegisterRwa instruction");
            };

            assert_eq!(
                register.rwa.domain,
                DomainId::try_new("commodities", "universal").expect("domain")
            );
            assert_eq!(
                register.rwa.quantity,
                Numeric::from_str("10.5").expect("quantity")
            );
            assert_eq!(register.rwa.primary_reference, "vault-cert-001");
            assert_eq!(
                register
                    .rwa
                    .metadata
                    .get("origin")
                    .and_then(|value| value.try_into_any_norito::<String>().ok())
                    .as_deref(),
                Some("AE")
            );
            assert!(register.rwa.controls.freeze_enabled);
        });
    }

    #[test]
    fn register_account_instruction_classmethod_is_domainless() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let account_id = canonical_i105_from_seed(0x33);
            let instruction =
                Instruction::register_account(&instruction_type, py, &account_id, None)
                    .expect("register account builds");
            let decoded = json::from_str::<InstructionBox>(&instruction.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let Some(register_box) = instruction_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::RegisterBox>()
            else {
                panic!("expected Register instruction");
            };
            let iroha_data_model::isi::RegisterBox::Account(register) = register_box else {
                panic!("expected Register<Account> instruction");
            };

            assert_eq!(
                register.object.id,
                parse_account_id(&account_id).expect("account parses")
            );
            assert_eq!(register.object.metadata, Metadata::default());
        });
    }

    #[test]
    fn zk_ace_instruction_classmethods_serialize_payloads() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let json_module = py.import("json").expect("json module");
            let verifier = json_module
                .call_method1(
                    "loads",
                    (r#"{"backend":"stark/fri/sha256-goldilocks","name":"zk_ace_pq_authorization_v0"}"#,),
                )
                .expect("verifier loads");
            let proof = json_module
                .call_method1(
                    "loads",
                    (r#"{"backend":"stark/fri/sha256-goldilocks","proof_b64":"cHJvb2Y=","verifying_key_ref":{"backend":"stark/fri/sha256-goldilocks","name":"zk_ace_pq_authorization_v0"}}"#,),
                )
                .expect("proof loads");
            let identity = PyBytes::new(py, &[0x11; 32]);
            let replacement = PyBytes::new(py, &[0x12; 32]);
            let policy = PyBytes::new(py, &[0x22; 32]);
            let tx_digest = PyBytes::new(py, &[0x33; 32]);
            let replay = PyBytes::new(py, &[0x44; 32]);
            let reason = PyBytes::new(py, &[0x55; 32]);
            let source = canonical_i105_from_seed(0x34);
            let destination = canonical_i105_from_seed(0x35);
            let allowed_accounts = PyList::empty(py);
            allowed_accounts
                .append(source.clone())
                .expect("allowed account append");

            let register = Instruction::register_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                identity.as_any(),
                policy.as_any(),
                allowed_accounts.as_any(),
                Some(verifier.as_any()),
                None,
                None,
            )
            .expect("register ZK-ACE identity builds");
            let decoded = json::from_str::<InstructionBox>(&register.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let register = instruction_ref
                .as_any()
                .downcast_ref::<RegisterZkAceIdentityCommitment>()
                .expect("expected RegisterZkAceIdentityCommitment");
            assert_eq!(register.identity_commitment, [0x11; 32]);
            assert_eq!(register.policy_hash, [0x22; 32]);
            assert_eq!(
                register.action_class,
                ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER
            );
            assert_eq!(register.domain_tag, ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG);
            assert_eq!(
                register.verifier_key.backend.to_string(),
                ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND
            );

            let rotate = Instruction::rotate_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                identity.as_any(),
                replacement.as_any(),
                policy.as_any(),
                allowed_accounts.as_any(),
                Some(verifier.as_any()),
                None,
                None,
            )
            .expect("rotate ZK-ACE identity builds");
            let decoded = json::from_str::<InstructionBox>(&rotate.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let rotate = instruction_ref
                .as_any()
                .downcast_ref::<RotateZkAceIdentityCommitment>()
                .expect("expected RotateZkAceIdentityCommitment");
            assert_eq!(rotate.old_identity_commitment, [0x11; 32]);
            assert_eq!(rotate.new_identity_commitment, [0x12; 32]);

            let revoke = Instruction::revoke_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                identity.as_any(),
                Some(reason.as_any()),
            )
            .expect("revoke ZK-ACE identity builds");
            let decoded = json::from_str::<InstructionBox>(&revoke.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let revoke = instruction_ref
                .as_any()
                .downcast_ref::<RevokeZkAceIdentityCommitment>()
                .expect("expected RevokeZkAceIdentityCommitment");
            assert_eq!(revoke.identity_commitment, [0x11; 32]);
            assert_eq!(revoke.reason_hash, Some([0x55; 32]));

            let transfer = Instruction::zk_ace_authorized_transfer(
                &instruction_type,
                &source,
                &destination,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "7",
                identity.as_any(),
                tx_digest.as_any(),
                "chain",
                ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
                ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
                replay.as_any(),
                policy.as_any(),
                proof.as_any(),
            )
            .expect("ZK-ACE transfer builds");
            let decoded = json::from_str::<InstructionBox>(&transfer.to_json().expect("json"))
                .expect("instruction json decodes");
            let instruction_ref: &dyn iroha_data_model::isi::Instruction = &*decoded;
            let transfer = instruction_ref
                .as_any()
                .downcast_ref::<SubmitZkAceAuthorizedTransfer>()
                .expect("expected SubmitZkAceAuthorizedTransfer");
            assert_eq!(transfer.amount, 7);
            assert_eq!(transfer.identity_commitment, [0x11; 32]);
            assert_eq!(transfer.tx_digest, [0x33; 32]);
            assert_eq!(transfer.replay_nullifier, [0x44; 32]);
            assert_eq!(transfer.policy_hash, [0x22; 32]);
            assert_eq!(
                transfer.proof.backend.to_string(),
                ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND
            );
        });
    }

    #[test]
    fn zk_ace_instruction_classmethods_reject_adversarial_inputs() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let json_module = py.import("json").expect("json module");
            let verifier = json_module
                .call_method1(
                    "loads",
                    (r#"{"backend":"stark/fri/sha256-goldilocks","name":"zk_ace_pq_authorization_v0"}"#,),
                )
                .expect("verifier loads");
            let wrong_verifier = json_module
                .call_method1(
                    "loads",
                    (r#"{"backend":"halo2/ipa","name":"zk_ace_pq_authorization_v0"}"#,),
                )
                .expect("wrong verifier loads");
            let wrong_proof = json_module
                .call_method1(
                    "loads",
                    (r#"{"backend":"halo2/ipa","proof_b64":"cHJvb2Y=","verifying_key_ref":{"backend":"halo2/ipa","name":"vk"}}"#,),
                )
                .expect("wrong proof loads");
            let zero = PyBytes::new(py, &[0x00; 32]);
            let identity = PyBytes::new(py, &[0x11; 32]);
            let policy = PyBytes::new(py, &[0x22; 32]);
            let digest = PyBytes::new(py, &[0x33; 32]);
            let replay = PyBytes::new(py, &[0x44; 32]);
            let source = canonical_i105_from_seed(0x36);
            let destination = canonical_i105_from_seed(0x37);
            let allowed_accounts = PyList::empty(py);
            allowed_accounts
                .append(source.clone())
                .expect("allowed account append");

            let err = match Instruction::register_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                zero.as_any(),
                policy.as_any(),
                allowed_accounts.as_any(),
                Some(verifier.as_any()),
                None,
                None,
            ) {
                Ok(_) => panic!("zero identity commitment must fail"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains("identity_commitment"));

            let err = match Instruction::register_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                identity.as_any(),
                policy.as_any(),
                allowed_accounts.as_any(),
                Some(wrong_verifier.as_any()),
                None,
                None,
            ) {
                Ok(_) => panic!("wrong verifier backend must fail"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND));

            let err = match Instruction::rotate_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                identity.as_any(),
                identity.as_any(),
                policy.as_any(),
                allowed_accounts.as_any(),
                Some(verifier.as_any()),
                None,
                None,
            ) {
                Ok(_) => panic!("same replacement commitment must fail"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains("must differ"));

            let err = match Instruction::register_zk_ace_identity_commitment(
                &instruction_type,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                identity.as_any(),
                policy.as_any(),
                allowed_accounts.as_any(),
                Some(verifier.as_any()),
                Some("wrong-action"),
                None,
            ) {
                Ok(_) => panic!("wrong action must fail"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains(ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER));

            let err = match Instruction::zk_ace_authorized_transfer(
                &instruction_type,
                &source,
                &destination,
                "7MBRDd8cGFBZkFGdDMwV7S6FPwbw",
                "1",
                identity.as_any(),
                digest.as_any(),
                "chain",
                ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
                ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
                replay.as_any(),
                policy.as_any(),
                wrong_proof.as_any(),
            ) {
                Ok(_) => panic!("wrong proof backend must fail"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains(ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND));
        });
    }

    #[test]
    fn merge_rwas_and_set_rwa_controls_classmethods_roundtrip_payloads() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let json_module = py.import("json").expect("json module");
            let controller = canonical_i105_from_seed(0x33);
            let merge_payload = json_module
                .call_method1(
                    "loads",
                    (r#"{
                            "parents": [
                                {
                                    "rwa": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities.universal",
                                    "quantity": "1.500"
                                }
                            ],
                            "primary_reference": "blend-cert-007",
                            "status": "blended",
                            "metadata": {"grade": "A"}
                        }"#,),
                )
                .expect("merge payload loads");
            let merge_instruction =
                Instruction::merge_rwas(&instruction_type, merge_payload.as_any())
                    .expect("merge builds");
            let merge_decoded =
                json::from_str::<InstructionBox>(&merge_instruction.to_json().expect("json"))
                    .expect("merge json decodes");
            let merge_ref: &dyn iroha_data_model::isi::Instruction = &*merge_decoded;
            let Some(rwa_box) = merge_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected MergeRwas instruction");
            };
            let iroha_data_model::isi::rwa::RwaInstructionBox::Merge(merge) = rwa_box else {
                panic!("expected MergeRwas instruction");
            };

            assert_eq!(merge.parents.len(), 1);
            assert_eq!(merge.primary_reference, "blend-cert-007");
            assert_eq!(
                merge.status.as_ref().map(ToString::to_string).as_deref(),
                Some("blended")
            );
            assert_eq!(
                merge
                    .metadata
                    .get("grade")
                    .and_then(|value| value.try_into_any_norito::<String>().ok())
                    .as_deref(),
                Some("A")
            );

            let controls_payload = json_module
                .call_method1(
                    "loads",
                    (format!(
                        r#"{{
                            "controller_accounts": ["{controller}"],
                            "controller_roles": [],
                            "freeze_enabled": true,
                            "hold_enabled": true,
                            "force_transfer_enabled": false,
                            "redeem_enabled": true
                        }}"#
                    ),),
                )
                .expect("controls payload loads");
            let controls_instruction = Instruction::set_rwa_controls(
                &instruction_type,
                SAMPLE_RWA_ID,
                controls_payload.as_any(),
            )
            .expect("controls build");
            let controls_decoded =
                json::from_str::<InstructionBox>(&controls_instruction.to_json().expect("json"))
                    .expect("controls json decodes");
            let controls_ref: &dyn iroha_data_model::isi::Instruction = &*controls_decoded;
            let Some(rwa_box) = controls_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected SetRwaControls instruction");
            };
            let iroha_data_model::isi::rwa::RwaInstructionBox::SetControls(controls) = rwa_box
            else {
                panic!("expected SetRwaControls instruction");
            };

            assert_eq!(controls.controls.controller_accounts.len(), 1);
            assert!(controls.controls.freeze_enabled);
            assert!(controls.controls.hold_enabled);
            assert!(controls.controls.redeem_enabled);
        });
    }

    #[test]
    fn rwa_scalar_instruction_classmethods_roundtrip_payloads() {
        ensure_python();
        Python::attach(|py| {
            let instruction_type = py.get_type::<Instruction>();
            let rwa_id = SAMPLE_RWA_ID;
            let destination = canonical_i105_from_seed(0x44);

            let redeem = Instruction::redeem_rwa(
                &instruction_type,
                rwa_id,
                pyo3::types::PyString::new(py, "2.500").as_any(),
            )
            .expect("redeem builds");
            let hold = Instruction::hold_rwa(
                &instruction_type,
                rwa_id,
                pyo3::types::PyString::new(py, "1.2500").as_any(),
            )
            .expect("hold builds");
            let release = Instruction::release_rwa(
                &instruction_type,
                rwa_id,
                pyo3::types::PyString::new(py, "0.500").as_any(),
            )
            .expect("release builds");
            let force = Instruction::force_transfer_rwa(
                &instruction_type,
                rwa_id,
                pyo3::types::PyString::new(py, "4").as_any(),
                &destination,
            )
            .expect("force transfer builds");
            let freeze = Instruction::freeze_rwa(&instruction_type, rwa_id).expect("freeze builds");
            let unfreeze =
                Instruction::unfreeze_rwa(&instruction_type, rwa_id).expect("unfreeze builds");
            let metadata = PyDict::new(py);
            metadata.set_item("origin", "AE").expect("origin");
            metadata.set_item("lot", 3).expect("lot");
            let set_metadata = Instruction::set_rwa_key_value(
                &instruction_type,
                rwa_id,
                "grade",
                Some(metadata.as_any()),
            )
            .expect("set metadata builds");
            let remove_metadata =
                Instruction::remove_rwa_key_value(&instruction_type, rwa_id, "grade")
                    .expect("remove metadata builds");

            let decoded = |instruction: &Instruction| {
                json::from_str::<InstructionBox>(&instruction.to_json().expect("json"))
                    .expect("instruction json decodes")
            };

            let redeem_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&redeem);
            let hold_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&hold);
            let release_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&release);
            let force_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&force);
            let freeze_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&freeze);
            let unfreeze_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&unfreeze);
            let set_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&set_metadata);
            let remove_ref: &dyn iroha_data_model::isi::Instruction = &*decoded(&remove_metadata);

            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::Redeem(redeem_box)) =
                redeem_ref
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected RedeemRwa");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::Hold(hold_box)) = hold_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected HoldRwa");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::Release(release_box)) =
                release_ref
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected ReleaseRwa");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::ForceTransfer(force_box)) =
                force_ref
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected ForceTransferRwa");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::Freeze(_)) = freeze_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected FreezeRwa");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::Unfreeze(_)) = unfreeze_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected UnfreezeRwa");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::SetKeyValue(set_box)) = set_ref
                .as_any()
                .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected SetRwaKeyValue");
            };
            let Some(iroha_data_model::isi::rwa::RwaInstructionBox::RemoveKeyValue(remove_box)) =
                remove_ref
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::rwa::RwaInstructionBox>()
            else {
                panic!("expected RemoveRwaKeyValue");
            };

            assert_eq!(
                redeem_box.quantity,
                Numeric::from_str("2.5").expect("numeric")
            );
            assert_eq!(
                hold_box.quantity,
                Numeric::from_str("1.25").expect("numeric")
            );
            assert_eq!(
                release_box.quantity,
                Numeric::from_str("0.5").expect("numeric")
            );
            assert_eq!(
                force_box.destination,
                parse_account_id(&destination).expect("destination parses")
            );
            assert_eq!(set_box.key.to_string(), "grade");
            assert_eq!(
                set_box
                    .value
                    .try_into_any_norito::<json::Value>()
                    .ok()
                    .and_then(|value| value.as_object().cloned())
                    .and_then(|obj| obj.get("origin").cloned())
                    .and_then(|value| value.as_str().map(|value| value.to_owned()))
                    .as_deref(),
                Some("AE")
            );
            assert_eq!(remove_box.key.to_string(), "grade");
        });
    }

    #[test]
    fn sorafs_gateway_fetch_py_rejects_single_gateway_provider() {
        ensure_python();
        let payload = vec![0x41; 128];
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialise plan");
        let providers = vec![PyGatewayProviderSpec {
            name: "alpha".to_string(),
            provider_id_hex: "55".repeat(32),
            base_url: "https://gateway.test".to_string(),
            stream_token_b64: "dG9rZW4=".to_string(),
            privacy_events_url: None,
        }];

        Python::attach(|py| {
            let result = sorafs_gateway_fetch_py(
                py,
                &"aa".repeat(32),
                "sorafs.sf1@1.0.0",
                &plan_json,
                providers,
                None,
            );
            match result {
                Ok(_) => panic!("single gateway provider must be rejected"),
                Err(err) => assert!(
                    err.to_string()
                        .contains("at least two unique gateway providers"),
                    "{err}"
                ),
            }
        });
    }

    #[test]
    fn sorafs_gateway_fetch_py_streams_payload() {
        ensure_python();
        let payload: Vec<u8> = (0..4096).map(|idx| (idx as u8).wrapping_mul(11)).collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialise plan");

        let mut car_bytes = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("car writer")
            .write_to(&mut car_bytes)
            .expect("car build");
        let root_cid = stats
            .root_cids
            .first()
            .cloned()
            .expect("car must have one root");
        let manifest = ManifestBuilder::new()
            .root_cid(root_cid.clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
            .content_length(plan.content_length)
            .car_digest(*stats.car_archive_digest.as_bytes())
            .car_size(stats.car_size)
            .pin_policy(PinPolicy::default())
            .governance(GovernanceProofs {
                council_signatures: vec![CouncilSignature {
                    signer: [0x11; 32],
                    signature: vec![0x22; 64],
                }],
            })
            .build()
            .expect("manifest");
        let manifest_bytes = manifest.encode().expect("manifest bytes");
        let manifest_digest = manifest.digest().expect("manifest digest");
        let manifest_id_hex = hex::encode(manifest_digest.as_bytes());
        let chunk_profile_handle = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        );
        let provider_id_bytes = [0x55u8; 32];
        let provider_id_hex = hex::encode(provider_id_bytes);
        let second_provider_id_bytes = [0x56u8; 32];
        let second_provider_id_hex = hex::encode(second_provider_id_bytes);

        let chunk_specs = plan.chunk_fetch_specs();
        let server = MockServer::start();
        let manifest_body = {
            let mut obj = json::Map::new();
            obj.insert(
                "manifest_b64".into(),
                json::Value::from(BASE64_STANDARD.encode(manifest_bytes)),
            );
            obj.insert(
                "manifest_digest_hex".into(),
                json::Value::from(manifest_id_hex.clone()),
            );
            obj.insert(
                "payload_digest_hex".into(),
                json::Value::from(hex::encode(plan.payload_digest.as_bytes())),
            );
            obj.insert(
                "content_length".into(),
                json::Value::from(plan.content_length),
            );
            obj.insert(
                "chunk_count".into(),
                json::Value::from(plan.chunks.len() as u64),
            );
            obj.insert(
                "chunk_profile_handle".into(),
                json::Value::from(chunk_profile_handle.clone()),
            );
            json::to_string(&json::Value::Object(obj)).expect("manifest response")
        };
        let manifest_id_clone = manifest_id_hex.clone();
        let manifest_body_clone = manifest_body.clone();
        let manifest_mock = server.mock(move |when, then| {
            when.method(GET)
                .path(format!("/v1/sorafs/storage/manifest/{manifest_id_clone}"));
            then.status(200).body(manifest_body_clone.clone());
        });
        let mut mocks = Vec::with_capacity(chunk_specs.len());
        for spec in &chunk_specs {
            let digest_hex = hex::encode(spec.digest);
            let start = spec.offset as usize;
            let end = start + spec.length as usize;
            let chunk_bytes = payload[start..end].to_vec();
            let manifest_clone = manifest_id_hex.clone();
            let mock = server.mock(move |when, then| {
                when.method(GET).path(format!(
                    "/v1/sorafs/storage/chunk/{manifest_clone}/{digest_hex}"
                ));
                then.status(200).body(chunk_bytes.clone());
            });
            mocks.push(mock);
        }

        let signing = SigningKey::from_bytes(&[0x7Bu8; 32]);
        let make_stream_token = |token_id: &str, provider_id: [u8; 32]| {
            let token_body = StreamTokenBodyV1 {
                token_id: token_id.to_string(),
                manifest_cid: root_cid.clone(),
                provider_id,
                profile_handle: chunk_profile_handle.clone(),
                max_streams: 4,
                ttl_epoch: 1_900_000_000,
                rate_limit_bytes: 32 * 1024 * 1024,
                issued_at: 1_800_000_000,
                requests_per_minute: 180,
                token_pk_version: 1,
            };
            let stream_token =
                StreamTokenV1::sign(token_body, &signing).expect("sign gateway stream token");
            BASE64_STANDARD.encode(to_bytes(&stream_token).expect("token bytes"))
        };
        let stream_token_b64 = make_stream_token("py-gateway-test-alpha", provider_id_bytes);
        let second_stream_token_b64 =
            make_stream_token("py-gateway-test-beta", second_provider_id_bytes);

        let providers = vec![
            PyGatewayProviderSpec {
                name: "alpha".to_string(),
                provider_id_hex: provider_id_hex.clone(),
                base_url: server.base_url(),
                stream_token_b64,
                privacy_events_url: None,
            },
            PyGatewayProviderSpec {
                name: "beta".to_string(),
                provider_id_hex: second_provider_id_hex,
                base_url: server.base_url(),
                stream_token_b64: second_stream_token_b64,
                privacy_events_url: None,
            },
        ];

        Python::attach(|py| {
            let options = PyGatewayFetchOptions {
                telemetry_region: Some("test-region".to_string()),
                scoreboard_telemetry_label: Some("ci-sdk-python".to_string()),
                max_peers: Some(2),
                retry_budget: Some(2),
                local_proxy: Some(PyLocalProxyOptions {
                    proxy_mode: Some("bridge".to_string()),
                    emit_browser_manifest: Some(false),
                    norito_bridge: Some(PyLocalProxyNoritoBridgeOptions {
                        spool_dir: "/tmp/norito-spool".to_string(),
                        extension: Some("norito".to_string()),
                    }),
                    kaigi_bridge: Some(PyLocalProxyKaigiBridgeOptions {
                        spool_dir: "/tmp/kaigi-spool".to_string(),
                        extension: Some("norito".to_string()),
                        room_policy: Some("authenticated".to_string()),
                    }),
                    ..PyLocalProxyOptions::default()
                }),
                ..PyGatewayFetchOptions::default()
            };
            let result = sorafs_gateway_fetch_py(
                py,
                &manifest_id_hex,
                &chunk_profile_handle,
                &plan_json,
                providers,
                Some(options),
            )
            .expect("gateway fetch");

            let dict = result.bind(py);
            let chunk_count_obj = dict
                .get_item("chunk_count")
                .expect("chunk_count lookup failed")
                .expect("chunk_count missing");
            let chunk_count: usize = chunk_count_obj.extract().expect("chunk_count");
            assert_eq!(chunk_count, chunk_specs.len());

            let payload_obj = dict
                .get_item("payload")
                .expect("payload lookup failed")
                .expect("payload missing");
            let payload_value = payload_obj.cast::<PyBytes>().expect("payload bytes");
            assert_eq!(payload_value.as_bytes(), payload.as_slice());

            let reports_obj = dict
                .get_item("provider_reports")
                .expect("provider reports lookup failed")
                .expect("provider reports missing");
            let reports = reports_obj.cast::<PyList>().expect("provider reports");
            assert_eq!(reports.len(), 1);
            let report_obj = reports.get_item(0).expect("provider report entry");
            let report = report_obj.cast::<PyDict>().expect("provider report dict");
            assert_eq!(
                report
                    .get_item("provider")
                    .expect("provider lookup failed")
                    .expect("provider id")
                    .extract::<String>()
                    .expect("provider id"),
                "alpha"
            );
            assert_eq!(
                dict.get_item("local_proxy_mode")
                    .expect("local_proxy_mode lookup failed")
                    .expect("local_proxy_mode")
                    .extract::<String>()
                    .expect("local_proxy_mode"),
                "bridge"
            );
            assert_eq!(
                dict.get_item("local_proxy_norito_spool")
                    .expect("local_proxy_norito_spool lookup failed")
                    .expect("local_proxy_norito_spool")
                    .extract::<String>()
                    .expect("local_proxy_norito_spool"),
                "/tmp/norito-spool"
            );
            assert_eq!(
                dict.get_item("local_proxy_kaigi_spool")
                    .expect("local_proxy_kaigi_spool lookup failed")
                    .expect("local_proxy_kaigi_spool")
                    .extract::<String>()
                    .expect("local_proxy_kaigi_spool"),
                "/tmp/kaigi-spool"
            );
            assert_eq!(
                dict.get_item("local_proxy_kaigi_policy")
                    .expect("local_proxy_kaigi_policy lookup failed")
                    .expect("local_proxy_kaigi_policy")
                    .extract::<String>()
                    .expect("local_proxy_kaigi_policy"),
                "authenticated"
            );

            let receipts_obj = dict
                .get_item("chunk_receipts")
                .expect("chunk receipts lookup failed")
                .expect("chunk receipts missing");
            let receipts = receipts_obj.cast::<PyList>().expect("chunk receipts");
            assert_eq!(receipts.len(), chunk_specs.len());
            let manifest_obj = dict
                .get_item("local_proxy_manifest")
                .expect("local proxy manifest lookup failed")
                .expect("local proxy manifest missing");
            assert!(
                manifest_obj.is_none(),
                "local proxy manifest should default to None when not configured"
            );
            assert_eq!(
                dict.get_item("telemetry_region")
                    .expect("telemetry region lookup failed")
                    .expect("telemetry region")
                    .extract::<String>()
                    .expect("telemetry region"),
                "test-region".to_string()
            );
            let metadata_obj = dict
                .get_item("metadata")
                .expect("metadata lookup failed")
                .expect("metadata missing");
            let metadata = metadata_obj.cast::<PyDict>().expect("metadata dict");
            assert_eq!(
                metadata
                    .get_item("gateway_provider_count")
                    .expect("gateway_provider_count lookup failed")
                    .expect("gateway_provider_count")
                    .extract::<u64>()
                    .expect("gateway count"),
                2
            );
            assert_eq!(
                metadata
                    .get_item("telemetry_region")
                    .expect("metadata telemetry_region lookup failed")
                    .expect("metadata telemetry region")
                    .extract::<String>()
                    .expect("metadata telemetry region"),
                "test-region"
            );
            assert_eq!(
                metadata
                    .get_item("telemetry_source_label")
                    .expect("metadata telemetry label lookup failed")
                    .expect("metadata telemetry label")
                    .extract::<String>()
                    .expect("metadata telemetry label"),
                "ci-sdk-python"
            );
        });

        for mock in mocks {
            mock.assert();
        }
        manifest_mock.assert();
    }

    #[test]
    fn py_taikai_cache_options_convert_to_internal_config() {
        ensure_python();
        let qos = PyTaikaiQosOptions {
            priority_rate_bps: 83_886_080,
            standard_rate_bps: 41_943_040,
            bulk_rate_bps: 12_582_912,
            burst_multiplier: 4,
        };
        let opts = PyTaikaiCacheOptions {
            hot_capacity_bytes: 8_388_608,
            hot_retention_secs: 45,
            warm_capacity_bytes: 33_554_432,
            warm_retention_secs: 180,
            cold_capacity_bytes: 268_435_456,
            cold_retention_secs: 3_600,
            qos,
            reliability: None,
        };
        let config = py_taikai_cache_to_internal(&opts).expect("config parses");
        assert_eq!(config.hot_capacity_bytes, 8_388_608);
        assert_eq!(config.hot_retention.as_secs(), 45);
        assert_eq!(config.qos.burst_multiplier, 4);
    }

    #[test]
    fn py_taikai_cache_options_reject_zero_values() {
        ensure_python();
        let qos = PyTaikaiQosOptions {
            priority_rate_bps: 83_886_080,
            standard_rate_bps: 41_943_040,
            bulk_rate_bps: 12_582_912,
            burst_multiplier: 4,
        };
        let opts = PyTaikaiCacheOptions {
            hot_capacity_bytes: 0,
            hot_retention_secs: 45,
            warm_capacity_bytes: 1,
            warm_retention_secs: 1,
            cold_capacity_bytes: 1,
            cold_retention_secs: 1,
            qos,
            reliability: None,
        };
        let err = py_taikai_cache_to_internal(&opts).expect_err("zero rejected");
        assert!(
            err.to_string()
                .contains("taikai_cache.hot_capacity_bytes must be greater than zero")
        );
    }

    #[test]
    fn parse_time_trigger_kwargs_handles_known_arguments() {
        ensure_python();
        Python::attach(|py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("period_ms", 150_u64).unwrap();
            kwargs.set_item("repeats", 3_u32).unwrap();
            kwargs.set_item("metadata", py.None()).unwrap();
            let parsed = parse_time_trigger_kwargs(Some(&kwargs)).expect("kwargs parse");
            assert_eq!(parsed.period_ms, Some(150));
            assert_eq!(parsed.repeats, Some(3));
            assert!(parsed.metadata.is_some());
        });
    }

    #[test]
    fn parse_time_trigger_kwargs_rejects_unknown_keys() {
        ensure_python();
        Python::attach(|py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("unexpected", 1).unwrap();
            let err = parse_time_trigger_kwargs(Some(&kwargs)).expect_err("expect error");
            assert!(err.is_instance_of::<PyTypeError>(py));
        });
    }

    #[test]
    fn repo_cash_leg_parser_validates_fields() {
        ensure_python();
        Python::attach(|py| {
            let dict = PyDict::new(py);
            dict.set_item("asset_definition_id", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1")
                .unwrap();
            dict.set_item("quantity", "10").unwrap();
            let leg = parse_repo_cash_leg(py, dict.as_any()).expect("repo cash leg should parse");
            assert_eq!(
                leg.asset_definition_id.to_string(),
                "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
            );
            assert_eq!(leg.quantity.to_string(), "10");

            let missing = PyDict::new(py);
            missing
                .set_item("asset_definition_id", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1")
                .unwrap();
            let err =
                parse_repo_cash_leg(py, missing.as_any()).expect_err("missing quantity rejected");
            assert!(err.is_instance_of::<PyValueError>(py));
        });
    }

    #[test]
    fn repo_governance_parser_accepts_mixed_numeric_sources() {
        ensure_python();
        Python::attach(|py| {
            let dict = PyDict::new(py);
            dict.set_item("haircut_bps", "250").unwrap();
            dict.set_item("margin_frequency_secs", 60_u64).unwrap();
            let governance =
                parse_repo_governance(dict.as_any()).expect("repo governance should parse");
            assert_eq!(governance.haircut_bps(), 250);
            assert_eq!(governance.margin_frequency_secs(), 60);
        });
    }

    #[test]
    fn sorafs_multi_fetch_local_executes_plan() {
        ensure_python();
        let tempdir = tempdir().expect("tempdir");
        let payload = (0..(8 * 1024))
            .map(|idx| (idx % 251) as u8)
            .collect::<Vec<_>>();
        let alpha_path = tempdir.path().join("alpha.bin");
        let beta_path = tempdir.path().join("beta.bin");
        fs::write(&alpha_path, &payload).expect("write alpha payload");
        fs::write(&beta_path, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialise plan");

        let providers = vec![
            PyLocalProviderSpec {
                name: "alpha".to_owned(),
                path: alpha_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
            PyLocalProviderSpec {
                name: "beta".to_owned(),
                path: beta_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
        ];

        Python::attach(|py| {
            let result = sorafs_multi_fetch_local_py(
                py,
                &plan_json,
                providers.clone(),
                Some(PyMultiFetchOptions {
                    max_peers: Some(1),
                    ..Default::default()
                }),
            )
            .expect("multi-fetch succeeds");
            let dict = result.bind(py);

            let payload_obj = dict
                .get_item("payload")
                .expect("payload key")
                .expect("payload missing");
            let payload_value = payload_obj.cast::<PyBytes>().expect("payload bytes");
            assert_eq!(payload_value.as_bytes(), payload.as_slice());

            let reports_obj = dict
                .get_item("provider_reports")
                .expect("provider_reports key")
                .expect("provider_reports missing");
            let reports = reports_obj.cast::<PyList>().expect("provider_reports list");
            assert_eq!(reports.len(), 1);
            let report_item = reports.get_item(0).expect("provider report entry");
            let report = report_item.cast::<PyDict>().expect("provider report dict");
            let provider_name: String = report
                .get_item("provider")
                .expect("provider field lookup failed")
                .expect("provider field")
                .extract()
                .expect("provider str");
            assert_eq!(provider_name, "alpha");

            let receipts_obj = dict
                .get_item("chunk_receipts")
                .expect("chunk_receipts key")
                .expect("chunk_receipts missing");
            let receipts = receipts_obj.cast::<PyList>().expect("chunk_receipts list");
            assert_eq!(receipts.len(), plan.chunk_fetch_specs().len());
            assert!(receipts.iter().all(|entry| {
                entry
                    .cast::<PyDict>()
                    .ok()
                    .and_then(|mapping| {
                        mapping
                            .get_item("provider")
                            .ok()
                            .and_then(|opt| opt)
                            .and_then(|value| value.extract::<String>().ok())
                    })
                    .map(|value| value == "alpha")
                    .unwrap_or(false)
            }));
        });
    }

    #[test]
    fn sorafs_multi_fetch_local_returns_scoreboard_and_filters_providers() {
        ensure_python();
        let tempdir = tempdir().expect("tempdir");
        let payload = (0..(4 * 1024))
            .map(|idx| (idx % 191) as u8)
            .collect::<Vec<_>>();
        let alpha_path = tempdir.path().join("alpha.bin");
        let beta_path = tempdir.path().join("beta.bin");
        fs::write(&alpha_path, &payload).expect("write alpha payload");
        fs::write(&beta_path, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialise plan");

        let providers = vec![
            PyLocalProviderSpec {
                name: "alpha".to_owned(),
                path: alpha_path.to_string_lossy().into_owned(),
                max_concurrent: Some(2),
                weight: None,
                metadata: Some(provider_metadata("alpha-id")),
            },
            PyLocalProviderSpec {
                name: "beta".to_owned(),
                path: beta_path.to_string_lossy().into_owned(),
                max_concurrent: Some(2),
                weight: None,
                metadata: Some(provider_metadata("beta-id")),
            },
        ];

        Python::attach(|py| {
            let telemetry = vec![
                PyTelemetryEntry {
                    provider_id: "alpha-id".to_string(),
                    qos_score: Some(95.0),
                    latency_p95_ms: Some(45.0),
                    failure_rate_ewma: Some(0.05),
                    token_health: Some(0.9),
                    staking_weight: Some(1.1),
                    penalty: Some(false),
                    last_updated_unix: Some(u64::MAX / 2),
                },
                PyTelemetryEntry {
                    provider_id: "beta-id".to_string(),
                    qos_score: Some(80.0),
                    latency_p95_ms: Some(120.0),
                    failure_rate_ewma: Some(0.2),
                    token_health: Some(0.6),
                    staking_weight: Some(1.0),
                    penalty: Some(true),
                    last_updated_unix: Some(u64::MAX / 2),
                },
            ];
            let result = sorafs_multi_fetch_local_py(
                py,
                &plan_json,
                providers.clone(),
                Some(PyMultiFetchOptions {
                    use_scoreboard: Some(true),
                    return_scoreboard: Some(true),
                    telemetry: Some(telemetry),
                    ..Default::default()
                }),
            )
            .expect("multi-fetch with scoreboard succeeds");
            let dict = result.bind(py);

            let scoreboard_obj = dict
                .get_item("scoreboard")
                .expect("scoreboard key")
                .expect("scoreboard missing");
            let scoreboard = scoreboard_obj.cast::<PyList>().expect("scoreboard list");
            assert_eq!(scoreboard.len(), 2);

            let alpha_item = scoreboard.get_item(0).expect("alpha scoreboard entry");
            let alpha = alpha_item.cast::<PyDict>().expect("alpha scoreboard entry");
            let alpha_id: String = alpha
                .get_item("provider_id")
                .expect("provider_id lookup failed")
                .expect("provider_id")
                .extract()
                .expect("alpha provider id");
            assert_eq!(alpha_id, "alpha-id");
            let alpha_alias: String = alpha
                .get_item("alias")
                .expect("alias lookup failed")
                .expect("alias")
                .extract()
                .expect("alpha alias");
            assert_eq!(alpha_alias, "alpha");
            let alpha_status: String = alpha
                .get_item("eligibility")
                .expect("eligibility lookup failed")
                .expect("eligibility")
                .extract()
                .expect("alpha eligibility");
            assert_eq!(alpha_status, "eligible");
            let alpha_weight: f64 = alpha
                .get_item("normalized_weight")
                .expect("normalized_weight lookup failed")
                .expect("normalized_weight")
                .extract()
                .expect("alpha weight");
            assert!(alpha_weight > 0.0);

            let beta_item = scoreboard.get_item(1).expect("beta scoreboard entry");
            let beta = beta_item.cast::<PyDict>().expect("beta scoreboard entry");
            let beta_id: String = beta
                .get_item("provider_id")
                .expect("provider_id lookup failed")
                .expect("provider_id")
                .extract()
                .expect("beta provider id");
            assert_eq!(beta_id, "beta-id");
            let beta_status: String = beta
                .get_item("eligibility")
                .expect("eligibility lookup failed")
                .expect("eligibility")
                .extract()
                .expect("beta eligibility");
            assert_eq!(beta_status, "telemetry penalty active");

            let reports_obj = dict
                .get_item("provider_reports")
                .expect("provider_reports")
                .expect("provider_reports missing");
            let reports = reports_obj.cast::<PyList>().expect("provider_reports list");
            assert_eq!(reports.len(), 1);
            let provider_item = reports.get_item(0).expect("provider report entry");
            let provider = provider_item.cast::<PyDict>().expect("provider report");
            let provider_name: String = provider
                .get_item("provider")
                .expect("provider field lookup failed")
                .expect("provider field")
                .extract()
                .expect("provider str");
            assert_eq!(provider_name, "alpha");

            let receipts_obj = dict
                .get_item("chunk_receipts")
                .expect("chunk_receipts")
                .expect("chunk_receipts missing");
            let receipts = receipts_obj.cast::<PyList>().expect("chunk_receipts list");
            assert_eq!(receipts.len(), plan.chunk_fetch_specs().len());
            assert!(receipts.iter().all(|entry| {
                entry
                    .cast::<PyDict>()
                    .ok()
                    .and_then(|mapping| {
                        mapping
                            .get_item("provider")
                            .ok()
                            .and_then(|opt| opt)
                            .and_then(|value| value.extract::<String>().ok())
                    })
                    .map(|value| value == "alpha")
                    .unwrap_or(false)
            }));
        });
    }

    #[test]
    fn sorafs_multi_fetch_local_applies_score_policy() {
        ensure_python();
        let tempdir = tempdir().expect("tempdir");
        let payload = (0..256).map(|idx| (idx % 127) as u8).collect::<Vec<_>>();
        let alpha_path = tempdir.path().join("alpha_policy.bin");
        let beta_path = tempdir.path().join("beta_policy.bin");
        fs::write(&alpha_path, &payload).expect("write alpha payload");
        fs::write(&beta_path, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialise plan");
        let chunk_count = plan.chunk_fetch_specs().len() as u64;

        let providers = vec![
            PyLocalProviderSpec {
                name: "alpha".to_owned(),
                path: alpha_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
            PyLocalProviderSpec {
                name: "beta".to_owned(),
                path: beta_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
        ];

        Python::attach(|py| {
            let denied = sorafs_multi_fetch_local_py(
                py,
                &plan_json,
                providers.clone(),
                Some(PyMultiFetchOptions {
                    deny_providers: Some(vec!["alpha".to_string()]),
                    ..Default::default()
                }),
            )
            .expect("multi-fetch with deny policy succeeds");
            let denied_dict = denied.bind(py);
            let denied_reports_obj = denied_dict
                .get_item("provider_reports")
                .expect("provider_reports")
                .expect("provider_reports missing");
            let denied_reports = denied_reports_obj
                .cast::<PyList>()
                .expect("provider_reports list");
            let mut alpha_successes = None;
            let mut beta_successes = None;
            for report in denied_reports.iter() {
                let report = report.cast::<PyDict>().expect("provider report");
                let provider: String = report
                    .get_item("provider")
                    .expect("provider field lookup failed")
                    .expect("provider field")
                    .extract()
                    .expect("provider str");
                let successes: u64 = report
                    .get_item("successes")
                    .expect("successes field lookup failed")
                    .expect("successes field")
                    .extract()
                    .expect("successes count");
                match provider.as_str() {
                    "alpha" => alpha_successes = Some(successes),
                    "beta" => beta_successes = Some(successes),
                    _ => {}
                }
            }
            assert_eq!(alpha_successes.unwrap_or_default(), 0);
            assert_eq!(beta_successes, Some(chunk_count));

            let denied_receipts_obj = denied_dict
                .get_item("chunk_receipts")
                .expect("chunk_receipts")
                .expect("chunk_receipts missing");
            let denied_receipts = denied_receipts_obj
                .cast::<PyList>()
                .expect("chunk_receipts list");
            assert!(denied_receipts.iter().all(|entry| {
                entry
                    .cast::<PyDict>()
                    .ok()
                    .and_then(|mapping| {
                        mapping
                            .get_item("provider")
                            .ok()
                            .and_then(|opt| opt)
                            .and_then(|value| value.extract::<String>().ok())
                    })
                    .map(|value| value == "beta")
                    .unwrap_or(false)
            }));

            let boosted = sorafs_multi_fetch_local_py(
                py,
                &plan_json,
                providers,
                Some(PyMultiFetchOptions {
                    boost_providers: Some(vec![PyProviderBoost {
                        provider: "beta".to_string(),
                        delta: 50,
                    }]),
                    ..Default::default()
                }),
            )
            .expect("multi-fetch with boost policy succeeds");
            let boosted_dict = boosted.bind(py);
            let boosted_receipts_obj = boosted_dict
                .get_item("chunk_receipts")
                .expect("chunk_receipts")
                .expect("chunk_receipts missing");
            let boosted_receipts = boosted_receipts_obj
                .cast::<PyList>()
                .expect("chunk_receipts list");
            assert!(boosted_receipts.iter().all(|entry| {
                entry
                    .cast::<PyDict>()
                    .ok()
                    .and_then(|mapping| {
                        mapping
                            .get_item("provider")
                            .ok()
                            .and_then(|opt| opt)
                            .and_then(|value| value.extract::<String>().ok())
                    })
                    .map(|value| value == "beta")
                    .unwrap_or(false)
            }));
        });
    }

    fn expect_poseidon2(a: u64, b: u64, gpu: Option<u64>) {
        let expected = ivm::poseidon2(a, b);
        match gpu {
            Some(value) => assert_eq!(value, expected),
            None => assert!(!super::cuda_available_py() || super::cuda_disabled_py()),
        }
    }

    fn expect_poseidon6(inputs: [u64; 6], gpu: Option<u64>) {
        let expected = ivm::poseidon6(inputs);
        match gpu {
            Some(value) => assert_eq!(value, expected),
            None => assert!(!super::cuda_available_py() || super::cuda_disabled_py()),
        }
    }

    fn expect_bn254<F>(a: [u64; 4], b: [u64; 4], gpu: Option<[u64; 4]>, reference_impl: F)
    where
        F: Fn(FieldElem, FieldElem) -> FieldElem,
    {
        let expected = reference_impl(FieldElem(a), FieldElem(b)).0;
        match gpu {
            Some(value) => assert_eq!(value, expected),
            None => assert!(!super::cuda_available_py() || super::cuda_disabled_py()),
        }
    }

    fn expect_bn254_many<F>(
        lhs: &[[u64; 4]],
        rhs: &[[u64; 4]],
        gpu: Option<Vec<[u64; 4]>>,
        reference_impl: F,
    ) where
        F: Fn(FieldElem, FieldElem) -> FieldElem,
    {
        let expected: Vec<[u64; 4]> = lhs
            .iter()
            .copied()
            .zip(rhs.iter().copied())
            .map(|(a, b)| reference_impl(FieldElem(a), FieldElem(b)).0)
            .collect();
        match gpu {
            Some(value) => assert_eq!(value, expected),
            None => assert!(!super::cuda_available_py() || super::cuda_disabled_py()),
        }
    }

    #[test]
    fn cuda_probes_reflect_ivm_state() {
        assert_eq!(super::cuda_available_py(), ivm::cuda_available());
        assert_eq!(super::cuda_disabled_py(), ivm::cuda_disabled());
    }

    #[test]
    fn poseidon2_wrapper_matches_cpu() {
        expect_poseidon2(1, 2, super::poseidon2_cuda_py(1, 2));
    }

    #[test]
    fn poseidon6_wrapper_matches_cpu() {
        expect_poseidon6(
            [1, 2, 3, 4, 5, 6],
            super::poseidon6_cuda_py([1, 2, 3, 4, 5, 6]),
        );
    }

    #[test]
    fn bn254_add_wrapper_matches_cpu() {
        let a = [1, 0, 0, 0];
        let b = [2, 0, 0, 0];
        expect_bn254(a, b, super::bn254_add_cuda_py(a, b), bn254_vec::add);
    }

    #[test]
    fn bn254_sub_wrapper_matches_cpu() {
        let a = [5, 0, 0, 0];
        let b = [3, 0, 0, 0];
        expect_bn254(a, b, super::bn254_sub_cuda_py(a, b), bn254_vec::sub);
    }

    #[test]
    fn bn254_mul_wrapper_matches_cpu() {
        let a = [7, 0, 0, 0];
        let b = [11, 0, 0, 0];
        expect_bn254(a, b, super::bn254_mul_cuda_py(a, b), bn254_vec::mul);
    }

    #[test]
    fn bn254_add_many_wrapper_matches_cpu() {
        let lhs = vec![[1, 0, 0, 0], [2, 0, 0, 0], [9, 0, 0, 0]];
        let rhs = vec![[2, 0, 0, 0], [3, 0, 0, 0], [4, 0, 0, 0]];
        expect_bn254_many(
            &lhs,
            &rhs,
            super::bn254_add_cuda_many_py(lhs.clone(), rhs.clone()),
            bn254_vec::add_scalar,
        );
        let empty = super::bn254_add_cuda_many_py(Vec::new(), Vec::new());
        if super::cuda_available_py() && !super::cuda_disabled_py() {
            assert_eq!(empty, Some(Vec::new()));
        } else {
            assert!(empty.is_none());
        }
    }

    #[test]
    fn bn254_sub_many_wrapper_matches_cpu() {
        let lhs = vec![[5, 0, 0, 0], [8, 0, 0, 0], [13, 0, 0, 0]];
        let rhs = vec![[3, 0, 0, 0], [2, 0, 0, 0], [6, 0, 0, 0]];
        expect_bn254_many(
            &lhs,
            &rhs,
            super::bn254_sub_cuda_many_py(lhs.clone(), rhs.clone()),
            bn254_vec::sub_scalar,
        );
    }

    #[test]
    fn bn254_mul_many_wrapper_matches_cpu() {
        let lhs = vec![[7, 0, 0, 0], [11, 0, 0, 0], [5, 0, 0, 0]];
        let rhs = vec![[11, 0, 0, 0], [7, 0, 0, 0], [9, 0, 0, 0]];
        expect_bn254_many(
            &lhs,
            &rhs,
            super::bn254_mul_cuda_many_py(lhs.clone(), rhs.clone()),
            bn254_vec::mul_scalar,
        );
    }

    #[test]
    fn attempt_failure_payload_renders_policy_block() {
        ensure_python();
        let evidence = PolicyBlockEvidence {
            observed_status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            canonical_status: StatusCode::FORBIDDEN,
            code: Some("denylisted".to_owned()),
            cache_version: Some("frozen".to_owned()),
            denylist_version: None,
            proof_token_present: true,
            message: Some("blocked".to_owned()),
        };

        Python::attach(|py| {
            let payload = attempt_failure_payload(
                py,
                AttemptFailure::Provider {
                    message: "blocked".to_owned(),
                    policy_block: Some(evidence.clone()),
                },
            )
            .expect("payload");
            let payload = payload.bind(py);
            let policy = payload
                .get_item("policy_block")
                .expect("policy block entry")
                .expect("policy block entry");
            let policy = policy.cast::<PyDict>().expect("policy dict");

            assert_eq!(
                policy
                    .get_item("observed_status")
                    .expect("observed")
                    .expect("observed")
                    .extract::<u16>()
                    .expect("status code"),
                evidence.observed_status.as_u16()
            );
            assert_eq!(
                policy
                    .get_item("canonical_status")
                    .expect("canonical")
                    .expect("canonical")
                    .extract::<u16>()
                    .expect("status code"),
                evidence.canonical_status.as_u16()
            );
            assert_eq!(
                policy
                    .get_item("code")
                    .expect("code")
                    .expect("code")
                    .extract::<String>()
                    .expect("code"),
                "denylisted"
            );
            assert_eq!(
                policy
                    .get_item("cache_version")
                    .expect("cache_version")
                    .expect("cache_version")
                    .extract::<String>()
                    .expect("cache_version"),
                "frozen"
            );
            assert!(
                policy
                    .get_item("denylist_version")
                    .expect("denylist version lookup")
                    .is_none()
            );
            assert_eq!(
                policy
                    .get_item("proof_token_present")
                    .expect("proof token")
                    .expect("proof token")
                    .extract::<bool>()
                    .expect("bool"),
                evidence.proof_token_present
            );
            assert_eq!(
                policy
                    .get_item("message")
                    .expect("message")
                    .expect("message")
                    .extract::<String>()
                    .expect("message"),
                "blocked"
            );
        });
    }

    #[test]
    fn taikai_cache_payload_helpers_render_counts() {
        ensure_python();
        let mut evictions = EvictionStats::default();
        evictions.hot.expired = 1;
        evictions.hot.capacity = 2;
        evictions.warm.expired = 3;
        evictions.warm.capacity = 4;
        evictions.cold.expired = 5;
        evictions.cold.capacity = 6;

        let stats = TaikaiCacheStatsSnapshot {
            hits: TierStats {
                hot: 7,
                warm: 8,
                cold: 9,
            },
            misses: 10,
            inserts: TierStats {
                hot: 11,
                warm: 12,
                cold: 13,
            },
            evictions,
            promotions: PromotionStats {
                warm_to_hot: 14,
                cold_to_warm: 15,
                cold_to_hot: 16,
            },
            qos_denials: QosStats {
                priority: 17,
                standard: 18,
                bulk: 19,
            },
        };

        Python::attach(|py| {
            let summary = taikai_cache_stats_payload(py, stats).expect("payload");
            let summary = summary.bind(py);
            let hits = summary
                .get_item("hits")
                .expect("hits entry")
                .expect("hits entry");
            let hits = hits.cast::<PyDict>().expect("dict");
            assert_eq!(
                hits.get_item("hot")
                    .expect("hot count")
                    .expect("hot count")
                    .extract::<u64>()
                    .expect("u64"),
                7
            );
            let qos = summary.get_item("qos_denials").expect("qos").expect("qos");
            let qos = qos.cast::<PyDict>().expect("dict");
            assert_eq!(
                qos.get_item("standard")
                    .expect("standard")
                    .expect("standard")
                    .extract::<u64>()
                    .expect("u64"),
                18
            );
        });
    }

    #[test]
    fn taikai_queue_payload_helpers_render_counts() {
        ensure_python();
        let queue = TaikaiPullQueueStats {
            pending_segments: 2,
            pending_bytes: 3,
            pending_batches: 4,
            in_flight_batches: 5,
            hedged_batches: 6,
            shaper_denials: QosStats {
                priority: 1,
                standard: 2,
                bulk: 3,
            },
            dropped_segments: 7,
            failovers: 8,
            open_circuits: 9,
        };

        Python::attach(|py| {
            let payload = taikai_queue_stats_payload(py, queue).expect("payload");
            let payload = payload.bind(py);
            assert_eq!(
                payload
                    .get_item("hedged_batches")
                    .expect("hedged")
                    .expect("hedged")
                    .extract::<u64>()
                    .expect("u64"),
                6
            );
            let shaper = payload
                .get_item("shaper_denials")
                .expect("shaper")
                .expect("shaper");
            let shaper = shaper.cast::<PyDict>().expect("dict");
            assert_eq!(
                shaper
                    .get_item("bulk")
                    .expect("bulk")
                    .expect("bulk")
                    .extract::<u64>()
                    .expect("u64"),
                3
            );
        });
    }
}

fn py_to_metadata(py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<Metadata> {
    match value {
        None => Ok(Metadata::default()),
        Some(obj) => {
            let dumped = py_to_json_string(py, obj, "metadata")?;
            json::from_str::<Metadata>(&dumped)
                .map_err(|err| PyValueError::new_err(format!("invalid metadata value: {err}")))
        }
    }
}

fn py_to_json_string(py: Python<'_>, value: &Bound<'_, PyAny>, context: &str) -> PyResult<String> {
    let json_module = py
        .import("json")
        .map_err(|err| PyValueError::new_err(format!("failed to import json module: {err}")))?;
    let dumped = json_module.call_method1("dumps", (value,)).map_err(|err| {
        PyValueError::new_err(format!("{context} must be JSON serializable: {err}"))
    })?;
    dumped
        .extract()
        .map_err(|err| PyValueError::new_err(format!("expected JSON string: {err}")))
}

fn py_to_json_model<T>(py: Python<'_>, value: &Bound<'_, PyAny>, context: &str) -> PyResult<T>
where
    T: norito::json::JsonDeserialize,
{
    let dumped = py_to_json_string(py, value, context)?;
    json::from_str::<T>(&dumped)
        .map_err(|err| PyValueError::new_err(format!("invalid {context} value: {err}")))
}

fn json_required_value(
    fields: &mut std::collections::BTreeMap<String, json::Value>,
    key: &str,
    context: &str,
) -> PyResult<json::Value> {
    fields
        .remove(key)
        .ok_or_else(|| PyValueError::new_err(format!("{context}.{key} is required")))
}

fn json_string_value(value: json::Value, context: &str) -> PyResult<String> {
    match value {
        json::Value::String(value) => Ok(value),
        _ => Err(PyValueError::new_err(format!("{context} must be a string"))),
    }
}

fn json_rwa_id_value(value: json::Value, context: &str) -> PyResult<RwaId> {
    let literal = json_string_value(value, context)?;
    literal
        .parse()
        .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{literal}`: {err}")))
}

fn json_rwa_parent_refs_value(value: json::Value, context: &str) -> PyResult<Vec<RwaParentRef>> {
    let json::Value::Array(entries) = value else {
        return Err(PyValueError::new_err(format!("{context} must be an array")));
    };

    let mut parents = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let entry_context = format!("{context}[{index}]");
        let json::Value::Object(mut fields) = entry else {
            return Err(PyValueError::new_err(format!(
                "{entry_context} must be an object"
            )));
        };
        let rwa = json_rwa_id_value(
            json_required_value(&mut fields, "rwa", &entry_context)?,
            &format!("{entry_context}.rwa"),
        )?;
        let quantity = json::from_value::<Numeric>(json_required_value(
            &mut fields,
            "quantity",
            &entry_context,
        )?)
        .map_err(|err| {
            PyValueError::new_err(format!("invalid {entry_context}.quantity value: {err}"))
        })?;
        parents.push(RwaParentRef::new(rwa, quantity));
    }
    Ok(parents)
}

fn parse_new_rwa_payload(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<NewRwa> {
    let dumped = py_to_json_string(py, value, "rwa")?;
    let json::Value::Object(mut fields) = json::from_str::<json::Value>(&dumped)
        .map_err(|err| PyValueError::new_err(format!("invalid rwa value: {err}")))?
    else {
        return Err(PyValueError::new_err("rwa must be a JSON object"));
    };

    let domain =
        json::from_value::<DomainId>(json_required_value(&mut fields, "domain", "rwa")?)
            .map_err(|err| PyValueError::new_err(format!("invalid rwa.domain value: {err}")))?;
    let quantity =
        json::from_value::<Numeric>(json_required_value(&mut fields, "quantity", "rwa")?)
            .map_err(|err| PyValueError::new_err(format!("invalid rwa.quantity value: {err}")))?;
    let spec = json::from_value::<NumericSpec>(json_required_value(&mut fields, "spec", "rwa")?)
        .map_err(|err| PyValueError::new_err(format!("invalid rwa.spec value: {err}")))?;
    let primary_reference = json_string_value(
        json_required_value(&mut fields, "primary_reference", "rwa")?,
        "rwa.primary_reference",
    )?;
    let status = fields
        .remove("status")
        .map_or(Ok(None), |value| match value {
            json::Value::Null => Ok(None),
            other => json::from_value(other)
                .map(Some)
                .map_err(|err| PyValueError::new_err(format!("invalid rwa.status value: {err}"))),
        })?;
    let metadata = fields
        .remove("metadata")
        .map_or(Ok(Metadata::default()), |value| {
            json::from_value(value)
                .map_err(|err| PyValueError::new_err(format!("invalid rwa.metadata value: {err}")))
        })?;
    let parents = fields.remove("parents").map_or(Ok(Vec::new()), |value| {
        json_rwa_parent_refs_value(value, "rwa.parents")
    })?;
    let controls = fields
        .remove("controls")
        .map_or(Ok(RwaControlPolicy::default()), |value| {
            json::from_value(value)
                .map_err(|err| PyValueError::new_err(format!("invalid rwa.controls value: {err}")))
        })?;

    Ok(NewRwa::new(
        domain,
        quantity,
        spec,
        primary_reference,
        status,
        metadata,
        parents,
        controls,
    ))
}

fn parse_merge_rwas_payload(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<iroha_data_model::isi::rwa::MergeRwas> {
    let dumped = py_to_json_string(py, value, "merge")?;
    let json::Value::Object(mut fields) = json::from_str::<json::Value>(&dumped)
        .map_err(|err| PyValueError::new_err(format!("invalid merge value: {err}")))?
    else {
        return Err(PyValueError::new_err("merge must be a JSON object"));
    };

    let parents = json_rwa_parent_refs_value(
        json_required_value(&mut fields, "parents", "merge")?,
        "merge.parents",
    )?;
    let primary_reference = json_string_value(
        json_required_value(&mut fields, "primary_reference", "merge")?,
        "merge.primary_reference",
    )?;
    let status = fields
        .remove("status")
        .map_or(Ok(None), |value| match value {
            json::Value::Null => Ok(None),
            other => json::from_value(other)
                .map(Some)
                .map_err(|err| PyValueError::new_err(format!("invalid merge.status value: {err}"))),
        })?;
    let metadata = fields
        .remove("metadata")
        .map_or(Ok(Metadata::default()), |value| {
            json::from_value(value).map_err(|err| {
                PyValueError::new_err(format!("invalid merge.metadata value: {err}"))
            })
        })?;

    Ok(iroha_data_model::isi::rwa::MergeRwas {
        parents,
        primary_reference,
        status,
        metadata,
    })
}

fn py_to_json_value(py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<Json> {
    match value {
        None => Ok(Json::default()),
        Some(obj) => {
            let dumped = py_to_json_string(py, obj, "trigger arguments")?;
            Json::from_str_norito(&dumped)
                .map_err(|err| PyValueError::new_err(format!("invalid JSON payload: {err}")))
        }
    }
}

fn parse_numeric(quantity: &str) -> PyResult<Numeric> {
    Numeric::from_str(quantity)
        .map(Numeric::trim_trailing_zeros)
        .map_err(|err| {
            PyValueError::new_err(format!("invalid numeric quantity `{quantity}`: {err}"))
        })
}

fn numeric_from_py(value: &Bound<'_, PyAny>) -> PyResult<Numeric> {
    if let Ok(s) = value.extract::<String>() {
        return parse_numeric(&s);
    }
    if let Ok(i) = value.extract::<i128>() {
        return parse_numeric(&i.to_string());
    }
    if let Ok(u) = value.extract::<u128>() {
        return parse_numeric(&u.to_string());
    }
    if let Ok(f) = value.extract::<f64>() {
        return parse_numeric(&f.to_string());
    }
    let py_str = value
        .str()
        .map_err(|err| PyValueError::new_err(format!("invalid numeric value: {err}")))?;
    let s = py_str
        .to_cow()
        .map_err(|err| PyValueError::new_err(format!("invalid numeric value: {err}")))?;
    parse_numeric(&s)
}

fn parse_repo_cash_leg(_py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<RepoCashLeg> {
    let dict = value.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err("cash_leg must be a mapping with asset_definition_id/quantity fields")
    })?;
    let asset_obj = dict
        .get_item("asset_definition_id")?
        .ok_or_else(|| PyValueError::new_err("cash_leg requires `asset_definition_id`"))?;
    let asset_str: String = asset_obj
        .extract()
        .map_err(|_| PyValueError::new_err("cash_leg `asset_definition_id` must be a string"))?;
    let asset_definition_id = asset_str.parse().map_err(|err| {
        PyValueError::new_err(format!(
            "invalid cash_leg asset definition `{asset_str}`: {err}"
        ))
    })?;
    let quantity_obj = dict
        .get_item("quantity")?
        .ok_or_else(|| PyValueError::new_err("cash_leg requires `quantity`"))?;
    let quantity = numeric_from_py(&quantity_obj)?;
    Ok(RepoCashLeg {
        asset_definition_id,
        quantity,
    })
}

fn parse_repo_collateral_leg(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<RepoCollateralLeg> {
    let dict = value.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err(
            "collateral_leg must be a mapping with asset_definition_id/quantity fields",
        )
    })?;
    let asset_obj = dict
        .get_item("asset_definition_id")?
        .ok_or_else(|| PyValueError::new_err("collateral_leg requires `asset_definition_id`"))?;
    let asset_str: String = asset_obj.extract().map_err(|_| {
        PyValueError::new_err("collateral_leg `asset_definition_id` must be a string")
    })?;
    let asset_definition_id = asset_str.parse().map_err(|err| {
        PyValueError::new_err(format!(
            "invalid collateral asset definition `{asset_str}`: {err}"
        ))
    })?;
    let quantity_obj = dict
        .get_item("quantity")?
        .ok_or_else(|| PyValueError::new_err("collateral_leg requires `quantity`"))?;
    let quantity = numeric_from_py(&quantity_obj)?;
    let metadata = match dict.get_item("metadata")? {
        Some(meta) => py_to_metadata(py, Some(&meta))?,
        None => Metadata::default(),
    };
    Ok(RepoCollateralLeg {
        asset_definition_id,
        quantity,
        metadata,
    })
}

fn parse_settlement_leg(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<SettlementLeg> {
    let dict = value.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err(format!(
            "{name} must be a mapping with asset_definition_id, quantity, from, and to fields"
        ))
    })?;

    let asset_obj = dict
        .get_item("asset_definition_id")?
        .ok_or_else(|| PyValueError::new_err(format!("{name} requires `asset_definition_id`")))?;
    let asset_str: String = asset_obj.extract().map_err(|_| {
        PyValueError::new_err(format!("{name} `asset_definition_id` must be a string"))
    })?;
    let asset_definition_id = asset_str.parse().map_err(|err| {
        PyValueError::new_err(format!(
            "invalid {name} asset definition `{asset_str}`: {err}"
        ))
    })?;

    let quantity_obj = dict
        .get_item("quantity")?
        .ok_or_else(|| PyValueError::new_err(format!("{name} requires `quantity`")))?;
    let quantity = numeric_from_py(&quantity_obj)?;

    let from_obj = dict
        .get_item("from")?
        .ok_or_else(|| PyValueError::new_err(format!("{name} requires `from`")))?;
    let from_str: String = from_obj
        .extract()
        .map_err(|_| PyValueError::new_err(format!("{name} `from` must be a string")))?;
    let from_account = parse_account_id(&from_str).map_err(|err| {
        PyValueError::new_err(format!("invalid {name} `from` account `{from_str}`: {err}"))
    })?;
    ensure_ed25519_account(&from_account)?;

    let to_obj = dict
        .get_item("to")?
        .ok_or_else(|| PyValueError::new_err(format!("{name} requires `to`")))?;
    let to_str: String = to_obj
        .extract()
        .map_err(|_| PyValueError::new_err(format!("{name} `to` must be a string")))?;
    let to_account = parse_account_id(&to_str).map_err(|err| {
        PyValueError::new_err(format!("invalid {name} `to` account `{to_str}`: {err}"))
    })?;
    ensure_ed25519_account(&to_account)?;

    let metadata = match dict.get_item("metadata")? {
        Some(meta) => py_to_metadata(py, Some(&meta))?,
        None => Metadata::default(),
    };

    Ok(SettlementLeg {
        asset_definition_id,
        quantity,
        from: from_account,
        to: to_account,
        metadata,
    })
}

fn parse_settlement_order(value: &str) -> PyResult<SettlementExecutionOrder> {
    let normalized = value.replace('-', "_").to_ascii_lowercase();
    match normalized.as_str() {
        "delivery_then_payment" => Ok(SettlementExecutionOrder::DeliveryThenPayment),
        "payment_then_delivery" => Ok(SettlementExecutionOrder::PaymentThenDelivery),
        _ => Err(PyValueError::new_err(format!(
            "unknown settlement order `{value}` (expected `delivery_then_payment` or `payment_then_delivery`)"
        ))),
    }
}

fn parse_settlement_atomicity(value: &str) -> PyResult<SettlementAtomicity> {
    let normalized = value.replace('-', "_").to_ascii_lowercase();
    match normalized.as_str() {
        "all_or_nothing" => Ok(SettlementAtomicity::AllOrNothing),
        "commit_first_leg" => Err(PyValueError::new_err(
            "atomicity `commit_first_leg` is not supported yet; choose `all_or_nothing`",
        )),
        "commit_second_leg" => Err(PyValueError::new_err(
            "atomicity `commit_second_leg` is not supported yet; choose `all_or_nothing`",
        )),
        _ => Err(PyValueError::new_err(format!(
            "unknown settlement atomicity `{value}` (expected `all_or_nothing`)"
        ))),
    }
}

fn parse_u16_field(value: &Bound<'_, PyAny>, field: &str) -> PyResult<u16> {
    if let Ok(v) = value.extract::<u16>() {
        return Ok(v);
    }
    if let Ok(s) = value.extract::<String>() {
        return s
            .parse::<u16>()
            .map_err(|err| PyValueError::new_err(format!("invalid `{field}` value `{s}`: {err}")));
    }
    Err(PyValueError::new_err(format!(
        "`{field}` must be an unsigned 16-bit integer"
    )))
}

fn parse_u64_field(value: &Bound<'_, PyAny>, field: &str) -> PyResult<u64> {
    if let Ok(v) = value.extract::<u64>() {
        return Ok(v);
    }
    if let Ok(s) = value.extract::<String>() {
        return s
            .parse::<u64>()
            .map_err(|err| PyValueError::new_err(format!("invalid `{field}` value `{s}`: {err}")));
    }
    Err(PyValueError::new_err(format!(
        "`{field}` must be an unsigned 64-bit integer"
    )))
}

fn parse_repo_governance(value: &Bound<'_, PyAny>) -> PyResult<RepoGovernance> {
    let dict = value.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err(
            "governance must be a mapping with `haircut_bps` and `margin_frequency_secs` fields",
        )
    })?;
    let haircut_obj = dict
        .get_item("haircut_bps")?
        .ok_or_else(|| PyValueError::new_err("governance requires `haircut_bps`"))?;
    let margin_obj = dict
        .get_item("margin_frequency_secs")?
        .ok_or_else(|| PyValueError::new_err("governance requires `margin_frequency_secs`"))?;
    let haircut_bps = parse_u16_field(&haircut_obj, "haircut_bps")?;
    let margin_frequency_secs = parse_u64_field(&margin_obj, "margin_frequency_secs")?;
    Ok(RepoGovernance::with_defaults(
        haircut_bps,
        margin_frequency_secs,
    ))
}

fn parse_mintable(mode: Option<&str>) -> PyResult<Mintable> {
    let label = mode.unwrap_or("Infinitely");
    match label {
        "Infinitely" => Ok(Mintable::Infinitely),
        "Once" => Ok(Mintable::Once),
        "Not" => Ok(Mintable::Not),
        other if other.starts_with("Limited(") && other.ends_with(')') => {
            let inner = &other["Limited(".len()..other.len() - 1];
            let tokens = inner.parse::<u32>().map_err(|_| {
                PyValueError::new_err(format!("invalid Limited token count `{inner}`"))
            })?;
            Mintable::limited_from_u32(tokens).map_err(|err| PyValueError::new_err(err.to_string()))
        }
        other => Err(PyValueError::new_err(format!(
            "invalid mintable value `{other}`; expected Infinitely/Once/Not/Limited(n)"
        ))),
    }
}

fn parse_confidential_policy(mode: Option<&str>) -> PyResult<Option<AssetConfidentialPolicy>> {
    let Some(mode) = mode else {
        return Ok(None);
    };
    let policy = match mode {
        "TransparentOnly" => AssetConfidentialPolicy::transparent(),
        "ShieldedOnly" => AssetConfidentialPolicy::shielded_only(),
        "Convertible" => AssetConfidentialPolicy::convertible(),
        other => {
            return Err(PyValueError::new_err(format!(
                "invalid confidential policy `{other}`; expected TransparentOnly/ShieldedOnly/Convertible"
            )));
        }
    };
    Ok(Some(policy))
}

fn parse_balance_scope_policy(mode: Option<&str>) -> PyResult<Option<AssetBalancePolicy>> {
    let Some(mode) = mode else {
        return Ok(None);
    };
    let policy = match mode {
        "Global" => AssetBalancePolicy::Global,
        "DataspaceRestricted" => AssetBalancePolicy::DataspaceRestricted,
        other => {
            return Err(PyValueError::new_err(format!(
                "invalid balance scope policy `{other}`; expected Global/DataspaceRestricted"
            )));
        }
    };
    Ok(Some(policy))
}

fn domain_id_to_py(py: Python<'_>, id: &DomainId) -> PyResult<Py<PyDomainId>> {
    Py::new(py, PyDomainId { inner: id.clone() })
}

fn account_id_to_py(py: Python<'_>, id: &AccountId) -> PyResult<Py<PyAccountId>> {
    Py::new(py, PyAccountId { inner: id.clone() })
}

fn asset_definition_id_to_py(
    py: Python<'_>,
    id: &AssetDefinitionId,
) -> PyResult<Py<PyAssetDefinitionId>> {
    Py::new(py, PyAssetDefinitionId { inner: id.clone() })
}

#[pyclass(name = "DomainId", module = "iroha_python._crypto")]
#[derive(Clone)]
struct PyDomainId {
    inner: DomainId,
}

#[pymethods]
impl PyDomainId {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        let inner = DomainId::parse_fully_qualified(value)
            .map_err(|err| PyValueError::new_err(format!("invalid domain id `{value}`: {err}")))?;
        Ok(Self { inner })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __str__(&self) -> String {
        self.value()
    }

    fn __repr__(&self) -> String {
        format!("DomainId('{}')", self.value())
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

#[pyclass(name = "AccountId", module = "iroha_python._crypto")]
#[derive(Clone)]
struct PyAccountId {
    inner: AccountId,
}

#[pymethods]
impl PyAccountId {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        let id = parse_account_id(value)?;
        ensure_ed25519_account(&id)?;
        Ok(Self { inner: id })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    #[getter]
    fn public_key_hex(&self) -> PyResult<String> {
        let (algorithm, bytes) =
            public_key_to_bytes(self.inner.signatory(), "account signatory public key")?;
        algorithm_guard(algorithm)?;
        Ok(hex::encode(bytes))
    }

    fn __str__(&self) -> String {
        self.value()
    }

    fn __repr__(&self) -> String {
        format!("AccountId('{}')", self.value())
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

#[pyclass(name = "AssetDefinitionId", module = "iroha_python._crypto")]
#[derive(Clone)]
struct PyAssetDefinitionId {
    inner: AssetDefinitionId,
}

#[pymethods]
impl PyAssetDefinitionId {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        let inner = value.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid asset definition id `{value}`: {err}"))
        })?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_domain_and_name(domain_id: &str, name: &str) -> PyResult<Self> {
        let domain = DomainId::parse_fully_qualified(domain_id).map_err(|err| {
            PyValueError::new_err(format!("invalid domain id `{domain_id}`: {err}"))
        })?;
        let name: Name = name
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid asset name `{name}`: {err}")))?;
        Ok(Self {
            inner: AssetDefinitionId::new(domain, name),
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn canonical_address(&self) -> String {
        self.inner.canonical_address().to_string()
    }

    #[getter]
    fn domain<'py>(&self, py: Python<'py>) -> PyResult<Py<PyDomainId>> {
        domain_id_to_py(py, self.inner.domain())
    }

    fn __str__(&self) -> String {
        self.value()
    }

    fn __repr__(&self) -> String {
        format!("AssetDefinitionId('{}')", self.value())
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

#[pyclass(name = "AssetId", module = "iroha_python._crypto")]
#[derive(Clone)]
struct PyAssetId {
    inner: AssetId,
}

#[pymethods]
impl PyAssetId {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        let inner = value
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid asset id `{value}`: {err}")))?;
        Ok(Self { inner })
    }

    #[classmethod]
    fn from_parts(
        _cls: &Bound<'_, PyType>,
        definition: &PyAssetDefinitionId,
        account: &PyAccountId,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: AssetId::new(definition.inner.clone(), account.inner.clone()),
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    #[getter]
    fn definition<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAssetDefinitionId>> {
        asset_definition_id_to_py(py, self.inner.definition())
    }

    #[getter]
    fn account<'py>(&self, py: Python<'py>) -> PyResult<Py<PyAccountId>> {
        account_id_to_py(py, self.inner.account())
    }

    fn __str__(&self) -> String {
        self.value()
    }

    fn __repr__(&self) -> String {
        format!("AssetId('{}')", self.value())
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

#[pyclass(module = "iroha_python._crypto")]
#[derive(Clone)]
struct Instruction {
    inner: InstructionBox,
}

impl Instruction {
    fn new(inner: InstructionBox) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl Instruction {
    #[classmethod]
    fn from_json(_cls: &Bound<'_, PyType>, payload: &str) -> PyResult<Self> {
        let instruction = json::from_str::<InstructionBox>(payload)
            .map_err(|err| PyValueError::new_err(format!("invalid instruction JSON: {err}")))?;
        Ok(Instruction::new(instruction))
    }

    fn to_json(&self) -> PyResult<String> {
        let mut output = String::new();
        self.inner.json_serialize(&mut output);
        Ok(output)
    }

    fn as_dict<'py>(&self, py: Python<'py>) -> PyResult<Py<PyDict>> {
        let json_str = self.to_json()?;
        let json_module = py.import("json")?;
        let loads = json_module.getattr("loads")?;
        let value = loads.call1((json_str,))?;
        let dict: Py<PyDict> = value.extract()?;
        Ok(dict)
    }

    #[classmethod]
    fn register_domain<'py>(
        _cls: &Bound<'py, PyType>,
        py: Python<'py>,
        domain_id: &str,
        metadata: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let domain_id = DomainId::parse_fully_qualified(domain_id).map_err(|err| {
            PyValueError::new_err(format!("invalid domain id `{domain_id}`: {err}"))
        })?;
        let metadata = py_to_metadata(py, metadata)?;
        let new_domain = Domain::new(domain_id).with_metadata(metadata);
        let instruction = Register::<Domain>::domain(new_domain);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn register_account<'py>(
        _cls: &Bound<'py, PyType>,
        py: Python<'py>,
        account_id: &str,
        metadata: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let account_id: AccountId = parse_account_id(account_id)?;
        ensure_ed25519_account(&account_id)?;
        let metadata = py_to_metadata(py, metadata)?;
        let mut new_account = Account::new(account_id);
        new_account.metadata = metadata;
        let instruction = Register::<Account>::account(new_account);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (definition_id, owner, *, name=None, description=None, alias=None, scale=None, mintable=None, balance_scope_policy=None, confidential_policy=None, metadata=None))]
    #[allow(clippy::too_many_arguments)] // PyO3 signature mirrors the Python surface and requires explicit keyword params
    fn register_asset_definition_numeric<'py>(
        _cls: &Bound<'py, PyType>,
        py: Python<'py>,
        definition_id: &str,
        owner: &str,
        name: Option<&str>,
        description: Option<&str>,
        alias: Option<&str>,
        scale: Option<u32>,
        mintable: Option<&str>,
        balance_scope_policy: Option<&str>,
        confidential_policy: Option<&str>,
        metadata: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let definition_id: AssetDefinitionId = definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{definition_id}`: {err}"
            ))
        })?;
        {
            let owner = parse_account_id(owner)?;
            ensure_ed25519_account(&owner)?;
        }

        let spec = match scale {
            Some(s) => NumericSpec::fractional(s),
            None => NumericSpec::unconstrained(),
        };
        let mut new_asset = AssetDefinition::new(definition_id, spec);

        if let Some(name) = name {
            new_asset = new_asset.with_name(name.to_owned());
        }

        if let Some(description) = description {
            new_asset = new_asset.with_description(Some(description.to_owned()));
        }

        if let Some(alias) = alias {
            let alias = alias.parse::<AssetDefinitionAlias>().map_err(|err| {
                PyValueError::new_err(format!("invalid asset definition alias `{alias}`: {err}"))
            })?;
            new_asset = new_asset.with_alias(Some(alias));
        }

        if let Some(meta) = metadata {
            let metadata = py_to_metadata(py, Some(meta))?;
            new_asset = new_asset.with_metadata(metadata);
        }

        let mintable_mode = parse_mintable(mintable)?;
        new_asset = match mintable_mode {
            Mintable::Infinitely => new_asset,
            Mintable::Once => new_asset.mintable_once(),
            Mintable::Limited(tokens) => new_asset.mintable_limited(tokens),
            Mintable::Not => new_asset.with_mintable(Mintable::Not),
        };

        if let Some(policy) = parse_confidential_policy(confidential_policy)? {
            new_asset = new_asset.confidential_policy(policy);
        }

        if let Some(policy) = parse_balance_scope_policy(balance_scope_policy)? {
            new_asset = new_asset.with_balance_scope_policy(policy);
        }

        let instruction = Register::<AssetDefinition>::asset_definition(new_asset);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (asset_definition_id, *, mode=None, allow_shield=true, allow_unshield=true, vk_transfer=None, vk_unshield=None, vk_shield=None))]
    #[allow(clippy::too_many_arguments)]
    fn register_zk_asset<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        mode: Option<&str>,
        allow_shield: bool,
        allow_unshield: bool,
        vk_transfer: Option<&Bound<'py, PyAny>>,
        vk_unshield: Option<&Bound<'py, PyAny>>,
        vk_shield: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let instruction = RegisterZkAsset::new(
            asset,
            parse_zk_asset_mode(mode)?,
            allow_shield,
            allow_unshield,
            parse_verifying_key_id_py(vk_transfer, "vk_transfer")?,
            parse_verifying_key_id_py(vk_unshield, "vk_unshield")?,
            parse_verifying_key_id_py(vk_shield, "vk_shield")?,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (asset_definition_id, identity_commitment, policy_hash, allowed_accounts, verifier_key=None, *, action_class=None, domain_tag=None))]
    fn register_zk_ace_identity_commitment<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        identity_commitment: &Bound<'py, PyAny>,
        policy_hash: &Bound<'py, PyAny>,
        allowed_accounts: &Bound<'py, PyAny>,
        verifier_key: Option<&Bound<'py, PyAny>>,
        action_class: Option<&str>,
        domain_tag: Option<&str>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let instruction = RegisterZkAceIdentityCommitment::new(
            asset,
            py_non_zero_fixed_array::<32>(identity_commitment, "identity_commitment")?,
            py_non_zero_fixed_array::<32>(policy_hash, "policy_hash")?,
            py_account_id_list(allowed_accounts, "allowed_accounts")?,
            parse_zk_ace_action(action_class, "action_class")?,
            parse_zk_ace_domain_tag(domain_tag, "domain_tag")?,
            parse_optional_zk_ace_verifying_key_id_py(verifier_key, "verifier_key")?,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (asset_definition_id, old_identity_commitment, new_identity_commitment, policy_hash, allowed_accounts, verifier_key=None, *, action_class=None, domain_tag=None))]
    #[allow(clippy::too_many_arguments)]
    fn rotate_zk_ace_identity_commitment<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        old_identity_commitment: &Bound<'py, PyAny>,
        new_identity_commitment: &Bound<'py, PyAny>,
        policy_hash: &Bound<'py, PyAny>,
        allowed_accounts: &Bound<'py, PyAny>,
        verifier_key: Option<&Bound<'py, PyAny>>,
        action_class: Option<&str>,
        domain_tag: Option<&str>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let old_identity_commitment =
            py_non_zero_fixed_array::<32>(old_identity_commitment, "old_identity_commitment")?;
        let new_identity_commitment =
            py_non_zero_fixed_array::<32>(new_identity_commitment, "new_identity_commitment")?;
        if old_identity_commitment == new_identity_commitment {
            return Err(PyValueError::new_err(
                "new_identity_commitment must differ from old_identity_commitment",
            ));
        }
        let instruction = RotateZkAceIdentityCommitment::new(
            asset,
            old_identity_commitment,
            new_identity_commitment,
            py_non_zero_fixed_array::<32>(policy_hash, "policy_hash")?,
            py_account_id_list(allowed_accounts, "allowed_accounts")?,
            parse_zk_ace_action(action_class, "action_class")?,
            parse_zk_ace_domain_tag(domain_tag, "domain_tag")?,
            parse_optional_zk_ace_verifying_key_id_py(verifier_key, "verifier_key")?,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (asset_definition_id, identity_commitment, *, reason_hash=None))]
    fn revoke_zk_ace_identity_commitment<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        identity_commitment: &Bound<'py, PyAny>,
        reason_hash: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let instruction = RevokeZkAceIdentityCommitment::new(
            asset,
            py_non_zero_fixed_array::<32>(identity_commitment, "identity_commitment")?,
            parse_optional_fixed_array_py::<32>(reason_hash, "reason_hash")?,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (asset_definition_id, from_account_id, amount, note_commitment, ephemeral_public_key, nonce, ciphertext))]
    #[allow(clippy::too_many_arguments)]
    fn shield_asset<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        from_account_id: &str,
        amount: &str,
        note_commitment: &Bound<'py, PyAny>,
        ephemeral_public_key: &Bound<'py, PyAny>,
        nonce: &Bound<'py, PyAny>,
        ciphertext: &Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let from = parse_account_id(from_account_id)?;
        ensure_ed25519_account(&from)?;
        let amount = parse_u128_text(amount, "amount")?;
        let note_commitment = py_fixed_array::<32>(note_commitment, "note_commitment")?;
        let ephemeral_public_key =
            py_fixed_array::<32>(ephemeral_public_key, "ephemeral_public_key")?;
        let nonce = py_fixed_array::<24>(nonce, "nonce")?;
        let ciphertext = py_bytes_or_base64(ciphertext, "ciphertext")?;
        if ciphertext.is_empty() {
            return Err(PyValueError::new_err("ciphertext must be non-empty"));
        }
        let encrypted_payload =
            ConfidentialEncryptedPayload::new(ephemeral_public_key, nonce, ciphertext);
        let instruction = Shield::new(asset, from, amount, note_commitment, encrypted_payload);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (asset_definition_id, inputs, outputs, proof, *, root_hint=None))]
    fn zk_transfer_prepared<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        inputs: &Bound<'py, PyAny>,
        outputs: &Bound<'py, PyAny>,
        proof: &Bound<'py, PyAny>,
        root_hint: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let inputs = py_fixed_array_list(inputs, "inputs")?;
        if inputs.is_empty() {
            return Err(PyValueError::new_err(
                "inputs must contain at least one nullifier",
            ));
        }
        ensure_unique_fixed_arrays(&inputs, "inputs")?;
        let outputs = py_fixed_array_list(outputs, "outputs")?;
        if outputs.is_empty() {
            return Err(PyValueError::new_err(
                "outputs must contain at least one commitment",
            ));
        }
        ensure_unique_fixed_arrays(&outputs, "outputs")?;
        let proof = parse_zk_proof_attachment(proof, "proof")?;
        let root_hint = parse_optional_root_hint(root_hint, "root_hint")?;
        let instruction = ZkTransfer::new(asset, inputs, outputs, proof, root_hint);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (asset_definition_id, to_account_id, public_amount, inputs, proof, *, outputs=None, root_hint=None))]
    #[allow(clippy::too_many_arguments)]
    fn unshield_prepared<'py>(
        _cls: &Bound<'py, PyType>,
        asset_definition_id: &str,
        to_account_id: &str,
        public_amount: &str,
        inputs: &Bound<'py, PyAny>,
        proof: &Bound<'py, PyAny>,
        outputs: Option<&Bound<'py, PyAny>>,
        root_hint: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let to = parse_account_id(to_account_id)?;
        ensure_ed25519_account(&to)?;
        let public_amount = parse_u128_text(public_amount, "public_amount")?;
        let inputs = py_fixed_array_list(inputs, "inputs")?;
        if inputs.is_empty() {
            return Err(PyValueError::new_err(
                "inputs must contain at least one nullifier",
            ));
        }
        ensure_unique_fixed_arrays(&inputs, "inputs")?;
        let outputs = match outputs {
            Some(value) if !value.is_none() => py_fixed_array_list(value, "outputs")?,
            _ => Vec::new(),
        };
        ensure_unique_fixed_arrays(&outputs, "outputs")?;
        let proof = parse_zk_proof_attachment(proof, "proof")?;
        let root_hint = parse_optional_root_hint(root_hint, "root_hint")?;
        let instruction =
            Unshield::new_with_outputs(asset, to, public_amount, inputs, outputs, proof, root_hint);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (from_account_id, to_account_id, asset_definition_id, amount, identity_commitment, tx_digest, chain_id, domain_tag, action_class, replay_nullifier, policy_hash, proof))]
    #[allow(clippy::too_many_arguments)]
    fn zk_ace_authorized_transfer<'py>(
        _cls: &Bound<'py, PyType>,
        from_account_id: &str,
        to_account_id: &str,
        asset_definition_id: &str,
        amount: &str,
        identity_commitment: &Bound<'py, PyAny>,
        tx_digest: &Bound<'py, PyAny>,
        chain_id: &str,
        domain_tag: &str,
        action_class: &str,
        replay_nullifier: &Bound<'py, PyAny>,
        policy_hash: &Bound<'py, PyAny>,
        proof: &Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let from = parse_account_id(from_account_id)?;
        ensure_ed25519_account(&from)?;
        let to = parse_account_id(to_account_id)?;
        ensure_ed25519_account(&to)?;
        let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{asset_definition_id}`: {err}"
            ))
        })?;
        let amount = parse_u128_text(amount, "amount")?;
        if amount == 0 {
            return Err(PyValueError::new_err("amount must be positive"));
        }
        let chain_id: ChainId = chain_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid chain_id `{chain_id}`: {err}"))
        })?;
        let proof =
            ensure_zk_ace_proof_attachment(parse_zk_proof_attachment(proof, "proof")?, "proof")?;
        let instruction = SubmitZkAceAuthorizedTransfer::new(
            from,
            to,
            asset,
            amount,
            py_non_zero_fixed_array::<32>(identity_commitment, "identity_commitment")?,
            py_non_zero_fixed_array::<32>(tx_digest, "tx_digest")?,
            chain_id,
            parse_zk_ace_domain_tag(Some(domain_tag), "domain_tag")?,
            parse_zk_ace_action(Some(action_class), "action_class")?,
            py_non_zero_fixed_array::<32>(replay_nullifier, "replay_nullifier")?,
            py_non_zero_fixed_array::<32>(policy_hash, "policy_hash")?,
            proof,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn mint_asset_numeric(
        _cls: &Bound<'_, PyType>,
        asset_id: &str,
        quantity: &str,
    ) -> PyResult<Self> {
        let asset_id: AssetId = asset_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid asset id `{asset_id}`: {err}"))
        })?;
        let quantity = parse_numeric(quantity)?;
        let instruction = Mint::asset_numeric(quantity, asset_id);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn burn_asset_numeric(
        _cls: &Bound<'_, PyType>,
        asset_id: &str,
        quantity: &str,
    ) -> PyResult<Self> {
        let asset_id: AssetId = asset_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid asset id `{asset_id}`: {err}"))
        })?;
        let quantity = parse_numeric(quantity)?;
        let instruction = Burn::asset_numeric(quantity, asset_id);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn transfer_asset_numeric(
        _cls: &Bound<'_, PyType>,
        asset_id: &str,
        quantity: &str,
        destination: &str,
    ) -> PyResult<Self> {
        let asset_id: AssetId = asset_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid asset id `{asset_id}`: {err}"))
        })?;
        let destination: AccountId = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let quantity = parse_numeric(quantity)?;
        let instruction = Transfer::asset_numeric(asset_id, quantity, destination);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (destination, name, *, payload=None))]
    fn grant_account_permission<'py>(
        cls: &Bound<'py, PyType>,
        destination: &str,
        name: &str,
        payload: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let destination: AccountId = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let permission_name: Ident = name.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid permission name `{name}`: {err}"))
        })?;
        let payload = py_to_json_value(cls.py(), payload)?;
        let permission = Permission::new(permission_name, payload);
        let instruction = Grant::account_permission(permission, destination);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (destination, name, *, payload=None))]
    fn revoke_account_permission<'py>(
        cls: &Bound<'py, PyType>,
        destination: &str,
        name: &str,
        payload: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let destination: AccountId = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let permission_name: Ident = name.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid permission name `{name}`: {err}"))
        })?;
        let payload = py_to_json_value(cls.py(), payload)?;
        let permission = Permission::new(permission_name, payload);
        let instruction = Revoke::account_permission(permission, destination);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (account_id, key, value=None))]
    fn set_account_key_value<'py>(
        cls: &Bound<'py, PyType>,
        account_id: &str,
        key: &str,
        value: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let account_id = parse_account_id(account_id)?;
        ensure_ed25519_account(&account_id)?;
        let key: Name = key
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid metadata key `{key}`: {err}")))?;
        let json_value = py_to_json_value(cls.py(), value)?;
        let instruction = SetKeyValue::account(account_id, key, json_value);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn remove_account_key_value(
        _cls: &Bound<'_, PyType>,
        account_id: &str,
        key: &str,
    ) -> PyResult<Self> {
        let account_id = parse_account_id(account_id)?;
        ensure_ed25519_account(&account_id)?;
        let key: Name = key
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid metadata key `{key}`: {err}")))?;
        let instruction = RemoveKeyValue::account(account_id, key);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn register_rwa<'py>(cls: &Bound<'py, PyType>, rwa: &Bound<'py, PyAny>) -> PyResult<Self> {
        let rwa = parse_new_rwa_payload(cls.py(), rwa)?;
        let instruction = iroha_data_model::isi::rwa::RegisterRwa { rwa };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn merge_rwas<'py>(cls: &Bound<'py, PyType>, merge: &Bound<'py, PyAny>) -> PyResult<Self> {
        let instruction = parse_merge_rwas_payload(cls.py(), merge)?;
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (agreement_id, initiator, counterparty, *, custodian=None, cash_leg, collateral_leg, rate_bps, maturity_timestamp_ms, governance))]
    #[allow(clippy::too_many_arguments)]
    fn repo_initiate<'py>(
        cls: &Bound<'py, PyType>,
        agreement_id: &str,
        initiator: &str,
        counterparty: &str,
        custodian: Option<&str>,
        cash_leg: &Bound<'py, PyAny>,
        collateral_leg: &Bound<'py, PyAny>,
        rate_bps: u16,
        maturity_timestamp_ms: u64,
        governance: &Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let agreement_id = RepoAgreementId::from_str(agreement_id).map_err(|err| {
            PyValueError::new_err(format!("invalid repo agreement id `{agreement_id}`: {err}"))
        })?;
        let initiator = parse_account_id(initiator)?;
        ensure_ed25519_account(&initiator)?;
        let counterparty = parse_account_id(counterparty)?;
        ensure_ed25519_account(&counterparty)?;
        let custodian = match custodian {
            Some(value) => {
                let account = parse_account_id(value)?;
                ensure_ed25519_account(&account)?;
                Some(account)
            }
            None => None,
        };
        let py = cls.py();
        let cash_leg = parse_repo_cash_leg(py, cash_leg)?;
        let collateral_leg = parse_repo_collateral_leg(py, collateral_leg)?;
        let governance = parse_repo_governance(governance)?;
        let instruction = RepoIsi::new(
            agreement_id,
            initiator,
            counterparty,
            custodian,
            cash_leg,
            collateral_leg,
            rate_bps,
            maturity_timestamp_ms,
            governance,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (agreement_id, initiator, counterparty, cash_leg, collateral_leg, settlement_timestamp_ms))]
    fn repo_unwind<'py>(
        cls: &Bound<'py, PyType>,
        agreement_id: &str,
        initiator: &str,
        counterparty: &str,
        cash_leg: &Bound<'py, PyAny>,
        collateral_leg: &Bound<'py, PyAny>,
        settlement_timestamp_ms: u64,
    ) -> PyResult<Self> {
        let agreement_id = RepoAgreementId::from_str(agreement_id).map_err(|err| {
            PyValueError::new_err(format!("invalid repo agreement id `{agreement_id}`: {err}"))
        })?;
        let initiator = parse_account_id(initiator)?;
        ensure_ed25519_account(&initiator)?;
        let counterparty = parse_account_id(counterparty)?;
        ensure_ed25519_account(&counterparty)?;
        let py = cls.py();
        let cash_leg = parse_repo_cash_leg(py, cash_leg)?;
        let collateral_leg = parse_repo_collateral_leg(py, collateral_leg)?;
        let instruction = ReverseRepoIsi::new(
            agreement_id,
            initiator,
            counterparty,
            cash_leg,
            collateral_leg,
            settlement_timestamp_ms,
        );
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn repo_margin_call(_cls: &Bound<'_, PyType>, agreement_id: &str) -> PyResult<Self> {
        let agreement_id = RepoAgreementId::from_str(agreement_id).map_err(|err| {
            PyValueError::new_err(format!("invalid repo agreement id `{agreement_id}`: {err}"))
        })?;
        let instruction = RepoMarginCallIsi::new(agreement_id);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (settlement_id, delivery_leg, payment_leg, *, order="delivery_then_payment", atomicity="all_or_nothing", metadata=None))]
    #[allow(clippy::too_many_arguments)]
    fn settlement_dvp<'py>(
        cls: &Bound<'py, PyType>,
        settlement_id: &str,
        delivery_leg: &Bound<'py, PyAny>,
        payment_leg: &Bound<'py, PyAny>,
        order: &str,
        atomicity: &str,
        metadata: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let settlement_id = SettlementId::from_str(settlement_id).map_err(|err| {
            PyValueError::new_err(format!("invalid settlement id `{settlement_id}`: {err}"))
        })?;
        let py = cls.py();
        let delivery_leg = parse_settlement_leg(py, delivery_leg, "delivery_leg")?;
        let payment_leg = parse_settlement_leg(py, payment_leg, "payment_leg")?;
        let order = parse_settlement_order(order)?;
        let atomicity = parse_settlement_atomicity(atomicity)?;
        let plan = SettlementPlan::new(order, atomicity);
        let metadata = match metadata {
            Some(value) => py_to_metadata(py, Some(value))?,
            None => Metadata::default(),
        };
        let instruction = DvpIsi {
            settlement_id,
            delivery_leg,
            payment_leg,
            plan,
            metadata,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (settlement_id, primary_leg, counter_leg, *, order="delivery_then_payment", atomicity="all_or_nothing", metadata=None))]
    #[allow(clippy::too_many_arguments)]
    fn settlement_pvp<'py>(
        cls: &Bound<'py, PyType>,
        settlement_id: &str,
        primary_leg: &Bound<'py, PyAny>,
        counter_leg: &Bound<'py, PyAny>,
        order: &str,
        atomicity: &str,
        metadata: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let settlement_id = SettlementId::from_str(settlement_id).map_err(|err| {
            PyValueError::new_err(format!("invalid settlement id `{settlement_id}`: {err}"))
        })?;
        let py = cls.py();
        let primary_leg = parse_settlement_leg(py, primary_leg, "primary_leg")?;
        let counter_leg = parse_settlement_leg(py, counter_leg, "counter_leg")?;
        let order = parse_settlement_order(order)?;
        let atomicity = parse_settlement_atomicity(atomicity)?;
        let plan = SettlementPlan::new(order, atomicity);
        let metadata = match metadata {
            Some(value) => py_to_metadata(py, Some(value))?,
            None => Metadata::default(),
        };
        let instruction = PvpIsi {
            settlement_id,
            primary_leg,
            counter_leg,
            plan,
            metadata,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn transfer_domain(
        _cls: &Bound<'_, PyType>,
        source: &str,
        domain_id: &str,
        destination: &str,
    ) -> PyResult<Self> {
        let source = parse_account_id(source)?;
        ensure_ed25519_account(&source)?;
        let destination = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let domain_id = DomainId::parse_fully_qualified(domain_id).map_err(|err| {
            PyValueError::new_err(format!("invalid domain id `{domain_id}`: {err}"))
        })?;
        let instruction = Transfer::domain(source, domain_id, destination);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn transfer_asset_definition(
        _cls: &Bound<'_, PyType>,
        source: &str,
        definition_id: &str,
        destination: &str,
    ) -> PyResult<Self> {
        let source = parse_account_id(source)?;
        ensure_ed25519_account(&source)?;
        let destination = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let definition_id: AssetDefinitionId = definition_id.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid asset definition id `{definition_id}`: {err}"
            ))
        })?;
        let instruction = Transfer::asset_definition(source, definition_id, destination);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn transfer_nft(
        _cls: &Bound<'_, PyType>,
        source: &str,
        nft_id: &str,
        destination: &str,
    ) -> PyResult<Self> {
        let source = parse_account_id(source)?;
        ensure_ed25519_account(&source)?;
        let destination = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let nft_id: NftId = nft_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid NFT id `{nft_id}`: {err}")))?;
        let instruction = Transfer::nft(source, nft_id, destination);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn transfer_rwa(
        _cls: &Bound<'_, PyType>,
        source: &str,
        rwa_id: &str,
        quantity: &Bound<'_, PyAny>,
        destination: &str,
    ) -> PyResult<Self> {
        let source = parse_account_id(source)?;
        ensure_ed25519_account(&source)?;
        let destination = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let quantity = numeric_from_py(quantity)?;
        let instruction = iroha_data_model::isi::rwa::TransferRwa {
            source,
            rwa: rwa_id,
            quantity,
            destination,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn redeem_rwa(
        _cls: &Bound<'_, PyType>,
        rwa_id: &str,
        quantity: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let quantity = numeric_from_py(quantity)?;
        let instruction = iroha_data_model::isi::rwa::RedeemRwa {
            rwa: rwa_id,
            quantity,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn freeze_rwa(_cls: &Bound<'_, PyType>, rwa_id: &str) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let instruction = iroha_data_model::isi::rwa::FreezeRwa { rwa: rwa_id };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn unfreeze_rwa(_cls: &Bound<'_, PyType>, rwa_id: &str) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let instruction = iroha_data_model::isi::rwa::UnfreezeRwa { rwa: rwa_id };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn hold_rwa(
        _cls: &Bound<'_, PyType>,
        rwa_id: &str,
        quantity: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let quantity = numeric_from_py(quantity)?;
        let instruction = iroha_data_model::isi::rwa::HoldRwa {
            rwa: rwa_id,
            quantity,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn release_rwa(
        _cls: &Bound<'_, PyType>,
        rwa_id: &str,
        quantity: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let quantity = numeric_from_py(quantity)?;
        let instruction = iroha_data_model::isi::rwa::ReleaseRwa {
            rwa: rwa_id,
            quantity,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn force_transfer_rwa(
        _cls: &Bound<'_, PyType>,
        rwa_id: &str,
        quantity: &Bound<'_, PyAny>,
        destination: &str,
    ) -> PyResult<Self> {
        let destination = parse_account_id(destination)?;
        ensure_ed25519_account(&destination)?;
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let quantity = numeric_from_py(quantity)?;
        let instruction = iroha_data_model::isi::rwa::ForceTransferRwa {
            rwa: rwa_id,
            quantity,
            destination,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn set_rwa_controls<'py>(
        cls: &Bound<'py, PyType>,
        rwa_id: &str,
        controls: &Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let controls = py_to_json_model::<RwaControlPolicy>(cls.py(), controls, "controls")?;
        let instruction = iroha_data_model::isi::rwa::SetRwaControls {
            rwa: rwa_id,
            controls,
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (rwa_id, key, value=None))]
    fn set_rwa_key_value<'py>(
        cls: &Bound<'py, PyType>,
        rwa_id: &str,
        key: &str,
        value: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let key: Name = key
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid metadata key `{key}`: {err}")))?;
        let json_value = py_to_json_value(cls.py(), value)?;
        let instruction = SetKeyValue::rwa(rwa_id, key, json_value);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn remove_rwa_key_value(_cls: &Bound<'_, PyType>, rwa_id: &str, key: &str) -> PyResult<Self> {
        let rwa_id: RwaId = rwa_id
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid RWA id `{rwa_id}`: {err}")))?;
        let key: Name = key
            .parse()
            .map_err(|err| PyValueError::new_err(format!("invalid metadata key `{key}`: {err}")))?;
        let instruction = RemoveKeyValue::rwa(rwa_id, key);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (trigger_id, authority, instructions, *, start_ms, **kwargs))]
    fn register_time_trigger<'py>(
        cls: &Bound<'py, PyType>,
        trigger_id: &str,
        authority: &str,
        instructions: Vec<Bound<'py, Instruction>>,
        start_ms: u64,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Self> {
        if instructions.is_empty() {
            return Err(PyValueError::new_err(
                "time trigger requires at least one instruction",
            ));
        }

        let trigger_id: TriggerId = trigger_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid trigger id `{trigger_id}`: {err}"))
        })?;
        let authority = parse_account_id(authority)?;
        ensure_ed25519_account(&authority)?;

        if start_ms == 0 {
            return Err(PyValueError::new_err("start_ms must be greater than zero"));
        }

        let py = cls.py();
        let TimeTriggerKwargsParsed {
            period_ms,
            repeats: repeats_kwarg,
            metadata: metadata_obj,
        } = parse_time_trigger_kwargs(kwargs)?;

        let mut schedule = TimeSchedule::starting_at(Duration::from_millis(start_ms));
        if let Some(period_ms) = period_ms {
            if period_ms == 0 {
                return Err(PyValueError::new_err("period_ms must be greater than zero"));
            }
            schedule = schedule.with_period(Duration::from_millis(period_ms));
        }

        let repeats = match repeats_kwarg {
            Some(0) => {
                return Err(PyValueError::new_err(
                    "repeats must be greater than zero when provided",
                ));
            }
            Some(value) => Repeats::Exactly(value),
            None => Repeats::Indefinitely,
        };

        let metadata = py_to_metadata(py, metadata_obj.as_ref())?;
        let mut instruction_boxes = Vec::with_capacity(instructions.len());
        for instr in instructions {
            let instruction = instr.borrow();
            instruction_boxes.push(instruction.inner.clone());
        }
        let executable = Executable::from(instruction_boxes);
        let action = TriggerAction::new(
            executable,
            repeats,
            authority.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
        )
        .with_metadata(metadata);
        let trigger = Trigger::new(trigger_id, action);
        let instruction = Register::trigger(trigger);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (trigger_id, authority, instructions, *, repeats=None, metadata=None))]
    fn register_precommit_trigger<'py>(
        cls: &Bound<'py, PyType>,
        trigger_id: &str,
        authority: &str,
        instructions: Vec<Bound<'py, Instruction>>,
        repeats: Option<u32>,
        metadata: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        if instructions.is_empty() {
            return Err(PyValueError::new_err(
                "pre-commit trigger requires at least one instruction",
            ));
        }

        let trigger_id: TriggerId = trigger_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid trigger id `{trigger_id}`: {err}"))
        })?;
        let authority = parse_account_id(authority)?;
        ensure_ed25519_account(&authority)?;

        let repeats = match repeats {
            Some(0) => {
                return Err(PyValueError::new_err(
                    "repeats must be greater than zero when provided",
                ));
            }
            Some(value) => Repeats::Exactly(value),
            None => Repeats::Indefinitely,
        };

        let py = cls.py();
        let metadata = py_to_metadata(py, metadata)?;
        let mut instruction_boxes = Vec::with_capacity(instructions.len());
        for instr in instructions {
            let instruction = instr.borrow();
            instruction_boxes.push(instruction.inner.clone());
        }
        let executable = Executable::from(instruction_boxes);
        let action = TriggerAction::new(
            executable,
            repeats,
            authority.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .with_metadata(metadata);
        let trigger = Trigger::new(trigger_id, action);
        let instruction = Register::trigger(trigger);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (trigger_id, *, args=None))]
    fn execute_trigger<'py>(
        cls: &Bound<'py, PyType>,
        trigger_id: &str,
        args: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        let trigger_id: TriggerId = trigger_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid trigger id `{trigger_id}`: {err}"))
        })?;
        let instruction = ExecuteTrigger::new(trigger_id);
        let py = cls.py();
        let instruction = match args {
            None => instruction,
            Some(payload) => {
                let json_value = py_to_json_value(py, Some(payload))?;
                instruction.with_args(json_value)
            }
        };
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn unregister_trigger(_cls: &Bound<'_, PyType>, trigger_id: &str) -> PyResult<Self> {
        let trigger_id: TriggerId = trigger_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid trigger id `{trigger_id}`: {err}"))
        })?;
        let instruction = Unregister::trigger(trigger_id);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn mint_trigger_repetitions(
        _cls: &Bound<'_, PyType>,
        trigger_id: &str,
        repetitions: u32,
    ) -> PyResult<Self> {
        if repetitions == 0 {
            return Err(PyValueError::new_err(
                "repetitions must be greater than zero",
            ));
        }
        let trigger_id: TriggerId = trigger_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid trigger id `{trigger_id}`: {err}"))
        })?;
        let instruction = Mint::trigger_repetitions(repetitions, trigger_id);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    fn burn_trigger_repetitions(
        _cls: &Bound<'_, PyType>,
        trigger_id: &str,
        repetitions: u32,
    ) -> PyResult<Self> {
        if repetitions == 0 {
            return Err(PyValueError::new_err(
                "repetitions must be greater than zero",
            ));
        }
        let trigger_id: TriggerId = trigger_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid trigger id `{trigger_id}`: {err}"))
        })?;
        let instruction = Burn::trigger_repetitions(repetitions, trigger_id);
        Ok(Instruction::new(instruction.into()))
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

/// Thin wrapper around [`TransactionBuilder`] with JSON instruction support.
#[pyclass(module = "iroha_python._crypto")]
#[derive(Clone)]
struct TransactionBuilder {
    chain_id: ChainId,
    authority: AccountId,
    creation_time: Option<Duration>,
    ttl: Option<Duration>,
    nonce: Option<NonZeroU32>,
    instructions: Vec<InstructionBox>,
    metadata: Metadata,
    executable_override: Option<Executable>,
    attachments: Vec<ProofAttachment>,
}

impl TransactionBuilder {
    fn to_model_builder(&self) -> ModelTransactionBuilder {
        let mut builder =
            ModelTransactionBuilder::new(self.chain_id.clone(), self.authority.clone());
        if let Some(creation_time) = self.creation_time {
            builder.set_creation_time(creation_time);
        }
        if let Some(ttl) = self.ttl {
            builder.set_ttl(ttl);
        }
        if let Some(nonce) = self.nonce {
            builder.set_nonce(nonce);
        }

        if let Some(ref executable) = self.executable_override {
            builder = builder.with_executable(executable.clone());
        } else if !self.instructions.is_empty() {
            builder = builder.with_instructions(self.instructions.clone());
        }

        builder = builder.with_metadata(self.metadata.clone());
        if !self.attachments.is_empty() {
            builder = builder.with_attachments(ProofAttachmentList(self.attachments.clone()));
        }
        builder
    }

    fn envelope_from_signed(
        &self,
        signed: &SignedTransaction,
    ) -> PyResult<SignedTransactionEnvelope> {
        let signature: Signature = signed.signature().payload().clone();
        let signature_bytes = signature.payload().to_vec();

        let hash: HashOf<SignedTransaction> = signed.hash();
        let hash_bytes: [u8; Hash::LENGTH] = *hash.as_ref();

        let signed_bytes = codec::encode_adaptive(signed);
        let signed_versioned = signed.encode_versioned();

        let (_, public_key_bytes) =
            public_key_to_bytes(signed.authority().signatory(), "authority public key")?;
        Ok(SignedTransactionEnvelope {
            chain_id: self.chain_id.to_string(),
            authority: self.authority.to_string(),
            signed_transaction: signed_bytes,
            signed_transaction_versioned: signed_versioned,
            hash: hash_bytes,
            signature: signature_bytes,
            public_key: public_key_bytes.to_vec(),
        })
    }

    fn clear_transaction_state(&mut self) {
        self.instructions.clear();
        self.executable_override = None;
        self.attachments.clear();
    }
}

#[pymethods]
impl TransactionBuilder {
    #[new]
    fn new(chain_id: &str, authority: &str) -> PyResult<Self> {
        let chain_id = parse_chain_id(chain_id)?;
        let authority = parse_account_id(authority)?;
        ensure_ed25519_account(&authority)?;
        Ok(Self {
            chain_id,
            authority,
            creation_time: None,
            ttl: None,
            nonce: None,
            instructions: Vec::new(),
            metadata: Metadata::default(),
            executable_override: None,
            attachments: Vec::new(),
        })
    }

    /// Set a deterministic creation timestamp (milliseconds since UNIX epoch).
    fn set_creation_time_ms(&mut self, timestamp_ms: u64) -> PyResult<()> {
        self.creation_time = Some(Duration::from_millis(timestamp_ms));
        Ok(())
    }

    /// Set the transaction time-to-live in milliseconds.
    fn set_ttl_ms(&mut self, ttl_ms: u64) -> PyResult<()> {
        self.ttl = Some(Duration::from_millis(ttl_ms));
        Ok(())
    }

    /// Set the transaction nonce.
    fn set_nonce(&mut self, nonce: u32) -> PyResult<()> {
        let Some(nonce) = NonZeroU32::new(nonce) else {
            return Err(PyValueError::new_err("nonce must be non-zero"));
        };
        self.nonce = Some(nonce);
        Ok(())
    }

    /// Replace metadata using a Norito-compatible JSON string.
    fn set_metadata_json(&mut self, json_payload: &str) -> PyResult<()> {
        self.metadata = json::from_str::<Metadata>(json_payload)
            .map_err(|err| PyValueError::new_err(format!("invalid metadata JSON: {err}")))?;
        Ok(())
    }

    /// Replace metadata using a Python mapping (converted via `json.dumps`).
    fn set_metadata(&mut self, py: Python<'_>, metadata: &Bound<'_, PyAny>) -> PyResult<()> {
        self.metadata = py_to_metadata(py, Some(metadata))?;
        Ok(())
    }

    /// Remove all staged proof attachments.
    fn clear_attachments(&mut self) {
        self.attachments.clear();
    }

    /// Add a Merkle-based lane privacy proof attachment for Nexus private lanes.
    ///
    /// `leaf` and `audit_path` entries are treated as pre-hashed 32-byte digests.
    /// `audit_path` entries may be `None` to represent missing siblings.
    #[allow(clippy::too_many_arguments)]
    fn add_lane_privacy_merkle_attachment(
        &mut self,
        commitment_id: u16,
        leaf: &[u8],
        leaf_index: u32,
        audit_path: Vec<Option<Vec<u8>>>,
        proof_backend: &str,
        proof_bytes: &[u8],
        verifying_key_name: &str,
    ) -> PyResult<()> {
        if leaf.len() != 32 {
            return Err(PyValueError::new_err(
                "leaf must be a 32-byte hash (pre-hashed commitment leaf)",
            ));
        }
        if verifying_key_name.trim().is_empty() {
            return Err(PyValueError::new_err(
                "verifying_key_name must not be empty",
            ));
        }
        let backend = Ident::from_str(proof_backend).map_err(|err| {
            PyValueError::new_err(format!("invalid proof backend identifier: {err}"))
        })?;
        let leaf_arr: [u8; 32] = leaf
            .try_into()
            .map_err(|_| PyValueError::new_err("leaf must be exactly 32 bytes"))?;

        let mut audit_bytes = Vec::with_capacity(audit_path.len());
        for (index, entry) in audit_path.into_iter().enumerate() {
            let converted = match entry {
                Some(bytes) => {
                    let arr: [u8; 32] = bytes.try_into().map_err(|_| {
                        PyValueError::new_err(format!(
                            "audit_path[{index}] must be 32 bytes when provided"
                        ))
                    })?;
                    Some(arr)
                }
                None => None,
            };
            audit_bytes.push(converted);
        }

        let privacy_proof = LanePrivacyProof::merkle_from_raw_path(
            LaneCommitmentId::new(commitment_id),
            leaf_arr,
            leaf_index,
            audit_bytes,
        )
        .map_err(|err| PyValueError::new_err(err.to_string()))?;

        let mut attachment = ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), proof_bytes.to_vec()),
            VerifyingKeyId::new(backend, verifying_key_name.trim()),
        );
        attachment.lane_privacy = Some(privacy_proof);
        self.attachments.push(attachment);
        Ok(())
    }

    /// Add an instruction described by `norito::json` syntax.
    fn add_instruction_json(&mut self, instruction_json: &str) -> PyResult<()> {
        let instruction = json::from_str::<InstructionBox>(instruction_json)
            .map_err(|err| PyValueError::new_err(format!("invalid instruction JSON: {err}")))?;
        self.instructions.push(instruction);
        Ok(())
    }

    /// Append a pre-built instruction.
    fn add_instruction(&mut self, instruction: &Instruction) {
        self.instructions.push(instruction.inner.clone());
    }

    /// Encode the canonical transaction payload bytes without signing.
    fn encode_payload<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let payload_bytes = self.to_model_builder().encode_payload();
        PyBytes::new(py, &payload_bytes)
    }

    /// Return the canonical Iroha transaction payload prehash bytes.
    fn payload_hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let payload_hash = self.to_model_builder().payload_hash_bytes();
        PyBytes::new(py, &payload_hash)
    }

    /// Return the canonical Iroha transaction payload prehash as lowercase hex.
    fn payload_hash_hex(&self) -> String {
        hex_encode(self.to_model_builder().payload_hash_bytes())
    }

    /// Override the executable with raw IVM bytecode (Norito-encoded hex string).
    fn set_bytecode_hex(&mut self, hex_payload: &str) -> PyResult<()> {
        let bytes = hex::decode(hex_payload)
            .map_err(|err| PyValueError::new_err(format!("invalid hex bytecode: {err}")))?;
        let bytecode = IvmBytecode::from_compiled(bytes);
        self.executable_override = Some(Executable::Ivm(bytecode));
        Ok(())
    }

    /// Sign the transaction, returning an envelope with Norito payloads and hash.
    fn sign(&mut self, private_key: &[u8]) -> PyResult<SignedTransactionEnvelope> {
        let private_key = parse_private_key(private_key)?;
        let signed = self.to_model_builder().sign(&private_key);
        let envelope = self.envelope_from_signed(&signed)?;

        // Reset instructions for the next transaction while keeping metadata.
        self.clear_transaction_state();

        Ok(envelope)
    }

    /// Finalize the transaction using a wallet-provided external signature.
    fn build_with_signature(&mut self, signature: &[u8]) -> PyResult<SignedTransactionEnvelope> {
        if signature.len() != 64 {
            return Err(PyValueError::new_err(format!(
                "Ed25519 signature must be 64 bytes, got {}",
                signature.len()
            )));
        }

        let signed = self
            .to_model_builder()
            .build_with_signature(Signature::from_bytes(signature));
        signed.verify_signature().map_err(|err| {
            PyValueError::new_err(format!("signature verification failed: {err}"))
        })?;
        let envelope = self.envelope_from_signed(&signed)?;
        self.clear_transaction_state();
        Ok(envelope)
    }
}

/// Signed transaction outputs exposed to Python.
#[pyclass(module = "iroha_python._crypto")]
struct SignedTransactionEnvelope {
    chain_id: String,
    authority: String,
    signed_transaction: Vec<u8>,
    signed_transaction_versioned: Vec<u8>,
    hash: [u8; Hash::LENGTH],
    signature: Vec<u8>,
    public_key: Vec<u8>,
}

#[pymethods]
impl SignedTransactionEnvelope {
    /// Construct an envelope from its JSON representation produced by `to_json`.
    #[classmethod]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let value: norito::json::Value = norito::json::from_str(json_str).map_err(|err| {
            PyValueError::new_err(format!("failed to parse envelope JSON: {err}"))
        })?;
        let obj = value
            .as_object()
            .ok_or_else(|| PyValueError::new_err("expected JSON object"))?;

        let chain_id = obj
            .get("chain_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyValueError::new_err("missing `chain_id` field"))?
            .to_string();
        let authority = obj
            .get("authority")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyValueError::new_err("missing `authority` field"))?
            .to_string();

        let signed_b64 = obj
            .get("signed_transaction_b64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyValueError::new_err("missing `signed_transaction_b64` field"))?;
        let signed_versioned_b64 = obj
            .get("signed_transaction_versioned_b64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PyValueError::new_err("missing `signed_transaction_versioned_b64` field")
            })?;
        let signature_b64 = obj
            .get("signature_b64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyValueError::new_err("missing `signature_b64` field"))?;
        let public_key_b64 = obj
            .get("public_key_b64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyValueError::new_err("missing `public_key_b64` field"))?;
        let hash_hex = obj
            .get("hash_hex")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PyValueError::new_err("missing `hash_hex` field"))?;

        let signed_transaction = BASE64.decode(signed_b64.as_bytes()).map_err(|err| {
            PyValueError::new_err(format!("invalid signed_transaction_b64: {err}"))
        })?;
        let signed_transaction_versioned =
            BASE64
                .decode(signed_versioned_b64.as_bytes())
                .map_err(|err| {
                    PyValueError::new_err(format!(
                        "invalid signed_transaction_versioned_b64: {err}"
                    ))
                })?;
        let signature = BASE64
            .decode(signature_b64.as_bytes())
            .map_err(|err| PyValueError::new_err(format!("invalid signature_b64: {err}")))?;
        let public_key = BASE64
            .decode(public_key_b64.as_bytes())
            .map_err(|err| PyValueError::new_err(format!("invalid public_key_b64: {err}")))?;

        if signature.len() != 64 {
            return Err(PyValueError::new_err(format!(
                "signature must be 64 bytes, got {}",
                signature.len()
            )));
        }
        if public_key.len() != 32 {
            return Err(PyValueError::new_err(format!(
                "public key must be 32 bytes, got {}",
                public_key.len()
            )));
        }

        let mut hash = [0u8; Hash::LENGTH];
        let hash_bytes = hex::decode(hash_hex).map_err(|err| {
            PyValueError::new_err(format!("invalid hash_hex value `{hash_hex}`: {err}"))
        })?;
        if hash_bytes.len() != Hash::LENGTH {
            return Err(PyValueError::new_err(format!(
                "hash must be {} bytes, got {}",
                Hash::LENGTH,
                hash_bytes.len()
            )));
        }
        hash.copy_from_slice(&hash_bytes);

        Ok(Self {
            chain_id,
            authority,
            signed_transaction,
            signed_transaction_versioned,
            hash,
            signature,
            public_key,
        })
    }

    #[getter]
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    #[getter]
    fn authority(&self) -> &str {
        &self.authority
    }

    #[getter]
    fn signed_transaction<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.signed_transaction)
    }

    #[getter]
    fn signed_transaction_versioned<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.signed_transaction_versioned)
    }

    #[getter]
    fn hash<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.hash)
    }

    #[getter]
    fn signature<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.signature)
    }

    #[getter]
    fn public_key<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.public_key)
    }

    /// Return the transaction hash as a hex string.
    fn hash_hex(&self) -> String {
        hex::encode(self.hash)
    }

    /// Return the attached signature as a hex string.
    fn signature_hex(&self) -> String {
        hex::encode(&self.signature)
    }

    /// Return the authority public key as a hex string.
    fn public_key_hex(&self) -> String {
        hex::encode(&self.public_key)
    }

    /// Return a Python dict summarising the envelope contents.
    fn as_dict<'py>(&self, py: Python<'py>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("chain_id", &self.chain_id)?;
        dict.set_item("authority", &self.authority)?;
        dict.set_item(
            "signed_transaction",
            PyBytes::new(py, &self.signed_transaction),
        )?;
        dict.set_item(
            "signed_transaction_versioned",
            PyBytes::new(py, &self.signed_transaction_versioned),
        )?;
        dict.set_item("hash", PyBytes::new(py, &self.hash))?;
        dict.set_item("hash_hex", self.hash_hex())?;
        dict.set_item("signature", PyBytes::new(py, &self.signature))?;
        dict.set_item("signature_hex", self.signature_hex())?;
        dict.set_item("public_key", PyBytes::new(py, &self.public_key))?;
        dict.set_item("public_key_hex", self.public_key_hex())?;
        Ok(dict.unbind())
    }

    /// Decode proof attachments (if present) into a Norito JSON string.
    fn attachments_json(&self) -> PyResult<Option<String>> {
        let signed = SignedTransaction::decode_all_versioned(&self.signed_transaction_versioned)
            .map_err(|err| {
                PyValueError::new_err(format!("failed to decode signed transaction: {err}"))
            })?;
        match signed.attachments() {
            Some(list) => {
                let value = norito::json::to_value(list).map_err(|err| {
                    PyValueError::new_err(format!("failed to serialize attachments: {err}"))
                })?;
                norito::json::to_string(&value).map(Some).map_err(|err| {
                    PyValueError::new_err(format!("failed to serialize attachments: {err}"))
                })
            }
            None => Ok(None),
        }
    }

    /// Return a JSON string representation with base64-encoded binary fields.
    fn to_json(&self) -> PyResult<String> {
        let mut map = norito::json::Map::new();
        map.insert(
            "chain_id".into(),
            norito::json::Value::String(self.chain_id.clone()),
        );
        map.insert(
            "authority".into(),
            norito::json::Value::String(self.authority.clone()),
        );
        map.insert(
            "signed_transaction_b64".into(),
            norito::json::Value::String(BASE64.encode(&self.signed_transaction)),
        );
        map.insert(
            "signed_transaction_versioned_b64".into(),
            norito::json::Value::String(BASE64.encode(&self.signed_transaction_versioned)),
        );
        map.insert(
            "hash_hex".into(),
            norito::json::Value::String(self.hash_hex()),
        );
        map.insert(
            "signature_b64".into(),
            norito::json::Value::String(BASE64.encode(&self.signature)),
        );
        map.insert(
            "signature_hex".into(),
            norito::json::Value::String(self.signature_hex()),
        );
        map.insert(
            "public_key_b64".into(),
            norito::json::Value::String(BASE64.encode(&self.public_key)),
        );
        map.insert(
            "public_key_hex".into(),
            norito::json::Value::String(self.public_key_hex()),
        );
        let value = norito::json::Value::Object(map);
        norito::json::to_string(&value)
            .map_err(|err| PyValueError::new_err(format!("failed to serialize envelope: {err}")))
    }
}

#[pyfunction]
#[pyo3(name = "supported_crypto_algorithms")]
/// Return the canonical names of signature algorithms compiled into the Python SDK.
fn supported_crypto_algorithms_py() -> Vec<String> {
    supported_crypto_algorithms()
        .into_iter()
        .map(|algorithm| algorithm.as_static_str().to_owned())
        .collect()
}

#[pyfunction]
#[pyo3(name = "normalize_crypto_algorithm")]
/// Normalize a crypto algorithm alias to the canonical `iroha_crypto` label.
fn normalize_crypto_algorithm_py(algorithm: &str) -> PyResult<String> {
    parse_algorithm_arg(algorithm).map(|algorithm| algorithm.as_static_str().to_owned())
}

#[pyfunction]
#[pyo3(name = "generate_keypair")]
/// Generate a random key pair for any signature algorithm compiled into the SDK.
fn generate_keypair_py(py: Python<'_>, algorithm: &str) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let key_pair = KeyPair::random_with_algorithm(algorithm);
    keypair_to_py(py, key_pair)
}

#[pyfunction]
#[pyo3(name = "derive_keypair_from_seed")]
/// Derive a key pair for any supported algorithm from arbitrary seed material.
fn derive_keypair_from_seed_py(
    py: Python<'_>,
    seed: &[u8],
    algorithm: &str,
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let key_pair = KeyPair::from_seed(seed.to_vec(), algorithm);
    keypair_to_py(py, key_pair)
}

#[pyfunction]
#[pyo3(name = "load_keypair")]
/// Reconstruct a key pair for any supported algorithm from raw private-key payload bytes.
fn load_keypair_py(
    py: Python<'_>,
    private_key: &[u8],
    algorithm: &str,
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let private = parse_private_key_for_algorithm(algorithm, private_key)?;
    let key_pair = KeyPair::from_private_key(private)
        .map_err(|err| PyValueError::new_err(format!("failed to reconstruct key pair: {err}")))?;
    keypair_to_py(py, key_pair)
}

#[pyfunction]
#[pyo3(name = "sign")]
/// Sign `message` with the private-key payload for any supported signature algorithm.
fn sign_py(
    py: Python<'_>,
    algorithm: &str,
    private_key: &[u8],
    message: &[u8],
) -> PyResult<Py<PyBytes>> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let private_key = parse_private_key_for_algorithm(algorithm, private_key)?;
    let signature = Signature::new(&private_key, message);
    Ok(Py::from(PyBytes::new(py, signature.payload())))
}

#[pyfunction]
#[pyo3(name = "verify")]
/// Verify a raw signature against a public-key payload for any supported signature algorithm.
fn verify_py(
    algorithm: &str,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> PyResult<bool> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let public_key = parse_public_key_for_algorithm(algorithm, public_key)?;
    let signature = Signature::from_bytes(signature);
    Ok(signature.verify(&public_key, message).is_ok())
}

#[pyfunction]
#[pyo3(name = "public_key_multihash", signature = (algorithm, public_key, prefixed=false))]
/// Return the canonical multihash encoding for a public-key payload.
fn public_key_multihash_py(algorithm: &str, public_key: &[u8], prefixed: bool) -> PyResult<String> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let public_key = parse_public_key_for_algorithm(algorithm, public_key)?;
    public_key_multihash_string(&public_key, prefixed, "public key multihash")
}

#[pyfunction]
#[pyo3(name = "private_key_multihash", signature = (algorithm, private_key, prefixed=false))]
/// Return the canonical multihash encoding for a private-key payload.
fn private_key_multihash_py(
    algorithm: &str,
    private_key: &[u8],
    prefixed: bool,
) -> PyResult<String> {
    let algorithm = parse_algorithm_arg(algorithm)?;
    let private_key = parse_private_key_for_algorithm(algorithm, private_key)?;
    let exposed = ExposedPrivateKey(private_key);
    private_key_multihash_string(&exposed, prefixed, "private key multihash")
}

#[pyfunction]
#[pyo3(name = "parse_public_key_multihash")]
/// Decode a public key from a bare or algorithm-prefixed multihash string.
fn parse_public_key_multihash_py(py: Python<'_>, encoded: &str) -> PyResult<(String, Py<PyBytes>)> {
    let public_key = encoded.parse::<PublicKey>().map_err(|err| {
        PyValueError::new_err(format!("failed to parse public key multihash: {err}"))
    })?;
    let (algorithm, payload) = public_key_to_bytes(&public_key, "public key multihash")?;
    Ok((
        algorithm.as_static_str().to_owned(),
        Py::from(PyBytes::new(py, payload)),
    ))
}

#[pyfunction]
#[pyo3(name = "parse_private_key_multihash")]
/// Decode a private key from a bare or algorithm-prefixed multihash string.
fn parse_private_key_multihash_py(
    py: Python<'_>,
    encoded: &str,
) -> PyResult<(String, Py<PyBytes>)> {
    let exposed = encoded.parse::<ExposedPrivateKey>().map_err(|err| {
        PyValueError::new_err(format!("failed to parse private key multihash: {err}"))
    })?;
    let (algorithm, mut payload) = exposed.0.to_bytes();
    let bytes = Py::from(PyBytes::new(py, payload.as_slice()));
    payload.fill(0);
    Ok((algorithm.as_static_str().to_owned(), bytes))
}

#[pyfunction]
#[pyo3(name = "load_keypair_from_multihash")]
/// Reconstruct a key pair from a private-key multihash string.
fn load_keypair_from_multihash_py(
    py: Python<'_>,
    encoded: &str,
) -> PyResult<(String, Py<PyBytes>, Py<PyBytes>)> {
    let exposed = encoded.parse::<ExposedPrivateKey>().map_err(|err| {
        PyValueError::new_err(format!("failed to parse private key multihash: {err}"))
    })?;
    let algorithm = exposed.0.algorithm();
    let key_pair = KeyPair::from_private_key(exposed.0)
        .map_err(|err| PyValueError::new_err(format!("failed to reconstruct key pair: {err}")))?;
    let (private, public) = keypair_to_py(py, key_pair)?;
    Ok((algorithm.as_static_str().to_owned(), private, public))
}

#[pyfunction]
#[pyo3(name = "generate_ed25519_keypair")]
/// Generate a random Ed25519 key pair using `iroha_crypto` defaults.
fn generate_ed25519_keypair_py(py: Python<'_>) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    keypair_to_py(py, key_pair)
}

#[pyfunction]
#[pyo3(name = "derive_ed25519_keypair_from_seed")]
/// Derive an Ed25519 key pair from an arbitrary seed (hashed internally to 32 bytes).
fn derive_ed25519_keypair_from_seed_py(
    py: Python<'_>,
    seed: &[u8],
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let key_pair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
    keypair_to_py(py, key_pair)
}

#[pyfunction]
#[pyo3(name = "load_ed25519_keypair")]
/// Reconstruct an Ed25519 key pair from raw private key bytes.
fn load_ed25519_keypair_py(
    py: Python<'_>,
    private_key: &[u8],
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let private = parse_private_key(private_key)?;
    let key_pair = KeyPair::from_private_key(private)
        .map_err(|err| PyValueError::new_err(format!("failed to reconstruct key pair: {err}")))?;
    keypair_to_py(py, key_pair)
}

#[pyfunction]
#[pyo3(name = "sign_ed25519")]
/// Sign `message` using the given Ed25519 private key; returns the raw signature bytes.
fn sign_ed25519_py(py: Python<'_>, private_key: &[u8], message: &[u8]) -> PyResult<Py<PyBytes>> {
    let private_key = parse_private_key(private_key)?;
    let signature = Signature::new(&private_key, message);
    Ok(Py::from(PyBytes::new(py, signature.payload())))
}

#[pyfunction]
#[pyo3(name = "verify_ed25519")]
/// Verify `signature` against `message` and the provided Ed25519 public key.
fn verify_ed25519_py(public_key: &[u8], message: &[u8], signature: &[u8]) -> PyResult<bool> {
    let public_key = parse_public_key(public_key)?;
    let signature = Signature::from_bytes(signature);
    Ok(signature.verify(&public_key, message).is_ok())
}

#[pyfunction]
#[pyo3(name = "sm2_default_distid")]
/// Return the default SM2 distinguishing identifier.
fn sm2_default_distid_py() -> String {
    Sm2PublicKey::default_distid()
}

#[pyfunction]
#[pyo3(name = "generate_sm2_keypair", signature = (distid=None))]
/// Generate a random SM2 key pair; returns the 32-byte private scalar and 65-byte SEC1 public key.
fn generate_sm2_keypair_py(
    py: Python<'_>,
    distid: Option<&str>,
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let distid = sm2_distid_arg(distid);
    let mut rng = OsRng06;
    let private = Sm2PrivateKey::random(distid, &mut rng)
        .into_sm2_result()
        .map_err(|err| PyValueError::new_err(format!("failed to generate SM2 key pair: {err}")))?;
    let public = private.public_key();
    let private_bytes = private.secret_bytes();
    let public_bytes = public.to_sec1_bytes(false);
    Ok((
        Py::from(PyBytes::new(py, &private_bytes)),
        Py::from(PyBytes::new(py, &public_bytes)),
    ))
}

#[pyfunction]
#[pyo3(name = "derive_sm2_keypair_from_seed", signature = (seed, distid=None))]
/// Deterministically derive an SM2 key pair from `seed`.
fn derive_sm2_keypair_from_seed_py(
    py: Python<'_>,
    seed: &[u8],
    distid: Option<&str>,
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let distid = sm2_distid_arg(distid);
    let private = Sm2PrivateKey::from_seed(distid, seed).map_err(|err| {
        PyValueError::new_err(format!("failed to derive SM2 key from seed: {err}"))
    })?;
    let public = private.public_key();
    let private_bytes = private.secret_bytes();
    let public_bytes = public.to_sec1_bytes(false);
    Ok((
        Py::from(PyBytes::new(py, &private_bytes)),
        Py::from(PyBytes::new(py, &public_bytes)),
    ))
}

#[pyfunction]
#[pyo3(name = "load_sm2_keypair", signature = (private_key, distid=None))]
/// Reconstruct an SM2 key pair from raw private-key bytes.
fn load_sm2_keypair_py(
    py: Python<'_>,
    private_key: &[u8],
    distid: Option<&str>,
) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let private = parse_sm2_private_key(distid, private_key)?;
    let public = private.public_key();
    let private_bytes = private.secret_bytes();
    let public_bytes = public.to_sec1_bytes(false);
    Ok((
        Py::from(PyBytes::new(py, &private_bytes)),
        Py::from(PyBytes::new(py, &public_bytes)),
    ))
}

#[pyfunction]
#[pyo3(name = "sm2_public_key_multihash", signature = (public_key, distid=None))]
/// Return the canonical multihash encoding for an SM2 public key.
fn sm2_public_key_multihash_py(public_key: &[u8], distid: Option<&str>) -> PyResult<String> {
    let distid = sm2_distid_arg(distid);
    let _ = parse_sm2_public_key(Some(distid.as_str()), public_key)?;
    let payload = encode_sm2_public_key_payload(&distid, public_key).map_err(|err| {
        PyValueError::new_err(format!("failed to encode SM2 public key payload: {err}"))
    })?;
    PublicKey::from_bytes(Algorithm::Sm2, &payload)
        .map_err(|err| PyValueError::new_err(format!("failed to construct SM2 public key: {err}")))
        .and_then(|pk| public_key_multihash_string(&pk, false, "SM2 public key multihash"))
}

fn sm2_fixture_public_key_multihashes(public_key: &PublicKey) -> PyResult<(String, String)> {
    let multihash = public_key_multihash_string(public_key, false, "SM2 fixture public key")?;
    let prefixed = public_key_multihash_string(public_key, true, "SM2 fixture public key")?;
    Ok((multihash, prefixed))
}

#[pyfunction]
#[pyo3(name = "sign_sm2", signature = (private_key, message, distid=None))]
/// Sign `message` with the provided SM2 private key.
fn sign_sm2_py(
    py: Python<'_>,
    private_key: &[u8],
    message: &[u8],
    distid: Option<&str>,
) -> PyResult<Py<PyBytes>> {
    let private = parse_sm2_private_key(distid, private_key)?;
    let signature = private.sign(message).to_bytes();
    Ok(Py::from(PyBytes::new(py, &signature)))
}

#[pyfunction]
#[pyo3(name = "verify_sm2", signature = (public_key, message, signature, distid=None))]
/// Verify an SM2 signature against `message` and the provided public key.
fn verify_sm2_py(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
    distid: Option<&str>,
) -> PyResult<bool> {
    let public = parse_sm2_public_key(distid, public_key)?;
    let signature = parse_sm2_signature(signature)?;
    Ok(public.verify(message, &signature).is_ok())
}

#[pyfunction]
#[pyo3(name = "hash_blake2b_32")]
/// Compute the canonical Iroha Blake2b-256 hash for the given bytes.
fn hash_blake2b_32_py(py: Python<'_>, payload: &[u8]) -> PyResult<Py<PyBytes>> {
    let hash = Hash::new(payload);
    let bytes: [u8; Hash::LENGTH] = hash.into();
    Ok(Py::from(PyBytes::new(py, &bytes)))
}

#[pyfunction]
#[pyo3(name = "verify_signed_transaction_versioned")]
/// Decode a versioned signed transaction and verify its signature.
fn verify_signed_transaction_versioned_py(bytes: &[u8]) -> PyResult<bool> {
    let signed = SignedTransaction::decode_all_versioned(bytes).map_err(|err| {
        PyValueError::new_err(format!("failed to decode SignedTransaction: {err}"))
    })?;
    Ok(signed.verify_signature().is_ok())
}

#[pyfunction]
#[pyo3(name = "derive_confidential_keyset")]
/// Derive the confidential key hierarchy from a 32-byte spend key.
fn derive_confidential_keyset_py(py: Python<'_>, spend_key: &[u8]) -> PyResult<Py<PyDict>> {
    let keyset = derive_keyset_from_slice(spend_key)
        .map_err(|err| PyValueError::new_err(format!("invalid confidential spend key: {err}")))?;
    let as_dict = PyDict::new(py);
    as_dict.set_item("sk_spend", PyBytes::new(py, keyset.spend_key()))?;
    as_dict.set_item("nk", PyBytes::new(py, keyset.nullifier_key()))?;
    as_dict.set_item("ivk", PyBytes::new(py, keyset.incoming_view_key()))?;
    as_dict.set_item("ovk", PyBytes::new(py, keyset.outgoing_view_key()))?;
    as_dict.set_item("fvk", PyBytes::new(py, keyset.full_view_key()))?;
    Ok(as_dict.unbind())
}

#[pyfunction]
#[pyo3(name = "sm2_fixture_from_seed")]
/// Compute the canonical SM2 fixture values for the given distinguishing ID, seed, and message.
fn sm2_fixture_from_seed_py(
    py: Python<'_>,
    distid: &str,
    seed: &[u8],
    message: &[u8],
) -> PyResult<Py<PyDict>> {
    let private = Sm2PrivateKey::from_seed(distid, seed).map_err(|err| {
        PyValueError::new_err(format!("failed to derive SM2 private key from seed: {err}"))
    })?;
    let public = private.public_key();
    let secret_hex = hex::encode_upper(private.secret_bytes());
    let public_bytes = public.to_sec1_bytes(false);
    let public_hex = hex::encode_upper(&public_bytes);
    let payload = encode_sm2_public_key_payload(distid, &public_bytes).map_err(|err| {
        PyValueError::new_err(format!("failed to encode SM2 public key payload: {err}"))
    })?;
    let public_key = PublicKey::from_bytes(Algorithm::Sm2, &payload).map_err(|err| {
        PyValueError::new_err(format!("failed to construct SM2 public key: {err}"))
    })?;
    let (multihash, prefixed) = sm2_fixture_public_key_multihashes(&public_key)?;
    let za = public
        .compute_z(distid)
        .map_err(|err| PyValueError::new_err(format!("failed to compute SM2 ZA: {err}")))?;
    let za_hex = hex::encode_upper(za);
    let signature = private.sign(message);
    let signature_bytes = signature.as_bytes();
    let signature_hex = hex::encode_upper(signature_bytes);
    let r_hex = hex::encode_upper(signature.r);
    let s_hex = hex::encode_upper(signature.s);
    let seed_hex = hex::encode_upper(seed);
    let message_hex = hex::encode_upper(message);

    let result = PyDict::new(py);
    result.set_item("distid", distid)?;
    result.set_item("seed_hex", seed_hex)?;
    result.set_item("message_hex", message_hex)?;
    result.set_item("private_key_hex", secret_hex)?;
    result.set_item("public_key_sec1_hex", public_hex)?;
    result.set_item("public_key_multihash", multihash)?;
    result.set_item("public_key_prefixed", prefixed)?;
    result.set_item("za", za_hex)?;
    result.set_item("signature", signature_hex)?;
    result.set_item("r", r_hex)?;
    result.set_item("s", s_hex)?;
    Ok(result.unbind())
}

#[pyfunction]
#[pyo3(name = "encode_connect_frame")]
/// Encode a connect frame described by a Python dictionary into Norito bytes.
fn encode_connect_frame_py(py: Python<'_>, frame: &Bound<'_, PyDict>) -> PyResult<Py<PyBytes>> {
    let sid_bytes = dict_require(frame, "sid", || {
        PyValueError::new_err("connect frame `sid` is required")
    })?
    .extract::<Vec<u8>>()?;
    let sid = fixed_array::<32>(&sid_bytes, "sid")?;
    let dir_str = dict_require(frame, "direction", || {
        PyValueError::new_err("connect frame `direction` is required")
    })?
    .extract::<String>()?;
    let dir = parse_connect_direction(&dir_str)?;
    let seq = dict_require(frame, "sequence", || {
        PyValueError::new_err("connect frame `sequence` is required")
    })?
    .extract::<u64>()?;
    let kind_obj = dict_require(frame, "kind", || {
        PyValueError::new_err("connect frame `kind` is required")
    })?;
    let kind_mapping = kind_obj
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("connect frame `kind` must be a dict"))?;
    let kind = parse_frame_kind(kind_mapping)?;
    let proto_frame = ConnectFrameV1 {
        sid,
        dir,
        seq,
        kind,
    };
    let encoded = norito::codec::Encode::encode(&proto_frame);
    Ok(Py::from(PyBytes::new(py, encoded.as_slice())))
}

#[pyfunction]
#[pyo3(name = "decode_connect_frame")]
/// Decode Norito-encoded connect frame bytes into a Python dictionary.
fn decode_connect_frame_py(py: Python<'_>, payload: &[u8]) -> PyResult<Py<PyDict>> {
    let frame = decode_connect_frame_bytes(payload)?;
    let mapping = PyDict::new(py);
    mapping.set_item("sid", PyBytes::new(py, &frame.sid))?;
    mapping.set_item("direction", connect_direction_str(frame.dir))?;
    mapping.set_item("sequence", frame.seq)?;
    mapping.set_item("kind", encode_frame_kind(py, &frame.kind)?)?;
    Ok(mapping.unbind())
}

#[pyfunction]
/// Return `True` when the CUDA backend initialised successfully for the current process.
fn cuda_available_py() -> bool {
    ivm::cuda_available()
}

#[pyfunction]
/// Return `True` when the CUDA backend has been disabled after an error or self-test failure.
fn cuda_disabled_py() -> bool {
    ivm::cuda_disabled()
}

#[pyfunction]
/// Execute the Poseidon2 permutation on the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn poseidon2_cuda_py(a: u64, b: u64) -> Option<u64> {
    ivm::poseidon2_cuda(a, b)
}

#[pyfunction]
/// Execute multiple Poseidon2 permutations on the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn poseidon2_cuda_many_py(inputs: Vec<(u64, u64)>) -> Option<Vec<u64>> {
    ivm::poseidon2_cuda_many(&inputs)
}

#[pyfunction]
/// Execute the Poseidon6 permutation on the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn poseidon6_cuda_py(inputs: [u64; 6]) -> Option<u64> {
    ivm::poseidon6_cuda(inputs)
}

#[pyfunction]
/// Execute multiple Poseidon6 permutations on the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn poseidon6_cuda_many_py(inputs: Vec<[u64; 6]>) -> Option<Vec<u64>> {
    ivm::poseidon6_cuda_many(&inputs)
}

#[pyfunction]
/// Add two BN254 field elements using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn bn254_add_cuda_py(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
    ivm::bn254_add_cuda(a, b)
}

#[pyfunction]
/// Add many BN254 field-element pairs using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime, or
/// when the input vectors differ in length.
fn bn254_add_cuda_many_py(lhs: Vec<[u64; 4]>, rhs: Vec<[u64; 4]>) -> Option<Vec<[u64; 4]>> {
    ivm::bn254_add_batch_cuda(&lhs, &rhs)
}

#[pyfunction]
/// Subtract two BN254 field elements using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn bn254_sub_cuda_py(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
    ivm::bn254_sub_cuda(a, b)
}

#[pyfunction]
/// Subtract many BN254 field-element pairs using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime, or
/// when the input vectors differ in length.
fn bn254_sub_cuda_many_py(lhs: Vec<[u64; 4]>, rhs: Vec<[u64; 4]>) -> Option<Vec<[u64; 4]>> {
    ivm::bn254_sub_batch_cuda(&lhs, &rhs)
}

#[pyfunction]
/// Multiply two BN254 field elements using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn bn254_mul_cuda_py(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
    ivm::bn254_mul_cuda(a, b)
}

#[pyfunction]
/// Multiply many BN254 field-element pairs using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime, or
/// when the input vectors differ in length.
fn bn254_mul_cuda_many_py(lhs: Vec<[u64; 4]>, rhs: Vec<[u64; 4]>) -> Option<Vec<[u64; 4]>> {
    ivm::bn254_mul_batch_cuda(&lhs, &rhs)
}

#[pyfunction]
/// Return a deterministic relay envelope fixture and a tampered copy for testing.
fn lane_relay_envelope_fixture_py() -> PyResult<(Vec<u8>, Vec<u8>)> {
    let lane_id = LaneId::new(3);
    let dataspace_id = DataSpaceId::new(2);
    let settlement = LaneBlockCommitment {
        block_height: 1,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 10,
        total_xor_due_micro: 5,
        total_xor_after_haircut_micro: 4,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let mut header = BlockHeader::new(
        NonZeroU64::new(1).expect("nonzero height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    );
    let da_hash = HashOf::from_untyped_unchecked(Hash::new([0xAA; 4]));
    header.set_da_commitments_hash(Some(da_hash));
    let validator_set: Vec<PeerId> = Vec::new();
    let qc = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: header.hash(),
        parent_state_root: Hash::new([0xBA; 4]),
        post_state_root: Hash::new([0xBB; 4]),
        height: header.height().get(),
        view: 1,
        epoch: 0,
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![0x01],
            bls_aggregate_signature: vec![0xCC; 48],
        },
    };
    let envelope = LaneRelayEnvelope::new(header, Some(qc), Some(da_hash), settlement, 64)
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    let valid = norito::to_bytes(&envelope)
        .map_err(|err| PyValueError::new_err(format!("failed to serialize envelope: {err}")))?;

    let mut tampered = valid.clone();
    if let Some(last) = tampered.last_mut() {
        *last ^= 0xFF;
    }
    Ok((valid, tampered))
}

#[pyfunction]
/// Verify the Norito-encoded relay envelope bytes returned by `/v1/sumeragi/status`.
fn verify_lane_relay_envelope_bytes_py(envelope: &[u8]) -> PyResult<()> {
    let mut slice = envelope;
    let parsed = LaneRelayEnvelope::decode_all(&mut slice)
        .map_err(|err| PyValueError::new_err(format!("failed to decode relay envelope: {err}")))?;
    parsed
        .verify()
        .map_err(|err| PyValueError::new_err(err.to_string()))
}

#[pyfunction]
/// Decode relay envelope bytes into a JSON string for inspection.
fn decode_lane_relay_envelope_json_py(envelope: &[u8]) -> PyResult<String> {
    let mut slice = envelope;
    let parsed = LaneRelayEnvelope::decode_all(&mut slice)
        .map_err(|err| PyValueError::new_err(format!("failed to decode relay envelope: {err}")))?;
    let value = norito::json::to_value(&parsed)
        .map_err(|err| PyValueError::new_err(format!("failed to encode envelope JSON: {err}")))?;
    norito::json::to_string_pretty(&value)
        .map_err(|err| PyValueError::new_err(format!("failed to encode envelope JSON: {err}")))
}

#[pyfunction]
/// Compute the settlement hash for a JSON `LaneBlockCommitment`.
fn lane_settlement_hash_py(settlement_json: &str) -> PyResult<String> {
    let commitment: LaneBlockCommitment = norito::json::from_str(settlement_json)
        .map_err(|err| PyValueError::new_err(format!("invalid settlement JSON: {err}")))?;
    let hash = compute_settlement_hash(&commitment)
        .map_err(|err| PyValueError::new_err(format!("failed to hash settlement: {err}")))?;
    Ok(hex_encode_upper(hash.as_ref()))
}

const PRIVACY_FFI_VERSION_V1: u32 = 1;
const PRIVACY_FFI_STATUS_ERROR: u32 = 1;
const PRIVACY_FFI_ERROR_MALFORMED_NORITO: u32 = 2;
const PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM: u32 = 3;
const PRIVACY_FFI_ERROR_PRODUCTION_DISABLED: u32 = 4;
const PRIVACY_FFI_ERROR_INVALID_REQUEST: u32 = 5;
const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;
const PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES: usize = 1024;
const PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES: usize = 1024 * 1024;
const PRIVACY_REQUEST_WITNESS_MAX_BYTES: usize = PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2;
const PRIVACY_REQUEST_PROOF_MAX_BYTES: usize = PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2;
const PRIVACY_NORITO_SCHEMA_START: usize = 6;
const PRIVACY_NORITO_SCHEMA_END: usize = 22;
const PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE: u8 = 0x50;
const PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE: u8 = 0x42;
const PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE: u8 = 0x56;
const PRIVACY_REQUEST_SCHEMA_BYTE: u8 = 0x52;
const PRIVACY_PRODUCTION_GATE_VERSION: &str = "privacy-production-gate-v1";
const PRIVACY_PRODUCTION_GATE_MISSING_ENGINE: &str =
    "real protocol engine is not production-enabled";
const PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST: &str =
    "Iroha production allowlist is not enabled for this audited row";
const PRIVACY_PRODUCTION_DISABLED_MESSAGE: &str = "privacy production is disabled until exact protocol implementation, real proving, real verification, chain admission, cross-SDK parity, wallet/state support, deterministic tests, fuzzing, performance gates, external audit, real protocol engine enablement, and Iroha production allowlist evidence all pass";
#[cfg(test)]
const PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE: &[u8] =
    b"iroha-privacy-native-availability-probe-v1";

fn privacy_request_archive_out_of_bounds(len: usize) -> bool {
    len == 0 || len > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES
}

fn privacy_archive_has_repeated_schema_byte(bytes: &[u8], schema_byte: u8) -> bool {
    bytes
        .get(PRIVACY_NORITO_SCHEMA_START..PRIVACY_NORITO_SCHEMA_END)
        .is_some_and(|schema| schema.iter().all(|byte| *byte == schema_byte))
}

fn privacy_patch_archive_schema_hash(bytes: &mut [u8], schema_hash: [u8; 16]) -> bool {
    let Some(schema) = bytes.get_mut(PRIVACY_NORITO_SCHEMA_START..PRIVACY_NORITO_SCHEMA_END) else {
        return false;
    };
    schema.copy_from_slice(&schema_hash);
    true
}

fn privacy_patch_archive_repeated_schema_byte(bytes: &mut [u8], schema_byte: u8) -> bool {
    let Some(schema) = bytes.get_mut(PRIVACY_NORITO_SCHEMA_START..PRIVACY_NORITO_SCHEMA_END) else {
        return false;
    };
    schema.fill(schema_byte);
    true
}

fn privacy_result_schema_byte(operation: PrivacyProofOperationV1) -> u8 {
    match operation {
        PrivacyProofOperationV1::Build => PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE,
        PrivacyProofOperationV1::Verify => PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE,
    }
}

const PRIVACY_PRODUCTION_GATE_REQUIREMENTS: &[(&str, &str)] = &[
    ("real_proving", "real proving engine is not registered"),
    ("real_verification", "real verifier is not registered"),
    ("chain_admission", "chain admission path is not enabled"),
    ("sdk_parity", "cross-SDK parity is incomplete"),
    ("wallet_state", "wallet/state support is incomplete"),
    ("deterministic_tests", "deterministic tests are incomplete"),
    ("fuzzing", "fuzzing gate is incomplete"),
    ("performance_gates", "performance gate is incomplete"),
    ("external_audit", "external audit signoff is missing"),
];

const PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS: &[(&str, &str, &str)] = &[
    (
        "anonymous-pgc-k-out-of-n-v1",
        "anonymous-pgc-k-out-of-n",
        "anonymous-pgc",
    ),
    (
        "verange-transparent-range-v1",
        "verange-transparent-range",
        "verange",
    ),
    (
        "zkat-policy-private-auth-v1",
        "zkat-policy-private-authenticator",
        "zkat",
    ),
    (
        "zk-ams-recursive-admission-v0",
        "recursive-anonymous-admission",
        "recursive-anonymous-admission",
    ),
    (
        "vega-existing-credential-zk-v0",
        "existing-credential-zk",
        "vega-existing-credential-zk",
    ),
    (
        "silent-threshold-anoncred-v0",
        "threshold-anonymous-credentials",
        "silent-threshold-anoncred",
    ),
    (
        "zk-x509-onchain-identity-v0",
        "zkvm-x509-identity",
        "zk-x509",
    ),
    (
        "jindo-lattice-pcs-zk-v0",
        "lattice-polynomial-commitment",
        "lattice-pcs-sis",
    ),
    (
        "sis-hints-anoncred-pq-v0",
        "lattice-anonymous-credentials",
        "sis-with-hints",
    ),
    (
        "zk-ace-pq-authorization-v0",
        "stark/fri/sha256-goldilocks",
        "stark-fri",
    ),
    (
        "orchard-halo2-actions-v1",
        "halo2-pasta-action-bundle",
        "halo2-ipa-orchard",
    ),
    (
        "penumbra-masp-v1",
        "groth16-bls12-377-decaf377",
        "groth16-bls12-377",
    ),
    (
        "monero-fcmp-plus-plus-v1",
        "fcmp-plus-plus-curve-trees-bulletproofs",
        "fcmp-plus-plus-curve-tree",
    ),
    (
        "miden-stark-note-v1",
        "stark-vm-note-transaction",
        "miden-stark",
    ),
    (
        "aztec-private-rollup-v1",
        "plonkish-private-kernel-rollup",
        "aztec-plonkish-private-kernel",
    ),
    ("pq-masp-stark-v0", "stark-fri", "pq-masp-stark-fri"),
];

const PRIVACY_COMPONENT_ALGORITHM_IDS: &[&str] = &["verange-transparent-range-v1"];
const PRIVACY_RESEARCH_TARGET_ALGORITHM_IDS: &[&str] = &[
    "orchard-halo2-actions-v1",
    "penumbra-masp-v1",
    "monero-fcmp-plus-plus-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
    "pq-masp-stark-v0",
];
const PRIVACY_EXPOSED_PRODUCTION_CLAIM_FRAGMENTS: &[&str] = &[
    "productionready",
    "productionhardened",
    "productionenabled",
    "productionapproved",
    "productioncertified",
    "productionclaim",
    "claimedproduction",
    "mainnetready",
    "mainnetcomplete",
    "mainnetclaim",
    "claimedmainnet",
    "auditedproduction",
    "externallyaudited",
    "auditpassed",
    "auditapproved",
    "auditsignoff",
    "auditclaim",
    "claimedaudit",
    "securityreviewpassed",
];

#[derive(Clone, Copy)]
struct PrivacyAlgorithmEntry {
    id: &'static str,
    proof_family: &'static str,
    backend_family: &'static str,
    sdk_entrypoints: &'static [&'static str],
    planned_entrypoints: &'static [&'static str],
}

const PRIVACY_ALGORITHM_ENTRIES: &[PrivacyAlgorithmEntry] = &[
    PrivacyAlgorithmEntry {
        id: "transparent-transfer",
        proof_family: "none",
        backend_family: "none",
        sdk_entrypoints: &[
            "buildTransferAssetInstruction",
            "buildTransaction",
            "submitSignedTransaction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "shield",
        proof_family: "commitment-only",
        backend_family: "commitment-only",
        sdk_entrypoints: &[
            "buildShieldInstruction",
            "buildTransaction",
            "submitSignedTransaction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "confidential-transfer-v2",
        proof_family: "halo2-ipa-pasta",
        backend_family: "halo2-ipa-pasta",
        sdk_entrypoints: &[
            "buildConfidentialTransferProofV2",
            "buildZkTransferInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "unshield",
        proof_family: "halo2-ipa-pasta",
        backend_family: "halo2-ipa-pasta",
        sdk_entrypoints: &[
            "buildConfidentialUnshieldProofV3",
            "buildUnshieldInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "asset-hidden-confidential-transfer-v1",
        proof_family: "halo2-ipa-pasta",
        backend_family: "halo2-ipa-pasta",
        sdk_entrypoints: &[
            "buildRegisterAssetHiddenZkPoolInstruction",
            "buildAssetHiddenZkTransferInstruction",
        ],
        planned_entrypoints: &["buildConfidentialAssetHiddenTransferProofV1"],
    },
    PrivacyAlgorithmEntry {
        id: "zk-ace-pq-authorization-v0",
        proof_family: "stark/fri/sha256-goldilocks",
        backend_family: "stark-fri",
        sdk_entrypoints: &[
            "buildRegisterZkAceIdentityCommitmentInstruction",
            "buildRotateZkAceIdentityCommitmentInstruction",
            "buildRevokeZkAceIdentityCommitmentInstruction",
            "buildZkAceAuthorizedTransferInstruction",
            "buildZkAceAuthorizationProofV1",
        ],
        planned_entrypoints: &[
            "buildShieldedZkAceAuthorizationProofV1",
            "buildShieldedZkAceAuthorizedTransferInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "anonymous-pgc-k-out-of-n-v1",
        proof_family: "anonymous-pgc-k-out-of-n",
        backend_family: "anonymous-pgc",
        sdk_entrypoints: &[
            "buildAnonymousPgcReceiverSet",
            "buildAnonymousPgcDevProofFixture",
            "verifyAnonymousPgcDevProofLocally",
        ],
        planned_entrypoints: &[
            "buildAnonymousPgcAccountCommitmentInstruction",
            "buildAnonymousPgcKOutOfNProofV1",
            "buildAnonymousPgcTransferInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "verange-transparent-range-v1",
        proof_family: "verange-transparent-range",
        backend_family: "verange",
        sdk_entrypoints: &[
            "buildRangeCommitment",
            "buildVeRangeDevProofFixture",
            "buildVeRangeProofEnvelope",
            "verifyVeRangeProofLocally",
        ],
        planned_entrypoints: &["buildVeRangeProofV1"],
    },
    PrivacyAlgorithmEntry {
        id: "zkat-policy-private-auth-v1",
        proof_family: "zkat-policy-private-authenticator",
        backend_family: "zkat",
        sdk_entrypoints: &[
            "buildZkAtPolicyCommitment",
            "buildZkAtAuthenticatorEnvelope",
            "buildZkAtDevProofFixture",
            "verifyZkAtAuthenticatorLocally",
        ],
        planned_entrypoints: &[
            "buildZkAtPolicyCommitmentInstruction",
            "buildZkAtPolicyProofV1",
            "buildZkAtAuthorizedTransaction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "zk-ams-recursive-admission-v0",
        proof_family: "recursive-anonymous-admission",
        backend_family: "recursive-anonymous-admission",
        sdk_entrypoints: &[
            "buildZkAmsAdmissionBatch",
            "buildZkAmsAdmissionProofEnvelope",
            "buildZkAmsAdmissionDevProofFixture",
            "verifyZkAmsAdmissionProofLocally",
        ],
        planned_entrypoints: &[
            "buildZkAmsAdmissionBatchProofV0",
            "buildSubmitZkAmsAdmissionBatchInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "vega-existing-credential-zk-v0",
        proof_family: "existing-credential-zk",
        backend_family: "vega-existing-credential-zk",
        sdk_entrypoints: &[
            "buildVegaCredentialPredicateCommitment",
            "buildVegaCredentialProofEnvelope",
            "buildVegaCredentialDevProofFixture",
            "verifyVegaCredentialProofLocally",
        ],
        planned_entrypoints: &[
            "buildVegaCredentialPredicateProofV0",
            "buildSubmitVegaCredentialProofInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "silent-threshold-anoncred-v0",
        proof_family: "threshold-anonymous-credentials",
        backend_family: "silent-threshold-anoncred",
        sdk_entrypoints: &[
            "buildSilentThresholdCredentialCommitments",
            "buildSilentThresholdCredentialEnvelope",
            "buildSilentThresholdCredentialDevProofFixture",
            "verifySilentThresholdCredentialProofLocally",
        ],
        planned_entrypoints: &[
            "buildSilentThresholdCredentialShowingProofV0",
            "buildSubmitSilentThresholdCredentialProofInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "zk-x509-onchain-identity-v0",
        proof_family: "zkvm-x509-identity",
        backend_family: "zk-x509",
        sdk_entrypoints: &[
            "buildZkX509IdentityCommitments",
            "buildZkX509IdentityEnvelope",
            "buildZkX509IdentityDevProofFixture",
            "verifyZkX509IdentityProofLocally",
        ],
        planned_entrypoints: &[
            "buildZkX509IdentityProofV0",
            "buildSubmitZkX509IdentityProofInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "jindo-lattice-pcs-zk-v0",
        proof_family: "lattice-polynomial-commitment",
        backend_family: "lattice-pcs-sis",
        sdk_entrypoints: &[
            "buildJindoLatticePublicInputs",
            "buildJindoLatticeProofEnvelope",
            "buildJindoLatticeDevProofFixture",
            "verifyJindoLatticeProofLocally",
        ],
        planned_entrypoints: &[
            "buildJindoLatticeProofV0",
            "verifyJindoPolynomialCommitmentV0",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "sis-hints-anoncred-pq-v0",
        proof_family: "lattice-anonymous-credentials",
        backend_family: "sis-with-hints",
        sdk_entrypoints: &[
            "buildSisHintsCredentialCommitments",
            "buildSisHintsCredentialEnvelope",
            "buildSisHintsCredentialDevProofFixture",
            "verifySisHintsCredentialProofLocally",
        ],
        planned_entrypoints: &[
            "buildSisHintsAnonymousCredentialProofV0",
            "buildSubmitSisHintsCredentialProofInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "orchard-halo2-actions-v1",
        proof_family: "halo2-pasta-action-bundle",
        backend_family: "halo2-ipa-orchard",
        sdk_entrypoints: &[],
        planned_entrypoints: &[
            "buildOrchardActionBundleProofV1",
            "buildOrchardActionBundleInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "penumbra-masp-v1",
        proof_family: "groth16-bls12-377-decaf377",
        backend_family: "groth16-bls12-377",
        sdk_entrypoints: &[],
        planned_entrypoints: &[
            "buildPenumbraSpendProofV1",
            "buildPenumbraOutputProofV1",
            "buildPenumbraShieldedPoolTransaction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "monero-fcmp-plus-plus-v1",
        proof_family: "fcmp-plus-plus-curve-trees-bulletproofs",
        backend_family: "fcmp-plus-plus-curve-tree",
        sdk_entrypoints: &[],
        planned_entrypoints: &[
            "buildFcmpPlusPlusMembershipProofV1",
            "buildFcmpPlusPlusTransferInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "miden-stark-note-v1",
        proof_family: "stark-vm-note-transaction",
        backend_family: "miden-stark",
        sdk_entrypoints: &[],
        planned_entrypoints: &[
            "buildMidenStarkTransactionProofV1",
            "buildMidenNoteTransactionInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "aztec-private-rollup-v1",
        proof_family: "plonkish-private-kernel-rollup",
        backend_family: "aztec-plonkish-private-kernel",
        sdk_entrypoints: &[],
        planned_entrypoints: &[
            "buildAztecPrivateKernelProofV1",
            "buildAztecPrivateRollupTransactionInstruction",
        ],
    },
    PrivacyAlgorithmEntry {
        id: "pq-masp-stark-v0",
        proof_family: "stark-fri",
        backend_family: "pq-masp-stark-fri",
        sdk_entrypoints: &[],
        planned_entrypoints: &[
            "buildPqMaspStarkTransferProofV0",
            "buildPqMaspStarkRegisterPoolInstruction",
            "buildPqMaspStarkTransferInstruction",
            "generateMlDsaKeyPair",
            "encapsulateMlKem",
        ],
    },
];

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyProductionGateStatusV1 {
    key: String,
    passed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyProductionGateV1 {
    version: String,
    ready: bool,
    gates: Vec<PrivacyProductionGateStatusV1>,
    missing: Vec<String>,
    audit_references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyCapabilityV1 {
    algorithm_id: String,
    proof_family: String,
    backend_family: String,
    sdk_entrypoints: Vec<String>,
    planned_entrypoints: Vec<String>,
    production_ready: bool,
    production_gate: PrivacyProductionGateV1,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyCapabilitiesV1 {
    version: u32,
    gate_version: String,
    algorithms: Vec<PrivacyCapabilityV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyProofRequestV1 {
    algorithm_id: String,
    entrypoint: String,
    vk_ref: String,
    public_inputs: Vec<u8>,
    witness: Vec<u8>,
    proof: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyProofResultV1 {
    version: u32,
    status: u32,
    error_code: u32,
    message: String,
    algorithm_id: String,
    entrypoint: String,
    vk_ref: String,
    public_inputs: Vec<u8>,
    proof: Vec<u8>,
    verified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrivacyProofOperationV1 {
    Build,
    Verify,
}

fn privacy_production_gate() -> PrivacyProductionGateV1 {
    PrivacyProductionGateV1 {
        version: PRIVACY_PRODUCTION_GATE_VERSION.to_owned(),
        ready: false,
        gates: PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .map(|(key, _)| PrivacyProductionGateStatusV1 {
                key: (*key).to_owned(),
                passed: false,
            })
            .collect(),
        missing: PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .map(|(_, label)| (*label).to_owned())
            .chain(
                [
                    "real protocol engine is not production-enabled",
                    "Iroha production allowlist is not enabled for this audited row",
                ]
                .into_iter()
                .map(str::to_owned),
            )
            .collect(),
        audit_references: Vec::new(),
    }
}

fn privacy_capabilities() -> PrivacyCapabilitiesV1 {
    debug_assert!(privacy_algorithm_catalog_invariants_hold());
    let capabilities = PrivacyCapabilitiesV1 {
        version: PRIVACY_FFI_VERSION_V1,
        gate_version: PRIVACY_PRODUCTION_GATE_VERSION.to_owned(),
        algorithms: PRIVACY_ALGORITHM_ENTRIES
            .iter()
            .map(|entry| PrivacyCapabilityV1 {
                algorithm_id: entry.id.to_owned(),
                proof_family: entry.proof_family.to_owned(),
                backend_family: entry.backend_family.to_owned(),
                sdk_entrypoints: entry
                    .sdk_entrypoints
                    .iter()
                    .map(|entrypoint| (*entrypoint).to_owned())
                    .collect(),
                planned_entrypoints: entry
                    .planned_entrypoints
                    .iter()
                    .map(|entrypoint| (*entrypoint).to_owned())
                    .collect(),
                production_ready: false,
                production_gate: privacy_production_gate(),
            })
            .collect(),
    };
    debug_assert!(privacy_capabilities_invariants_hold(&capabilities));
    capabilities
}

fn privacy_algorithm_entry(algorithm_id: &str) -> Option<&'static PrivacyAlgorithmEntry> {
    PRIVACY_ALGORITHM_ENTRIES
        .iter()
        .find(|entry| entry.id == algorithm_id)
}

fn privacy_entrypoint_supported(entry: &PrivacyAlgorithmEntry, entrypoint: &str) -> bool {
    entry.sdk_entrypoints.contains(&entrypoint)
}

fn privacy_entrypoint_planned(entry: &PrivacyAlgorithmEntry, entrypoint: &str) -> bool {
    entry.planned_entrypoints.contains(&entrypoint)
}

fn privacy_proof_family_is_portable(label: &str) -> bool {
    !label.is_empty()
        && label.split(['-', '/']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn privacy_string_slice_has_duplicates(values: &[&'static str]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].iter().any(|other| other == value))
}

fn privacy_entrypoints_overlap(left: &[&'static str], right: &[&'static str]) -> bool {
    left.iter()
        .any(|candidate| right.iter().any(|other| other == candidate))
}

fn privacy_entrypoint_name(entrypoint: &str) -> &str {
    entrypoint.rsplit('.').next().unwrap_or(entrypoint)
}

fn privacy_entrypoint_compact_lowercase(entrypoint: &str) -> String {
    entrypoint
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric())
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect()
}

fn privacy_exposed_label_claims_production_readiness(value: &str) -> bool {
    let compact = privacy_entrypoint_compact_lowercase(value);
    PRIVACY_EXPOSED_PRODUCTION_CLAIM_FRAGMENTS
        .iter()
        .any(|fragment| compact.contains(fragment))
}

fn privacy_entrypoint_is_dev_fixture(entrypoint: &str) -> bool {
    let normalized = entrypoint.replace('-', "_").to_ascii_lowercase();
    let compact = privacy_entrypoint_compact_lowercase(entrypoint);
    normalized.contains("devfixture")
        || normalized.contains("dev_fixture")
        || normalized.contains("devprooffixture")
        || normalized.contains("dev_proof_fixture")
        || normalized.contains("fixture")
        || normalized.contains("mock")
        || compact.contains("devfixture")
        || compact.contains("devprooffixture")
        || compact.contains("fixture")
        || compact.contains("mock")
}

fn privacy_entrypoint_is_explicit_dev_fixture(entrypoint: &str) -> bool {
    let normalized = entrypoint.replace('-', "_").to_ascii_lowercase();
    let compact = privacy_entrypoint_compact_lowercase(entrypoint);
    normalized.contains("devfixture")
        || normalized.contains("dev_fixture")
        || normalized.contains("devprooffixture")
        || normalized.contains("dev_proof_fixture")
        || compact.contains("devfixture")
        || compact.contains("devprooffixture")
}

fn privacy_entrypoint_is_local_verifier(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    let lower = name.to_ascii_lowercase();
    lower.starts_with("verify")
        && (lower.ends_with("locally")
            || lower.ends_with("local")
            || lower.contains("localverifier")
            || lower.contains("localonly"))
}

fn privacy_entrypoint_is_instruction_builder(entrypoint: &str) -> bool {
    privacy_entrypoint_name(entrypoint).ends_with("Instruction")
}

fn privacy_entrypoint_is_ledger_mutation(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    name.ends_with("Instruction") || name.ends_with("Transaction") || name.contains("Submit")
}

fn privacy_entrypoint_is_generic_ledger_mutation(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    name == "buildTransaction" || name == "submitSignedTransaction"
}

fn privacy_entrypoint_is_untyped_ledger_mutation(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    privacy_entrypoint_is_ledger_mutation(entrypoint)
        && !name.ends_with("Instruction")
        && !name.ends_with("Transaction")
}

fn privacy_entrypoint_is_proof_helper(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    name.contains("ProofEnvelope")
        || name.contains("ProofWitness")
        || name.contains("ProofPublicInputs")
        || name.contains("ProofRequest")
        || name.contains("ProofCommitment")
}

fn privacy_entrypoint_is_production_proof_builder(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    name.starts_with("build")
        && name.contains("Proof")
        && !privacy_entrypoint_is_instruction_builder(entrypoint)
        && !privacy_entrypoint_is_ledger_mutation(entrypoint)
        && !privacy_entrypoint_is_proof_helper(entrypoint)
        && !privacy_entrypoint_is_dev_fixture(entrypoint)
}

fn privacy_algorithm_entry_is_component(entry: &PrivacyAlgorithmEntry) -> bool {
    PRIVACY_COMPONENT_ALGORITHM_IDS.contains(&entry.id)
}

fn privacy_algorithm_entry_is_research_target(entry: &PrivacyAlgorithmEntry) -> bool {
    PRIVACY_RESEARCH_TARGET_ALGORITHM_IDS.contains(&entry.id)
}

fn privacy_algorithm_entry_is_proofed_privacy(entry: &PrivacyAlgorithmEntry) -> bool {
    entry.proof_family != "none" && entry.proof_family != "commitment-only"
}

fn privacy_entrypoints_include_ledger_mutation(entrypoints: &[&'static str]) -> bool {
    entrypoints
        .iter()
        .any(|entrypoint| privacy_entrypoint_is_ledger_mutation(entrypoint))
}

fn privacy_entrypoints_include_generic_ledger_mutation(entrypoints: &[&'static str]) -> bool {
    entrypoints
        .iter()
        .any(|entrypoint| privacy_entrypoint_is_generic_ledger_mutation(entrypoint))
}

fn privacy_entrypoints_include_untyped_ledger_mutation(entrypoints: &[&'static str]) -> bool {
    entrypoints
        .iter()
        .any(|entrypoint| privacy_entrypoint_is_untyped_ledger_mutation(entrypoint))
}

fn privacy_entrypoints_include_production_proof_builder(entrypoints: &[&'static str]) -> bool {
    entrypoints
        .iter()
        .any(|entrypoint| privacy_entrypoint_is_production_proof_builder(entrypoint))
}

fn privacy_algorithm_entry_invariants_hold(entry: &PrivacyAlgorithmEntry) -> bool {
    let has_local_verifier = entry
        .sdk_entrypoints
        .iter()
        .any(|entrypoint| privacy_entrypoint_is_local_verifier(entrypoint));
    let has_explicit_dev_fixture = entry
        .sdk_entrypoints
        .iter()
        .any(|entrypoint| privacy_entrypoint_is_explicit_dev_fixture(entrypoint));
    let has_sdk_ledger_mutation =
        privacy_entrypoints_include_ledger_mutation(entry.sdk_entrypoints);
    let has_planned_ledger_mutation =
        privacy_entrypoints_include_ledger_mutation(entry.planned_entrypoints);
    let has_production_proof_builder =
        privacy_entrypoints_include_production_proof_builder(entry.sdk_entrypoints)
            || privacy_entrypoints_include_production_proof_builder(entry.planned_entrypoints);
    let proofed_privacy_row = privacy_algorithm_entry_is_proofed_privacy(entry);
    let has_generic_ledger_mutation =
        privacy_entrypoints_include_generic_ledger_mutation(entry.sdk_entrypoints)
            || privacy_entrypoints_include_generic_ledger_mutation(entry.planned_entrypoints);
    let has_untyped_ledger_mutation =
        privacy_entrypoints_include_untyped_ledger_mutation(entry.sdk_entrypoints)
            || privacy_entrypoints_include_untyped_ledger_mutation(entry.planned_entrypoints);

    privacy_algorithm_id_is_portable(entry.id)
        && privacy_proof_family_is_portable(entry.proof_family)
        && privacy_vk_ref_backend_family_is_portable(entry.backend_family)
        && !privacy_exposed_label_claims_production_readiness(entry.id)
        && !privacy_exposed_label_claims_production_readiness(entry.proof_family)
        && !privacy_exposed_label_claims_production_readiness(entry.backend_family)
        && privacy_catalog_vk_ref_name_is_registered(entry)
        && entry
            .sdk_entrypoints
            .iter()
            .all(|entrypoint| privacy_sdk_entrypoint_is_portable(entrypoint))
        && entry
            .planned_entrypoints
            .iter()
            .all(|entrypoint| privacy_sdk_entrypoint_is_portable(entrypoint))
        && entry
            .sdk_entrypoints
            .iter()
            .all(|entrypoint| !privacy_exposed_label_claims_production_readiness(entrypoint))
        && entry
            .planned_entrypoints
            .iter()
            .all(|entrypoint| !privacy_exposed_label_claims_production_readiness(entrypoint))
        && !privacy_string_slice_has_duplicates(entry.sdk_entrypoints)
        && !privacy_string_slice_has_duplicates(entry.planned_entrypoints)
        && !privacy_entrypoints_overlap(entry.sdk_entrypoints, entry.planned_entrypoints)
        && entry.planned_entrypoints.iter().all(|entrypoint| {
            !privacy_entrypoint_is_dev_fixture(entrypoint)
                && !privacy_entrypoint_is_local_verifier(entrypoint)
        })
        && entry.sdk_entrypoints.iter().all(|entrypoint| {
            !privacy_entrypoint_is_dev_fixture(entrypoint)
                || privacy_entrypoint_is_explicit_dev_fixture(entrypoint)
        })
        && (!has_local_verifier || has_explicit_dev_fixture)
        && (!has_explicit_dev_fixture || has_local_verifier)
        && (!has_explicit_dev_fixture
            || privacy_entrypoints_include_production_proof_builder(entry.planned_entrypoints))
        && (!has_planned_ledger_mutation || has_production_proof_builder)
        && (!proofed_privacy_row || !has_sdk_ledger_mutation || has_production_proof_builder)
        && (!proofed_privacy_row || !has_generic_ledger_mutation)
        && (!proofed_privacy_row || !has_untyped_ledger_mutation)
        && (!privacy_algorithm_entry_is_component(entry)
            || (!privacy_entrypoints_include_ledger_mutation(entry.sdk_entrypoints)
                && !privacy_entrypoints_include_ledger_mutation(entry.planned_entrypoints)))
        && (!privacy_algorithm_entry_is_research_target(entry) || entry.sdk_entrypoints.is_empty())
}

fn privacy_algorithm_catalog_entries_are_valid(entries: &[PrivacyAlgorithmEntry]) -> bool {
    entries.iter().all(privacy_algorithm_entry_invariants_hold)
        && !privacy_algorithm_catalog_vk_ref_names_have_duplicates(entries)
        && entries.iter().enumerate().all(|(index, entry)| {
            !entries[index + 1..]
                .iter()
                .any(|other| other.id == entry.id)
        })
}

fn privacy_algorithm_catalog_vk_ref_names_have_duplicates(
    entries: &[PrivacyAlgorithmEntry],
) -> bool {
    entries.iter().enumerate().any(|(index, entry)| {
        let name = privacy_catalog_vk_ref_name(entry);
        entries[index + 1..]
            .iter()
            .any(|other| privacy_catalog_vk_ref_name(other) == name)
    })
}

fn privacy_required_production_plan_rows_are_present(entries: &[PrivacyAlgorithmEntry]) -> bool {
    PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS.iter().all(
        |(algorithm_id, proof_family, backend_family)| {
            let mut matching_rows = entries.iter().filter(|entry| entry.id == *algorithm_id);
            matches!(
                (matching_rows.next(), matching_rows.next()),
                (Some(entry), None)
                    if entry.proof_family == *proof_family
                        && entry.backend_family == *backend_family
                        && privacy_entrypoints_include_production_proof_builder(
                            entry.planned_entrypoints,
                        )
            )
        },
    )
}

fn privacy_algorithm_catalog_invariants_hold() -> bool {
    privacy_algorithm_catalog_entries_are_valid(PRIVACY_ALGORITHM_ENTRIES)
        && privacy_required_production_plan_rows_are_present(PRIVACY_ALGORITHM_ENTRIES)
}

fn privacy_string_vec_has_duplicates(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].iter().any(|other| other == value))
}

fn privacy_string_vec_matches_slice(values: &[String], expected: &[&'static str]) -> bool {
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected.iter())
            .all(|(value, expected)| value.as_str() == *expected)
}

fn privacy_string_vecs_overlap(left: &[String], right: &[String]) -> bool {
    left.iter()
        .any(|candidate| right.iter().any(|other| other == candidate))
}

fn privacy_gate_status_keys_have_duplicates(gates: &[PrivacyProductionGateStatusV1]) -> bool {
    gates.iter().enumerate().any(|(index, status)| {
        gates[index + 1..]
            .iter()
            .any(|other| other.key.as_str() == status.key.as_str())
    })
}

fn privacy_production_gate_key_is_required(key: &str) -> bool {
    PRIVACY_PRODUCTION_GATE_REQUIREMENTS
        .iter()
        .any(|(required_key, _)| key == *required_key)
}

fn privacy_production_gate_missing_reason_is_required(missing: &str) -> bool {
    PRIVACY_PRODUCTION_GATE_REQUIREMENTS
        .iter()
        .any(|(_, label)| missing == *label)
        || missing == PRIVACY_PRODUCTION_GATE_MISSING_ENGINE
        || missing == PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST
}

fn privacy_gate_statuses_match_requirements(gates: &[PrivacyProductionGateStatusV1]) -> bool {
    gates.len() == PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len()
        && gates
            .iter()
            .zip(PRIVACY_PRODUCTION_GATE_REQUIREMENTS.iter())
            .all(|(status, (key, _))| status.key.as_str() == *key && !status.passed)
}

fn privacy_gate_missing_reasons_match_requirements(missing: &[String]) -> bool {
    missing.len() == PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len() + 2
        && missing
            .iter()
            .take(PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len())
            .zip(PRIVACY_PRODUCTION_GATE_REQUIREMENTS.iter())
            .all(|(missing, (_, label))| missing.as_str() == *label)
        && missing[PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len()].as_str()
            == PRIVACY_PRODUCTION_GATE_MISSING_ENGINE
        && missing[PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len() + 1].as_str()
            == PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST
}

fn privacy_production_gate_invariants_hold(gate: &PrivacyProductionGateV1) -> bool {
    gate.version == PRIVACY_PRODUCTION_GATE_VERSION
        && !gate.ready
        && gate.audit_references.is_empty()
        && gate.gates.len() == PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len()
        && gate.missing.len() == PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len() + 2
        && privacy_gate_statuses_match_requirements(&gate.gates)
        && privacy_gate_missing_reasons_match_requirements(&gate.missing)
        && !privacy_gate_status_keys_have_duplicates(&gate.gates)
        && !privacy_string_vec_has_duplicates(&gate.missing)
        && gate.gates.iter().all(|status| {
            privacy_text_field_is_portable_identifier(&status.key)
                && privacy_production_gate_key_is_required(&status.key)
                && !status.passed
        })
        && gate
            .missing
            .iter()
            .all(|missing| privacy_production_gate_missing_reason_is_required(missing))
        && PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .all(|(key, label)| {
                gate.gates
                    .iter()
                    .any(|status| status.key.as_str() == *key && !status.passed)
                    && gate
                        .missing
                        .iter()
                        .any(|missing| missing.as_str() == *label)
            })
        && gate
            .missing
            .iter()
            .any(|missing| missing == PRIVACY_PRODUCTION_GATE_MISSING_ENGINE)
        && gate
            .missing
            .iter()
            .any(|missing| missing == PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST)
}

fn privacy_capability_rows_match_catalog_order(algorithms: &[PrivacyCapabilityV1]) -> bool {
    algorithms.len() == PRIVACY_ALGORITHM_ENTRIES.len()
        && algorithms
            .iter()
            .zip(PRIVACY_ALGORITHM_ENTRIES.iter())
            .all(|(algorithm, entry)| algorithm.algorithm_id.as_str() == entry.id)
}

fn privacy_capability_invariants_hold(capability: &PrivacyCapabilityV1) -> bool {
    let Some(entry) = privacy_algorithm_entry(&capability.algorithm_id) else {
        return false;
    };

    privacy_algorithm_id_is_portable(&capability.algorithm_id)
        && !privacy_exposed_label_claims_production_readiness(&capability.algorithm_id)
        && capability.proof_family.as_str() == entry.proof_family
        && privacy_proof_family_is_portable(&capability.proof_family)
        && !privacy_exposed_label_claims_production_readiness(&capability.proof_family)
        && capability.backend_family.as_str() == entry.backend_family
        && privacy_vk_ref_backend_family_is_portable(&capability.backend_family)
        && !privacy_exposed_label_claims_production_readiness(&capability.backend_family)
        && privacy_string_vec_matches_slice(&capability.sdk_entrypoints, entry.sdk_entrypoints)
        && privacy_string_vec_matches_slice(
            &capability.planned_entrypoints,
            entry.planned_entrypoints,
        )
        && capability
            .sdk_entrypoints
            .iter()
            .all(|entrypoint| privacy_sdk_entrypoint_is_portable(entrypoint))
        && capability
            .sdk_entrypoints
            .iter()
            .all(|entrypoint| !privacy_exposed_label_claims_production_readiness(entrypoint))
        && capability
            .planned_entrypoints
            .iter()
            .all(|entrypoint| privacy_sdk_entrypoint_is_portable(entrypoint))
        && capability
            .planned_entrypoints
            .iter()
            .all(|entrypoint| !privacy_exposed_label_claims_production_readiness(entrypoint))
        && !privacy_string_vec_has_duplicates(&capability.sdk_entrypoints)
        && !privacy_string_vec_has_duplicates(&capability.planned_entrypoints)
        && !privacy_string_vecs_overlap(
            &capability.sdk_entrypoints,
            &capability.planned_entrypoints,
        )
        && !capability.production_ready
        && privacy_production_gate_invariants_hold(&capability.production_gate)
}

fn privacy_capabilities_invariants_hold(capabilities: &PrivacyCapabilitiesV1) -> bool {
    capabilities.version == PRIVACY_FFI_VERSION_V1
        && capabilities.gate_version == PRIVACY_PRODUCTION_GATE_VERSION
        && capabilities.algorithms.len() == PRIVACY_ALGORITHM_ENTRIES.len()
        && privacy_capability_rows_match_catalog_order(&capabilities.algorithms)
        && capabilities
            .algorithms
            .iter()
            .all(privacy_capability_invariants_hold)
        && capabilities
            .algorithms
            .iter()
            .enumerate()
            .all(|(index, algorithm)| {
                !capabilities.algorithms[index + 1..]
                    .iter()
                    .any(|other| other.algorithm_id.as_str() == algorithm.algorithm_id.as_str())
            })
}

fn privacy_request_text_fields(request: &PrivacyProofRequestV1) -> [&str; 3] {
    [&request.algorithm_id, &request.entrypoint, &request.vk_ref]
}

fn privacy_request_has_oversized_text_field(request: &PrivacyProofRequestV1) -> bool {
    privacy_request_text_fields(request)
        .iter()
        .any(|field| field.len() > PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES)
}

fn privacy_request_has_control_text_field(request: &PrivacyProofRequestV1) -> bool {
    privacy_request_text_fields(request)
        .iter()
        .any(|field| field.chars().any(|ch| ch.is_control()))
}

fn privacy_request_has_non_ascii_text_field(request: &PrivacyProofRequestV1) -> bool {
    privacy_request_text_fields(request)
        .iter()
        .any(|field| !field.is_ascii())
}

fn privacy_text_field_is_portable_identifier(field: &str) -> bool {
    field
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn privacy_vk_ref_backend_family_is_portable(field: &str) -> bool {
    let Some(first) = field.bytes().next() else {
        return false;
    };
    let Some(last) = field.bytes().last() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && (last.is_ascii_lowercase() || last.is_ascii_digit())
        && !field.contains("--")
        && field
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn privacy_vk_ref_name_is_portable(field: &str) -> bool {
    let Some(first) = field.bytes().next() else {
        return false;
    };
    let Some(last) = field.bytes().last() else {
        return false;
    };
    first.is_ascii_lowercase()
        && (last.is_ascii_lowercase() || last.is_ascii_digit())
        && !field.contains("__")
        && field
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn privacy_algorithm_id_is_portable(field: &str) -> bool {
    let Some(first) = field.bytes().next() else {
        return false;
    };
    let Some(last) = field.bytes().last() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && (last.is_ascii_lowercase() || last.is_ascii_digit())
        && field.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn privacy_sdk_entrypoint_is_portable(field: &str) -> bool {
    !field.is_empty()
        && field.split('.').all(|segment| {
            let Some(first) = segment.bytes().next() else {
                return false;
            };
            let Some(last) = segment.bytes().last() else {
                return false;
            };
            first.is_ascii_alphabetic()
                && last.is_ascii_alphanumeric()
                && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn privacy_request_has_unportable_text_field(request: &PrivacyProofRequestV1) -> bool {
    privacy_request_text_fields(request)
        .iter()
        .any(|field| !privacy_text_field_is_portable_identifier(field))
}

fn privacy_request_has_invalid_catalog_shape(request: &PrivacyProofRequestV1) -> bool {
    !privacy_algorithm_id_is_portable(&request.algorithm_id)
        || !privacy_sdk_entrypoint_is_portable(&request.entrypoint)
}

fn privacy_request_has_exposed_production_claim_text_field(
    request: &PrivacyProofRequestV1,
) -> bool {
    privacy_request_text_fields(request)
        .iter()
        .any(|field| privacy_exposed_label_claims_production_readiness(field))
}

fn privacy_vk_ref_parts(vk_ref: &str) -> Option<(&str, &str)> {
    let (backend, name) = vk_ref.split_once(':')?;
    if backend.is_empty() || name.is_empty() || name.contains(':') {
        return None;
    }
    Some((backend, name))
}

fn privacy_vk_ref_is_well_formed(vk_ref: &str) -> bool {
    matches!(
        privacy_vk_ref_parts(vk_ref),
        Some((backend, name))
            if privacy_vk_ref_backend_family_is_portable(backend)
                && privacy_vk_ref_name_is_portable(name)
    )
}

fn privacy_vk_ref_matches_backend(entry: &PrivacyAlgorithmEntry, vk_ref: &str) -> bool {
    matches!(
        privacy_vk_ref_parts(vk_ref),
        Some((backend, _name)) if backend == entry.backend_family
    )
}

fn privacy_catalog_vk_ref_name(entry: &PrivacyAlgorithmEntry) -> &'static str {
    match entry.id {
        "transparent-transfer" => "transparent_transfer",
        "shield" => "shield",
        "confidential-transfer-v2" => "confidential_transfer_v2",
        "unshield" => "confidential_unshield_v3",
        "asset-hidden-confidential-transfer-v1" => "asset_hidden_transfer_v1",
        "zk-ace-pq-authorization-v0" => "zk_ace_pq_authorization_v0",
        "anonymous-pgc-k-out-of-n-v1" => "anonymous_pgc_k_out_of_n_v1",
        "verange-transparent-range-v1" => "verange_transparent_range_v1",
        "zkat-policy-private-auth-v1" => "zkat_policy_private_auth_v1",
        "zk-ams-recursive-admission-v0" => "zk_ams_recursive_admission_v0",
        "vega-existing-credential-zk-v0" => "vega_existing_credential_zk_v0",
        "silent-threshold-anoncred-v0" => "silent_threshold_anoncred_v0",
        "zk-x509-onchain-identity-v0" => "zk_x509_onchain_identity_v0",
        "jindo-lattice-pcs-zk-v0" => "jindo_lattice_pcs_zk_v0",
        "sis-hints-anoncred-pq-v0" => "sis_hints_anoncred_pq_v0",
        "orchard-halo2-actions-v1" => "orchard_halo2_action_bundle_v1",
        "penumbra-masp-v1" => "penumbra_masp_v1",
        "monero-fcmp-plus-plus-v1" => "monero_fcmp_plus_plus_v1",
        "miden-stark-note-v1" => "miden_stark_note_v1",
        "aztec-private-rollup-v1" => "aztec_private_kernel_v1",
        "pq-masp-stark-v0" => "pq_masp_stark_v0",
        _ => "unknown",
    }
}

fn privacy_catalog_vk_ref_name_is_registered(entry: &PrivacyAlgorithmEntry) -> bool {
    let name = privacy_catalog_vk_ref_name(entry);
    name != "unknown" && privacy_vk_ref_name_is_portable(name)
}

fn privacy_canonical_vk_ref_name(entry: &PrivacyAlgorithmEntry) -> String {
    privacy_catalog_vk_ref_name(entry).to_owned()
}

fn privacy_vk_ref_name_matches_algorithm(entry: &PrivacyAlgorithmEntry, vk_ref: &str) -> bool {
    let Some((_backend, name)) = privacy_vk_ref_parts(vk_ref) else {
        return false;
    };
    let expected_name = privacy_canonical_vk_ref_name(entry);
    name == expected_name.as_str()
}

fn privacy_failure_result(
    error_code: u32,
    message: &str,
    request: Option<&PrivacyProofRequestV1>,
) -> PrivacyProofResultV1 {
    let result = PrivacyProofResultV1 {
        version: PRIVACY_FFI_VERSION_V1,
        status: PRIVACY_FFI_STATUS_ERROR,
        error_code,
        message: message.to_owned(),
        algorithm_id: request
            .map(|request| request.algorithm_id.clone())
            .unwrap_or_default(),
        entrypoint: request
            .map(|request| request.entrypoint.clone())
            .unwrap_or_default(),
        vk_ref: request
            .map(|request| request.vk_ref.clone())
            .unwrap_or_default(),
        public_inputs: request
            .map(|request| request.public_inputs.clone())
            .unwrap_or_default(),
        proof: Vec::new(),
        verified: false,
    };
    debug_assert!(privacy_failure_result_invariants_hold(&result));
    result
}

fn privacy_failure_result_invariants_hold(result: &PrivacyProofResultV1) -> bool {
    result.version == PRIVACY_FFI_VERSION_V1
        && result.status == PRIVACY_FFI_STATUS_ERROR
        && result.error_code != 0
        && result.proof.is_empty()
        && !result.verified
}

fn privacy_production_disabled_result(request: &PrivacyProofRequestV1) -> PrivacyProofResultV1 {
    privacy_failure_result(
        PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
        PRIVACY_PRODUCTION_DISABLED_MESSAGE,
        Some(request),
    )
}

fn privacy_result_for_request(
    request: PrivacyProofRequestV1,
    operation: PrivacyProofOperationV1,
) -> PrivacyProofResultV1 {
    if privacy_request_has_oversized_text_field(&request) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request text fields exceed maximum length",
            None,
        );
    }

    if privacy_request_has_control_text_field(&request) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request text fields must not contain control characters",
            None,
        );
    }

    if privacy_request_has_non_ascii_text_field(&request) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request text fields must be printable ASCII",
            None,
        );
    }

    if privacy_request_has_unportable_text_field(&request) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request text fields must use portable identifier characters",
            None,
        );
    }

    if privacy_request_has_exposed_production_claim_text_field(&request) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request text fields must not claim production/mainnet/audit readiness",
            None,
        );
    }

    if request.public_inputs.len() > PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request public_inputs exceeds maximum length",
            None,
        );
    }

    if request.witness.len() > PRIVACY_REQUEST_WITNESS_MAX_BYTES {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request witness exceeds maximum length",
            None,
        );
    }

    if request.proof.len() > PRIVACY_REQUEST_PROOF_MAX_BYTES {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request proof exceeds maximum length",
            None,
        );
    }

    if request.algorithm_id.trim().is_empty() || request.entrypoint.trim().is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request must include non-empty algorithm_id and entrypoint",
            None,
        );
    }

    if privacy_request_has_invalid_catalog_shape(&request) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request algorithm_id and entrypoint must use catalog identifier shapes",
            None,
        );
    }

    if request.vk_ref.trim().is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request must include non-empty vk_ref",
            None,
        );
    }

    if !privacy_vk_ref_is_well_formed(&request.vk_ref) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request vk_ref must use backend:name with portable verifier-key components",
            None,
        );
    }

    let Some(entry) = privacy_algorithm_entry(&request.algorithm_id) else {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
            "unsupported privacy algorithm id",
            Some(&request),
        );
    };

    if privacy_entrypoint_planned(entry, &request.entrypoint) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request entrypoint is planned but not executable until the production gate passes",
            Some(&request),
        );
    }

    if !privacy_entrypoint_supported(entry, &request.entrypoint) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request entrypoint is not registered for the algorithm",
            Some(&request),
        );
    }

    if !privacy_entrypoint_is_production_proof_builder(&request.entrypoint) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request entrypoint must be a production proof builder",
            Some(&request),
        );
    }

    if !privacy_vk_ref_matches_backend(entry, &request.vk_ref) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request vk_ref backend must match algorithm backend family",
            Some(&request),
        );
    }

    if !privacy_vk_ref_name_matches_algorithm(entry, &request.vk_ref) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request vk_ref name must match algorithm verifier key name",
            Some(&request),
        );
    }

    if request.public_inputs.is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof request must include non-empty public_inputs",
            Some(&request),
        );
    }

    if operation == PrivacyProofOperationV1::Build && !request.proof.is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof build request must not include proof bytes",
            Some(&request),
        );
    }

    if operation == PrivacyProofOperationV1::Verify && !request.witness.is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof verify request must not include witness bytes",
            Some(&request),
        );
    }

    if operation == PrivacyProofOperationV1::Build && request.witness.is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof build request must include witness bytes",
            Some(&request),
        );
    }

    if operation == PrivacyProofOperationV1::Verify && request.proof.is_empty() {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
            "privacy proof verify request must include proof bytes",
            Some(&request),
        );
    }

    privacy_production_disabled_result(&request)
}

fn privacy_result_for_request_archive(
    request_archive: &[u8],
    operation: PrivacyProofOperationV1,
) -> PrivacyProofResultV1 {
    if privacy_request_archive_out_of_bounds(request_archive.len()) {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_MALFORMED_NORITO,
            "malformed Norito V1 privacy proof request",
            None,
        );
    }
    let request = privacy_decode_public_request_archive(request_archive);
    match request {
        Ok(request) => privacy_result_for_request(request, operation),
        Err(_) => privacy_failure_result(
            PRIVACY_FFI_ERROR_MALFORMED_NORITO,
            "malformed Norito V1 privacy proof request",
            None,
        ),
    }
}

fn privacy_decode_public_request_archive(
    request_archive: &[u8],
) -> Result<PrivacyProofRequestV1, ()> {
    if !privacy_archive_has_repeated_schema_byte(request_archive, PRIVACY_REQUEST_SCHEMA_BYTE) {
        return Err(());
    }
    let mut normalized = request_archive.to_vec();
    if !privacy_patch_archive_schema_hash(
        &mut normalized,
        <PrivacyProofRequestV1 as norito::NoritoSerialize>::schema_hash(),
    ) {
        return Err(());
    }
    norito::decode_from_bytes(&normalized).map_err(|_| ())
}

fn encode_privacy_archive_py<T>(
    py: Python<'_>,
    value: &T,
    context: &str,
    schema_byte: u8,
) -> PyResult<Py<PyBytes>>
where
    T: norito::NoritoSerialize,
{
    let mut bytes = norito::to_bytes(value)
        .map_err(|err| PyRuntimeError::new_err(format!("{context}: {err}")))?;
    if !privacy_patch_archive_repeated_schema_byte(&mut bytes, schema_byte) {
        return Err(PyRuntimeError::new_err(format!(
            "{context}: encoded privacy archive is missing a Norito schema slot"
        )));
    }
    if bytes.len() > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES {
        return Err(PyRuntimeError::new_err(format!(
            "{context}: encoded privacy archive exceeds {PRIVACY_NATIVE_ARCHIVE_MAX_BYTES} bytes"
        )));
    }
    Ok(Py::from(PyBytes::new(py, &bytes)))
}

#[pyfunction]
#[pyo3(name = "privacy_capabilities_v1")]
fn privacy_capabilities_v1_py(py: Python<'_>) -> PyResult<Py<PyBytes>> {
    encode_privacy_archive_py(
        py,
        &privacy_capabilities(),
        "encode privacy capabilities",
        PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE,
    )
}

#[pyfunction]
#[pyo3(name = "privacy_bridge_abi_version")]
fn privacy_bridge_abi_version_py() -> u32 {
    7
}

#[pyfunction]
#[pyo3(name = "privacy_build_proof_v1")]
fn privacy_build_proof_v1_py(py: Python<'_>, request_archive: &[u8]) -> PyResult<Py<PyBytes>> {
    let result =
        privacy_result_for_request_archive(request_archive, PrivacyProofOperationV1::Build);
    encode_privacy_archive_py(
        py,
        &result,
        "encode privacy proof build result",
        privacy_result_schema_byte(PrivacyProofOperationV1::Build),
    )
}

#[pyfunction]
#[pyo3(name = "privacy_verify_proof_v1")]
fn privacy_verify_proof_v1_py(py: Python<'_>, request_archive: &[u8]) -> PyResult<Py<PyBytes>> {
    let result =
        privacy_result_for_request_archive(request_archive, PrivacyProofOperationV1::Verify);
    encode_privacy_archive_py(
        py,
        &result,
        "encode privacy proof verify result",
        privacy_result_schema_byte(PrivacyProofOperationV1::Verify),
    )
}

#[pymodule]
fn _crypto(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "SorafsMultiFetchError",
        _py.get_type::<SorafsMultiFetchError>(),
    )?;
    module.add_class::<PyDomainId>()?;
    module.add_class::<PyAccountId>()?;
    module.add_class::<PyAssetDefinitionId>()?;
    module.add_class::<PyAssetId>()?;
    module.add_class::<Instruction>()?;
    module.add_class::<TransactionBuilder>()?;
    module.add_class::<SignedTransactionEnvelope>()?;
    module.add_function(wrap_pyfunction!(supported_crypto_algorithms_py, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_crypto_algorithm_py, module)?)?;
    module.add_function(wrap_pyfunction!(generate_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(derive_keypair_from_seed_py, module)?)?;
    module.add_function(wrap_pyfunction!(load_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(sign_py, module)?)?;
    module.add_function(wrap_pyfunction!(verify_py, module)?)?;
    module.add_function(wrap_pyfunction!(public_key_multihash_py, module)?)?;
    module.add_function(wrap_pyfunction!(private_key_multihash_py, module)?)?;
    module.add_function(wrap_pyfunction!(parse_public_key_multihash_py, module)?)?;
    module.add_function(wrap_pyfunction!(parse_private_key_multihash_py, module)?)?;
    module.add_function(wrap_pyfunction!(load_keypair_from_multihash_py, module)?)?;
    module.add_function(wrap_pyfunction!(generate_ed25519_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        derive_ed25519_keypair_from_seed_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(load_ed25519_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(sign_ed25519_py, module)?)?;
    module.add_function(wrap_pyfunction!(verify_ed25519_py, module)?)?;
    module.add_function(wrap_pyfunction!(sm2_default_distid_py, module)?)?;
    module.add_function(wrap_pyfunction!(generate_sm2_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(derive_sm2_keypair_from_seed_py, module)?)?;
    module.add_function(wrap_pyfunction!(load_sm2_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(sm2_public_key_multihash_py, module)?)?;
    module.add_function(wrap_pyfunction!(sign_sm2_py, module)?)?;
    module.add_function(wrap_pyfunction!(verify_sm2_py, module)?)?;
    module.add_function(wrap_pyfunction!(hash_blake2b_32_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        verify_signed_transaction_versioned_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(lane_relay_envelope_fixture_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        verify_lane_relay_envelope_bytes_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        decode_lane_relay_envelope_json_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(lane_settlement_hash_py, module)?)?;
    module.add_function(wrap_pyfunction!(derive_confidential_keyset_py, module)?)?;
    module.add_function(wrap_pyfunction!(sm2_fixture_from_seed_py, module)?)?;
    module.add_function(wrap_pyfunction!(encode_connect_frame_py, module)?)?;
    module.add_function(wrap_pyfunction!(decode_connect_frame_py, module)?)?;
    module.add_function(wrap_pyfunction!(generate_connect_keypair_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        connect_public_key_from_private_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(derive_connect_direction_keys_py, module)?)?;
    module.add_function(wrap_pyfunction!(build_connect_approve_preimage_py, module)?)?;
    module.add_function(wrap_pyfunction!(seal_connect_payload_py, module)?)?;
    module.add_function(wrap_pyfunction!(open_connect_payload_py, module)?)?;
    module.add_function(wrap_pyfunction!(sorafs_alias_policy_defaults_py, module)?)?;
    module.add_function(wrap_pyfunction!(sorafs_evaluate_alias_proof_py, module)?)?;
    module.add_function(wrap_pyfunction!(sorafs_alias_proof_fixture_py, module)?)?;
    module.add_function(wrap_pyfunction!(sorafs_multi_fetch_local_py, module)?)?;
    module.add_function(wrap_pyfunction!(sorafs_gateway_fetch_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        sorafs_decode_replication_order_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        decode_transaction_receipt_json_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        zk_ace_build_transfer_authorization_v1_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_prove_verified_compact_payment_token_with_records_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_verify_recursive_compact_payment_token_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_bridge_abi_version_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(kagemusha_recursive_spend_init_py, module)?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_append_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_transition_profile_init_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_transition_profile_append_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_lineage_append_boundary_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_lineage_witness_from_init_result_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_lineage_witness_append_result_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_verify_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        kagemusha_recursive_spend_redeem_py,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(privacy_bridge_abi_version_py, module)?)?;
    module.add_function(wrap_pyfunction!(privacy_capabilities_v1_py, module)?)?;
    module.add_function(wrap_pyfunction!(privacy_build_proof_v1_py, module)?)?;
    module.add_function(wrap_pyfunction!(privacy_verify_proof_v1_py, module)?)?;
    module.add_function(wrap_pyfunction!(cuda_available_py, module)?)?;
    module.add_function(wrap_pyfunction!(cuda_disabled_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon2_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon2_cuda_many_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon6_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon6_cuda_many_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_add_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_add_cuda_many_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_sub_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_sub_cuda_many_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_mul_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_mul_cuda_many_py, module)?)?;
    module.add(
        "__doc__",
        "Iroha crypto and transaction helpers exposed to Python via PyO3.",
    )?;
    Ok(())
}

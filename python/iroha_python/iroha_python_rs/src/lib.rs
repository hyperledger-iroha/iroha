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
    Algorithm, Hash, HashOf, KeyGenOption, KeyPair, LaneCommitmentId, PrivateKey, PublicKey,
    Signature, derive_keyset_from_slice,
    error::ParseError,
    kex::{KeyExchangeScheme, X25519Sha256},
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature},
};
use iroha_data_model::{
    account::Account,
    asset::{
        definition::AssetConfidentialPolicy,
        prelude::{AssetDefinition, AssetDefinitionId, AssetId, Mintable},
    },
    block::{BlockHeader, consensus::LaneBlockCommitment},
    consensus::ExecutionQcRecord,
    domain::prelude::{Domain, DomainId},
    events::time::{ExecutionTime, Schedule as TimeSchedule, TimeEventFilter},
    isi::{
        Burn, ExecuteTrigger, InstructionBox, Mint, Register, RemoveKeyValue, SetKeyValue,
        Transfer, Unregister,
        repo::{RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementId,
            SettlementLeg, SettlementPlan,
        },
    },
    metadata::Metadata,
    name::Name,
    nexus::{DataSpaceId, LaneId, LanePrivacyProof, LaneRelayEnvelope, compute_settlement_hash},
    nft::NftId,
    prelude::{AccountId, ChainId},
    proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox},
    repo::prelude::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    transaction::{
        Executable, IvmBytecode, SignedTransaction, TransactionBuilder as ModelTransactionBuilder,
    },
    trigger::{
        Trigger, TriggerId,
        action::{Action as TriggerAction, Repeats},
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
    exceptions::{PyException, PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyBytes, PyDict, PyDictMethods, PyList, PyModule, PyStringMethods, PyType},
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
fn parse_err(kind: &str, err: ParseError) -> PyErr {
    PyValueError::new_err(format!("failed to parse {kind} key: {err}"))
}

fn parse_private_key(bytes: &[u8]) -> PyResult<PrivateKey> {
    PrivateKey::from_bytes(Algorithm::Ed25519, bytes).map_err(|err| parse_err("private", err))
}

fn parse_public_key(bytes: &[u8]) -> PyResult<PublicKey> {
    PublicKey::from_bytes(Algorithm::Ed25519, bytes).map_err(|err| parse_err("public", err))
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

fn keypair_to_py(py: Python<'_>, key_pair: KeyPair) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let (pub_alg, public_bytes) = key_pair.public_key().to_bytes();
    algorithm_guard(pub_alg)?;
    let (priv_alg, mut private_bytes) = key_pair.private_key().to_bytes();
    algorithm_guard(priv_alg)?;

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
    AccountId::from_str(value)
        .map_err(|err| PyValueError::new_err(format!("invalid account id: {err}")))
}

fn ensure_ed25519_account(account: &AccountId) -> PyResult<()> {
    let (algorithm, _) = account.signatory().to_bytes();
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
    let (_, signer_bytes) = keypair.public_key().to_bytes();
    let signer: [u8; 32] = signer_bytes
        .try_into()
        .expect("ed25519 public key must be 32 bytes");
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
    allow_single_source_fallback: bool,
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
    metadata.set_item("allow_single_source_fallback", allow_single_source_fallback)?;
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
            path: vec!["payload.bin".to_string()],
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
            path: vec!["payload.bin".to_string()],
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
#[pyo3(name = "generate_connect_keypair")]
/// Generate an X25519 keypair for Connect.
fn generate_connect_keypair_py(py: Python<'_>) -> PyResult<(Py<PyBytes>, Py<PyBytes>)> {
    let scheme = X25519Sha256::new();
    let (public, secret) = scheme.keypair(KeyGenOption::Random);
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
    let frame = connect_sdk::seal_envelope_v1(&key_arr, &sid_arr, dir, sequence, payload);
    let encoded = codec::Encode::encode(&frame);
    Ok(Py::from(PyBytes::new(py, encoded.as_slice())))
}

#[pyfunction]
#[pyo3(name = "open_connect_payload")]
fn open_connect_payload_py(py: Python<'_>, key: &[u8], frame_bytes: &[u8]) -> PyResult<Py<PyDict>> {
    let key_arr = fixed_array::<32>(key, "key")?;
    let frame = decode_connect_frame_bytes(frame_bytes)?;
    let envelope = connect_sdk::open_envelope_v1(&key_arr, &frame).map_err(|err| {
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
    use std::fs;

    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::SigningKey;
    use http::StatusCode;
    use httpmock::{MockServer, prelude::*};
    use ivm::bn254_vec::{self, FieldElem};
    use norito::to_bytes;
    use once_cell::sync::OnceCell;
    use pyo3::{
        Python,
        types::{PyBytes, PyDict, PyList},
    };
    use sorafs_car::multi_fetch::PolicyBlockEvidence;
    use sorafs_manifest::{StreamTokenBodyV1, StreamTokenV1};
    use tempfile::tempdir;

    use super::*;

    fn ensure_python() {
        static INIT: OnceCell<()> = OnceCell::new();
        INIT.get_or_init(|| {
            Python::initialize();
        });
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
    fn attachments_json_decodes_versioned_signed_transaction() {
        ensure_python();
        let signing = SigningKey::from_bytes(&[0x11u8; 32]);
        let private_key =
            parse_private_key(signing.as_bytes()).expect("ed25519 private key parses");
        let public_key = PublicKey::from(private_key.clone());
        let authority = format!("{}@wonderland", public_key);

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
    fn sorafs_gateway_fetch_py_streams_payload() {
        ensure_python();
        let payload: Vec<u8> = (0..4096).map(|idx| (idx as u8).wrapping_mul(11)).collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialise plan");

        let manifest_id_hex = hex::encode([0x24u8; 32]);
        let provider_id_bytes = [0x55u8; 32];
        let provider_id_hex = hex::encode(provider_id_bytes);

        let chunk_specs = plan.chunk_fetch_specs();
        let server = MockServer::start();
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
        let token_body = StreamTokenBodyV1 {
            token_id: "py-gateway-test".to_string(),
            manifest_cid: vec![0x01, 0x55, 0x01],
            provider_id: provider_id_bytes,
            profile_handle: "sorafs.sf1@1.0.0".to_string(),
            max_streams: 4,
            ttl_epoch: 1_900_000_000,
            rate_limit_bytes: 32 * 1024 * 1024,
            issued_at: 1_800_000_000,
            requests_per_minute: 180,
            token_pk_version: 1,
        };
        let stream_token =
            StreamTokenV1::sign(token_body, &signing).expect("sign gateway stream token");
        let stream_token_b64 =
            BASE64_STANDARD.encode(to_bytes(&stream_token).expect("token bytes"));

        let providers = vec![PyGatewayProviderSpec {
            name: "alpha".to_string(),
            provider_id_hex: provider_id_hex.clone(),
            base_url: server.base_url(),
            stream_token_b64,
            privacy_events_url: None,
        }];

        Python::attach(|py| {
            let options = PyGatewayFetchOptions {
                telemetry_region: Some("test-region".to_string()),
                scoreboard_telemetry_label: Some("ci-sdk-python".to_string()),
                max_peers: Some(1),
                retry_budget: Some(2),
                local_proxy: Some(PyLocalProxyOptions {
                    proxy_mode: Some("bridge".to_string()),
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
                "sorafs.sf1@1.0.0",
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
                1
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
            dict.set_item("asset_definition_id", "usd#wonderland")
                .unwrap();
            dict.set_item("quantity", "10").unwrap();
            let leg = parse_repo_cash_leg(py, dict.as_any()).expect("repo cash leg should parse");
            assert_eq!(leg.asset_definition_id.to_string(), "usd#wonderland");
            assert_eq!(leg.quantity.to_string(), "10");

            let missing = PyDict::new(py);
            missing
                .set_item("asset_definition_id", "usd#wonderland")
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

    fn expect_bn254<F>(a: [u64; 4], b: [u64; 4], gpu: Option<[u64; 4]>, fallback: F)
    where
        F: Fn(FieldElem, FieldElem) -> FieldElem,
    {
        let expected = fallback(FieldElem(a), FieldElem(b)).0;
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
            let json_module = py.import("json").map_err(|err| {
                PyValueError::new_err(format!("failed to import json module: {err}"))
            })?;
            let dumped = json_module.call_method1("dumps", (obj,)).map_err(|err| {
                PyValueError::new_err(format!("metadata must be JSON serializable: {err}"))
            })?;
            let dumped: String = dumped
                .extract()
                .map_err(|err| PyValueError::new_err(format!("expected JSON string: {err}")))?;
            json::from_str::<Metadata>(&dumped)
                .map_err(|err| PyValueError::new_err(format!("invalid metadata value: {err}")))
        }
    }
}

fn py_to_json_value(py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<Json> {
    match value {
        None => Ok(Json::default()),
        Some(obj) => {
            let json_module = py.import("json").map_err(|err| {
                PyValueError::new_err(format!("failed to import json module: {err}"))
            })?;
            let dumped = json_module.call_method1("dumps", (obj,)).map_err(|err| {
                PyValueError::new_err(format!(
                    "trigger arguments must be JSON serializable: {err}"
                ))
            })?;
            let dumped: String = dumped
                .extract()
                .map_err(|err| PyValueError::new_err(format!("expected JSON string: {err}")))?;
            Json::from_str_norito(&dumped)
                .map_err(|err| PyValueError::new_err(format!("invalid JSON payload: {err}")))
        }
    }
}

fn parse_numeric(quantity: &str) -> PyResult<Numeric> {
    Numeric::from_str(quantity).map_err(|err| {
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
        let inner = value
            .parse()
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
    fn domain<'py>(&self, py: Python<'py>) -> PyResult<Py<PyDomainId>> {
        domain_id_to_py(py, self.inner.domain())
    }

    #[getter]
    fn public_key_hex(&self) -> PyResult<String> {
        let (algorithm, bytes) = self.inner.signatory().to_bytes();
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

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
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
        let domain_id: DomainId = domain_id.parse().map_err(|err| {
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
        let account_id: AccountId = account_id.parse().map_err(|err| {
            PyValueError::new_err(format!("invalid account id `{account_id}`: {err}"))
        })?;
        ensure_ed25519_account(&account_id)?;
        let metadata = py_to_metadata(py, metadata)?;
        let mut new_account = Account::new(account_id.clone());
        new_account.metadata = metadata;
        let instruction = Register::<Account>::account(new_account);
        Ok(Instruction::new(instruction.into()))
    }

    #[classmethod]
    #[pyo3(signature = (definition_id, owner, *, scale=None, mintable=None, confidential_policy=None, metadata=None))]
    #[allow(clippy::too_many_arguments)] // PyO3 signature mirrors the Python surface and requires explicit keyword params
    fn register_asset_definition_numeric<'py>(
        _cls: &Bound<'py, PyType>,
        py: Python<'py>,
        definition_id: &str,
        owner: &str,
        scale: Option<u32>,
        mintable: Option<&str>,
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

        let instruction = Register::<AssetDefinition>::asset_definition(new_asset);
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
        let destination: AccountId = destination.parse().map_err(|err| {
            PyValueError::new_err(format!(
                "invalid destination account `{destination}`: {err}"
            ))
        })?;
        ensure_ed25519_account(&destination)?;
        let quantity = parse_numeric(quantity)?;
        let instruction = Transfer::asset_numeric(asset_id, quantity, destination);
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
        let domain_id: DomainId = domain_id.parse().map_err(|err| {
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
        verifying_key_bytes: &[u8],
    ) -> PyResult<()> {
        if leaf.len() != 32 {
            return Err(PyValueError::new_err(
                "leaf must be a 32-byte hash (pre-hashed commitment leaf)",
            ));
        }
        if verifying_key_bytes.is_empty() {
            return Err(PyValueError::new_err(
                "verifying_key_bytes must not be empty",
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

        let mut attachment = ProofAttachment::new_inline(
            backend.clone(),
            ProofBox::new(backend.clone(), proof_bytes.to_vec()),
            VerifyingKeyBox::new(backend, verifying_key_bytes.to_vec()),
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

        let signed = builder.sign(&private_key);
        let signature: Signature = signed.signature().payload().clone();
        let signature_bytes = signature.payload().to_vec();

        let hash: HashOf<SignedTransaction> = signed.hash();
        let hash_bytes: [u8; Hash::LENGTH] = *hash.as_ref();

        let signed_bytes = codec::encode_adaptive(&signed);
        let signed_versioned = signed.encode_versioned();

        let (_, public_key_bytes) = signed.authority().signatory().to_bytes();
        let envelope = SignedTransactionEnvelope {
            chain_id: self.chain_id.to_string(),
            authority: self.authority.to_string(),
            signed_transaction: signed_bytes,
            signed_transaction_versioned: signed_versioned,
            hash: hash_bytes,
            signature: signature_bytes,
            public_key: public_key_bytes.to_vec(),
        };

        // Reset instructions for the next transaction while keeping metadata.
        self.instructions.clear();
        self.executable_override = None;
        self.attachments.clear();

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
    let private = Sm2PrivateKey::random(distid, &mut rng);
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
    let _ = parse_sm2_public_key(distid, public_key)?;
    PublicKey::from_bytes(Algorithm::Sm2, public_key)
        .map(|pk| pk.to_string())
        .map_err(|err| PyValueError::new_err(format!("failed to construct SM2 public key: {err}")))
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
    let public_key = PublicKey::from_bytes(Algorithm::Sm2, &public_bytes).map_err(|err| {
        PyValueError::new_err(format!("failed to construct SM2 public key: {err}"))
    })?;
    let multihash = public_key.to_string();
    let prefixed = public_key.to_prefixed_string();
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
/// Subtract two BN254 field elements using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn bn254_sub_cuda_py(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
    ivm::bn254_sub_cuda(a, b)
}

#[pyfunction]
/// Multiply two BN254 field elements using the CUDA backend when available.
///
/// Returns `None` when CUDA support is unavailable or disabled at runtime.
fn bn254_mul_cuda_py(a: [u64; 4], b: [u64; 4]) -> Option<[u64; 4]> {
    ivm::bn254_mul_cuda(a, b)
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
    let qc = ExecutionQcRecord {
        subject_block_hash: header.hash(),
        parent_state_root: Hash::new([0xBA; 4]),
        post_state_root: Hash::new([0xBB; 4]),
        height: header.height().get(),
        view: 1,
        epoch: 0,
        signers_bitmap: vec![0x01],
        bls_aggregate_signature: vec![0xCC; 48],
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
    module.add_function(wrap_pyfunction!(cuda_available_py, module)?)?;
    module.add_function(wrap_pyfunction!(cuda_disabled_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon2_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon2_cuda_many_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon6_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(poseidon6_cuda_many_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_add_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_sub_cuda_py, module)?)?;
    module.add_function(wrap_pyfunction!(bn254_mul_cuda_py, module)?)?;
    module.add(
        "__doc__",
        "Iroha crypto and transaction helpers exposed to Python via PyO3.",
    )?;
    Ok(())
}

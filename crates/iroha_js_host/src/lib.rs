//! Native bindings exposed to the JavaScript SDK.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::implicit_clone,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_lines,
    clippy::unnecessary_wraps
)]

macro_rules! norito_json {
    ({ $($key:literal : $value:expr),* $(,)? }) => {{
        let mut object = norito::json::Map::new();
        $(
            object.insert(
                $key.to_string(),
                norito::json::to_value(&$value)
                    .expect("serialize iroha_js_host JSON payload"),
            );
        )*
        norito::json::Value::Object(object)
    }};
}

use std::{
    collections::{BTreeMap, HashSet},
    convert::{TryFrom, TryInto},
    fmt, fs, mem,
    num::{NonZeroU32, NonZeroU64},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    ptr,
    str::FromStr,
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use blake3::hash as blake3_hash;
use halo2_proofs::{
    halo2curves::{
        ff::PrimeField as _,
        pasta::{EqAffine as Halo2Curve, Fp as Halo2Scalar},
    },
    plonk::{create_proof, keygen_pk, keygen_vk},
    poly::commitment::ParamsProver,
    poly::ipa::{
        commitment::{IPACommitmentScheme, ParamsIPA},
        multiopen::ProverIPA,
    },
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
};
use iroha::da::{
    DaProofConfig as IrohaDaProofConfig,
    generate_da_proof_summary as iroha_generate_da_proof_summary,
};
use iroha_core::soracloud_runtime::{
    HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS, HF_GENERATED_AGENT_LEASE_TICKS,
    build_soracloud_hf_generated_agent_manifest, build_soracloud_hf_generated_service_bundle,
};
use iroha_core::zk::{
    confidential_v2::{
        self, ConfidentialTransferInputV2, ConfidentialTransferOutputV2,
        ConfidentialUnshieldInputV2, ConfidentialUnshieldOutputV3,
    },
    hash_vk as hash_verifying_key_box,
    test_utils::halo2_fixture_envelope,
};
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair, PrivateKey, PublicKey, Signature,
    derive_keyset_from_slice,
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature, encode_sm2_public_key_payload},
};
#[cfg(test)]
use iroha_data_model::da::types::DaRentQuote;
use iroha_data_model::{
    ChainId,
    account::{
        Account, AccountId, NewAccount,
        address::{AccountAddress, AccountAddressError, ChainDiscriminantGuard},
    },
    asset::{
        AssetDefinitionAlias,
        definition::{AssetDefinition, NewAssetDefinition},
        id::{AssetDefinitionId, AssetId},
    },
    block::{
        BlockHeader,
        consensus::{LaneBlockCommitment, PERMISSIONED_TAG},
    },
    consensus::{CertPhase, Qc, QcAggregate, default_chain_order_hash},
    da::manifest::DaManifestV1,
    domain::{Domain, DomainId, NewDomain},
    events::time::{ExecutionTime, Schedule as TimeSchedule, TimeEventFilter},
    isi::{
        Burn, BurnBox, CreateKaigi, CustomInstruction, EndKaigi, ExecuteTrigger, Grant, GrantBox,
        Instruction as InstructionTrait, InstructionBox, JoinKaigi, LeaveKaigi, Mint, MintBox,
        RecordKaigiUsage, Register, RegisterBox, RegisterKaigiRelay, RegisterPeerWithPop,
        RemoveKeyValue, ReportKaigiRelayHealth, SetAssetDefinitionAlias, SetKaigiRelayManifest,
        SetKeyValue, Transfer, TransferBox, Unregister, UnregisterBox,
        governance::{
            CastPlainBallot, CastZkBallot, CouncilDerivationKind, EnactReferendum,
            FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract, RegisterCitizen,
            VotingMode,
        },
        ministry::SubmitAgendaProposal,
        rwa::{
            ForceTransferRwa, FreezeRwa, HoldRwa, MergeRwas, RedeemRwa, RegisterRwa, ReleaseRwa,
            RwaInstructionBox, SetRwaControls, TransferRwa, UnfreezeRwa,
        },
        smart_contract_code::{
            ActivateContractInstance, DeactivateContractInstance, RegisterSmartContractBytes,
            RegisterSmartContractCode, RemoveSmartContractBytes,
        },
        sns::RegisterSnsName,
        social::{CancelTwitterEscrow, ClaimTwitterFollowReward, SendToTwitter},
        zk::{
            CancelConfidentialPolicyTransition, CreateElection, FinalizeElection, RegisterZkAsset,
            ScheduleConfidentialPolicyTransition, Shield, SubmitBallot, Unshield, ZkTransfer,
        },
    },
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiRelayHealthStatus,
        KaigiRelayRegistration, NewKaigi,
    },
    metadata::Metadata,
    ministry::AgendaProposalV1,
    name::Name,
    nexus::{
        AxtDescriptor, AxtDescriptorBuilder, AxtTouchFragment, DataSpaceId, LaneId,
        LaneRelayEnvelope, TouchManifest, compute_descriptor_binding, compute_settlement_hash,
        validate_descriptor,
    },
    nft::{NewNft, Nft, NftId},
    oracle::KeyedHash,
    peer::{Peer, PeerId},
    permission::Permission,
    proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId},
    role::{NewRole, Role, RoleId},
    rwa::{NewRwa, RwaControlPolicy, RwaId, RwaParentRef},
    smart_contract::manifest::{ContractManifest, ManifestProvenance},
    sns::RegisterNameRequestV1,
    soracloud::{
        SecretEnvelopeV1, encode_agent_deploy_provenance_payload,
        encode_bundle_with_materials_provenance_payload,
        encode_hf_shared_lease_join_provenance_payload,
    },
    sorafs::pin_registry::StorageClass,
    transaction::{
        Executable, IvmProved, PrivateCreateKaigi, PrivateEndKaigi, PrivateJoinKaigi,
        PrivateKaigiAction, PrivateKaigiArtifacts, PrivateKaigiFeeSpend, PrivateKaigiTemplate,
        PrivateKaigiTransaction, TransactionSubmissionReceipt,
        signed::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
    },
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
    zk::{ZkAcePublicInputsV1, ZkAceWitnessV1},
};
use iroha_primitives::{
    json::Json,
    numeric::Numeric,
    soradns::{
        GatewayHostBindings, GatewayHostProfile, derive_gateway_hosts,
        derive_gateway_hosts_with_profile,
    },
};
use kaigi_zk::{
    KAIGI_ROSTER_BACKEND, KAIGI_ROSTER_CIRCUIT_K, KaigiRosterJoinCircuit, compute_commitment,
    compute_commitment_bytes, compute_nullifier, compute_nullifier_bytes, empty_roster_root_hash,
    roster_root_limbs,
};
use napi::{
    ValueType,
    bindgen_prelude::{
        BigInt, Buffer, FromNapiValue, ToNapiValue, TypeName, Uint8Array, ValidateNapiValue,
    },
    sys,
};
use napi_derive::napi;
use norito::{
    codec::{DecodeAll, Encode},
    core::{self as norito_core, DecodeFromSlice},
    decode_from_bytes,
    json::{self, JsonDeserialize, Map, Value},
};
use rand_core_06::OsRng;
use sorafs_car::{
    CarBuildPlan, CarChunk, ChunkFetchSpec, ChunkStore, ChunkStoreError, FilePlan, InMemoryPayload,
    PorProof,
    fetch_plan::chunk_fetch_specs_from_json,
    gateway::{GatewayFetchConfig, GatewayProviderInput},
    local_fetch::{
        self, LocalFetchError, LocalFetchOptions, LocalProviderInput, ProviderMetadataInput,
        RangeCapabilityInput, StreamBudgetInput, TelemetryEntryInput, TransportHintInput,
    },
    multi_fetch::{
        self, AttemptError, AttemptFailure, CapabilityMismatch, ChunkVerificationError,
        MultiSourceError,
    },
};
use sorafs_manifest::{
    alias_cache::{AliasCachePolicy, AliasProofState, decode_alias_proof, unix_now_secs},
    capacity::ReplicationOrderV1,
    pin_registry::{
        AliasBindingV1, AliasProofBundleV1, alias_merkle_root, alias_proof_signature_digest,
    },
};
use sorafs_orchestrator::{
    AnonymityPolicy, FetchSession, GatewayOrchestratorError, OrchestratorConfig, OrchestratorError,
    RolloutPhase, TransportPolicy, WriteModeHint, fetch_via_gateway,
    proxy::{
        LocalQuicProxyConfig, ProxyCarBridgeConfig, ProxyKaigiBridgeConfig, ProxyMode,
        ProxyNoritoBridgeConfig,
    },
    taikai_cache::{
        EvictionStats, QosConfig, QosStats, ReliabilityTuning, TaikaiCacheConfig,
        TaikaiCacheStatsSnapshot, TaikaiPullQueueStats, TierStats,
    },
};
use tokio::runtime::Runtime;

const SM2_PRIVATE_KEY_LENGTH: usize = 32;
const SM2_PUBLIC_KEY_LENGTH: usize = 65;
const SM2_SIGNATURE_LENGTH: usize = Sm2Signature::LENGTH;
const KAIGI_ROSTER_PUBLIC_INPUTS_DESC: &[u8] = br#"{"schema":"kaigi_roster_current","inputs":["commitment","nullifier","roster_root_limb0","roster_root_limb1","roster_root_limb2","roster_root_limb3"]}"#;
const ZK1_ENVELOPE_PREFIX: &[u8] = b"ZK1\0";

const SORAFS_ALIAS_POSITIVE_TTL_SECS: u64 = 10 * 60;
const SORAFS_ALIAS_REFRESH_WINDOW_SECS: u64 = 2 * 60;
const SORAFS_ALIAS_HARD_EXPIRY_SECS: u64 = 15 * 60;
const SORAFS_ALIAS_NEGATIVE_TTL_SECS: u64 = 60;
const SORAFS_ALIAS_REVOCATION_TTL_SECS: u64 = 5 * 60;
const SORAFS_ALIAS_ROTATION_MAX_AGE_SECS: u64 = 6 * 60 * 60;
const SORAFS_ALIAS_SUCCESSOR_GRACE_SECS: u64 = 5 * 60;
const SORAFS_ALIAS_GOVERNANCE_GRACE_SECS: u64 = 0;
const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

const SUPPORTED_CRYPTO_ALGORITHMS: &[Algorithm] = &[
    Algorithm::Ed25519,
    Algorithm::Secp256k1,
    Algorithm::BlsNormal,
    Algorithm::BlsSmall,
    Algorithm::MlDsa,
    Algorithm::Gost3410_2012_256ParamSetA,
    Algorithm::Gost3410_2012_256ParamSetB,
    Algorithm::Gost3410_2012_256ParamSetC,
    Algorithm::Gost3410_2012_512ParamSetA,
    Algorithm::Gost3410_2012_512ParamSetB,
    Algorithm::Sm2,
];

fn ensure_packed_struct_disabled() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {});
}

/// Ed25519 key pair returned to JavaScript.
#[napi(object)]
pub struct JsKeyPair {
    /// Algorithm identifier (`"ed25519"`).
    pub algorithm: String,
    /// Raw public key bytes.
    pub public_key: Buffer,
    /// Raw private key bytes (ed25519 seed material).
    pub private_key: Buffer,
    /// Optional distinguishing identifier for algorithms that require it (SM2).
    pub distid: Option<String>,
}

/// Confidential key hierarchy returned to JavaScript callers.
#[napi(object)]
pub struct JsConfidentialKeyset {
    /// Confidential spend key (input seed).
    pub sk_spend: Buffer,
    /// Nullifier key (nk).
    pub nk: Buffer,
    /// Incoming view key (ivk).
    pub ivk: Buffer,
    /// Outgoing view key (ovk).
    pub ovk: Buffer,
    /// Full view key (fvk).
    pub fvk: Buffer,
}

/// Proof artefacts required for a privacy-mode Kaigi join.
#[napi(object)]
pub struct JsKaigiRosterJoinProof {
    /// Commitment digest bound into the Kaigi roster.
    pub commitment: Buffer,
    /// Join nullifier digest used for replay protection.
    pub nullifier: Buffer,
    /// Roster root that the proof binds to.
    pub roster_root: Buffer,
    /// Norito-encoded `OpenVerifyEnvelope` payload.
    pub proof: Buffer,
}

/// Canonical SM2 fixture describing deterministic signing outputs.
#[napi(object)]
pub struct JsSm2Fixture {
    /// Distinguishing identifier used for ZA derivation.
    pub distid: String,
    /// Seed rendered as uppercase hexadecimal.
    pub seed_hex: String,
    /// Message rendered as uppercase hexadecimal.
    pub message_hex: String,
    /// Private key encoded as uppercase hexadecimal.
    pub private_key_hex: String,
    /// SEC1 uncompressed public key (uppercase hex).
    pub public_key_sec1_hex: String,
    /// Multihash string for the SM2 public key.
    pub public_key_multihash: String,
    /// Prefixed multihash string (`"sm2:"` prefix).
    pub public_key_prefixed: String,
    /// ZA (user information hash) in uppercase hexadecimal.
    pub za: String,
    /// Deterministic SM2 signature (r∥s) in uppercase hex.
    pub signature: String,
    /// Canonical `r` component (uppercase hex).
    pub r: String,
    /// Canonical `s` component (uppercase hex).
    pub s: String,
}

/// Optional overrides for `SoraFS` alias cache policy (all values in seconds).
#[napi(object)]
#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::struct_field_names)] // field names mirror the JavaScript property surface
pub struct JsAliasPolicy {
    /// Time-to-live for positively resolved aliases.
    pub positive_ttl_secs: Option<i64>,
    /// Max age before refreshing cached aliases.
    pub refresh_window_secs: Option<i64>,
    /// Hard expiry after which cached aliases are invalid.
    pub hard_expiry_secs: Option<i64>,
    /// Negative cache duration for unresolved aliases.
    pub negative_ttl_secs: Option<i64>,
    /// Time-to-live for revocation records.
    pub revocation_ttl_secs: Option<i64>,
    /// Maximum age before alias rotation is required.
    pub rotation_max_age_secs: Option<i64>,
    /// Grace window applied after a successor manifest is approved.
    pub successor_grace_secs: Option<i64>,
    /// Grace window applied to governance-triggered alias rotations.
    pub governance_grace_secs: Option<i64>,
}

/// Evaluation output exposed to JavaScript callers.
#[napi(object)]
pub struct JsAliasEvaluation {
    /// Current cache state classification.
    pub state: String,
    /// Human-readable status label.
    pub status_label: String,
    /// Whether rotation is due under the cache policy.
    pub rotation_due: bool,
    /// Age of the proof bundle in seconds.
    pub age_seconds: i64,
    /// UNIX timestamp when the bundle was generated.
    pub generated_at_unix: i64,
    /// UNIX timestamp when the bundle expires.
    pub expires_at_unix: i64,
    /// Remaining lifetime in seconds, if calculable.
    pub expires_in_seconds: Option<i64>,
    /// Whether the bundle should be served to clients.
    pub servable: bool,
}

/// Result of parsing an account address string via the shared codec.
#[napi(object)]
pub struct JsAccountAddressParse {
    /// Canonical bytes for the parsed account address.
    pub canonical_bytes: Buffer,
    /// Network prefix inferred while parsing the encoded literal.
    pub network_prefix: Option<u16>,
}

/// Rendered textual encodings for an account address.
#[napi(object)]
pub struct JsAccountAddressRender {
    /// Canonical hexadecimal encoding with `0x` prefix.
    pub canonical_hex: String,
    /// I105 encoding generated with the supplied network prefix.
    pub i105: String,
}

/// Deterministic gateway host bindings exposed to JavaScript callers.
#[napi(object)]
pub struct JsGatewayHosts {
    /// Canonicalised `SoraDNS` FQDN used for hashing.
    pub normalized_name: String,
    /// Blake3 + base32 label that prefixes the canonical host.
    pub canonical_label: String,
    /// Canonical gateway host (`<hash>.gw.sora.id`).
    pub canonical_host: String,
    /// Wildcard pattern that must be present in GAR host lists.
    pub canonical_wildcard: String,
    /// Pretty gateway host (`<fqdn>.gw.sora.name`).
    pub pretty_host: String,
    /// Host patterns the runtime must authorise (canonical, wildcard, pretty).
    pub host_patterns: Vec<String>,
}

/// Fixture options for generating synthetic alias proof bundles.
#[napi(object)]
#[derive(Default)]
pub struct JsAliasProofFixtureOptions {
    /// Alias string placed into the generated bundle.
    pub alias: Option<String>,
    /// Optional manifest CID encoded as hexadecimal.
    pub manifest_cid_hex: Option<String>,
    /// Override for the `generated_at` UNIX timestamp.
    pub generated_at_unix: Option<i64>,
    /// Override for the `expires_at` UNIX timestamp.
    pub expires_at_unix: Option<i64>,
    /// Optional override for the `bound_at` epoch field.
    pub bound_at_epoch: Option<i64>,
    /// Optional override for the expiry epoch value.
    pub expiry_epoch: Option<i64>,
}

/// Alias proof fixture payload returned to JavaScript.
#[napi(object)]
pub struct JsAliasProofFixture {
    /// Generated proof bundle encoded as base64.
    pub proof_b64: String,
    /// Alias name embedded in the fixture.
    pub alias: String,
    /// UNIX timestamp when the proof bundle was generated.
    pub generated_at_unix: i64,
    /// UNIX timestamp when the proof bundle expires.
    pub expires_at_unix: i64,
    /// Hexadecimal encoding of the registry root.
    pub registry_root_hex: String,
    /// Registry tree height encoded into the bundle.
    pub registry_height: i64,
}

/// Assignment entry returned when decoding a replication order.
#[napi(object)]
pub struct JsReplicationAssignment {
    /// Provider identifier encoded as lowercase hex.
    pub provider_id_hex: String,
    /// Capacity slice allocated to the provider (GiB).
    pub slice_gib: i64,
    /// Optional lane hint supplied by governance.
    pub lane: Option<String>,
}

/// SLA parameters returned when decoding a replication order.
#[derive(Clone, Copy)]
#[napi(object)]
pub struct JsReplicationSla {
    /// Ingestion deadline window (seconds).
    pub ingest_deadline_secs: u32,
    /// Minimum availability percentage scaled by 1000.
    pub min_availability_percent_milli: u32,
    /// Minimum `PoR` success percentage scaled by 1000.
    pub min_por_success_percent_milli: u32,
}

/// Metadata entry embedded in a replication order.
#[napi(object)]
pub struct JsReplicationMetadataEntry {
    /// Metadata key.
    pub key: String,
    /// Metadata value.
    pub value: String,
}

/// Result of decoding a Norito-encoded replication order.
#[napi(object)]
pub struct JsReplicationOrder {
    /// Schema version for the replication order.
    pub schema_version: u8,
    /// Order identifier encoded as lowercase hex.
    pub order_id_hex: String,
    /// Manifest CID encoded as UTF-8 when possible.
    pub manifest_cid_utf8: Option<String>,
    /// Manifest CID encoded as base64.
    pub manifest_cid_base64: String,
    /// Canonical manifest digest (lowercase hex).
    pub manifest_digest_hex: String,
    /// Required chunking profile handle.
    pub chunking_profile: String,
    /// Desired redundancy level (number of replicas).
    pub target_replicas: u32,
    /// Provider assignments mandated by the order.
    pub assignments: Vec<JsReplicationAssignment>,
    /// UNIX timestamp when the order was issued.
    pub issued_at_unix: i64,
    /// UNIX timestamp when ingestion must complete.
    pub deadline_at_unix: i64,
    /// Service-level agreement settings.
    pub sla: JsReplicationSla,
    /// Metadata entries attached to the order.
    pub metadata: Vec<JsReplicationMetadataEntry>,
}

/// Derive deterministic `SoraDNS` gateway hosts from an FQDN.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String` for bindings
pub fn soradns_derive_gateway_hosts(fqdn: String) -> napi::Result<JsGatewayHosts> {
    let bindings = derive_gateway_hosts(&fqdn).map_err(|err| {
        napi::Error::from_reason(format!("failed to derive deterministic hosts: {err}"))
    })?;
    js_gateway_hosts_from_bindings(&bindings)
}

/// Derive deterministic `SoraDNS` gateway hosts using a custom pretty suffix.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String` for bindings
pub fn soradns_derive_gateway_hosts_with_pretty_suffix(
    fqdn: String,
    pretty_suffix: String,
) -> napi::Result<JsGatewayHosts> {
    let bindings =
        derive_gateway_hosts_with_profile(&fqdn, GatewayHostProfile::new(&pretty_suffix)).map_err(
            |err| napi::Error::from_reason(format!("failed to derive deterministic hosts: {err}")),
        )?;
    js_gateway_hosts_from_bindings(&bindings)
}

fn js_gateway_hosts_from_bindings(bindings: &GatewayHostBindings) -> napi::Result<JsGatewayHosts> {
    let host_patterns = bindings
        .host_patterns()
        .into_iter()
        .map(str::to_string)
        .collect();
    Ok(JsGatewayHosts {
        normalized_name: bindings.normalized_name().to_string(),
        canonical_label: bindings.canonical_label().to_string(),
        canonical_host: bindings.canonical_host().to_string(),
        canonical_wildcard: GatewayHostBindings::canonical_wildcard().to_string(),
        pretty_host: bindings.pretty_host().to_string(),
        host_patterns,
    })
}

/// Parse an account address string in strict encoded form (canonical I105).
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String` for bindings
pub fn account_address_parse_encoded(
    input: String,
    expected_prefix: Option<u16>,
) -> napi::Result<JsAccountAddressParse> {
    let address =
        AccountAddress::parse_encoded(&input, expected_prefix).map_err(account_address_err)?;
    let canonical_hex = address.canonical_hex().map_err(account_address_err)?;
    let hex_body = canonical_hex
        .strip_prefix("0x")
        .unwrap_or(canonical_hex.as_str());
    let canonical =
        hex::decode(hex_body).map_err(|err| napi::Error::from_reason(err.to_string()))?;
    let network_prefix = Some(
        expected_prefix.unwrap_or_else(iroha_data_model::account::address::chain_discriminant),
    );
    Ok(JsAccountAddressParse {
        canonical_bytes: Buffer::from(canonical),
        network_prefix,
    })
}

/// Render canonical account address bytes into textual encodings.
#[allow(clippy::needless_pass_by_value)] // napi binding prefers owned typed arrays
#[napi]
pub fn account_address_render(
    bytes: Uint8Array,
    network_prefix: u16,
) -> napi::Result<JsAccountAddressRender> {
    let address =
        AccountAddress::from_canonical_bytes(bytes.as_ref()).map_err(account_address_err)?;
    let canonical_hex = address.canonical_hex().map_err(account_address_err)?;
    let i105 = address
        .to_i105_for_discriminant(network_prefix)
        .map_err(account_address_err)?;
    Ok(JsAccountAddressRender {
        canonical_hex,
        i105,
    })
}

fn parse_account_id(input: &str, label: &str) -> napi::Result<AccountId> {
    let raw = input.trim();
    let parsed = match i105_discriminant_hint(raw) {
        Some(discriminant) => AccountAddress::parse_encoded(raw, Some(discriminant))
            .and_then(|address| address.to_account_id())
            .map_err(|err| err.to_string()),
        None => AccountId::parse_encoded(raw)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .map_err(|err| err.to_string()),
    };
    parsed.map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid {label}: {err}"))
    })
}

fn i105_discriminant_hint(input: &str) -> Option<u16> {
    let raw = input.trim();
    if raw.starts_with("sora") {
        return Some(753);
    }
    if raw.starts_with("test") {
        return Some(369);
    }
    if raw.starts_with("dev") {
        return Some(0);
    }
    raw.strip_prefix('n')?.parse::<u16>().ok()
}

fn scoped_chain_discriminant_for_literal(input: &str) -> Option<ChainDiscriminantGuard> {
    i105_discriminant_hint(input).map(ChainDiscriminantGuard::enter)
}

/// Build a canonical public `AssetId` literal from definition/account parts.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String` inputs at the boundary
pub fn encode_asset_id(asset_definition_id: String, account_id: String) -> napi::Result<String> {
    let definition =
        AssetDefinitionId::parse_address_literal(&asset_definition_id).map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("invalid asset definition id: {err}"),
            )
        })?;
    let account = parse_account_id(&account_id, "account id")?;
    Ok(AssetId::new(definition, account).canonical_literal())
}

#[napi(js_name = "blake3Hash")]
/// Compute the BLAKE3-256 digest for the provided payload.
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
pub fn blake3_hash_bytes(payload: Uint8Array) -> napi::Result<Buffer> {
    let digest = blake3_hash(payload.as_ref());
    Ok(Buffer::from(digest.as_bytes().to_vec()))
}

fn parse_zk_ace_verifier_key_id(value: Option<String>) -> napi::Result<VerifyingKeyId> {
    let Some(value) = value else {
        return Ok(zk_ace_prover::zk_ace_verifier_key_id(
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        ));
    };
    let trimmed = value.trim();
    let Some((backend, name)) = trimmed.split_once(':') else {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "verifier_key_id must be in 'backend:name' format",
        ));
    };
    let backend = backend.trim();
    let name = name.trim();
    if backend.is_empty() || name.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "verifier_key_id backend and name must be non-empty",
        ));
    }
    let backend = iroha_schema::Ident::from_str(backend).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid verifier_key_id backend: {err}"),
        )
    })?;
    Ok(VerifyingKeyId::new(backend, name))
}

fn parse_zk_ace_fixed32(value: &Uint8Array, context: &str) -> napi::Result<[u8; 32]> {
    let bytes = value.as_ref();
    if bytes.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be exactly 32 bytes"),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn parse_optional_zk_ace_commitment(value: Option<Uint8Array>) -> napi::Result<[u8; 32]> {
    match value {
        Some(value) => parse_zk_ace_fixed32(&value, "vk_commitment"),
        None => zk_ace_prover::zk_ace_verifying_key_commitment_v1()
            .map_err(|err| norito_to_napi(format!("build ZK-ACE verifier commitment: {err}"))),
    }
}

fn hex32_lower(value: &[u8; 32]) -> String {
    hex::encode(value)
}

fn proof_attachment_to_js_value(attachment: &ProofAttachment) -> napi::Result<Value> {
    let mut vk_ref = Map::new();
    vk_ref.insert(
        "backend".to_owned(),
        Value::String(attachment.vk_ref.backend.as_str().to_owned()),
    );
    vk_ref.insert(
        "name".to_owned(),
        Value::String(attachment.vk_ref.name.clone()),
    );

    let mut proof = Map::new();
    proof.insert(
        "backend".to_owned(),
        Value::String(attachment.backend.as_str().to_owned()),
    );
    proof.insert("verifying_key_ref".to_owned(), Value::Object(vk_ref));
    proof.insert(
        "proof_b64".to_owned(),
        Value::String(STANDARD.encode(&attachment.proof.bytes)),
    );
    if let Some(commitment) = attachment.vk_commitment {
        proof.insert(
            "verifying_key_commitment".to_owned(),
            Value::String(hex32_lower(&commitment)),
        );
    }
    if let Some(envelope_hash) = attachment.envelope_hash {
        proof.insert(
            "envelope_hash".to_owned(),
            Value::String(hex32_lower(&envelope_hash)),
        );
    }
    Ok(Value::Object(proof))
}

fn zk_ace_authorization_to_json(
    public_inputs: &ZkAcePublicInputsV1,
    proof: &ProofAttachment,
    public_inputs_bytes: &[u8],
) -> napi::Result<String> {
    let mut root = Map::new();
    root.insert(
        "public_inputs".to_owned(),
        json::to_value(public_inputs).map_err(norito_to_napi)?,
    );
    root.insert("proof".to_owned(), proof_attachment_to_js_value(proof)?);
    root.insert(
        "identity_commitment".to_owned(),
        Value::String(hex32_lower(&public_inputs.identity_commitment)),
    );
    root.insert(
        "tx_digest".to_owned(),
        Value::String(hex32_lower(&public_inputs.tx_digest)),
    );
    root.insert(
        "replay_nullifier".to_owned(),
        Value::String(hex32_lower(&public_inputs.replay_nullifier)),
    );
    root.insert(
        "policy_hash".to_owned(),
        Value::String(hex32_lower(&public_inputs.policy_hash)),
    );
    root.insert(
        "verifier_key_id".to_owned(),
        Value::String(format!(
            "{}:{}",
            public_inputs.verifier_key_id.backend.as_str(),
            public_inputs.verifier_key_id.name
        )),
    );
    root.insert(
        "authorization_proof_bytes".to_owned(),
        json::to_value(&proof.proof.bytes.len()).map_err(norito_to_napi)?,
    );
    root.insert(
        "authorization_public_input_bytes".to_owned(),
        json::to_value(&public_inputs_bytes.len()).map_err(norito_to_napi)?,
    );
    root.insert(
        "replay_nullifier_bytes".to_owned(),
        json::to_value(&32usize).map_err(norito_to_napi)?,
    );
    json::to_json(&Value::Object(root)).map_err(norito_to_napi)
}

#[napi(js_name = "zkAceBuildAuthorizationProofV1")]
#[allow(clippy::needless_pass_by_value)]
/// Build a ZK-ACE authorization proof from canonical public inputs and private witness JSON.
pub fn zk_ace_build_authorization_proof_v1(
    public_inputs_json: String,
    witness_json: String,
    vk_commitment: Option<Uint8Array>,
) -> napi::Result<String> {
    let public_inputs: ZkAcePublicInputsV1 =
        json::from_json(&public_inputs_json).map_err(norito_to_napi)?;
    let witness: ZkAceWitnessV1 = json::from_json(&witness_json).map_err(norito_to_napi)?;
    let vk_commitment = parse_optional_zk_ace_commitment(vk_commitment)?;
    let proof =
        zk_ace_prover::build_zk_ace_authorization_proof_v1(&public_inputs, &witness, vk_commitment)
            .map_err(|err| norito_to_napi(format!("build ZK-ACE proof: {err}")))?;
    let public_inputs_bytes = norito::to_bytes(&public_inputs).map_err(norito_to_napi)?;
    zk_ace_authorization_to_json(&public_inputs, &proof, &public_inputs_bytes)
}

#[napi(js_name = "zkAceBuildTransferAuthorizationV1")]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
/// Build a complete ZK-ACE transparent-transfer authorization proof and public input payload.
pub fn zk_ace_build_transfer_authorization_v1(
    from_account_id: String,
    to_account_id: String,
    asset_definition_id: String,
    amount: String,
    chain_id: String,
    identity_root: Uint8Array,
    identity_blinding: Uint8Array,
    replay_secret: Uint8Array,
    policy_hash: Uint8Array,
    verifier_key_id: Option<String>,
    vk_commitment: Option<Uint8Array>,
) -> napi::Result<String> {
    let from = parse_account_id(&from_account_id, "from_account_id")?;
    let to = parse_account_id(&to_account_id, "to_account_id")?;
    let asset: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset_definition_id: {err}"),
        )
    })?;
    let amount = amount.trim().parse::<u128>().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("amount must be an unsigned integer string: {err}"),
        )
    })?;
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain_id: {err}"))
    })?;
    let witness = ZkAceWitnessV1 {
        identity_root: parse_zk_ace_fixed32(&identity_root, "identity_root")?,
        identity_blinding: parse_zk_ace_fixed32(&identity_blinding, "identity_blinding")?,
        replay_secret: parse_zk_ace_fixed32(&replay_secret, "replay_secret")?,
    };
    let policy_hash = parse_zk_ace_fixed32(&policy_hash, "policy_hash")?;
    let verifier_key_id = parse_zk_ace_verifier_key_id(verifier_key_id)?;
    let vk_commitment = parse_optional_zk_ace_commitment(vk_commitment)?;
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
    .map_err(|err| norito_to_napi(format!("build ZK-ACE transfer authorization: {err}")))?;
    zk_ace_authorization_to_json(
        &authorization.public_inputs,
        &authorization.proof,
        &authorization.public_inputs_bytes,
    )
}

fn derive_kaigi_scalar_u64(seed: &[u8], label: &[u8]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"iroha-js:kaigi:roster-join:v1");
    hasher.update(label);
    hasher.update(seed);
    let digest = hasher.finalize();
    let mut scalar = [0u8; 8];
    scalar.copy_from_slice(&digest.as_bytes()[..8]);
    let value = u64::from_le_bytes(scalar);
    if value == 0 { 1 } else { value }
}

fn parse_kaigi_roster_root_hex(value: Option<String>) -> napi::Result<Hash> {
    let Some(raw) = value.map(|entry| entry.trim().to_owned()) else {
        return Ok(empty_roster_root_hash());
    };
    if raw.is_empty() {
        return Ok(empty_roster_root_hash());
    }
    let trimmed = raw.strip_prefix("0x").unwrap_or(raw.as_str());
    let decoded = hex::decode(trimmed).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("rosterRootHex must be valid hex: {err}"),
        )
    })?;
    if decoded.len() != Hash::LENGTH {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "rosterRootHex must be {} bytes, got {}",
                Hash::LENGTH,
                decoded.len()
            ),
        ));
    }
    let mut bytes = [0u8; Hash::LENGTH];
    bytes.copy_from_slice(decoded.as_slice());
    Ok(Hash::prehashed(bytes))
}

fn usize_to_u32_len(len: usize, context: &str) -> u32 {
    u32::try_from(len).unwrap_or_else(|_| panic!("{context} length exceeds u32::MAX"))
}

fn zk1_append_tlv(buf: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
    buf.extend_from_slice(&tag);
    buf.extend_from_slice(&usize_to_u32_len(payload.len(), "zk1 tlv payload").to_le_bytes());
    buf.extend_from_slice(payload);
}

fn zk1_append_proof(buf: &mut Vec<u8>, proof: &[u8]) {
    zk1_append_tlv(buf, *b"PROF", proof);
}

fn zk1_append_instances_cols(buf: &mut Vec<u8>, columns: &[&[Halo2Scalar]]) {
    if columns.is_empty() {
        return;
    }
    let rows = columns[0].len();
    if columns.iter().any(|column| column.len() != rows) {
        return;
    }

    let mut payload = Vec::with_capacity(8 + rows * columns.len() * mem::size_of::<Halo2Scalar>());
    payload
        .extend_from_slice(&usize_to_u32_len(columns.len(), "zk1 instance columns").to_le_bytes());
    payload.extend_from_slice(&usize_to_u32_len(rows, "zk1 instance rows").to_le_bytes());
    for row in 0..rows {
        for column in columns {
            payload.extend_from_slice(column[row].to_repr().as_ref());
        }
    }
    zk1_append_tlv(buf, *b"I10P", payload.as_slice());
}

fn build_kaigi_roster_join_proof_bytes(
    seed: &[u8],
    roster_root: &Hash,
) -> napi::Result<JsKaigiRosterJoinProof> {
    let account_idx = derive_kaigi_scalar_u64(seed, b"account");
    let domain_salt = derive_kaigi_scalar_u64(seed, b"domain");
    let nullifier_seed = derive_kaigi_scalar_u64(seed, b"nullifier");

    let account_scalar = Halo2Scalar::from(account_idx);
    let domain_scalar = Halo2Scalar::from(domain_salt);
    let nullifier_scalar = Halo2Scalar::from(nullifier_seed);
    let root_scalars = roster_root_limbs(roster_root);

    let params: ParamsIPA<Halo2Curve> = ParamsIPA::new(KAIGI_ROSTER_CIRCUIT_K);
    let verifying_key = keygen_vk(&params, &KaigiRosterJoinCircuit::default()).map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("failed to generate Kaigi roster verifying key: {err}"),
        )
    })?;

    let circuit = KaigiRosterJoinCircuit::new(
        account_scalar,
        domain_scalar,
        nullifier_scalar,
        root_scalars,
    );
    let proving_key = keygen_pk(&params, verifying_key.clone(), &circuit).map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("failed to generate Kaigi roster proving key: {err}"),
        )
    })?;

    let commitment_scalar = compute_commitment(account_scalar, domain_scalar);
    let nullifier_scalar_public = compute_nullifier(account_scalar, nullifier_scalar);
    let mut instance_columns = vec![vec![commitment_scalar], vec![nullifier_scalar_public]];
    instance_columns.extend(root_scalars.iter().map(|scalar| vec![*scalar]));
    let instance_refs: Vec<&[Halo2Scalar]> = instance_columns.iter().map(Vec::as_slice).collect();
    let proof_instances = vec![instance_refs.as_slice()];

    let mut transcript = Blake2bWrite::<_, Halo2Curve, Challenge255<Halo2Curve>>::init(Vec::new());
    create_proof::<
        IPACommitmentScheme<Halo2Curve>,
        ProverIPA<'_, Halo2Curve>,
        Challenge255<Halo2Curve>,
        _,
        _,
        _,
    >(
        &params,
        &proving_key,
        &[circuit],
        &proof_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("failed to create Kaigi roster proof: {err}"),
        )
    })?;
    let proof_payload = transcript.finalize();

    let mut zk1 = ZK1_ENVELOPE_PREFIX.to_vec();
    zk1_append_proof(&mut zk1, proof_payload.as_slice());
    zk1_append_instances_cols(&mut zk1, instance_refs.as_slice());

    let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
        backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        circuit_id: KAIGI_ROSTER_BACKEND.to_string(),
        vk_hash: [0u8; 32],
        public_inputs: KAIGI_ROSTER_PUBLIC_INPUTS_DESC.to_vec(),
        proof_bytes: zk1,
        aux: Vec::new(),
    };
    let encoded = norito::to_bytes(&envelope).map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("failed to encode Kaigi roster proof envelope: {err}"),
        )
    })?;

    Ok(JsKaigiRosterJoinProof {
        commitment: Buffer::from(compute_commitment_bytes(account_idx, domain_salt).to_vec()),
        nullifier: Buffer::from(compute_nullifier_bytes(account_idx, nullifier_seed).to_vec()),
        roster_root: Buffer::from(<[u8; 32]>::from(*roster_root).to_vec()),
        proof: Buffer::from(encoded),
    })
}

/// Build a Halo2/IPA Kaigi roster-join proof for `ZkRosterV1` joins.
#[napi(js_name = "buildKaigiRosterJoinProof")]
#[allow(clippy::needless_pass_by_value)]
pub fn build_kaigi_roster_join_proof(
    seed: Uint8Array,
    roster_root_hex: Option<String>,
) -> napi::Result<JsKaigiRosterJoinProof> {
    if seed.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "seed must be non-empty",
        ));
    }
    let roster_root = parse_kaigi_roster_root_hex(roster_root_hex)?;
    build_kaigi_roster_join_proof_bytes(seed.as_ref(), &roster_root)
}

/// Generate an Ed25519 key pair using `iroha_crypto`.
#[napi]
pub fn ed25519_keypair(seed: Option<Uint8Array>) -> napi::Result<JsKeyPair> {
    let keypair = match seed {
        Some(seed) => KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519),
        None => KeyPair::try_random_with_algorithm(Algorithm::Ed25519),
    }
    .map_err(norito_to_napi)?;

    let public_bytes = checked_public_key_payload(keypair.public_key())?;
    let (_, private_bytes) = keypair.private_key().to_bytes();

    Ok(JsKeyPair {
        algorithm: "ed25519".to_owned(),
        public_key: Buffer::from(public_bytes.to_vec()),
        private_key: Buffer::from(private_bytes),
        distid: None,
    })
}

fn algorithm_alias_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_crypto_algorithm(value: Option<&str>) -> napi::Result<Algorithm> {
    let value = value.unwrap_or("ed25519").trim();
    let key = algorithm_alias_key(value);
    let algorithm = match key.as_str() {
        "ed25519" | "ed" | "eddsa" => Algorithm::Ed25519,
        "secp256k1" | "secp" | "secpk1" => Algorithm::Secp256k1,
        "mldsa" | "mldsa65" | "mldsa44" | "mldsa87" => Algorithm::MlDsa,
        "blsnormal" | "bls12381g1" => Algorithm::BlsNormal,
        "blssmall" | "bls12381g2" => Algorithm::BlsSmall,
        "gost256a" | "gost34102012256paramseta" => Algorithm::Gost3410_2012_256ParamSetA,
        "gost256b" | "gost34102012256paramsetb" => Algorithm::Gost3410_2012_256ParamSetB,
        "gost256c" | "gost34102012256paramsetc" => Algorithm::Gost3410_2012_256ParamSetC,
        "gost512a" | "gost34102012512paramseta" => Algorithm::Gost3410_2012_512ParamSetA,
        "gost512b" | "gost34102012512paramsetb" => Algorithm::Gost3410_2012_512ParamSetB,
        "sm2" => Algorithm::Sm2,
        _ => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("unsupported crypto algorithm: {value}"),
            ));
        }
    };
    Ok(algorithm)
}

fn checked_public_key_payload(public_key: &PublicKey) -> napi::Result<&[u8]> {
    public_key
        .try_to_bytes()
        .map(|(_algorithm, payload)| payload)
        .map_err(norito_to_napi)
}

fn js_keypair_from_keypair(keypair: KeyPair) -> napi::Result<JsKeyPair> {
    let algorithm = keypair.algorithm();
    let public_bytes = checked_public_key_payload(keypair.public_key())?;
    let (_, private_bytes) = keypair.private_key().to_bytes();
    Ok(JsKeyPair {
        algorithm: algorithm.as_static_str().to_owned(),
        public_key: Buffer::from(public_bytes.to_vec()),
        private_key: Buffer::from(private_bytes),
        distid: None,
    })
}

/// Return canonical algorithm labels available through the JavaScript native binding.
#[napi(js_name = "supportedCryptoAlgorithms")]
pub fn supported_crypto_algorithms_js() -> Vec<String> {
    SUPPORTED_CRYPTO_ALGORITHMS
        .iter()
        .map(|algorithm| algorithm.as_static_str().to_owned())
        .collect()
}

/// Normalize a user-facing algorithm label to the canonical Rust `iroha_crypto` label.
#[napi(js_name = "normalizeCryptoAlgorithm")]
#[allow(clippy::needless_pass_by_value)]
pub fn normalize_crypto_algorithm_js(algorithm: Option<String>) -> napi::Result<String> {
    Ok(parse_crypto_algorithm(algorithm.as_deref())?
        .as_static_str()
        .to_owned())
}

/// Generate or deterministically derive a key pair for any supported Iroha signing algorithm.
#[napi(js_name = "cryptoKeypair")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_keypair(
    algorithm: Option<String>,
    seed: Option<Uint8Array>,
) -> napi::Result<JsKeyPair> {
    let algorithm = parse_crypto_algorithm(algorithm.as_deref())?;
    let keypair = match seed {
        Some(seed) => KeyPair::try_from_seed(seed.to_vec(), algorithm),
        None => KeyPair::try_random_with_algorithm(algorithm),
    }
    .map_err(norito_to_napi)?;
    js_keypair_from_keypair(keypair)
}

/// Reconstruct a key pair from private-key bytes for any supported Iroha signing algorithm.
#[napi(js_name = "cryptoKeypairFromPrivate")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_keypair_from_private(
    algorithm: String,
    private_key: Uint8Array,
) -> napi::Result<JsKeyPair> {
    let algorithm = parse_crypto_algorithm(Some(&algorithm))?;
    let private_key =
        PrivateKey::from_bytes(algorithm, private_key.as_ref()).map_err(norito_to_napi)?;
    let keypair = KeyPair::from_private_key(private_key).map_err(norito_to_napi)?;
    js_keypair_from_keypair(keypair)
}

/// Derive public-key bytes from private-key bytes for any supported Iroha signing algorithm.
#[napi(js_name = "cryptoPublicKeyFromPrivate")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_public_key_from_private(
    algorithm: String,
    private_key: Uint8Array,
) -> napi::Result<Buffer> {
    let algorithm = parse_crypto_algorithm(Some(&algorithm))?;
    let private_key =
        PrivateKey::from_bytes(algorithm, private_key.as_ref()).map_err(norito_to_napi)?;
    let public_key = PublicKey::from(private_key);
    let public_bytes = checked_public_key_payload(&public_key)?;
    Ok(Buffer::from(public_bytes.to_vec()))
}

/// Sign a message with private-key bytes for any supported Iroha signing algorithm.
#[napi(js_name = "cryptoSign")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_sign(
    algorithm: String,
    private_key: Uint8Array,
    message: Uint8Array,
) -> napi::Result<Buffer> {
    let algorithm = parse_crypto_algorithm(Some(&algorithm))?;
    let private_key =
        PrivateKey::from_bytes(algorithm, private_key.as_ref()).map_err(norito_to_napi)?;
    let signature = Signature::try_new(&private_key, message.as_ref()).map_err(norito_to_napi)?;
    Ok(Buffer::from(signature.payload().to_vec()))
}

/// Verify a signature against public-key bytes for any supported Iroha signing algorithm.
#[napi(js_name = "cryptoVerify")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_verify(
    algorithm: String,
    public_key: Uint8Array,
    message: Uint8Array,
    signature: Uint8Array,
) -> napi::Result<bool> {
    let algorithm = parse_crypto_algorithm(Some(&algorithm))?;
    let public_key =
        PublicKey::from_bytes(algorithm, public_key.as_ref()).map_err(norito_to_napi)?;
    let signature = Signature::from_bytes(signature.as_ref());
    Ok(signature.verify(&public_key, message.as_ref()).is_ok())
}

/// Encode public-key bytes as an Iroha multihash literal.
#[napi(js_name = "cryptoPublicKeyMultihash")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_public_key_multihash(
    algorithm: String,
    public_key: Uint8Array,
) -> napi::Result<String> {
    let algorithm = parse_crypto_algorithm(Some(&algorithm))?;
    PublicKey::from_bytes(algorithm, public_key.as_ref())
        .and_then(|public_key| public_key.try_to_multihash_string())
        .map_err(norito_to_napi)
}

/// Encode private-key bytes as an exposed Iroha multihash literal.
#[napi(js_name = "cryptoPrivateKeyMultihash")]
#[allow(clippy::needless_pass_by_value)]
pub fn crypto_private_key_multihash(
    algorithm: String,
    private_key: Uint8Array,
) -> napi::Result<String> {
    let algorithm = parse_crypto_algorithm(Some(&algorithm))?;
    let private_key =
        PrivateKey::from_bytes(algorithm, private_key.as_ref()).map_err(norito_to_napi)?;
    ExposedPrivateKey(private_key)
        .try_to_multihash_string()
        .map_err(norito_to_napi)
}

/// Derive an Ed25519 public key from a private key seed or keypair payload.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs typed arrays require owned values at the boundary
pub fn ed25519_public_key_from_private(private_key: Uint8Array) -> napi::Result<Buffer> {
    let secret =
        PrivateKey::from_bytes(Algorithm::Ed25519, private_key.as_ref()).map_err(norito_to_napi)?;
    let keypair = KeyPair::from_private_key(secret).map_err(norito_to_napi)?;
    let public_bytes = checked_public_key_payload(keypair.public_key())?;
    Ok(Buffer::from(public_bytes.to_vec()))
}

fn parse_soracloud_storage_class(value: &str) -> napi::Result<StorageClass> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        _ => Err(napi::Error::new(
            napi::Status::InvalidArg,
            "storage_class must be hot, warm, or cold",
        )),
    }
}

fn parse_positive_u64_literal(value: &str, label: &str) -> napi::Result<u64> {
    let parsed = value.trim().parse::<u64>().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{label} must be a positive integer: {err}"),
        )
    })?;
    if parsed == 0 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{label} must be greater than zero"),
        ));
    }
    Ok(parsed)
}

fn parse_positive_u128_literal(value: &str, label: &str) -> napi::Result<u128> {
    let parsed = value.trim().parse::<u128>().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{label} must be a positive integer: {err}"),
        )
    })?;
    if parsed == 0 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{label} must be greater than zero"),
        ));
    }
    Ok(parsed)
}

fn parse_ed25519_keypair_hex(private_key_hex: &str) -> napi::Result<KeyPair> {
    let private_key_bytes = hex::decode(private_key_hex.trim()).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("private_key_hex must be hex-encoded Ed25519 key material: {err}"),
        )
    })?;
    let private_key =
        PrivateKey::from_bytes(Algorithm::Ed25519, &private_key_bytes).map_err(norito_to_napi)?;
    KeyPair::from_private_key(private_key).map_err(norito_to_napi)
}

fn sign_soracloud_payload(keypair: &KeyPair, payload: &[u8]) -> napi::Result<ManifestProvenance> {
    Ok(ManifestProvenance {
        signer: keypair.public_key().clone(),
        signature: Signature::try_new(keypair.private_key(), payload).map_err(norito_to_napi)?,
    })
}

fn soracloud_source_hash(repo_id: &str, resolved_revision: &str) -> napi::Result<Hash> {
    let payload = norito::to_bytes(&(repo_id, resolved_revision)).map_err(norito_to_napi)?;
    Ok(Hash::new(payload))
}

/// Build the fully signed request body accepted by `/v1/soracloud/hf/deploy`.
#[allow(clippy::too_many_arguments)]
#[napi]
pub fn soracloud_build_hf_deploy_request_json(
    repo_id: String,
    revision: Option<String>,
    model_name: String,
    service_name: String,
    apartment_name: Option<String>,
    storage_class: String,
    lease_term_ms: String,
    lease_asset_definition_id: String,
    base_fee_nanos: String,
    private_key_hex: String,
) -> napi::Result<String> {
    let repo_id = repo_id.trim().to_owned();
    let revision = revision
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let resolved_revision = revision.clone().unwrap_or_else(|| "main".to_owned());
    let model_name = model_name.trim().to_owned();
    let service_name = service_name
        .trim()
        .parse::<Name>()
        .map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("invalid service_name: {err}"),
            )
        })?
        .to_string();
    let apartment_name = apartment_name
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<Name>()
                .map(|name| name.to_string())
                .map_err(|err| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("invalid apartment_name: {err}"),
                    )
                })
        })
        .transpose()?;
    if repo_id.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "repo_id must not be empty",
        ));
    }
    if model_name.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "model_name must not be empty",
        ));
    }

    let storage_class = parse_soracloud_storage_class(&storage_class)?;
    let lease_term_ms = parse_positive_u64_literal(&lease_term_ms, "lease_term_ms")?;
    let base_fee_nanos = parse_positive_u128_literal(&base_fee_nanos, "base_fee_nanos")?;
    let lease_asset_definition_id = lease_asset_definition_id
        .trim()
        .parse::<AssetDefinitionId>()
        .map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("invalid lease_asset_definition_id: {err}"),
            )
        })?;
    let keypair = parse_ed25519_keypair_hex(&private_key_hex)?;

    let deploy_payload = encode_hf_shared_lease_join_provenance_payload(
        &repo_id,
        &resolved_revision,
        &model_name,
        &service_name,
        apartment_name.as_deref(),
        storage_class,
        lease_term_ms,
        &lease_asset_definition_id,
        base_fee_nanos,
    )
    .map_err(norito_to_napi)?;
    let provenance = sign_soracloud_payload(&keypair, &deploy_payload)?;

    let service_name_typed = service_name.parse::<Name>().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid service_name: {err}"),
        )
    })?;
    let source_id = soracloud_source_hash(&repo_id, &resolved_revision)?;
    let generated_bundle = build_soracloud_hf_generated_service_bundle(
        service_name_typed,
        &source_id.to_string(),
        &repo_id,
        &resolved_revision,
        &model_name,
    );
    let configs: BTreeMap<String, Json> = BTreeMap::new();
    let secrets: BTreeMap<String, SecretEnvelopeV1> = BTreeMap::new();
    let service_provenance_payload =
        encode_bundle_with_materials_provenance_payload(&generated_bundle, &configs, &secrets)
            .map_err(norito_to_napi)?;
    let generated_service_provenance =
        sign_soracloud_payload(&keypair, &service_provenance_payload)?;

    let generated_apartment_provenance = apartment_name
        .as_deref()
        .map(|apartment_name| {
            let apartment_name = apartment_name.parse::<Name>().map_err(|err| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("invalid apartment_name: {err}"),
                )
            })?;
            let manifest =
                build_soracloud_hf_generated_agent_manifest(apartment_name, &generated_bundle);
            let payload = encode_agent_deploy_provenance_payload(
                manifest,
                HF_GENERATED_AGENT_LEASE_TICKS,
                Some(HF_GENERATED_AGENT_AUTONOMY_BUDGET_UNITS),
            )
            .map_err(norito_to_napi)?;
            sign_soracloud_payload(&keypair, &payload)
        })
        .transpose()?;

    let mut payload = Map::new();
    payload.insert(
        "repo_id".to_owned(),
        json::to_value(&repo_id).map_err(norito_to_napi)?,
    );
    if let Some(revision) = &revision {
        payload.insert(
            "revision".to_owned(),
            json::to_value(revision).map_err(norito_to_napi)?,
        );
    }
    payload.insert(
        "model_name".to_owned(),
        json::to_value(&model_name).map_err(norito_to_napi)?,
    );
    payload.insert(
        "service_name".to_owned(),
        json::to_value(&service_name).map_err(norito_to_napi)?,
    );
    if let Some(apartment_name) = &apartment_name {
        payload.insert(
            "apartment_name".to_owned(),
            json::to_value(apartment_name).map_err(norito_to_napi)?,
        );
    }
    payload.insert(
        "storage_class".to_owned(),
        json::to_value(&storage_class).map_err(norito_to_napi)?,
    );
    payload.insert(
        "lease_term_ms".to_owned(),
        json::to_value(&lease_term_ms).map_err(norito_to_napi)?,
    );
    payload.insert(
        "lease_asset_definition_id".to_owned(),
        json::to_value(&lease_asset_definition_id).map_err(norito_to_napi)?,
    );
    payload.insert(
        "base_fee_nanos".to_owned(),
        json::to_value(&base_fee_nanos).map_err(norito_to_napi)?,
    );

    let mut root = Map::new();
    root.insert("payload".to_owned(), Value::Object(payload));
    root.insert(
        "provenance".to_owned(),
        json::to_value(&provenance).map_err(norito_to_napi)?,
    );
    root.insert(
        "generated_service_provenance".to_owned(),
        json::to_value(&generated_service_provenance).map_err(norito_to_napi)?,
    );
    if let Some(generated_apartment_provenance) = &generated_apartment_provenance {
        root.insert(
            "generated_apartment_provenance".to_owned(),
            json::to_value(generated_apartment_provenance).map_err(norito_to_napi)?,
        );
    }

    json::to_json(&Value::Object(root)).map_err(norito_to_napi)
}

/// Return the default SM2 distinguishing identifier used when none is provided.
#[napi]
pub fn sm2_default_distid() -> String {
    Sm2PublicKey::default_distid()
}

/// Generate an SM2 key pair using `iroha_crypto` defaults.
#[napi]
pub fn sm2_keypair(distid: Option<String>) -> napi::Result<JsKeyPair> {
    let distid = sm2_distid_arg(distid);
    let private = Sm2PrivateKey::try_random_from_os(distid.clone()).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("failed to generate SM2 key pair: {err}"),
        )
    })?;
    let public = private.public_key();
    let private_bytes = private.secret_bytes();
    let public_bytes = public.to_sec1_bytes(false);
    Ok(JsKeyPair {
        algorithm: "sm2".to_owned(),
        public_key: Buffer::from(public_bytes),
        private_key: Buffer::from(private_bytes.to_vec()),
        distid: Some(distid),
    })
}

/// Derive an SM2 key pair deterministically from a seed.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn sm2_keypair_from_seed(distid: Option<String>, seed: Uint8Array) -> napi::Result<JsKeyPair> {
    let distid = sm2_distid_arg(distid);
    let private = Sm2PrivateKey::from_seed(distid.clone(), seed.as_ref()).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("failed to derive SM2 private key from seed: {err}"),
        )
    })?;
    let public = private.public_key();
    let private_bytes = private.secret_bytes();
    let public_bytes = public.to_sec1_bytes(false);
    Ok(JsKeyPair {
        algorithm: "sm2".to_owned(),
        public_key: Buffer::from(public_bytes),
        private_key: Buffer::from(private_bytes.to_vec()),
        distid: Some(distid),
    })
}

/// Reconstruct an SM2 key pair from raw private-key bytes.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn sm2_keypair_from_private(
    distid: Option<String>,
    private_key: Uint8Array,
) -> napi::Result<JsKeyPair> {
    let private = parse_sm2_private_key(distid, private_key.as_ref())?;
    let public = private.public_key();
    let private_bytes = private.secret_bytes();
    let public_bytes = public.to_sec1_bytes(false);
    Ok(JsKeyPair {
        algorithm: "sm2".to_owned(),
        public_key: Buffer::from(public_bytes),
        private_key: Buffer::from(private_bytes.to_vec()),
        distid: Some(private.distid().to_owned()),
    })
}

/// Compute the canonical multihash string for an SM2 public key.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn sm2_public_key_multihash(
    public_key: Uint8Array,
    distid: Option<String>,
) -> napi::Result<String> {
    let payload = public_key.as_ref();
    if payload.len() != SM2_PUBLIC_KEY_LENGTH {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "sm2 public key must be {SM2_PUBLIC_KEY_LENGTH} bytes, got {}",
                payload.len()
            ),
        ));
    }
    let distid = sm2_distid_arg(distid);
    let _ = parse_sm2_public_key(Some(distid.clone()), payload)?;
    let encoded = encode_sm2_public_key_payload(&distid, payload)
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))?;
    PublicKey::from_bytes(Algorithm::Sm2, &encoded)
        .and_then(|pk| pk.try_to_multihash_string())
        .map_err(norito_to_napi)
}

/// Sign a message using an SM2 private key.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn sm2_sign(
    private_key: Uint8Array,
    message: Uint8Array,
    distid: Option<String>,
) -> napi::Result<Buffer> {
    let private = parse_sm2_private_key(distid, private_key.as_ref())?;
    let signature = private.sign(message.as_ref()).to_bytes();
    Ok(Buffer::from(signature.to_vec()))
}

/// Verify an SM2 signature against the provided message and public key.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn sm2_verify(
    public_key: Uint8Array,
    message: Uint8Array,
    signature: Uint8Array,
    distid: Option<String>,
) -> napi::Result<bool> {
    let public = parse_sm2_public_key(distid, public_key.as_ref())?;
    let signature = parse_sm2_signature(signature.as_ref())?;
    Ok(public.verify(message.as_ref(), &signature).is_ok())
}

/// Encode an instruction JSON payload to canonical Norito bytes.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String` for `#[napi]` bindings
pub fn norito_encode_instruction(json_payload: String) -> napi::Result<Buffer> {
    ensure_packed_struct_disabled();
    let instruction = instruction_from_json(&json_payload)?;
    let encoded = norito_core::to_bytes(&instruction).map_err(norito_to_napi)?;
    Ok(Buffer::from(encoded))
}

/// Decode canonical Norito bytes for an instruction back into JSON form.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned typed arrays for `#[napi]` bindings
pub fn norito_decode_instruction(bytes: Uint8Array) -> napi::Result<String> {
    ensure_packed_struct_disabled();
    let decode = catch_unwind(AssertUnwindSafe(|| {
        let slice = bytes.as_ref();
        let instruction = decode_instruction_aligned(slice).map_err(norito_to_napi)?;
        let value = instruction_to_json_value(&instruction)?;
        json::to_json(&value).map_err(norito_to_napi)
    }));

    match decode {
        Ok(result) => result,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic");
            Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!("panic during Norito decode: {message}"),
            ))
        }
    }
}

/// Relay envelope fixture used in Nexus cross-lane verification tests.
#[napi(object)]
pub struct JsLaneRelaySample {
    /// Norito-encoded relay envelope bytes.
    pub valid: Buffer,
    /// Same envelope with a tampered checksum byte.
    pub tampered: Buffer,
}

/// Return a deterministic relay envelope fixture and a tampered copy for testing.
#[napi]
pub fn lane_relay_envelope_sample() -> napi::Result<JsLaneRelaySample> {
    ensure_packed_struct_disabled();
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
    let validator_key = KeyPair::try_random().map_err(norito_to_napi)?;
    let validator_set = vec![PeerId::from(validator_key.public_key().clone())];
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
        validator_set_hash_version: 1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![0x01],
            bls_aggregate_signature: vec![0xCC; 48],
        },
    };
    let envelope = LaneRelayEnvelope::new(header, Some(qc), Some(da_hash), settlement, 64)
        .map_err(norito_to_napi)?;
    let valid =
        Buffer::from(norito::to_bytes(&envelope).map_err(|err| norito_to_napi(format!("{err}")))?);

    let mut tampered = valid.to_vec();
    if let Some(last) = tampered.last_mut() {
        *last ^= 0xFF;
    }

    Ok(JsLaneRelaySample {
        valid,
        tampered: Buffer::from(tampered),
    })
}

/// Verify the Norito-encoded relay envelope bytes returned by `/v1/sumeragi/status`.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API typed arrays require ownership at the boundary
pub fn verify_lane_relay_envelope(envelope: Uint8Array) -> napi::Result<()> {
    ensure_packed_struct_disabled();
    let slice = envelope.to_vec();
    let mut view = slice.as_slice();
    let parsed = LaneRelayEnvelope::decode_all(&mut view).or_else(|err| {
        decode_from_bytes::<LaneRelayEnvelope>(slice.as_ref())
            .map_err(|_| norito_to_napi(format!("{err}")))
    })?;
    parsed.verify().map_err(norito_to_napi)
}

/// Decode relay envelope bytes into a JSON string for inspection.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API typed arrays require ownership at the boundary
pub fn decode_lane_relay_envelope(envelope: Uint8Array) -> napi::Result<String> {
    ensure_packed_struct_disabled();
    let slice = envelope.to_vec();
    let mut view = slice.as_slice();
    let parsed = LaneRelayEnvelope::decode_all(&mut view).or_else(|err| {
        decode_from_bytes::<LaneRelayEnvelope>(slice.as_ref())
            .map_err(|_| norito_to_napi(format!("{err}")))
    })?;
    json::to_json_pretty(&parsed).map_err(norito_to_napi)
}

/// Verify a relay envelope provided as a JSON string.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API strings are owned at the boundary
pub fn verify_lane_relay_envelope_json(envelope_json: String) -> napi::Result<()> {
    ensure_packed_struct_disabled();
    let parsed: LaneRelayEnvelope = json::from_json(&envelope_json).map_err(norito_to_napi)?;
    parsed.verify().map_err(norito_to_napi)
}

/// Compute the settlement hash for a JSON `LaneBlockCommitment`.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API strings are owned at the boundary
pub fn lane_settlement_hash(settlement_json: String) -> napi::Result<String> {
    ensure_packed_struct_disabled();
    let commitment: LaneBlockCommitment =
        json::from_json(&settlement_json).map_err(norito_to_napi)?;
    let hash = compute_settlement_hash(&commitment).map_err(norito_to_napi)?;
    Ok(hex::encode_upper(hash.as_ref()))
}

/// Touch manifest output returned to JavaScript callers.
#[napi(object)]
pub struct JsTouchManifest {
    /// Manifest rendered as Norito JSON.
    pub manifest_json: String,
}

/// Canonicalise a touch manifest by sorting and deduplicating keys.
#[napi]
pub fn axt_touch_manifest(read: Vec<String>, write: Vec<String>) -> napi::Result<JsTouchManifest> {
    ensure_packed_struct_disabled();
    let manifest = TouchManifest::from_read_write(read, write);
    Ok(JsTouchManifest {
        manifest_json: json::to_json(&manifest).map_err(norito_to_napi)?,
    })
}

/// Canonicalised AXT descriptor and derived binding bytes.
#[napi(object)]
pub struct JsAxtDescriptorArtifacts {
    /// Descriptor rendered as Norito JSON.
    pub descriptor_json: String,
    /// Descriptor encoded as Norito bytes.
    pub descriptor_bytes: Buffer,
    /// Optional touch manifest fragments rendered as Norito JSON.
    pub touch_manifest_json: String,
    /// Poseidon-derived binding in hexadecimal form.
    pub binding_hex: String,
    /// Poseidon-derived binding bytes.
    pub binding: Buffer,
}

/// Touch declaration provided by JavaScript callers.
#[napi(object)]
pub struct JsAxtTouchSpec {
    /// Dataspace identifier associated with the touch spec.
    pub dsid: u32,
    /// Declared read set (deduplicated and sorted internally).
    pub read: Option<Vec<String>>,
    /// Declared write set (deduplicated and sorted internally).
    pub write: Option<Vec<String>>,
}

/// Build a canonical AXT descriptor and binding from JavaScript inputs.
#[napi]
pub fn axt_build_descriptor(
    dataspace_ids: Vec<u32>,
    touches: Vec<JsAxtTouchSpec>,
) -> napi::Result<JsAxtDescriptorArtifacts> {
    ensure_packed_struct_disabled();

    let mut builder = AxtDescriptorBuilder::new();
    for dsid in dataspace_ids {
        builder = builder.dataspace(DataSpaceId::new(dsid.into()));
    }
    for touch in touches {
        let manifest = TouchManifest::from_read_write(
            touch.read.unwrap_or_default(),
            touch.write.unwrap_or_default(),
        );
        builder = builder.touch(
            DataSpaceId::new(u64::from(touch.dsid)),
            manifest.read,
            manifest.write,
        );
    }

    let descriptor = builder
        .build()
        .map_err(|err| norito_to_napi(format!("{err}")))?;
    validate_descriptor(&descriptor).map_err(|err| norito_to_napi(format!("{err}")))?;

    let descriptor_json = json::to_json(&descriptor).map_err(norito_to_napi)?;
    let descriptor_bytes =
        norito::to_bytes(&descriptor).map_err(|err| norito_to_napi(format!("{err}")))?;

    let binding_bytes = compute_descriptor_binding(&descriptor).map_err(norito_to_napi)?;
    let touch_manifest: Vec<AxtTouchFragment> = descriptor
        .touches
        .iter()
        .map(|touch| AxtTouchFragment {
            dsid: touch.dsid,
            manifest: TouchManifest {
                read: touch.read.clone(),
                write: touch.write.clone(),
            },
        })
        .collect();

    Ok(JsAxtDescriptorArtifacts {
        descriptor_json,
        descriptor_bytes: Buffer::from(descriptor_bytes),
        touch_manifest_json: json::to_json(&touch_manifest).map_err(norito_to_napi)?,
        binding_hex: hex::encode(binding_bytes),
        binding: Buffer::from(binding_bytes.to_vec()),
    })
}

/// Compute an AXT binding from canonical Norito descriptor bytes.
#[napi]
pub fn axt_compute_binding(descriptor_bytes: Buffer) -> napi::Result<Buffer> {
    ensure_packed_struct_disabled();
    let descriptor: AxtDescriptor = decode_from_bytes(descriptor_bytes.as_ref())
        .map_err(|err| norito_to_napi(format!("{err}")))?;
    validate_descriptor(&descriptor).map_err(|err| norito_to_napi(format!("{err}")))?;
    let binding_bytes = compute_descriptor_binding(&descriptor).map_err(norito_to_napi)?;
    Ok(Buffer::from(binding_bytes.to_vec()))
}

#[allow(unsafe_code)]
fn decode_instruction_aligned(bytes: &[u8]) -> Result<InstructionBox, norito_core::Error> {
    if let Ok(instruction) = decode_from_bytes::<InstructionBox>(bytes) {
        return Ok(instruction);
    }
    let view = norito_core::from_bytes_view(bytes)?;
    let payload = view.as_bytes();
    let (instruction, used) = <InstructionBox as DecodeFromSlice>::decode_from_slice(payload)?;
    if used != payload.len() {
        return Err(norito_core::Error::LengthMismatch);
    }
    Ok(instruction)
}
/// Derive the confidential key hierarchy from a 32-byte spend key.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API typed arrays require ownership at the boundary
pub fn derive_confidential_keyset(spend_key: Uint8Array) -> napi::Result<JsConfidentialKeyset> {
    let seed = spend_key.as_ref();
    if seed.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let keyset = derive_keyset_from_slice(seed).map_err(norito_to_napi)?;
    Ok(JsConfidentialKeyset {
        sk_spend: Buffer::from(keyset.spend_key().to_vec()),
        nk: Buffer::from(keyset.nullifier_key().to_vec()),
        ivk: Buffer::from(keyset.incoming_view_key().to_vec()),
        ovk: Buffer::from(keyset.outgoing_view_key().to_vec()),
        fvk: Buffer::from(keyset.full_view_key().to_vec()),
    })
}

/// Derive the confidential v2 owner tag from a 32-byte spend key.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn derive_confidential_owner_tag_v2(
    spend_key: Uint8Array,
    diversifier_hex: Option<String>,
) -> napi::Result<Buffer> {
    let spend_key = spend_key.as_ref();
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let diversifier =
        parse_optional_confidential_diversifier_hex("diversifier_hex", diversifier_hex.as_deref())?;
    Ok(Buffer::from(
        confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(spend_key, diversifier)
            .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?
            .to_vec(),
    ))
}

/// Derive a canonical confidential v2 note diversifier from seed material.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn derive_confidential_diversifier_v2(seed: Uint8Array) -> napi::Result<Buffer> {
    let seed = seed.as_ref();
    if seed.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential diversifier seed must not be empty",
        ));
    }
    Ok(Buffer::from(
        confidential_v2::derive_confidential_diversifier_v2(seed).to_vec(),
    ))
}

/// Derive a diversified confidential v2 receive address from a spend key and seed.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn derive_confidential_receive_address_v2(
    spend_key: Uint8Array,
    diversifier_seed: Uint8Array,
) -> napi::Result<JsConfidentialReceiveAddressV2> {
    let spend_key = spend_key.as_ref();
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let seed = diversifier_seed.as_ref();
    if seed.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential diversifier seed must not be empty",
        ));
    }
    let diversifier = confidential_v2::derive_confidential_diversifier_v2(seed);
    let owner_tag =
        confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(spend_key, diversifier)
            .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(JsConfidentialReceiveAddressV2 {
        owner_tag_hex: hex::encode(owner_tag),
        diversifier_hex: hex::encode(diversifier),
    })
}

/// Derive a confidential v2 note commitment from note material.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn derive_confidential_note_v2(
    asset_definition_id: String,
    amount: String,
    rho_hex: String,
    owner_tag_hex: String,
) -> napi::Result<Buffer> {
    let asset_definition_id: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset definition id: {err}"),
        )
    })?;
    let amount = parse_confidential_amount_u128("amount", &amount)?;
    let rho = parse_fixed_32_hex("rho_hex", &rho_hex)?;
    let owner_tag = parse_fixed_32_hex("owner_tag_hex", &owner_tag_hex)?;
    let commitment = confidential_v2::derive_confidential_note_v2(
        &asset_definition_id.to_string(),
        amount,
        rho,
        owner_tag,
    )
    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(Buffer::from(commitment.to_vec()))
}

/// Derive a confidential v2 nullifier from spend key material.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn derive_confidential_nullifier_v2(
    chain_id: String,
    asset_definition_id: String,
    spend_key: Uint8Array,
    rho_hex: String,
) -> napi::Result<Buffer> {
    let spend_key = spend_key.as_ref();
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let asset_definition_id: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset definition id: {err}"),
        )
    })?;
    let rho = parse_fixed_32_hex("rho_hex", &rho_hex)?;
    Ok(Buffer::from(
        confidential_v2::derive_confidential_nullifier_v2(
            chain_id.trim(),
            &asset_definition_id.to_string(),
            spend_key,
            rho,
        )
        .to_vec(),
    ))
}

/// Build a confidential transfer v2 proof envelope.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn build_confidential_transfer_proof_v2(
    chain_id: String,
    asset_definition_id: String,
    spend_key: Uint8Array,
    tree_commitments_hex: Vec<String>,
    inputs: Vec<JsConfidentialTransferInputV2>,
    outputs: Vec<JsConfidentialTransferOutputV2>,
    root_hint_hex: String,
    vk_backend: String,
    vk_circuit_id: String,
    vk_bytes: Uint8Array,
) -> napi::Result<JsConfidentialTransferProofEnvelopeV2> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let asset_definition_id: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset definition id: {err}"),
        )
    })?;
    let spend_key = spend_key.as_ref();
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let tree_commitments = parse_confidential_tree_commitments(tree_commitments_hex)?;
    let inputs = parse_confidential_transfer_inputs_v2(inputs)?;
    let outputs = parse_confidential_transfer_outputs_v2(outputs)?;
    let root_hint = parse_fixed_32_hex("root_hint_hex", &root_hint_hex)?;
    let vk_box = iroha_data_model::proof::VerifyingKeyBox::new(
        vk_backend.trim().to_owned(),
        vk_bytes.to_vec(),
    );
    let proof = confidential_v2::build_confidential_transfer_proof_v2(
        &chain_id,
        &asset_definition_id.to_string(),
        spend_key,
        &tree_commitments,
        &inputs,
        &outputs,
        root_hint,
        vk_circuit_id.trim(),
        &vk_box,
    )
    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(JsConfidentialTransferProofEnvelopeV2 {
        nullifiers: proof
            .nullifiers
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        output_commitments: proof
            .output_commitments
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        root: Buffer::from(proof.root.to_vec()),
        proof: Buffer::from(proof.proof.bytes),
    })
}

/// Build an asset-hidden transfer v1 proof envelope.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn build_confidential_asset_hidden_transfer_proof_v1(
    chain_id: String,
    pool_id: String,
    asset_set_root_hex: String,
    input_commitments_hex: Vec<String>,
    nullifiers_hex: Vec<String>,
    output_commitments_hex: Vec<String>,
    root_hint_hex: String,
    vk_backend: String,
    vk_circuit_id: String,
    vk_bytes: Uint8Array,
) -> napi::Result<JsAssetHiddenTransferProofEnvelopeV1> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let asset_set_root = parse_fixed_32_hex("asset_set_root_hex", &asset_set_root_hex)?;
    let input_commitments =
        parse_fixed_32_hex_list("input_commitments_hex", input_commitments_hex)?;
    let nullifiers = parse_fixed_32_hex_list("nullifiers_hex", nullifiers_hex)?;
    let output_commitments =
        parse_fixed_32_hex_list("output_commitments_hex", output_commitments_hex)?;
    let root_hint = parse_fixed_32_hex("root_hint_hex", &root_hint_hex)?;
    let vk_box = iroha_data_model::proof::VerifyingKeyBox::new(
        vk_backend.trim().to_owned(),
        vk_bytes.to_vec(),
    );
    let proof = confidential_v2::build_asset_hidden_transfer_proof_v1(
        &chain_id,
        pool_id.trim(),
        asset_set_root,
        &input_commitments,
        &nullifiers,
        &output_commitments,
        root_hint,
        vk_circuit_id.trim(),
        &vk_box,
    )
    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(JsAssetHiddenTransferProofEnvelopeV1 {
        input_commitments: proof
            .input_commitments
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        nullifiers: proof
            .nullifiers
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        output_commitments: proof
            .output_commitments
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        root: Buffer::from(proof.root.to_vec()),
        proof: Buffer::from(proof.proof.bytes),
    })
}

/// Build a confidential unshield v2 proof envelope.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn build_confidential_unshield_proof_v2(
    chain_id: String,
    asset_definition_id: String,
    spend_key: Uint8Array,
    tree_commitments_hex: Vec<String>,
    inputs: Vec<JsConfidentialTransferInputV2>,
    public_amount: String,
    root_hint_hex: String,
    vk_backend: String,
    vk_circuit_id: String,
    vk_bytes: Uint8Array,
) -> napi::Result<JsConfidentialUnshieldProofEnvelopeV2> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let asset_definition_id: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset definition id: {err}"),
        )
    })?;
    let spend_key = spend_key.as_ref();
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let tree_commitments = parse_confidential_tree_commitments(tree_commitments_hex)?;
    let inputs = parse_confidential_unshield_inputs_v2(inputs)?;
    let public_amount = parse_confidential_amount_u128("public_amount", &public_amount)?;
    let root_hint = parse_fixed_32_hex("root_hint_hex", &root_hint_hex)?;
    let vk_box = iroha_data_model::proof::VerifyingKeyBox::new(
        vk_backend.trim().to_owned(),
        vk_bytes.to_vec(),
    );
    let proof = confidential_v2::build_confidential_unshield_proof_v2(
        &chain_id,
        &asset_definition_id.to_string(),
        spend_key,
        &tree_commitments,
        &inputs,
        public_amount,
        root_hint,
        vk_circuit_id.trim(),
        &vk_box,
    )
    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(JsConfidentialUnshieldProofEnvelopeV2 {
        nullifiers: proof
            .nullifiers
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        root: Buffer::from(proof.root.to_vec()),
        proof: Buffer::from(proof.proof.bytes),
    })
}

/// Build a confidential unshield v3 proof envelope with optional private change.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn build_confidential_unshield_proof_v3(
    chain_id: String,
    asset_definition_id: String,
    spend_key: Uint8Array,
    tree_commitments_hex: Vec<String>,
    inputs: Vec<JsConfidentialTransferInputV2>,
    outputs: Vec<JsConfidentialUnshieldOutputV3>,
    public_amount: String,
    root_hint_hex: String,
    vk_backend: String,
    vk_circuit_id: String,
    vk_bytes: Uint8Array,
) -> napi::Result<JsConfidentialUnshieldProofEnvelopeV3> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let asset_definition_id: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset definition id: {err}"),
        )
    })?;
    let spend_key = spend_key.as_ref();
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let tree_commitments = parse_confidential_tree_commitments(tree_commitments_hex)?;
    let inputs = parse_confidential_unshield_inputs_v2(inputs)?;
    let outputs = parse_confidential_unshield_outputs_v3(outputs)?;
    let public_amount = parse_confidential_amount_u128("public_amount", &public_amount)?;
    let root_hint = parse_fixed_32_hex("root_hint_hex", &root_hint_hex)?;
    let vk_box = iroha_data_model::proof::VerifyingKeyBox::new(
        vk_backend.trim().to_owned(),
        vk_bytes.to_vec(),
    );
    let proof = confidential_v2::build_confidential_unshield_proof_v3(
        &chain_id,
        &asset_definition_id.to_string(),
        spend_key,
        &tree_commitments,
        &inputs,
        &outputs,
        public_amount,
        root_hint,
        vk_circuit_id.trim(),
        &vk_box,
    )
    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(JsConfidentialUnshieldProofEnvelopeV3 {
        nullifiers: proof
            .nullifiers
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        output_commitments: proof
            .output_commitments
            .into_iter()
            .map(|entry| Buffer::from(entry.to_vec()))
            .collect(),
        root: Buffer::from(proof.root.to_vec()),
        proof: Buffer::from(proof.proof.bytes),
    })
}

const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;

fn ensure_kagemusha_recursive_archive_len(
    archive_len: usize,
    archive_name: &str,
) -> napi::Result<()> {
    if archive_len == 0 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{archive_name} must not be empty"),
        ));
    }
    if archive_len > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{archive_name} must not exceed {KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes"),
        ));
    }
    Ok(())
}

fn decode_kagemusha_recursive_archive<T>(archive: &Uint8Array, context: &str) -> napi::Result<T>
where
    T: for<'de> norito::core::NoritoDeserialize<'de>,
{
    ensure_kagemusha_recursive_archive_len(archive.len(), &format!("{context} archive"))?;
    decode_from_bytes(archive.as_ref()).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid {context} archive: {err}"),
        )
    })
}

fn encode_kagemusha_recursive_archive<T>(value: &T, context: &str) -> napi::Result<Buffer>
where
    T: norito::core::NoritoSerialize,
{
    let bytes = norito::to_bytes(value).map_err(|err| {
        napi::Error::new(napi::Status::GenericFailure, format!("{context}: {err}"))
    })?;
    if bytes.len() > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES {
        return Err(napi::Error::new(
            napi::Status::GenericFailure,
            format!(
                "{context}: encoded Kagemusha archive exceeds {KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes"
            ),
        ));
    }
    Ok(Buffer::from(bytes))
}

fn ensure_kagemusha_recursive_spend_pallas_archive(archive: &[u8]) -> napi::Result<()> {
    ensure_kagemusha_recursive_archive_len(
        archive.len(),
        "Kagemusha recursive spend Pallas open-envelope archive",
    )
}

fn is_kagemusha_recursive_compact_unavailable_error(err: &str) -> bool {
    matches!(
        err,
        iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE
            | iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
    )
}

/// Native ABI level required by the recursive Kagemusha spend helpers.
#[napi(js_name = "connectNoritoBridgeAbiVersion")]
pub fn connect_norito_bridge_abi_version() -> u32 {
    7
}

/// Prove a record-backed Kagemusha compact payment token.
#[napi(js_name = "kagemushaProveVerifiedCompactPaymentTokenWithRecords")]
pub fn kagemusha_prove_verified_compact_payment_token_with_records(
    record_bundle_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle =
        decode_kagemusha_recursive_archive(&record_bundle_archive, "Kagemusha record bundle")?;
    let vk_box = iroha_core::zk::kagemusha_folded_vk_box()
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    let token = iroha_core::zk::prove_verified_kagemusha_compact_payment_token_from_record_bundle(
        &record_bundle,
        iroha_core::zk::KAGEMUSHA_FOLDED_CIRCUIT_ID,
        &vk_box,
        None,
    )
    .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    encode_kagemusha_recursive_archive(&token, "serialize Kagemusha compact payment-token archive")
}

/// Prove a record-backed Kagemusha recursive aggregation proof bundle.
#[napi(
    js_name = "kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes"
)]
pub fn kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes(
    record_bundle_archive: Uint8Array,
    pallas_open_envelopes_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle =
        decode_kagemusha_recursive_archive(&record_bundle_archive, "Kagemusha record bundle")?;
    ensure_kagemusha_recursive_archive_len(
        pallas_open_envelopes_archive.len(),
        "pallasOpenEnvelopesArchive",
    )?;
    let vk_box = iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    let proof_bundle =
        iroha_core::zk::prove_verified_kagemusha_recursive_aggregation_proof_bundle_from_record_bundle_and_pallas_open_envelope_archive(
            &record_bundle,
            pallas_open_envelopes_archive.as_ref(),
            iroha_core::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID,
            &vk_box,
            None,
        )
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    encode_kagemusha_recursive_archive(
        &proof_bundle,
        "serialize Kagemusha recursive aggregation proof-bundle archive",
    )
}

/// Prove an ABI-7 recursive compact Kagemusha payment token.
#[napi(
    js_name = "kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes"
)]
pub fn kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
    record_bundle_archive: Uint8Array,
    pallas_open_envelopes_archive: Uint8Array,
    recursive_compact_key_artifacts_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle =
        decode_kagemusha_recursive_archive(&record_bundle_archive, "Kagemusha record bundle")?;
    let key_artifacts: iroha_data_model::offline::KagemushaRecursiveCompactKeyArtifactsV1 =
        decode_kagemusha_recursive_archive(
            &recursive_compact_key_artifacts_archive,
            "Kagemusha recursive compact key artifacts",
        )?;
    ensure_kagemusha_recursive_archive_len(
        pallas_open_envelopes_archive.len(),
        "pallasOpenEnvelopesArchive",
    )?;
    let token =
        iroha_core::zk::prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts(
            &record_bundle,
            pallas_open_envelopes_archive.as_ref(),
            &key_artifacts,
        )
        .map_err(|err| {
            if err.starts_with(
                "failed to decode Kagemusha recursive compact Pallas open-envelope archive",
            ) || err.starts_with(
                "invalid Kagemusha recursive compact Pallas open-envelope archive",
            ) || err.starts_with(
                "invalid Kagemusha recursive compact record-backed Pallas preflight",
            ) {
                return napi::Error::new(
                    napi::Status::InvalidArg,
                    err.replacen("failed to decode", "invalid", 1),
                );
            }
            napi::Error::new(napi::Status::GenericFailure, err)
        })?;
    encode_kagemusha_recursive_archive(
        &token,
        "serialize Kagemusha recursive compact payment-token archive",
    )
}

/// Project a recursive spend bundle into an ABI-7 recursive compact Kagemusha payment token.
#[napi(js_name = "kagemushaRecursiveSpendCompactPaymentTokenFromBundle")]
pub fn kagemusha_recursive_spend_compact_payment_token_from_bundle(
    bundle_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 =
        decode_kagemusha_recursive_archive(
            &bundle_archive,
            "Kagemusha recursive spend compact-token bundle",
        )?;
    let token =
        iroha_data_model::offline::kagemusha_recursive_spend_compact_payment_token_from_bundle(
            &bundle,
        )
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    encode_kagemusha_recursive_archive(
        &token,
        "serialize Kagemusha recursive spend compact payment-token archive",
    )
}

fn verify_kagemusha_recursive_spend_compact_payment_token_projection_inner(
    compact_token_archive: &Uint8Array,
    verifier_record_archive: &Uint8Array,
    block_height: Option<u64>,
) -> napi::Result<bool> {
    let token: iroha_data_model::offline::KagemushaCompactPaymentToken =
        decode_kagemusha_recursive_archive(
            compact_token_archive,
            "Kagemusha recursive spend compact projection token",
        )?;
    let record: iroha_data_model::proof::VerifyingKeyRecord = decode_kagemusha_recursive_archive(
        verifier_record_archive,
        "Kagemusha recursive spend compact projection verifier record",
    )?;
    match block_height {
        Some(height) => iroha_core::zk::preverify_kagemusha_recursive_spend_compact_payment_token_projection_with_record_at_height(
            &token,
            &record,
            height,
        ),
        None => iroha_core::zk::preverify_kagemusha_recursive_spend_compact_payment_token_projection_with_record(
            &token,
            &record,
        ),
    }
    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
    Ok(match block_height {
        Some(height) => iroha_core::zk::verify_kagemusha_recursive_spend_compact_payment_token_projection_with_record_at_height(
            &token,
            &record,
            height,
        ),
        None => iroha_core::zk::verify_kagemusha_recursive_spend_compact_payment_token_projection_with_record(
            &token,
            &record,
        ),
    })
}

/// Verify a projected recursive spend compact Kagemusha payment token against a lineage verifier record.
#[napi(js_name = "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjection")]
pub fn kagemusha_verify_recursive_spend_compact_payment_token_projection(
    compact_token_archive: Uint8Array,
    verifier_record_archive: Uint8Array,
) -> napi::Result<bool> {
    verify_kagemusha_recursive_spend_compact_payment_token_projection_inner(
        &compact_token_archive,
        &verifier_record_archive,
        None,
    )
}

/// Verify a projected recursive spend compact Kagemusha payment token against a lineage verifier record at `block_height`.
#[napi(js_name = "kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight")]
pub fn kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height(
    compact_token_archive: Uint8Array,
    verifier_record_archive: Uint8Array,
    block_height: JsU64,
) -> napi::Result<bool> {
    verify_kagemusha_recursive_spend_compact_payment_token_projection_inner(
        &compact_token_archive,
        &verifier_record_archive,
        Some(block_height.into()),
    )
}

/// Verify an ABI-7 recursive compact Kagemusha payment token.
#[napi(js_name = "kagemushaVerifyRecursiveCompactPaymentToken")]
pub fn kagemusha_verify_recursive_compact_payment_token(
    compact_token_archive: Uint8Array,
    recursive_compact_verifier_keys_archive: Uint8Array,
) -> napi::Result<bool> {
    let token: iroha_data_model::offline::KagemushaCompactPaymentToken =
        decode_kagemusha_recursive_archive(
            &compact_token_archive,
            "Kagemusha recursive compact payment token",
        )?;
    let verifier_keys: iroha_data_model::offline::KagemushaRecursiveCompactVerifierKeysV1 =
        decode_kagemusha_recursive_archive(
            &recursive_compact_verifier_keys_archive,
            "Kagemusha recursive compact verifier keys",
        )?;
    let vk_box =
        iroha_core::zk::kagemusha_recursive_compact_payment_token_verifier_key_from_package(
            &token,
            &verifier_keys,
        )
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    match iroha_core::zk::preverify_kagemusha_recursive_compact_payment_token(&token, vk_box) {
        Err(err) if is_kagemusha_recursive_compact_unavailable_error(&err) => {
            return Ok(false);
        }
        Err(err) => return Err(napi::Error::new(napi::Status::InvalidArg, err)),
        Ok(()) => {}
    }
    if iroha_core::zk::verify_kagemusha_recursive_compact_payment_token(&token, vk_box) {
        return Ok(true);
    }
    Ok(false)
}

/// Build the initial recursive Kagemusha spend bundle from a raw Norito request archive.
#[napi(js_name = "kagemushaRecursiveSpendInit")]
pub fn kagemusha_recursive_spend_init(request_archive: Uint8Array) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 =
        decode_kagemusha_recursive_archive(&request_archive, "Kagemusha recursive spend init")?;
    ensure_kagemusha_recursive_spend_pallas_archive(&request.pallas_open_envelopes_archive)?;
    request
        .validate_public_binding()
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    let lineage_verifier_key = request.lineage_verifier_key.as_ref().ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "Kagemusha Reserved-lineage init requires lineage_verifier_key",
        )
    })?;
    let lineage_proving_key_archive =
        request
            .lineage_proving_key_archive
            .as_deref()
            .ok_or_else(|| {
                napi::Error::new(
                    napi::Status::InvalidArg,
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
    .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    encode_kagemusha_recursive_archive(
        &bundle,
        "failed to encode Kagemusha recursive spend init bundle",
    )
}

/// Append one offline hop to a recursive Kagemusha spend bundle from a raw Norito request archive.
#[napi(js_name = "kagemushaRecursiveSpendAppend")]
pub fn kagemusha_recursive_spend_append(request_archive: Uint8Array) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 =
        decode_kagemusha_recursive_archive(&request_archive, "Kagemusha recursive spend append")?;
    ensure_kagemusha_recursive_spend_pallas_archive(&request.pallas_open_envelopes_archive)?;
    request
        .validate_public_binding()
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    let output_proof_circuit_id = request.output_proof_circuit_id().to_owned();
    let output_append_is_currently_provable =
        iroha_data_model::offline::can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
            output_proof_circuit_id.as_str(),
            request.previous_bundle.accumulator.hop_count,
        );
    if !output_append_is_currently_provable {
        return Err(napi::Error::new(
            napi::Status::GenericFailure,
            format!(
                "Kagemusha recursive spend append cannot prove output proof circuit `{}` at previous hop {}",
                output_proof_circuit_id, request.previous_bundle.accumulator.hop_count,
            ),
        ));
    }
    let mut lineage_proving_key_archive = None;
    let vk_box = match output_proof_circuit_id.as_str() {
        iroha_core::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID => {
            iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
                .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?
        }
        output_circuit
            if iroha_data_model::offline::is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
                output_circuit,
            ) =>
        {
            lineage_proving_key_archive =
                Some(request.lineage_proving_key_archive.as_deref().ok_or_else(|| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        "Kagemusha Reserved-lineage append requires lineage_proving_key_archive",
                    )
                })?);
            request.lineage_verifier_key.clone().ok_or_else(|| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    "Kagemusha Reserved-lineage append requires lineage_verifier_key",
                )
            })?
        }
        other => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!(
                    "Kagemusha recursive spend append requires a supported output proof circuit id (found `{other}`)"
                ),
            ));
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
    .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    encode_kagemusha_recursive_archive(
        &bundle,
        "failed to encode Kagemusha recursive spend append bundle",
    )
}

/// Build the canonical Reserved-lineage transition profile from an init request archive.
#[napi(js_name = "kagemushaRecursiveSpendTransitionProfileInit")]
pub fn kagemusha_recursive_spend_transition_profile_init(
    request_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 =
        decode_kagemusha_recursive_archive(
            &request_archive,
            "Kagemusha recursive spend transition profile init",
        )?;
    ensure_kagemusha_recursive_spend_pallas_archive(&request.pallas_open_envelopes_archive)?;
    request
        .validate_public_binding()
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    let evidence = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelope_archive_at_height(
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
                block_height,
            )
        }
        None => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelope_archive(
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
            )
        }
    }
    .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    let profile =
        iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_from_initial_evidence(
            &evidence,
            &request.current_note,
        )
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    encode_kagemusha_recursive_archive(
        &profile,
        "failed to encode Kagemusha recursive spend transition profile",
    )
}

/// Build the canonical Reserved-lineage transition profile from an append request archive.
#[napi(js_name = "kagemushaRecursiveSpendTransitionProfileAppend")]
pub fn kagemusha_recursive_spend_transition_profile_append(
    request_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 =
        decode_kagemusha_recursive_archive(
            &request_archive,
            "Kagemusha recursive spend transition profile append",
        )?;
    ensure_kagemusha_recursive_spend_pallas_archive(&request.pallas_open_envelopes_archive)?;
    request
        .validate_public_binding()
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    let evidence = match request.block_height {
        Some(block_height) => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelope_archive_at_height(
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
                block_height,
            )
        }
        None => {
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelope_archive(
                &request.record_bundle,
                &request.pallas_open_envelopes_archive,
            )
        }
    }
    .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
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
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?
    } else {
        let hop = request.record_bundle.bundle.steps.first().ok_or_else(|| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "Kagemusha recursive spend append request has no current hop",
            )
        })?;
        let current_hop_proof_hash =
            iroha_core::zk::kagemusha_fold_step_proof_hash(&hop.attachment.proof)
                .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
        let append_opening_preflight =
            iroha_core::zk::kagemusha_recursive_spend_lineage_append_opening_preflight_from_archives(
                &request.previous_bundle,
                &request.previous_recursive_proof_open_envelopes_archive,
                &current_hop_proof_hash,
                &request.pallas_open_envelopes_archive,
            )
            .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
        iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
            &request.previous_bundle.accumulator,
            &request.previous_bundle.recursive_proof,
            &request.previous_recursive_proof_open_envelopes_archive,
            append_opening_preflight.contract,
            &evidence,
            &request.current_note,
        )
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?
    };
    encode_kagemusha_recursive_archive(
        &profile,
        "failed to encode Kagemusha recursive spend transition profile",
    )
}

/// Build the compact Reserved-lineage append boundary from a transition profile archive.
#[napi(js_name = "kagemushaRecursiveSpendLineageAppendBoundary")]
pub fn kagemusha_recursive_spend_lineage_append_boundary(
    profile_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
        decode_kagemusha_recursive_archive(
            &profile_archive,
            "Kagemusha recursive spend lineage append boundary",
        )?;
    let boundary =
        iroha_data_model::offline::kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
            &profile,
        )
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    boundary
        .validate_against_transition_profile(&profile)
        .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err.to_string()))?;
    encode_kagemusha_recursive_archive(
        &boundary,
        "failed to encode Kagemusha recursive spend lineage append boundary",
    )
}

/// Build the initial recursive Kagemusha spend lineage witness from raw Norito archives.
#[napi(js_name = "kagemushaRecursiveSpendLineageWitnessFromInitResult")]
pub fn kagemusha_recursive_spend_lineage_witness_from_init_result(
    request_archive: Uint8Array,
    bundle_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 =
        decode_kagemusha_recursive_archive(
            &request_archive,
            "Kagemusha recursive spend lineage witness init request",
        )?;
    ensure_kagemusha_recursive_spend_pallas_archive(&request.pallas_open_envelopes_archive)?;
    let bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 =
        decode_kagemusha_recursive_archive(
            &bundle_archive,
            "Kagemusha recursive spend lineage witness init bundle",
        )?;
    let witness =
        iroha_data_model::offline::kagemusha_recursive_spend_lineage_witness_from_init_result(
            &request, &bundle,
        )
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))?;
    encode_kagemusha_recursive_archive(
        &witness,
        "failed to encode Kagemusha recursive spend lineage witness",
    )
}

/// Append one hop of recursive Kagemusha spend lineage witness material from raw Norito archives.
#[napi(js_name = "kagemushaRecursiveSpendLineageWitnessAppendResult")]
pub fn kagemusha_recursive_spend_lineage_witness_append_result(
    previous_witness_archive: Uint8Array,
    request_archive: Uint8Array,
    bundle_archive: Uint8Array,
) -> napi::Result<Buffer> {
    let previous_witness: iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 =
        decode_kagemusha_recursive_archive(
            &previous_witness_archive,
            "Kagemusha recursive spend previous lineage witness",
        )?;
    let request: iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 =
        decode_kagemusha_recursive_archive(
            &request_archive,
            "Kagemusha recursive spend lineage witness append request",
        )?;
    ensure_kagemusha_recursive_spend_pallas_archive(&request.pallas_open_envelopes_archive)?;
    let bundle: iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 =
        decode_kagemusha_recursive_archive(
            &bundle_archive,
            "Kagemusha recursive spend lineage witness append bundle",
        )?;
    let witness =
        iroha_data_model::offline::kagemusha_recursive_spend_lineage_witness_append_result(
            &previous_witness,
            &request,
            &bundle,
        )
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))?;
    encode_kagemusha_recursive_archive(
        &witness,
        "failed to encode Kagemusha recursive spend lineage witness",
    )
}

/// Verify a recursive Kagemusha spend bundle from a raw Norito request archive.
#[napi(js_name = "kagemushaRecursiveSpendVerify")]
pub fn kagemusha_recursive_spend_verify(request_archive: Uint8Array) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1 =
        decode_kagemusha_recursive_archive(&request_archive, "Kagemusha recursive spend verify")?;
    request
        .validate_public_binding()
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))?;
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
    .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;
    encode_kagemusha_recursive_archive(
        &result,
        "failed to encode Kagemusha recursive spend verify result",
    )
}

/// Build the online redeem instruction from a recursive Kagemusha spend redeem request archive.
#[napi(js_name = "kagemushaRecursiveSpendRedeem")]
pub fn kagemusha_recursive_spend_redeem(request_archive: Uint8Array) -> napi::Result<Buffer> {
    let request: iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1 =
        decode_kagemusha_recursive_archive(&request_archive, "Kagemusha recursive spend redeem")?;
    let instruction = kagemusha_recursive_spend_redeem_instruction_from_request(request)?;
    encode_kagemusha_recursive_archive(
        &instruction,
        "failed to encode Kagemusha recursive spend redeem instruction",
    )
}

fn kagemusha_recursive_spend_redeem_instruction_from_request(
    request: iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1,
) -> napi::Result<iroha_data_model::isi::offline::RedeemKagemushaRecursive> {
    request
        .validate_public_binding()
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))?;
    if let Some(lineage_witness) = &request.lineage_witness {
        match request.bundle.recursive_proof.verifier_key_id.name.as_str() {
            iroha_core::zk::KAGEMUSHA_RECURSIVE_AGGREGATION_CIRCUIT_ID => {
                let vk_box = iroha_core::zk::kagemusha_recursive_aggregation_proof_vk_box()
                    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))?;
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
                    }
                    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
                    if !iroha_core::zk::verify_kagemusha_recursive_spend_bundle(
                        &request.bundle,
                        &vk_box,
                    ) {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            "record-backed recursive Kagemusha lineage final proof did not verify",
                        ));
                    }
                } else {
                    iroha_core::zk::verify_kagemusha_recursive_spend_lineage_witness_and_bundle_with_vk_box(
                        &request.bundle,
                        lineage_witness,
                        &vk_box,
                    )
                    .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
                }
            }
            circuit_id
                if iroha_data_model::offline::is_kagemusha_recursive_spend_lineage_proof_circuit_id(
                    circuit_id,
                ) =>
            {
                let record = request.lineage_verifier_record.as_ref().ok_or_else(|| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        "reserved-lineage Kagemusha recursive spend redeem requires a lineage verifier record",
                    )
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
                }
                .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
            }
            other => {
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    format!(
                        "Kagemusha recursive spend redeem requires a supported proof circuit id (found `{other}`)"
                    ),
                ));
            }
        }
    } else {
        iroha_core::zk::ensure_kagemusha_recursive_spend_chain_admission_proves_lineage(
            &request.bundle,
        )
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
        if iroha_data_model::offline::is_kagemusha_recursive_spend_lineage_proof_circuit_id(
            &request.bundle.recursive_proof.verifier_key_id.name,
        ) {
            let record = request.lineage_verifier_record.as_ref().ok_or_else(|| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    "reserved-lineage Kagemusha recursive spend redeem requires a lineage verifier record",
                )
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
            }
            .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err))?;
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
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "reserved-lineage Kagemusha recursive spend proof did not verify",
                ));
            }
        }
    }
    let instruction =
        iroha_data_model::isi::offline::RedeemKagemushaRecursive::new_with_lineage_witness(
            request.bundle,
            request.recipient,
            request.public_amount,
            request.redeem_proof,
            request.lineage_witness,
        );
    Ok(instruction)
}

/// Produce the canonical SM2 fixture output for the given distinguishing ID, seed, and message.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn sm2_fixture_from_seed(
    distid: String,
    seed: Uint8Array,
    message: Uint8Array,
) -> napi::Result<JsSm2Fixture> {
    let seed_bytes = seed.to_vec();
    let message_bytes = message.to_vec();
    let private = Sm2PrivateKey::from_seed(distid.as_str(), &seed_bytes).map_err(norito_to_napi)?;
    let public = private.public_key();
    let secret_hex = hex::encode_upper(private.secret_bytes());
    let public_bytes = public.to_sec1_bytes(false);
    let public_hex = hex::encode_upper(&public_bytes);
    let payload =
        encode_sm2_public_key_payload(distid.as_str(), &public_bytes).map_err(norito_to_napi)?;
    let public_key = PublicKey::from_bytes(Algorithm::Sm2, &payload).map_err(norito_to_napi)?;
    let multihash = public_key
        .try_to_multihash_string()
        .map_err(norito_to_napi)?;
    let prefixed = public_key
        .try_to_prefixed_string()
        .map_err(norito_to_napi)?;
    let za = public.compute_z(distid.as_str()).map_err(norito_to_napi)?;
    let za_hex = hex::encode_upper(za);
    let signature = private.sign(&message_bytes);
    let signature_hex = hex::encode_upper(signature.as_bytes());
    let r_hex = hex::encode_upper(signature.r);
    let s_hex = hex::encode_upper(signature.s);

    Ok(JsSm2Fixture {
        distid,
        seed_hex: hex::encode_upper(seed_bytes),
        message_hex: hex::encode_upper(message_bytes),
        private_key_hex: secret_hex,
        public_key_sec1_hex: public_hex,
        public_key_multihash: multihash,
        public_key_prefixed: prefixed,
        za: za_hex,
        signature: signature_hex,
        r: r_hex,
        s: s_hex,
    })
}

fn sm2_distid_arg(distid: Option<String>) -> String {
    distid.unwrap_or_else(Sm2PublicKey::default_distid)
}

fn parse_sm2_private_key(distid: Option<String>, bytes: &[u8]) -> napi::Result<Sm2PrivateKey> {
    if bytes.len() != SM2_PRIVATE_KEY_LENGTH {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "sm2 private key must be {SM2_PRIVATE_KEY_LENGTH} bytes, got {}",
                bytes.len()
            ),
        ));
    }
    let distid = sm2_distid_arg(distid);
    Sm2PrivateKey::from_bytes(distid, bytes)
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))
}

fn parse_sm2_public_key(distid: Option<String>, bytes: &[u8]) -> napi::Result<Sm2PublicKey> {
    if bytes.len() != SM2_PUBLIC_KEY_LENGTH {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "sm2 public key must be {SM2_PUBLIC_KEY_LENGTH} bytes, got {}",
                bytes.len()
            ),
        ));
    }
    let distid = sm2_distid_arg(distid);
    Sm2PublicKey::from_sec1_bytes(distid, bytes)
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))
}

fn parse_sm2_signature(bytes: &[u8]) -> napi::Result<Sm2Signature> {
    if bytes.len() != SM2_SIGNATURE_LENGTH {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "sm2 signature must be {SM2_SIGNATURE_LENGTH} bytes, got {}",
                bytes.len()
            ),
        ));
    }
    let mut array = [0u8; SM2_SIGNATURE_LENGTH];
    array.copy_from_slice(bytes);
    Sm2Signature::from_bytes(&array)
        .map_err(|err| napi::Error::new(napi::Status::InvalidArg, err.to_string()))
}

fn account_address_err(err: AccountAddressError) -> napi::Error {
    napi::Error::new(
        napi::Status::InvalidArg,
        format!("{}: {err}", err.code_str()),
    )
}

fn norito_to_napi<E: fmt::Display>(error: E) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, error.to_string())
}

fn sign_js_transaction(
    builder: TransactionBuilder,
    private_key: &PrivateKey,
    context: &str,
) -> napi::Result<SignedTransaction> {
    builder
        .try_sign(private_key)
        .map_err(|err| norito_to_napi(format!("failed to sign {context} transaction: {err}",)))
}

fn alias_policy_from_js(policy: Option<&JsAliasPolicy>) -> napi::Result<AliasCachePolicy> {
    let mut positive = SORAFS_ALIAS_POSITIVE_TTL_SECS;
    let mut refresh = SORAFS_ALIAS_REFRESH_WINDOW_SECS;
    let mut hard = SORAFS_ALIAS_HARD_EXPIRY_SECS;
    let mut negative = SORAFS_ALIAS_NEGATIVE_TTL_SECS;
    let mut revocation = SORAFS_ALIAS_REVOCATION_TTL_SECS;
    let mut rotation = SORAFS_ALIAS_ROTATION_MAX_AGE_SECS;
    let mut successor = SORAFS_ALIAS_SUCCESSOR_GRACE_SECS;
    let mut governance = SORAFS_ALIAS_GOVERNANCE_GRACE_SECS;

    if let Some(policy) = policy {
        if let Some(value) = policy.positive_ttl_secs {
            positive = ensure_positive(value, "positiveTtlSecs")?;
        }
        if let Some(value) = policy.refresh_window_secs {
            refresh = ensure_positive(value, "refreshWindowSecs")?;
        }
        if let Some(value) = policy.hard_expiry_secs {
            hard = ensure_positive(value, "hardExpirySecs")?;
        }
        if let Some(value) = policy.negative_ttl_secs {
            negative = ensure_positive(value, "negativeTtlSecs")?;
        }
        if let Some(value) = policy.revocation_ttl_secs {
            revocation = ensure_positive(value, "revocationTtlSecs")?;
        }
        if let Some(value) = policy.rotation_max_age_secs {
            rotation = ensure_positive(value, "rotationMaxAgeSecs")?;
        }
        if let Some(value) = policy.successor_grace_secs {
            successor = ensure_positive(value, "successorGraceSecs")?;
        }
        if let Some(value) = policy.governance_grace_secs {
            governance = ensure_non_negative(value, "governanceGraceSecs")?;
        }
    }

    if refresh > positive {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "refreshWindowSecs must not exceed positiveTtlSecs",
        ));
    }
    if hard < positive {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "hardExpirySecs must be greater than or equal to positiveTtlSecs",
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

fn ensure_positive(value: i64, name: &str) -> napi::Result<u64> {
    if value <= 0 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{name} must be greater than zero"),
        ));
    }
    u64::try_from(value).map_err(|_| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{name} must fit within the JavaScript number range"),
        )
    })
}

fn ensure_non_negative(value: i64, name: &str) -> napi::Result<u64> {
    if value < 0 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{name} must be zero or positive"),
        ));
    }
    u64::try_from(value).map_err(|_| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{name} must fit within the JavaScript number range"),
        )
    })
}

fn parse_hex_bytes(input: &str, context: &str) -> napi::Result<Vec<u8>> {
    let trimmed = input.trim_start_matches("0x");
    if !trimmed.len().is_multiple_of(2) {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must contain an even number of hex characters"),
        ));
    }
    hex::decode(trimmed).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("failed to decode {context}: {err}"),
        )
    })
}

/// Return the default alias cache policy used by `SoraFS` gateways.
#[napi]
pub fn sorafs_alias_policy_defaults() -> JsAliasPolicy {
    JsAliasPolicy {
        positive_ttl_secs: Some(
            i64::try_from(SORAFS_ALIAS_POSITIVE_TTL_SECS).expect("alias TTL fits in i64"),
        ),
        refresh_window_secs: Some(
            i64::try_from(SORAFS_ALIAS_REFRESH_WINDOW_SECS).expect("refresh window fits in i64"),
        ),
        hard_expiry_secs: Some(
            i64::try_from(SORAFS_ALIAS_HARD_EXPIRY_SECS).expect("hard expiry fits in i64"),
        ),
        negative_ttl_secs: Some(
            i64::try_from(SORAFS_ALIAS_NEGATIVE_TTL_SECS).expect("negative TTL fits in i64"),
        ),
        revocation_ttl_secs: Some(
            i64::try_from(SORAFS_ALIAS_REVOCATION_TTL_SECS).expect("revocation TTL fits in i64"),
        ),
        rotation_max_age_secs: Some(
            i64::try_from(SORAFS_ALIAS_ROTATION_MAX_AGE_SECS).expect("rotation age fits in i64"),
        ),
        successor_grace_secs: Some(
            i64::try_from(SORAFS_ALIAS_SUCCESSOR_GRACE_SECS).expect("successor grace fits in i64"),
        ),
        governance_grace_secs: Some(
            i64::try_from(SORAFS_ALIAS_GOVERNANCE_GRACE_SECS)
                .expect("governance grace fits in i64"),
        ),
    }
}

/// Evaluate an alias proof bundle against the provided or default policy.
#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String`/struct inputs
#[napi]
pub fn sorafs_evaluate_alias_proof(
    proof_b64: String,
    policy: Option<JsAliasPolicy>,
    now_secs: Option<i64>,
) -> napi::Result<JsAliasEvaluation> {
    let policy = alias_policy_from_js(policy.as_ref())?;
    let now = match now_secs {
        Some(value) => u64::try_from(value).map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "now_secs must be non-negative and fit within JavaScript number range",
            )
        })?,
        None => unix_now_secs(),
    };
    let trimmed = proof_b64.trim();
    if trimmed.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "proof must not be empty",
        ));
    }
    let proof_bytes = STANDARD.decode(trimmed.as_bytes()).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("failed to decode base64 proof: {err}"),
        )
    })?;
    let bundle = decode_alias_proof(&proof_bytes).map_err(norito_to_napi)?;
    let evaluation = policy.evaluate(&bundle, now);
    let state = match evaluation.state {
        AliasProofState::Fresh => "fresh",
        AliasProofState::RefreshWindow => "refresh_window",
        AliasProofState::Expired => "expired",
        AliasProofState::HardExpired => "hard_expired",
    }
    .to_owned();
    let status_label = evaluation.status_label().to_owned();
    Ok(JsAliasEvaluation {
        state,
        status_label,
        rotation_due: evaluation.rotation_due,
        age_seconds: i64::try_from(evaluation.age.as_secs()).expect("age fits in i64"),
        generated_at_unix: i64::try_from(evaluation.generated_at_unix)
            .expect("generated_at fits in i64"),
        expires_at_unix: i64::try_from(evaluation.expires_at_unix).expect("expires_at fits in i64"),
        expires_in_seconds: evaluation
            .expires_in
            .map(|dur| i64::try_from(dur.as_secs()).expect("expires_in fits in i64")),
        servable: evaluation.state.is_servable(),
    })
}

fn resolve_manifest_cid(opts: &JsAliasProofFixtureOptions) -> napi::Result<Vec<u8>> {
    if let Some(hex) = opts.manifest_cid_hex.as_ref() {
        let cid = parse_hex_bytes(hex, "manifestCidHex")?;
        if cid.is_empty() {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                "manifestCidHex must not decode to an empty value",
            ));
        }
        Ok(cid)
    } else {
        Ok(vec![0xAA, 0xBB])
    }
}

fn resolve_fixture_timestamps(
    opts: &JsAliasProofFixtureOptions,
    now: u64,
) -> napi::Result<(u64, u64)> {
    let generated = match opts.generated_at_unix {
        Some(value) => u64::try_from(value).map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "generatedAtUnix must be non-negative and fit in JavaScript number range",
            )
        })?,
        None => now.saturating_sub(60),
    };
    let expires = match opts.expires_at_unix {
        Some(value) => u64::try_from(value).map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "expiresAtUnix must be non-negative and fit in JavaScript number range",
            )
        })?,
        None => generated + SORAFS_ALIAS_POSITIVE_TTL_SECS,
    };
    if expires <= generated {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "expiresAtUnix must be greater than generatedAtUnix",
        ));
    }
    Ok((generated, expires))
}

fn resolve_fixture_epochs(opts: &JsAliasProofFixtureOptions) -> napi::Result<(u64, u64)> {
    let bound_at = opts
        .bound_at_epoch
        .map(u64::try_from)
        .transpose()
        .map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "boundAtEpoch must be non-negative and fit in JavaScript number range",
            )
        })?
        .unwrap_or(1);
    let expiry_epoch = opts
        .expiry_epoch
        .map(u64::try_from)
        .transpose()
        .map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "expiryEpoch must be non-negative and fit in JavaScript number range",
            )
        })?
        .unwrap_or(bound_at + 100);
    Ok((bound_at, expiry_epoch))
}

fn sign_bundle_with_council(bundle: &mut AliasProofBundleV1) -> napi::Result<()> {
    let root = alias_merkle_root(&bundle.binding, &bundle.merkle_path).map_err(norito_to_napi)?;
    bundle.registry_root = root;
    let digest = alias_proof_signature_digest(bundle);
    let keypair = KeyPair::from_private_key(
        PrivateKey::from_bytes(Algorithm::Ed25519, &[0x55; 32]).expect("seeded key"),
    )
    .expect("derive keypair");
    let signature =
        Signature::try_new(keypair.private_key(), digest.as_ref()).map_err(norito_to_napi)?;
    let signer_bytes = checked_public_key_payload(keypair.public_key())?;
    let signer: [u8; 32] = signer_bytes
        .try_into()
        .map_err(|_| generic_failure("ed25519 public key must be 32 bytes"))?;
    bundle
        .council_signatures
        .push(sorafs_manifest::CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        });
    bundle.validate().map_err(norito_to_napi)
}

/// Produce a deterministic alias proof example for documentation and testing.
#[napi]
pub fn sorafs_alias_proof_fixture(
    options: Option<JsAliasProofFixtureOptions>,
) -> napi::Result<JsAliasProofFixture> {
    let opts = options.unwrap_or_default();
    let alias = opts.alias.as_deref().unwrap_or("docs/sora").to_owned();
    let manifest_cid = resolve_manifest_cid(&opts)?;
    let now = unix_now_secs();
    let (generated, expires) = resolve_fixture_timestamps(&opts, now)?;
    let (bound_at, expiry_epoch) = resolve_fixture_epochs(&opts)?;

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

    sign_bundle_with_council(&mut bundle)?;

    let proof_bytes = norito::to_bytes(&bundle).map_err(norito_to_napi)?;
    let proof_b64 = STANDARD.encode(proof_bytes);
    let registry_root_hex = hex::encode(bundle.registry_root);
    let generated_i64 = i64::try_from(generated).expect("generated fits in i64");
    let expires_i64 = i64::try_from(expires).expect("expires fits in i64");

    Ok(JsAliasProofFixture {
        proof_b64,
        alias,
        generated_at_unix: generated_i64,
        expires_at_unix: expires_i64,
        registry_root_hex,
        registry_height: i64::try_from(bundle.registry_height)
            .expect("registry height fits in i64"),
    })
}

#[napi(object)]
/// Provider descriptor used by `sorafsMultiFetchLocal`.
pub struct JsLocalProviderSpec {
    /// Human-readable provider identifier (emitted in receipts and reports).
    pub name: String,
    /// Filesystem path to the local chunk source backing this provider.
    pub path: String,
    /// Optional cap on concurrent chunk requests served from this provider.
    pub max_concurrent: Option<u32>,
    /// Optional weighting applied during scoreboard normalisation.
    pub weight: Option<u32>,
    /// Optional provider metadata (range capability, quotas, etc.).
    pub metadata: Option<JsProviderMetadata>,
}

#[napi(object)]
#[derive(Clone, Copy)]
/// Server-advertised chunk range limits for `SoraFS` providers.
pub struct JsRangeCapability {
    /// Maximum contiguous chunk span that the provider can deliver.
    pub max_chunk_span: u32,
    /// Smallest chunk granularity the provider supports.
    pub min_granularity: u32,
    /// Whether the provider can fetch discontiguous chunk offsets in one request.
    pub supports_sparse_offsets: Option<bool>,
    /// Whether fetch requests must align on chunk boundaries.
    pub requires_alignment: Option<bool>,
    /// Whether the provider can attach Merkle proofs alongside chunks.
    pub supports_merkle_proof: Option<bool>,
}

#[napi(object)]
#[derive(Clone, Copy)]
/// Concurrency and throughput quotas enforced during orchestrated fetches.
pub struct JsStreamBudget {
    /// Maximum simultaneous chunks placed in-flight for this provider.
    pub max_in_flight: u16,
    /// Sustained byte-per-second limit for the stream.
    pub max_bytes_per_sec: JsU64,
    /// Optional burst allowance expressed in bytes.
    pub burst_bytes: Option<JsU64>,
}

#[napi(object)]
/// Transport hint describing how to reach a provider.
pub struct JsTransportHint {
    /// Transport protocol label understood by the orchestrator.
    pub protocol: String,
    /// Integer protocol identifier used in orchestrator internals.
    pub protocol_id: u8,
    /// Relative priority applied when choosing between multiple hints.
    pub priority: u8,
}

/// Lossless wrapper that accepts JavaScript `number` (within safe range) or `bigint` and stores it as `u64`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsU64(pub u64);

impl TypeName for JsU64 {
    fn type_name() -> &'static str {
        "number | bigint"
    }

    fn value_type() -> ValueType {
        ValueType::Unknown
    }
}

impl ValidateNapiValue for JsU64 {
    #[allow(unsafe_code)]
    unsafe fn validate(
        env: sys::napi_env,
        napi_val: sys::napi_value,
    ) -> napi::Result<sys::napi_value> {
        match napi::type_of!(env, napi_val)? {
            ValueType::Number | ValueType::BigInt => Ok(ptr::null_mut()),
            other => Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("expected number or bigint, received {other}"),
            )),
        }
    }
}

impl FromNapiValue for JsU64 {
    #[allow(unsafe_code)]
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        match napi::type_of!(env, napi_val)? {
            ValueType::Number => {
                let mut raw = 0f64;
                let raw_ptr = ptr::addr_of_mut!(raw);
                unsafe {
                    napi::check_status!(sys::napi_get_value_double(env, napi_val, raw_ptr))?;
                }
                if !raw.is_finite() {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "expected finite number",
                    ));
                }
                if raw < 0.0 {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "expected non-negative number",
                    ));
                }
                if raw.fract() != 0.0 {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "expected integer-valued number",
                    ));
                }
                if raw > JS_MAX_SAFE_INTEGER {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "number exceeds JavaScript safe integer range; use bigint",
                    ));
                }
                let coerced = {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        raw as u64
                    }
                };
                Ok(Self(coerced))
            }
            ValueType::BigInt => {
                let bigint = unsafe { BigInt::from_napi_value(env, napi_val)? };
                let (sign_bit, value, lossless) = bigint.get_u64();
                if sign_bit {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "bigint must be non-negative",
                    ));
                }
                if !lossless {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "bigint exceeds u64 range",
                    ));
                }
                Ok(Self(value))
            }
            other => Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("expected number or bigint, received {other}"),
            )),
        }
    }
}

impl ToNapiValue for JsU64 {
    #[allow(unsafe_code)]
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        let mut raw = ptr::null_mut();
        let raw_ptr = ptr::addr_of_mut!(raw);
        unsafe {
            napi::check_status!(sys::napi_create_bigint_uint64(env, val.0, raw_ptr))?;
        }
        Ok(raw)
    }
}

impl From<JsU64> for u64 {
    fn from(value: JsU64) -> Self {
        value.0
    }
}

impl From<u64> for JsU64 {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[napi(object)]
/// Detailed provider metadata consumed by the orchestrator.
pub struct JsProviderMetadata {
    /// Optional provider identifier override; falls back to the alias when missing.
    pub provider_id: Option<String>,
    /// Optional identifier of the advertised profile.
    pub profile_id: Option<String>,
    /// Alternate aliases pointing at the same profile.
    pub profile_aliases: Option<Vec<String>>,
    /// Provider availability note (e.g., `"online"`, maintenance message).
    pub availability: Option<String>,
    /// Optional staking weight associated with the provider.
    pub stake_amount: Option<String>,
    /// Maximum concurrent stream slots offered by the provider.
    pub max_streams: Option<u32>,
    /// Deadline (Unix seconds) after which the provider should be refreshed.
    pub refresh_deadline: Option<JsU64>,
    /// Expiration timestamp for the metadata entry.
    pub expires_at: Option<JsU64>,
    /// Suggested time-to-live for cached metadata.
    pub ttl_secs: Option<JsU64>,
    /// Whether unknown capabilities should be tolerated.
    pub allow_unknown_capabilities: Option<bool>,
    /// Capability names declared by the provider.
    pub capability_names: Option<Vec<String>>,
    /// Rendezvous topics used during provider discovery.
    pub rendezvous_topics: Option<Vec<String>>,
    /// Free-form notes supplied by the provider.
    pub notes: Option<String>,
    /// Range capability information for chunk scheduling.
    pub range_capability: Option<JsRangeCapability>,
    /// Stream quota policy for orchestrated fetches.
    pub stream_budget: Option<JsStreamBudget>,
    /// Transport hints advertising available protocols.
    pub transport_hints: Option<Vec<JsTransportHint>>,
}

#[napi(object)]
/// Snapshot of provider telemetry inputs for scoreboard weighting.
pub struct JsTelemetryEntry {
    /// Provider identifier referenced by the telemetry source.
    pub provider_id: String,
    /// Quality of service score in the range [0, 1].
    pub qos_score: Option<f64>,
    /// Observed 95th percentile latency in milliseconds.
    pub latency_p95_ms: Option<f64>,
    /// Exponentially weighted median failure rate.
    pub failure_rate_ewma: Option<f64>,
    /// Token health metric used for staking-aware weighting.
    pub token_health: Option<f64>,
    /// Stake weight observed for the provider.
    pub staking_weight: Option<f64>,
    /// Whether the provider is currently penalised.
    pub penalty: Option<bool>,
    /// Last update timestamp expressed in Unix seconds.
    pub last_updated_unix: Option<JsU64>,
}

#[napi(object)]
/// Scoreboard boost configuration for a specific provider.
pub struct JsProviderBoost {
    /// Provider identifier to adjust.
    pub provider: String,
    /// Signed adjustment applied to the provider score.
    pub delta: i32,
}

#[napi(object)]
/// Scoreboard entry summarising eligibility and weighted score.
pub struct JsScoreboardEntry {
    /// Provider identifier tied to the scoreboard record.
    pub provider_id: String,
    /// Alias used for presenting the provider.
    pub alias: String,
    /// Raw score computed by the scoreboard.
    pub raw_score: f64,
    /// Normalised weight applied to scheduling decisions.
    pub normalized_weight: f64,
    /// Eligibility status string (or reason when ineligible).
    pub eligibility: String,
}

#[napi(object)]
#[derive(Default)]
/// Optional tuning knobs for the local multi-fetch helper.
pub struct JsMultiFetchOptions {
    /// Toggle Norito digest verification for each chunk.
    pub verify_digests: Option<bool>,
    /// Toggle byte-length verification for each chunk.
    pub verify_lengths: Option<bool>,
    /// Maximum number of retry attempts per chunk (>= 1).
    pub retry_budget: Option<u32>,
    /// Consecutive provider failures tolerated before disabling it.
    pub provider_failure_threshold: Option<u32>,
    /// Global parallelism limit applied to the orchestrator.
    pub max_parallel: Option<u32>,
    /// Upper bound for the number of providers considered eligible.
    pub max_peers: Option<u32>,
    /// Optional chunker handle used when deriving scoreboard plans.
    pub chunker_handle: Option<String>,
    /// Optional telemetry entries used when computing scoreboard weights.
    pub telemetry: Option<Vec<JsTelemetryEntry>>,
    /// Whether to derive provider weights from metadata + telemetry.
    pub use_scoreboard: Option<bool>,
    /// Override the scoreboard reference timestamp (Unix seconds).
    pub scoreboard_now_unix_secs: Option<JsU64>,
    /// Providers to skip deterministically via the scoring policy.
    pub deny_providers: Option<Vec<String>>,
    /// Providers to boost (positive) or penalise (negative) during scheduling.
    pub boost_providers: Option<Vec<JsProviderBoost>>,
    /// Include the computed scoreboard entries in the return payload.
    pub return_scoreboard: Option<bool>,
}

#[napi(object)]
/// Aggregate provider statistics produced by a multi-fetch run.
pub struct JsMultiFetchProviderReport {
    /// Provider identifier.
    pub provider: String,
    /// Number of successful chunk deliveries.
    pub successes: u32,
    /// Number of failed chunk attempts.
    pub failures: u32,
    /// Whether the provider was disabled due to failures.
    pub disabled: bool,
}

#[napi(object)]
/// Per-chunk execution details returned from multi-fetch.
pub struct JsMultiFetchChunkReceipt {
    /// Chunk index within the plan.
    pub chunk_index: u32,
    /// Provider that supplied the chunk.
    pub provider: String,
    /// Total attempts required until success.
    pub attempts: u32,
    /// Latency of the successful attempt in milliseconds.
    pub latency_ms: f64,
    /// Size of the chunk payload in bytes.
    pub bytes: u32,
}

#[napi(object)]
/// Result payload produced by `sorafsMultiFetchLocal`.
pub struct JsMultiFetchResult {
    /// Number of chunks assembled into the final payload.
    pub chunk_count: u32,
    /// Concatenated chunk payload (`CARv2` body) once the fetch completes.
    pub payload: Buffer,
    /// Summary statistics for each participating provider.
    pub provider_reports: Vec<JsMultiFetchProviderReport>,
    /// Receipts describing how each chunk was downloaded.
    pub chunk_receipts: Vec<JsMultiFetchChunkReceipt>,
    /// Optional scoreboard entries used for the fetch session.
    pub scoreboard: Option<Vec<JsScoreboardEntry>>,
}

/// Options controlling DA proof generation behaviour.
#[napi(object)]
#[derive(Default)]
pub struct JsDaProofOptions {
    /// Number of `PoR` leaves to sample deterministically (default: 8, min: 0).
    pub sample_count: Option<u32>,
    /// Seed forwarded to the deterministic sampler (default: 0).
    pub sample_seed: Option<JsU64>,
    /// Explicit `PoR` leaf indexes to verify.
    pub leaf_indexes: Option<Vec<u32>>,
}

/// Single proof-of-retrievability record returned to JavaScript callers.
/// Single proof-of-retrievability record returned to JavaScript callers.
#[derive(Clone)]
#[napi(object)]
pub struct JsDaProofRecord {
    /// Whether this proof was produced via sampling or explicit indexes.
    pub origin: String,
    /// Zero-based global leaf index covered by the proof.
    pub leaf_index: u32,
    /// Chunk index containing the leaf.
    pub chunk_index: u32,
    /// Segment index containing the leaf.
    pub segment_index: u32,
    /// Byte offset of the leaf slice.
    pub leaf_offset: JsU64,
    /// Leaf length in bytes.
    pub leaf_length: u32,
    #[doc = "Byte offset of the enclosing segment."]
    pub segment_offset: JsU64,
    #[doc = "Segment length in bytes."]
    pub segment_length: u32,
    #[doc = "Byte offset of the enclosing chunk."]
    pub chunk_offset: JsU64,
    #[doc = "Chunk length in bytes."]
    pub chunk_length: u32,
    #[doc = "Total payload length observed while proving."]
    pub payload_len: JsU64,
    #[doc = "Hex-encoded chunk digest."]
    pub chunk_digest_hex: String,
    #[doc = "Hex-encoded chunk Merkle root."]
    pub chunk_root_hex: String,
    #[doc = "Hex-encoded segment digest."]
    pub segment_digest_hex: String,
    #[doc = "Hex-encoded leaf digest."]
    pub leaf_digest_hex: String,
    #[doc = "Base64-encoded leaf bytes."]
    pub leaf_bytes_b64: String,
    #[doc = "Hex digests of sibling leaves within the segment."]
    pub segment_leaves_hex: Vec<String>,
    #[doc = "Hex digests for each segment-level branch."]
    pub chunk_segments_hex: Vec<String>,
    #[doc = "Hex digests for each chunk-level branch."]
    pub chunk_roots_hex: Vec<String>,
    #[doc = "Whether the proof verified against the supplied root."]
    pub verified: bool,
}

/// Summary describing manifest/payload `PoR` verification results.
#[napi(object)]
pub struct JsDaProofSummary {
    #[doc = "Hex-encoded manifest blob hash."]
    pub blob_hash_hex: String,
    #[doc = "Hex-encoded manifest chunk root."]
    pub chunk_root_hex: String,
    #[doc = "Hex-encoded `PoR` root derived from the payload."]
    pub por_root_hex: String,
    #[doc = "Total number of leaves observed."]
    pub leaf_count: JsU64,
    #[doc = "Total number of segments observed."]
    pub segment_count: JsU64,
    #[doc = "Total number of chunks observed."]
    pub chunk_count: JsU64,
    #[doc = "Number of deterministically sampled proofs."]
    pub sample_count: u32,
    #[doc = "Seed used for deterministic sampling."]
    pub sample_seed: JsU64,
    #[doc = "Number of proofs returned (sampled + explicit)."]
    pub proof_count: u32,
    #[doc = "Individual proof records corresponding to the manifest/payload set."]
    pub proofs: Vec<JsDaProofRecord>,
}

#[derive(Clone)]
struct DaProofOptionsNormalized {
    sample_count: usize,
    sample_seed: u64,
    explicit_indexes: Vec<usize>,
}

impl DaProofOptionsNormalized {
    fn from_js(options: Option<JsDaProofOptions>) -> napi::Result<Self> {
        let sample_count = options
            .as_ref()
            .and_then(|opts| opts.sample_count)
            .unwrap_or(8);
        let sample_seed = options
            .as_ref()
            .and_then(|opts| opts.sample_seed)
            .map_or(0, |seed| seed.0);
        let explicit_indexes = options
            .and_then(|opts| opts.leaf_indexes)
            .unwrap_or_default()
            .into_iter()
            .map(|value| {
                usize::try_from(value)
                    .map_err(|_| invalid_arg("leafIndexes entries must fit within usize"))
            })
            .collect::<napi::Result<Vec<_>>>()?;
        Ok(Self {
            sample_count: usize::try_from(sample_count)
                .map_err(|_| invalid_arg("sampleCount must fit within usize"))?,
            sample_seed,
            explicit_indexes,
        })
    }
}

#[derive(Clone, Copy)]
enum ProofOrigin {
    Sampled,
    Explicit,
}

impl ProofOrigin {
    fn label(self) -> &'static str {
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

fn build_car_plan_from_manifest(manifest: &DaManifestV1) -> napi::Result<CarBuildPlan> {
    sorafs_car::build_plan_from_da_manifest(manifest)
        .map_err(|err| invalid_arg(format!("failed to build CAR plan: {err}")))
}

fn validate_manifest_consistency(manifest: &DaManifestV1, store: &ChunkStore) -> napi::Result<()> {
    let blob_hash_bytes = manifest.blob_hash.as_ref();
    if store.payload_digest().as_bytes() != blob_hash_bytes {
        return Err(napi::Error::from_reason(format!(
            "payload hash mismatch: manifest={} computed={}",
            hex::encode(blob_hash_bytes),
            hex::encode(store.payload_digest().as_bytes())
        )));
    }
    let chunk_root_bytes = manifest.chunk_root.as_ref();
    if store.por_tree().root() != chunk_root_bytes {
        return Err(napi::Error::from_reason(format!(
            "chunk root mismatch: manifest={} computed={}",
            hex::encode(chunk_root_bytes),
            hex::encode(store.por_tree().root())
        )));
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn chunk_store_err(err: ChunkStoreError) -> napi::Error {
    napi::Error::from_reason(format!("chunk store error: {err}"))
}

fn collect_sampled_proofs(
    store: &ChunkStore,
    payload: &[u8],
    options: &DaProofOptionsNormalized,
    por_root: &[u8; 32],
) -> napi::Result<Vec<ProofReport>> {
    if options.sample_count == 0 {
        return Ok(Vec::new());
    }
    let mut source = InMemoryPayload::new(payload);
    let samples = store
        .sample_leaves_with(options.sample_count, options.sample_seed, &mut source)
        .map_err(chunk_store_err)?;
    Ok(samples
        .into_iter()
        .map(|(idx, proof)| ProofReport {
            origin: ProofOrigin::Sampled,
            leaf_index: idx,
            verified: proof.verify(por_root),
            proof,
        })
        .collect())
}

fn collect_explicit_proofs(
    store: &ChunkStore,
    payload: &[u8],
    options: &DaProofOptionsNormalized,
    por_root: &[u8; 32],
) -> napi::Result<Vec<ProofReport>> {
    if options.explicit_indexes.is_empty() {
        return Ok(Vec::new());
    }
    let mut source = InMemoryPayload::new(payload);
    let mut proofs = Vec::with_capacity(options.explicit_indexes.len());
    let mut seen = HashSet::new();
    for &leaf_index in &options.explicit_indexes {
        if !seen.insert(leaf_index) {
            continue;
        }
        let (chunk_idx, segment_idx, inner_idx) = store
            .por_tree()
            .leaf_path(leaf_index)
            .ok_or_else(|| invalid_arg(format!("leaf_index {leaf_index} out of range")))?;
        let proof = store
            .por_tree()
            .prove_leaf_with(chunk_idx, segment_idx, inner_idx, &mut source)
            .map_err(chunk_store_err)?
            .ok_or_else(|| invalid_arg(format!("missing PoR proof for leaf_index {leaf_index}")))?;
        proofs.push(ProofReport {
            origin: ProofOrigin::Explicit,
            leaf_index,
            verified: proof.verify(por_root),
            proof,
        });
    }
    Ok(proofs)
}

fn hex_list(values: &[[u8; 32]]) -> Vec<String> {
    values.iter().map(hex::encode).collect()
}

fn proof_to_js_record(report: &ProofReport) -> JsDaProofRecord {
    JsDaProofRecord {
        origin: report.origin.label().to_string(),
        leaf_index: u32::try_from(report.leaf_index).unwrap_or(u32::MAX),
        chunk_index: u32::try_from(report.proof.chunk_index).unwrap_or(u32::MAX),
        segment_index: u32::try_from(report.proof.segment_index).unwrap_or(u32::MAX),
        leaf_offset: JsU64(report.proof.leaf_offset),
        leaf_length: report.proof.leaf_length,
        segment_offset: JsU64(report.proof.segment_offset),
        segment_length: report.proof.segment_length,
        chunk_offset: JsU64(report.proof.chunk_offset),
        chunk_length: report.proof.chunk_length,
        payload_len: JsU64(report.proof.payload_len),
        chunk_digest_hex: hex::encode(report.proof.chunk_digest),
        chunk_root_hex: hex::encode(report.proof.chunk_root),
        segment_digest_hex: hex::encode(report.proof.segment_digest),
        leaf_digest_hex: hex::encode(report.proof.leaf_digest),
        leaf_bytes_b64: STANDARD.encode(&report.proof.leaf_bytes),
        segment_leaves_hex: hex_list(&report.proof.segment_leaves),
        chunk_segments_hex: hex_list(&report.proof.chunk_segments),
        chunk_roots_hex: hex_list(&report.proof.chunk_roots),
        verified: report.verified,
    }
}

#[napi(object)]
/// Norito bridge configuration for the local QUIC proxy.
pub struct JsProxyNoritoBridgeConfig {
    /// Directory where Norito payloads are spooled.
    pub spool_dir: String,
    /// Optional file extension applied to spool artefacts.
    pub extension: Option<String>,
}

#[napi(object)]
/// CAR bridge configuration for the local QUIC proxy.
pub struct JsProxyCarBridgeConfig {
    /// Directory where CAR archives are cached.
    pub cache_dir: String,
    /// Optional file extension applied to cached archives.
    pub extension: Option<String>,
    /// Whether `.zst` archives are permitted.
    pub allow_zst: Option<bool>,
}

#[napi(object)]
/// Kaigi bridge configuration for the local QUIC proxy.
pub struct JsProxyKaigiBridgeConfig {
    /// Directory where Kaigi spool entries are stored.
    pub spool_dir: String,
    /// Optional file extension applied to Kaigi spool files.
    pub extension: Option<String>,
    /// Optional room policy label (`public` or `authenticated`).
    pub room_policy: Option<String>,
}

#[napi(object)]
/// Optional local QUIC proxy configuration surfaced to gateway fetches.
pub struct JsLocalProxyConfig {
    /// Bind address (e.g. `127.0.0.1:0`) for the proxy.
    pub bind_addr: Option<String>,
    /// Telemetry label attached to proxy metrics.
    pub telemetry_label: Option<String>,
    /// Optional guard cache key rendered as hexadecimal.
    pub guard_cache_key_hex: Option<String>,
    /// Whether to emit browser manifests in the handshake.
    pub emit_browser_manifest: Option<bool>,
    /// Proxy mode label (`bridge` or `metadata-only`).
    pub proxy_mode: Option<String>,
    /// Whether circuits should be pre-warmed.
    pub prewarm_circuits: Option<bool>,
    /// Maximum concurrent streams per circuit.
    pub max_streams_per_circuit: Option<u32>,
    /// Suggested circuit TTL in seconds.
    pub circuit_ttl_hint_secs: Option<u32>,
    /// Optional Norito bridge configuration.
    pub norito_bridge: Option<JsProxyNoritoBridgeConfig>,
    /// Optional CAR bridge configuration.
    pub car_bridge: Option<JsProxyCarBridgeConfig>,
    /// Optional Kaigi bridge configuration.
    pub kaigi_bridge: Option<JsProxyKaigiBridgeConfig>,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// `QoS` envelope for Taikai cache classes.
pub struct JsTaikaiQosConfig {
    /// Priority lane throughput (bytes/sec).
    pub priority_rate_bps: JsU64,
    /// Standard lane throughput (bytes/sec).
    pub standard_rate_bps: JsU64,
    /// Bulk lane throughput (bytes/sec).
    pub bulk_rate_bps: JsU64,
    /// Token burst multiplier.
    pub burst_multiplier: u32,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Taikai cache tier/retention configuration.
pub struct JsTaikaiCacheConfig {
    /// Hot-tier storage capacity in bytes.
    pub hot_capacity_bytes: JsU64,
    /// Hot-tier retention window in seconds.
    pub hot_retention_secs: JsU64,
    /// Warm-tier storage capacity in bytes.
    pub warm_capacity_bytes: JsU64,
    /// Warm-tier retention window in seconds.
    pub warm_retention_secs: JsU64,
    /// Cold-tier storage capacity in bytes.
    pub cold_capacity_bytes: JsU64,
    /// Cold-tier retention window in seconds.
    pub cold_retention_secs: JsU64,
    /// `QoS` token-bucket parameters per class.
    pub qos: JsTaikaiQosConfig,
    /// Optional reliability tuning for shard circuit breakers.
    pub reliability: Option<JsTaikaiReliabilityConfig>,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Reliability configuration for the Taikai pull queue.
pub struct JsTaikaiReliabilityConfig {
    /// Consecutive failures required to trip a circuit breaker.
    pub failures_to_trip: Option<u32>,
    /// Duration (seconds) a circuit stays open before retry.
    pub open_secs: Option<JsU64>,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Per-tier hit/insert counters for the Taikai cache.
pub struct JsTaikaiCacheTierCounts {
    /// Count recorded for the hot tier.
    pub hot: JsU64,
    /// Count recorded for the warm tier.
    pub warm: JsU64,
    /// Count recorded for the cold tier.
    pub cold: JsU64,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Eviction counters partitioned by reason.
pub struct JsTaikaiCacheEvictionCounts {
    /// Number of entries evicted due to expiry.
    pub expired: JsU64,
    /// Number of entries evicted due to capacity pressure.
    pub capacity: JsU64,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Eviction counters per tier.
pub struct JsTaikaiCacheEvictions {
    /// Evictions from the hot tier.
    pub hot: JsTaikaiCacheEvictionCounts,
    /// Evictions from the warm tier.
    pub warm: JsTaikaiCacheEvictionCounts,
    /// Evictions from the cold tier.
    pub cold: JsTaikaiCacheEvictionCounts,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Promotion counters captured by the Taikai cache.
pub struct JsTaikaiCachePromotions {
    /// Promotions from warm to hot.
    pub warm_to_hot: JsU64,
    /// Promotions from cold to warm.
    pub cold_to_warm: JsU64,
    /// Promotions from cold directly to hot.
    pub cold_to_hot: JsU64,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// `QoS` counters for Taikai cache and queue telemetry.
pub struct JsTaikaiQosCounts {
    /// Count recorded for the priority class.
    pub priority: JsU64,
    /// Count recorded for the standard class.
    pub standard: JsU64,
    /// Count recorded for the bulk class.
    pub bulk: JsU64,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Snapshot of Taikai cache activity recorded after a fetch.
pub struct JsTaikaiCacheStats {
    /// Cache hits per tier.
    pub hits: JsTaikaiCacheTierCounts,
    /// Total cache misses.
    pub misses: JsU64,
    /// Cache inserts per tier.
    pub inserts: JsTaikaiCacheTierCounts,
    /// Evictions observed during the fetch.
    pub evictions: JsTaikaiCacheEvictions,
    /// Promotions observed during the fetch.
    pub promotions: JsTaikaiCachePromotions,
    /// `QoS` denials recorded during the fetch.
    pub qos_denials: JsTaikaiQosCounts,
}

#[napi(object)]
#[derive(Clone, Copy, Debug)]
/// Snapshot of the Taikai pull queue state.
pub struct JsTaikaiQueueStats {
    /// Queued segment count.
    pub pending_segments: JsU64,
    /// Queued bytes across all pending segments.
    pub pending_bytes: JsU64,
    /// Pending batches awaiting issuance.
    pub pending_batches: JsU64,
    /// Batches currently in flight.
    pub in_flight_batches: JsU64,
    /// Number of hedged batches.
    pub hedged_batches: JsU64,
    /// `QoS` denials emitted by the shaper.
    pub shaper_denials: JsTaikaiQosCounts,
    /// Segments dropped due to backpressure.
    pub dropped_segments: JsU64,
    /// Failover events recorded by the queue.
    pub failovers: JsU64,
    /// Open circuit count across shards.
    pub open_circuits: JsU64,
}

#[napi(object)]
#[derive(Default)]
/// Options controlling `sorafsGatewayFetch`.
pub struct JsGatewayFetchOptions {
    /// Base64-encoded manifest envelope forwarded to providers.
    pub manifest_envelope_b64: Option<String>,
    /// Expected manifest CID expressed as hexadecimal.
    pub manifest_cid_hex: Option<String>,
    /// Expected cache/denylist version advertised by gateways.
    pub cache_version: Option<String>,
    /// Base64-encoded moderation proof key for validating denylist tokens.
    pub moderation_token_key: Option<String>,
    /// Optional client identifier forwarded via headers.
    pub client_id: Option<String>,
    /// Telemetry region label attached to orchestrator metrics.
    pub telemetry_region: Option<String>,
    /// Rollout phase label controlling the default anonymity policy.
    pub rollout_phase: Option<String>,
    /// Maximum number of providers considered for the session.
    pub max_peers: Option<u32>,
    /// Retry budget applied per chunk (minimum 1).
    pub retry_budget: Option<u32>,
    /// Transport policy label (`soranet-first`, `soranet-strict`, `direct-only`).
    pub transport_policy: Option<String>,
    /// Anonymity policy label (`anon-guard-pq`, `anon-majority-pq`, `anon-strict-pq`).
    pub anonymity_policy: Option<String>,
    /// Write-mode hint controlling PQ enforcement (`read-only`, `upload-pq-only`).
    pub write_mode: Option<String>,
    /// Optional local proxy configuration for browser integrations.
    pub local_proxy: Option<JsLocalProxyConfig>,
    /// Optional Taikai cache configuration (SNNet-14 pilots).
    pub taikai_cache: Option<JsTaikaiCacheConfig>,
    /// File path used to persist the computed scoreboard (mirrors `--scoreboard-out`).
    pub scoreboard_out_path: Option<String>,
    /// Override for the Unix timestamp used when evaluating adverts (`--scoreboard-now`).
    pub scoreboard_now_unix_secs: Option<JsU64>,
    /// Optional label recorded as the scoreboard `telemetry_source`.
    pub scoreboard_telemetry_label: Option<String>,
    /// Whether implicit provider metadata was allowed during scoring (metadata hint).
    pub scoreboard_allow_implicit_metadata: Option<bool>,
    /// Whether the caller allowed a temporary downgrade to a single provider.
    pub allow_single_source_fallback: Option<bool>,
}

#[napi(object)]
/// Gateway provider descriptor supplied to orchestrator fetches.
pub struct JsGatewayProviderSpec {
    /// Human-readable provider name.
    pub name: String,
    /// Provider identifier rendered as 32-byte hexadecimal.
    pub provider_id_hex: String,
    /// Base URL for the Torii gateway.
    pub base_url: String,
    /// Stream token presented when fetching chunks.
    pub stream_token_b64: String,
    /// Optional privacy events endpoint.
    pub privacy_events_url: Option<String>,
}

#[napi(object)]
/// CAR archive statistics returned after verification.
pub struct JsCarArchiveStats {
    /// Total CAR archive size in bytes.
    pub size: JsU64,
    /// Hex-encoded digest of the CAR payload section.
    pub payload_digest_hex: String,
    /// Hex-encoded digest of the full CAR archive.
    pub archive_digest_hex: String,
    /// CID rendered as hexadecimal.
    pub cid_hex: String,
    /// Root CIDs for the archive rendered as hexadecimal.
    pub root_cids_hex: Vec<String>,
    /// Whether verification succeeded.
    pub verified: bool,
    /// Number of `PoR` leaves observed during verification.
    pub por_leaf_count: JsU64,
}

#[napi(object)]
/// Council signature exported from a manifest governance proof bundle.
pub struct JsCouncilSignature {
    /// Signer identifier rendered as hexadecimal.
    pub signer_hex: String,
    /// Raw signature rendered as hexadecimal.
    pub signature_hex: String,
}

#[napi(object)]
/// Governance proofs bundled with the manifest.
pub struct JsManifestGovernance {
    /// Council signatures authorising the manifest.
    pub council_signatures: Vec<JsCouncilSignature>,
}

#[napi(object)]
/// CAR verification artefacts emitted after gateway fetches.
pub struct JsCarVerification {
    /// Hex digest of the manifest.
    pub manifest_digest_hex: String,
    /// Hex digest of the manifest payload.
    pub manifest_payload_digest_hex: String,
    /// Hex digest of the CAR archive recorded in the manifest.
    pub manifest_car_digest_hex: String,
    /// Manifest-declared content length.
    pub manifest_content_length: JsU64,
    /// Manifest-declared chunk count.
    pub manifest_chunk_count: JsU64,
    /// Chunk profile handle advertised by the manifest.
    pub manifest_chunk_profile_handle: String,
    /// Governance proofs bundled with the manifest.
    pub manifest_governance: JsManifestGovernance,
    /// CAR archive statistics.
    pub car_archive: JsCarArchiveStats,
}

#[napi(object)]
/// Result payload produced by `sorafsGatewayFetch`.
pub struct JsGatewayFetchResult {
    /// Manifest identifier rendered as hexadecimal.
    pub manifest_id_hex: String,
    /// Chunker handle used for the session.
    pub chunker_handle: String,
    /// Number of chunks assembled.
    pub chunk_count: u32,
    /// Total assembled bytes.
    pub assembled_bytes: JsU64,
    /// Concatenated payload (`CARv2` body).
    pub payload: Buffer,
    /// Optional telemetry region label.
    pub telemetry_region: Option<String>,
    /// Requested anonymity policy label.
    pub anonymity_policy: String,
    /// Resulting policy status label.
    pub anonymity_status: String,
    /// Reason for policy fallback, if any.
    pub anonymity_reason: String,
    /// Number of `SoraNet` providers selected.
    pub anonymity_soranet_selected: u32,
    /// Number of PQ-capable providers selected.
    pub anonymity_pq_selected: u32,
    /// Number of classical providers selected.
    pub anonymity_classical_selected: u32,
    /// Ratio of classical providers in the selection.
    pub anonymity_classical_ratio: f64,
    /// Ratio of PQ-capable providers in the selection.
    pub anonymity_pq_ratio: f64,
    /// Ratio of PQ-capable candidates in the scoreboard.
    pub anonymity_candidate_ratio: f64,
    /// PQ deficit ratio relative to the requested policy.
    pub anonymity_deficit_ratio: f64,
    /// PQ supply delta between candidates and selection.
    pub anonymity_supply_delta: f64,
    /// Whether a brownout occurred.
    pub anonymity_brownout: bool,
    /// Whether the brownout should trigger operator alerts.
    pub anonymity_brownout_effective: bool,
    /// Whether classical providers participated.
    pub anonymity_uses_classical: bool,
    /// Provider-level outcome reports.
    pub provider_reports: Vec<JsMultiFetchProviderReport>,
    /// Per-chunk receipts summarising fetch attempts.
    pub chunk_receipts: Vec<JsMultiFetchChunkReceipt>,
    /// Browser manifest rendered as JSON when a local proxy is active.
    pub local_proxy_manifest_json: Option<String>,
    /// Manifest/CAR verification metadata.
    pub car_verification: Option<JsCarVerification>,
    /// Scoreboard metadata captured during the fetch session.
    pub metadata: JsGatewayMetadata,
    /// Snapshot of Taikai cache activity captured after the fetch.
    pub taikai_cache_summary: Option<JsTaikaiCacheStats>,
    /// Snapshot of the Taikai pull queue captured after the fetch.
    pub taikai_cache_queue: Option<JsTaikaiQueueStats>,
}

#[napi(object)]
#[allow(clippy::struct_excessive_bools)]
/// Scoreboard metadata emitted by the gateway orchestrator.
pub struct JsGatewayMetadata {
    /// Number of direct `SoraNet` providers participating in the session.
    pub provider_count: JsU64,
    /// Number of Torii gateway providers participating in the session.
    pub gateway_provider_count: JsU64,
    /// Provider-mix label derived from the direct/gateway counts.
    pub provider_mix: String,
    /// Requested transport policy label.
    pub transport_policy: String,
    /// Whether a transport-policy override was applied.
    pub transport_policy_override: bool,
    /// Optional label describing the override that was applied.
    pub transport_policy_override_label: Option<String>,
    /// Requested anonymity policy label.
    pub anonymity_policy: String,
    /// Whether an anonymity-policy override was applied.
    pub anonymity_policy_override: bool,
    /// Optional label describing the anonymity override that was applied.
    pub anonymity_policy_override_label: Option<String>,
    /// Write-mode hint applied during the session.
    pub write_mode: String,
    /// Whether the write-mode enforces PQ-only transport.
    pub write_mode_enforces_pq: bool,
    /// Maximum number of parallel chunks fetched per batch.
    pub max_parallel: Option<JsU64>,
    /// Maximum number of providers considered for the session.
    pub max_peers: Option<JsU64>,
    /// Retry budget enforced per chunk.
    pub retry_budget: Option<JsU64>,
    /// Provider failure threshold enforced by the orchestrator.
    pub provider_failure_threshold: JsU64,
    /// Unix timestamp used when evaluating provider adverts.
    pub assume_now_unix: JsU64,
    /// Telemetry label recorded for the capture.
    pub telemetry_source_label: Option<String>,
    /// Telemetry region label recorded for the capture.
    pub telemetry_region: Option<String>,
    /// Whether a signed gateway manifest envelope was supplied.
    pub gateway_manifest_provided: bool,
    /// Optional manifest identifier recorded for the capture.
    pub gateway_manifest_id: Option<String>,
    /// Optional manifest CID recorded for the capture.
    pub gateway_manifest_cid: Option<String>,
    /// Whether downgrades to a single provider were permitted.
    pub allow_single_source_fallback: bool,
    /// Whether implicit provider metadata was allowed when scoring adverts.
    pub allow_implicit_metadata: bool,
}

impl From<TaikaiCacheStatsSnapshot> for JsTaikaiCacheStats {
    fn from(stats: TaikaiCacheStatsSnapshot) -> Self {
        let tier_counts = |counts: TierStats| JsTaikaiCacheTierCounts {
            hot: JsU64(counts.hot),
            warm: JsU64(counts.warm),
            cold: JsU64(counts.cold),
        };
        let evictions = |stats: EvictionStats| JsTaikaiCacheEvictions {
            hot: JsTaikaiCacheEvictionCounts {
                expired: JsU64(stats.hot.expired),
                capacity: JsU64(stats.hot.capacity),
            },
            warm: JsTaikaiCacheEvictionCounts {
                expired: JsU64(stats.warm.expired),
                capacity: JsU64(stats.warm.capacity),
            },
            cold: JsTaikaiCacheEvictionCounts {
                expired: JsU64(stats.cold.expired),
                capacity: JsU64(stats.cold.capacity),
            },
        };
        let qos = |counts: QosStats| JsTaikaiQosCounts {
            priority: JsU64(counts.priority),
            standard: JsU64(counts.standard),
            bulk: JsU64(counts.bulk),
        };

        Self {
            hits: tier_counts(stats.hits),
            misses: JsU64(stats.misses),
            inserts: tier_counts(stats.inserts),
            evictions: evictions(stats.evictions),
            promotions: JsTaikaiCachePromotions {
                warm_to_hot: JsU64(stats.promotions.warm_to_hot),
                cold_to_warm: JsU64(stats.promotions.cold_to_warm),
                cold_to_hot: JsU64(stats.promotions.cold_to_hot),
            },
            qos_denials: qos(stats.qos_denials),
        }
    }
}

impl From<TaikaiPullQueueStats> for JsTaikaiQueueStats {
    fn from(stats: TaikaiPullQueueStats) -> Self {
        let qos = JsTaikaiQosCounts {
            priority: JsU64(stats.shaper_denials.priority),
            standard: JsU64(stats.shaper_denials.standard),
            bulk: JsU64(stats.shaper_denials.bulk),
        };
        Self {
            pending_segments: JsU64(stats.pending_segments),
            pending_bytes: JsU64(stats.pending_bytes),
            pending_batches: JsU64(stats.pending_batches),
            in_flight_batches: JsU64(stats.in_flight_batches),
            hedged_batches: JsU64(stats.hedged_batches),
            shaper_denials: qos,
            dropped_segments: JsU64(stats.dropped_segments),
            failovers: JsU64(stats.failovers),
            open_circuits: JsU64(stats.open_circuits),
        }
    }
}

fn js_range_capability_to_input(range: JsRangeCapability) -> RangeCapabilityInput {
    RangeCapabilityInput {
        max_chunk_span: range.max_chunk_span,
        min_granularity: range.min_granularity,
        supports_sparse_offsets: range.supports_sparse_offsets,
        requires_alignment: range.requires_alignment,
        supports_merkle_proof: range.supports_merkle_proof,
    }
}

fn js_stream_budget_to_input(budget: JsStreamBudget) -> StreamBudgetInput {
    StreamBudgetInput {
        max_in_flight: budget.max_in_flight,
        max_bytes_per_sec: budget.max_bytes_per_sec.into(),
        burst_bytes: budget.burst_bytes.map(Into::into),
    }
}

fn js_transport_hints_to_input(hints: &[JsTransportHint]) -> Vec<TransportHintInput> {
    hints
        .iter()
        .map(|hint| TransportHintInput {
            protocol: hint.protocol.clone(),
            protocol_id: hint.protocol_id,
            priority: hint.priority,
        })
        .collect()
}

fn js_metadata_to_input(metadata: JsProviderMetadata, alias: &str) -> ProviderMetadataInput {
    let JsProviderMetadata {
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
    } = metadata;

    ProviderMetadataInput {
        provider_id: Some(provider_id.unwrap_or_else(|| alias.to_string())),
        profile_id,
        profile_aliases,
        availability,
        stake_amount,
        max_streams,
        refresh_deadline: refresh_deadline.map(Into::into),
        expires_at: expires_at.map(Into::into),
        ttl_secs: ttl_secs.map(Into::into),
        allow_unknown_capabilities,
        capability_names,
        rendezvous_topics,
        notes,
        range_capability: range_capability.map(js_range_capability_to_input),
        stream_budget: stream_budget.map(js_stream_budget_to_input),
        transport_hints: transport_hints.map(|hints| js_transport_hints_to_input(&hints)),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn js_provider_to_local(spec: JsLocalProviderSpec) -> napi::Result<LocalProviderInput> {
    let metadata = match spec.metadata {
        Some(meta) => Some(js_metadata_to_input(meta, &spec.name)),
        None => None,
    };
    Ok(LocalProviderInput {
        name: spec.name,
        path: PathBuf::from(spec.path),
        max_concurrent: spec.max_concurrent,
        weight: spec.weight,
        metadata,
    })
}

fn js_telemetry_to_inputs(entries: &[JsTelemetryEntry]) -> Vec<TelemetryEntryInput> {
    entries
        .iter()
        .map(|entry| TelemetryEntryInput {
            provider_id: entry.provider_id.clone(),
            qos_score: entry.qos_score,
            latency_p95_ms: entry.latency_p95_ms,
            failure_rate_ewma: entry.failure_rate_ewma,
            token_health: entry.token_health,
            staking_weight: entry.staking_weight,
            penalty: entry.penalty,
            last_updated_unix: entry.last_updated_unix.map(Into::into),
        })
        .collect()
}

#[allow(clippy::unnecessary_wraps)]
fn build_local_fetch_options(
    options: Option<JsMultiFetchOptions>,
) -> napi::Result<LocalFetchOptions> {
    let mut local = LocalFetchOptions::default();
    if let Some(opts) = options {
        local.verify_digests = opts.verify_digests;
        local.verify_lengths = opts.verify_lengths;
        local.retry_budget = opts.retry_budget;
        local.provider_failure_threshold = opts.provider_failure_threshold;
        local.max_parallel = opts.max_parallel;
        local.max_peers = opts.max_peers;
        local.chunker_handle = opts.chunker_handle;
        if let Some(entries) = opts.telemetry {
            local.telemetry = js_telemetry_to_inputs(&entries);
        }
        local.use_scoreboard = opts.use_scoreboard;
        local.scoreboard_now_unix_secs = opts.scoreboard_now_unix_secs.map(Into::into);
        if let Some(mut deny) = opts.deny_providers {
            local.deny_providers.append(&mut deny);
        }
        if let Some(boosts) = opts.boost_providers {
            local.boost_providers = boosts
                .into_iter()
                .map(|boost| (boost.provider, i64::from(boost.delta)))
                .collect();
        }
        local.return_scoreboard = opts.return_scoreboard;
    }
    Ok(local)
}

fn local_fetch_result_to_js(
    result: local_fetch::LocalFetchResult,
) -> napi::Result<JsMultiFetchResult> {
    let chunk_count = u32::try_from(result.chunk_count).map_err(|_| {
        napi::Error::new(
            napi::Status::GenericFailure,
            "chunk count exceeds JavaScript number range",
        )
    })?;

    let payload = Buffer::from(result.outcome.assemble_payload());

    let mut provider_reports = Vec::with_capacity(result.outcome.provider_reports.len());
    for report in &result.outcome.provider_reports {
        provider_reports.push(JsMultiFetchProviderReport {
            provider: report.provider.id().as_str().to_string(),
            successes: u32::try_from(report.successes).map_err(|_| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "provider success count exceeds JavaScript number range",
                )
            })?,
            failures: u32::try_from(report.failures).map_err(|_| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "provider failure count exceeds JavaScript number range",
                )
            })?,
            disabled: report.disabled,
        });
    }

    let mut chunk_receipts = Vec::with_capacity(result.outcome.chunk_receipts.len());
    for receipt in &result.outcome.chunk_receipts {
        chunk_receipts.push(JsMultiFetchChunkReceipt {
            chunk_index: u32::try_from(receipt.chunk_index).map_err(|_| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "chunk index exceeds JavaScript number range",
                )
            })?,
            provider: receipt.provider.as_str().to_string(),
            attempts: u32::try_from(receipt.attempts).map_err(|_| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "chunk attempt count exceeds JavaScript number range",
                )
            })?,
            latency_ms: receipt.latency_ms,
            bytes: receipt.bytes,
        });
    }

    let scoreboard = result.scoreboard.map(|entries| {
        entries
            .into_iter()
            .map(|entry| JsScoreboardEntry {
                provider_id: entry.provider_id,
                alias: entry.alias,
                raw_score: entry.raw_score,
                normalized_weight: entry.normalized_weight,
                eligibility: entry.eligibility,
            })
            .collect()
    });

    Ok(JsMultiFetchResult {
        chunk_count,
        payload,
        provider_reports,
        chunk_receipts,
        scoreboard,
    })
}

fn proxy_mode_from_label(label: &str) -> napi::Result<ProxyMode> {
    ProxyMode::parse(label).ok_or_else(|| {
        invalid_arg(format!(
            "proxy_mode must be one of 'bridge' or 'metadata-only', got '{label}'"
        ))
    })
}

fn build_local_proxy_config(cfg: &JsLocalProxyConfig) -> napi::Result<LocalQuicProxyConfig> {
    let mut proxy = LocalQuicProxyConfig::default();
    if let Some(bind) = cfg.bind_addr.as_ref() {
        let trimmed = bind.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "localProxy.bindAddr must not be empty when provided",
            ));
        }
        proxy.bind_addr = trimmed.to_string();
    }
    proxy.telemetry_label = cfg
        .telemetry_label
        .as_ref()
        .map(|label| label.trim().to_string());
    proxy.guard_cache_key_hex = cfg
        .guard_cache_key_hex
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(flag) = cfg.emit_browser_manifest {
        proxy.emit_browser_manifest = flag;
    }
    if let Some(mode) = cfg.proxy_mode.as_ref() {
        proxy.proxy_mode = proxy_mode_from_label(mode)?;
    }
    if let Some(prewarm) = cfg.prewarm_circuits {
        proxy.prewarm_circuits = prewarm;
    }
    proxy.max_streams_per_circuit = cfg.max_streams_per_circuit;
    proxy.circuit_ttl_hint_secs = cfg.circuit_ttl_hint_secs;
    if let Some(norito_cfg) = cfg.norito_bridge.as_ref() {
        let trimmed = norito_cfg.spool_dir.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "localProxy.noritoBridge.spoolDir must not be empty when provided",
            ));
        }
        proxy.norito_bridge = Some(ProxyNoritoBridgeConfig {
            spool_dir: trimmed.to_string(),
            extension: norito_cfg
                .extension
                .as_ref()
                .map(|ext| ext.trim().to_string())
                .filter(|ext| !ext.is_empty()),
        });
    }
    if let Some(car_cfg) = cfg.car_bridge.as_ref() {
        let trimmed = car_cfg.cache_dir.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "localProxy.carBridge.cacheDir must not be empty when provided",
            ));
        }
        proxy.car_bridge = Some(ProxyCarBridgeConfig {
            cache_dir: trimmed.to_string(),
            extension: car_cfg
                .extension
                .as_ref()
                .map(|ext| ext.trim().to_string())
                .filter(|ext| !ext.is_empty()),
            allow_zst: car_cfg.allow_zst.unwrap_or(false),
        });
    }
    if let Some(kaigi_cfg) = cfg.kaigi_bridge.as_ref() {
        let trimmed = kaigi_cfg.spool_dir.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "localProxy.kaigiBridge.spoolDir must not be empty when provided",
            ));
        }
        let mut bridge = ProxyKaigiBridgeConfig {
            spool_dir: trimmed.to_string(),
            extension: kaigi_cfg
                .extension
                .as_ref()
                .map(|ext| ext.trim().to_string())
                .filter(|ext| !ext.is_empty()),
            room_policy: None,
        };
        if let Some(policy) = kaigi_cfg.room_policy.as_ref() {
            let normalized = policy.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "public" | "authenticated" => {
                    bridge.room_policy = Some(normalized);
                }
                _ => {
                    return Err(invalid_arg(
                        "localProxy.kaigiBridge.roomPolicy must be `public` or `authenticated`",
                    ));
                }
            }
        }
        proxy.kaigi_bridge = Some(bridge);
    }
    Ok(proxy)
}

fn build_taikai_cache_config(cfg: &JsTaikaiCacheConfig) -> napi::Result<TaikaiCacheConfig> {
    fn ensure_positive(value: u64, label: &str) -> napi::Result<u64> {
        if value == 0 {
            Err(invalid_arg(format!("{label} must be greater than zero")))
        } else {
            Ok(value)
        }
    }
    fn duration_from_secs(value: u64, label: &str) -> napi::Result<Duration> {
        ensure_positive(value, label).map(Duration::from_secs)
    }

    let qos_cfg = &cfg.qos;
    if qos_cfg.burst_multiplier == 0 {
        return Err(invalid_arg(
            "taikaiCache.qos.burstMultiplier must be greater than zero",
        ));
    }

    let reliability_cfg = cfg.reliability.unwrap_or(JsTaikaiReliabilityConfig {
        failures_to_trip: None,
        open_secs: None,
    });
    let failures_to_trip = reliability_cfg.failures_to_trip.unwrap_or(3).max(1);
    let open_secs = reliability_cfg.open_secs.map_or(2, Into::into);

    Ok(TaikaiCacheConfig {
        hot_capacity_bytes: ensure_positive(
            cfg.hot_capacity_bytes.into(),
            "taikaiCache.hotCapacityBytes",
        )?,
        hot_retention: duration_from_secs(
            cfg.hot_retention_secs.into(),
            "taikaiCache.hotRetentionSecs",
        )?,
        warm_capacity_bytes: ensure_positive(
            cfg.warm_capacity_bytes.into(),
            "taikaiCache.warmCapacityBytes",
        )?,
        warm_retention: duration_from_secs(
            cfg.warm_retention_secs.into(),
            "taikaiCache.warmRetentionSecs",
        )?,
        cold_capacity_bytes: ensure_positive(
            cfg.cold_capacity_bytes.into(),
            "taikaiCache.coldCapacityBytes",
        )?,
        cold_retention: duration_from_secs(
            cfg.cold_retention_secs.into(),
            "taikaiCache.coldRetentionSecs",
        )?,
        qos: QosConfig {
            priority_rate_bps: ensure_positive(
                qos_cfg.priority_rate_bps.into(),
                "taikaiCache.qos.priorityRateBps",
            )?,
            standard_rate_bps: ensure_positive(
                qos_cfg.standard_rate_bps.into(),
                "taikaiCache.qos.standardRateBps",
            )?,
            bulk_rate_bps: ensure_positive(
                qos_cfg.bulk_rate_bps.into(),
                "taikaiCache.qos.bulkRateBps",
            )?,
            burst_multiplier: qos_cfg.burst_multiplier,
        },
        reliability: ReliabilityTuning {
            failures_to_trip,
            open_secs,
        },
    })
}

fn build_gateway_provider_input(
    spec: &JsGatewayProviderSpec,
) -> napi::Result<GatewayProviderInput> {
    let name = spec.name.trim().to_string();
    if name.is_empty() {
        return Err(invalid_arg("provider name must not be empty"));
    }
    let provider_id = spec.provider_id_hex.trim().to_ascii_lowercase();
    if provider_id.len() != 64 || !provider_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid_arg(format!(
            "provider '{name}' has invalid providerIdHex; expected 32-byte hex"
        )));
    }
    let base_url = spec.base_url.trim();
    if base_url.is_empty() {
        return Err(invalid_arg(format!(
            "provider '{name}' baseUrl must not be empty"
        )));
    }
    let stream_token = spec.stream_token_b64.trim();
    if stream_token.is_empty() {
        return Err(invalid_arg(format!(
            "provider '{name}' streamToken must not be empty"
        )));
    }
    let privacy_url = spec
        .privacy_events_url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(GatewayProviderInput {
        name,
        provider_id_hex: provider_id,
        base_url: base_url.to_string(),
        stream_token_b64: stream_token.to_string(),
        privacy_events_url: privacy_url,
    })
}

fn manifest_envelope_from_options(options: &JsGatewayFetchOptions) -> Option<String> {
    options
        .manifest_envelope_b64
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn client_id_from_options(options: &JsGatewayFetchOptions) -> Option<String> {
    options
        .client_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_gateway_fetch_config(
    manifest_id_hex: &str,
    chunker_handle: &str,
    options: &JsGatewayFetchOptions,
) -> napi::Result<GatewayFetchConfig> {
    let manifest_envelope_b64 = manifest_envelope_from_options(options);
    let client_id = client_id_from_options(options);
    let expected_cid_hex = options
        .manifest_cid_hex
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let expected_cache_version = options
        .cache_version
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let moderation_token_key_b64 = options
        .moderation_token_key
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(ref cid) = expected_cid_hex
        && (cid.len() != 64 || !cid.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(invalid_arg(
            "manifestCidHex must be a 32-byte hex string when provided",
        ));
    }
    Ok(GatewayFetchConfig {
        manifest_id_hex: manifest_id_hex.to_string(),
        chunker_handle: chunker_handle.to_string(),
        manifest_envelope_b64,
        client_id,
        expected_manifest_cid_hex: expected_cid_hex,
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version,
        moderation_token_key_b64,
    })
}

fn build_gateway_plan(
    specs: &[ChunkFetchSpec],
    chunker_handle: &str,
) -> napi::Result<CarBuildPlan> {
    let descriptor = sorafs_car::chunker_registry::lookup_by_handle(chunker_handle)
        .ok_or_else(|| invalid_arg(format!("unknown chunker handle '{chunker_handle}'")))?;
    let content_length = specs
        .iter()
        .map(|spec| spec.offset + u64::from(spec.length))
        .max()
        .unwrap_or(0);
    let chunks = specs
        .iter()
        .map(|spec| CarChunk {
            offset: spec.offset,
            length: spec.length,
            digest: spec.digest,
            taikai_segment_hint: spec.taikai_segment_hint.clone(),
        })
        .collect();
    Ok(CarBuildPlan {
        chunk_profile: descriptor.profile,
        payload_digest: blake3_hash(&[]),
        content_length,
        chunks,
        files: vec![FilePlan {
            path: vec!["payload.bin".to_string()],
            first_chunk: 0,
            chunk_count: specs.len(),
            size: content_length,
        }],
    })
}

fn scoreboard_path_from_options(options: &JsGatewayFetchOptions) -> napi::Result<Option<PathBuf>> {
    let Some(path) = options.scoreboard_out_path.as_ref() else {
        return Ok(None);
    };
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(invalid_arg(
            "scoreboardOutPath must not be empty when provided",
        ));
    }
    let pathbuf = PathBuf::from(trimmed);
    if let Some(parent) = pathbuf.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            generic_failure(format!(
                "failed to create scoreboard directory `{}`: {err}",
                parent.display()
            ))
        })?;
    }
    Ok(Some(pathbuf))
}

fn trimmed_string_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

#[derive(Clone, Copy)]
struct ScoreboardMetadataInputs<'a> {
    allow_implicit_metadata: bool,
    telemetry_label: Option<&'a str>,
    telemetry_region: Option<&'a str>,
    gateway_manifest_provided: bool,
    gateway_manifest_id: Option<&'a str>,
    gateway_manifest_cid: Option<&'a str>,
    allow_single_source_fallback: bool,
}

fn option_u64_value(value: Option<u64>) -> Value {
    value.map_or(Value::Null, Value::from)
}

fn opt_usize_to_u64(value: Option<usize>, field: &str) -> napi::Result<Option<u64>> {
    value
        .map(|val| {
            u64::try_from(val)
                .map_err(|_| invalid_arg(format!("{field} exceeds 64-bit range (value: {val})")))
        })
        .transpose()
}

fn transport_policy_labels(
    requested: TransportPolicy,
    override_policy: Option<TransportPolicy>,
) -> (&'static str, bool, Option<&'static str>) {
    override_policy.map_or_else(
        || (requested.label(), false, None),
        |policy| (policy.label(), true, Some(policy.label())),
    )
}

fn anonymity_policy_labels(
    requested: AnonymityPolicy,
    override_policy: Option<AnonymityPolicy>,
) -> (&'static str, bool, Option<&'static str>) {
    override_policy.map_or_else(
        || (requested.label(), false, None),
        |policy| (policy.label(), true, Some(policy.label())),
    )
}

#[allow(clippy::too_many_lines)]
fn build_scoreboard_metadata_value(
    provider_count: usize,
    gateway_provider_count: usize,
    config: &OrchestratorConfig,
    inputs: ScoreboardMetadataInputs<'_>,
) -> napi::Result<Value> {
    let provider_count_u64 = u64::try_from(provider_count).map_err(|_| {
        invalid_arg(format!(
            "provider_count exceeds 64-bit range (value: {provider_count})"
        ))
    })?;
    let gateway_count_u64 = u64::try_from(gateway_provider_count).map_err(|_| {
        invalid_arg(format!(
            "gateway_provider_count exceeds 64-bit range (value: {gateway_provider_count})"
        ))
    })?;
    let max_parallel = opt_usize_to_u64(config.fetch.global_parallel_limit, "max_parallel")?;
    let max_peers = opt_usize_to_u64(
        config.max_providers.map(std::num::NonZeroUsize::get),
        "max_peers",
    )?;
    let retry_budget = opt_usize_to_u64(config.fetch.per_chunk_retry_limit, "retry_budget")?;
    let provider_failure_threshold = u64::try_from(config.fetch.provider_failure_threshold)
        .map_err(|_| {
            invalid_arg(format!(
                "provider_failure_threshold exceeds 64-bit range (value: {})",
                config.fetch.provider_failure_threshold
            ))
        })?;
    let mut metadata = Map::new();
    metadata.insert("version".into(), Value::from(env!("CARGO_PKG_VERSION")));
    metadata.insert("use_scoreboard".into(), Value::from(true));
    metadata.insert(
        "allow_implicit_metadata".into(),
        Value::from(inputs.allow_implicit_metadata),
    );
    metadata.insert(
        "allow_single_source_fallback".into(),
        Value::from(inputs.allow_single_source_fallback),
    );
    metadata.insert("provider_count".into(), Value::from(provider_count_u64));
    metadata.insert(
        "gateway_provider_count".into(),
        Value::from(gateway_count_u64),
    );
    metadata.insert("max_parallel".into(), option_u64_value(max_parallel));
    metadata.insert("max_peers".into(), option_u64_value(max_peers));
    metadata.insert("retry_budget".into(), option_u64_value(retry_budget));
    metadata.insert(
        "provider_failure_threshold".into(),
        Value::from(provider_failure_threshold),
    );
    metadata.insert(
        "assume_now".into(),
        Value::from(config.scoreboard.now_unix_secs),
    );
    metadata.insert(
        "telemetry_source".into(),
        inputs
            .telemetry_label
            .map_or(Value::Null, |label| Value::from(label.to_string())),
    );
    metadata.insert(
        "telemetry_region".into(),
        inputs
            .telemetry_region
            .map_or(Value::Null, |label| Value::from(label.to_string())),
    );
    metadata.insert(
        "gateway_manifest_provided".into(),
        Value::from(inputs.gateway_manifest_provided),
    );
    metadata.insert(
        "gateway_manifest_id".into(),
        inputs
            .gateway_manifest_id
            .map_or(Value::Null, |value| Value::from(value.to_string())),
    );
    metadata.insert(
        "gateway_manifest_cid".into(),
        inputs
            .gateway_manifest_cid
            .map_or(Value::Null, |value| Value::from(value.to_string())),
    );
    let (transport_label, transport_override_flag, transport_override_label) =
        transport_policy_labels(
            config.transport_policy,
            config.policy_override.transport_policy,
        );
    metadata.insert("transport_policy".into(), Value::from(transport_label));
    metadata.insert(
        "transport_policy_override".into(),
        Value::from(transport_override_flag),
    );
    metadata.insert(
        "transport_policy_override_label".into(),
        transport_override_label.map_or(Value::Null, Value::from),
    );
    let (anonymity_label, anonymity_override_flag, anonymity_override_label) =
        anonymity_policy_labels(
            config.anonymity_policy,
            config.policy_override.anonymity_policy,
        );
    metadata.insert("anonymity_policy".into(), Value::from(anonymity_label));
    metadata.insert(
        "anonymity_policy_override".into(),
        Value::from(anonymity_override_flag),
    );
    metadata.insert(
        "anonymity_policy_override_label".into(),
        anonymity_override_label.map_or(Value::Null, Value::from),
    );
    let write_mode_label = config.write_mode.label().replace('_', "-");
    metadata.insert("write_mode".into(), Value::from(write_mode_label));
    metadata.insert(
        "write_mode_enforces_pq".into(),
        Value::from(config.write_mode.enforces_pq_only()),
    );
    Ok(Value::Object(metadata))
}

fn provider_mix_label_from_counts(direct: u64, gateway: u64) -> &'static str {
    match (direct > 0, gateway > 0) {
        (true, true) => "mixed",
        (true, false) => "direct-only",
        (false, true) => "gateway-only",
        (false, false) => "none",
    }
}

fn build_gateway_metadata(
    provider_count: usize,
    gateway_provider_count: usize,
    config: &OrchestratorConfig,
    inputs: &ScoreboardMetadataInputs<'_>,
) -> napi::Result<JsGatewayMetadata> {
    let provider_count_u64 = u64::try_from(provider_count).map_err(|_| {
        invalid_arg(format!(
            "provider_count exceeds 64-bit range (value: {provider_count})"
        ))
    })?;
    let gateway_count_u64 = u64::try_from(gateway_provider_count).map_err(|_| {
        invalid_arg(format!(
            "gateway_provider_count exceeds 64-bit range (value: {gateway_provider_count})"
        ))
    })?;
    let provider_mix = provider_mix_label_from_counts(provider_count_u64, gateway_count_u64);
    let (transport_label, transport_override_flag, transport_override_label) =
        transport_policy_labels(
            config.transport_policy,
            config.policy_override.transport_policy,
        );
    let (anonymity_label, anonymity_override_flag, anonymity_override_label) =
        anonymity_policy_labels(
            config.anonymity_policy,
            config.policy_override.anonymity_policy,
        );
    let max_parallel = opt_usize_to_u64(config.fetch.global_parallel_limit, "max_parallel")?;
    let max_peers = opt_usize_to_u64(
        config.max_providers.map(std::num::NonZeroUsize::get),
        "max_peers",
    )?;
    let retry_budget = opt_usize_to_u64(config.fetch.per_chunk_retry_limit, "retry_budget")?;
    let provider_failure_threshold = u64::try_from(config.fetch.provider_failure_threshold)
        .map_err(|_| {
            invalid_arg(format!(
                "provider_failure_threshold exceeds 64-bit range (value: {})",
                config.fetch.provider_failure_threshold
            ))
        })?;
    let write_mode_label = config.write_mode.label().replace('_', "-");
    let write_mode_enforces_pq = config.write_mode.enforces_pq_only();

    Ok(JsGatewayMetadata {
        provider_count: JsU64(provider_count_u64),
        gateway_provider_count: JsU64(gateway_count_u64),
        provider_mix: provider_mix.to_string(),
        transport_policy: transport_label.to_string(),
        transport_policy_override: transport_override_flag,
        transport_policy_override_label: transport_override_label.map(str::to_string),
        anonymity_policy: anonymity_label.to_string(),
        anonymity_policy_override: anonymity_override_flag,
        anonymity_policy_override_label: anonymity_override_label.map(str::to_string),
        write_mode: write_mode_label,
        write_mode_enforces_pq,
        max_parallel: max_parallel.map(JsU64),
        max_peers: max_peers.map(JsU64),
        retry_budget: retry_budget.map(JsU64),
        provider_failure_threshold: JsU64(provider_failure_threshold),
        assume_now_unix: JsU64(config.scoreboard.now_unix_secs),
        telemetry_source_label: inputs.telemetry_label.map(str::to_string),
        telemetry_region: inputs.telemetry_region.map(str::to_string),
        gateway_manifest_provided: inputs.gateway_manifest_provided,
        gateway_manifest_id: inputs.gateway_manifest_id.map(str::to_string),
        gateway_manifest_cid: inputs.gateway_manifest_cid.map(str::to_string),
        allow_single_source_fallback: inputs.allow_single_source_fallback,
        allow_implicit_metadata: inputs.allow_implicit_metadata,
    })
}

fn apply_gateway_options(
    config: &mut OrchestratorConfig,
    options: &JsGatewayFetchOptions,
) -> napi::Result<Option<usize>> {
    if let Some(budget) = options.retry_budget {
        if budget == 0 {
            config.fetch.per_chunk_retry_limit = None;
        } else {
            let limit = usize::try_from(budget).map_err(|_| {
                invalid_arg("retryBudget exceeds supported range (must fit within usize)")
            })?;
            config.fetch.per_chunk_retry_limit = Some(limit);
        }
    }
    let max_peers = if let Some(limit) = options.max_peers {
        let limit = usize::try_from(limit.max(1))
            .map_err(|_| invalid_arg("maxPeers exceeds supported range (must fit within usize)"))?;
        config.fetch.global_parallel_limit = Some(
            config
                .fetch
                .global_parallel_limit
                .map_or(limit, |existing| existing.min(limit)),
        );
        Some(limit)
    } else {
        None
    };
    if let Some(region) = options.telemetry_region.as_ref() {
        let trimmed = region.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "telemetryRegion must not be empty when provided",
            ));
        }
        config.telemetry_region = Some(trimmed.to_string());
    }
    if let Some(phase) = options.rollout_phase.as_ref() {
        let trimmed = phase.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg("rolloutPhase must not be empty when provided"));
        }
        let parsed = RolloutPhase::parse(trimmed).ok_or_else(|| {
            invalid_arg(
                "rolloutPhase must be one of 'canary', 'ramp', 'default', or stage_a/stage_b/stage_c aliases",
            )
        })?;
        config.rollout_phase = parsed;
        if config.anonymity_policy_override.is_none() {
            config.anonymity_policy = parsed.default_anonymity_policy();
        }
    }
    if let Some(policy) = options.transport_policy.as_ref() {
        let trimmed = policy.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "transportPolicy must not be empty when provided",
            ));
        }
        config.transport_policy = TransportPolicy::parse(trimmed).ok_or_else(|| {
            invalid_arg(
                "transportPolicy must be one of 'soranet-first', 'soranet-strict', or 'direct-only'",
            )
        })?;
    }
    if let Some(policy) = options.anonymity_policy.as_ref() {
        let trimmed = policy.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg(
                "anonymityPolicy must not be empty when provided",
            ));
        }
        config.anonymity_policy = AnonymityPolicy::parse(trimmed).ok_or_else(|| {
            invalid_arg(
                "anonymityPolicy must be one of 'anon-guard-pq', 'anon-majority-pq', or 'anon-strict-pq'",
            )
        })?;
        config.anonymity_policy_override = Some(config.anonymity_policy);
    }
    if let Some(mode) = options.write_mode.as_ref() {
        let trimmed = mode.trim();
        if trimmed.is_empty() {
            return Err(invalid_arg("writeMode must not be empty when provided"));
        }
        config.write_mode = WriteModeHint::parse(trimmed).ok_or_else(|| {
            invalid_arg(
                "writeMode must be one of 'read-only', 'read_only', 'upload-pq-only', or 'upload_pq_only'",
            )
        })?;
    }
    if let Some(proxy_cfg) = options.local_proxy.as_ref() {
        let config_value = build_local_proxy_config(proxy_cfg)?;
        config.local_proxy = Some(config_value);
    }
    if let Some(cache_cfg) = options.taikai_cache.as_ref() {
        let cache = build_taikai_cache_config(cache_cfg)?;
        config.taikai_cache = Some(cache);
    }
    Ok(max_peers)
}

fn usize_to_u32(value: usize, field: &str) -> napi::Result<u32> {
    u32::try_from(value)
        .map_err(|_| invalid_arg(format!("{field} exceeds 32-bit range (value: {value})")))
}

#[allow(clippy::too_many_lines)]
fn convert_fetch_session_to_js(
    session: FetchSession,
    manifest_id_hex: &str,
    chunker_handle: &str,
    telemetry_region: Option<String>,
    metadata: JsGatewayMetadata,
) -> napi::Result<JsGatewayFetchResult> {
    let outcome = session.outcome;
    let policy_report = session.policy_report;

    let payload_bytes = outcome.assemble_payload();
    let assembled_bytes_u64 = u64::try_from(payload_bytes.len()).map_err(|_| {
        invalid_arg("assembled payload exceeds u64 range (too large for JavaScript)")
    })?;

    let chunk_count = usize_to_u32(outcome.chunks.len(), "chunk_count")?;
    let provider_reports = outcome
        .provider_reports
        .iter()
        .map(|report| {
            Ok(JsMultiFetchProviderReport {
                provider: report.provider.id().as_str().to_string(),
                successes: u32::try_from(report.successes)
                    .map_err(|_| invalid_arg("provider success count exceeds 32-bit range"))?,
                failures: u32::try_from(report.failures)
                    .map_err(|_| invalid_arg("provider failure count exceeds 32-bit range"))?,
                disabled: report.disabled,
            })
        })
        .collect::<napi::Result<Vec<_>>>()?;

    let chunk_receipts = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| {
            Ok(JsMultiFetchChunkReceipt {
                chunk_index: u32::try_from(receipt.chunk_index)
                    .map_err(|_| invalid_arg("chunk index exceeds 32-bit range in receipt"))?,
                provider: receipt.provider.as_str().to_string(),
                attempts: u32::try_from(receipt.attempts).map_err(|_| {
                    invalid_arg("chunk attempt count exceeds 32-bit range in receipt")
                })?,
                latency_ms: receipt.latency_ms,
                bytes: receipt.bytes,
            })
        })
        .collect::<napi::Result<Vec<_>>>()?;

    let local_proxy_manifest_json = match session.local_proxy_manifest.as_ref() {
        Some(manifest) => {
            let value = json::to_value(manifest).map_err(|err| {
                generic_failure(format!("failed to serialise proxy manifest: {err}"))
            })?;
            Some(json::to_string(&value).map_err(|err| {
                generic_failure(format!("failed to render proxy manifest json: {err}"))
            })?)
        }
        None => None,
    };

    let car_verification = session.car_verification.map(|verification| {
        let car_stats = verification.car_stats;
        let governance = JsManifestGovernance {
            council_signatures: verification
                .manifest_governance
                .council_signatures
                .iter()
                .map(|sig| JsCouncilSignature {
                    signer_hex: hex::encode_upper(sig.signer),
                    signature_hex: hex::encode_upper(sig.signature.as_slice()),
                })
                .collect(),
        };
        JsCarVerification {
            manifest_digest_hex: hex::encode_upper(verification.manifest_digest.as_bytes()),
            manifest_payload_digest_hex: hex::encode_upper(
                verification.manifest_payload_digest.as_bytes(),
            ),
            manifest_car_digest_hex: hex::encode_upper(verification.manifest_car_digest),
            manifest_content_length: JsU64(verification.manifest_content_length),
            manifest_chunk_count: JsU64(verification.manifest_chunk_count),
            manifest_chunk_profile_handle: verification.chunk_profile_handle,
            manifest_governance: governance,
            car_archive: JsCarArchiveStats {
                size: JsU64(car_stats.car_size),
                payload_digest_hex: hex::encode_upper(car_stats.car_payload_digest.as_bytes()),
                archive_digest_hex: hex::encode_upper(car_stats.car_archive_digest.as_bytes()),
                cid_hex: hex::encode_upper(car_stats.car_cid),
                root_cids_hex: car_stats.root_cids.iter().map(hex::encode_upper).collect(),
                verified: true,
                por_leaf_count: JsU64(
                    u64::try_from(verification.por_leaf_count).unwrap_or(u64::MAX),
                ),
            },
        }
    });
    let taikai_cache_summary = session.taikai_cache_stats.map(JsTaikaiCacheStats::from);
    let taikai_cache_queue = session.taikai_cache_queue.map(JsTaikaiQueueStats::from);

    Ok(JsGatewayFetchResult {
        manifest_id_hex: manifest_id_hex.to_string(),
        chunker_handle: chunker_handle.to_string(),
        chunk_count,
        assembled_bytes: JsU64(assembled_bytes_u64),
        payload: Buffer::from(payload_bytes),
        telemetry_region,
        anonymity_policy: policy_report.policy.label().to_string(),
        anonymity_status: policy_report.status_label().to_string(),
        anonymity_reason: policy_report.reason_label().to_string(),
        anonymity_soranet_selected: usize_to_u32(
            policy_report.selected_soranet_total,
            "anonymity_soranet_selected",
        )?,
        anonymity_pq_selected: usize_to_u32(policy_report.selected_pq, "anonymity_pq_selected")?,
        anonymity_classical_selected: usize_to_u32(
            policy_report.selected_classical(),
            "anonymity_classical_selected",
        )?,
        anonymity_classical_ratio: policy_report.classical_ratio(),
        anonymity_pq_ratio: policy_report.pq_ratio(),
        anonymity_candidate_ratio: policy_report.candidate_ratio(),
        anonymity_deficit_ratio: policy_report.deficit_ratio(),
        anonymity_supply_delta: policy_report.supply_delta_ratio(),
        anonymity_brownout: policy_report.is_brownout(),
        anonymity_brownout_effective: policy_report.should_flag_brownout(),
        anonymity_uses_classical: policy_report.uses_classical(),
        provider_reports,
        chunk_receipts,
        local_proxy_manifest_json,
        car_verification,
        metadata,
        taikai_cache_summary,
        taikai_cache_queue,
    })
}

#[cfg(test)]
type FetchViaGatewayOverride = Box<
    dyn Fn(
            OrchestratorConfig,
            &CarBuildPlan,
            GatewayFetchConfig,
            Vec<GatewayProviderInput>,
            Option<&sorafs_car::scoreboard::TelemetrySnapshot>,
            Option<usize>,
        ) -> Result<FetchSession, GatewayOrchestratorError>
        + Send
        + Sync,
>;

#[cfg(test)]
fn fetch_override_slot() -> &'static std::sync::Mutex<Option<FetchViaGatewayOverride>> {
    static STORAGE: std::sync::OnceLock<std::sync::Mutex<Option<FetchViaGatewayOverride>>> =
        std::sync::OnceLock::new();
    STORAGE.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(test)]
pub(crate) struct FetchViaGatewayOverrideGuard;

#[cfg(test)]
pub(crate) fn set_fetch_via_gateway_override<F>(override_fn: F) -> FetchViaGatewayOverrideGuard
where
    F: Fn(
            OrchestratorConfig,
            &CarBuildPlan,
            GatewayFetchConfig,
            Vec<GatewayProviderInput>,
            Option<&sorafs_car::scoreboard::TelemetrySnapshot>,
            Option<usize>,
        ) -> Result<FetchSession, GatewayOrchestratorError>
        + Send
        + Sync
        + 'static,
{
    let slot = fetch_override_slot();
    *slot
        .lock()
        .expect("fetch_via_gateway override mutex poisoned") = Some(Box::new(override_fn));
    FetchViaGatewayOverrideGuard
}

#[cfg(test)]
impl Drop for FetchViaGatewayOverrideGuard {
    fn drop(&mut self) {
        let slot = fetch_override_slot();
        *slot
            .lock()
            .expect("fetch_via_gateway override mutex poisoned during drop") = None;
    }
}

async fn run_fetch_via_gateway(
    config: OrchestratorConfig,
    plan: &CarBuildPlan,
    gateway_config: GatewayFetchConfig,
    provider_inputs: Vec<GatewayProviderInput>,
    telemetry: Option<&sorafs_car::scoreboard::TelemetrySnapshot>,
    max_peers: Option<usize>,
) -> Result<FetchSession, GatewayOrchestratorError> {
    #[cfg(test)]
    if let Some(override_fn) = fetch_override_slot()
        .lock()
        .expect("fetch_via_gateway override mutex poisoned")
        .as_ref()
    {
        return override_fn(
            config,
            plan,
            gateway_config,
            provider_inputs,
            telemetry,
            max_peers,
        );
    }

    fetch_via_gateway(
        config,
        plan,
        gateway_config,
        provider_inputs,
        telemetry,
        max_peers,
    )
    .await
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
#[napi(js_name = "sorafsGatewayFetch")]
/// Execute a gateway-backed multi-provider fetch via the Rust orchestrator.
pub fn sorafs_gateway_fetch(
    manifest_id_hex: String,
    chunker_handle: String,
    plan_json: String,
    providers: Vec<JsGatewayProviderSpec>,
    options: Option<JsGatewayFetchOptions>,
) -> napi::Result<JsGatewayFetchResult> {
    if providers.is_empty() {
        return Err(invalid_arg(
            "providers list must contain at least one entry",
        ));
    }

    let manifest_id = manifest_id_hex.trim().to_ascii_lowercase();
    if manifest_id.len() != 64 || !manifest_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid_arg("manifestIdHex must be a 32-byte hex string"));
    }

    let chunker_handle_trimmed = chunker_handle.trim();
    if chunker_handle_trimmed.is_empty() {
        return Err(invalid_arg("chunkerHandle must not be empty"));
    }

    let plan_value: json::Value = json::from_str(&plan_json)
        .map_err(|err| invalid_arg(format!("failed to parse plan JSON: {err}")))?;
    let mut specs = chunk_fetch_specs_from_json(&plan_value)
        .map_err(|err| invalid_arg(format!("invalid chunk fetch plan: {err}")))?;
    if specs.is_empty() {
        return Err(invalid_arg(
            "chunk fetch plan must contain at least one chunk",
        ));
    }
    specs.sort_by_key(|spec| spec.chunk_index);
    for (expected, spec) in specs.iter().enumerate() {
        if spec.chunk_index != expected {
            return Err(invalid_arg(format!(
                "chunk fetch plan missing chunk index {expected}"
            )));
        }
    }

    let plan = build_gateway_plan(&specs, chunker_handle_trimmed)?;

    let provider_inputs = providers
        .iter()
        .map(build_gateway_provider_input)
        .collect::<napi::Result<Vec<_>>>()?;

    let opts = options.unwrap_or_default();
    let mut orchestrator_config = OrchestratorConfig::default();
    let mut scoreboard_path = scoreboard_path_from_options(&opts)?;
    if let Some(now) = opts.scoreboard_now_unix_secs.as_ref().map(|value| value.0) {
        orchestrator_config.scoreboard.now_unix_secs = now;
    }
    let telemetry_label = trimmed_string_option(opts.scoreboard_telemetry_label.as_deref());
    let allow_implicit_metadata = opts.scoreboard_allow_implicit_metadata.unwrap_or(false);
    let allow_single_source_fallback = opts.allow_single_source_fallback.unwrap_or(false);

    let max_peers = apply_gateway_options(&mut orchestrator_config, &opts)?;
    let telemetry_region = orchestrator_config.telemetry_region.clone();

    let gateway_config = build_gateway_fetch_config(&manifest_id, chunker_handle_trimmed, &opts)?;
    let manifest_envelope_present = gateway_config.manifest_envelope_b64.is_some();
    let manifest_cid_metadata = gateway_config.expected_manifest_cid_hex.clone();

    let direct_provider_count = 0usize;
    let gateway_provider_count = provider_inputs.len();
    let metadata_inputs = ScoreboardMetadataInputs {
        allow_implicit_metadata,
        telemetry_label: telemetry_label.as_deref(),
        telemetry_region: telemetry_region.as_deref(),
        gateway_manifest_provided: manifest_envelope_present,
        gateway_manifest_id: Some(manifest_id.as_str()),
        gateway_manifest_cid: manifest_cid_metadata.as_deref(),
        allow_single_source_fallback,
    };

    if let Some(path) = scoreboard_path.take() {
        let metadata = build_scoreboard_metadata_value(
            direct_provider_count,
            gateway_provider_count,
            &orchestrator_config,
            metadata_inputs,
        )?;
        orchestrator_config.scoreboard.persist_path = Some(path);
        orchestrator_config.scoreboard.persist_metadata = Some(metadata);
    }
    let js_metadata = build_gateway_metadata(
        direct_provider_count,
        gateway_provider_count,
        &orchestrator_config,
        &metadata_inputs,
    )?;

    let runtime = Runtime::new()
        .map_err(|err| generic_failure(format!("failed to initialise Tokio runtime: {err}")))?;

    let session = runtime
        .block_on(run_fetch_via_gateway(
            orchestrator_config,
            &plan,
            gateway_config,
            provider_inputs,
            None::<&sorafs_car::scoreboard::TelemetrySnapshot>,
            max_peers,
        ))
        .map_err(map_gateway_error)?;

    convert_fetch_session_to_js(
        session,
        &manifest_id,
        chunker_handle_trimmed,
        telemetry_region,
        js_metadata,
    )
}

#[napi(js_name = "daManifestChunkerHandle")]
/// Derive the canonical chunker handle used to encode a DA manifest.
#[allow(clippy::needless_pass_by_value)]
pub fn da_manifest_chunker_handle(manifest_bytes: Uint8Array) -> napi::Result<String> {
    let manifest = decode_da_manifest(manifest_bytes.as_ref())?;
    derive_da_chunker_handle(&manifest)
}

#[napi(js_name = "daGenerateProofs")]
/// Generate `PoR` proofs for a DA payload using the canonical manifest chunk plan.
#[allow(clippy::needless_pass_by_value)]
pub fn da_generate_proofs(
    manifest_bytes: Uint8Array,
    payload_bytes: Uint8Array,
    options: Option<JsDaProofOptions>,
) -> napi::Result<JsDaProofSummary> {
    let manifest = decode_da_manifest(manifest_bytes.as_ref())?;
    let payload = payload_bytes.to_vec();
    let opts = DaProofOptionsInternal::from_js(options)?;
    let iroha_config = opts.to_iroha_config();
    let summary_value = iroha_generate_da_proof_summary(&manifest, &payload, &iroha_config)
        .map_err(|err| generic_failure(format!("failed to generate DA proof summary: {err}")))?;
    let summary = DaProofSummaryInternal::try_from(summary_value)?;
    Ok(summary.into())
}

const DEFAULT_DA_SAMPLE_COUNT: usize = 8;

#[derive(Clone)]
struct DaProofSummaryInternal {
    blob_hash_hex: String,
    chunk_root_hex: String,
    por_root_hex: String,
    leaf_count: usize,
    segment_count: usize,
    chunk_count: usize,
    sample_count: usize,
    sample_seed: u64,
    proofs: Vec<JsDaProofRecord>,
}

#[derive(Clone)]
struct DaProofOptionsInternal {
    sample_count: usize,
    sample_seed: u64,
    leaf_indexes: Vec<usize>,
}

impl DaProofOptionsInternal {
    fn from_js(options: Option<JsDaProofOptions>) -> napi::Result<Self> {
        let opts = options.unwrap_or_default();
        let sample_count = opts
            .sample_count
            .map(|value| {
                usize::try_from(value).map_err(|_| invalid_arg("sampleCount exceeds host limits"))
            })
            .transpose()?
            .unwrap_or(DEFAULT_DA_SAMPLE_COUNT);
        let sample_seed = opts.sample_seed.map_or(0, |value| {
            let raw: u64 = value.into();
            raw
        });
        let mut leaf_indexes = Vec::new();
        if let Some(values) = opts.leaf_indexes {
            for (idx, entry) in values.into_iter().enumerate() {
                let raw: u64 = entry.into();
                let coerced = usize::try_from(raw).map_err(|_| {
                    invalid_arg(format!(
                        "leafIndexes[{idx}] exceeds host limits (value must fit within usize)"
                    ))
                })?;
                leaf_indexes.push(coerced);
            }
        }
        Ok(Self {
            sample_count,
            sample_seed,
            leaf_indexes,
        })
    }

    fn to_iroha_config(&self) -> IrohaDaProofConfig {
        IrohaDaProofConfig {
            sample_count: self.sample_count,
            sample_seed: self.sample_seed,
            leaf_indexes: self.leaf_indexes.clone(),
        }
    }
}

impl From<DaProofSummaryInternal> for JsDaProofSummary {
    fn from(summary: DaProofSummaryInternal) -> Self {
        JsDaProofSummary {
            blob_hash_hex: summary.blob_hash_hex,
            chunk_root_hex: summary.chunk_root_hex,
            por_root_hex: summary.por_root_hex,
            leaf_count: JsU64(summary.leaf_count as u64),
            segment_count: JsU64(summary.segment_count as u64),
            chunk_count: JsU64(summary.chunk_count as u64),
            sample_count: u32::try_from(summary.sample_count).unwrap_or(u32::MAX),
            sample_seed: JsU64(summary.sample_seed),
            proof_count: u32::try_from(summary.proofs.len()).unwrap_or(u32::MAX),
            proofs: summary.proofs,
        }
    }
}

impl TryFrom<Value> for DaProofSummaryInternal {
    type Error = napi::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let map = value
            .as_object()
            .ok_or_else(|| invalid_arg("DA proof summary must be a JSON object"))?;
        let proofs_value = map
            .get("proofs")
            .ok_or_else(|| invalid_arg("DA proof summary missing `proofs` field"))?;
        let proofs_array = proofs_value
            .as_array()
            .ok_or_else(|| invalid_arg("`proofs` must be an array"))?;
        let mut proofs = Vec::with_capacity(proofs_array.len());
        for (idx, entry) in proofs_array.iter().enumerate() {
            proofs.push(parse_da_proof_record(entry, idx)?);
        }

        Ok(Self {
            blob_hash_hex: string_field(map, "blob_hash")?,
            chunk_root_hex: string_field(map, "chunk_root")?,
            por_root_hex: string_field(map, "por_root")?,
            leaf_count: usize_field(map, "leaf_count")?,
            segment_count: usize_field(map, "segment_count")?,
            chunk_count: usize_field(map, "chunk_count")?,
            sample_count: usize_field(map, "sample_count")?,
            sample_seed: u64_field(map, "sample_seed")?,
            proofs,
        })
    }
}

fn parse_da_proof_record(value: &Value, index: usize) -> napi::Result<JsDaProofRecord> {
    let ctx = format!("proofs[{index}]");
    let map = value
        .as_object()
        .ok_or_else(|| invalid_arg(format!("{ctx} must be an object")))?;
    Ok(JsDaProofRecord {
        origin: string_field_ctx(map, "origin", &ctx)?,
        leaf_index: u32_field_ctx(map, "leaf_index", &ctx)?,
        chunk_index: u32_field_ctx(map, "chunk_index", &ctx)?,
        segment_index: u32_field_ctx(map, "segment_index", &ctx)?,
        leaf_offset: JsU64(u64_field_ctx(map, "leaf_offset", &ctx)?),
        leaf_length: u32_field_ctx(map, "leaf_length", &ctx)?,
        segment_offset: JsU64(u64_field_ctx(map, "segment_offset", &ctx)?),
        segment_length: u32_field_ctx(map, "segment_length", &ctx)?,
        chunk_offset: JsU64(u64_field_ctx(map, "chunk_offset", &ctx)?),
        chunk_length: u32_field_ctx(map, "chunk_length", &ctx)?,
        payload_len: JsU64(u64_field_ctx(map, "payload_len", &ctx)?),
        chunk_digest_hex: string_field_ctx(map, "chunk_digest", &ctx)?,
        chunk_root_hex: string_field_ctx(map, "chunk_root", &ctx)?,
        segment_digest_hex: string_field_ctx(map, "segment_digest", &ctx)?,
        leaf_digest_hex: string_field_ctx(map, "leaf_digest", &ctx)?,
        leaf_bytes_b64: string_field_ctx(map, "leaf_bytes_b64", &ctx)?,
        segment_leaves_hex: string_list_field_ctx(map, "segment_leaves", &ctx)?,
        chunk_segments_hex: string_list_field_ctx(map, "chunk_segments", &ctx)?,
        chunk_roots_hex: string_list_field_ctx(map, "chunk_roots", &ctx)?,
        verified: bool_field_ctx(map, "verified", &ctx)?,
    })
}

fn string_field(map: &Map, key: &str) -> napi::Result<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid_arg(format!("DA proof summary missing string field `{key}`")))
}

fn string_field_ctx(map: &Map, key: &str, ctx: &str) -> napi::Result<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid_arg(format!("{ctx} missing string field `{key}`")))
}

fn string_list_field_ctx(map: &Map, key: &str, ctx: &str) -> napi::Result<Vec<String>> {
    let value = map
        .get(key)
        .ok_or_else(|| invalid_arg(format!("{ctx} missing `{key}` field")))?;
    let array = value
        .as_array()
        .ok_or_else(|| invalid_arg(format!("{ctx}.{key} must be an array")))?;
    let mut entries = Vec::with_capacity(array.len());
    for (idx, entry) in array.iter().enumerate() {
        if let Value::String(inner) = entry {
            entries.push(inner.clone());
        } else {
            return Err(invalid_arg(format!("{ctx}.{key}[{idx}] must be a string")));
        }
    }
    Ok(entries)
}

fn bool_field_ctx(map: &Map, key: &str, ctx: &str) -> napi::Result<bool> {
    match map.get(key) {
        Some(Value::Bool(flag)) => Ok(*flag),
        _ => Err(invalid_arg(format!("{ctx} missing boolean field `{key}`"))),
    }
}

fn u64_field(map: &Map, key: &str) -> napi::Result<u64> {
    map.get(key)
        .and_then(value_to_u64)
        .ok_or_else(|| invalid_arg(format!("DA proof summary missing integer field `{key}`")))
}

fn u64_field_ctx(map: &Map, key: &str, ctx: &str) -> napi::Result<u64> {
    map.get(key)
        .and_then(value_to_u64)
        .ok_or_else(|| invalid_arg(format!("{ctx} missing integer field `{key}`")))
}

fn usize_field(map: &Map, key: &str) -> napi::Result<usize> {
    let value = u64_field(map, key)?;
    usize::try_from(value).map_err(|_| invalid_arg(format!("`{key}` exceeds host limits")))
}

fn u32_field_ctx(map: &Map, key: &str, ctx: &str) -> napi::Result<u32> {
    let value = u64_field_ctx(map, key, ctx)?;
    u32::try_from(value)
        .map_err(|_| invalid_arg(format!("{ctx}.{key} exceeds 32-bit integer limits")))
}

fn value_to_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(raw) => raw.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn decode_da_manifest(bytes: &[u8]) -> napi::Result<DaManifestV1> {
    decode_from_bytes(bytes)
        .map_err(|err| invalid_arg(format!("failed to decode DA manifest: {err}")))
}

fn derive_da_chunker_handle(manifest: &DaManifestV1) -> napi::Result<String> {
    let plan = build_car_plan_from_manifest(manifest)
        .map_err(|err| invalid_arg(format!("failed to derive chunk plan from manifest: {err}")))?;
    let descriptor = sorafs_manifest::chunker_registry::lookup_by_profile(
        plan.chunk_profile,
        sorafs_manifest::chunker_registry::DEFAULT_MULTIHASH_CODE,
    )
    .unwrap_or_else(sorafs_manifest::chunker_registry::default_descriptor);
    Ok(format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    ))
}

fn map_local_fetch_error(err: LocalFetchError) -> napi::Error {
    match err {
        LocalFetchError::NoProviders => {
            invalid_arg("providers list must contain at least one entry")
        }
        LocalFetchError::DuplicateProvider(name) => {
            invalid_arg(format!("duplicate provider '{name}'"))
        }
        LocalFetchError::ProviderPathMissing { path } => invalid_arg(format!(
            "provider payload '{}' does not exist",
            path.display()
        )),
        LocalFetchError::ProviderPathNotFile { path } => invalid_arg(format!(
            "provider payload '{}' is not a regular file",
            path.display()
        )),
        LocalFetchError::InvalidMaxConcurrent => {
            invalid_arg("max_concurrent must be greater than zero when provided")
        }
        LocalFetchError::InvalidWeight => {
            invalid_arg("weight must be greater than zero when provided")
        }
        LocalFetchError::InvalidPlan(err) => invalid_arg(err.to_string()),
        LocalFetchError::MissingScoreboardMetadata(name) => invalid_arg(format!(
            "scoreboard requires metadata for provider '{name}' (provide advert metadata or disable use_scoreboard)"
        )),
        LocalFetchError::ScoreboardExcludedAll => {
            invalid_arg("no providers available after applying scoreboard filters")
        }
        LocalFetchError::ScoreboardBuild(err) => {
            generic_failure(format!("failed to build scoreboard: {err}"))
        }
        LocalFetchError::Fetch(message) => {
            let status = if message.starts_with("multi-fetch failed:") {
                napi::Status::GenericFailure
            } else {
                napi::Status::InvalidArg
            };
            napi::Error::new(status, message)
        }
        LocalFetchError::UnknownChunkerHandle(handle) => {
            invalid_arg(format!("unknown chunker handle '{handle}'"))
        }
    }
}

fn invalid_arg(message: impl Into<String>) -> napi::Error {
    napi::Error::new(napi::Status::InvalidArg, message.into())
}

fn generic_failure(message: impl Into<String>) -> napi::Error {
    napi::Error::new(napi::Status::GenericFailure, message.into())
}

fn map_gateway_error(err: GatewayOrchestratorError) -> napi::Error {
    match err {
        GatewayOrchestratorError::Orchestrator(OrchestratorError::MultiSource(multi)) => {
            multi_source_js_error(multi)
        }
        other => generic_failure(format!("sorafs gateway fetch failed: {other}")),
    }
}

fn multi_source_js_error(error: MultiSourceError) -> napi::Error {
    use multi_fetch::MultiSourceError::*;

    let message = format!("{error}");
    let payload = match error {
        NoProviders => norito_json!({
            "kind": "multi_source",
            "code": "no_providers",
            "message": message,
            "retryable": false,
        }),
        NoHealthyProviders {
            chunk_index,
            attempts,
            last_error,
        } => norito_json!({
            "kind": "multi_source",
            "code": "no_healthy_providers",
            "message": message,
            "chunkIndex": chunk_index,
            "attempts": attempts,
            "lastError": last_error.map(|error| attempt_error_to_value(*error)),
            "retryable": true,
        }),
        NoCompatibleProviders {
            chunk_index,
            providers,
        } => norito_json!({
            "kind": "multi_source",
            "code": "no_compatible_providers",
            "message": message,
            "chunkIndex": chunk_index,
            "providers": capability_mismatch_values(&providers),
            "retryable": false,
        }),
        ExhaustedRetries {
            chunk_index,
            attempts,
            last_error,
        } => norito_json!({
            "kind": "multi_source",
            "code": "exhausted_retries",
            "message": message,
            "chunkIndex": chunk_index,
            "attempts": attempts,
            "lastError": attempt_error_to_value(*last_error),
            "retryable": false,
        }),
        ObserverFailed {
            chunk_index,
            source,
        } => norito_json!({
            "kind": "multi_source",
            "code": "observer_failed",
            "message": message,
            "chunkIndex": chunk_index,
            "observerError": source.to_string(),
            "retryable": false,
        }),
        InternalInvariant(reason) => norito_json!({
            "kind": "multi_source",
            "code": "internal_invariant",
            "message": message,
            "details": reason,
            "retryable": false,
        }),
    };
    norito::json::to_string(&payload).map_or_else(
        |_| napi::Error::new(napi::Status::GenericFailure, message),
        |rendered| napi::Error::new(napi::Status::GenericFailure, rendered),
    )
}

fn attempt_error_to_value(error: AttemptError) -> Value {
    norito_json!({
        "providerId": error.provider.to_string(),
        "failure": attempt_failure_to_value(error.failure),
    })
}

fn attempt_failure_to_value(failure: AttemptFailure) -> Value {
    match failure {
        AttemptFailure::Provider {
            message,
            policy_block,
        } => {
            let policy = policy_block.map(|policy| {
                norito_json!({
                    "observedStatus": policy.observed_status.as_u16(),
                    "canonicalStatus": policy.canonical_status.as_u16(),
                    "code": policy.code,
                    "cacheVersion": policy.cache_version,
                    "denylistVersion": policy.denylist_version,
                    "proofTokenPresent": policy.proof_token_present,
                    "message": policy.message,
                })
            });
            norito_json!({
                "kind": "provider",
                "message": message,
                "policyBlock": policy,
            })
        }
        AttemptFailure::InvalidChunk(reason) => norito_json!({
            "kind": "invalid_chunk",
            "reason": chunk_verification_error_value(&reason),
        }),
    }
}

/// Verify a manifest/payload pair and emit `PoR` proofs for JavaScript callers.
#[napi]
/// Generate a summary digest describing the provided manifest/payload `PoR` proofs.
#[allow(clippy::needless_pass_by_value)]
pub fn da_generate_proof_summary(
    manifest_bytes: Buffer,
    payload_bytes: Buffer,
    options: Option<JsDaProofOptions>,
) -> napi::Result<JsDaProofSummary> {
    if manifest_bytes.is_empty() {
        return Err(invalid_arg("manifest bytes must not be empty"));
    }
    if payload_bytes.is_empty() {
        return Err(invalid_arg("payload bytes must not be empty"));
    }
    let manifest: DaManifestV1 = decode_from_bytes(manifest_bytes.as_ref()).map_err(|err| {
        napi::Error::from_reason(format!("failed to decode DA manifest bytes: {err}"))
    })?;
    if manifest.total_stripes == 0 || manifest.shards_per_stripe == 0 {
        return Err(invalid_arg(
            "DA manifest missing total_stripes or shards_per_stripe",
        ));
    }
    let plan = build_car_plan_from_manifest(&manifest)?;
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    let mut source = InMemoryPayload::new(payload_bytes.as_ref());
    store
        .ingest_plan_source(&plan, &mut source)
        .map_err(chunk_store_err)?;
    validate_manifest_consistency(&manifest, &store)?;

    let proof_options = DaProofOptionsNormalized::from_js(options)?;
    let por_root = *store.por_tree().root();
    let mut reports =
        collect_sampled_proofs(&store, payload_bytes.as_ref(), &proof_options, &por_root)?;
    let mut explicit =
        collect_explicit_proofs(&store, payload_bytes.as_ref(), &proof_options, &por_root)?;
    reports.append(&mut explicit);

    let proofs_js = reports.iter().map(proof_to_js_record).collect::<Vec<_>>();
    Ok(JsDaProofSummary {
        blob_hash_hex: hex::encode(manifest.blob_hash.as_ref()),
        chunk_root_hex: hex::encode(manifest.chunk_root.as_ref()),
        por_root_hex: hex::encode(por_root),
        leaf_count: JsU64(store.por_tree().leaf_count() as u64),
        segment_count: JsU64(store.por_tree().segment_count() as u64),
        chunk_count: JsU64(store.chunks().len() as u64),
        sample_count: u32::try_from(proof_options.sample_count).unwrap_or(u32::MAX),
        sample_seed: JsU64(proof_options.sample_seed),
        proof_count: u32::try_from(proofs_js.len()).unwrap_or(u32::MAX),
        proofs: proofs_js,
    })
}

fn chunk_verification_error_value(error: &ChunkVerificationError) -> Value {
    match error {
        ChunkVerificationError::LengthMismatch { expected, actual } => {
            norito_json!({
                "kind": "length_mismatch",
                "expected": *expected,
                "actual": *actual,
            })
        }
        ChunkVerificationError::DigestMismatch { expected, actual } => {
            norito_json!({
                "kind": "digest_mismatch",
                "expected": hex::encode(expected),
                "actual": hex::encode(actual),
            })
        }
    }
}

fn capability_mismatch_values(
    providers: &[(multi_fetch::ProviderId, CapabilityMismatch)],
) -> Value {
    let entries: Vec<Value> = providers
        .iter()
        .map(|(provider, mismatch)| capability_mismatch_entry(provider, mismatch))
        .collect();
    Value::Array(entries)
}

fn capability_mismatch_entry(
    provider: &multi_fetch::ProviderId,
    mismatch: &CapabilityMismatch,
) -> Value {
    match mismatch {
        CapabilityMismatch::MissingRangeCapability => norito_json!({
            "providerId": provider.to_string(),
            "reason": mismatch.to_string(),
        }),
        CapabilityMismatch::ChunkTooLarge {
            chunk_length,
            max_span,
        } => norito_json!({
            "providerId": provider.to_string(),
            "reason": mismatch.to_string(),
            "chunkLength": *chunk_length,
            "maxSpan": *max_span,
        }),
        CapabilityMismatch::OffsetMisaligned {
            offset,
            required_alignment,
        } => norito_json!({
            "providerId": provider.to_string(),
            "reason": mismatch.to_string(),
            "offset": *offset,
            "requiredAlignment": *required_alignment,
        }),
        CapabilityMismatch::LengthMisaligned {
            length,
            required_alignment,
        } => norito_json!({
            "providerId": provider.to_string(),
            "reason": mismatch.to_string(),
            "length": *length,
            "requiredAlignment": *required_alignment,
        }),
        CapabilityMismatch::StreamBurstTooSmall {
            chunk_length,
            burst_limit,
        } => norito_json!({
            "providerId": provider.to_string(),
            "reason": mismatch.to_string(),
            "chunkLength": *chunk_length,
            "burstLimit": *burst_limit,
        }),
    }
}

#[allow(clippy::needless_pass_by_value)] // napi-rs requires owned `String`
#[napi(js_name = "sorafsMultiFetchLocal")]
/// Execute a multi-provider fetch entirely against the local filesystem.
pub fn sorafs_multi_fetch_local(
    plan_json: String,
    providers: Vec<JsLocalProviderSpec>,
    options: Option<JsMultiFetchOptions>,
) -> napi::Result<JsMultiFetchResult> {
    let plan_value: json::Value = json::from_str(&plan_json).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("failed to parse plan JSON: {err}"),
        )
    })?;

    let provider_inputs = providers
        .into_iter()
        .map(js_provider_to_local)
        .collect::<napi::Result<Vec<_>>>()?;

    let options = build_local_fetch_options(options)?;

    let result = local_fetch::execute_local_fetch(&plan_value, provider_inputs, options)
        .map_err(map_local_fetch_error)?;

    local_fetch_result_to_js(result)
}

fn to_js_replication_assignments(
    assignments: Vec<sorafs_manifest::capacity::ReplicationAssignmentV1>,
) -> Vec<JsReplicationAssignment> {
    assignments
        .into_iter()
        .map(|assignment| JsReplicationAssignment {
            provider_id_hex: hex::encode(assignment.provider_id),
            slice_gib: i64::try_from(assignment.slice_gib)
                .expect("slice_gib should fit within JavaScript safe integers"),
            lane: assignment.lane,
        })
        .collect()
}

fn to_js_replication_metadata(
    metadata: Vec<sorafs_manifest::capacity::CapacityMetadataEntry>,
) -> Vec<JsReplicationMetadataEntry> {
    metadata
        .into_iter()
        .map(|entry| JsReplicationMetadataEntry {
            key: entry.key,
            value: entry.value,
        })
        .collect()
}

fn to_js_replication_order(order: ReplicationOrderV1) -> napi::Result<JsReplicationOrder> {
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

    let manifest_cid_base64 = STANDARD.encode(&manifest_cid);
    let manifest_cid_utf8 = String::from_utf8(manifest_cid).ok();
    let target_replicas = u32::from(target_replicas);
    let issued_at_unix = i64::try_from(issued_at).map_err(|_| {
        napi::Error::new(
            napi::Status::GenericFailure,
            "issued_at exceeds JavaScript integer range",
        )
    })?;
    let deadline_at_unix = i64::try_from(deadline_at).map_err(|_| {
        napi::Error::new(
            napi::Status::GenericFailure,
            "deadline_at exceeds JavaScript integer range",
        )
    })?;

    let js_sla = JsReplicationSla {
        ingest_deadline_secs: sla.ingest_deadline_secs,
        min_availability_percent_milli: sla.min_availability_percent_milli,
        min_por_success_percent_milli: sla.min_por_success_percent_milli,
    };

    Ok(JsReplicationOrder {
        schema_version: version,
        order_id_hex: hex::encode(order_id),
        manifest_cid_utf8,
        manifest_cid_base64,
        manifest_digest_hex: hex::encode(manifest_digest),
        chunking_profile,
        target_replicas,
        assignments: to_js_replication_assignments(assignments),
        issued_at_unix,
        deadline_at_unix,
        sla: js_sla,
        metadata: to_js_replication_metadata(metadata),
    })
}

/// Decode a Norito-encoded replication order into a typed JavaScript object.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Uint8Array boundary requires ownership
pub fn sorafs_decode_replication_order(bytes: Uint8Array) -> napi::Result<JsReplicationOrder> {
    let order: ReplicationOrderV1 =
        decode_from_bytes(bytes.as_ref()).map_err(|err| norito_to_napi(format!("{err}")))?;
    order
        .validate()
        .map_err(|err| norito_to_napi(format!("invalid replication order: {err}")))?;
    to_js_replication_order(order)
}

fn parse_hash_string(input: &str, context: &str) -> napi::Result<Hash> {
    let trimmed = input.trim();
    if trimmed.starts_with("hash:") {
        return json::from_value(json::Value::String(trimmed.to_owned())).map_err(norito_to_napi);
    }
    if trimmed.len() != Hash::LENGTH * 2
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c.is_ascii_whitespace())
    {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be a 64-character hexadecimal hash literal"),
        ));
    }
    let uppercase = trimmed.to_ascii_uppercase();
    Hash::from_str(&uppercase).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} invalid hash literal: {err}"),
        )
    })
}

fn parse_hash_value(value: json::Value, context: &str) -> napi::Result<Hash> {
    match value {
        json::Value::String(ref s) => parse_hash_string(s, context),
        other => json::from_value(other).map_err(norito_to_napi),
    }
}

fn parse_optional_hash(value: Option<json::Value>, context: &str) -> napi::Result<Option<Hash>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(value) => parse_hash_value(value, context).map(Some),
    }
}

fn parse_optional_string_value(
    value: Option<json::Value>,
    context: &str,
) -> napi::Result<Option<String>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::String(s)) => Ok(Some(s)),
        Some(other) => parse_string_value(other, context).map(Some),
    }
}

fn parse_keyed_hash(value: json::Value, context: &str) -> napi::Result<KeyedHash> {
    let mut map = match value {
        json::Value::Object(map) => map,
        other => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be an object (found {other:?})"),
            ));
        }
    };
    let pepper_id = parse_string_value(
        required_value(&mut map, "pepper_id", context)?,
        &format!("{context}.pepper_id"),
    )?;
    let digest = parse_hash_value(
        required_value(&mut map, "digest", context)?,
        &format!("{context}.digest"),
    )?;
    Ok(KeyedHash { pepper_id, digest })
}

fn parse_optional_commitment(
    value: Option<json::Value>,
    context: &str,
) -> napi::Result<Option<KaigiParticipantCommitment>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::Object(mut map)) => {
            let commitment_value = map.remove("commitment").ok_or_else(|| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("{context}.commitment field missing"),
                )
            })?;
            let commitment = parse_hash_value(commitment_value, &format!("{context}.commitment"))?;
            let alias_tag_value = map.remove("alias_tag").or_else(|| map.remove("aliasTag"));
            let alias_tag = match alias_tag_value {
                None | Some(json::Value::Null) => None,
                Some(json::Value::String(s)) => Some(s),
                Some(other) => {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("{context}.alias_tag must be a string when present, got {other:?}"),
                    ));
                }
            };
            Ok(Some(KaigiParticipantCommitment {
                commitment,
                alias_tag,
            }))
        }
        Some(other) => json::from_value(other).map(Some).map_err(norito_to_napi),
    }
}

fn parse_optional_nullifier(
    value: Option<json::Value>,
    context: &str,
) -> napi::Result<Option<KaigiParticipantNullifier>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::Object(mut map)) => {
            let digest_value = map.remove("digest").ok_or_else(|| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("{context}.digest field missing"),
                )
            })?;
            let digest = parse_hash_value(digest_value, &format!("{context}.digest"))?;
            let issued_at_value = map
                .remove("issued_at_ms")
                .or_else(|| map.remove("issuedAtMs").or_else(|| map.remove("issuedAt")));
            let issued_at_ms: u64 = issued_at_value
                .ok_or_else(|| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("{context}.issued_at_ms field missing"),
                    )
                })
                .and_then(|value| json::from_value(value).map_err(norito_to_napi))?;
            Ok(Some(KaigiParticipantNullifier {
                digest,
                issued_at_ms,
            }))
        }
        Some(other) => json::from_value(other).map(Some).map_err(norito_to_napi),
    }
}

fn optional_hash_to_json(value: Option<&Hash>) -> json::Value {
    value.map_or(json::Value::Null, |hash| {
        json::to_value(hash).expect("hash serialization")
    })
}

fn optional_commitment_to_json(value: Option<&KaigiParticipantCommitment>) -> json::Value {
    value.map_or(json::Value::Null, |commitment| {
        let mut map = json::Map::new();
        map.insert(
            "commitment".to_owned(),
            json::to_value(&commitment.commitment).expect("commitment serialization"),
        );
        map.insert(
            "alias_tag".to_owned(),
            commitment
                .alias_tag
                .as_ref()
                .map_or(json::Value::Null, |alias| {
                    json::Value::String(alias.clone())
                }),
        );
        json::Value::Object(map)
    })
}

fn optional_nullifier_to_json(value: Option<&KaigiParticipantNullifier>) -> json::Value {
    value.map_or(json::Value::Null, |nullifier| {
        let mut map = json::Map::new();
        map.insert(
            "digest".to_owned(),
            json::to_value(&nullifier.digest).expect("nullifier serialization"),
        );
        map.insert(
            "issued_at_ms".to_owned(),
            json::Value::Number(json::Number::U64(nullifier.issued_at_ms)),
        );
        json::Value::Object(map)
    })
}

fn optional_proof_to_json(value: Option<&Vec<u8>>) -> json::Value {
    value.map_or(json::Value::Null, |bytes| {
        json::Value::String(STANDARD.encode(bytes))
    })
}

fn parse_optional_base64(
    value: Option<json::Value>,
    context: &str,
) -> napi::Result<Option<Vec<u8>>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::String(s)) => STANDARD.decode(s.as_bytes()).map(Some).map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be a valid base64 string: {err}"),
            )
        }),
        Some(other) => json::from_value(other).map_err(norito_to_napi),
    }
}

fn parse_base64(value: json::Value, context: &str) -> napi::Result<Vec<u8>> {
    match value {
        json::Value::String(s) => STANDARD.decode(s.as_bytes()).map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be a valid base64 string: {err}"),
            )
        }),
        json::Value::Array(bytes) => {
            let mut buffer = Vec::with_capacity(bytes.len());
            for (index, value) in bytes.into_iter().enumerate() {
                let number = match value {
                    json::Value::Number(n) => n.as_u64().ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("{context}[{index}] must be an unsigned byte"),
                        )
                    })?,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("{context}[{index}] must be an unsigned byte, found {other:?}"),
                        ));
                    }
                };
                if number > 0xFF {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("{context}[{index}] must be between 0 and 255"),
                    ));
                }
                buffer.push(u8::try_from(number).expect("validated byte range"));
            }
            Ok(buffer)
        }
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be a base64 string or byte array (found {other:?})"),
        )),
    }
}

fn required_value(map: &mut json::Map, key: &str, context: &str) -> napi::Result<json::Value> {
    map.remove(key).ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context}.{key} field missing"),
        )
    })
}

fn parse_string_value(value: json::Value, context: &str) -> napi::Result<String> {
    match value {
        json::Value::String(s) => Ok(s),
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be a string (found {other:?})"),
        )),
    }
}

fn parse_account_id_value(value: json::Value, context: &str) -> napi::Result<AccountId> {
    let literal = parse_string_value(value, context)?;
    parse_account_id(&literal, context)
}

fn parse_rwa_id_value(value: json::Value, context: &str) -> napi::Result<RwaId> {
    let literal = parse_string_value(value, context)?;
    literal.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid RWA id `{literal}`: {err}"),
        )
    })
}

fn account_id_to_canonical_i105(account_id: &AccountId) -> napi::Result<String> {
    account_id.canonical_i105().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("failed to encode account id as canonical I105: {err}"),
        )
    })
}

fn parse_rwa_parent_refs_value(
    value: json::Value,
    context: &str,
) -> napi::Result<Vec<RwaParentRef>> {
    let json::Value::Array(entries) = value else {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be an array"),
        ));
    };
    let mut parents = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let entry_context = format!("{context}[{index}]");
        let json::Value::Object(mut fields) = entry else {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("{entry_context} must be an object"),
            ));
        };
        let rwa = parse_rwa_id_value(
            required_value(&mut fields, "rwa", &entry_context)?,
            &format!("{entry_context}.rwa"),
        )?;
        let quantity: Numeric =
            json::from_value(required_value(&mut fields, "quantity", &entry_context)?)
                .map_err(norito_to_napi)?;
        parents.push(RwaParentRef::new(rwa, quantity));
    }
    Ok(parents)
}

fn rwa_parent_refs_to_json(parents: &[RwaParentRef]) -> json::Value {
    json::Value::Array(
        parents
            .iter()
            .map(|parent| {
                norito_json!({
                    "rwa": parent.rwa().to_string(),
                    "quantity": parent.quantity(),
                })
            })
            .collect(),
    )
}

fn rwa_status_to_json(status: Option<&Name>) -> json::Value {
    status.map_or(json::Value::Null, |status| {
        json::Value::String(status.to_string())
    })
}

fn rwa_control_policy_to_json(policy: &RwaControlPolicy) -> napi::Result<json::Value> {
    let controller_accounts = policy
        .controller_accounts()
        .iter()
        .map(account_id_to_canonical_i105)
        .collect::<napi::Result<Vec<_>>>()?;
    let mut payload = json::Map::new();
    payload.insert(
        "controller_accounts".to_owned(),
        json::to_value(&controller_accounts).map_err(norito_to_napi)?,
    );
    payload.insert(
        "controller_roles".to_owned(),
        json::to_value(
            &policy
                .controller_roles()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .map_err(norito_to_napi)?,
    );
    payload.insert(
        "freeze_enabled".to_owned(),
        json::Value::Bool(*policy.freeze_enabled()),
    );
    payload.insert(
        "hold_enabled".to_owned(),
        json::Value::Bool(*policy.hold_enabled()),
    );
    payload.insert(
        "force_transfer_enabled".to_owned(),
        json::Value::Bool(*policy.force_transfer_enabled()),
    );
    payload.insert(
        "redeem_enabled".to_owned(),
        json::Value::Bool(*policy.redeem_enabled()),
    );
    Ok(json::Value::Object(payload))
}

fn new_rwa_to_json(rwa: &NewRwa) -> napi::Result<json::Value> {
    Ok(norito_json!({
        "domain": rwa.domain(),
        "quantity": rwa.quantity(),
        "spec": rwa.spec(),
        "primary_reference": rwa.primary_reference(),
        "status": rwa_status_to_json(rwa.status().as_ref()),
        "metadata": rwa.metadata(),
        "parents": rwa_parent_refs_to_json(rwa.parents()),
        "controls": rwa_control_policy_to_json(rwa.controls())?,
    }))
}

fn normalize_zk_ballot_public_inputs_json(raw: &str, context: &str) -> napi::Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be valid JSON"),
        ));
    }
    let mut value: json::Value = json::from_str(trimmed).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be valid JSON: {err}"),
        )
    })?;
    normalize_zk_ballot_public_inputs(&mut value, context)?;
    json::to_string(&value).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be valid JSON: {err}"),
        )
    })
}

fn normalize_zk_ballot_public_inputs(value: &mut json::Value, context: &str) -> napi::Result<()> {
    let map = match value {
        json::Value::Object(map) => map,
        other => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be a JSON object (found {other:?})"),
            ));
        }
    };
    reject_zk_public_input_key(map, "durationBlocks", "duration_blocks", context)?;
    reject_zk_public_input_key(map, "root_hint_hex", "root_hint", context)?;
    reject_zk_public_input_key(map, "rootHintHex", "root_hint", context)?;
    reject_zk_public_input_key(map, "rootHint", "root_hint", context)?;
    reject_zk_public_input_key(map, "nullifier_hex", "nullifier", context)?;
    reject_zk_public_input_key(map, "nullifierHex", "nullifier", context)?;
    canonicalize_hex32_public_input(map, "root_hint", "root_hint", context)?;
    canonicalize_hex32_public_input(map, "nullifier", "nullifier", context)?;
    let has_owner = zk_hint_present(map, "owner");
    let has_amount = zk_hint_present(map, "amount");
    let has_duration = zk_hint_present(map, "duration_blocks");
    let any = has_owner || has_amount || has_duration;
    if any && !(has_owner && has_amount && has_duration) {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "{context} must include owner, amount, and duration_blocks when providing lock hints"
            ),
        ));
    }
    ensure_zk_public_input_owner_canonical(map, context)?;
    Ok(())
}

fn reject_zk_public_input_key(
    map: &json::Map,
    key: &str,
    canonical: &str,
    context: &str,
) -> napi::Result<()> {
    if map.contains_key(key) {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must use {canonical} (unsupported key {key})"),
        ));
    }
    Ok(())
}

fn ensure_zk_public_input_owner_canonical(map: &json::Map, context: &str) -> napi::Result<()> {
    let Some(value) = map.get("owner") else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let owner = value.as_str().ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context}.owner must be a canonical I105 account id"),
        )
    })?;
    let canonical = AccountId::parse_encoded(owner)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map(|account| account.to_string())
        .map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context}.owner must be a canonical I105 account id"),
            )
        })?;
    if canonical != owner {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context}.owner must use canonical I105 account id form"),
        ));
    }
    Ok(())
}

fn canonicalize_hex32_public_input(
    map: &mut json::Map,
    key: &str,
    label: &str,
    context: &str,
) -> napi::Result<()> {
    let Some(value) = map.get_mut(key) else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let raw = value.as_str().ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context}.{label} must be 32-byte hex"),
        )
    })?;
    let canonical = canonicalize_hex32_value(raw).ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context}.{label} must be 32-byte hex"),
        )
    })?;
    *value = json::Value::String(canonical);
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

fn zk_hint_present(map: &json::Map, key: &str) -> bool {
    map.get(key)
        .is_some_and(|value| !matches!(value, json::Value::Null))
}

fn parse_u64_value(value: json::Value, context: &str) -> napi::Result<u64> {
    match value {
        json::Value::Number(number) => number.as_u64().ok_or_else(|| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be an unsigned integer"),
            )
        }),
        json::Value::String(s) => s.parse::<u64>().map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be an unsigned integer string: {err}"),
            )
        }),
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be an unsigned integer (found {other:?})"),
        )),
    }
}

fn parse_u32_value(value: json::Value, context: &str) -> napi::Result<u32> {
    let parsed = parse_u64_value(value, context)?;
    u32::try_from(parsed).map_err(|_| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must fit into u32"),
        )
    })
}

fn remove_case_insensitive(map: &mut json::Map, key: &str) -> Option<json::Value> {
    map.remove(key)
        .or_else(|| map.remove(&key.to_ascii_lowercase()))
        .or_else(|| map.remove(&key.to_ascii_uppercase()))
}

fn parse_u8_value(value: json::Value, context: &str) -> napi::Result<u8> {
    let parsed = parse_u64_value(value, context)?;
    u8::try_from(parsed).map_err(|_| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must fit into u8"),
        )
    })
}

fn parse_u128_value(value: json::Value, context: &str) -> napi::Result<u128> {
    match value {
        json::Value::Number(number) => number.as_u64().map(u128::from).ok_or_else(|| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be an unsigned integer"),
            )
        }),
        json::Value::String(s) => s.parse::<u128>().map_err(|err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("{context} must be an unsigned integer string: {err}"),
            )
        }),
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be an unsigned integer (found {other:?})"),
        )),
    }
}

fn parse_optional_voting_mode(
    value: Option<json::Value>,
    context: &str,
) -> napi::Result<Option<VotingMode>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::String(label)) => {
            let mode = match label.trim() {
                "Zk" | "zk" | "ZK" => VotingMode::Zk,
                "Plain" | "plain" | "PLAIN" => VotingMode::Plain,
                other => {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("{context}.mode must be one of: Zk, Plain (found {other})"),
                    ));
                }
            };
            Ok(Some(mode))
        }
        Some(other) => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context}.mode must be a string (found {other:?})"),
        )),
    }
}

fn parse_council_derivation_kind(
    value: json::Value,
    context: &str,
) -> napi::Result<CouncilDerivationKind> {
    let label = parse_string_value(value, context)?;
    match label.as_str() {
        "Vrf" | "vrf" | "VRF" => Ok(CouncilDerivationKind::Vrf),
        "Fallback" | "fallback" | "FALLBACK" => Ok(CouncilDerivationKind::Fallback),
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be either \"Vrf\" or \"Fallback\" (found {other})"),
        )),
    }
}

fn council_derivation_to_json(kind: CouncilDerivationKind) -> json::Value {
    let label = match kind {
        CouncilDerivationKind::Vrf => "Vrf",
        CouncilDerivationKind::Fallback => "Fallback",
    };
    json::Value::String(label.to_owned())
}

fn voting_mode_to_json(mode: VotingMode) -> &'static str {
    match mode {
        VotingMode::Zk => "Zk",
        VotingMode::Plain => "Plain",
    }
}

fn instruction_from_json(payload: &str) -> napi::Result<InstructionBox> {
    let value: json::Value = json::from_json(payload).map_err(norito_to_napi)?;
    value_to_instruction(value)
}

fn parse_instruction_payloads(payloads: Vec<String>) -> napi::Result<Vec<InstructionBox>> {
    if payloads.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "instructions must be a non-empty array",
        ));
    }
    let mut instructions = Vec::with_capacity(payloads.len());
    for payload in payloads {
        let instruction = instruction_from_json(&payload)?;
        instructions.push(instruction);
    }
    Ok(instructions)
}

fn kagemusha_instruction_archive_from_json(value: json::Value) -> napi::Result<InstructionBox> {
    let mut map = match value {
        json::Value::Object(map) => map,
        other => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("KagemushaInstructionArchive payload must be an object (found {other:?})"),
            ));
        }
    };
    let type_value = remove_case_insensitive(&mut map, "type").ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "KagemushaInstructionArchive.type field missing",
        )
    })?;
    let instruction_type = type_value.as_str().ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "KagemushaInstructionArchive.type must be a string",
        )
    })?;
    let bytes_value = remove_case_insensitive(&mut map, "bytes_base64")
        .or_else(|| remove_case_insensitive(&mut map, "bytesBase64"))
        .ok_or_else(|| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "KagemushaInstructionArchive.bytes_base64 field missing",
            )
        })?;
    let bytes_base64 = bytes_value.as_str().ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "KagemushaInstructionArchive.bytes_base64 must be a string",
        )
    })?;
    if !map.is_empty() {
        let mut keys = map.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "KagemushaInstructionArchive contains unexpected field(s): {}",
                keys.join(", ")
            ),
        ));
    }
    let archive = STANDARD.decode(bytes_base64.as_bytes()).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid KagemushaInstructionArchive.bytes_base64: {err}"),
        )
    })?;
    if STANDARD.encode(&archive) != bytes_base64 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "KagemushaInstructionArchive.bytes_base64 must be canonical standard base64",
        ));
    }
    ensure_kagemusha_recursive_archive_len(archive.len(), "Kagemusha instruction archive")?;
    match instruction_type {
        "KagemushaTransfer" => {
            let instruction: iroha_data_model::isi::offline::KagemushaTransfer =
                decode_from_bytes(&archive).map_err(|err| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("invalid KagemushaTransfer instruction archive: {err}"),
                    )
                })?;
            Ok(InstructionBox::from(instruction))
        }
        "RedeemKagemushaRecursive" => {
            let instruction: iroha_data_model::isi::offline::RedeemKagemushaRecursive =
                decode_from_bytes(&archive).map_err(|err| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("invalid RedeemKagemushaRecursive instruction archive: {err}"),
                    )
                })?;
            Ok(InstructionBox::from(instruction))
        }
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "unsupported KagemushaInstructionArchive.type `{other}`; expected KagemushaTransfer or RedeemKagemushaRecursive"
            ),
        )),
    }
}

fn encode_trigger_action(action: &Action) -> napi::Result<String> {
    norito::to_bytes(action)
        .map(|bytes| STANDARD.encode(bytes))
        .map_err(norito_to_napi)
}

fn parse_metadata_payload(context: &str, payload: Option<String>) -> napi::Result<Metadata> {
    payload.map_or_else(
        || Ok(Metadata::default()),
        |raw| {
            json::from_json(&raw).map_err(|err| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("invalid {context} metadata json: {err}"),
                )
            })
        },
    )
}

#[allow(clippy::too_many_lines)] // comprehensive translation keeps instruction handling centralized
fn value_to_instruction(value: json::Value) -> napi::Result<InstructionBox> {
    if let Ok(instruction) = json::from_value::<InstructionBox>(value.clone()) {
        return Ok(instruction);
    }
    match value {
        json::Value::Object(mut map) => {
            if let Some(kagemusha_value) =
                remove_case_insensitive(&mut map, "KagemushaInstructionArchive")
            {
                return kagemusha_instruction_archive_from_json(kagemusha_value);
            }

            if let Some(json::Value::Object(mut register_map)) = map.remove("Register") {
                if let Some(domain_value) = register_map.remove("Domain") {
                    let new_domain: NewDomain =
                        json::from_value(domain_value).map_err(norito_to_napi)?;
                    let register_box = RegisterBox::Domain(Register::<Domain>::domain(new_domain));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(account_value) = register_map.remove("Account") {
                    let new_account: NewAccount =
                        json::from_value(account_value).map_err(norito_to_napi)?;
                    let register_box =
                        RegisterBox::Account(Register::<Account>::account(new_account));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(asset_value) = register_map.remove("AssetDefinition") {
                    let new_asset: NewAssetDefinition =
                        json::from_value(asset_value).map_err(norito_to_napi)?;
                    let register_box = RegisterBox::AssetDefinition(
                        Register::<AssetDefinition>::asset_definition(new_asset),
                    );
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(nft_value) = register_map.remove("Nft") {
                    let new_nft: NewNft = json::from_value(nft_value).map_err(norito_to_napi)?;
                    let register_box = RegisterBox::Nft(Register::<Nft>::nft(new_nft));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(role_value) = register_map.remove("Role") {
                    let new_role: NewRole = json::from_value(role_value).map_err(norito_to_napi)?;
                    let register_box = RegisterBox::Role(Register::<Role>::role(new_role));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(trigger_value) = register_map.remove("Trigger") {
                    let trigger: Trigger =
                        json::from_value(trigger_value).map_err(norito_to_napi)?;
                    let register_box = RegisterBox::Trigger(Register::<Trigger>::trigger(trigger));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(peer_value) = register_map.remove("Peer") {
                    let peer_registration: RegisterPeerWithPop =
                        json::from_value(peer_value).map_err(norito_to_napi)?;
                    let register_box = RegisterBox::Peer(peer_registration);
                    return Ok(InstructionBox::from(register_box));
                }
            }
            if let Some(json::Value::Object(mut mint_map)) = map.remove("Mint") {
                if let Some(json::Value::Object(mut asset_fields)) = mint_map.remove("Asset") {
                    let quantity_value = asset_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Mint.Asset.object field missing",
                        )
                    })?;
                    let destination_value =
                        asset_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Mint.Asset.destination field missing",
                            )
                        })?;
                    let quantity: Numeric =
                        json::from_value(quantity_value).map_err(norito_to_napi)?;
                    let destination: AssetId =
                        json::from_value(destination_value).map_err(norito_to_napi)?;
                    let mint = Mint::asset_numeric(quantity, destination);
                    return Ok(InstructionBox::from(MintBox::Asset(mint)));
                }
                if let Some(json::Value::Object(mut trigger_fields)) =
                    mint_map.remove("TriggerRepetitions")
                {
                    let repetitions_value = trigger_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Mint.TriggerRepetitions.object field missing",
                        )
                    })?;
                    let destination_value =
                        trigger_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Mint.TriggerRepetitions.destination field missing",
                            )
                        })?;
                    let repetitions: u32 =
                        json::from_value(repetitions_value).map_err(norito_to_napi)?;
                    let trigger_id: TriggerId =
                        json::from_value(destination_value).map_err(norito_to_napi)?;
                    let mint = Mint::trigger_repetitions(repetitions, trigger_id);
                    return Ok(InstructionBox::from(MintBox::TriggerRepetitions(mint)));
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported Mint instruction variant; expected keys: Asset or TriggerRepetitions",
                ));
            }
            if let Some(json::Value::Object(mut unregister_map)) = map.remove("Unregister") {
                if let Some(peer_value) = unregister_map.remove("Peer") {
                    let peer_id: PeerId = json::from_value(peer_value).map_err(norito_to_napi)?;
                    let unregister_box = UnregisterBox::Peer(Unregister::<Peer>::peer(peer_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(domain_value) = unregister_map.remove("Domain") {
                    let domain_id: DomainId =
                        json::from_value(domain_value).map_err(norito_to_napi)?;
                    let unregister_box =
                        UnregisterBox::Domain(Unregister::<Domain>::domain(domain_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(account_value) = unregister_map.remove("Account") {
                    let account_id = parse_account_id_value(account_value, "Unregister.Account")?;
                    let unregister_box =
                        UnregisterBox::Account(Unregister::<Account>::account(account_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(asset_value) = unregister_map.remove("AssetDefinition") {
                    let definition_id: AssetDefinitionId =
                        json::from_value(asset_value).map_err(norito_to_napi)?;
                    let unregister_box =
                        UnregisterBox::AssetDefinition(
                            Unregister::<AssetDefinition>::asset_definition(definition_id),
                        );
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(nft_value) = unregister_map.remove("Nft") {
                    let nft_id: NftId = json::from_value(nft_value).map_err(norito_to_napi)?;
                    let unregister_box = UnregisterBox::Nft(Unregister::<Nft>::nft(nft_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(role_value) = unregister_map.remove("Role") {
                    let role_id: RoleId = json::from_value(role_value).map_err(norito_to_napi)?;
                    let unregister_box = UnregisterBox::Role(Unregister::<Role>::role(role_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(trigger_value) = unregister_map.remove("Trigger") {
                    let trigger_id: TriggerId =
                        json::from_value(trigger_value).map_err(norito_to_napi)?;
                    let unregister_box =
                        UnregisterBox::Trigger(Unregister::<Trigger>::trigger(trigger_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported Unregister instruction variant; expected keys: Peer, Domain, Account, AssetDefinition, Nft, Role, Trigger",
                ));
            }
            if let Some(register_sns_value) = remove_case_insensitive(&mut map, "RegisterSnsName") {
                let request: RegisterNameRequestV1 =
                    json::from_value(register_sns_value).map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RegisterSnsName::new(request)));
            }
            if let Some(json::Value::Object(mut burn_map)) = map.remove("Burn") {
                if let Some(json::Value::Object(mut asset_fields)) = burn_map.remove("Asset") {
                    let quantity_value = asset_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Burn.Asset.object field missing",
                        )
                    })?;
                    let destination_value =
                        asset_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Burn.Asset.destination field missing",
                            )
                        })?;
                    let quantity: Numeric =
                        json::from_value(quantity_value).map_err(norito_to_napi)?;
                    let asset_id: AssetId =
                        json::from_value(destination_value).map_err(norito_to_napi)?;
                    let burn = Burn::asset_numeric(quantity, asset_id);
                    return Ok(InstructionBox::from(BurnBox::Asset(burn)));
                }
                if let Some(json::Value::Object(mut trigger_fields)) =
                    burn_map.remove("TriggerRepetitions")
                {
                    let repetitions_value = trigger_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Burn.TriggerRepetitions.object field missing",
                        )
                    })?;
                    let destination_value =
                        trigger_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Burn.TriggerRepetitions.destination field missing",
                            )
                        })?;
                    let repetitions: u32 =
                        json::from_value(repetitions_value).map_err(norito_to_napi)?;
                    let trigger_id: TriggerId =
                        json::from_value(destination_value).map_err(norito_to_napi)?;
                    let burn = Burn::trigger_repetitions(repetitions, trigger_id);
                    return Ok(InstructionBox::from(BurnBox::TriggerRepetitions(burn)));
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported Burn instruction variant; expected keys: Asset or TriggerRepetitions",
                ));
            }
            if let Some(json::Value::Object(mut execute_fields)) = map.remove("ExecuteTrigger") {
                let trigger: TriggerId = json::from_value(required_value(
                    &mut execute_fields,
                    "trigger",
                    "ExecuteTrigger",
                )?)
                .map_err(norito_to_napi)?;
                let args = execute_fields
                    .remove("args")
                    .map(Json::from)
                    .unwrap_or_default();
                return Ok(InstructionBox::from(ExecuteTrigger { trigger, args }));
            }
            if let Some(json::Value::Object(mut transfer_map)) = map.remove("Transfer") {
                if let Some(json::Value::Object(mut asset_fields)) = transfer_map.remove("Asset") {
                    let source_value = asset_fields.remove("source").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Asset.source field missing",
                        )
                    })?;
                    let quantity_value = asset_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Asset.object field missing",
                        )
                    })?;
                    let destination_value =
                        asset_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Transfer.Asset.destination field missing",
                            )
                        })?;
                    let source: AssetId = json::from_value(source_value).map_err(norito_to_napi)?;
                    let quantity: Numeric =
                        json::from_value(quantity_value).map_err(norito_to_napi)?;
                    let destination =
                        parse_account_id_value(destination_value, "Transfer.Asset.destination")?;
                    let transfer = Transfer::asset_numeric(source, quantity, destination);
                    return Ok(InstructionBox::from(TransferBox::Asset(transfer)));
                }
                if let Some(json::Value::Object(mut domain_fields)) = transfer_map.remove("Domain")
                {
                    let source_value = domain_fields.remove("source").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Domain.source field missing",
                        )
                    })?;
                    let object_value = domain_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Domain.object field missing",
                        )
                    })?;
                    let destination_value =
                        domain_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Transfer.Domain.destination field missing",
                            )
                        })?;
                    let source = parse_account_id_value(source_value, "Transfer.Domain.source")?;
                    let domain_id: DomainId =
                        json::from_value(object_value).map_err(norito_to_napi)?;
                    let destination =
                        parse_account_id_value(destination_value, "Transfer.Domain.destination")?;
                    let transfer = Transfer::domain(source, domain_id, destination);
                    return Ok(InstructionBox::from(TransferBox::Domain(transfer)));
                }
                if let Some(json::Value::Object(mut definition_fields)) =
                    transfer_map.remove("AssetDefinition")
                {
                    let source_value = definition_fields.remove("source").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.AssetDefinition.source field missing",
                        )
                    })?;
                    let object_value = definition_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.AssetDefinition.object field missing",
                        )
                    })?;
                    let destination_value =
                        definition_fields.remove("destination").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "Transfer.AssetDefinition.destination field missing",
                            )
                        })?;
                    let source =
                        parse_account_id_value(source_value, "Transfer.AssetDefinition.source")?;
                    let definition: AssetDefinitionId =
                        json::from_value(object_value).map_err(norito_to_napi)?;
                    let destination = parse_account_id_value(
                        destination_value,
                        "Transfer.AssetDefinition.destination",
                    )?;
                    let transfer = Transfer::asset_definition(source, definition, destination);
                    return Ok(InstructionBox::from(TransferBox::AssetDefinition(transfer)));
                }
                if let Some(json::Value::Object(mut nft_fields)) = transfer_map.remove("Nft") {
                    let source_value = nft_fields.remove("source").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Nft.source field missing",
                        )
                    })?;
                    let object_value = nft_fields.remove("object").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Nft.object field missing",
                        )
                    })?;
                    let destination_value = nft_fields.remove("destination").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "Transfer.Nft.destination field missing",
                        )
                    })?;
                    let source = parse_account_id_value(source_value, "Transfer.Nft.source")?;
                    let nft_id: NftId = json::from_value(object_value).map_err(norito_to_napi)?;
                    let destination =
                        parse_account_id_value(destination_value, "Transfer.Nft.destination")?;
                    let transfer = Transfer::nft(source, nft_id, destination);
                    return Ok(InstructionBox::from(TransferBox::Nft(transfer)));
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported Transfer instruction variant; expected keys: Asset, Domain, AssetDefinition, or Nft",
                ));
            }
            if let Some(json::Value::Object(mut grant_map)) = map.remove("Grant") {
                if let Some(json::Value::Object(mut fields)) = grant_map.remove("Permission") {
                    let object_value = required_value(&mut fields, "object", "Grant.Permission")?;
                    let destination = parse_account_id_value(
                        required_value(&mut fields, "destination", "Grant.Permission")?,
                        "Grant.Permission.destination",
                    )?;
                    if !fields.is_empty() {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "Grant.Permission contains unsupported fields: {}",
                                fields.keys().cloned().collect::<Vec<_>>().join(",")
                            ),
                        ));
                    }
                    let mut permission_fields = match object_value {
                        json::Value::Object(map) => map,
                        other => {
                            return Err(napi::Error::new(
                                napi::Status::InvalidArg,
                                format!(
                                    "Grant.Permission.object must be an object (found {other:?})"
                                ),
                            ));
                        }
                    };
                    permission_fields
                        .entry("payload".to_owned())
                        .or_insert(json::Value::Null);
                    let permission: Permission =
                        json::from_value(json::Value::Object(permission_fields))
                            .map_err(norito_to_napi)?;
                    let grant = Grant::account_permission(permission, destination);
                    return Ok(InstructionBox::from(GrantBox::Permission(grant)));
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported Grant instruction variant; expected key: Permission",
                ));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SetAssetDefinitionAlias") {
                let asset_definition_id: AssetDefinitionId = parse_string_value(
                    required_value(
                        &mut fields,
                        "asset_definition_id",
                        "SetAssetDefinitionAlias",
                    )?,
                    "SetAssetDefinitionAlias.asset_definition_id",
                )?
                .parse()
                .map_err(|err| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!(
                            "invalid SetAssetDefinitionAlias.asset_definition_id literal: {err}"
                        ),
                    )
                })?;
                let alias = parse_optional_string_value(
                    fields.remove("alias"),
                    "SetAssetDefinitionAlias.alias",
                )?
                .map(|literal| {
                    literal.parse::<AssetDefinitionAlias>().map_err(|err| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("invalid SetAssetDefinitionAlias.alias literal: {err}"),
                        )
                    })
                })
                .transpose()?;
                let lease_expiry_ms = match fields.remove("lease_expiry_ms") {
                    None | Some(json::Value::Null) => None,
                    Some(value) => Some(parse_u64_value(
                        value,
                        "SetAssetDefinitionAlias.lease_expiry_ms",
                    )?),
                };
                if !fields.is_empty() {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        format!(
                            "SetAssetDefinitionAlias contains unsupported fields: {}",
                            fields.keys().cloned().collect::<Vec<_>>().join(",")
                        ),
                    ));
                }
                let instruction = match alias {
                    Some(alias) => {
                        SetAssetDefinitionAlias::bind(asset_definition_id, alias, lease_expiry_ms)
                    }
                    None => SetAssetDefinitionAlias::clear(asset_definition_id),
                };
                return Ok(InstructionBox::from(instruction));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterRwa") {
                let rwa_value = required_value(&mut fields, "rwa", "RegisterRwa")?;
                let json::Value::Object(mut fields) = rwa_value else {
                    return Err(napi::Error::new(
                        napi::Status::InvalidArg,
                        "RegisterRwa.rwa must be an object",
                    ));
                };
                let domain: DomainId =
                    json::from_value(required_value(&mut fields, "domain", "RegisterRwa.rwa")?)
                        .map_err(norito_to_napi)?;
                let quantity: Numeric =
                    json::from_value(required_value(&mut fields, "quantity", "RegisterRwa.rwa")?)
                        .map_err(norito_to_napi)?;
                let spec =
                    json::from_value(required_value(&mut fields, "spec", "RegisterRwa.rwa")?)
                        .map_err(norito_to_napi)?;
                let primary_reference = parse_string_value(
                    required_value(&mut fields, "primary_reference", "RegisterRwa.rwa")?,
                    "RegisterRwa.rwa.primary_reference",
                )?;
                let status: Option<Name> =
                    fields
                        .remove("status")
                        .map_or(Ok(None), |value| match value {
                            json::Value::Null => Ok(None),
                            other => json::from_value(other).map_err(norito_to_napi),
                        })?;
                let metadata = fields
                    .remove("metadata")
                    .map_or(Ok(Metadata::default()), |value| {
                        json::from_value(value).map_err(norito_to_napi)
                    })?;
                let parents = fields.remove("parents").map_or(Ok(Vec::new()), |value| {
                    parse_rwa_parent_refs_value(value, "RegisterRwa.rwa.parents")
                })?;
                let controls = fields
                    .remove("controls")
                    .map_or(Ok(RwaControlPolicy::default()), |value| {
                        json::from_value(value).map_err(norito_to_napi)
                    })?;
                let register = RegisterRwa {
                    rwa: NewRwa::new(
                        domain,
                        quantity,
                        spec,
                        primary_reference,
                        status,
                        metadata,
                        parents,
                        controls,
                    ),
                };
                return Ok(InstructionBox::from(RwaInstructionBox::from(register)));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("TransferRwa") {
                let source = parse_account_id_value(
                    required_value(&mut fields, "source", "TransferRwa")?,
                    "TransferRwa.source",
                )?;
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "TransferRwa")?,
                    "TransferRwa.rwa",
                )?;
                let quantity: Numeric =
                    json::from_value(required_value(&mut fields, "quantity", "TransferRwa")?)
                        .map_err(norito_to_napi)?;
                let destination = parse_account_id_value(
                    required_value(&mut fields, "destination", "TransferRwa")?,
                    "TransferRwa.destination",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(TransferRwa {
                    source,
                    rwa,
                    quantity,
                    destination,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("MergeRwas") {
                let parents = parse_rwa_parent_refs_value(
                    required_value(&mut fields, "parents", "MergeRwas")?,
                    "MergeRwas.parents",
                )?;
                let primary_reference = parse_string_value(
                    required_value(&mut fields, "primary_reference", "MergeRwas")?,
                    "MergeRwas.primary_reference",
                )?;
                let status: Option<Name> =
                    fields
                        .remove("status")
                        .map_or(Ok(None), |value| match value {
                            json::Value::Null => Ok(None),
                            other => json::from_value(other).map_err(norito_to_napi),
                        })?;
                let metadata = fields
                    .remove("metadata")
                    .map_or(Ok(Metadata::default()), |value| {
                        json::from_value(value).map_err(norito_to_napi)
                    })?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(MergeRwas {
                    parents,
                    primary_reference,
                    status,
                    metadata,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RedeemRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "RedeemRwa")?,
                    "RedeemRwa.rwa",
                )?;
                let quantity: Numeric =
                    json::from_value(required_value(&mut fields, "quantity", "RedeemRwa")?)
                        .map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(RedeemRwa {
                    rwa,
                    quantity,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("FreezeRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "FreezeRwa")?,
                    "FreezeRwa.rwa",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(FreezeRwa {
                    rwa,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("UnfreezeRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "UnfreezeRwa")?,
                    "UnfreezeRwa.rwa",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(UnfreezeRwa {
                    rwa,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("HoldRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "HoldRwa")?,
                    "HoldRwa.rwa",
                )?;
                let quantity: Numeric =
                    json::from_value(required_value(&mut fields, "quantity", "HoldRwa")?)
                        .map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(HoldRwa {
                    rwa,
                    quantity,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("ReleaseRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "ReleaseRwa")?,
                    "ReleaseRwa.rwa",
                )?;
                let quantity: Numeric =
                    json::from_value(required_value(&mut fields, "quantity", "ReleaseRwa")?)
                        .map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(ReleaseRwa {
                    rwa,
                    quantity,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("ForceTransferRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "ForceTransferRwa")?,
                    "ForceTransferRwa.rwa",
                )?;
                let quantity: Numeric =
                    json::from_value(required_value(&mut fields, "quantity", "ForceTransferRwa")?)
                        .map_err(norito_to_napi)?;
                let destination = parse_account_id_value(
                    required_value(&mut fields, "destination", "ForceTransferRwa")?,
                    "ForceTransferRwa.destination",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    ForceTransferRwa {
                        rwa,
                        quantity,
                        destination,
                    },
                )));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SetRwaControls") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "SetRwaControls")?,
                    "SetRwaControls.rwa",
                )?;
                let controls: RwaControlPolicy =
                    json::from_value(required_value(&mut fields, "controls", "SetRwaControls")?)
                        .map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    SetRwaControls { rwa, controls },
                )));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SetRwaKeyValue") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "SetRwaKeyValue")?,
                    "SetRwaKeyValue.rwa",
                )?;
                let key: Name =
                    json::from_value(required_value(&mut fields, "key", "SetRwaKeyValue")?)
                        .map_err(norito_to_napi)?;
                let value: Json =
                    json::from_value(required_value(&mut fields, "value", "SetRwaKeyValue")?)
                        .map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    SetKeyValue::rwa(rwa, key, value),
                )));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RemoveRwaKeyValue") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "RemoveRwaKeyValue")?,
                    "RemoveRwaKeyValue.rwa",
                )?;
                let key: Name =
                    json::from_value(required_value(&mut fields, "key", "RemoveRwaKeyValue")?)
                        .map_err(norito_to_napi)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    RemoveKeyValue::rwa(rwa, key),
                )));
            }
            if let Some(json::Value::Object(mut kaigi_map)) = map.remove("Kaigi") {
                if let Some(json::Value::Object(mut create_fields)) =
                    kaigi_map.remove("CreateKaigi")
                {
                    let call_value = create_fields.remove("call").ok_or_else(|| {
                        napi::Error::new(napi::Status::InvalidArg, "CreateKaigi.call field missing")
                    })?;
                    let call: NewKaigi = json::from_value(call_value).map_err(|err| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("CreateKaigi.call parse error: {err}"),
                        )
                    })?;
                    let commitment = parse_optional_commitment(
                        create_fields.remove("commitment"),
                        "CreateKaigi",
                    )?;
                    let nullifier =
                        parse_optional_nullifier(create_fields.remove("nullifier"), "CreateKaigi")?;
                    let roster_root = parse_optional_hash(
                        create_fields.remove("roster_root"),
                        "CreateKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(create_fields.remove("proof"), "CreateKaigi.proof")?;
                    let instruction = CreateKaigi {
                        call,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(json::Value::Object(mut join_fields)) = kaigi_map.remove("JoinKaigi") {
                    let call_id_value = join_fields.remove("call_id").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "JoinKaigi.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId =
                        json::from_value(call_id_value).map_err(norito_to_napi)?;
                    let participant_value = join_fields.remove("participant").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "JoinKaigi.participant field missing",
                        )
                    })?;
                    let participant =
                        parse_account_id_value(participant_value, "JoinKaigi.participant")?;
                    let commitment =
                        parse_optional_commitment(join_fields.remove("commitment"), "JoinKaigi")?;
                    let nullifier =
                        parse_optional_nullifier(join_fields.remove("nullifier"), "JoinKaigi")?;
                    let roster_root = parse_optional_hash(
                        join_fields.remove("roster_root"),
                        "JoinKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(join_fields.remove("proof"), "JoinKaigi.proof")?;
                    let join = JoinKaigi {
                        call_id,
                        participant,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(join).into_instruction_box());
                }
                if let Some(json::Value::Object(mut leave_fields)) = kaigi_map.remove("LeaveKaigi")
                {
                    let call_id_value = leave_fields.remove("call_id").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "LeaveKaigi.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId =
                        json::from_value(call_id_value).map_err(norito_to_napi)?;
                    let participant_value =
                        leave_fields.remove("participant").ok_or_else(|| {
                            napi::Error::new(
                                napi::Status::InvalidArg,
                                "LeaveKaigi.participant field missing",
                            )
                        })?;
                    let participant =
                        parse_account_id_value(participant_value, "LeaveKaigi.participant")?;
                    let commitment =
                        parse_optional_commitment(leave_fields.remove("commitment"), "LeaveKaigi")?;
                    let nullifier =
                        parse_optional_nullifier(leave_fields.remove("nullifier"), "LeaveKaigi")?;
                    let roster_root = parse_optional_hash(
                        leave_fields.remove("roster_root"),
                        "LeaveKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(leave_fields.remove("proof"), "LeaveKaigi.proof")?;
                    let leave = LeaveKaigi {
                        call_id,
                        participant,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(leave).into_instruction_box());
                }
                if let Some(json::Value::Object(mut end_fields)) = kaigi_map.remove("EndKaigi") {
                    let call_id_value = end_fields.remove("call_id").ok_or_else(|| {
                        napi::Error::new(napi::Status::InvalidArg, "EndKaigi.call_id field missing")
                    })?;
                    let call_id: KaigiId =
                        json::from_value(call_id_value).map_err(norito_to_napi)?;
                    let ended_at = match end_fields.remove("ended_at_ms") {
                        None | Some(json::Value::Null) => None,
                        Some(value) => Some(json::from_value(value).map_err(norito_to_napi)?),
                    };
                    let commitment =
                        parse_optional_commitment(end_fields.remove("commitment"), "EndKaigi")?;
                    let nullifier =
                        parse_optional_nullifier(end_fields.remove("nullifier"), "EndKaigi")?;
                    let roster_root = parse_optional_hash(
                        end_fields.remove("roster_root"),
                        "EndKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(end_fields.remove("proof"), "EndKaigi.proof")?;
                    let end = EndKaigi {
                        call_id,
                        ended_at_ms: ended_at,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(end).into_instruction_box());
                }
                if let Some(json::Value::Object(mut usage_fields)) =
                    kaigi_map.remove("RecordKaigiUsage")
                {
                    let call_id_value = usage_fields.remove("call_id").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "RecordKaigiUsage.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId =
                        json::from_value(call_id_value).map_err(norito_to_napi)?;
                    let duration_value = usage_fields.remove("duration_ms").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "RecordKaigiUsage.duration_ms field missing",
                        )
                    })?;
                    let duration_ms: u64 =
                        json::from_value(duration_value).map_err(norito_to_napi)?;
                    let billed_gas = usage_fields
                        .remove("billed_gas")
                        .map(|value| json::from_value(value).map_err(norito_to_napi))
                        .transpose()?
                        .unwrap_or_default();
                    let usage_commitment = parse_optional_hash(
                        usage_fields.remove("usage_commitment"),
                        "RecordKaigiUsage.usage_commitment",
                    )?;
                    let proof = parse_optional_base64(
                        usage_fields.remove("proof"),
                        "RecordKaigiUsage.proof",
                    )?;
                    let usage = RecordKaigiUsage {
                        call_id,
                        duration_ms,
                        billed_gas,
                        usage_commitment,
                        proof,
                    };
                    return Ok(Box::new(usage).into_instruction_box());
                }
                if let Some(json::Value::Object(mut manifest_fields)) =
                    kaigi_map.remove("SetKaigiRelayManifest")
                {
                    let call_id_value = manifest_fields.remove("call_id").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "SetKaigiRelayManifest.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId =
                        json::from_value(call_id_value).map_err(norito_to_napi)?;
                    let relay_manifest =
                        manifest_fields
                            .remove("relay_manifest")
                            .map_or(Ok(None), |value| match value {
                                json::Value::Null => Ok(None),
                                other => json::from_value(other).map(Some).map_err(norito_to_napi),
                            })?;
                    let manifest = SetKaigiRelayManifest {
                        call_id,
                        relay_manifest,
                    };
                    return Ok(Box::new(manifest).into_instruction_box());
                }
                if let Some(json::Value::Object(mut register_fields)) =
                    kaigi_map.remove("RegisterKaigiRelay")
                {
                    let relay_value = register_fields.remove("relay").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "RegisterKaigiRelay.relay field missing",
                        )
                    })?;
                    let relay: KaigiRelayRegistration =
                        json::from_value(relay_value).map_err(norito_to_napi)?;
                    let registration = RegisterKaigiRelay { relay };
                    return Ok(Box::new(registration).into_instruction_box());
                }
                if let Some(json::Value::Object(mut health_fields)) =
                    kaigi_map.remove("ReportKaigiRelayHealth")
                {
                    let call_id_value = health_fields.remove("call_id").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "ReportKaigiRelayHealth.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId =
                        json::from_value(call_id_value).map_err(norito_to_napi)?;
                    let relay_id_value = health_fields.remove("relay_id").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "ReportKaigiRelayHealth.relay_id field missing",
                        )
                    })?;
                    let relay_id =
                        parse_account_id_value(relay_id_value, "ReportKaigiRelayHealth.relay_id")?;
                    let status_value = health_fields.remove("status").ok_or_else(|| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            "ReportKaigiRelayHealth.status field missing",
                        )
                    })?;
                    let status: KaigiRelayHealthStatus =
                        json::from_value(status_value).map_err(norito_to_napi)?;
                    let reported_at_ms = health_fields
                        .remove("reported_at_ms")
                        .map_or(Ok(0_u64), |value| {
                            json::from_value(value).map_err(norito_to_napi)
                        })?;
                    let notes =
                        health_fields
                            .remove("notes")
                            .map_or(Ok(None), |value| match value {
                                json::Value::Null => Ok(None),
                                other => json::from_value(other).map(Some).map_err(norito_to_napi),
                            })?;
                    let report = ReportKaigiRelayHealth {
                        call_id,
                        relay_id,
                        status,
                        reported_at_ms,
                        notes,
                    };
                    return Ok(Box::new(report).into_instruction_box());
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported Kaigi instruction variant; see iroha_data_model::isi::kaigi for supported set",
                ));
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("ProposeDeployContract") {
                let contract_address: iroha_data_model::smart_contract::ContractAddress =
                    parse_string_value(
                        required_value(&mut fields, "contract_address", "ProposeDeployContract")?,
                        "ProposeDeployContract.contract_address",
                    )?
                    .parse()
                    .map_err(|err| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "invalid ProposeDeployContract.contract_address literal: {err}"
                            ),
                        )
                    })?;
                let code_hash_hex = parse_string_value(
                    required_value(&mut fields, "code_hash_hex", "ProposeDeployContract")?,
                    "ProposeDeployContract.code_hash_hex",
                )?;
                let abi_hash_hex = parse_string_value(
                    required_value(&mut fields, "abi_hash_hex", "ProposeDeployContract")?,
                    "ProposeDeployContract.abi_hash_hex",
                )?;
                let abi_version = parse_string_value(
                    required_value(&mut fields, "abi_version", "ProposeDeployContract")?,
                    "ProposeDeployContract.abi_version",
                )?;
                let window = match fields.remove("window") {
                    None | Some(json::Value::Null) => None,
                    Some(value) => Some(json::from_value(value).map_err(norito_to_napi)?),
                };
                let mode =
                    parse_optional_voting_mode(fields.remove("mode"), "ProposeDeployContract")?;
                let manifest_provenance = match fields.remove("manifest_provenance") {
                    None | Some(json::Value::Null) => None,
                    Some(value) => Some(json::from_value(value).map_err(norito_to_napi)?),
                };
                let instruction = ProposeDeployContract {
                    contract_address,
                    code_hash_hex,
                    abi_hash_hex,
                    abi_version,
                    window,
                    mode,
                    manifest_provenance,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("CastZkBallot") {
                let election_id = parse_string_value(
                    required_value(&mut fields, "election_id", "CastZkBallot")?,
                    "CastZkBallot.election_id",
                )?;
                let proof_b64 = parse_string_value(
                    required_value(&mut fields, "proof_b64", "CastZkBallot")?,
                    "CastZkBallot.proof_b64",
                )?;
                let public_inputs_json = parse_string_value(
                    required_value(&mut fields, "public_inputs_json", "CastZkBallot")?,
                    "CastZkBallot.public_inputs_json",
                )?;
                let public_inputs_json = normalize_zk_ballot_public_inputs_json(
                    public_inputs_json.as_str(),
                    "CastZkBallot.public_inputs_json",
                )?;
                let ballot = CastZkBallot {
                    election_id,
                    proof_b64,
                    public_inputs_json,
                };
                return Ok(Box::new(ballot).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("CastPlainBallot") {
                let referendum_id = parse_string_value(
                    required_value(&mut fields, "referendum_id", "CastPlainBallot")?,
                    "CastPlainBallot.referendum_id",
                )?;
                let owner_value = required_value(&mut fields, "owner", "CastPlainBallot")?;
                let owner = parse_account_id_value(owner_value, "CastPlainBallot.owner")?;
                let amount = parse_u128_value(
                    required_value(&mut fields, "amount", "CastPlainBallot")?,
                    "CastPlainBallot.amount",
                )?;
                let duration_blocks = parse_u64_value(
                    required_value(&mut fields, "duration_blocks", "CastPlainBallot")?,
                    "CastPlainBallot.duration_blocks",
                )?;
                let direction = parse_u8_value(
                    required_value(&mut fields, "direction", "CastPlainBallot")?,
                    "CastPlainBallot.direction",
                )?;
                let ballot = CastPlainBallot {
                    referendum_id,
                    owner,
                    amount,
                    duration_blocks,
                    direction,
                };
                return Ok(Box::new(ballot).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterCitizen") {
                let owner_value = required_value(&mut fields, "owner", "RegisterCitizen")?;
                let owner = parse_account_id_value(owner_value, "RegisterCitizen.owner")?;
                let amount = parse_u128_value(
                    required_value(&mut fields, "amount", "RegisterCitizen")?,
                    "RegisterCitizen.amount",
                )?;
                let instruction = RegisterCitizen { owner, amount };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(enact_value) = map.remove("EnactReferendum") {
                let enact: EnactReferendum =
                    json::from_value(enact_value).map_err(norito_to_napi)?;
                return Ok(Box::new(enact).into_instruction_box());
            }

            if let Some(finalize_value) = map.remove("FinalizeReferendum") {
                let finalize: FinalizeReferendum =
                    json::from_value(finalize_value).map_err(norito_to_napi)?;
                return Ok(Box::new(finalize).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("PersistCouncilForEpoch") {
                let epoch = parse_u64_value(
                    required_value(&mut fields, "epoch", "PersistCouncilForEpoch")?,
                    "PersistCouncilForEpoch.epoch",
                )?;
                let members_value =
                    required_value(&mut fields, "members", "PersistCouncilForEpoch")?;
                let members: Vec<AccountId> =
                    json::from_value(members_value).map_err(norito_to_napi)?;
                let alternates_value = fields
                    .remove("alternates")
                    .unwrap_or_else(|| json::Value::Array(Vec::new()));
                let alternates: Vec<AccountId> =
                    json::from_value(alternates_value).map_err(norito_to_napi)?;
                let verified = parse_u32_value(
                    fields
                        .remove("verified")
                        .unwrap_or_else(|| json::Value::Number(json::Number::from(0u64))),
                    "PersistCouncilForEpoch.verified",
                )?;
                let candidates_count = parse_u32_value(
                    required_value(&mut fields, "candidates_count", "PersistCouncilForEpoch")?,
                    "PersistCouncilForEpoch.candidates_count",
                )?;
                let derived_by_value =
                    required_value(&mut fields, "derived_by", "PersistCouncilForEpoch")?;
                let derived_by = parse_council_derivation_kind(
                    derived_by_value,
                    "PersistCouncilForEpoch.derived_by",
                )?;
                let persist = PersistCouncilForEpoch {
                    epoch,
                    members,
                    alternates,
                    verified,
                    candidates_count,
                    derived_by,
                };
                return Ok(Box::new(persist).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("SubmitAgendaProposal") {
                let proposal: AgendaProposalV1 = json::from_value(required_value(
                    &mut fields,
                    "proposal",
                    "SubmitAgendaProposal",
                )?)
                .map_err(norito_to_napi)?;
                let instruction = SubmitAgendaProposal { proposal };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterSmartContractCode") {
                let manifest_value =
                    required_value(&mut fields, "manifest", "RegisterSmartContractCode")?;
                let manifest: ContractManifest =
                    json::from_value(manifest_value).map_err(norito_to_napi)?;
                let instruction = RegisterSmartContractCode { manifest };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterSmartContractBytes")
            {
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "RegisterSmartContractBytes")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "RegisterSmartContractBytes.code_hash")?;
                let code_value = required_value(&mut fields, "code", "RegisterSmartContractBytes")?;
                let code = parse_base64(code_value, "RegisterSmartContractBytes.code")?;
                let instruction = RegisterSmartContractBytes { code_hash, code };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("RemoveSmartContractBytes") {
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "RemoveSmartContractBytes")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "RemoveSmartContractBytes.code_hash")?;
                let reason = parse_optional_string_value(
                    fields.remove("reason"),
                    "RemoveSmartContractBytes.reason",
                )?;
                let instruction = RemoveSmartContractBytes { code_hash, reason };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("ActivateContractInstance") {
                let contract_address: iroha_data_model::smart_contract::ContractAddress =
                    parse_string_value(
                        required_value(
                            &mut fields,
                            "contract_address",
                            "ActivateContractInstance",
                        )?,
                        "ActivateContractInstance.contract_address",
                    )?
                    .parse()
                    .map_err(|err| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "invalid ActivateContractInstance.contract_address literal: {err}"
                            ),
                        )
                    })?;
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "ActivateContractInstance")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "ActivateContractInstance.code_hash")?;
                let instruction = ActivateContractInstance {
                    contract_address,
                    code_hash,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("DeactivateContractInstance")
            {
                let contract_address: iroha_data_model::smart_contract::ContractAddress =
                    parse_string_value(
                        required_value(
                            &mut fields,
                            "contract_address",
                            "DeactivateContractInstance",
                        )?,
                        "DeactivateContractInstance.contract_address",
                    )?
                    .parse()
                    .map_err(|err| {
                        napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "invalid DeactivateContractInstance.contract_address literal: {err}"
                            ),
                        )
                    })?;
                let reason = parse_optional_string_value(
                    fields.remove("reason"),
                    "DeactivateContractInstance.reason",
                )?;
                let instruction = DeactivateContractInstance {
                    contract_address,
                    reason,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(value) = map.remove("ClaimTwitterFollowReward") {
                let mut fields = match value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "ClaimTwitterFollowReward payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                let binding_hash = parse_keyed_hash(
                    required_value(&mut fields, "binding_hash", "ClaimTwitterFollowReward")?,
                    "ClaimTwitterFollowReward.binding_hash",
                )?;
                let instruction = ClaimTwitterFollowReward { binding_hash };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(value) = map.remove("SendToTwitter") {
                let mut fields = match value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("SendToTwitter payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let binding_hash = parse_keyed_hash(
                    required_value(&mut fields, "binding_hash", "SendToTwitter")?,
                    "SendToTwitter.binding_hash",
                )?;
                let amount: Numeric =
                    json::from_value(required_value(&mut fields, "amount", "SendToTwitter")?)
                        .map_err(norito_to_napi)?;
                let instruction = SendToTwitter {
                    binding_hash,
                    amount,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(value) = map.remove("CancelTwitterEscrow") {
                let mut fields = match value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "CancelTwitterEscrow payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                let binding_hash = parse_keyed_hash(
                    required_value(&mut fields, "binding_hash", "CancelTwitterEscrow")?,
                    "CancelTwitterEscrow.binding_hash",
                )?;
                let instruction = CancelTwitterEscrow { binding_hash };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(custom_value) = remove_case_insensitive(&mut map, "Custom") {
                let mut custom_map = match custom_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "Custom instruction payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                let payload =
                    remove_case_insensitive(&mut custom_map, "payload").ok_or_else(|| {
                        napi::Error::new(napi::Status::InvalidArg, "Custom.payload field missing")
                    })?;
                return Ok(InstructionBox::from(CustomInstruction::new(payload)));
            }

            if let Some(multisig_value) = remove_case_insensitive(&mut map, "Multisig") {
                let multisig_map = match multisig_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!(
                                "Multisig instruction payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(multisig_map),
                )));
            }

            if let Some(propose_value) = remove_case_insensitive(&mut map, "MultisigPropose") {
                let propose_fields = match propose_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("MultisigPropose payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Propose".to_owned(), json::Value::Object(propose_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }

            if let Some(approve_value) = remove_case_insensitive(&mut map, "MultisigApprove") {
                let approve_fields = match approve_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("MultisigApprove payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Approve".to_owned(), json::Value::Object(approve_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }

            if let Some(cancel_value) = remove_case_insensitive(&mut map, "MultisigCancel") {
                let cancel_fields = match cancel_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("MultisigCancel payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Cancel".to_owned(), json::Value::Object(cancel_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }

            if let Some(register_value) = remove_case_insensitive(&mut map, "MultisigRegister") {
                let register_fields = match register_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("MultisigRegister payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Register".to_owned(), json::Value::Object(register_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }

            if let Some(zk_value) = remove_case_insensitive(&mut map, "Zk") {
                let mut zk_map = match zk_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(napi::Error::new(
                            napi::Status::InvalidArg,
                            format!("Zk instruction payload must be an object (found {other:?})"),
                        ));
                    }
                };
                if let Some(payload) = zk_map.remove("RegisterZkAsset") {
                    let instruction: RegisterZkAsset =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("ScheduleConfidentialPolicyTransition") {
                    let instruction: ScheduleConfidentialPolicyTransition =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("CancelConfidentialPolicyTransition") {
                    let instruction: CancelConfidentialPolicyTransition =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("Shield") {
                    let instruction: Shield = json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("ZkTransfer") {
                    let instruction: ZkTransfer =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("Unshield") {
                    let instruction: Unshield =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("CreateElection") {
                    let instruction: CreateElection =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("SubmitBallot") {
                    let instruction: SubmitBallot =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("FinalizeElection") {
                    let instruction: FinalizeElection =
                        json::from_value(payload).map_err(norito_to_napi)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                return Err(napi::Error::new(
                    napi::Status::InvalidArg,
                    "unsupported zk instruction variant",
                ));
            }

            Err(napi::Error::new(
                napi::Status::InvalidArg,
                "unsupported instruction; refer to Iroha data model instructions for supported variants",
            ))
        }
        _ => Err(napi::Error::new(
            napi::Status::InvalidArg,
            "instruction JSON must be an object",
        )),
    }
}

#[allow(clippy::too_many_lines)] // mirrors `value_to_instruction` for full roundtrips
fn instruction_to_json_value(instruction: &InstructionBox) -> napi::Result<json::Value> {
    let instruction_ref: &dyn InstructionTrait = &**instruction;
    if let Some(register_box) = instruction_ref.as_any().downcast_ref::<RegisterBox>() {
        let mut register_map = json::Map::new();
        match register_box {
            RegisterBox::Domain(register) => {
                let inner = json::to_value(register.object()).map_err(norito_to_napi)?;
                register_map.insert("Domain".to_owned(), inner);
            }
            RegisterBox::Account(register) => {
                let inner = json::to_value(register.object()).map_err(norito_to_napi)?;
                register_map.insert("Account".to_owned(), inner);
            }
            RegisterBox::AssetDefinition(register) => {
                let inner = json::to_value(register.object()).map_err(norito_to_napi)?;
                register_map.insert("AssetDefinition".to_owned(), inner);
            }
            RegisterBox::Nft(register) => {
                let inner = json::to_value(register.object()).map_err(norito_to_napi)?;
                register_map.insert("Nft".to_owned(), inner);
            }
            RegisterBox::Role(register) => {
                let inner = json::to_value(register.object()).map_err(norito_to_napi)?;
                register_map.insert("Role".to_owned(), inner);
            }
            RegisterBox::Trigger(register) => {
                let inner = json::to_value(register.object()).map_err(norito_to_napi)?;
                register_map.insert("Trigger".to_owned(), inner);
            }
            RegisterBox::Peer(register) => {
                let inner = json::to_value(register).map_err(norito_to_napi)?;
                register_map.insert("Peer".to_owned(), inner);
            }
        }
        if !register_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Register".to_owned(), json::Value::Object(register_map));
            return Ok(json::Value::Object(outer));
        }
    }

    if let Some(unregister_box) = instruction_ref.as_any().downcast_ref::<UnregisterBox>() {
        let mut unregister_map = json::Map::new();
        match unregister_box {
            UnregisterBox::Peer(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("Peer".to_owned(), inner);
            }
            UnregisterBox::Domain(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("Domain".to_owned(), inner);
            }
            UnregisterBox::Account(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("Account".to_owned(), inner);
            }
            UnregisterBox::AssetDefinition(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("AssetDefinition".to_owned(), inner);
            }
            UnregisterBox::Nft(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("Nft".to_owned(), inner);
            }
            UnregisterBox::Role(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("Role".to_owned(), inner);
            }
            UnregisterBox::Trigger(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(norito_to_napi)?;
                unregister_map.insert("Trigger".to_owned(), inner);
            }
        }
        if !unregister_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Unregister".to_owned(), json::Value::Object(unregister_map));
            return Ok(json::Value::Object(outer));
        }
    }

    if let Some(mint_box) = instruction_ref.as_any().downcast_ref::<MintBox>() {
        let mut mint_map = json::Map::new();
        if let MintBox::Asset(mint) = mint_box {
            let mut asset_fields = json::Map::new();
            let object = json::to_value(mint.object()).map_err(norito_to_napi)?;
            let destination = json::Value::String(mint.destination().canonical_literal());
            asset_fields.insert("object".to_owned(), object);
            asset_fields.insert("destination".to_owned(), destination);
            mint_map.insert("Asset".to_owned(), json::Value::Object(asset_fields));
        }
        if let MintBox::TriggerRepetitions(mint) = mint_box {
            let mut trigger_fields = json::Map::new();
            let repetitions = json::to_value(mint.object()).map_err(norito_to_napi)?;
            let destination = json::to_value(mint.destination()).map_err(norito_to_napi)?;
            trigger_fields.insert("object".to_owned(), repetitions);
            trigger_fields.insert("destination".to_owned(), destination);
            mint_map.insert(
                "TriggerRepetitions".to_owned(),
                json::Value::Object(trigger_fields),
            );
        }
        if !mint_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Mint".to_owned(), json::Value::Object(mint_map));
            return Ok(json::Value::Object(outer));
        }
    }

    if let Some(transfer_box) = instruction_ref.as_any().downcast_ref::<TransferBox>() {
        let mut transfer_map = json::Map::new();
        if let TransferBox::Asset(transfer) = transfer_box {
            let mut asset_fields = json::Map::new();
            let source = json::Value::String(transfer.source().canonical_literal());
            let quantity = json::to_value(transfer.object()).map_err(norito_to_napi)?;
            let destination = json::to_value(transfer.destination()).map_err(norito_to_napi)?;
            asset_fields.insert("source".to_owned(), source);
            asset_fields.insert("object".to_owned(), quantity);
            asset_fields.insert("destination".to_owned(), destination);
            transfer_map.insert("Asset".to_owned(), json::Value::Object(asset_fields));
        }
        if let TransferBox::Domain(transfer) = transfer_box {
            let mut domain_fields = json::Map::new();
            let source = json::to_value(transfer.source()).map_err(norito_to_napi)?;
            let object = json::to_value(transfer.object()).map_err(norito_to_napi)?;
            let destination = json::to_value(transfer.destination()).map_err(norito_to_napi)?;
            domain_fields.insert("source".to_owned(), source);
            domain_fields.insert("object".to_owned(), object);
            domain_fields.insert("destination".to_owned(), destination);
            transfer_map.insert("Domain".to_owned(), json::Value::Object(domain_fields));
        }
        if let TransferBox::AssetDefinition(transfer) = transfer_box {
            let mut definition_fields = json::Map::new();
            let source = json::to_value(transfer.source()).map_err(norito_to_napi)?;
            let object = json::to_value(transfer.object()).map_err(norito_to_napi)?;
            let destination = json::to_value(transfer.destination()).map_err(norito_to_napi)?;
            definition_fields.insert("source".to_owned(), source);
            definition_fields.insert("object".to_owned(), object);
            definition_fields.insert("destination".to_owned(), destination);
            transfer_map.insert(
                "AssetDefinition".to_owned(),
                json::Value::Object(definition_fields),
            );
        }
        if let TransferBox::Nft(transfer) = transfer_box {
            let mut nft_fields = json::Map::new();
            let source = json::to_value(transfer.source()).map_err(norito_to_napi)?;
            let object = json::to_value(transfer.object()).map_err(norito_to_napi)?;
            let destination = json::to_value(transfer.destination()).map_err(norito_to_napi)?;
            nft_fields.insert("source".to_owned(), source);
            nft_fields.insert("object".to_owned(), object);
            nft_fields.insert("destination".to_owned(), destination);
            transfer_map.insert("Nft".to_owned(), json::Value::Object(nft_fields));
        }
        if !transfer_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Transfer".to_owned(), json::Value::Object(transfer_map));
            return Ok(json::Value::Object(outer));
        }
    }

    if let Some(burn_box) = instruction_ref.as_any().downcast_ref::<BurnBox>() {
        let mut burn_map = json::Map::new();
        if let BurnBox::Asset(burn) = burn_box {
            let mut asset_fields = json::Map::new();
            let object = json::to_value(burn.object()).map_err(norito_to_napi)?;
            let destination = json::Value::String(burn.destination().canonical_literal());
            asset_fields.insert("object".to_owned(), object);
            asset_fields.insert("destination".to_owned(), destination);
            burn_map.insert("Asset".to_owned(), json::Value::Object(asset_fields));
        }
        if let BurnBox::TriggerRepetitions(burn) = burn_box {
            let mut trigger_fields = json::Map::new();
            let repetitions = json::to_value(burn.object()).map_err(norito_to_napi)?;
            let destination = json::to_value(burn.destination()).map_err(norito_to_napi)?;
            trigger_fields.insert("object".to_owned(), repetitions);
            trigger_fields.insert("destination".to_owned(), destination);
            burn_map.insert(
                "TriggerRepetitions".to_owned(),
                json::Value::Object(trigger_fields),
            );
        }
        if !burn_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Burn".to_owned(), json::Value::Object(burn_map));
            return Ok(json::Value::Object(outer));
        }
    }

    if let Some(grant_box) = instruction_ref.as_any().downcast_ref::<GrantBox>() {
        if let GrantBox::Permission(grant) = grant_box {
            let mut fields = json::Map::new();
            fields.insert(
                "object".to_owned(),
                json::to_value(grant.object()).map_err(norito_to_napi)?,
            );
            fields.insert(
                "destination".to_owned(),
                json::to_value(grant.destination()).map_err(norito_to_napi)?,
            );
            let mut grant_map = json::Map::new();
            grant_map.insert("Permission".to_owned(), json::Value::Object(fields));
            let mut outer = json::Map::new();
            outer.insert("Grant".to_owned(), json::Value::Object(grant_map));
            return Ok(json::Value::Object(outer));
        }
    }

    if let Some(alias) = instruction_ref
        .as_any()
        .downcast_ref::<SetAssetDefinitionAlias>()
    {
        let mut fields = json::Map::new();
        fields.insert(
            "asset_definition_id".to_owned(),
            json::Value::String(alias.asset_definition_id().to_string()),
        );
        fields.insert(
            "alias".to_owned(),
            alias.alias().as_ref().map_or(json::Value::Null, |value| {
                json::Value::String(value.to_string())
            }),
        );
        fields.insert(
            "lease_expiry_ms".to_owned(),
            alias
                .lease_expiry_ms()
                .as_ref()
                .map_or(json::Value::Null, |value| {
                    json::Value::Number(json::Number::from(*value))
                }),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "SetAssetDefinitionAlias".to_owned(),
            json::Value::Object(fields),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(execute_trigger) = instruction_ref.as_any().downcast_ref::<ExecuteTrigger>() {
        let mut payload = json::Map::new();
        payload.insert(
            "trigger".to_owned(),
            json::to_value(execute_trigger.trigger()).map_err(norito_to_napi)?,
        );
        let args = json::parse_value(execute_trigger.args().get()).map_err(|error| {
            napi::Error::new(
                napi::Status::InvalidArg,
                format!("ExecuteTrigger.args is not valid JSON: {error}"),
            )
        })?;
        payload.insert("args".to_owned(), args);
        let mut outer = json::Map::new();
        outer.insert("ExecuteTrigger".to_owned(), json::Value::Object(payload));
        return Ok(json::Value::Object(outer));
    }

    if let Some(rwa_box) = instruction_ref.as_any().downcast_ref::<RwaInstructionBox>() {
        let (label, payload) = match rwa_box {
            RwaInstructionBox::Register(register) => (
                "RegisterRwa",
                norito_json!({ "rwa": new_rwa_to_json(register.rwa())? }),
            ),
            RwaInstructionBox::Transfer(transfer) => (
                "TransferRwa",
                norito_json!({
                    "source": account_id_to_canonical_i105(transfer.source())?,
                    "rwa": transfer.rwa().to_string(),
                    "quantity": transfer.quantity(),
                    "destination": account_id_to_canonical_i105(transfer.destination())?,
                }),
            ),
            RwaInstructionBox::Merge(merge) => {
                let mut payload = json::Map::new();
                payload.insert(
                    "parents".to_owned(),
                    rwa_parent_refs_to_json(merge.parents()),
                );
                payload.insert(
                    "primary_reference".to_owned(),
                    json::Value::String(merge.primary_reference().clone()),
                );
                payload.insert(
                    "status".to_owned(),
                    rwa_status_to_json(merge.status().as_ref()),
                );
                payload.insert(
                    "metadata".to_owned(),
                    json::to_value(merge.metadata()).map_err(norito_to_napi)?,
                );
                ("MergeRwas", json::Value::Object(payload))
            }
            RwaInstructionBox::Redeem(redeem) => (
                "RedeemRwa",
                norito_json!({
                    "rwa": redeem.rwa().to_string(),
                    "quantity": redeem.quantity(),
                }),
            ),
            RwaInstructionBox::Freeze(freeze) => (
                "FreezeRwa",
                norito_json!({ "rwa": freeze.rwa().to_string() }),
            ),
            RwaInstructionBox::Unfreeze(unfreeze) => (
                "UnfreezeRwa",
                norito_json!({ "rwa": unfreeze.rwa().to_string() }),
            ),
            RwaInstructionBox::Hold(hold) => (
                "HoldRwa",
                norito_json!({
                    "rwa": hold.rwa().to_string(),
                    "quantity": hold.quantity(),
                }),
            ),
            RwaInstructionBox::Release(release) => (
                "ReleaseRwa",
                norito_json!({
                    "rwa": release.rwa().to_string(),
                    "quantity": release.quantity(),
                }),
            ),
            RwaInstructionBox::ForceTransfer(force_transfer) => (
                "ForceTransferRwa",
                norito_json!({
                    "rwa": force_transfer.rwa().to_string(),
                    "quantity": force_transfer.quantity(),
                    "destination": account_id_to_canonical_i105(force_transfer.destination())?,
                }),
            ),
            RwaInstructionBox::SetControls(set_controls) => (
                "SetRwaControls",
                norito_json!({
                    "rwa": set_controls.rwa().to_string(),
                    "controls": rwa_control_policy_to_json(set_controls.controls())?,
                }),
            ),
            RwaInstructionBox::SetKeyValue(set) => (
                "SetRwaKeyValue",
                norito_json!({
                    "rwa": set.object().to_string(),
                    "key": set.key().clone(),
                    "value": json::to_value(set.value()).map_err(norito_to_napi)?,
                }),
            ),
            RwaInstructionBox::RemoveKeyValue(remove) => (
                "RemoveRwaKeyValue",
                norito_json!({
                    "rwa": remove.object().to_string(),
                    "key": remove.key().clone(),
                }),
            ),
        };
        let mut outer = json::Map::new();
        outer.insert(label.to_owned(), payload);
        return Ok(json::Value::Object(outer));
    }

    if let Some(custom_instruction) = instruction_ref.as_any().downcast_ref::<CustomInstruction>() {
        let payload_json =
            json::parse_value(custom_instruction.payload.get()).map_err(|error| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("Custom.payload is not valid JSON: {error}"),
                )
            })?;
        return Ok(custom_json_value(payload_json));
    }

    if let Some(register) = instruction_ref.as_any().downcast_ref::<RegisterRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "RegisterRwa".to_owned(),
            norito_json!({ "rwa": new_rwa_to_json(register.rwa())? }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(transfer) = instruction_ref.as_any().downcast_ref::<TransferRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "TransferRwa".to_owned(),
            norito_json!({
                "source": account_id_to_canonical_i105(transfer.source())?,
                "rwa": transfer.rwa().to_string(),
                "quantity": transfer.quantity(),
                "destination": account_id_to_canonical_i105(transfer.destination())?,
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(merge) = instruction_ref.as_any().downcast_ref::<MergeRwas>() {
        let mut payload = json::Map::new();
        payload.insert(
            "parents".to_owned(),
            rwa_parent_refs_to_json(merge.parents()),
        );
        payload.insert(
            "primary_reference".to_owned(),
            json::Value::String(merge.primary_reference().clone()),
        );
        payload.insert(
            "status".to_owned(),
            rwa_status_to_json(merge.status().as_ref()),
        );
        payload.insert(
            "metadata".to_owned(),
            json::to_value(merge.metadata()).map_err(norito_to_napi)?,
        );
        let mut outer = json::Map::new();
        outer.insert("MergeRwas".to_owned(), json::Value::Object(payload));
        return Ok(json::Value::Object(outer));
    }

    if let Some(redeem) = instruction_ref.as_any().downcast_ref::<RedeemRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "RedeemRwa".to_owned(),
            norito_json!({
                "rwa": redeem.rwa().to_string(),
                "quantity": redeem.quantity(),
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(freeze) = instruction_ref.as_any().downcast_ref::<FreezeRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "FreezeRwa".to_owned(),
            norito_json!({ "rwa": freeze.rwa().to_string() }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(unfreeze) = instruction_ref.as_any().downcast_ref::<UnfreezeRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "UnfreezeRwa".to_owned(),
            norito_json!({ "rwa": unfreeze.rwa().to_string() }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(hold) = instruction_ref.as_any().downcast_ref::<HoldRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "HoldRwa".to_owned(),
            norito_json!({
                "rwa": hold.rwa().to_string(),
                "quantity": hold.quantity(),
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(release) = instruction_ref.as_any().downcast_ref::<ReleaseRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "ReleaseRwa".to_owned(),
            norito_json!({
                "rwa": release.rwa().to_string(),
                "quantity": release.quantity(),
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(force_transfer) = instruction_ref.as_any().downcast_ref::<ForceTransferRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "ForceTransferRwa".to_owned(),
            norito_json!({
                "rwa": force_transfer.rwa().to_string(),
                "quantity": force_transfer.quantity(),
                "destination": account_id_to_canonical_i105(force_transfer.destination())?,
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(set_controls) = instruction_ref.as_any().downcast_ref::<SetRwaControls>() {
        let mut outer = json::Map::new();
        outer.insert(
            "SetRwaControls".to_owned(),
            norito_json!({
                "rwa": set_controls.rwa().to_string(),
                "controls": rwa_control_policy_to_json(set_controls.controls())?,
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(submit) = instruction_ref
        .as_any()
        .downcast_ref::<SubmitAgendaProposal>()
    {
        let mut outer = json::Map::new();
        outer.insert(
            "SubmitAgendaProposal".to_owned(),
            norito_json!({
                "proposal": submit.proposal,
            }),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(propose) = instruction_ref
        .as_any()
        .downcast_ref::<ProposeDeployContract>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "contract_address".to_owned(),
            json::Value::String(propose.contract_address.to_string()),
        );
        inner.insert(
            "code_hash_hex".to_owned(),
            json::Value::String(propose.code_hash_hex.clone()),
        );
        inner.insert(
            "abi_hash_hex".to_owned(),
            json::Value::String(propose.abi_hash_hex.clone()),
        );
        inner.insert(
            "abi_version".to_owned(),
            json::Value::String(propose.abi_version.clone()),
        );
        if let Some(window) = &propose.window {
            inner.insert(
                "window".to_owned(),
                json::to_value(window).map_err(norito_to_napi)?,
            );
        }
        if let Some(mode) = propose.mode {
            inner.insert(
                "mode".to_owned(),
                json::Value::String(voting_mode_to_json(mode).to_owned()),
            );
        }
        let mut outer = json::Map::new();
        outer.insert(
            "ProposeDeployContract".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(ballot) = instruction_ref.as_any().downcast_ref::<CastZkBallot>() {
        let mut inner = json::Map::new();
        inner.insert(
            "election_id".to_owned(),
            json::Value::String(ballot.election_id.clone()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(ballot.proof_b64.clone()),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(ballot.public_inputs_json.clone()),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }

    if let Some(ballot) = instruction_ref.as_any().downcast_ref::<CastPlainBallot>() {
        let mut inner = json::Map::new();
        inner.insert(
            "referendum_id".to_owned(),
            json::Value::String(ballot.referendum_id.clone()),
        );
        inner.insert(
            "owner".to_owned(),
            json::to_value(&ballot.owner).map_err(norito_to_napi)?,
        );
        inner.insert(
            "amount".to_owned(),
            json::Value::String(ballot.amount.to_string()),
        );
        inner.insert(
            "duration_blocks".to_owned(),
            json::to_value(&ballot.duration_blocks).map_err(norito_to_napi)?,
        );
        inner.insert(
            "direction".to_owned(),
            json::to_value(&ballot.direction).map_err(norito_to_napi)?,
        );
        let mut outer = json::Map::new();
        outer.insert("CastPlainBallot".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }

    if let Some(citizen) = instruction_ref.as_any().downcast_ref::<RegisterCitizen>() {
        let mut inner = json::Map::new();
        inner.insert(
            "owner".to_owned(),
            json::to_value(&citizen.owner).map_err(norito_to_napi)?,
        );
        inner.insert(
            "amount".to_owned(),
            json::Value::String(citizen.amount.to_string()),
        );
        let mut outer = json::Map::new();
        outer.insert("RegisterCitizen".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }

    if let Some(register) = instruction_ref.as_any().downcast_ref::<RegisterZkAsset>() {
        return Ok(zk_json_value(
            "RegisterZkAsset",
            json::to_value(register).map_err(norito_to_napi)?,
        ));
    }

    if let Some(transition) = instruction_ref
        .as_any()
        .downcast_ref::<ScheduleConfidentialPolicyTransition>()
    {
        return Ok(zk_json_value(
            "ScheduleConfidentialPolicyTransition",
            json::to_value(transition).map_err(norito_to_napi)?,
        ));
    }

    if let Some(cancel) = instruction_ref
        .as_any()
        .downcast_ref::<CancelConfidentialPolicyTransition>()
    {
        return Ok(zk_json_value(
            "CancelConfidentialPolicyTransition",
            json::to_value(cancel).map_err(norito_to_napi)?,
        ));
    }

    if let Some(shield) = instruction_ref.as_any().downcast_ref::<Shield>() {
        return Ok(zk_json_value(
            "Shield",
            json::to_value(shield).map_err(norito_to_napi)?,
        ));
    }

    if let Some(transfer) = instruction_ref.as_any().downcast_ref::<ZkTransfer>() {
        return Ok(zk_json_value(
            "ZkTransfer",
            json::to_value(transfer).map_err(norito_to_napi)?,
        ));
    }

    if let Some(unshield) = instruction_ref.as_any().downcast_ref::<Unshield>() {
        return Ok(zk_json_value(
            "Unshield",
            json::to_value(unshield).map_err(norito_to_napi)?,
        ));
    }

    if let Some(create) = instruction_ref.as_any().downcast_ref::<CreateElection>() {
        return Ok(zk_json_value(
            "CreateElection",
            json::to_value(create).map_err(norito_to_napi)?,
        ));
    }

    if let Some(submit) = instruction_ref.as_any().downcast_ref::<SubmitBallot>() {
        return Ok(zk_json_value(
            "SubmitBallot",
            json::to_value(submit).map_err(norito_to_napi)?,
        ));
    }

    if let Some(finalize) = instruction_ref.as_any().downcast_ref::<FinalizeElection>() {
        return Ok(zk_json_value(
            "FinalizeElection",
            json::to_value(finalize).map_err(norito_to_napi)?,
        ));
    }

    if let Some(enact) = instruction_ref.as_any().downcast_ref::<EnactReferendum>() {
        let mut outer = json::Map::new();
        outer.insert(
            "EnactReferendum".to_owned(),
            json::to_value(enact).map_err(norito_to_napi)?,
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(finalize) = instruction_ref
        .as_any()
        .downcast_ref::<FinalizeReferendum>()
    {
        let mut outer = json::Map::new();
        outer.insert(
            "FinalizeReferendum".to_owned(),
            json::to_value(finalize).map_err(norito_to_napi)?,
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(persist) = instruction_ref
        .as_any()
        .downcast_ref::<PersistCouncilForEpoch>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "epoch".to_owned(),
            json::to_value(&persist.epoch).map_err(norito_to_napi)?,
        );
        inner.insert(
            "members".to_owned(),
            json::to_value(&persist.members).map_err(norito_to_napi)?,
        );
        inner.insert(
            "alternates".to_owned(),
            json::to_value(&persist.alternates).map_err(norito_to_napi)?,
        );
        inner.insert(
            "verified".to_owned(),
            json::to_value(&persist.verified).map_err(norito_to_napi)?,
        );
        inner.insert(
            "candidates_count".to_owned(),
            json::to_value(&persist.candidates_count).map_err(norito_to_napi)?,
        );
        inner.insert(
            "derived_by".to_owned(),
            council_derivation_to_json(persist.derived_by),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "PersistCouncilForEpoch".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(register_code) = instruction_ref
        .as_any()
        .downcast_ref::<RegisterSmartContractCode>()
    {
        let mut manifest_value = json::to_value(&register_code.manifest).map_err(norito_to_napi)?;
        if let Some(map) = manifest_value.as_object_mut()
            && map.get("provenance").is_some_and(json::Value::is_null)
        {
            map.remove("provenance");
        }
        let mut inner = json::Map::new();
        inner.insert("manifest".to_owned(), manifest_value);
        let mut outer = json::Map::new();
        outer.insert(
            "RegisterSmartContractCode".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(register_bytes) = instruction_ref
        .as_any()
        .downcast_ref::<RegisterSmartContractBytes>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&register_bytes.code_hash).map_err(norito_to_napi)?,
        );
        inner.insert(
            "code".to_owned(),
            json::Value::String(STANDARD.encode(&register_bytes.code)),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "RegisterSmartContractBytes".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(remove_bytes) = instruction_ref
        .as_any()
        .downcast_ref::<RemoveSmartContractBytes>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&remove_bytes.code_hash).map_err(norito_to_napi)?,
        );
        if let Some(reason) = &remove_bytes.reason {
            inner.insert("reason".to_owned(), json::Value::String(reason.clone()));
        }
        let mut outer = json::Map::new();
        outer.insert(
            "RemoveSmartContractBytes".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(activate) = instruction_ref
        .as_any()
        .downcast_ref::<ActivateContractInstance>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "contract_address".to_owned(),
            json::Value::String(activate.contract_address.to_string()),
        );
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&activate.code_hash).map_err(norito_to_napi)?,
        );
        let mut outer = json::Map::new();
        outer.insert(
            "ActivateContractInstance".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(deactivate) = instruction_ref
        .as_any()
        .downcast_ref::<DeactivateContractInstance>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "contract_address".to_owned(),
            json::Value::String(deactivate.contract_address.to_string()),
        );
        if let Some(reason) = &deactivate.reason {
            inner.insert("reason".to_owned(), json::Value::String(reason.clone()));
        }
        let mut outer = json::Map::new();
        outer.insert(
            "DeactivateContractInstance".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(claim) = instruction_ref
        .as_any()
        .downcast_ref::<ClaimTwitterFollowReward>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "binding_hash".to_owned(),
            json::to_value(&claim.binding_hash).map_err(norito_to_napi)?,
        );
        let mut outer = json::Map::new();
        outer.insert(
            "ClaimTwitterFollowReward".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }

    if let Some(send) = instruction_ref.as_any().downcast_ref::<SendToTwitter>() {
        let mut inner = json::Map::new();
        inner.insert(
            "binding_hash".to_owned(),
            json::to_value(&send.binding_hash).map_err(norito_to_napi)?,
        );
        inner.insert(
            "amount".to_owned(),
            json::to_value(&send.amount).map_err(norito_to_napi)?,
        );
        let mut outer = json::Map::new();
        outer.insert("SendToTwitter".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }

    if let Some(cancel) = instruction_ref
        .as_any()
        .downcast_ref::<CancelTwitterEscrow>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "binding_hash".to_owned(),
            json::to_value(&cancel.binding_hash).map_err(norito_to_napi)?,
        );
        let mut outer = json::Map::new();
        outer.insert("CancelTwitterEscrow".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }

    if let Some(create) = instruction_ref.as_any().downcast_ref::<CreateKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call".to_owned(),
            json::to_value(create.call()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(create.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(create.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(create.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(create.proof().as_ref()),
        );
        return Ok(kaigi_json_value(
            "CreateKaigi",
            json::Value::Object(payload),
        ));
    }
    if let Some(join) = instruction_ref.as_any().downcast_ref::<JoinKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(join.call_id()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "participant".to_owned(),
            json::to_value(join.participant()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(join.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(join.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(join.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(join.proof().as_ref()),
        );
        return Ok(kaigi_json_value("JoinKaigi", json::Value::Object(payload)));
    }
    if let Some(leave) = instruction_ref.as_any().downcast_ref::<LeaveKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(leave.call_id()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "participant".to_owned(),
            json::to_value(leave.participant()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(leave.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(leave.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(leave.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(leave.proof().as_ref()),
        );
        return Ok(kaigi_json_value("LeaveKaigi", json::Value::Object(payload)));
    }
    if let Some(end) = instruction_ref.as_any().downcast_ref::<EndKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(end.call_id()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "ended_at_ms".to_owned(),
            json::to_value(end.ended_at_ms()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(end.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(end.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(end.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(end.proof().as_ref()),
        );
        return Ok(kaigi_json_value("EndKaigi", json::Value::Object(payload)));
    }
    if let Some(usage) = instruction_ref.as_any().downcast_ref::<RecordKaigiUsage>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(usage.call_id()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "duration_ms".to_owned(),
            json::to_value(usage.duration_ms()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "billed_gas".to_owned(),
            json::to_value(usage.billed_gas()).map_err(norito_to_napi)?,
        );
        payload.insert(
            "usage_commitment".to_owned(),
            optional_hash_to_json(usage.usage_commitment().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(usage.proof().as_ref()),
        );
        return Ok(kaigi_json_value(
            "RecordKaigiUsage",
            json::Value::Object(payload),
        ));
    }
    if let Some(health) = instruction_ref
        .as_any()
        .downcast_ref::<ReportKaigiRelayHealth>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(&health.call_id).map_err(norito_to_napi)?,
        );
        payload.insert(
            "relay_id".to_owned(),
            json::to_value(&health.relay_id).map_err(norito_to_napi)?,
        );
        payload.insert(
            "status".to_owned(),
            json::to_value(&health.status).map_err(norito_to_napi)?,
        );
        payload.insert(
            "reported_at_ms".to_owned(),
            json::Value::Number(health.reported_at_ms.into()),
        );
        payload.insert(
            "notes".to_owned(),
            health
                .notes
                .as_ref()
                .map_or(json::Value::Null, |s| json::Value::String(s.clone())),
        );
        return Ok(kaigi_json_value(
            "ReportKaigiRelayHealth",
            json::Value::Object(payload),
        ));
    }
    if let Some(manifest) = instruction_ref
        .as_any()
        .downcast_ref::<SetKaigiRelayManifest>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(manifest.call_id()).map_err(norito_to_napi)?,
        );
        let relay_manifest = manifest.relay_manifest().clone();
        payload.insert(
            "relay_manifest".to_owned(),
            json::to_value(&relay_manifest).map_err(norito_to_napi)?,
        );
        return Ok(kaigi_json_value(
            "SetKaigiRelayManifest",
            json::Value::Object(payload),
        ));
    }
    if let Some(registration) = instruction_ref
        .as_any()
        .downcast_ref::<RegisterKaigiRelay>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "relay".to_owned(),
            json::to_value(registration.relay()).map_err(norito_to_napi)?,
        );
        return Ok(kaigi_json_value(
            "RegisterKaigiRelay",
            json::Value::Object(payload),
        ));
    }

    Err(napi::Error::new(
        napi::Status::GenericFailure,
        "unsupported instruction variant; JSON conversion is not yet implemented for this instruction",
    ))
}

fn kaigi_json_value(tag: &str, payload: json::Value) -> json::Value {
    let mut variant = json::Map::new();
    variant.insert(tag.to_owned(), payload);
    let mut outer = json::Map::new();
    outer.insert("Kaigi".to_owned(), json::Value::Object(variant));
    json::Value::Object(outer)
}

fn custom_json_value(payload: json::Value) -> json::Value {
    let mut custom = json::Map::new();
    custom.insert("payload".to_owned(), payload);
    let mut outer = json::Map::new();
    outer.insert("Custom".to_owned(), json::Value::Object(custom));
    json::Value::Object(outer)
}

fn zk_json_value(tag: &str, payload: json::Value) -> json::Value {
    let mut variant = json::Map::new();
    variant.insert(tag.to_owned(), payload);
    let mut outer = json::Map::new();
    outer.insert("zk".to_owned(), json::Value::Object(variant));
    json::Value::Object(outer)
}

fn try_decode_signed_transaction_adaptive_with_flags(
    payload: &[u8],
    flags: u8,
) -> Result<SignedTransaction, String> {
    let attempt = catch_unwind(AssertUnwindSafe(|| {
        let _guard = norito_core::DecodeFlagsGuard::enter_with_hint(flags, flags);
        norito::codec::decode_adaptive::<SignedTransaction>(payload)
    }));
    match attempt {
        Ok(Ok(tx)) => Ok(tx),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err("panic".to_owned()),
    }
}

fn try_decode_signed_transaction_versioned(bytes: &[u8]) -> Result<SignedTransaction, String> {
    let Some((&version, payload)) = bytes.split_first() else {
        return Err("empty payload".to_owned());
    };
    if version != 1 {
        return Err(format!("unsupported version byte {version}"));
    }
    let (decoded, used) =
        SignedTransaction::decode_from_slice(payload).map_err(|err| err.to_string())?;
    if used != payload.len() {
        return Err(format!("trailing bytes ({used} of {} used)", payload.len()));
    }
    Ok(decoded)
}

fn decode_signed_transaction(bytes: &[u8]) -> napi::Result<SignedTransaction> {
    let mut attempts = Vec::new();

    match try_decode_signed_transaction_versioned(bytes) {
        Ok(decoded) => return Ok(decoded),
        Err(err) => attempts.push(format!("versioned: {err}")),
    }

    match SignedTransaction::decode_from_slice(bytes) {
        Ok((decoded, used)) if used == bytes.len() => return Ok(decoded),
        Ok((_, used)) => attempts.push(format!(
            "bare adaptive: trailing bytes ({used} of {} used)",
            bytes.len()
        )),
        Err(err) => attempts.push(format!("bare adaptive: {err}")),
    }

    match norito::decode_from_bytes::<SignedTransaction>(bytes) {
        Ok(decoded) => return Ok(decoded),
        Err(err) => attempts.push(format!("framed norito: {err}")),
    }

    if let Ok(view) = norito_core::from_bytes_view(bytes) {
        let payload = view.as_bytes();
        let packed = norito_core::header_flags::PACKED_STRUCT;
        for (label, flags) in [
            ("framed payload flags", view.flags() | view.flags_hint()),
            ("framed payload no flags", 0),
            ("framed payload packed-struct", packed),
        ] {
            match try_decode_signed_transaction_adaptive_with_flags(payload, flags) {
                Ok(decoded) => return Ok(decoded),
                Err(err) => attempts.push(format!("{label}: {err}")),
            }
        }
    }

    match try_decode_signed_transaction_adaptive_with_flags(bytes, 0) {
        Ok(decoded) => Ok(decoded),
        Err(err) => {
            attempts.push(format!("headerless adaptive fallback: {err}"));
            Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!(
                    "failed to decode signed transaction; attempts: {}",
                    attempts.join("; ")
                ),
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)] // mirrors TransactionBuilder inputs for clarity
fn assemble_executable_transaction(
    chain_id: ChainId,
    authority: AccountId,
    executable: Executable,
    metadata: Metadata,
    attachments: Option<ProofAttachmentList>,
    creation_time_ms: Option<i64>,
    ttl_ms: Option<i64>,
    nonce: Option<u32>,
    secret: &[u8],
) -> napi::Result<JsSignedTransaction> {
    let mut builder = TransactionBuilder::new(chain_id, authority).with_executable(executable);

    if let Some(ms) = creation_time_ms {
        let millis = u64::try_from(ms).map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "creation time must be non-negative",
            )
        })?;
        builder.set_creation_time(Duration::from_millis(millis));
    }

    if let Some(ms) = ttl_ms {
        let millis = u64::try_from(ms)
            .map_err(|_| napi::Error::new(napi::Status::InvalidArg, "ttl must be non-negative"))?;
        builder.set_ttl(Duration::from_millis(millis));
    }

    if let Some(value) = nonce {
        let nonce = NonZeroU32::new(value).ok_or_else(|| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "nonce must be non-zero (fits in u32)",
            )
        })?;
        builder.set_nonce(nonce);
    }

    builder = builder.with_metadata(metadata);
    if let Some(attachments) = attachments {
        builder = builder.with_attachments(attachments);
    }

    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, secret).map_err(norito_to_napi)?;
    let signed = sign_js_transaction(builder, &private_key, "JavaScript host assembled")?;
    let signed_bytes = Encode::encode(&signed);
    let hash = Buffer::from(signed.hash().as_ref().to_vec());

    Ok(JsSignedTransaction {
        signed_transaction: Buffer::from(signed_bytes),
        hash,
    })
}

#[allow(clippy::too_many_arguments)] // mirrors TransactionBuilder inputs for clarity
fn assemble_transaction(
    chain_id: ChainId,
    authority: AccountId,
    instructions: Vec<InstructionBox>,
    metadata: Metadata,
    creation_time_ms: Option<i64>,
    ttl_ms: Option<i64>,
    nonce: Option<u32>,
    secret: &[u8],
) -> napi::Result<JsSignedTransaction> {
    if instructions.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "instructions must be a non-empty array",
        ));
    }

    assemble_executable_transaction(
        chain_id,
        authority,
        Executable::from(instructions),
        metadata,
        None,
        creation_time_ms,
        ttl_ms,
        nonce,
        secret,
    )
}

/// Compute the canonical pipeline hash for a Norito-serialized signed transaction.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API typed arrays require ownership at the boundary
pub fn hash_signed_transaction(bytes: Uint8Array) -> napi::Result<Buffer> {
    let tx = decode_signed_transaction(bytes.as_ref())?;
    let hash = tx.hash();
    Ok(Buffer::from(hash.as_ref().to_vec()))
}

/// Decode a Norito-serialized signed transaction into its JSON representation.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Uint8Array boundary requires ownership
pub fn decode_signed_transaction_json(bytes: Uint8Array) -> napi::Result<String> {
    ensure_packed_struct_disabled();
    let tx = decode_signed_transaction(bytes.as_ref())?;
    json::to_json(&tx).map_err(norito_to_napi)
}

/// Convert a versioned signed transaction payload into Norito bytes.
///
/// This is used by Torii deployments that expose legacy `/transaction` submit
/// endpoints expecting `application/x-norito`.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Uint8Array boundary requires ownership
pub fn encode_signed_transaction_norito(bytes: Uint8Array) -> napi::Result<Buffer> {
    ensure_packed_struct_disabled();
    let tx = decode_signed_transaction(bytes.as_ref())?;
    let encoded = norito::to_bytes(&tx).map_err(norito_to_napi)?;
    Ok(Buffer::from(encoded))
}

/// Convert a signed transaction payload into versioned adaptive Norito bytes.
///
/// This is the public `/transaction` payload shape accepted by Torii routes
/// that decode `SignedTransaction::decode_all_versioned`.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Uint8Array boundary requires ownership
pub fn encode_signed_transaction_versioned(bytes: Uint8Array) -> napi::Result<Buffer> {
    ensure_packed_struct_disabled();
    let tx = decode_signed_transaction(bytes.as_ref())?;
    let mut encoded = Vec::with_capacity(bytes.len() + 1);
    encoded.push(1);
    encoded.extend(norito::codec::encode_adaptive(&tx));
    Ok(Buffer::from(encoded))
}

/// Decode a Norito-framed transaction submission receipt into its JSON representation.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Uint8Array boundary requires ownership
pub fn decode_transaction_receipt_json(bytes: Uint8Array) -> napi::Result<String> {
    ensure_packed_struct_disabled();
    let receipt: TransactionSubmissionReceipt =
        decode_from_bytes(bytes.as_ref()).map_err(norito_to_napi)?;
    json::to_json(&receipt).map_err(norito_to_napi)
}

/// Re-sign a Norito-serialized transaction with the provided Ed25519 private key
/// and return the updated signed transaction bytes.
#[napi]
#[allow(clippy::needless_pass_by_value)] // N-API typed arrays require ownership at the boundary
pub fn sign_transaction(bytes: Uint8Array, secret: Uint8Array) -> napi::Result<Buffer> {
    let tx = decode_signed_transaction(bytes.as_ref())?;
    let mut builder = TransactionBuilder::new(tx.chain().clone(), tx.authority().clone())
        .with_executable(tx.instructions().clone())
        .with_metadata(tx.metadata().clone());

    if let Some(nonce) = tx.nonce() {
        builder.set_nonce(nonce);
    }
    builder.set_creation_time(tx.creation_time());
    if let Some(ttl) = tx.time_to_live() {
        builder.set_ttl(ttl);
    }
    if let Some(attachments) = tx.attachments() {
        builder = builder.with_attachments(attachments.clone());
    }

    let private_key =
        PrivateKey::from_bytes(Algorithm::Ed25519, secret.as_ref()).map_err(norito_to_napi)?;
    let signed = sign_js_transaction(builder, &private_key, "JavaScript host re-signed")?;
    Ok(Buffer::from(Encode::encode(&signed)))
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
const PRIVACY_PRODUCTION_REVIEW_SCOPE_VERSION: &str = "privacy-production-review-scope-v1";
const PRIVACY_PRODUCTION_GATE_MISSING_ENGINE: &str =
    "real protocol engine is not production-enabled";
const PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST: &str =
    "Iroha production allowlist is not enabled for this audited row";
const PRIVACY_PRODUCTION_DISABLED_MESSAGE: &str = "privacy production is disabled until exact protocol implementation, real proving, real verification, chain admission, cross-SDK parity, wallet/state support, witness privacy checks, deterministic tests, negative/adversarial tests, replay/nullifier rejection tests, fuzzing, parser fuzzing, verifier fuzzing, performance gates, internal cryptographic review, real protocol engine enablement, and Iroha production allowlist evidence all pass";
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
    (
        "witness_privacy_checks",
        "witness privacy checks are incomplete",
    ),
    ("deterministic_tests", "deterministic tests are incomplete"),
    (
        "negative_adversarial_tests",
        "negative/adversarial tests are incomplete",
    ),
    (
        "replay_nullifier_tests",
        "replay/nullifier rejection tests are incomplete",
    ),
    ("fuzzing", "fuzzing gate is incomplete"),
    ("parser_fuzzing", "parser fuzzing gate is incomplete"),
    ("verifier_fuzzing", "verifier fuzzing gate is incomplete"),
    ("performance_gates", "performance gate is incomplete"),
    (
        "external_audit",
        "internal cryptographic review signoff is missing",
    ),
];
const PRIVACY_TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS: &[&str] = &[
    "real_proving",
    "real_verification",
    "witness_privacy_checks",
    "verifier_fuzzing",
];
const PRIVACY_PRODUCTION_EVIDENCE_HASH_PREFIX: &str = "sha256:";
const PRIVACY_PRODUCTION_LOCALNET_TARGET: &str = "localnet";
const PRIVACY_PRODUCTION_LOCALNET_PEER_COUNT: u8 = 4;
const PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES: &[&str] = &[
    "rust_core",
    "ffi",
    "python",
    "javascript",
    "java_android",
    "kotlin",
    "swift",
    "csharp",
];
const PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS: &[&str] =
    &["types", "validation_rules", "error_codes", "golden_vectors"];

const PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS: &[(&str, &str, &str)] = &[
    (
        "zk-ace-pq-authorization-v0",
        "stark/fri/sha256-goldilocks",
        "stark-fri",
    ),
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
const PRIVACY_RESEARCH_TARGET_ALGORITHM_IDS: &[&str] = &[];
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
    "mainnetcertified",
    "mainnetapproved",
    "mainnetrelease",
    "auditedproduction",
    "externallyaudited",
    "thirdpartyaudited",
    "boiaudited",
    "auditedmainnet",
    "externalaudit",
    "auditpassed",
    "auditapproved",
    "auditsignoff",
    "auditclaim",
    "claimedaudit",
    "securityreviewpassed",
    "securityauditpassed",
    "securityaudited",
    "externalsecurityreview",
    "certifiedproduction",
    "certifiedmainnet",
    "releaseready",
    "releaseapproved",
    "releasecertified",
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
            "buildConfidentialAssetHiddenTransferProofV1",
            "buildRegisterAssetHiddenZkPoolInstruction",
            "buildAssetHiddenZkTransferInstruction",
        ],
        planned_entrypoints: &[],
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
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "anonymous-pgc-k-out-of-n-v1",
        proof_family: "anonymous-pgc-k-out-of-n",
        backend_family: "anonymous-pgc",
        sdk_entrypoints: &[
            "buildAnonymousPgcReceiverSet",
            "buildAnonymousPgcAccountCommitmentInstruction",
            "buildAnonymousPgcKOutOfNProofV1",
            "buildAnonymousPgcTransferInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "verange-transparent-range-v1",
        proof_family: "verange-transparent-range",
        backend_family: "verange",
        sdk_entrypoints: &[
            "buildRangeCommitment",
            "buildVeRangeDevProofFixture",
            "buildVeRangeProofEnvelope",
            "buildVeRangeProofV1",
            "verifyVeRangeProofLocally",
            "verifyVeRangeProofV1",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "zkat-policy-private-auth-v1",
        proof_family: "zkat-policy-private-authenticator",
        backend_family: "zkat",
        sdk_entrypoints: &[
            "buildZkAtPolicyCommitment",
            "buildZkAtAuthenticatorEnvelope",
            "buildZkAtPolicyProofV1",
            "verifyZkAtPolicyProofV1",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "zk-ams-recursive-admission-v0",
        proof_family: "recursive-anonymous-admission",
        backend_family: "recursive-anonymous-admission",
        sdk_entrypoints: &[
            "buildZkAmsAdmissionBatch",
            "buildZkAmsAdmissionProofEnvelope",
            "buildZkAmsAdmissionBatchProofV0",
            "verifyZkAmsAdmissionBatchProofV0",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "vega-existing-credential-zk-v0",
        proof_family: "existing-credential-zk",
        backend_family: "vega-existing-credential-zk",
        sdk_entrypoints: &[
            "buildVegaCredentialPredicateCommitment",
            "buildVegaCredentialProofEnvelope",
            "buildVegaCredentialPredicateProofV0",
            "verifyVegaCredentialPredicateProofV0",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "silent-threshold-anoncred-v0",
        proof_family: "threshold-anonymous-credentials",
        backend_family: "silent-threshold-anoncred",
        sdk_entrypoints: &[
            "buildSilentThresholdCredentialCommitments",
            "buildSilentThresholdCredentialEnvelope",
            "buildSilentThresholdCredentialShowingProofV0",
            "verifySilentThresholdCredentialShowingProofV0",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "zk-x509-onchain-identity-v0",
        proof_family: "zkvm-x509-identity",
        backend_family: "zk-x509",
        sdk_entrypoints: &[
            "buildZkX509IdentityCommitments",
            "buildZkX509IdentityEnvelope",
            "buildZkX509IdentityProofV0",
            "verifyZkX509IdentityProofV0",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "jindo-lattice-pcs-zk-v0",
        proof_family: "lattice-polynomial-commitment",
        backend_family: "lattice-pcs-sis",
        sdk_entrypoints: &[
            "buildJindoLatticePublicInputs",
            "buildJindoLatticeProofEnvelope",
            "buildJindoLatticeProofV0",
            "verifyJindoPolynomialCommitmentV0",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "sis-hints-anoncred-pq-v0",
        proof_family: "lattice-anonymous-credentials",
        backend_family: "sis-with-hints",
        sdk_entrypoints: &[
            "buildSisHintsCredentialCommitments",
            "buildSisHintsCredentialEnvelope",
            "buildSisHintsAnonymousCredentialProofV0",
            "verifySisHintsAnonymousCredentialProofV0",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "orchard-halo2-actions-v1",
        proof_family: "halo2-pasta-action-bundle",
        backend_family: "halo2-ipa-orchard",
        sdk_entrypoints: &[
            "buildOrchardActionBundleProofV1",
            "buildOrchardActionBundleInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "penumbra-masp-v1",
        proof_family: "groth16-bls12-377-decaf377",
        backend_family: "groth16-bls12-377",
        sdk_entrypoints: &[
            "buildPenumbraSpendProofV1",
            "buildPenumbraOutputProofV1",
            "buildPenumbraShieldedPoolTransaction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "monero-fcmp-plus-plus-v1",
        proof_family: "fcmp-plus-plus-curve-trees-bulletproofs",
        backend_family: "fcmp-plus-plus-curve-tree",
        sdk_entrypoints: &[
            "buildFcmpPlusPlusMembershipProofV1",
            "buildFcmpPlusPlusTransferInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "miden-stark-note-v1",
        proof_family: "stark-vm-note-transaction",
        backend_family: "miden-stark",
        sdk_entrypoints: &[
            "buildMidenStarkTransactionProofV1",
            "buildMidenNoteTransactionInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "aztec-private-rollup-v1",
        proof_family: "plonkish-private-kernel-rollup",
        backend_family: "aztec-plonkish-private-kernel",
        sdk_entrypoints: &[
            "buildAztecPrivateKernelProofV1",
            "buildAztecPrivateRollupTransactionInstruction",
        ],
        planned_entrypoints: &[],
    },
    PrivacyAlgorithmEntry {
        id: "pq-masp-stark-v0",
        proof_family: "stark-fri",
        backend_family: "pq-masp-stark-fri",
        sdk_entrypoints: &[
            "buildPqMaspStarkTransferProofV0",
            "buildPqMaspStarkRegisterPoolInstruction",
            "buildPqMaspStarkTransferInstruction",
            "generateMlDsaKeyPair",
            "encapsulateMlKem",
        ],
        planned_entrypoints: &[],
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
    required_gates: Vec<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrivacyProductionGateEvidenceV1 {
    key: &'static str,
    artifact_hash: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct PrivacyProductionLocalnetEvidenceV1 {
    run_id: &'static str,
    target: &'static str,
    peer_count: u8,
    peer_ids: [&'static str; 4],
    chain_id: &'static str,
    smoke_passed: bool,
    smoke_tx_hash: &'static str,
    replay_rejected: bool,
    replay_rejection_hash: &'static str,
    restart_persistence_checked: bool,
    restart_replay_rejected: bool,
    restart_replay_rejection_hash: &'static str,
    state_recovery_passed: bool,
    state_recovery_hash: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PrivacyProductionSdkExportV1 {
    surface: &'static str,
    entrypoints: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrivacyProductionSdkParityArtifactV1 {
    kind: &'static str,
    surface: &'static str,
    artifact_hash: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PrivacyProductionReviewScopeV1 {
    version: &'static str,
    algorithm_id: &'static str,
    chain_id: &'static str,
    verifier_key_id: &'static str,
    proof_family: &'static str,
    public_inputs_schema: Option<&'static str>,
    sdk_entrypoints: Vec<&'static str>,
    required_state: Vec<&'static str>,
    fuzz_artifact_hash: &'static str,
    performance_artifact_hash: &'static str,
    localnet_run_id: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PrivacyProductionEvidenceRowV1 {
    algorithm_id: &'static str,
    chain_id: &'static str,
    reviewer_identity: &'static str,
    review_artifact_hash: &'static str,
    review_artifact_signature: &'static str,
    review_scope: PrivacyProductionReviewScopeV1,
    verifier_key_id: &'static str,
    proof_family: &'static str,
    public_inputs_schema: Option<&'static str>,
    sdk_entrypoints: Vec<&'static str>,
    sdk_exports: Vec<PrivacyProductionSdkExportV1>,
    sdk_parity_artifacts: Vec<PrivacyProductionSdkParityArtifactV1>,
    required_state: Vec<&'static str>,
    fuzz_artifact_hash: &'static str,
    performance_artifact_hash: &'static str,
    localnet_acceptance: PrivacyProductionLocalnetEvidenceV1,
    gate_evidence: Vec<PrivacyProductionGateEvidenceV1>,
}

fn privacy_production_gate_requirement_is_waived(entry: &PrivacyAlgorithmEntry, key: &str) -> bool {
    entry.id == "transparent-transfer"
        && PRIVACY_TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS.contains(&key)
}

fn privacy_required_production_gate_keys(entry: &PrivacyAlgorithmEntry) -> Vec<String> {
    PRIVACY_PRODUCTION_GATE_REQUIREMENTS
        .iter()
        .filter(|(key, _)| !privacy_production_gate_requirement_is_waived(entry, key))
        .map(|(key, _)| (*key).to_owned())
        .collect()
}

fn privacy_expected_verifier_key_id(entry: &PrivacyAlgorithmEntry) -> &'static str {
    match entry.id {
        "transparent-transfer" => "none",
        "shield" => "zk::Shield",
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
        _ => "",
    }
}

fn privacy_expected_public_inputs_schema(entry: &PrivacyAlgorithmEntry) -> Option<&'static str> {
    match entry.id {
        "shield" => Some("asset,from,amount,note_commitment"),
        "confidential-transfer-v2" => Some(
            "input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,output_commitment_0,output_commitment_1,root,asset_tag,chain_tag",
        ),
        "unshield" => Some(
            "input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,change_commitment_0,root,public_amount,asset_tag,chain_tag",
        ),
        "asset-hidden-confidential-transfer-v1" => Some(
            "pool_id,asset_set_root,input_commitment_0,input_commitment_1,nullifier_0,nullifier_1,output_commitment_0,output_commitment_1,root,chain_tag",
        ),
        "zk-ace-pq-authorization-v0" => Some(
            "identity_commitment,tx_digest,chain_id,domain_separator,action_class,replay_nullifier,policy_hash,from,to,asset,amount,verifier_key_id",
        ),
        "anonymous-pgc-k-out-of-n-v1" => Some(
            "anonymity_set_root,tx_digest,balance_commitments,receiver_set_commitment,receiver_ciphertext_commitments,receiver_threshold,receiver_count,link_tag,range_commitments,chain_id,domain_separator",
        ),
        "verange-transparent-range-v1" => {
            Some("commitments,range_parameters,aggregation_count,domain_separator,payload_digest")
        }
        "zkat-policy-private-auth-v1" => Some(
            "policy_commitment,tx_digest,account_id,action_class,domain_separator,policy_epoch",
        ),
        "zk-ams-recursive-admission-v0" => Some(
            "issuer_root,admission_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_admission_digest,domain_separator",
        ),
        "vega-existing-credential-zk-v0" => Some(
            "issuer_commitment,credential_schema,predicate_commitment,subject_binding,expiration_epoch,domain_separator",
        ),
        "silent-threshold-anoncred-v0" => Some(
            "issuer_set_commitment,threshold_policy_hash,credential_showing_commitment,showing_nullifier,verifier_policy_hash,domain_separator",
        ),
        "zk-x509-onchain-identity-v0" => Some(
            "ca_root_commitment,certificate_policy_hash,revocation_root,subject_commitment,address_binding,domain_separator",
        ),
        "jindo-lattice-pcs-zk-v0" => {
            Some("commitment,opening_claim,query_set,parameter_hash,domain_separator")
        }
        "sis-hints-anoncred-pq-v0" => Some(
            "issuer_commitment,credential_commitment,showing_policy_hash,parameter_hash,domain_separator",
        ),
        "orchard-halo2-actions-v1" => {
            Some("anchor,nullifiers,cmx,value_commitments,binding_signature")
        }
        "penumbra-masp-v1" => Some(
            "state_commitment_anchor,nullifiers,note_commitments,balance_commitment,asset_id_commitment",
        ),
        "monero-fcmp-plus-plus-v1" => Some(
            "membership_root,key_image_or_link_tag,amount_commitments,range_commitments,spend_authorization,chain_tag",
        ),
        "miden-stark-note-v1" => Some(
            "account_id,initial_account_commitment,final_account_commitment,input_note_nullifiers,output_note_hashes,reference_block",
        ),
        "aztec-private-rollup-v1" => Some(
            "note_hashes,nullifiers,encrypted_logs,public_call_requests,private_kernel_commitment,rollup_state_roots",
        ),
        "pq-masp-stark-v0" => Some(
            "pool_id,asset_set_root,nullifier_set,output_commitments,root,chain_tag,pq_policy_hash",
        ),
        _ => None,
    }
}

fn privacy_expected_required_state(entry: &PrivacyAlgorithmEntry) -> &'static [&'static str] {
    match entry.id {
        "zk-ace-pq-authorization-v0" => &[
            "registered ZK-ACE identity commitment",
            "source-account allowlist",
            "authorization policy hash registry",
            "active ZK-ACE verifier key",
            "chain/domain binding state",
            "transfer digest binding",
            "replay nullifier uniqueness set",
            "identity rotation/revocation registry",
            "STARK/FRI verifier parameter floors",
            "wallet identity witness and replay-secret store",
        ],
        "anonymous-pgc-k-out-of-n-v1" => &[
            "anonymous account commitment set",
            "recent anonymity-set roots",
            "spent link-tag set",
            "range-proof verifier parameters",
            "wallet account blinding and receiver recovery metadata",
        ],
        "verange-transparent-range-v1" => &[
            "range-proof verifier parameters",
            "VeRange verifier key registry",
            "range commitment domain separators",
            "maximum aggregation policy",
        ],
        "zkat-policy-private-auth-v1" => &[
            "policy commitment registry",
            "policy epoch state",
            "authorization replay guard",
            "authorization verifier registry",
            "wallet policy witness store",
        ],
        "zk-ams-recursive-admission-v0" => &[
            "issuer root registry",
            "admission nullifier set",
            "anonymous account commitment registry",
            "recursive verifier parameters",
            "recursive admission verifier key registry",
            "wallet admission witness store",
        ],
        "vega-existing-credential-zk-v0" => &[
            "credential issuer registry",
            "supported credential schema registry",
            "predicate registry",
            "revocation or expiration policy",
            "wallet credential predicate witness store",
            "credential predicate commitment registry",
            "credential predicate verifier key registry",
        ],
        "silent-threshold-anoncred-v0" => &[
            "threshold issuer registry",
            "credential parameter registry",
            "verifier policy registry",
            "credential showing nullifier policy",
            "wallet credential showing witness store",
            "credential showing commitment registry",
            "anonymous credential verifier key registry",
        ],
        "zk-x509-onchain-identity-v0" => &[
            "trusted CA root registry",
            "certificate policy registry",
            "revocation root registry",
            "identity proof verifier",
            "wallet certificate witness store",
            "certificate subject commitment registry",
            "ZK-X.509 verifier key registry",
        ],
        "jindo-lattice-pcs-zk-v0" => &[
            "lattice PCS parameter registry",
            "backend verifier implementation",
            "lattice PCS verifier key registry",
            "production benchmark vectors",
        ],
        "sis-hints-anoncred-pq-v0" => &[
            "lattice credential parameter registry",
            "issuer parameter registry",
            "credential showing verifier",
            "wallet lattice credential witness store",
            "lattice credential commitment registry",
            "lattice credential verifier key registry",
        ],
        "orchard-halo2-actions-v1" => &[
            "Orchard note commitment tree",
            "Orchard nullifier set",
            "Orchard action-bundle verifier key registry",
            "wallet Orchard witness store",
        ],
        "penumbra-masp-v1" => &[
            "multi-asset state commitment tree",
            "typed nullifier set",
            "Groth16 spend/output verifier key registry",
            "wallet asset metadata witness store",
        ],
        "monero-fcmp-plus-plus-v1" => &[
            "full-output-set commitment accumulator",
            "spent link-tag set",
            "FCMP++ verifier key registry",
            "wallet output ownership scan state",
        ],
        "miden-stark-note-v1" => &[
            "private note hash database",
            "input note nullifier set",
            "account commitment state",
            "STARK VM verifier key registry",
            "wallet private note witness store",
        ],
        "aztec-private-rollup-v1" => &[
            "private note-hash tree",
            "nullifier tree",
            "encrypted log delivery store",
            "private-kernel verifier key registry",
            "wallet private execution witness store",
        ],
        "pq-masp-stark-v0" => &[
            "PQ MASP asset-set commitment root",
            "PQ nullifier set",
            "ML-KEM encrypted note payload store",
            "wallet PQ note witness store",
        ],
        _ => &[],
    }
}

fn privacy_expected_production_sdk_entrypoints(entry: &PrivacyAlgorithmEntry) -> Vec<String> {
    entry
        .sdk_entrypoints
        .iter()
        .chain(entry.planned_entrypoints.iter())
        .filter(|entrypoint| {
            !privacy_entrypoint_is_dev_fixture(entrypoint)
                && !privacy_entrypoint_is_local_verifier(entrypoint)
        })
        .fold(Vec::new(), |mut acc, entrypoint| {
            if !acc.iter().any(|existing| existing == entrypoint) {
                acc.push((*entrypoint).to_owned());
            }
            acc
        })
}

fn privacy_evidence_public_text_is_clean(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.trim() == value
        && value
            .bytes()
            .all(|byte| matches!(byte, 0x20..=0x7e) && byte != b'\\')
}

fn privacy_evidence_text_has_non_production_marker(value: &str) -> bool {
    let compact = privacy_compact_ascii_lowercase(value);
    compact.contains("devfixture")
        || compact.contains("devprooffixture")
        || compact.contains("localonly")
        || compact.contains("mock")
}

fn privacy_production_evidence_hash_is_valid(value: &str) -> bool {
    let Some(digest) = value.strip_prefix(PRIVACY_PRODUCTION_EVIDENCE_HASH_PREFIX) else {
        return false;
    };

    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn privacy_production_review_signature_is_valid(value: &str) -> bool {
    let Some(signature) = value.strip_prefix("ed25519:") else {
        return false;
    };

    privacy_evidence_public_text_is_clean(value, 512)
        && !privacy_evidence_text_has_non_production_marker(value)
        && signature.len() == 128
        && signature
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn privacy_production_localnet_run_id_is_valid(value: &str) -> bool {
    if !privacy_evidence_public_text_is_clean(value, 160)
        || privacy_evidence_text_has_non_production_marker(value)
        || value.contains("..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        return false;
    }

    let compact = value.replace('_', "-").to_ascii_lowercase();
    compact.contains("4-peer") || compact.contains("4peer")
}

fn privacy_production_localnet_peer_ids_are_valid(peer_ids: &[&str; 4]) -> bool {
    for (index, peer_id) in peer_ids.iter().enumerate() {
        if !privacy_evidence_public_text_is_clean(peer_id, 160)
            || privacy_evidence_text_has_non_production_marker(peer_id)
            || peer_id.contains("..")
            || !peer_id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-' | b'@')
            })
            || peer_ids[(index + 1)..]
                .iter()
                .any(|candidate| candidate == peer_id)
        {
            return false;
        }
    }
    true
}

fn privacy_production_localnet_artifact_hashes_are_valid(
    acceptance: PrivacyProductionLocalnetEvidenceV1,
) -> bool {
    let hashes = [
        acceptance.smoke_tx_hash,
        acceptance.replay_rejection_hash,
        acceptance.restart_replay_rejection_hash,
        acceptance.state_recovery_hash,
    ];
    for (index, hash) in hashes.iter().enumerate() {
        if !privacy_production_evidence_hash_is_valid(hash)
            || hashes[(index + 1)..]
                .iter()
                .any(|candidate| candidate == hash)
        {
            return false;
        }
    }
    true
}

fn privacy_production_localnet_evidence_is_valid(
    acceptance: PrivacyProductionLocalnetEvidenceV1,
    expected_chain_id: &str,
) -> bool {
    privacy_production_localnet_run_id_is_valid(acceptance.run_id)
        && acceptance.target == PRIVACY_PRODUCTION_LOCALNET_TARGET
        && acceptance.peer_count == PRIVACY_PRODUCTION_LOCALNET_PEER_COUNT
        && privacy_production_localnet_peer_ids_are_valid(&acceptance.peer_ids)
        && acceptance.chain_id == expected_chain_id
        && privacy_text_field_is_portable_identifier(acceptance.chain_id)
        && !privacy_evidence_text_has_non_production_marker(acceptance.chain_id)
        && acceptance.smoke_passed
        && acceptance.replay_rejected
        && acceptance.restart_persistence_checked
        && acceptance.restart_replay_rejected
        && acceptance.state_recovery_passed
        && privacy_production_localnet_artifact_hashes_are_valid(acceptance)
}

fn privacy_string_slice_matches_vec(values: &[&'static str], expected: &[String]) -> bool {
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected.iter())
            .all(|(value, expected)| *value == expected.as_str())
}

fn privacy_string_slice_matches_slice(values: &[&'static str], expected: &[&'static str]) -> bool {
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected.iter())
            .all(|(value, expected)| *value == *expected)
}

fn privacy_production_gate_evidence_has_duplicate_keys(
    evidence: &[PrivacyProductionGateEvidenceV1],
) -> bool {
    evidence.iter().enumerate().any(|(index, gate)| {
        evidence[index + 1..]
            .iter()
            .any(|other| other.key == gate.key)
    })
}

fn privacy_production_gate_evidence_is_valid(
    entry: &PrivacyAlgorithmEntry,
    evidence: &[PrivacyProductionGateEvidenceV1],
) -> bool {
    let required_gates = privacy_required_production_gate_keys(entry);
    evidence.len() == required_gates.len()
        && !privacy_production_gate_evidence_has_duplicate_keys(evidence)
        && evidence
            .iter()
            .zip(required_gates.iter())
            .all(|(gate, expected_key)| {
                gate.key == expected_key
                    && privacy_text_field_is_portable_identifier(gate.key)
                    && privacy_production_gate_key_is_required(gate.key)
                    && privacy_production_evidence_hash_is_valid(gate.artifact_hash)
            })
}

fn privacy_production_evidence_sdk_entrypoints_are_valid(
    row: &PrivacyProductionEvidenceRowV1,
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    let expected_entrypoints = privacy_expected_production_sdk_entrypoints(entry);
    privacy_string_slice_matches_vec(&row.sdk_entrypoints, &expected_entrypoints)
        && !privacy_string_slice_has_duplicates(&row.sdk_entrypoints)
        && row.sdk_entrypoints.iter().all(|entrypoint| {
            privacy_sdk_entrypoint_is_portable(entrypoint)
                && !privacy_entrypoint_is_dev_fixture(entrypoint)
                && !privacy_entrypoint_is_local_verifier(entrypoint)
                && !privacy_exposed_label_claims_production_readiness(entrypoint)
        })
}

fn privacy_production_evidence_sdk_exports_are_valid(
    row: &PrivacyProductionEvidenceRowV1,
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    let expected_entrypoints = privacy_expected_production_sdk_entrypoints(entry);
    row.sdk_exports.len() == PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES.len()
        && PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES
            .iter()
            .enumerate()
            .all(|(index, expected_surface)| {
                let Some(export) = row.sdk_exports.get(index) else {
                    return false;
                };
                export.surface == *expected_surface
                    && privacy_text_field_is_portable_identifier(export.surface)
                    && !privacy_evidence_text_has_non_production_marker(export.surface)
                    && !privacy_exposed_label_claims_production_readiness(export.surface)
                    && privacy_string_slice_matches_vec(&export.entrypoints, &expected_entrypoints)
                    && !privacy_string_slice_has_duplicates(&export.entrypoints)
                    && export.entrypoints.iter().all(|entrypoint| {
                        privacy_sdk_entrypoint_is_portable(entrypoint)
                            && !privacy_entrypoint_is_dev_fixture(entrypoint)
                            && !privacy_entrypoint_is_local_verifier(entrypoint)
                            && !privacy_exposed_label_claims_production_readiness(entrypoint)
                    })
            })
}

fn privacy_production_evidence_sdk_parity_artifacts_are_valid(
    row: &PrivacyProductionEvidenceRowV1,
) -> bool {
    row.sdk_parity_artifacts.len()
        == PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS.len()
            * PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES.len()
        && PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS
            .iter()
            .enumerate()
            .all(|(kind_index, expected_kind)| {
                PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES
                    .iter()
                    .enumerate()
                    .all(|(surface_index, expected_surface)| {
                        let artifact_index = kind_index
                            * PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES.len()
                            + surface_index;
                        let Some(artifact) = row.sdk_parity_artifacts.get(artifact_index) else {
                            return false;
                        };
                        artifact.kind == *expected_kind
                            && artifact.surface == *expected_surface
                            && privacy_text_field_is_portable_identifier(artifact.kind)
                            && privacy_text_field_is_portable_identifier(artifact.surface)
                            && !privacy_evidence_text_has_non_production_marker(artifact.kind)
                            && !privacy_evidence_text_has_non_production_marker(artifact.surface)
                            && !privacy_exposed_label_claims_production_readiness(artifact.kind)
                            && !privacy_exposed_label_claims_production_readiness(artifact.surface)
                            && privacy_production_evidence_hash_is_valid(artifact.artifact_hash)
                    })
            })
}

fn privacy_production_evidence_required_state_is_valid(
    row: &PrivacyProductionEvidenceRowV1,
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    privacy_string_slice_matches_slice(&row.required_state, privacy_expected_required_state(entry))
        && !privacy_string_slice_has_duplicates(&row.required_state)
        && row
            .required_state
            .iter()
            .all(|item| privacy_evidence_public_text_is_clean(item, 256))
}

fn privacy_production_review_scope_is_valid(
    row: &PrivacyProductionEvidenceRowV1,
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    row.review_scope.version == PRIVACY_PRODUCTION_REVIEW_SCOPE_VERSION
        && row.review_scope.algorithm_id == row.algorithm_id
        && row.review_scope.algorithm_id == entry.id
        && row.review_scope.chain_id == row.chain_id
        && row.review_scope.verifier_key_id == row.verifier_key_id
        && row.review_scope.proof_family == row.proof_family
        && row.review_scope.public_inputs_schema == row.public_inputs_schema
        && privacy_string_slice_matches_slice(
            &row.review_scope.sdk_entrypoints,
            &row.sdk_entrypoints,
        )
        && privacy_string_slice_matches_slice(&row.review_scope.required_state, &row.required_state)
        && row.review_scope.fuzz_artifact_hash == row.fuzz_artifact_hash
        && row.review_scope.performance_artifact_hash == row.performance_artifact_hash
        && row.review_scope.localnet_run_id == row.localnet_acceptance.run_id
        && privacy_production_evidence_hash_is_valid(row.review_scope.fuzz_artifact_hash)
        && privacy_production_evidence_hash_is_valid(row.review_scope.performance_artifact_hash)
        && privacy_production_localnet_run_id_is_valid(row.review_scope.localnet_run_id)
}

fn privacy_production_evidence_row_is_valid(
    row: &PrivacyProductionEvidenceRowV1,
    entry: &PrivacyAlgorithmEntry,
    chain_id: Option<&str>,
) -> bool {
    let Some(expected_chain_id) = chain_id else {
        return false;
    };

    row.algorithm_id == entry.id
        && row.chain_id == expected_chain_id
        && privacy_text_field_is_portable_identifier(row.chain_id)
        && !privacy_evidence_text_has_non_production_marker(row.chain_id)
        && !privacy_exposed_label_claims_production_readiness(row.algorithm_id)
        && privacy_evidence_public_text_is_clean(row.reviewer_identity, 160)
        && !privacy_evidence_text_has_non_production_marker(row.reviewer_identity)
        && privacy_production_evidence_hash_is_valid(row.review_artifact_hash)
        && privacy_production_review_signature_is_valid(row.review_artifact_signature)
        && privacy_production_review_scope_is_valid(row, entry)
        && row.verifier_key_id == privacy_expected_verifier_key_id(entry)
        && privacy_text_field_is_portable_identifier(row.verifier_key_id)
        && row.proof_family == entry.proof_family
        && row.public_inputs_schema == privacy_expected_public_inputs_schema(entry)
        && privacy_production_evidence_sdk_entrypoints_are_valid(row, entry)
        && privacy_production_evidence_sdk_exports_are_valid(row, entry)
        && privacy_production_evidence_sdk_parity_artifacts_are_valid(row)
        && privacy_production_evidence_required_state_is_valid(row, entry)
        && privacy_production_evidence_hash_is_valid(row.fuzz_artifact_hash)
        && privacy_production_evidence_hash_is_valid(row.performance_artifact_hash)
        && privacy_production_localnet_evidence_is_valid(row.localnet_acceptance, expected_chain_id)
        && privacy_production_gate_evidence_is_valid(entry, &row.gate_evidence)
}

fn privacy_production_evidence_for_entry<'a>(
    entry: &PrivacyAlgorithmEntry,
    evidence: &'a [PrivacyProductionEvidenceRowV1],
    chain_id: Option<&str>,
) -> Option<&'a PrivacyProductionEvidenceRowV1> {
    let mut valid_row = None;
    for row in evidence.iter().filter(|row| row.algorithm_id == entry.id) {
        if !privacy_production_evidence_row_is_valid(row, entry, chain_id) || valid_row.is_some() {
            return None;
        }
        valid_row = Some(row);
    }
    valid_row
}

fn privacy_production_gate(entry: &PrivacyAlgorithmEntry) -> PrivacyProductionGateV1 {
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
        required_gates: privacy_required_production_gate_keys(entry),
        missing: PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .filter(|(key, _)| !privacy_production_gate_requirement_is_waived(entry, key))
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

fn privacy_production_gate_from_evidence(
    entry: &PrivacyAlgorithmEntry,
    evidence: &PrivacyProductionEvidenceRowV1,
) -> PrivacyProductionGateV1 {
    PrivacyProductionGateV1 {
        version: PRIVACY_PRODUCTION_GATE_VERSION.to_owned(),
        ready: true,
        gates: PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .map(|(key, _)| PrivacyProductionGateStatusV1 {
                key: (*key).to_owned(),
                passed: !privacy_production_gate_requirement_is_waived(entry, key),
            })
            .collect(),
        required_gates: privacy_required_production_gate_keys(entry),
        missing: Vec::new(),
        audit_references: vec![
            format!("chain_id:{}", evidence.chain_id),
            format!("reviewer:{}", evidence.reviewer_identity),
            format!("review_artifact_hash:{}", evidence.review_artifact_hash),
            format!(
                "review_artifact_signature:{}",
                evidence.review_artifact_signature
            ),
            format!("fuzz_artifact_hash:{}", evidence.fuzz_artifact_hash),
            format!(
                "performance_artifact_hash:{}",
                evidence.performance_artifact_hash
            ),
            format!("localnet_run_id:{}", evidence.localnet_acceptance.run_id),
        ],
    }
}

fn privacy_capability_from_entry(
    entry: &PrivacyAlgorithmEntry,
    evidence: Option<&PrivacyProductionEvidenceRowV1>,
) -> PrivacyCapabilityV1 {
    if let Some(evidence) = evidence {
        return PrivacyCapabilityV1 {
            algorithm_id: entry.id.to_owned(),
            proof_family: entry.proof_family.to_owned(),
            backend_family: entry.backend_family.to_owned(),
            sdk_entrypoints: privacy_expected_production_sdk_entrypoints(entry),
            planned_entrypoints: Vec::new(),
            production_ready: true,
            production_gate: privacy_production_gate_from_evidence(entry, evidence),
        };
    }

    PrivacyCapabilityV1 {
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
        production_gate: privacy_production_gate(entry),
    }
}

fn privacy_capabilities_with_production_evidence(
    evidence: &[PrivacyProductionEvidenceRowV1],
    chain_id: Option<&str>,
) -> PrivacyCapabilitiesV1 {
    debug_assert!(privacy_algorithm_catalog_invariants_hold());
    let capabilities = PrivacyCapabilitiesV1 {
        version: PRIVACY_FFI_VERSION_V1,
        gate_version: PRIVACY_PRODUCTION_GATE_VERSION.to_owned(),
        algorithms: PRIVACY_ALGORITHM_ENTRIES
            .iter()
            .map(|entry| {
                privacy_capability_from_entry(
                    entry,
                    privacy_production_evidence_for_entry(entry, evidence, chain_id),
                )
            })
            .collect(),
    };
    debug_assert!(privacy_capabilities_invariants_hold(&capabilities));
    capabilities
}

fn privacy_capabilities() -> PrivacyCapabilitiesV1 {
    privacy_capabilities_with_production_evidence(&[], None)
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

fn privacy_compact_ascii_lowercase(value: &str) -> String {
    value
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric())
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect()
}

fn privacy_entrypoint_compact_lowercase(entrypoint: &str) -> String {
    privacy_compact_ascii_lowercase(entrypoint)
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

fn privacy_entrypoint_is_production_proof_verifier(entrypoint: &str) -> bool {
    let name = privacy_entrypoint_name(entrypoint);
    name.starts_with("verify")
        && (name.contains("Proof") || name.contains("Commitment"))
        && !privacy_entrypoint_is_instruction_builder(entrypoint)
        && !privacy_entrypoint_is_ledger_mutation(entrypoint)
        && !privacy_entrypoint_is_dev_fixture(entrypoint)
        && !privacy_entrypoint_is_local_verifier(entrypoint)
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
        && (!has_explicit_dev_fixture || has_production_proof_builder)
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
                        && (privacy_entrypoints_include_production_proof_builder(
                            entry.sdk_entrypoints,
                        ) || privacy_entrypoints_include_production_proof_builder(
                            entry.planned_entrypoints,
                        ))
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

fn privacy_string_vec_matches_vec(values: &[String], expected: &[String]) -> bool {
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected.iter())
            .all(|(value, expected)| value == expected)
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

fn privacy_gate_statuses_match_requirements(
    gates: &[PrivacyProductionGateStatusV1],
    entry: &PrivacyAlgorithmEntry,
    ready: bool,
) -> bool {
    gates.len() == PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len()
        && gates
            .iter()
            .zip(PRIVACY_PRODUCTION_GATE_REQUIREMENTS.iter())
            .all(|(status, (key, _))| {
                status.key.as_str() == *key
                    && status.passed
                        == (ready && !privacy_production_gate_requirement_is_waived(entry, key))
            })
}

fn privacy_required_gate_keys_match_entry(
    required_gates: &[String],
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    let expected = privacy_required_production_gate_keys(entry);
    required_gates.len() == expected.len()
        && required_gates
            .iter()
            .zip(expected.iter())
            .all(|(required, expected)| required == expected)
}

fn privacy_gate_missing_reasons_match_requirements(
    missing: &[String],
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    let required_requirements = PRIVACY_PRODUCTION_GATE_REQUIREMENTS
        .iter()
        .filter(|(key, _)| !privacy_production_gate_requirement_is_waived(entry, key));
    let required_count = required_requirements.clone().count();

    missing.len() == required_count + 2
        && missing
            .iter()
            .take(required_count)
            .zip(required_requirements)
            .all(|(missing, (_, label))| missing.as_str() == *label)
        && missing[required_count].as_str() == PRIVACY_PRODUCTION_GATE_MISSING_ENGINE
        && missing[required_count + 1].as_str() == PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST
}

fn privacy_ready_gate_audit_references_are_valid(audit_references: &[String]) -> bool {
    if audit_references.len() != 7
        || privacy_string_vec_has_duplicates(audit_references)
        || !audit_references
            .iter()
            .all(|reference| privacy_evidence_public_text_is_clean(reference, 768))
        || audit_references
            .iter()
            .any(|reference| privacy_evidence_text_has_non_production_marker(reference))
    {
        return false;
    }

    let Some(chain_id) = audit_references[0].strip_prefix("chain_id:") else {
        return false;
    };
    let Some(reviewer) = audit_references[1].strip_prefix("reviewer:") else {
        return false;
    };
    let Some(review_hash) = audit_references[2].strip_prefix("review_artifact_hash:") else {
        return false;
    };
    let Some(review_signature) = audit_references[3].strip_prefix("review_artifact_signature:")
    else {
        return false;
    };
    let Some(fuzz_hash) = audit_references[4].strip_prefix("fuzz_artifact_hash:") else {
        return false;
    };
    let Some(performance_hash) = audit_references[5].strip_prefix("performance_artifact_hash:")
    else {
        return false;
    };
    let Some(localnet_run_id) = audit_references[6].strip_prefix("localnet_run_id:") else {
        return false;
    };

    privacy_text_field_is_portable_identifier(chain_id)
        && !privacy_evidence_text_has_non_production_marker(chain_id)
        && privacy_evidence_public_text_is_clean(reviewer, 160)
        && !privacy_evidence_text_has_non_production_marker(reviewer)
        && privacy_production_evidence_hash_is_valid(review_hash)
        && privacy_production_review_signature_is_valid(review_signature)
        && privacy_production_evidence_hash_is_valid(fuzz_hash)
        && privacy_production_evidence_hash_is_valid(performance_hash)
        && privacy_production_localnet_run_id_is_valid(localnet_run_id)
}

fn privacy_production_gate_invariants_hold(
    gate: &PrivacyProductionGateV1,
    entry: &PrivacyAlgorithmEntry,
) -> bool {
    let required_gate_count = privacy_required_production_gate_keys(entry).len();

    if !(gate.version == PRIVACY_PRODUCTION_GATE_VERSION
        && gate.gates.len() == PRIVACY_PRODUCTION_GATE_REQUIREMENTS.len()
        && gate.required_gates.len() == required_gate_count
        && privacy_gate_statuses_match_requirements(&gate.gates, entry, gate.ready)
        && privacy_required_gate_keys_match_entry(&gate.required_gates, entry)
        && !privacy_gate_status_keys_have_duplicates(&gate.gates)
        && !privacy_string_vec_has_duplicates(&gate.required_gates)
        && gate.gates.iter().all(|status| {
            privacy_text_field_is_portable_identifier(&status.key)
                && privacy_production_gate_key_is_required(&status.key)
        })
        && gate.required_gates.iter().all(|key| {
            privacy_text_field_is_portable_identifier(key)
                && privacy_production_gate_key_is_required(key)
        }))
    {
        return false;
    }

    if gate.ready {
        return gate.missing.is_empty()
            && privacy_ready_gate_audit_references_are_valid(&gate.audit_references)
            && PRIVACY_PRODUCTION_GATE_REQUIREMENTS
                .iter()
                .filter(|(key, _)| !privacy_production_gate_requirement_is_waived(entry, key))
                .all(|(key, _)| {
                    gate.required_gates
                        .iter()
                        .any(|required| required.as_str() == *key)
                        && gate
                            .gates
                            .iter()
                            .any(|status| status.key.as_str() == *key && status.passed)
                })
            && PRIVACY_PRODUCTION_GATE_REQUIREMENTS
                .iter()
                .filter(|(key, _)| privacy_production_gate_requirement_is_waived(entry, key))
                .all(|(key, _)| {
                    !gate
                        .required_gates
                        .iter()
                        .any(|required| required.as_str() == *key)
                        && gate
                            .gates
                            .iter()
                            .any(|status| status.key.as_str() == *key && !status.passed)
                });
    }

    gate.audit_references.is_empty()
        && gate.missing.len() == required_gate_count + 2
        && privacy_gate_missing_reasons_match_requirements(&gate.missing, entry)
        && !privacy_string_vec_has_duplicates(&gate.missing)
        && gate
            .missing
            .iter()
            .all(|missing| privacy_production_gate_missing_reason_is_required(missing))
        && PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .filter(|(key, _)| !privacy_production_gate_requirement_is_waived(entry, key))
            .all(|(key, label)| {
                gate.required_gates
                    .iter()
                    .any(|required| required.as_str() == *key)
                    && gate
                        .gates
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
    let production_entrypoints = privacy_expected_production_sdk_entrypoints(entry);
    let sdk_entrypoints_match = if capability.production_ready {
        capability.planned_entrypoints.is_empty()
            && privacy_string_vec_matches_vec(&capability.sdk_entrypoints, &production_entrypoints)
            && capability.sdk_entrypoints.iter().all(|entrypoint| {
                !privacy_entrypoint_is_dev_fixture(entrypoint)
                    && !privacy_entrypoint_is_local_verifier(entrypoint)
            })
    } else {
        privacy_string_vec_matches_slice(&capability.sdk_entrypoints, entry.sdk_entrypoints)
            && privacy_string_vec_matches_slice(
                &capability.planned_entrypoints,
                entry.planned_entrypoints,
            )
    };

    privacy_algorithm_id_is_portable(&capability.algorithm_id)
        && !privacy_exposed_label_claims_production_readiness(&capability.algorithm_id)
        && capability.proof_family.as_str() == entry.proof_family
        && privacy_proof_family_is_portable(&capability.proof_family)
        && !privacy_exposed_label_claims_production_readiness(&capability.proof_family)
        && capability.backend_family.as_str() == entry.backend_family
        && privacy_vk_ref_backend_family_is_portable(&capability.backend_family)
        && !privacy_exposed_label_claims_production_readiness(&capability.backend_family)
        && sdk_entrypoints_match
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
        && capability.production_ready == capability.production_gate.ready
        && privacy_production_gate_invariants_hold(&capability.production_gate, entry)
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

fn privacy_failure_result_without_vk_ref(
    error_code: u32,
    message: &str,
    request: &PrivacyProofRequestV1,
) -> PrivacyProofResultV1 {
    let mut sanitized = request.clone();
    sanitized.vk_ref.clear();
    sanitized.witness.clear();
    sanitized.proof.clear();
    privacy_failure_result(error_code, message, Some(&sanitized))
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

fn privacy_clear_request_byte_fields(request: &mut PrivacyProofRequestV1) {
    request.public_inputs.fill(0);
    request.witness.fill(0);
    request.proof.fill(0);
}

fn privacy_result_for_request(
    mut request: PrivacyProofRequestV1,
    operation: PrivacyProofOperationV1,
) -> PrivacyProofResultV1 {
    let result = (|| -> PrivacyProofResultV1 {
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
                Some(&request),
            );
        }

        if privacy_request_has_invalid_catalog_shape(&request) {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "privacy proof request algorithm_id and entrypoint must use catalog identifier shapes",
                None,
            );
        }

        let known_entry = privacy_algorithm_entry(&request.algorithm_id);
        if let Some(entry) = known_entry {
            if privacy_entrypoint_planned(entry, &request.entrypoint) {
                return privacy_failure_result_without_vk_ref(
                    PRIVACY_FFI_ERROR_INVALID_REQUEST,
                    "privacy proof request entrypoint is planned but not executable until the production gate passes",
                    &request,
                );
            }
        }

        if request.vk_ref.trim().is_empty() {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "privacy proof request must include non-empty vk_ref",
                Some(&request),
            );
        }

        if !privacy_vk_ref_is_well_formed(&request.vk_ref) {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "privacy proof request vk_ref must use backend:name with portable verifier-key components",
                None,
            );
        }

        let Some(entry) = known_entry else {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
                "unsupported privacy algorithm id",
                Some(&request),
            );
        };

        if !privacy_entrypoint_supported(entry, &request.entrypoint) {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "privacy proof request entrypoint is not registered for the algorithm",
                Some(&request),
            );
        }

        let production_builder =
            privacy_entrypoint_is_production_proof_builder(&request.entrypoint);
        let production_verifier =
            privacy_entrypoint_is_production_proof_verifier(&request.entrypoint);
        if operation == PrivacyProofOperationV1::Build && !production_builder {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "privacy proof build request entrypoint must be a production proof builder",
                Some(&request),
            );
        }

        if operation == PrivacyProofOperationV1::Verify
            && !production_builder
            && !production_verifier
        {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "privacy proof verify request entrypoint must be a production proof builder or verifier",
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
    })();
    privacy_clear_request_byte_fields(&mut request);
    result
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
        Err(()) => privacy_failure_result(
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
        normalized.fill(0);
        return Err(());
    }
    let decoded = norito::decode_from_bytes(&normalized).map_err(|_| ());
    normalized.fill(0);
    decoded
}

fn encode_privacy_archive<T>(value: &T, context: &str, schema_byte: u8) -> napi::Result<Buffer>
where
    T: norito::NoritoSerialize,
{
    let mut bytes = norito::to_bytes(value).map_err(|err| {
        napi::Error::new(napi::Status::GenericFailure, format!("{context}: {err}"))
    })?;
    if !privacy_patch_archive_repeated_schema_byte(&mut bytes, schema_byte) {
        bytes.fill(0);
        return Err(napi::Error::new(
            napi::Status::GenericFailure,
            format!("{context}: encoded privacy archive is missing a Norito schema slot"),
        ));
    }
    if bytes.len() > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES {
        bytes.fill(0);
        return Err(napi::Error::new(
            napi::Status::GenericFailure,
            format!(
                "{context}: encoded privacy archive exceeds {PRIVACY_NATIVE_ARCHIVE_MAX_BYTES} bytes"
            ),
        ));
    }
    Ok(Buffer::from(bytes))
}

#[napi]
/// Return Norito V1 privacy capability records from the native production gate.
pub fn privacy_capabilities_v1() -> napi::Result<Buffer> {
    encode_privacy_archive(
        &privacy_capabilities(),
        "encode privacy capabilities",
        PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE,
    )
}

#[napi]
#[allow(clippy::needless_pass_by_value)]
/// Encode a Norito V1 privacy proof request for the native build/verify FFI.
pub fn privacy_proof_request_v1(
    algorithm_id: String,
    entrypoint: String,
    vk_ref: String,
    public_inputs: Uint8Array,
    witness: Uint8Array,
    proof: Uint8Array,
) -> napi::Result<Buffer> {
    let mut request = PrivacyProofRequestV1 {
        algorithm_id,
        entrypoint,
        vk_ref,
        public_inputs: public_inputs.as_ref().to_vec(),
        witness: witness.as_ref().to_vec(),
        proof: proof.as_ref().to_vec(),
    };
    let encoded = encode_privacy_archive(
        &request,
        "encode privacy proof request",
        PRIVACY_REQUEST_SCHEMA_BYTE,
    );
    privacy_clear_request_byte_fields(&mut request);
    encoded
}

#[napi]
#[allow(clippy::needless_pass_by_value)]
/// Build a privacy proof through the native Rust engine, returning a Norito V1 result archive.
pub fn privacy_build_proof_v1(request_archive: Uint8Array) -> napi::Result<Buffer> {
    let result = privacy_result_for_request_archive(
        request_archive.as_ref(),
        PrivacyProofOperationV1::Build,
    );
    encode_privacy_archive(
        &result,
        "encode privacy proof build result",
        privacy_result_schema_byte(PrivacyProofOperationV1::Build),
    )
}

#[napi]
#[allow(clippy::needless_pass_by_value)]
/// Verify a privacy proof through the native Rust engine, returning a Norito V1 result archive.
pub fn privacy_verify_proof_v1(request_archive: Uint8Array) -> napi::Result<Buffer> {
    let result = privacy_result_for_request_archive(
        request_archive.as_ref(),
        PrivacyProofOperationV1::Verify,
    );
    encode_privacy_archive(
        &result,
        "encode privacy proof verify result",
        privacy_result_schema_byte(PrivacyProofOperationV1::Verify),
    )
}

/// Result of signing a transaction via the native helper.
#[napi(object)]
pub struct JsSignedTransaction {
    /// Norito-encoded signed transaction bytes.
    pub signed_transaction: Buffer,
    /// Canonical pipeline hash for the signed transaction.
    pub hash: Buffer,
}

/// Result of building an authority-free private Kaigi transaction entrypoint.
#[napi(object)]
pub struct JsPrivateKaigiTransactionEntrypoint {
    /// Norito-encoded transaction entrypoint bytes.
    pub transaction_entrypoint: Buffer,
    /// Canonical pipeline hash used by Torii status polling.
    pub hash: Buffer,
    /// Action hash bound into the fee-spend proof.
    pub action_hash: Buffer,
}

/// Result of building a private Kaigi confidential XOR fee-spend envelope.
#[napi(object)]
pub struct JsPrivateKaigiFeeSpendEnvelope {
    /// Asset definition that the confidential fee spend targets.
    pub asset_definition_id: String,
    /// Recent shielded Merkle root bound into the spend.
    pub anchor_root: Buffer,
    /// Consumed nullifiers for the fee spend.
    pub nullifiers: Vec<Buffer>,
    /// Output commitments created by the fee spend.
    pub output_commitments: Vec<Buffer>,
    /// Encrypted payloads attached to the output commitments.
    pub encrypted_change_payloads: Vec<Buffer>,
    /// Norito-encoded `OpenVerifyEnvelope` payload.
    pub proof: Buffer,
}

/// Input note material for confidential transfer/unshield proof construction.
#[napi(object)]
#[derive(Clone)]
pub struct JsConfidentialTransferInputV2 {
    /// Whole-number base-unit amount carried by the note.
    pub amount: String,
    /// Note rho rendered as 32-byte hexadecimal.
    pub rho_hex: String,
    /// Note diversifier rendered as 32-byte hexadecimal; omitted legacy notes use the default tag.
    pub diversifier_hex: Option<String>,
    /// Current note leaf index inside the confidential tree.
    pub leaf_index: u32,
}

/// Output note material for confidential transfer proof construction.
#[napi(object)]
#[derive(Clone)]
pub struct JsConfidentialTransferOutputV2 {
    /// Whole-number base-unit amount carried by the note.
    pub amount: String,
    /// Fresh note rho rendered as 32-byte hexadecimal.
    pub rho_hex: String,
    /// Recipient owner tag rendered as 32-byte hexadecimal.
    pub owner_tag_hex: String,
}

/// Output note material for confidential unshield v3 change-note construction.
#[napi(object)]
#[derive(Clone)]
pub struct JsConfidentialUnshieldOutputV3 {
    /// Whole-number base-unit amount carried by the note.
    pub amount: String,
    /// Fresh note rho rendered as 32-byte hexadecimal.
    pub rho_hex: String,
}

/// Diversified confidential v2 payment address material.
#[napi(object)]
pub struct JsConfidentialReceiveAddressV2 {
    /// Recipient owner tag rendered as 32-byte hexadecimal.
    pub owner_tag_hex: String,
    /// Note diversifier rendered as 32-byte hexadecimal.
    pub diversifier_hex: String,
}

/// Result of building a confidential transfer v2 proof envelope.
#[napi(object)]
pub struct JsConfidentialTransferProofEnvelopeV2 {
    /// Nullifiers consumed by the proof.
    pub nullifiers: Vec<Buffer>,
    /// Output commitments created by the proof.
    pub output_commitments: Vec<Buffer>,
    /// Merkle root bound into the proof.
    pub root: Buffer,
    /// Norito-encoded `OpenVerifyEnvelope` payload.
    pub proof: Buffer,
}

/// Result of building an asset-hidden transfer v1 proof envelope.
#[napi(object)]
pub struct JsAssetHiddenTransferProofEnvelopeV1 {
    /// Input commitments bound into the proof.
    pub input_commitments: Vec<Buffer>,
    /// Nullifiers consumed by the proof.
    pub nullifiers: Vec<Buffer>,
    /// Output commitments created by the proof.
    pub output_commitments: Vec<Buffer>,
    /// Merkle root bound into the proof.
    pub root: Buffer,
    /// Norito-encoded `OpenVerifyEnvelope` payload.
    pub proof: Buffer,
}

/// Result of building a confidential unshield v2 proof envelope.
#[napi(object)]
pub struct JsConfidentialUnshieldProofEnvelopeV2 {
    /// Nullifiers consumed by the proof.
    pub nullifiers: Vec<Buffer>,
    /// Merkle root bound into the proof.
    pub root: Buffer,
    /// Norito-encoded `OpenVerifyEnvelope` payload.
    pub proof: Buffer,
}

/// Result of building a confidential unshield v3 proof envelope.
#[napi(object)]
pub struct JsConfidentialUnshieldProofEnvelopeV3 {
    /// Nullifiers consumed by the proof.
    pub nullifiers: Vec<Buffer>,
    /// Output commitments created by the proof.
    pub output_commitments: Vec<Buffer>,
    /// Merkle root bound into the proof.
    pub root: Buffer,
    /// Norito-encoded `OpenVerifyEnvelope` payload.
    pub proof: Buffer,
}

fn parse_private_kaigi_json<T>(context: &str, payload: &str) -> napi::Result<T>
where
    T: JsonDeserialize,
{
    json::from_json(payload).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid {context} json: {err}"),
        )
    })
}

fn parse_kaigi_id_literal(value: &str, context: &str) -> napi::Result<KaigiId> {
    let trimmed = value.trim();
    let Some((domain, call_name)) = trimmed.split_once(':') else {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be in `domain.dataspace:callName` format"),
        ));
    };
    let domain_id = DomainId::parse_fully_qualified(domain).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid {context} domain id: {err}"),
        )
    })?;
    let call_name = Name::from_str(call_name).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid {context} call name: {err}"),
        )
    })?;
    Ok(KaigiId::new(domain_id, call_name))
}

fn normalize_private_kaigi_creation_time_ms(creation_time_ms: Option<i64>) -> napi::Result<u64> {
    creation_time_ms.map_or_else(
        || {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
                .map_err(norito_to_napi)
        },
        |ms| {
            u64::try_from(ms).map_err(|_| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    "creation_time_ms must be non-negative",
                )
            })
        },
    )
}

fn validate_private_kaigi_fee_fixture(
    vk_backend: &str,
    vk_circuit_id: &str,
    vk_bytes: &[u8],
) -> napi::Result<Vec<u8>> {
    if vk_backend != "halo2/ipa" {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "unsupported private Kaigi fee transfer verifier backend `{vk_backend}`; expected halo2/ipa"
            ),
        ));
    }
    if vk_bytes.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "vk_bytes must be present for private Kaigi fee spend construction",
        ));
    }

    let network_vk =
        iroha_data_model::proof::VerifyingKeyBox::new(vk_backend.to_owned(), vk_bytes.to_vec());
    let fixture = halo2_fixture_envelope(
        vk_circuit_id.to_owned(),
        hash_verifying_key_box(&network_vk),
    );
    let fixture_vk_bytes = fixture.vk_bytes.ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("unsupported private Kaigi fee verifier circuit `{vk_circuit_id}`"),
        )
    })?;
    if fixture_vk_bytes != vk_bytes {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "private Kaigi fee verifier `{vk_backend}::{vk_circuit_id}` does not match the built-in fixture circuit"
            ),
        ));
    }
    Ok(fixture.proof_bytes)
}

fn build_private_kaigi_fee_change_payload(
    asset_definition_id: &str,
    action_hash_hex: &str,
    fee_amount: &str,
) -> Vec<u8> {
    json::to_string(&norito_json!({
        "schema": "iroha.private_kaigi.change.v1",
        "asset_definition_id": asset_definition_id,
        "action_hash_hex": action_hash_hex,
        "fee_amount": fee_amount,
        "change_amount": "0",
    }))
    .expect("private Kaigi change payload JSON serialization")
    .into_bytes()
}

fn normalize_private_kaigi_fee_amount(fee_amount: &str) -> napi::Result<String> {
    let fee_amount = fee_amount.trim().to_owned();
    if fee_amount.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "fee_amount must be non-empty",
        ));
    }
    let _parsed_fee_amount = Numeric::from_str(&fee_amount).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid fee_amount numeric literal: {err}"),
        )
    })?;
    Ok(fee_amount)
}

fn normalize_private_kaigi_nonce(nonce: Option<u32>) -> napi::Result<Option<NonZeroU32>> {
    nonce
        .map(|value| {
            NonZeroU32::new(value).ok_or_else(|| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    "nonce must be non-zero (fits in u32)",
                )
            })
        })
        .transpose()
}

fn parse_fixed_32_hex(context: &str, value: &str) -> napi::Result<[u8; 32]> {
    let normalized = value.trim();
    let normalized = normalized.strip_prefix("0x").unwrap_or(normalized);
    let decoded = hex::decode(normalized).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be valid hex: {err}"),
        )
    })?;
    if decoded.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be exactly 32 bytes"),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&decoded);
    Ok(out)
}

fn parse_confidential_amount_u128(context: &str, value: &str) -> napi::Result<u128> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("{context} must be a non-empty whole number"),
        ));
    }
    normalized.parse::<u128>().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid {context} whole-number amount: {err}"),
        )
    })
}

fn parse_confidential_tree_commitments(
    commitments_hex: Vec<String>,
) -> napi::Result<Vec<[u8; 32]>> {
    parse_fixed_32_hex_list("tree_commitments_hex", commitments_hex)
}

fn parse_fixed_32_hex_list(context: &str, values: Vec<String>) -> napi::Result<Vec<[u8; 32]>> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| parse_fixed_32_hex(&format!("{context}[{index}]"), &value))
        .collect()
}

fn parse_optional_confidential_diversifier_hex(
    context: &str,
    value: Option<&str>,
) -> napi::Result<[u8; 32]> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => parse_fixed_32_hex(context, value),
        None => Ok(confidential_v2::default_confidential_diversifier_v2()),
    }
}

fn parse_confidential_transfer_inputs_v2(
    inputs: Vec<JsConfidentialTransferInputV2>,
) -> napi::Result<Vec<ConfidentialTransferInputV2>> {
    inputs
        .into_iter()
        .enumerate()
        .map(|(index, input)| {
            Ok(ConfidentialTransferInputV2 {
                amount: parse_confidential_amount_u128(
                    &format!("inputs[{index}].amount"),
                    &input.amount,
                )?,
                rho: parse_fixed_32_hex(&format!("inputs[{index}].rho_hex"), &input.rho_hex)?,
                diversifier: parse_optional_confidential_diversifier_hex(
                    &format!("inputs[{index}].diversifier_hex"),
                    input.diversifier_hex.as_deref(),
                )?,
                leaf_index: usize::try_from(input.leaf_index).map_err(|_| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("inputs[{index}].leaf_index is out of range"),
                    )
                })?,
            })
        })
        .collect()
}

fn parse_confidential_unshield_inputs_v2(
    inputs: Vec<JsConfidentialTransferInputV2>,
) -> napi::Result<Vec<ConfidentialUnshieldInputV2>> {
    inputs
        .into_iter()
        .enumerate()
        .map(|(index, input)| {
            Ok(ConfidentialUnshieldInputV2 {
                amount: parse_confidential_amount_u128(
                    &format!("inputs[{index}].amount"),
                    &input.amount,
                )?,
                rho: parse_fixed_32_hex(&format!("inputs[{index}].rho_hex"), &input.rho_hex)?,
                diversifier: parse_optional_confidential_diversifier_hex(
                    &format!("inputs[{index}].diversifier_hex"),
                    input.diversifier_hex.as_deref(),
                )?,
                leaf_index: usize::try_from(input.leaf_index).map_err(|_| {
                    napi::Error::new(
                        napi::Status::InvalidArg,
                        format!("inputs[{index}].leaf_index is out of range"),
                    )
                })?,
            })
        })
        .collect()
}

fn parse_confidential_transfer_outputs_v2(
    outputs: Vec<JsConfidentialTransferOutputV2>,
) -> napi::Result<Vec<ConfidentialTransferOutputV2>> {
    outputs
        .into_iter()
        .enumerate()
        .map(|(index, output)| {
            Ok(ConfidentialTransferOutputV2 {
                amount: parse_confidential_amount_u128(
                    &format!("outputs[{index}].amount"),
                    &output.amount,
                )?,
                rho: parse_fixed_32_hex(&format!("outputs[{index}].rho_hex"), &output.rho_hex)?,
                owner_tag: parse_fixed_32_hex(
                    &format!("outputs[{index}].owner_tag_hex"),
                    &output.owner_tag_hex,
                )?,
            })
        })
        .collect()
}

fn parse_confidential_unshield_outputs_v3(
    outputs: Vec<JsConfidentialUnshieldOutputV3>,
) -> napi::Result<Vec<ConfidentialUnshieldOutputV3>> {
    outputs
        .into_iter()
        .enumerate()
        .map(|(index, output)| {
            Ok(ConfidentialUnshieldOutputV3 {
                amount: parse_confidential_amount_u128(
                    &format!("outputs[{index}].amount"),
                    &output.amount,
                )?,
                rho: parse_fixed_32_hex(&format!("outputs[{index}].rho_hex"), &output.rho_hex)?,
            })
        })
        .collect()
}

fn private_kaigi_fee_aux_json(
    action_hash_hex: &str,
    chain_id: &str,
    asset_definition_id: &str,
    fee_amount: &str,
) -> Vec<u8> {
    json::to_string(&norito_json!({
        "schema": "iroha.private_kaigi.fee.v1",
        "action_hash_hex": action_hash_hex,
        "chain_id": chain_id,
        "asset_definition_id": asset_definition_id,
        "fee_amount": fee_amount,
    }))
    .expect("private Kaigi fee aux JSON serialization")
    .into_bytes()
}

fn build_private_kaigi_fee_digest(label: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label);
    for part in parts {
        hasher.update(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

fn build_private_kaigi_entrypoint_result(
    tx: PrivateKaigiTransaction,
) -> JsPrivateKaigiTransactionEntrypoint {
    let action_hash = tx.action_hash();
    let hash = tx.hash();
    let entrypoint = TransactionEntrypoint::PrivateKaigi(tx);
    let entrypoint_bytes = Encode::encode(&entrypoint);
    JsPrivateKaigiTransactionEntrypoint {
        transaction_entrypoint: Buffer::from(entrypoint_bytes),
        hash: Buffer::from(hash.as_ref().to_vec()),
        action_hash: Buffer::from(action_hash.as_ref().to_vec()),
    }
}

fn encode_private_kaigi_fee_proof(
    proof_bytes: &[u8],
    action_hash_hex: &str,
    chain_id: &str,
    asset_definition_id: &str,
    fee_amount: &str,
) -> napi::Result<Vec<u8>> {
    let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
        norito::decode_from_bytes(proof_bytes).map_err(|err| {
            napi::Error::new(
                napi::Status::GenericFailure,
                format!("failed to decode private Kaigi fee proof fixture: {err}"),
            )
        })?;
    envelope.aux =
        private_kaigi_fee_aux_json(action_hash_hex, chain_id, asset_definition_id, fee_amount);
    norito::to_bytes(&envelope).map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("failed to encode private Kaigi fee proof envelope: {err}"),
        )
    })
}

/// Build and sign a single-instruction `RegisterDomain` transaction.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)] // JS bindings expose this exact surface to callers
pub fn build_register_domain_transaction(
    chain_id: String,
    authority: String,
    domain_id: String,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    ttl_ms: Option<i64>,
    nonce: Option<u32>,
    secret: Uint8Array,
) -> napi::Result<JsSignedTransaction> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let authority = parse_account_id(&authority, "authority account id")?;
    let domain_id = DomainId::parse_fully_qualified(&domain_id).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid domain id: {err}"),
        )
    })?;

    let domain_metadata = parse_metadata_payload("domain", metadata_json)?;
    let new_domain = Domain::new(domain_id).with_metadata(domain_metadata);
    let instruction: InstructionBox = Register::<Domain>::domain(new_domain).into();
    assemble_transaction(
        chain_id,
        authority,
        vec![instruction],
        Metadata::default(),
        creation_time_ms,
        ttl_ms,
        nonce,
        secret.as_ref(),
    )
}

#[allow(clippy::too_many_arguments)] // helper mirrors the JS surface for clarity
fn build_transaction_from_instructions_json(
    chain_id: ChainId,
    authority: AccountId,
    instructions_json: Vec<String>,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    ttl_ms: Option<i64>,
    nonce: Option<u32>,
    secret: &[u8],
) -> napi::Result<JsSignedTransaction> {
    let instructions = parse_instruction_payloads(instructions_json)?;

    let metadata = parse_metadata_payload("transaction", metadata_json)?;
    assemble_transaction(
        chain_id,
        authority,
        instructions,
        metadata,
        creation_time_ms,
        ttl_ms,
        nonce,
        secret,
    )
}

/// Build and sign a transaction from an array of instruction JSON payloads.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)] // JS bindings expose this exact surface to callers
pub fn build_transaction(
    chain_id: String,
    authority: String,
    instructions_json: Vec<String>,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    ttl_ms: Option<i64>,
    nonce: Option<u32>,
    secret: Uint8Array,
) -> napi::Result<JsSignedTransaction> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let _chain_guard = scoped_chain_discriminant_for_literal(&authority);
    let authority = parse_account_id(&authority, "authority account id")?;

    build_transaction_from_instructions_json(
        chain_id,
        authority,
        instructions_json,
        metadata_json,
        creation_time_ms,
        ttl_ms,
        nonce,
        secret.as_ref(),
    )
}

/// Build and sign a transaction carrying an `Executable::IvmProved` payload and its proof attachment.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)] // JS bindings expose this exact surface to callers
pub fn build_ivm_proved_transaction(
    chain_id: String,
    authority: String,
    proved_json: String,
    attachment_json: String,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    ttl_ms: Option<i64>,
    nonce: Option<u32>,
    secret: Uint8Array,
) -> napi::Result<JsSignedTransaction> {
    let chain_id: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let _chain_guard = scoped_chain_discriminant_for_literal(&authority);
    let authority = parse_account_id(&authority, "authority account id")?;
    let proved: IvmProved = json::from_json(&proved_json).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid IvmProved json: {err}"),
        )
    })?;
    let attachment: ProofAttachment = json::from_json(&attachment_json).map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid ProofAttachment json: {err}"),
        )
    })?;
    let metadata = parse_metadata_payload("transaction", metadata_json)?;

    assemble_executable_transaction(
        chain_id,
        authority,
        Executable::IvmProved(proved),
        metadata,
        Some(ProofAttachmentList(vec![attachment])),
        creation_time_ms,
        ttl_ms,
        nonce,
        secret.as_ref(),
    )
}

/// Build a private Kaigi create transaction entrypoint.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn build_private_create_kaigi_transaction(
    chain_id: String,
    call_json: String,
    artifacts_json: String,
    fee_spend_json: String,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    nonce: Option<u32>,
) -> napi::Result<JsPrivateKaigiTransactionEntrypoint> {
    let chain: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let call: PrivateKaigiTemplate = parse_private_kaigi_json("private create call", &call_json)?;
    let artifacts: PrivateKaigiArtifacts =
        parse_private_kaigi_json("private Kaigi artifacts", &artifacts_json)?;
    let fee_spend: PrivateKaigiFeeSpend =
        parse_private_kaigi_json("private Kaigi fee spend", &fee_spend_json)?;
    let metadata = parse_metadata_payload("private Kaigi", metadata_json)?;
    let tx = PrivateKaigiTransaction {
        chain,
        creation_time_ms: normalize_private_kaigi_creation_time_ms(creation_time_ms)?,
        nonce: normalize_private_kaigi_nonce(nonce)?,
        metadata,
        action: PrivateKaigiAction::Create(PrivateCreateKaigi { call }),
        artifacts,
        fee_spend,
    };
    Ok(build_private_kaigi_entrypoint_result(tx))
}

/// Build a deterministic confidential XOR fee-spend envelope for private Kaigi.
///
/// This helper only supports transfer verifying keys whose circuit id matches one of the
/// built-in Halo2 fixture circuits. The caller must pass the active `vk_transfer` record bytes
/// advertised by the network so the helper can verify the local fixture matches the network VK.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn build_private_kaigi_fee_spend(
    chain_id: String,
    asset_definition_id: String,
    action_hash: Uint8Array,
    anchor_root_hex: String,
    fee_amount: String,
    vk_backend: String,
    vk_circuit_id: String,
    vk_bytes: Uint8Array,
) -> napi::Result<JsPrivateKaigiFeeSpendEnvelope> {
    let asset_definition_id: AssetDefinitionId = asset_definition_id.parse().map_err(|err| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("invalid asset definition id: {err}"),
        )
    })?;
    if action_hash.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "action_hash must be exactly 32 bytes",
        ));
    }
    let anchor_root = parse_fixed_32_hex("anchor_root_hex", &anchor_root_hex)?;
    let fee_amount = normalize_private_kaigi_fee_amount(&fee_amount)?;
    let vk_backend = vk_backend.trim();
    let vk_circuit_id = vk_circuit_id.trim();
    let proof_bytes =
        validate_private_kaigi_fee_fixture(vk_backend, vk_circuit_id, vk_bytes.as_ref())?;
    let asset_definition_string = asset_definition_id.to_string();
    let action_hash_hex = hex::encode(action_hash.as_ref());
    let nullifier = build_private_kaigi_fee_digest(
        b"iroha.private_kaigi.fee.nullifier.v1",
        &[
            action_hash.as_ref(),
            chain_id.as_bytes(),
            asset_definition_string.as_bytes(),
        ],
    );
    let output_commitment = build_private_kaigi_fee_digest(
        b"iroha.private_kaigi.fee.output.v1",
        &[
            action_hash.as_ref(),
            fee_amount.as_bytes(),
            anchor_root.as_slice(),
        ],
    );
    let encrypted_change_payload = build_private_kaigi_fee_change_payload(
        &asset_definition_string,
        &action_hash_hex,
        &fee_amount,
    );
    let encoded = encode_private_kaigi_fee_proof(
        &proof_bytes,
        &action_hash_hex,
        chain_id.trim(),
        &asset_definition_string,
        &fee_amount,
    )?;

    Ok(JsPrivateKaigiFeeSpendEnvelope {
        asset_definition_id: asset_definition_id.to_string(),
        anchor_root: Buffer::from(anchor_root.to_vec()),
        nullifiers: vec![Buffer::from(nullifier.to_vec())],
        output_commitments: vec![Buffer::from(output_commitment.to_vec())],
        encrypted_change_payloads: vec![Buffer::from(encrypted_change_payload)],
        proof: Buffer::from(encoded),
    })
}

/// Build a private Kaigi join transaction entrypoint.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn build_private_join_kaigi_transaction(
    chain_id: String,
    call_id: String,
    artifacts_json: String,
    fee_spend_json: String,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    nonce: Option<u32>,
) -> napi::Result<JsPrivateKaigiTransactionEntrypoint> {
    let chain: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let call_id = parse_kaigi_id_literal(&call_id, "call_id")?;
    let artifacts: PrivateKaigiArtifacts =
        parse_private_kaigi_json("private Kaigi artifacts", &artifacts_json)?;
    let fee_spend: PrivateKaigiFeeSpend =
        parse_private_kaigi_json("private Kaigi fee spend", &fee_spend_json)?;
    let metadata = parse_metadata_payload("private Kaigi", metadata_json)?;
    let tx = PrivateKaigiTransaction {
        chain,
        creation_time_ms: normalize_private_kaigi_creation_time_ms(creation_time_ms)?,
        nonce: normalize_private_kaigi_nonce(nonce)?,
        metadata,
        action: PrivateKaigiAction::Join(PrivateJoinKaigi { call_id }),
        artifacts,
        fee_spend,
    };
    Ok(build_private_kaigi_entrypoint_result(tx))
}

/// Build a private Kaigi end transaction entrypoint.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn build_private_end_kaigi_transaction(
    chain_id: String,
    call_id: String,
    ended_at_ms: Option<i64>,
    artifacts_json: String,
    fee_spend_json: String,
    metadata_json: Option<String>,
    creation_time_ms: Option<i64>,
    nonce: Option<u32>,
) -> napi::Result<JsPrivateKaigiTransactionEntrypoint> {
    let chain: ChainId = chain_id.parse().map_err(|err| {
        napi::Error::new(napi::Status::InvalidArg, format!("invalid chain id: {err}"))
    })?;
    let call_id = parse_kaigi_id_literal(&call_id, "call_id")?;
    let ended_at_ms = ended_at_ms
        .map(|value| {
            u64::try_from(value).map_err(|_| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    "ended_at_ms must be non-negative when provided",
                )
            })
        })
        .transpose()?;
    let artifacts: PrivateKaigiArtifacts =
        parse_private_kaigi_json("private Kaigi artifacts", &artifacts_json)?;
    let fee_spend: PrivateKaigiFeeSpend =
        parse_private_kaigi_json("private Kaigi fee spend", &fee_spend_json)?;
    let metadata = parse_metadata_payload("private Kaigi", metadata_json)?;
    let tx = PrivateKaigiTransaction {
        chain,
        creation_time_ms: normalize_private_kaigi_creation_time_ms(creation_time_ms)?,
        nonce: normalize_private_kaigi_nonce(nonce)?,
        metadata,
        action: PrivateKaigiAction::End(PrivateEndKaigi {
            call_id,
            ended_at_ms,
        }),
        artifacts,
        fee_spend,
    };
    Ok(build_private_kaigi_entrypoint_result(tx))
}

/// Build a Norito-encoded trigger action that executes on a time schedule.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn build_time_trigger_action(
    authority: String,
    instructions_json: Vec<String>,
    start_timestamp_ms: i64,
    period_ms: Option<i64>,
    repeats: Option<u32>,
    metadata_json: Option<String>,
) -> napi::Result<String> {
    let start_timestamp_ms = u64::try_from(start_timestamp_ms).map_err(|_| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "start_timestamp_ms must be a positive integer",
        )
    })?;
    if start_timestamp_ms == 0 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "start_timestamp_ms must be greater than zero",
        ));
    }
    let period_ms = if let Some(period) = period_ms {
        let as_u64 = u64::try_from(period).map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "period_ms must be a positive integer when provided",
            )
        })?;
        if as_u64 == 0 {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                "period_ms must be greater than zero when provided",
            ));
        }
        Some(as_u64)
    } else {
        None
    };

    let _chain_guard = scoped_chain_discriminant_for_literal(&authority);
    let authority = parse_account_id(&authority, "trigger authority")?;
    let instructions = parse_instruction_payloads(instructions_json)?;
    let executable = Executable::from(instructions);
    let repeats = match repeats {
        Some(0) => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                "repeats must be greater than zero when provided",
            ));
        }
        Some(value) => Repeats::Exactly(value),
        None => Repeats::Indefinitely,
    };
    let metadata = parse_metadata_payload("trigger", metadata_json)?;

    let mut schedule = TimeSchedule::starting_at(Duration::from_millis(start_timestamp_ms));
    if let Some(period) = period_ms {
        schedule = schedule.with_period(Duration::from_millis(period));
    }

    let action = Action::new(
        executable,
        repeats,
        authority,
        TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
    )
    .with_metadata(metadata);
    encode_trigger_action(&action)
}

/// Build a Norito-encoded trigger action that fires at the pre-commit stage.
#[napi]
#[allow(clippy::needless_pass_by_value)]
pub fn build_precommit_trigger_action(
    authority: String,
    instructions_json: Vec<String>,
    repeats: Option<u32>,
    metadata_json: Option<String>,
) -> napi::Result<String> {
    let _chain_guard = scoped_chain_discriminant_for_literal(&authority);
    let authority = parse_account_id(&authority, "trigger authority")?;
    let instructions = parse_instruction_payloads(instructions_json)?;
    let executable = Executable::from(instructions);
    let repeats = match repeats {
        Some(0) => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                "repeats must be greater than zero when provided",
            ));
        }
        Some(value) => Repeats::Exactly(value),
        None => Repeats::Indefinitely,
    };
    let metadata = parse_metadata_payload("trigger", metadata_json)?;

    let action = Action::new(
        executable,
        repeats,
        authority,
        TimeEventFilter::new(ExecutionTime::PreCommit),
    )
    .with_metadata(metadata);
    encode_trigger_action(&action)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Cursor,
        path::PathBuf,
        str::FromStr,
        sync::{Arc, OnceLock},
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        HasMetadata,
        account::AccountId,
        asset::id::{AssetDefinitionId, AssetId},
        da::{
            manifest::{ChunkCommitment, ChunkRole, DaManifestV1},
            types::{
                BlobClass, BlobCodec, BlobDigest, ErasureProfile, ExtraMetadata, MetadataEntry,
                MetadataVisibility, RetentionPolicy, StorageTicketId,
            },
        },
        domain::DomainId,
        events::EventFilterBox,
        isi::{
            Burn, BurnBox, CreateKaigi, CustomInstruction, InstructionBox, JoinKaigi, LeaveKaigi,
            Mint, MintBox, RecordKaigiUsage, RegisterBox, RegisterKaigiRelay, RegisterPeerWithPop,
            SetKaigiRelayManifest, Transfer, TransferBox, Unregister, UnregisterBox,
            governance::{
                AtWindow, CastPlainBallot, CastZkBallot, CouncilDerivationKind, EnactReferendum,
                FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract, RegisterCitizen,
                VotingMode,
            },
            smart_contract_code::{
                ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
            },
        },
        kaigi::{
            KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
            KaigiRelayHop, KaigiRelayManifest, KaigiRelayRegistration, KaigiRoomPolicy, NewKaigi,
        },
        metadata::Metadata,
        ministry::{
            AgendaEvidenceAttachment, AgendaEvidenceKind, AgendaProposalAction,
            AgendaProposalSubmitter, AgendaProposalSummary, AgendaProposalTarget, AgendaProposalV1,
        },
        name::Name,
        nexus::LaneId,
        nft::NftId,
        peer::{Peer, PeerId},
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        rwa::{NewRwa, RwaControlPolicy, RwaId, RwaParentRef},
        smart_contract::manifest::{
            AccessSetHints, ContractManifest, EntryPointKind, EntrypointDescriptor,
            EntrypointParamDescriptor, KotobaTranslation, KotobaTranslationEntry,
        },
        transaction::{
            Executable, IvmBytecode, IvmProved, TransactionSubmissionReceipt,
            TransactionSubmissionReceiptPayload,
        },
        trigger::TriggerId,
    };
    use norito::{
        NoritoDeserialize,
        codec::{Decode as NoritoDecode, Encode as NoritoEncode},
        from_bytes,
        json::{self, Value},
        to_bytes,
    };
    use sorafs_car::{
        CarBuildPlan, CarWriter, chunker_registry, fetch_plan::chunk_fetch_specs_to_string,
    };
    use sorafs_chunker::ChunkProfile;
    use sorafs_manifest::{
        ChunkingProfileV1, CouncilSignature, GovernanceProofs, ManifestBuilder, PinPolicy,
        StorageClass, StreamTokenBodyV1, StreamTokenV1,
    };
    use sorafs_orchestrator::{
        AnonymityPolicy, GatewayCarVerification, OrchestratorConfig, PolicyOverride, PolicyReport,
        PolicyStatus, RolloutPhase, TransportPolicy, prelude::BrowserExtensionManifest,
        proxy::ProxyMode, taikai_cache::PromotionStats,
    };
    use tempfile::tempdir;

    use super::*;

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
        let encoded = to_bytes(result).expect("encode privacy result");
        assert_subslice_absent(&encoded, witness, "Norito privacy result archive");
    }

    fn privacy_request(
        algorithm_id: &str,
        entrypoint: &str,
        proof: Vec<u8>,
    ) -> PrivacyProofRequestV1 {
        let vk_backend =
            privacy_algorithm_entry(algorithm_id).map_or("unknown", |entry| entry.backend_family);
        let vk_name = privacy_algorithm_entry(algorithm_id)
            .map_or_else(|| "vk_unknown".to_owned(), privacy_canonical_vk_ref_name);
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
        let mut archive = to_bytes(request).expect("encode privacy request");
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
                .all(|entry| entry.planned_entrypoints.is_empty()),
            "catalog invariant test must keep required privacy proof builders exported",
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
        assert_eq!(PRIVACY_RESEARCH_TARGET_ALGORITHM_IDS.len(), 0);
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
        const EMPTY: &[&str] = &[];
        const PLANNED_PROOF: &[&str] = &["buildUnmappedPrivacyProofV1"];

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
        const PROOF_HELPER_ONLY_PLANNED: &[&str] = &["buildOrchardActionBundleProofEnvelope"];

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
        {
            let entry = missing_planned
                .iter_mut()
                .find(|entry| entry.id == "anonymous-pgc-k-out-of-n-v1")
                .expect("required production plan row");
            entry.sdk_entrypoints = &[];
            entry.planned_entrypoints = &[];
        }
        assert!(
            !privacy_required_production_plan_rows_are_present(&missing_planned),
            "required production plan rows must keep production proof builders",
        );

        let mut helper_only_planned: Vec<PrivacyAlgorithmEntry> =
            PRIVACY_ALGORITHM_ENTRIES.to_vec();
        {
            let entry = helper_only_planned
                .iter_mut()
                .find(|entry| entry.id == "orchard-halo2-actions-v1")
                .expect("required production plan row");
            entry.sdk_entrypoints = &[];
            entry.planned_entrypoints = HELPER_ONLY_PLANNED;
        }
        assert!(
            !privacy_required_production_plan_rows_are_present(&helper_only_planned),
            "required production plan rows must keep a production proof builder",
        );

        let mut proof_helper_only_planned: Vec<PrivacyAlgorithmEntry> =
            PRIVACY_ALGORITHM_ENTRIES.to_vec();
        {
            let entry = proof_helper_only_planned
                .iter_mut()
                .find(|entry| entry.id == "orchard-halo2-actions-v1")
                .expect("required production plan row");
            entry.sdk_entrypoints = &[];
            entry.planned_entrypoints = PROOF_HELPER_ONLY_PLANNED;
        }
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

        for (case, entry) in [
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
            "buildRangeCommitment",
            "buildVeRangeDevProofFixture",
            "buildVeRangeProofEnvelope",
            "buildVeRangeProofV1",
            "verifyVeRangeProofLocally",
            "verifyVeRangeProofV1",
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
                !to_bytes(&result)
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
            let encoded = to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(&encoded, marker.as_bytes(), "non-ASCII text failure result");
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
            let encoded = to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(
                &encoded,
                marker.as_bytes(),
                "unportable text failure result",
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
        for field in ["algorithm_id", "entrypoint", "vk_ref"] {
            let mut request = privacy_request(
                "confidential-transfer-v2",
                "buildConfidentialTransferProofV2",
                Vec::new(),
            );
            request.public_inputs = b"public".to_vec();
            let witness = b"required-text-field-witness-never-echo";
            request.witness = witness.to_vec();
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

            assert_eq!(result.version, PRIVACY_FFI_VERSION_V1, "{field}");
            assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR, "{field}");
            assert_eq!(
                result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,
                "{field}"
            );
            assert!(result.message.contains(message_fragment), "{field}");
            assert_eq!(result.public_inputs, b"public", "{field}");
            assert!(result.proof.is_empty(), "{field}");
            assert!(!result.verified, "{field}");
            if field == "algorithm_id" {
                assert!(result.algorithm_id.is_empty(), "{field}");
            } else {
                assert_eq!(result.algorithm_id, "confidential-transfer-v2", "{field}");
            }
            if field == "entrypoint" {
                assert!(result.entrypoint.is_empty(), "{field}");
            } else {
                assert_eq!(
                    result.entrypoint, "buildConfidentialTransferProofV2",
                    "{field}"
                );
            }
            if field == "vk_ref" {
                assert!(result.vk_ref.is_empty(), "{field}");
            }
            let encoded = norito::to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(&encoded, witness, "empty required field failure result");
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
            let encoded = to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(
                &encoded,
                value.as_bytes(),
                "production-claim request failure result",
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
        let encoded = to_bytes(&result).expect("encode privacy result");
        assert_subslice_absent(&encoded, marker, "oversized witness failure result");
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
        let encoded = to_bytes(&result).expect("encode privacy result");
        assert_subslice_absent(&encoded, marker, "oversized proof failure result");
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

    const PRIVACY_TEST_PRODUCTION_CHAIN_ID: &str = "boi-privacy-4peer-chain";
    const PRIVACY_TEST_PRODUCTION_LOCALNET_RUN_ID: &str = "boi-privacy-4peer-localnet-2026-01-02";
    const PRIVACY_TEST_PRODUCTION_LOCALNET_PEER_IDS: [&str; 4] = [
        "boi-privacy-peer-1@localnet",
        "boi-privacy-peer-2@localnet",
        "boi-privacy-peer-3@localnet",
        "boi-privacy-peer-4@localnet",
    ];
    const PRIVACY_TEST_PRODUCTION_HASH: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PRIVACY_TEST_UPPERCASE_PRODUCTION_HASH: &str =
        "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const PRIVACY_TEST_PRODUCTION_SMOKE_HASH: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const PRIVACY_TEST_PRODUCTION_REPLAY_HASH: &str =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const PRIVACY_TEST_PRODUCTION_RESTART_REPLAY_HASH: &str =
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const PRIVACY_TEST_PRODUCTION_STATE_RECOVERY_HASH: &str =
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const PRIVACY_TEST_PRODUCTION_SIGNATURE: &str = "ed25519:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const PRIVACY_TEST_UPPERCASE_PRODUCTION_SIGNATURE: &str = "ed25519:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

    fn privacy_test_production_entrypoints(entry: &PrivacyAlgorithmEntry) -> Vec<&'static str> {
        entry
            .sdk_entrypoints
            .iter()
            .chain(entry.planned_entrypoints.iter())
            .copied()
            .filter(|entrypoint| {
                !privacy_entrypoint_is_dev_fixture(entrypoint)
                    && !privacy_entrypoint_is_local_verifier(entrypoint)
            })
            .fold(Vec::new(), |mut acc, entrypoint| {
                if !acc.iter().any(|existing| existing == &entrypoint) {
                    acc.push(entrypoint);
                }
                acc
            })
    }

    fn privacy_test_gate_evidence(
        entry: &PrivacyAlgorithmEntry,
    ) -> Vec<PrivacyProductionGateEvidenceV1> {
        PRIVACY_PRODUCTION_GATE_REQUIREMENTS
            .iter()
            .filter(|(key, _)| !privacy_production_gate_requirement_is_waived(entry, key))
            .map(|(key, _)| PrivacyProductionGateEvidenceV1 {
                key,
                artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
            })
            .collect()
    }

    fn privacy_test_sdk_exports(entrypoints: &[&'static str]) -> Vec<PrivacyProductionSdkExportV1> {
        PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES
            .iter()
            .map(|surface| PrivacyProductionSdkExportV1 {
                surface: *surface,
                entrypoints: entrypoints.to_vec(),
            })
            .collect()
    }

    fn privacy_test_sdk_parity_artifacts() -> Vec<PrivacyProductionSdkParityArtifactV1> {
        PRIVACY_PRODUCTION_SDK_PARITY_ARTIFACT_KINDS
            .iter()
            .flat_map(|kind| {
                PRIVACY_PRODUCTION_SDK_EXPORT_SURFACES
                    .iter()
                    .map(move |surface| PrivacyProductionSdkParityArtifactV1 {
                        kind: *kind,
                        surface: *surface,
                        artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
                    })
            })
            .collect()
    }

    fn privacy_test_review_scope(
        entry: &PrivacyAlgorithmEntry,
        sdk_entrypoints: &[&'static str],
    ) -> PrivacyProductionReviewScopeV1 {
        PrivacyProductionReviewScopeV1 {
            version: PRIVACY_PRODUCTION_REVIEW_SCOPE_VERSION,
            algorithm_id: entry.id,
            chain_id: PRIVACY_TEST_PRODUCTION_CHAIN_ID,
            verifier_key_id: privacy_expected_verifier_key_id(entry),
            proof_family: entry.proof_family,
            public_inputs_schema: privacy_expected_public_inputs_schema(entry),
            sdk_entrypoints: sdk_entrypoints.to_vec(),
            required_state: privacy_expected_required_state(entry).to_vec(),
            fuzz_artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
            performance_artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
            localnet_run_id: PRIVACY_TEST_PRODUCTION_LOCALNET_RUN_ID,
        }
    }

    fn privacy_test_evidence_row(entry: &PrivacyAlgorithmEntry) -> PrivacyProductionEvidenceRowV1 {
        let sdk_entrypoints = privacy_test_production_entrypoints(entry);
        PrivacyProductionEvidenceRowV1 {
            algorithm_id: entry.id,
            chain_id: PRIVACY_TEST_PRODUCTION_CHAIN_ID,
            reviewer_identity: "boi-crypto-reviewer-1",
            review_artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
            review_artifact_signature: PRIVACY_TEST_PRODUCTION_SIGNATURE,
            review_scope: privacy_test_review_scope(entry, &sdk_entrypoints),
            verifier_key_id: privacy_expected_verifier_key_id(entry),
            proof_family: entry.proof_family,
            public_inputs_schema: privacy_expected_public_inputs_schema(entry),
            sdk_entrypoints: sdk_entrypoints.clone(),
            sdk_exports: privacy_test_sdk_exports(&sdk_entrypoints),
            sdk_parity_artifacts: privacy_test_sdk_parity_artifacts(),
            required_state: privacy_expected_required_state(entry).to_vec(),
            fuzz_artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
            performance_artifact_hash: PRIVACY_TEST_PRODUCTION_HASH,
            localnet_acceptance: PrivacyProductionLocalnetEvidenceV1 {
                run_id: PRIVACY_TEST_PRODUCTION_LOCALNET_RUN_ID,
                target: PRIVACY_PRODUCTION_LOCALNET_TARGET,
                peer_count: PRIVACY_PRODUCTION_LOCALNET_PEER_COUNT,
                peer_ids: PRIVACY_TEST_PRODUCTION_LOCALNET_PEER_IDS,
                chain_id: PRIVACY_TEST_PRODUCTION_CHAIN_ID,
                smoke_passed: true,
                smoke_tx_hash: PRIVACY_TEST_PRODUCTION_SMOKE_HASH,
                replay_rejected: true,
                replay_rejection_hash: PRIVACY_TEST_PRODUCTION_REPLAY_HASH,
                restart_persistence_checked: true,
                restart_replay_rejected: true,
                restart_replay_rejection_hash: PRIVACY_TEST_PRODUCTION_RESTART_REPLAY_HASH,
                state_recovery_passed: true,
                state_recovery_hash: PRIVACY_TEST_PRODUCTION_STATE_RECOVERY_HASH,
            },
            gate_evidence: privacy_test_gate_evidence(entry),
        }
    }

    fn assert_zk_ace_evidence_rejected<F>(case: &str, mutate: F)
    where
        F: FnOnce(&mut PrivacyProductionEvidenceRowV1),
    {
        let entry =
            privacy_algorithm_entry("zk-ace-pq-authorization-v0").expect("ZK-ACE catalog row");
        let mut row = privacy_test_evidence_row(entry);
        mutate(&mut row);

        assert!(
            !privacy_production_evidence_row_is_valid(
                &row,
                entry,
                Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID)
            ),
            "{case} evidence row must be rejected before capability admission",
        );
        let capabilities = privacy_capabilities_with_production_evidence(
            &[row],
            Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID),
        );
        let zk_ace = capabilities
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == entry.id)
            .expect("ZK-ACE capability row");
        assert!(
            !zk_ace.production_ready,
            "{case} must keep ZK-ACE fail-closed",
        );
        assert!(
            !zk_ace.production_gate.ready,
            "{case} gate must fail closed"
        );
    }

    fn assert_privacy_evidence_rejected_for_all_rows<F>(case: &str, mutate: F)
    where
        F: Fn(&mut PrivacyProductionEvidenceRowV1),
    {
        for entry in PRIVACY_ALGORITHM_ENTRIES {
            let mut row = privacy_test_evidence_row(entry);
            mutate(&mut row);

            assert!(
                !privacy_production_evidence_row_is_valid(
                    &row,
                    entry,
                    Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID)
                ),
                "{case} evidence row must be rejected before capability admission for {}",
                entry.id,
            );
            let capabilities = privacy_capabilities_with_production_evidence(
                &[row],
                Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID),
            );
            let algorithm = capabilities
                .algorithms
                .iter()
                .find(|algorithm| algorithm.algorithm_id == entry.id)
                .expect("privacy capability row");
            assert!(
                !algorithm.production_ready,
                "{case} must keep {} fail-closed",
                entry.id,
            );
            assert!(
                !algorithm.production_gate.ready,
                "{case} gate must fail closed for {}",
                entry.id,
            );
        }
    }

    #[test]
    fn privacy_capabilities_are_norito_v1_and_fail_closed() {
        let mut encoded = privacy_capabilities_v1()
            .expect("encode privacy capabilities")
            .to_vec();
        normalize_privacy_public_archive_for_decode::<PrivacyCapabilitiesV1>(&mut encoded);
        let decoded: PrivacyCapabilitiesV1 =
            norito::decode_from_bytes(&encoded).expect("decode capabilities");

        assert!(privacy_capabilities_invariants_hold(&decoded));
        assert_eq!(decoded.version, PRIVACY_FFI_VERSION_V1);
        assert_eq!(decoded.gate_version, PRIVACY_PRODUCTION_GATE_VERSION);
        assert_eq!(decoded.algorithms.len(), PRIVACY_ALGORITHM_ENTRIES.len());
        assert!(
            decoded
                .algorithms
                .iter()
                .any(|entry| entry.algorithm_id == "pq-masp-stark-v0"),
        );
        let transparent = decoded
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == "transparent-transfer")
            .expect("transparent transfer native capability must be advertised");
        let transparent_entry = privacy_algorithm_entry("transparent-transfer")
            .expect("transparent transfer catalog row");
        assert_eq!(
            transparent.production_gate.required_gates,
            privacy_required_production_gate_keys(transparent_entry),
            "transparent transfer must advertise only baseline transfer production gates",
        );
        assert!(
            !transparent
                .production_gate
                .required_gates
                .iter()
                .any(|key| PRIVACY_TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS
                    .contains(&key.as_str())),
            "transparent transfer must not require proof-only gates",
        );
        assert!(
            !transparent.production_gate.missing.iter().any(|missing| {
                missing == "real proving engine is not registered"
                    || missing == "real verifier is not registered"
                    || missing == "witness privacy checks are incomplete"
                    || missing == "verifier fuzzing gate is incomplete"
            }),
            "transparent transfer must not report waived proof-only gates as missing",
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
        let zk_ace_entry =
            privacy_algorithm_entry("zk-ace-pq-authorization-v0").expect("ZK-ACE catalog row");
        assert_eq!(
            zk_ace.production_gate.required_gates,
            privacy_required_production_gate_keys(zk_ace_entry),
            "ZK-ACE must require the full production proof gate set",
        );
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
                    .gates
                    .iter()
                    .all(|gate| !gate.passed),
            );
            assert!(
                algorithm
                    .production_gate
                    .missing
                    .iter()
                    .any(|missing| missing.contains("internal cryptographic review")),
            );
            let entry = privacy_algorithm_entry(&algorithm.algorithm_id)
                .expect("privacy capability row must be cataloged");
            assert_eq!(
                algorithm.production_gate.required_gates,
                privacy_required_production_gate_keys(entry),
                "native privacy required production gates drifted for {}",
                algorithm.algorithm_id,
            );
        }
    }

    #[test]
    fn privacy_capabilities_accept_exact_internal_evidence_for_all_rows() {
        let evidence = PRIVACY_ALGORITHM_ENTRIES
            .iter()
            .map(privacy_test_evidence_row)
            .collect::<Vec<_>>();

        let capabilities = privacy_capabilities_with_production_evidence(
            &evidence,
            Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID),
        );

        assert!(privacy_capabilities_invariants_hold(&capabilities));
        assert_eq!(
            capabilities.algorithms.len(),
            PRIVACY_ALGORITHM_ENTRIES.len()
        );
        assert!(
            capabilities
                .algorithms
                .iter()
                .all(|algorithm| algorithm.production_ready && algorithm.production_gate.ready),
            "all rows with exact internal evidence must be admitted",
        );

        for algorithm in &capabilities.algorithms {
            let entry = privacy_algorithm_entry(&algorithm.algorithm_id)
                .expect("production-ready capability row must be cataloged");
            assert!(algorithm.planned_entrypoints.is_empty());
            assert_eq!(
                algorithm.sdk_entrypoints,
                privacy_expected_production_sdk_entrypoints(entry),
                "production capabilities must expose the complete filtered SDK surface for {}",
                algorithm.algorithm_id,
            );
            assert!(algorithm.production_gate.missing.is_empty());
            assert_eq!(
                algorithm.production_gate.required_gates,
                privacy_required_production_gate_keys(entry),
            );
            assert_eq!(algorithm.production_gate.audit_references.len(), 7);
            for status in &algorithm.production_gate.gates {
                assert_eq!(
                    status.passed,
                    !privacy_production_gate_requirement_is_waived(entry, &status.key),
                    "ready gate status drifted for {} / {}",
                    algorithm.algorithm_id,
                    status.key,
                );
            }
        }

        let transparent = capabilities
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == "transparent-transfer")
            .expect("transparent transfer capability");
        for waived_key in PRIVACY_TRANSPARENT_TRANSFER_BASELINE_WAIVED_GATE_KEYS {
            assert!(
                !transparent
                    .production_gate
                    .required_gates
                    .iter()
                    .any(|required| required == waived_key),
                "transparent transfer must keep proof-only gate {waived_key} waived",
            );
            assert!(
                transparent
                    .production_gate
                    .gates
                    .iter()
                    .any(|status| status.key == *waived_key && !status.passed),
                "transparent transfer must not mark proof-only gate {waived_key} passed",
            );
        }

        let zk_ace = capabilities
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == "zk-ace-pq-authorization-v0")
            .expect("ZK-ACE capability");
        assert!(
            zk_ace
                .sdk_entrypoints
                .contains(&"buildZkAceAuthorizationProofV1".to_owned())
        );
        assert!(
            zk_ace
                .sdk_entrypoints
                .contains(&"buildZkAceAuthorizedTransferInstruction".to_owned())
        );
    }

    #[test]
    fn privacy_production_evidence_rejects_adversarial_zk_ace_bindings() {
        assert_zk_ace_evidence_rejected("wrong chain", |row| {
            row.chain_id = "boi-privacy-other-4peer-chain";
        });
        assert_zk_ace_evidence_rejected("mock chain marker", |row| {
            row.chain_id = "mock-privacy-4peer-chain";
        });
        assert_zk_ace_evidence_rejected("wrong verifier key", |row| {
            row.verifier_key_id = "shadow_zk_ace_verifier";
        });
        assert_zk_ace_evidence_rejected("mutated public input schema", |row| {
            row.public_inputs_schema = Some("identity_commitment,tx_digest,chain_id");
        });
        assert_zk_ace_evidence_rejected("dev fixture entrypoint", |row| {
            row.sdk_entrypoints.push("buildZkAceDevProofFixture");
        });
        assert_zk_ace_evidence_rejected("local verifier entrypoint", |row| {
            row.sdk_entrypoints.push("verifyZkAceProofLocally");
        });
        assert_zk_ace_evidence_rejected("missing SDK export surface", |row| {
            row.sdk_exports.pop();
        });
        assert_zk_ace_evidence_rejected("mismatched SDK export entrypoint", |row| {
            row.sdk_exports[3]
                .entrypoints
                .push("buildShadowZkAceProductionProof");
        });
        assert_zk_ace_evidence_rejected("dev fixture SDK export", |row| {
            row.sdk_exports[2]
                .entrypoints
                .push("buildZkAceDevProofFixture");
        });
        assert_zk_ace_evidence_rejected("missing SDK parity artifact", |row| {
            row.sdk_parity_artifacts.pop();
        });
        assert_zk_ace_evidence_rejected("wrong SDK parity artifact kind", |row| {
            row.sdk_parity_artifacts[0].kind = "fixture_vectors";
        });
        assert_zk_ace_evidence_rejected("bad SDK parity artifact hash", |row| {
            row.sdk_parity_artifacts[0].artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_zk_ace_evidence_rejected("three-peer localnet downgrade", |row| {
            row.localnet_acceptance.peer_count = 3;
        });
        assert_zk_ace_evidence_rejected("duplicate localnet peer id", |row| {
            row.localnet_acceptance.peer_ids[3] = row.localnet_acceptance.peer_ids[0];
        });
        assert_zk_ace_evidence_rejected("wrong localnet chain", |row| {
            row.localnet_acceptance.chain_id = "boi-privacy-other-4peer-chain";
        });
        assert_zk_ace_evidence_rejected("bad localnet smoke hash", |row| {
            row.localnet_acceptance.smoke_tx_hash = "sha256:not-a-hex-digest";
        });
        assert_zk_ace_evidence_rejected("reused localnet replay hash", |row| {
            row.localnet_acceptance.replay_rejection_hash = row.localnet_acceptance.smoke_tx_hash;
        });
        assert_zk_ace_evidence_rejected("replay acceptance", |row| {
            row.localnet_acceptance.replay_rejected = false;
        });
        assert_zk_ace_evidence_rejected("restart replay acceptance", |row| {
            row.localnet_acceptance.restart_replay_rejected = false;
        });
        assert_zk_ace_evidence_rejected("mock localnet run", |row| {
            row.localnet_acceptance.run_id = "mock-privacy-4peer-localnet-2026-01-02";
        });
        assert_zk_ace_evidence_rejected("missing production gate evidence", |row| {
            row.gate_evidence.pop();
        });
        assert_zk_ace_evidence_rejected("duplicated production gate evidence", |row| {
            row.gate_evidence.push(row.gate_evidence[0]);
        });
        assert_zk_ace_evidence_rejected("bad review artifact hash", |row| {
            row.review_artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_zk_ace_evidence_rejected("uppercase review artifact hash", |row| {
            row.review_artifact_hash = PRIVACY_TEST_UPPERCASE_PRODUCTION_HASH;
        });
        assert_zk_ace_evidence_rejected("unsigned review artifact", |row| {
            row.review_artifact_signature = "ed25519:bbbb";
        });
        assert_zk_ace_evidence_rejected("uppercase review artifact signature", |row| {
            row.review_artifact_signature = PRIVACY_TEST_UPPERCASE_PRODUCTION_SIGNATURE;
        });
        assert_zk_ace_evidence_rejected("mock reviewer identity", |row| {
            row.reviewer_identity = "mock-crypto-reviewer";
        });
        assert_zk_ace_evidence_rejected("missing required state", |row| {
            row.required_state.pop();
        });
        assert_zk_ace_evidence_rejected("wrong review scope algorithm", |row| {
            row.review_scope.algorithm_id = "transparent-transfer";
        });
        assert_zk_ace_evidence_rejected("wrong review scope chain", |row| {
            row.review_scope.chain_id = "boi-privacy-other-4peer-chain";
        });
        assert_zk_ace_evidence_rejected("wrong review scope verifier key", |row| {
            row.review_scope.verifier_key_id = "shadow_zk_ace_verifier";
        });
        assert_zk_ace_evidence_rejected("mutated review scope public input schema", |row| {
            row.review_scope.public_inputs_schema = Some("identity_commitment,tx_digest,chain_id");
        });
        assert_zk_ace_evidence_rejected("missing review scope SDK entrypoint", |row| {
            row.review_scope.sdk_entrypoints.pop();
        });
        assert_zk_ace_evidence_rejected("dev fixture review scope SDK entrypoint", |row| {
            row.review_scope
                .sdk_entrypoints
                .push("buildZkAceDevProofFixture");
        });
        assert_zk_ace_evidence_rejected("missing review scope required state", |row| {
            row.review_scope.required_state.pop();
        });
        assert_zk_ace_evidence_rejected("bad review scope fuzz hash", |row| {
            row.review_scope.fuzz_artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_zk_ace_evidence_rejected("bad review scope performance hash", |row| {
            row.review_scope.performance_artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_zk_ace_evidence_rejected("mock review scope localnet run", |row| {
            row.review_scope.localnet_run_id = "mock-privacy-4peer-localnet-2026-01-02";
        });
    }

    #[test]
    fn privacy_production_evidence_rejects_adversarial_bindings_for_all_rows() {
        assert_privacy_evidence_rejected_for_all_rows("wrong algorithm", |row| {
            row.algorithm_id = "shadow-privacy-row-v1";
            row.review_scope.algorithm_id = "shadow-privacy-row-v1";
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong chain", |row| {
            row.chain_id = "boi-privacy-other-4peer-chain";
        });
        assert_privacy_evidence_rejected_for_all_rows("mock chain marker", |row| {
            row.chain_id = "mock-privacy-4peer-chain";
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong verifier key", |row| {
            row.verifier_key_id = "shadow_verifier_key";
        });
        assert_privacy_evidence_rejected_for_all_rows("mutated public input schema", |row| {
            row.public_inputs_schema = Some("mutated_public_inputs");
        });
        assert_privacy_evidence_rejected_for_all_rows("extra SDK entrypoint", |row| {
            row.sdk_entrypoints.push("buildShadowProof");
        });
        assert_privacy_evidence_rejected_for_all_rows("dev fixture entrypoint", |row| {
            row.sdk_entrypoints.push("buildShadowDevFixture");
        });
        assert_privacy_evidence_rejected_for_all_rows("local verifier entrypoint", |row| {
            row.sdk_entrypoints.push("verifyShadowProofLocally");
        });
        assert_privacy_evidence_rejected_for_all_rows("missing SDK export surface", |row| {
            row.sdk_exports.pop();
        });
        assert_privacy_evidence_rejected_for_all_rows("mismatched SDK export entrypoint", |row| {
            row.sdk_exports[0].entrypoints.push("buildShadowProof");
        });
        assert_privacy_evidence_rejected_for_all_rows("dev fixture SDK export", |row| {
            row.sdk_exports[0].entrypoints.push("buildShadowDevFixture");
        });
        assert_privacy_evidence_rejected_for_all_rows("missing SDK parity artifact", |row| {
            row.sdk_parity_artifacts.pop();
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong SDK parity artifact kind", |row| {
            row.sdk_parity_artifacts[0].kind = "fixture_vectors";
        });
        assert_privacy_evidence_rejected_for_all_rows("bad SDK parity artifact hash", |row| {
            row.sdk_parity_artifacts[0].artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_privacy_evidence_rejected_for_all_rows("three-peer localnet downgrade", |row| {
            row.localnet_acceptance.peer_count = 3;
        });
        assert_privacy_evidence_rejected_for_all_rows("duplicate localnet peer id", |row| {
            row.localnet_acceptance.peer_ids[3] = row.localnet_acceptance.peer_ids[0];
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong localnet chain", |row| {
            row.localnet_acceptance.chain_id = "boi-privacy-other-4peer-chain";
        });
        assert_privacy_evidence_rejected_for_all_rows("localnet smoke failure", |row| {
            row.localnet_acceptance.smoke_passed = false;
        });
        assert_privacy_evidence_rejected_for_all_rows("bad localnet smoke hash", |row| {
            row.localnet_acceptance.smoke_tx_hash = "sha256:not-a-hex-digest";
        });
        assert_privacy_evidence_rejected_for_all_rows("replay acceptance", |row| {
            row.localnet_acceptance.replay_rejected = false;
        });
        assert_privacy_evidence_rejected_for_all_rows("reused localnet replay hash", |row| {
            row.localnet_acceptance.replay_rejection_hash = row.localnet_acceptance.smoke_tx_hash;
        });
        assert_privacy_evidence_rejected_for_all_rows("restart persistence omitted", |row| {
            row.localnet_acceptance.restart_persistence_checked = false;
        });
        assert_privacy_evidence_rejected_for_all_rows("restart replay acceptance", |row| {
            row.localnet_acceptance.restart_replay_rejected = false;
        });
        assert_privacy_evidence_rejected_for_all_rows("state recovery omitted", |row| {
            row.localnet_acceptance.state_recovery_passed = false;
        });
        assert_privacy_evidence_rejected_for_all_rows("mock localnet run", |row| {
            row.localnet_acceptance.run_id = "mock-privacy-4peer-localnet-2026-01-02";
        });
        assert_privacy_evidence_rejected_for_all_rows("missing production gate evidence", |row| {
            row.gate_evidence.pop();
        });
        assert_privacy_evidence_rejected_for_all_rows(
            "duplicated production gate evidence",
            |row| {
                row.gate_evidence.push(row.gate_evidence[0]);
            },
        );
        assert_privacy_evidence_rejected_for_all_rows("bad review artifact hash", |row| {
            row.review_artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_privacy_evidence_rejected_for_all_rows("uppercase review artifact hash", |row| {
            row.review_artifact_hash = PRIVACY_TEST_UPPERCASE_PRODUCTION_HASH;
        });
        assert_privacy_evidence_rejected_for_all_rows("unsigned review artifact", |row| {
            row.review_artifact_signature = "ed25519:bbbb";
        });
        assert_privacy_evidence_rejected_for_all_rows(
            "uppercase review artifact signature",
            |row| {
                row.review_artifact_signature = PRIVACY_TEST_UPPERCASE_PRODUCTION_SIGNATURE;
            },
        );
        assert_privacy_evidence_rejected_for_all_rows("mock reviewer identity", |row| {
            row.reviewer_identity = "mock-crypto-reviewer";
        });
        assert_privacy_evidence_rejected_for_all_rows("required state mismatch", |row| {
            row.required_state.push("shadow unchecked state");
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong review scope algorithm", |row| {
            row.review_scope.algorithm_id = "shadow-privacy-row-v1";
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong review scope chain", |row| {
            row.review_scope.chain_id = "boi-privacy-other-4peer-chain";
        });
        assert_privacy_evidence_rejected_for_all_rows("wrong review scope verifier key", |row| {
            row.review_scope.verifier_key_id = "shadow_verifier_key";
        });
        assert_privacy_evidence_rejected_for_all_rows(
            "mutated review scope public input schema",
            |row| {
                row.review_scope.public_inputs_schema = Some("mutated_public_inputs");
            },
        );
        assert_privacy_evidence_rejected_for_all_rows(
            "missing review scope SDK entrypoint",
            |row| {
                row.review_scope.sdk_entrypoints.pop();
            },
        );
        assert_privacy_evidence_rejected_for_all_rows(
            "dev fixture review scope SDK entrypoint",
            |row| {
                row.review_scope
                    .sdk_entrypoints
                    .push("buildShadowDevFixture");
            },
        );
        assert_privacy_evidence_rejected_for_all_rows(
            "review scope required state mismatch",
            |row| {
                row.review_scope
                    .required_state
                    .push("shadow unchecked state");
            },
        );
        assert_privacy_evidence_rejected_for_all_rows("bad review scope fuzz hash", |row| {
            row.review_scope.fuzz_artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_privacy_evidence_rejected_for_all_rows("bad review scope performance hash", |row| {
            row.review_scope.performance_artifact_hash = "sha256:not-a-hex-digest";
        });
        assert_privacy_evidence_rejected_for_all_rows("mock review scope localnet run", |row| {
            row.review_scope.localnet_run_id = "mock-privacy-4peer-localnet-2026-01-02";
        });
    }

    #[test]
    fn privacy_production_evidence_rejects_missing_and_duplicate_rows() {
        let entry =
            privacy_algorithm_entry("zk-ace-pq-authorization-v0").expect("ZK-ACE catalog row");
        let row = privacy_test_evidence_row(entry);

        let no_chain_capabilities = privacy_capabilities_with_production_evidence(&[row], None);
        let zk_ace = no_chain_capabilities
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == entry.id)
            .expect("ZK-ACE capability row");
        assert!(
            !zk_ace.production_ready,
            "evidence cannot admit production readiness without expected chain binding",
        );

        let duplicate_capabilities = privacy_capabilities_with_production_evidence(
            &[
                privacy_test_evidence_row(entry),
                privacy_test_evidence_row(entry),
            ],
            Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID),
        );
        let duplicate_zk_ace = duplicate_capabilities
            .algorithms
            .iter()
            .find(|algorithm| algorithm.algorithm_id == entry.id)
            .expect("ZK-ACE capability row");
        assert!(
            !duplicate_zk_ace.production_ready,
            "duplicate valid evidence rows must not admit readiness",
        );
    }

    #[test]
    fn privacy_production_evidence_rejects_missing_chain_and_duplicates_for_all_rows() {
        for entry in PRIVACY_ALGORITHM_ENTRIES {
            let row = privacy_test_evidence_row(entry);

            let no_chain_capabilities =
                privacy_capabilities_with_production_evidence(&[row.clone()], None);
            let no_chain_algorithm = no_chain_capabilities
                .algorithms
                .iter()
                .find(|algorithm| algorithm.algorithm_id == entry.id)
                .expect("privacy capability row");
            assert!(
                !no_chain_algorithm.production_ready,
                "evidence cannot admit {} without expected chain binding",
                entry.id,
            );

            let duplicate_capabilities = privacy_capabilities_with_production_evidence(
                &[row.clone(), row],
                Some(PRIVACY_TEST_PRODUCTION_CHAIN_ID),
            );
            let duplicate_algorithm = duplicate_capabilities
                .algorithms
                .iter()
                .find(|algorithm| algorithm.algorithm_id == entry.id)
                .expect("privacy capability row");
            assert!(
                !duplicate_algorithm.production_ready,
                "duplicate valid evidence rows must not admit {}",
                entry.id,
            );
        }
    }

    #[test]
    fn privacy_native_archives_use_public_schema_hashes() {
        let mut capabilities_archive = privacy_capabilities_v1()
            .expect("encode privacy capabilities")
            .to_vec();
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
        let build_result = privacy_result_for_request_archive(
            &build_request_archive,
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            build_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED
        );
        let mut build_archive = encode_privacy_archive(
            &build_result,
            "encode privacy proof build result",
            privacy_result_schema_byte(PrivacyProofOperationV1::Build),
        )
        .expect("encode build result")
        .to_vec();
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &build_archive,
                PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE,
            ),
            "build output must use the public privacy build-result schema"
        );
        normalize_privacy_public_archive_for_decode::<PrivacyProofResultV1>(&mut build_archive);
        let decoded_build_result: PrivacyProofResultV1 =
            norito::decode_from_bytes(&build_archive).expect("decode build result");
        assert_eq!(
            decoded_build_result.error_code,
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
        let verify_result = privacy_result_for_request_archive(
            &verify_request_archive,
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(
            verify_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED
        );
        let mut verify_archive = encode_privacy_archive(
            &verify_result,
            "encode privacy proof verify result",
            privacy_result_schema_byte(PrivacyProofOperationV1::Verify),
        )
        .expect("encode verify result")
        .to_vec();
        assert!(
            privacy_archive_has_repeated_schema_byte(
                &verify_archive,
                PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE,
            ),
            "verify output must use the public privacy verify-result schema"
        );
        normalize_privacy_public_archive_for_decode::<PrivacyProofResultV1>(&mut verify_archive);
        let decoded_verify_result: PrivacyProofResultV1 =
            norito::decode_from_bytes(&verify_archive).expect("decode verify result");
        assert_eq!(
            decoded_verify_result.error_code,
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
        let private_archive = to_bytes(&request).expect("encode private privacy request");
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
            let entry = privacy_algorithm_entry(&algorithm.algorithm_id)
                .expect("privacy capability row must be cataloged");
            !algorithm.production_ready
                && privacy_production_gate_invariants_hold(&algorithm.production_gate, entry)
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
            .retain(|missing| missing != "internal cryptographic review signoff is missing");
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
            .push("internal cryptographic review signoff passed without evidence".to_owned());
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

        let mut missing_required_gate = base.clone();
        missing_required_gate.production_gate.required_gates.clear();
        assert!(
            !privacy_capability_invariants_hold(&missing_required_gate),
            "missing required production gate keys must be rejected",
        );

        let mut duplicate_required_gate = base.clone();
        duplicate_required_gate
            .production_gate
            .required_gates
            .push(duplicate_required_gate.production_gate.required_gates[0].clone());
        assert!(
            !privacy_capability_invariants_hold(&duplicate_required_gate),
            "duplicate required production gate keys must be rejected",
        );

        let mut unknown_required_gate = base.clone();
        unknown_required_gate.production_gate.required_gates[0] = "shadow_gate".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&unknown_required_gate),
            "unknown required production gate keys must be rejected",
        );

        let mut unportable_required_gate = base.clone();
        unportable_required_gate.production_gate.required_gates[0] = "shadow gate".to_owned();
        assert!(
            !privacy_capability_invariants_hold(&unportable_required_gate),
            "unportable required production gate keys must be rejected",
        );

        let mut forged_waived_required_gate = base.clone();
        forged_waived_required_gate
            .production_gate
            .required_gates
            .insert(0, "real_proving".to_owned());
        assert!(
            !privacy_capability_invariants_hold(&forged_waived_required_gate),
            "transparent-transfer waived proof gates must not be reintroduced as required",
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
    fn privacy_build_proof_rejects_malformed_norito() {
        let result =
            privacy_result_for_request_archive(b"not norito", PrivacyProofOperationV1::Build);

        assert_eq!(result.version, PRIVACY_FFI_VERSION_V1);
        assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_MALFORMED_NORITO);
        assert!(result.proof.is_empty());
        assert!(result.public_inputs.is_empty());
        assert!(!result.verified);
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
    fn privacy_build_proof_rejects_unknown_algorithm() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "fake-shadow-row".to_owned(),
                entrypoint: "buildFakeShadowProof".to_owned(),
                vk_ref: "vk:fake".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"secret".to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM);
        assert_eq!(result.algorithm_id, "fake-shadow-row");
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_failure_results_never_serialize_witness_material() {
        let witness = b"js-host-witness-never-echo-2cf14a6e";

        let unsupported_result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "fake-shadow-row".to_owned(),
                entrypoint: "buildFakeShadowProof".to_owned(),
                vk_ref: "vk:fake".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            unsupported_result.error_code,
            PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
        );
        assert_privacy_result_does_not_serialize_witness(&unsupported_result, witness);

        let bad_entrypoint_result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "aztec-private-rollup-v1".to_owned(),
                entrypoint: "buildForgedPrivateKernelProof".to_owned(),
                vk_ref: "aztec-plonkish-private-kernel:vk_private_kernel_v1".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: witness.to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(
            bad_entrypoint_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&bad_entrypoint_result, witness);

        let missing_vk_result = privacy_result_for_request(
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
        assert_eq!(
            missing_vk_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&missing_vk_result, witness);

        let wrong_vk_backend_result = privacy_result_for_request(
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
            wrong_vk_backend_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&wrong_vk_backend_result, witness);

        let wrong_vk_name_result = privacy_result_for_request(
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
        assert_eq!(
            wrong_vk_name_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&wrong_vk_name_result, witness);

        let empty_public_inputs_result = privacy_result_for_request(
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
            empty_public_inputs_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&empty_public_inputs_result, witness);

        let disabled_build_result = privacy_result_for_request(
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
            disabled_build_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
        );
        assert_privacy_result_does_not_serialize_witness(&disabled_build_result, witness);

        let disabled_verify_result = privacy_result_for_request(
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
            disabled_verify_result.error_code,
            PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
        );
        assert_privacy_result_does_not_serialize_witness(&disabled_verify_result, witness);

        let witness_shadow_verify_result = privacy_result_for_request(
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
            witness_shadow_verify_result.error_code,
            PRIVACY_FFI_ERROR_INVALID_REQUEST,
        );
        assert_privacy_result_does_not_serialize_witness(&witness_shadow_verify_result, witness);
    }

    #[test]
    fn privacy_failure_results_preserve_error_invariants_without_proof_reflection() {
        let proof_marker = b"js-host-proof-never-echo-33f1";

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
            assert_subslice_absent(
                result.message.as_bytes(),
                proof_marker,
                "privacy failure result message",
            );
            let encoded = to_bytes(&result).expect("encode privacy result");
            assert_subslice_absent(&encoded, proof_marker, "Norito privacy result archive");
        }
    }

    #[test]
    fn privacy_build_proof_rejects_empty_algorithm_and_entrypoint() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: String::new(),
                entrypoint: String::new(),
                vk_ref: String::new(),
                public_inputs: b"public".to_vec(),
                witness: b"secret".to_vec(),
                proof: b"proof".to_vec(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.public_inputs, b"public");
        assert!(result.proof.is_empty());
        assert!(!result.message.contains("secret"));
    }

    #[test]
    fn privacy_build_proof_rejects_unknown_entrypoint_for_known_algorithm() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "aztec-private-rollup-v1".to_owned(),
                entrypoint: "buildForgedPrivateKernelProof".to_owned(),
                vk_ref: "aztec-plonkish-private-kernel:vk_private_kernel_v1".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "aztec-private-rollup-v1");
        assert_eq!(result.entrypoint, "buildForgedPrivateKernelProof");
        assert!(result.message.contains("entrypoint"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_planned_entrypoint_before_request_validation() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "aztec-private-rollup-v1".to_owned(),
                entrypoint: "buildAztecPrivateKernelProofV1".to_owned(),
                vk_ref: String::new(),
                public_inputs: b"public".to_vec(),
                witness: b"planned-entrypoint-witness-must-not-echo".to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert_eq!(result.algorithm_id, "aztec-private-rollup-v1");
        assert_eq!(result.entrypoint, "buildAztecPrivateKernelProofV1");
        assert!(result.message.contains("planned"));
        assert!(result.message.contains("not executable"));
        assert!(!result.message.contains("vk_ref"));
        assert!(!result.message.contains("witness"));
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_empty_vk_ref() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: String::new(),
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("vk_ref"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
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
            let result = privacy_result_for_request(
                PrivacyProofRequestV1 {
                    algorithm_id: "confidential-transfer-v2".to_owned(),
                    entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                    vk_ref,
                    public_inputs: b"public".to_vec(),
                    witness: b"secret witness".to_vec(),
                    proof: Vec::new(),
                },
                PrivacyProofOperationV1::Build,
            );

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
                "pq-masp-stark-v0",
                "buildPqMaspStarkTransferProofV0",
                format!("stark-fri:bad.vk.name_{marker}"),
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

            if case == "planned-entrypoint" {
                assert_eq!(result.version, PRIVACY_FFI_VERSION_V1, "{case}");
                assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR, "{case}");
                assert_eq!(
                    result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST,
                    "{case}"
                );
                assert!(result.message.contains("planned"), "{case}");
                assert!(result.message.contains("not executable"), "{case}");
                assert_eq!(result.algorithm_id, algorithm_id, "{case}");
                assert_eq!(result.entrypoint, entrypoint, "{case}");
                assert!(result.vk_ref.is_empty(), "{case}");
                assert!(result.proof.is_empty(), "{case}");
                assert!(!result.verified, "{case}");
            } else {
                assert_unreflected_invalid_privacy_request_result(&result, "backend:name", case);
            }
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
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: vk_ref.to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

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
            let result = privacy_result_for_request(
                PrivacyProofRequestV1 {
                    algorithm_id: "confidential-transfer-v2".to_owned(),
                    entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                    vk_ref: vk_ref.to_owned(),
                    public_inputs: b"public".to_vec(),
                    witness: b"secret witness".to_vec(),
                    proof: Vec::new(),
                },
                PrivacyProofOperationV1::Build,
            );

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
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: Vec::new(),
                witness: b"secret witness".to_vec(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("public_inputs"));
        assert!(result.message.contains("non-empty"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert_eq!(result.entrypoint, "buildConfidentialTransferProofV2");
        assert!(result.public_inputs.is_empty());
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_verify_proof_rejects_empty_public_inputs_before_production_gate() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: Vec::new(),
                witness: Vec::new(),
                proof: b"proof bytes".to_vec(),
            },
            PrivacyProofOperationV1::Verify,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("public_inputs"));
        assert!(result.message.contains("non-empty"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert_eq!(result.entrypoint, "buildConfidentialTransferProofV2");
        assert!(result.public_inputs.is_empty());
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_missing_witness_before_production_gate() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: Vec::new(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("witness"));
        assert!(result.message.contains("build"));
        assert!(result.message.contains("must include"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_build_proof_rejects_proof_shadow_before_production_gate() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"secret witness".to_vec(),
                proof: b"forged-build-proof-shadow".to_vec(),
            },
            PrivacyProofOperationV1::Build,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("build"));
        assert!(result.message.contains("proof"));
        assert!(result.message.contains("must not include"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
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
                "build-envelope-helper",
                PrivacyProofOperationV1::Build,
                privacy_request(
                    "verange-transparent-range-v1",
                    "buildVeRangeProofEnvelope",
                    Vec::new(),
                ),
            ),
            (
                "verify-proof-envelope-helper",
                PrivacyProofOperationV1::Verify,
                privacy_request(
                    "verange-transparent-range-v1",
                    "buildVeRangeProofEnvelope",
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
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: Vec::new(),
                proof: Vec::new(),
            },
            PrivacyProofOperationV1::Verify,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("proof"));
        assert!(result.message.contains("verify"));
        assert!(result.message.contains("must include"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    #[test]
    fn privacy_verify_proof_rejects_witness_shadow_before_production_gate() {
        let result = privacy_result_for_request(
            PrivacyProofRequestV1 {
                algorithm_id: "confidential-transfer-v2".to_owned(),
                entrypoint: "buildConfidentialTransferProofV2".to_owned(),
                vk_ref: "halo2-ipa-pasta:confidential_transfer_v2".to_owned(),
                public_inputs: b"public".to_vec(),
                witness: b"forged-verify-witness-shadow".to_vec(),
                proof: b"candidate proof".to_vec(),
            },
            PrivacyProofOperationV1::Verify,
        );

        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(result.message.contains("verify"));
        assert!(result.message.contains("witness"));
        assert!(result.message.contains("must not include"));
        assert_eq!(result.algorithm_id, "confidential-transfer-v2");
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
            "witness privacy checks",
            "deterministic tests",
            "negative/adversarial tests",
            "replay/nullifier rejection tests",
            "fuzzing",
            "parser fuzzing",
            "verifier fuzzing",
            "performance gates",
            "internal cryptographic review",
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
            "witness privacy checks",
            "deterministic tests",
            "negative/adversarial tests",
            "replay/nullifier rejection tests",
            "fuzzing",
            "parser fuzzing",
            "verifier fuzzing",
            "performance gates",
            "internal cryptographic review",
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

    fn disable_packed_struct_once() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(ensure_packed_struct_disabled);
    }

    fn hash_literal(byte: u8) -> String {
        let mut buf = [byte; Hash::LENGTH];
        buf[buf.len() - 1] |= 1;
        let hash = Hash::prehashed(buf);
        match json::to_value(&hash).expect("hash json value") {
            Value::String(s) => s,
            other => panic!("expected hash literal string, got {other:?}"),
        }
    }

    #[test]
    fn build_kaigi_roster_join_proof_emits_envelope() {
        let proof = build_kaigi_roster_join_proof_bytes(&[0x42; 32], &empty_roster_root_hash())
            .expect("build proof");

        assert_eq!(proof.commitment.len(), Hash::LENGTH);
        assert_eq!(proof.nullifier.len(), Hash::LENGTH);
        assert_eq!(proof.roster_root.len(), Hash::LENGTH);
        assert!(!proof.proof.is_empty());

        let envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(proof.proof.as_ref()).expect("decode envelope");
        assert_eq!(envelope.circuit_id, KAIGI_ROSTER_BACKEND);
        assert_eq!(envelope.public_inputs, KAIGI_ROSTER_PUBLIC_INPUTS_DESC);
    }

    #[test]
    fn lane_relay_envelope_sample_uses_checked_validator_generation() {
        let sample =
            lane_relay_envelope_sample().expect("checked validator generation for relay sample");

        assert!(!sample.valid.is_empty());
        assert!(!sample.tampered.is_empty());
    }

    #[test]
    fn crypto_keypair_exports_checked_public_key_payload() {
        let seed = vec![0xA5; 32];
        let expected =
            KeyPair::try_from_seed(seed.clone(), Algorithm::Ed25519).expect("checked seed keypair");
        let (_, expected_public_key) = expected
            .public_key()
            .try_to_bytes()
            .expect("checked public-key payload");

        let keypair = crypto_keypair(Some("ed25519".to_owned()), Some(Uint8Array::from(seed)))
            .expect("derive keypair");

        assert_eq!(keypair.algorithm, Algorithm::Ed25519.as_static_str());
        assert_eq!(keypair.public_key.as_ref(), expected_public_key);
    }

    #[test]
    fn ed25519_keypair_derives_checked_public_key_payload() {
        let seed = vec![0x5C; 32];
        let expected =
            KeyPair::try_from_seed(seed.clone(), Algorithm::Ed25519).expect("checked seed keypair");
        let (_, expected_public_key) = expected
            .public_key()
            .try_to_bytes()
            .expect("checked public-key payload");

        let keypair = ed25519_keypair(Some(Uint8Array::from(seed))).expect("derive keypair");

        assert_eq!(keypair.algorithm, Algorithm::Ed25519.as_static_str());
        assert_eq!(keypair.public_key.as_ref(), expected_public_key);
    }

    #[test]
    fn crypto_keypair_random_path_uses_checked_generation() {
        let keypair = crypto_keypair(Some("ed25519".to_owned()), None)
            .expect("checked random keypair generation");

        assert_eq!(keypair.algorithm, Algorithm::Ed25519.as_static_str());
        assert_eq!(keypair.public_key.len(), 32);
    }

    #[test]
    fn crypto_public_key_from_private_exports_checked_payload() {
        let mut private_key_bytes = [0u8; 32];
        private_key_bytes[31] = 1;
        let private_key =
            PrivateKey::from_bytes(Algorithm::Secp256k1, &private_key_bytes).expect("private key");
        let public_key = PublicKey::from(private_key);
        let (_, expected_public_key) = public_key
            .try_to_bytes()
            .expect("checked public-key payload");

        let public_key = crypto_public_key_from_private(
            "secp256k1".to_owned(),
            Uint8Array::from(private_key_bytes.to_vec()),
        )
        .expect("derive public key");

        assert_eq!(public_key.as_ref(), expected_public_key);
    }

    #[test]
    fn crypto_sign_exports_verifiable_signature() {
        let seed = vec![0x33; 32];
        let message = b"js-host-crypto-sign";
        let keypair =
            KeyPair::try_from_seed(seed.clone(), Algorithm::Ed25519).expect("checked seed keypair");
        let (_, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("checked public-key payload");

        let signature = crypto_sign(
            "ed25519".to_owned(),
            Uint8Array::from(seed),
            Uint8Array::from(message.to_vec()),
        )
        .expect("crypto sign");

        let verified = crypto_verify(
            "ed25519".to_owned(),
            Uint8Array::from(public_key.to_vec()),
            Uint8Array::from(message.to_vec()),
            Uint8Array::from(signature.as_ref().to_vec()),
        )
        .expect("crypto verify");
        assert!(verified);
    }

    #[test]
    fn crypto_multihash_helpers_use_checked_formatters() {
        let seed = vec![0x5A; 32];
        let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
        let (_, public_payload) = keypair
            .public_key()
            .try_to_bytes()
            .expect("checked public-key payload");
        let expected_public = keypair
            .public_key()
            .try_to_multihash_string()
            .expect("checked public-key multihash");
        let expected_private = ExposedPrivateKey(keypair.private_key().clone())
            .try_to_multihash_string()
            .expect("checked private-key multihash");
        let (_, private_payload) = keypair.private_key().to_bytes();

        assert_eq!(
            crypto_public_key_multihash(
                "ed25519".to_owned(),
                Uint8Array::from(public_payload.to_vec()),
            )
            .expect("format public key multihash"),
            expected_public
        );
        assert_eq!(
            crypto_private_key_multihash("ed25519".to_owned(), Uint8Array::from(private_payload),)
                .expect("format private key multihash"),
            expected_private
        );
    }

    #[test]
    fn sm2_fixture_from_seed_uses_checked_public_key_formatters() {
        let distid = "js-sm2-fixture".to_owned();
        let seed = b"js-sm2-fixture-seed".to_vec();
        let message = b"js-sm2-fixture-message".to_vec();
        let fixture = sm2_fixture_from_seed(
            distid.clone(),
            Uint8Array::from(seed.clone()),
            Uint8Array::from(message),
        )
        .expect("build SM2 fixture");

        let private = Sm2PrivateKey::from_seed(&distid, &seed).expect("derive SM2 key");
        let public_bytes = private.public_key().to_sec1_bytes(false);
        let payload =
            encode_sm2_public_key_payload(&distid, &public_bytes).expect("SM2 public payload");
        let public_key = PublicKey::from_bytes(Algorithm::Sm2, &payload).expect("SM2 public key");

        assert_eq!(
            fixture.public_key_multihash,
            public_key
                .try_to_multihash_string()
                .expect("checked SM2 public-key multihash")
        );
        assert_eq!(
            fixture.public_key_prefixed,
            public_key
                .try_to_prefixed_string()
                .expect("checked SM2 public-key prefixed multihash")
        );
    }

    #[test]
    fn alias_proof_fixture_uses_checked_council_signer_payload() {
        let fixture = sorafs_alias_proof_fixture(Some(JsAliasProofFixtureOptions {
            generated_at_unix: Some(10),
            expires_at_unix: Some(20),
            ..Default::default()
        }))
        .expect("build alias proof fixture");
        let proof_bytes = BASE64
            .decode(fixture.proof_b64.as_bytes())
            .expect("decode proof fixture");
        let bundle = decode_alias_proof(&proof_bytes).expect("decode alias proof");
        let keypair = KeyPair::from_private_key(
            PrivateKey::from_bytes(Algorithm::Ed25519, &[0x55; 32]).expect("seeded key"),
        )
        .expect("derive keypair");
        let (_, expected_signer) = keypair
            .public_key()
            .try_to_bytes()
            .expect("checked public-key payload");

        assert_eq!(bundle.council_signatures.len(), 1);
        assert_eq!(
            bundle.council_signatures[0].signer.as_slice(),
            expected_signer
        );
    }

    #[test]
    fn kagemusha_recursive_spend_bridge_abi_version_is_additive_seven() {
        assert_eq!(connect_norito_bridge_abi_version(), 7);
    }

    fn empty_kagemusha_record_bundle_archive_for_js_host() -> Vec<u8> {
        let record_bundle = iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
            bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                chain_id: "js-host-recursive-compact-empty-record"
                    .parse()
                    .expect("chain id"),
                asset: AssetDefinitionId::new(
                    DomainId::try_new("offline", "universal").expect("domain id"),
                    "kgm-js-compact-empty"
                        .parse()
                        .expect("asset definition name"),
                ),
                steps: Vec::new(),
            },
            verifier_records: Vec::new(),
        };
        to_bytes(&record_bundle).expect("encode empty Kagemusha record bundle")
    }

    fn recursive_compact_token_archive_for_js_host(
        verifier_key_name: String,
        bind_public_inputs_hash: bool,
    ) -> Vec<u8> {
        let public_inputs = iroha_data_model::offline::KagemushaFoldedPublicInputs {
            domain: iroha_data_model::offline::KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
            aggregation_mode:
                iroha_data_model::offline::KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
            chain_id: "js-host-recursive-compact-malformed"
                .parse()
                .expect("chain id"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("offline", "universal").expect("domain id"),
                "kgm-js-compact-malformed"
                    .parse()
                    .expect("asset definition name"),
            ),
            initial_root: [0x11; 32],
            final_root: [0x22; 32],
            hop_count: 1,
            nullifier_digest: Hash::new(b"js-host-recursive-compact-nullifiers"),
            output_commitment_digest: Hash::new(b"js-host-recursive-compact-outputs"),
            fold_digest: Hash::new(b"js-host-recursive-compact-fold"),
            aggregation_transcript_digest: [0x33; 32],
        };
        let public_inputs_hash = if bind_public_inputs_hash {
            public_inputs
                .public_inputs_hash()
                .expect("JS host recursive compact public-input hash")
        } else {
            Hash::new(b"forged-js-host-recursive-compact-hash")
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
        to_bytes(&token).expect("encode malformed recursive compact token")
    }

    fn malformed_recursive_compact_token_archive_for_js_host() -> Vec<u8> {
        recursive_compact_token_archive_for_js_host(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1.to_owned(),
            false,
        )
    }

    fn sentinel_spoofed_recursive_compact_token_archive_for_js_host() -> Vec<u8> {
        recursive_compact_token_archive_for_js_host(
            format!(
                "forged::{}",
                iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE
            ),
            true,
        )
    }

    fn recursive_compact_forged_vk_hash_token_archive_for_js_host(
        record_bundle: &iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
    ) -> Vec<u8> {
        recursive_compact_shape_token_archive_for_js_host(
            record_bundle,
            false,
            Some(sample_hash(0x4F)),
        )
    }

    fn recursive_compact_multi_row_token_archive_for_js_host(
        record_bundle: &iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
    ) -> Vec<u8> {
        recursive_compact_shape_token_archive_for_js_host(record_bundle, true, None)
    }

    fn recursive_compact_shape_token_archive_for_js_host(
        record_bundle: &iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
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
                    .expect("JS host Kagemusha hop proof hash"),
                proof_public_inputs_digest:
                    iroha_core::zk::kagemusha_fold_step_public_inputs_digest(&step.attachment.proof)
                        .expect("JS host Kagemusha hop public-input digest"),
                verifier_key_id: step.attachment.vk_ref.clone(),
                verifier_key_commitment: step
                    .attachment
                    .vk_commitment
                    .expect("JS host sample hop has verifier-key commitment"),
                verifier_key_poseidon_digest:
                    iroha_data_model::offline::kagemusha_verifier_key_poseidon_digest(
                        step.verifier_key.backend.as_str(),
                        &step.verifier_key.bytes,
                    )
                    .expect("JS host Kagemusha verifier key poseidon digest"),
            })
            .collect::<Vec<_>>();
        let evidence =
            iroha_data_model::offline::kagemusha_recursive_aggregation_evidence_from_steps(
                &record_bundle.bundle.chain_id,
                &record_bundle.bundle.asset,
                &verified_steps,
                4,
                sample_hash(0x61),
                iroha_core::zk::kagemusha_recursive_fixed_window_table_schedule_digest(4)
                    .expect("JS host recursive compact fixed-window schedule digest"),
                iroha_core::zk::kagemusha_recursive_fixed_window_shared_table_manifest_digest(4)
                    .expect("JS host recursive compact shared-table manifest digest"),
                sample_hash(0x62),
                sample_hash(0x63),
            )
            .expect("JS host recursive compact shape evidence");
        let public_inputs =
            iroha_data_model::offline::kagemusha_folded_public_inputs_from_aggregation_statement(
                &evidence.aggregation_statement,
            )
            .expect("JS host recursive compact folded public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("JS host recursive compact public-input hash");
        let mut recursive_public_inputs =
            iroha_data_model::offline::kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(
                &evidence,
            )
            .expect("JS host recursive compact proof public inputs");
        let mut scalar_projection = [0_u8; Hash::LENGTH];
        scalar_projection[0] = 7;
        recursive_public_inputs.recursive_verifier_scalar_projection_digest = scalar_projection;

        let compact_vk = iroha_core::zk::kagemusha_recursive_compact_payment_token_vk_box()
            .expect("JS host recursive compact vk");
        let mut proof_bytes = ZK1_ENVELOPE_PREFIX.to_vec();
        zk1_append_proof(&mut proof_bytes, &[0xC7; 64]);
        let mut instance_columns =
            iroha_core::zk::kagemusha_recursive_aggregation_proof_public_input_instance_values(
                &recursive_public_inputs,
            )
            .expect("JS host recursive compact public instance values")
            .public_instance_columns();
        instance_columns.push(vec![
            recursive_public_inputs.recursive_verifier_scalar_projection_digest,
        ]);
        if multi_row_instances {
            for (index, column) in instance_columns.iter_mut().enumerate() {
                let mut row = [0_u8; Hash::LENGTH];
                row[..8].copy_from_slice(
                    &(u64::try_from(index).expect("JS host test index fits u64") + 1).to_le_bytes(),
                );
                column.push(row);
            }
        }
        append_zk1_raw_instance_columns_for_js_host(&mut proof_bytes, instance_columns);
        let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
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
                    iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
                ),
                public_inputs_hash,
                proof: ProofBox::new(
                    iroha_core::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                    to_bytes(&envelope).expect("encode JS host multi-row compact envelope"),
                ),
            },
        };
        to_bytes(&token).expect("encode JS host multi-row recursive compact token")
    }

    fn recursive_compact_key_artifacts_for_js_host()
    -> &'static iroha_data_model::offline::KagemushaRecursiveCompactKeyArtifactsV1 {
        static KEY_ARTIFACTS: OnceLock<
            iroha_data_model::offline::KagemushaRecursiveCompactKeyArtifactsV1,
        > = OnceLock::new();
        KEY_ARTIFACTS.get_or_init(|| {
            iroha_core::zk::kagemusha_recursive_compact_payment_token_key_artifacts()
                .expect("JS host recursive compact key artifacts")
        })
    }

    fn recursive_compact_key_artifacts_archive_for_js_host() -> Vec<u8> {
        static ARCHIVE: OnceLock<Vec<u8>> = OnceLock::new();
        ARCHIVE
            .get_or_init(|| {
                to_bytes(recursive_compact_key_artifacts_for_js_host())
                    .expect("encode JS host recursive compact key artifacts")
            })
            .clone()
    }

    fn recursive_compact_verifier_keys_archive_for_js_host() -> Vec<u8> {
        static ARCHIVE: OnceLock<Vec<u8>> = OnceLock::new();
        ARCHIVE
            .get_or_init(|| {
                let verifier_keys = recursive_compact_key_artifacts_for_js_host()
                    .verifier_keys()
                    .expect("JS host recursive compact verifier keys");
                to_bytes(&verifier_keys).expect("encode JS host recursive compact verifier keys")
            })
            .clone()
    }

    #[test]
    fn kagemusha_recursive_compact_payment_token_js_host_rejects_malformed_inputs() {
        let empty_record = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(Vec::<u8>::new()),
            Uint8Array::from(vec![2]),
            Uint8Array::from(Vec::<u8>::new()),
        ) {
            Ok(_) => panic!("empty record bundle must reject"),
            Err(err) => err,
        };
        assert_eq!(empty_record.status, napi::Status::InvalidArg);
        assert!(empty_record.reason.contains("Kagemusha record bundle"));

        let malformed_record = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(vec![1]),
            Uint8Array::from(vec![2]),
            Uint8Array::from(Vec::<u8>::new()),
        ) {
            Ok(_) => panic!("malformed record bundle must reject"),
            Err(err) => err,
        };
        assert_eq!(malformed_record.status, napi::Status::InvalidArg);
        assert!(
            malformed_record
                .reason
                .contains("invalid Kagemusha record bundle archive")
        );

        let oversized_archive = vec![0u8; KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1];
        let oversized_record = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(oversized_archive.clone()),
            Uint8Array::from(vec![2]),
            Uint8Array::from(Vec::<u8>::new()),
        ) {
            Ok(_) => panic!("oversized record bundle must reject before Norito decode"),
            Err(err) => err,
        };
        assert_eq!(oversized_record.status, napi::Status::InvalidArg);
        assert!(
            oversized_record
                .reason
                .contains("Kagemusha record bundle archive must not exceed"),
            "unexpected oversized record error: {oversized_record}"
        );

        let malformed_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(empty_kagemusha_record_bundle_archive_for_js_host()),
            Uint8Array::from(vec![2]),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("malformed recursive compact Pallas archive must reject"),
            Err(err) => err,
        };
        assert_eq!(malformed_pallas.status, napi::Status::InvalidArg);
        assert!(
            malformed_pallas
                .reason
                .contains("invalid Kagemusha recursive compact Pallas open-envelope archive")
        );

        let oversized_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(empty_kagemusha_record_bundle_archive_for_js_host()),
            Uint8Array::from(oversized_archive.clone()),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("oversized recursive compact Pallas archive must reject before core preflight"),
            Err(err) => err,
        };
        assert_eq!(oversized_pallas.status, napi::Status::InvalidArg);
        assert!(
            oversized_pallas
                .reason
                .contains("pallasOpenEnvelopesArchive must not exceed"),
            "unexpected oversized Pallas error: {oversized_pallas}"
        );

        let (one_hop_record_bundle, one_hop_pallas_archive) =
            sample_real_current_hop_record_bundle_for_js_host();
        let detached_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(empty_kagemusha_record_bundle_archive_for_js_host()),
            Uint8Array::from(one_hop_pallas_archive.clone()),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("detached valid recursive compact Pallas archive must reject"),
            Err(err) => err,
        };
        assert_eq!(detached_pallas.status, napi::Status::InvalidArg);
        assert!(
            detached_pallas
                .reason
                .contains("invalid Kagemusha recursive compact record-backed Pallas preflight"),
            "unexpected detached recursive compact Pallas error: {detached_pallas}"
        );

        let one_hop_record_archive =
            to_bytes(&one_hop_record_bundle).expect("encode JS host one-hop record bundle");
        let mut extra_pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&one_hop_pallas_archive)
                .expect("decode JS host one-hop Pallas archive");
        extra_pallas_open_envelopes.push(
            extra_pallas_open_envelopes
                .first()
                .expect("one-hop Pallas archive contains one envelope")
                .clone(),
        );
        let extra_pallas_archive = norito::to_bytes(&extra_pallas_open_envelopes)
            .expect("encode JS host extra Pallas archive");
        let extra_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(one_hop_record_archive),
            Uint8Array::from(extra_pallas_archive),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact prover must reject extra valid Pallas opening archive"),
            Err(err) => err,
        };
        assert_eq!(extra_pallas.status, napi::Status::InvalidArg);
        assert!(
            extra_pallas
                .reason
                .contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                && extra_pallas.reason.contains("witness"),
            "unexpected extra recursive compact Pallas error: {extra_pallas}"
        );

        let (multi_hop_record_bundle, multi_hop_pallas_archive) =
            sample_two_hop_real_record_bundle_for_js_host();
        let multi_hop_record_archive =
            to_bytes(&multi_hop_record_bundle).expect("encode JS host multi-hop record bundle");
        let mut missing_pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&multi_hop_pallas_archive)
                .expect("decode JS host multi-hop Pallas archive");
        let mut reordered_pallas_open_envelopes = missing_pallas_open_envelopes.clone();
        missing_pallas_open_envelopes.pop();
        let missing_pallas_archive = norito::to_bytes(&missing_pallas_open_envelopes)
            .expect("encode JS host missing Pallas archive");
        let missing_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(multi_hop_record_archive.clone()),
            Uint8Array::from(missing_pallas_archive),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact prover must reject missing valid Pallas opening archive"),
            Err(err) => err,
        };
        assert_eq!(missing_pallas.status, napi::Status::InvalidArg);
        assert!(
            missing_pallas
                .reason
                .contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                && missing_pallas.reason.contains("witness"),
            "unexpected missing recursive compact Pallas error: {missing_pallas}"
        );
        let mut duplicated_pallas_open_envelopes = reordered_pallas_open_envelopes.clone();
        duplicated_pallas_open_envelopes.push(
            duplicated_pallas_open_envelopes
                .last()
                .expect("multi-hop Pallas archive contains envelopes")
                .clone(),
        );
        let duplicated_pallas_archive = norito::to_bytes(&duplicated_pallas_open_envelopes)
            .expect("encode JS host duplicated Pallas archive");
        let duplicated_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(multi_hop_record_archive.clone()),
            Uint8Array::from(duplicated_pallas_archive),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact prover must reject duplicated multi-hop valid Pallas opening archive"),
            Err(err) => err,
        };
        assert_eq!(duplicated_pallas.status, napi::Status::InvalidArg);
        assert!(
            duplicated_pallas
                .reason
                .contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                && !duplicated_pallas.reason.contains(
                    iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
                ),
            "unexpected duplicated recursive compact Pallas error: {duplicated_pallas}"
        );
        let mut forged_metadata_pallas_open_envelopes = reordered_pallas_open_envelopes.clone();
        forged_metadata_pallas_open_envelopes
            .first_mut()
            .expect("multi-hop Pallas archive contains envelopes")
            .domain_tag = Some(sample_hash(0xC7));
        let forged_metadata_pallas_archive =
            norito::to_bytes(&forged_metadata_pallas_open_envelopes)
                .expect("encode JS host forged-metadata Pallas archive");
        let forged_metadata_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(multi_hop_record_archive.clone()),
            Uint8Array::from(forged_metadata_pallas_archive),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact prover must reject forged multi-hop Pallas metadata"),
            Err(err) => err,
        };
        assert_eq!(forged_metadata_pallas.status, napi::Status::InvalidArg);
        assert!(
            forged_metadata_pallas
                .reason
                .contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                && !forged_metadata_pallas.reason.contains(
                    iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
                ),
            "unexpected forged-metadata recursive compact Pallas error: {forged_metadata_pallas}"
        );
        reordered_pallas_open_envelopes.reverse();
        let reordered_pallas_archive = norito::to_bytes(&reordered_pallas_open_envelopes)
            .expect("encode JS host reordered Pallas archive");
        let reordered_pallas = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(multi_hop_record_archive.clone()),
            Uint8Array::from(reordered_pallas_archive),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact prover must reject reordered valid Pallas opening archive"),
            Err(err) => err,
        };
        assert_eq!(reordered_pallas.status, napi::Status::InvalidArg);
        assert!(
            reordered_pallas
                .reason
                .contains("invalid Kagemusha recursive compact record-backed Pallas preflight")
                && !reordered_pallas.reason.contains(
                    iroha_core::zk::KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE
                ),
            "unexpected reordered recursive compact Pallas error: {reordered_pallas}"
        );
        let multi_hop = match kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(
            Uint8Array::from(multi_hop_record_archive),
            Uint8Array::from(multi_hop_pallas_archive),
            Uint8Array::from(recursive_compact_key_artifacts_archive_for_js_host().to_vec()),
        ) {
            Ok(token_archive) => token_archive,
            Err(err) => panic!("valid multi-hop recursive compact archive must produce a token: {err}"),
        };
        assert!(!multi_hop.is_empty());

        let malformed_token = match kagemusha_verify_recursive_compact_payment_token(
            Uint8Array::from(vec![1]),
            Uint8Array::from(recursive_compact_verifier_keys_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("malformed recursive compact token must reject"),
            Err(err) => err,
        };
        assert_eq!(malformed_token.status, napi::Status::InvalidArg);
        assert!(
            malformed_token
                .reason
                .contains("invalid Kagemusha recursive compact payment token archive")
        );

        let oversized_token = match kagemusha_verify_recursive_compact_payment_token(
            Uint8Array::from(oversized_archive),
            Uint8Array::from(recursive_compact_verifier_keys_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("oversized recursive compact token must reject before Norito decode"),
            Err(err) => err,
        };
        assert_eq!(oversized_token.status, napi::Status::InvalidArg);
        assert!(
            oversized_token
                .reason
                .contains("Kagemusha recursive compact payment token archive must not exceed"),
            "unexpected oversized token error: {oversized_token}"
        );

        let malformed_binding = match kagemusha_verify_recursive_compact_payment_token(
            Uint8Array::from(malformed_recursive_compact_token_archive_for_js_host()),
            Uint8Array::from(recursive_compact_verifier_keys_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact token with malformed binding must reject"),
            Err(err) => err,
        };
        assert_eq!(malformed_binding.status, napi::Status::InvalidArg);
        assert!(
            malformed_binding
                .reason
                .contains("public-input hash mismatch"),
            "unexpected malformed binding error: {malformed_binding}"
        );

        let forged_vk_hash = match kagemusha_verify_recursive_compact_payment_token(
            Uint8Array::from(recursive_compact_forged_vk_hash_token_archive_for_js_host(
                &one_hop_record_bundle,
            )),
            Uint8Array::from(recursive_compact_verifier_keys_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("recursive compact token with forged verifier-key hash must reject"),
            Err(err) => err,
        };
        assert_eq!(forged_vk_hash.status, napi::Status::InvalidArg);
        assert!(
            forged_vk_hash
                .reason
                .contains("envelope verifier-key hash mismatch"),
            "unexpected forged verifier-key hash error: {forged_vk_hash}"
        );

        let multi_row_token = match kagemusha_verify_recursive_compact_payment_token(
            Uint8Array::from(recursive_compact_multi_row_token_archive_for_js_host(
                &one_hop_record_bundle,
            )),
            Uint8Array::from(recursive_compact_verifier_keys_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => {
                panic!("JS host recursive compact verifier must reject multi-row public instances")
            }
            Err(err) => err,
        };
        assert_eq!(multi_row_token.status, napi::Status::InvalidArg);
        assert!(
            multi_row_token.reason.contains("exactly one row"),
            "unexpected JS host multi-row compact-token error: {multi_row_token}"
        );

        let sentinel_spoofed_binding = match kagemusha_verify_recursive_compact_payment_token(
            Uint8Array::from(sentinel_spoofed_recursive_compact_token_archive_for_js_host()),
            Uint8Array::from(recursive_compact_verifier_keys_archive_for_js_host().to_vec()),
        ) {
            Ok(_) => panic!("sentinel-spoofed recursive compact token must reject"),
            Err(err) => err,
        };
        assert_eq!(sentinel_spoofed_binding.status, napi::Status::InvalidArg);
        assert!(
            sentinel_spoofed_binding
                .reason
                .contains("circuit id `forged::"),
            "unexpected sentinel-spoofed recursive compact error: {sentinel_spoofed_binding}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_compact_projection_verifier_js_host_rejects_malformed_inputs() {
        let malformed_token =
            match kagemusha_verify_recursive_spend_compact_payment_token_projection(
                Uint8Array::from(vec![1]),
                Uint8Array::from(vec![2]),
            ) {
                Ok(_) => panic!("malformed projection token must reject"),
                Err(err) => err,
            };
        assert_eq!(malformed_token.status, napi::Status::InvalidArg);
        assert!(
            malformed_token
                .reason
                .contains("invalid Kagemusha recursive spend compact projection token archive"),
            "unexpected malformed projection token error: {malformed_token}"
        );

        let malformed_record =
            match kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height(
                Uint8Array::from(malformed_recursive_compact_token_archive_for_js_host()),
                Uint8Array::from(vec![2]),
                JsU64(2),
            ) {
                Ok(_) => panic!("malformed projection verifier record must reject"),
                Err(err) => err,
            };
        assert_eq!(malformed_record.status, napi::Status::InvalidArg);
        assert!(
            malformed_record.reason.contains(
                "invalid Kagemusha recursive spend compact projection verifier record archive"
            ),
            "unexpected malformed projection verifier record error: {malformed_record}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_compact_projection_js_host_binds_bundle() {
        let malformed = match kagemusha_recursive_spend_compact_payment_token_from_bundle(
            Uint8Array::from(vec![1]),
        ) {
            Ok(_) => panic!("malformed recursive spend bundle must reject"),
            Err(err) => err,
        };
        assert_eq!(malformed.status, napi::Status::InvalidArg);
        assert!(
            malformed
                .reason
                .contains("invalid Kagemusha recursive spend compact-token bundle archive"),
            "unexpected malformed projection error: {malformed}"
        );

        let bundle = sample_kagemusha_recursive_spend_bundle_for_js_host();
        let bundle_archive = norito::to_bytes(&bundle).expect("encode JS host spend bundle");
        let projected_archive = kagemusha_recursive_spend_compact_payment_token_from_bundle(
            Uint8Array::from(bundle_archive),
        )
        .expect("project JS host recursive spend compact token");
        let token: iroha_data_model::offline::KagemushaCompactPaymentToken =
            norito::decode_from_bytes(projected_archive.as_ref())
                .expect("decode projected JS host compact token");
        let expected =
            iroha_data_model::offline::kagemusha_recursive_spend_compact_payment_token_from_bundle(
                &bundle,
            )
            .expect("expected JS host compact projection");
        assert_eq!(token.public_inputs, expected.public_inputs);
        assert_eq!(
            token.folded_proof.verifier_key_id,
            bundle.recursive_proof.verifier_key_id
        );
        assert_eq!(
            token.folded_proof.proof.bytes,
            bundle.recursive_proof.proof.bytes
        );

        let mut forged_bundle = sample_kagemusha_recursive_spend_bundle_for_js_host();
        forged_bundle.recursive_proof.public_inputs_hash =
            Hash::new(b"js-host-forged-recursive-spend-public-input-hash");
        let forged_archive =
            norito::to_bytes(&forged_bundle).expect("encode forged JS host spend bundle");
        let forged = match kagemusha_recursive_spend_compact_payment_token_from_bundle(
            Uint8Array::from(forged_archive),
        ) {
            Ok(_) => panic!("forged recursive proof public-input binding must reject"),
            Err(err) => err,
        };
        assert_eq!(forged.status, napi::Status::InvalidArg);
        assert!(
            forged.reason.contains("public-input hash"),
            "unexpected forged projection error: {forged}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_transition_profile_init_enforces_request_block_height() {
        let mut base =
            sample_kagemusha_recursive_spend_transition_profile_init_request_for_js_host();
        window_first_recursive_spend_hop_record_for_js_host(&mut base.record_bundle);

        let err = match kagemusha_recursive_spend_transition_profile_init(Uint8Array::from(
            norito::to_bytes(&base).expect("encode no-height init transition request"),
        )) {
            Ok(_) => panic!("height-unbound current-hop record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("chain height"),
            "unexpected no-height init transition error: {err}"
        );

        let mut future = base.clone();
        future.block_height = Some(1);
        let err = match kagemusha_recursive_spend_transition_profile_init(Uint8Array::from(
            norito::to_bytes(&future).expect("encode future init transition request"),
        )) {
            Ok(_) => panic!("future current-hop record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("not active"),
            "unexpected future init transition error: {err}"
        );

        let mut in_window = base.clone();
        in_window.block_height = Some(2);
        let profile_archive = kagemusha_recursive_spend_transition_profile_init(Uint8Array::from(
            norito::to_bytes(&in_window).expect("encode in-window init transition request"),
        ))
        .expect("in-window current-hop record should build a transition profile");
        let profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(profile_archive.as_ref())
                .expect("decode JS host init transition profile");
        assert_eq!(profile.hop_count, 1);

        let mut withdrawn = base;
        withdrawn.block_height = Some(4);
        let err = match kagemusha_recursive_spend_transition_profile_init(Uint8Array::from(
            norito::to_bytes(&withdrawn).expect("encode withdrawn init transition request"),
        )) {
            Ok(_) => panic!("withdrawn current-hop record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("not active"),
            "unexpected withdrawn init transition error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_transition_profile_append_enforces_request_block_height() {
        let mut base =
            sample_kagemusha_recursive_spend_transition_profile_append_request_for_js_host();
        window_first_recursive_spend_hop_record_for_js_host(&mut base.record_bundle);

        let err = match kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
            norito::to_bytes(&base).expect("encode no-height append transition request"),
        )) {
            Ok(_) => panic!("height-unbound current-hop record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("chain height"),
            "unexpected no-height append transition error: {err}"
        );

        let mut future = base.clone();
        future.block_height = Some(1);
        let err = match kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
            norito::to_bytes(&future).expect("encode future append transition request"),
        )) {
            Ok(_) => panic!("future current-hop record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("not active"),
            "unexpected future append transition error: {err}"
        );

        let mut in_window = base.clone();
        in_window.block_height = Some(2);
        let profile_archive =
            kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
                norito::to_bytes(&in_window).expect("encode in-window append transition request"),
            ))
            .expect("in-window current-hop record should build an append transition profile");
        let profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(profile_archive.as_ref())
                .expect("decode JS host append transition profile");
        assert_eq!(
            profile.hop_count,
            in_window.previous_bundle.accumulator.hop_count + 1
        );

        let mut withdrawn = base;
        withdrawn.block_height = Some(4);
        let err = match kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
            norito::to_bytes(&withdrawn).expect("encode withdrawn append transition request"),
        )) {
            Ok(_) => panic!("withdrawn current-hop record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("not active"),
            "unexpected withdrawn append transition error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_host_helpers_reject_malformed_archives() {
        let err = match kagemusha_recursive_spend_lineage_witness_from_init_result(
            Uint8Array::from(vec![0x01, 0x02]),
            Uint8Array::from(vec![0x03, 0x04]),
        ) {
            Ok(_) => panic!("init lineage helper must reject malformed archives"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("invalid Kagemusha recursive spend"),
            "unexpected init lineage helper error: {err}"
        );

        let err = match kagemusha_recursive_spend_lineage_witness_append_result(
            Uint8Array::from(vec![0x01, 0x02]),
            Uint8Array::from(vec![0x03, 0x04]),
            Uint8Array::from(vec![0x05, 0x06]),
        ) {
            Ok(_) => panic!("append lineage helper must reject malformed archives"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("invalid Kagemusha recursive spend"),
            "unexpected append lineage helper error: {err}"
        );
    }

    fn assert_oversized_archive_rejected_for_js_host(
        label: &str,
        result: napi::Result<Buffer>,
        context: &str,
    ) {
        let err = match result {
            Ok(_) => panic!("{label} must reject oversized archives before Norito decode"),
            Err(err) => err,
        };
        assert_eq!(err.status, napi::Status::InvalidArg);
        assert!(
            err.reason.contains(context) && err.reason.contains("must not exceed"),
            "{label} oversized-archive error lost context: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_js_host_rejects_oversized_archives_before_decode() {
        fn oversized_archive() -> Uint8Array {
            Uint8Array::from(vec![0u8; KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES + 1])
        }

        assert_oversized_archive_rejected_for_js_host(
            "init",
            kagemusha_recursive_spend_init(oversized_archive()),
            "Kagemusha recursive spend init archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "append",
            kagemusha_recursive_spend_append(oversized_archive()),
            "Kagemusha recursive spend append archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "transition profile init",
            kagemusha_recursive_spend_transition_profile_init(oversized_archive()),
            "Kagemusha recursive spend transition profile init archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "transition profile append",
            kagemusha_recursive_spend_transition_profile_append(oversized_archive()),
            "Kagemusha recursive spend transition profile append archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "lineage append boundary",
            kagemusha_recursive_spend_lineage_append_boundary(oversized_archive()),
            "Kagemusha recursive spend lineage append boundary archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "verify",
            kagemusha_recursive_spend_verify(oversized_archive()),
            "Kagemusha recursive spend verify archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "redeem",
            kagemusha_recursive_spend_redeem(oversized_archive()),
            "Kagemusha recursive spend redeem archive",
        );

        let init_request_archive =
            norito::to_bytes(&sample_kagemusha_recursive_spend_init_request_for_js_host())
                .expect("encode JS host init request archive");
        let init_bundle_archive =
            norito::to_bytes(&sample_kagemusha_recursive_spend_bundle_for_js_host())
                .expect("encode JS host init bundle archive");
        assert_oversized_archive_rejected_for_js_host(
            "lineage witness init request",
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                oversized_archive(),
                Uint8Array::from(init_bundle_archive.clone()),
            ),
            "Kagemusha recursive spend lineage witness init request archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "lineage witness init bundle",
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                Uint8Array::from(init_request_archive.clone()),
                oversized_archive(),
            ),
            "Kagemusha recursive spend lineage witness init bundle archive",
        );

        let append_request_archive = norito::to_bytes(
            &sample_kagemusha_recursive_spend_transition_profile_append_request_for_js_host(),
        )
        .expect("encode JS host append request archive");
        let previous_witness_archive = norito::to_bytes(
            &sample_kagemusha_recursive_spend_lineage_witness_for_js_host(
                &sample_kagemusha_recursive_spend_bundle_for_js_host(),
            ),
        )
        .expect("encode JS host previous witness archive");
        assert_oversized_archive_rejected_for_js_host(
            "lineage witness append previous witness",
            kagemusha_recursive_spend_lineage_witness_append_result(
                oversized_archive(),
                Uint8Array::from(append_request_archive.clone()),
                Uint8Array::from(init_bundle_archive.clone()),
            ),
            "Kagemusha recursive spend previous lineage witness archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "lineage witness append request",
            kagemusha_recursive_spend_lineage_witness_append_result(
                Uint8Array::from(previous_witness_archive.clone()),
                oversized_archive(),
                Uint8Array::from(init_bundle_archive.clone()),
            ),
            "Kagemusha recursive spend lineage witness append request archive",
        );
        assert_oversized_archive_rejected_for_js_host(
            "lineage witness append bundle",
            kagemusha_recursive_spend_lineage_witness_append_result(
                Uint8Array::from(previous_witness_archive),
                Uint8Array::from(append_request_archive),
                oversized_archive(),
            ),
            "Kagemusha recursive spend lineage witness append bundle archive",
        );
    }

    fn assert_empty_nested_pallas_archive_rejected_for_js_host(
        label: &str,
        result: napi::Result<Buffer>,
    ) {
        let err = match result {
            Ok(_) => panic!("{label} must reject empty nested Pallas archives before core"),
            Err(err) => err,
        };
        assert_eq!(err.status, napi::Status::InvalidArg);
        assert!(
            err.reason.contains(
                "Kagemusha recursive spend Pallas open-envelope archive must not be empty"
            ),
            "{label} empty nested-Pallas error lost context: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_js_host_rejects_empty_nested_pallas_archives_before_core() {
        let mut init_request = sample_kagemusha_recursive_spend_init_request_for_js_host();
        init_request.pallas_open_envelopes_archive.clear();
        let init_request_archive =
            norito::to_bytes(&init_request).expect("encode empty-Pallas init request");
        assert_empty_nested_pallas_archive_rejected_for_js_host(
            "init",
            kagemusha_recursive_spend_init(Uint8Array::from(init_request_archive.clone())),
        );
        assert_empty_nested_pallas_archive_rejected_for_js_host(
            "transition profile init",
            kagemusha_recursive_spend_transition_profile_init(Uint8Array::from(
                init_request_archive.clone(),
            )),
        );
        assert_empty_nested_pallas_archive_rejected_for_js_host(
            "lineage witness init",
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                Uint8Array::from(init_request_archive),
                Uint8Array::from(
                    norito::to_bytes(&sample_kagemusha_recursive_spend_bundle_for_js_host())
                        .expect("encode JS host bundle archive"),
                ),
            ),
        );

        let mut append_request =
            sample_kagemusha_recursive_spend_transition_profile_append_request_for_js_host();
        append_request.pallas_open_envelopes_archive.clear();
        let append_request_archive =
            norito::to_bytes(&append_request).expect("encode empty-Pallas append request");
        assert_empty_nested_pallas_archive_rejected_for_js_host(
            "append",
            kagemusha_recursive_spend_append(Uint8Array::from(append_request_archive.clone())),
        );
        assert_empty_nested_pallas_archive_rejected_for_js_host(
            "transition profile append",
            kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
                append_request_archive.clone(),
            )),
        );
        let previous_witness_archive = norito::to_bytes(
            &sample_kagemusha_recursive_spend_lineage_witness_for_js_host(
                &sample_kagemusha_recursive_spend_bundle_for_js_host(),
            ),
        )
        .expect("encode JS host previous witness archive");
        assert_empty_nested_pallas_archive_rejected_for_js_host(
            "lineage witness append",
            kagemusha_recursive_spend_lineage_witness_append_result(
                Uint8Array::from(previous_witness_archive),
                Uint8Array::from(append_request_archive),
                Uint8Array::from(
                    norito::to_bytes(&sample_kagemusha_recursive_spend_bundle_for_js_host())
                        .expect("encode JS host append bundle archive"),
                ),
            ),
        );
    }

    #[test]
    fn kagemusha_recursive_spend_verify_requires_lineage_record_for_reserved_lineage() {
        let mut bundle = sample_kagemusha_recursive_spend_bundle_for_js_host();
        bundle.recursive_proof.verifier_key_id.name =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                .to_owned();
        bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest = sample_hash(0xE5);
        bundle.recursive_proof.public_inputs_hash = bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("lineage recursive spend public-input hash");
        let request = iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1 {
            bundle,
            lineage_verifier_record: None,
            block_height: None,
        };
        let archive = norito::to_bytes(&request).expect("encode recursive spend verify request");

        let err = match kagemusha_recursive_spend_verify(Uint8Array::from(archive)) {
            Ok(_) => panic!("reserved lineage verify request without a record must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("lineage_verifier_record"),
            "reserved lineage verification did not reject the malformed request: {err}"
        );

        let mut forged_record =
            sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host();
        forged_record.commitment = sample_hash(0xE6);
        let forged_request = iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1 {
            bundle: request.bundle.clone(),
            lineage_verifier_record: Some(forged_record),
            block_height: None,
        };
        let forged_archive =
            norito::to_bytes(&forged_request).expect("encode forged-lineage verify request");
        let err = match kagemusha_recursive_spend_verify(Uint8Array::from(forged_archive)) {
            Ok(_) => panic!("forged lineage verify request must reject"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("lineage_verifier_record.commitment"),
            "forged lineage verifier record was not rejected clearly: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_verify_enforces_request_block_height() {
        fn verify_result(
            request: &iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1,
        ) -> iroha_data_model::offline::KagemushaRecursiveSpendVerifyResultV1 {
            let archive = norito::to_bytes(request).expect("encode recursive spend verify request");
            let output = kagemusha_recursive_spend_verify(Uint8Array::from(archive))
                .expect("verify function returns a diagnostic result archive");
            norito::decode_from_bytes(output.as_ref()).expect("decode recursive spend result")
        }

        let mut record = sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host();
        record.activation_height = Some(2);
        record.withdraw_height = Some(4);
        let base = iroha_data_model::offline::KagemushaRecursiveSpendVerifyRequestV1 {
            bundle: sample_reserved_lineage_previous_bundle_for_js_host(),
            lineage_verifier_record: Some(record),
            block_height: None,
        };

        let no_height = verify_result(&base);
        assert!(!no_height.valid);
        assert!(!no_height.chain_admissible);
        assert!(!no_height.witnessless_redeem_supported);
        assert!(no_height.lineage_witness_required_for_redeem);
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
            !in_window.reason.contains("chain height") && !in_window.reason.contains("not active"),
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
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_forged_previous_proof_opening_metadata() {
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
            let previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
            let mut previous_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(
                    &sample_previous_recursive_proof_open_envelopes_archive_for_js_host(
                        &previous_bundle,
                    ),
                )
                .expect("decode JS host previous proof open envelopes");
            let envelope = previous_open_envelopes
                .first_mut()
                .expect("previous proof archive contains one envelope");
            match case {
                "vk_commitment" => envelope.vk_commitment = Some(sample_hash(0xD1)),
                "public_inputs_schema_hash" => {
                    envelope.public_inputs_schema_hash = Some(sample_hash(0xD2));
                }
                "domain_tag" => envelope.domain_tag = Some(sample_hash(0xD3)),
                _ => unreachable!("covered previous-proof opening metadata case"),
            }

            let request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
                previous_bundle: previous_bundle.clone(),
                previous_lineage_verifier_record: Some(
                    sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
                ),
                record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                    bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                        chain_id: previous_bundle.accumulator.chain_id.clone(),
                        asset: previous_bundle.accumulator.asset.clone(),
                        steps: Vec::new(),
                    },
                    verifier_records: Vec::new(),
                },
                pallas_open_envelopes_archive: Vec::new(),
                current_note: previous_bundle.accumulator.current_note.clone(),
                output_proof_circuit_id:
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                        .to_owned(),
                previous_recursive_proof_open_envelopes_archive: norito::to_bytes(
                    &previous_open_envelopes,
                )
                .expect("encode forged JS host previous proof open archive"),
                lineage_verifier_key: None,
                lineage_proving_key_archive: None,
                block_height: None,
            };
            let archive = norito::to_bytes(&request).expect("encode JS host append request");

            let err = match kagemusha_recursive_spend_append(Uint8Array::from(archive)) {
                Ok(_) => panic!("JS host must reject forged previous-proof opening metadata"),
                Err(err) => err,
            };
            assert!(
                err.reason.contains(expected_field),
                "{case} metadata splice returned unexpected error: {err}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_spend_init_rejects_forged_current_hop_pallas_metadata() {
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
            let mut request = sample_kagemusha_recursive_spend_init_request_for_js_host();
            let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                norito::decode_from_bytes(&request.pallas_open_envelopes_archive)
                    .expect("decode JS host current-hop Pallas archive");
            let envelope = envelopes
                .first_mut()
                .expect("current-hop Pallas archive contains one envelope");
            match case {
                "vk_commitment" => envelope.vk_commitment = Some(sample_hash(0xD4)),
                "public_inputs_schema_hash" => {
                    envelope.public_inputs_schema_hash = Some(sample_hash(0xD5));
                }
                _ => unreachable!("covered current-hop Pallas metadata case"),
            }
            request.pallas_open_envelopes_archive =
                norito::to_bytes(&envelopes).expect("encode forged JS host Pallas archive");
            let archive = norito::to_bytes(&request).expect("encode JS host init request");

            let err = match kagemusha_recursive_spend_init(Uint8Array::from(archive)) {
                Ok(_) => panic!("JS host must reject forged current-hop Pallas metadata"),
                Err(err) => err,
            };
            assert!(
                err.reason.contains(expected_field),
                "{case} current-hop metadata splice returned unexpected error: {err}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_spend_init_rejects_forged_current_hop_proof_circuit_id() {
        let mut request = sample_kagemusha_recursive_spend_init_request_for_js_host();
        let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope = norito::decode_from_bytes(
            &request.record_bundle.bundle.steps[0].attachment.proof.bytes,
        )
        .expect("decode JS host current-hop proof envelope");
        envelope.circuit_id = "forged-js-host-current-hop-proof-circuit-id".to_owned();
        request.record_bundle.bundle.steps[0].attachment.proof.bytes =
            norito::to_bytes(&envelope).expect("encode forged JS host current-hop proof envelope");
        let archive = norito::to_bytes(&request).expect("encode JS host init request");

        let err = match kagemusha_recursive_spend_init(Uint8Array::from(archive)) {
            Ok(_) => panic!("JS host must reject forged current-hop proof circuit id"),
            Err(err) => err,
        };
        assert!(
            err.reason
                .contains("lineage_witness.record_bundle.bundle.steps.attachment.proof.circuit_id"),
            "current-hop proof circuit-id splice returned unexpected error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_init_rejects_missing_lineage_key_artifacts() {
        let request = sample_kagemusha_recursive_spend_init_request_for_js_host();
        let archive = norito::to_bytes(&request).expect("encode JS host init request");

        let err = match kagemusha_recursive_spend_init(Uint8Array::from(archive)) {
            Ok(_) => panic!("JS host must reject missing Reserved-lineage key artifacts"),
            Err(err) => err,
        };
        assert!(
            err.reason.contains("lineage_verifier_key"),
            "missing lineage key artifacts returned unexpected error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_missing_lineage_key_artifacts() {
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
                Some(iroha_data_model::proof::VerifyingKeyBox::new(
                    "halo2/ipa".to_owned(),
                    vec![0xAB; 32],
                )),
                None,
            ),
            (
                "missing verifier key",
                "lineage_verifier_key",
                None,
                Some(vec![0xAC; 32]),
            ),
        ] {
            let mut request =
                sample_reserved_lineage_append_request_missing_key_artifacts_for_js_host();
            request.lineage_verifier_key = verifier_key;
            request.lineage_proving_key_archive = proving_key_archive;
            let archive = norito::to_bytes(&request).expect("encode JS host append request");

            let err = match kagemusha_recursive_spend_append(Uint8Array::from(archive)) {
                Ok(_) => {
                    panic!("JS host must reject missing Reserved-lineage key artifacts: {case}")
                }
                Err(err) => err,
            };
            assert!(
                err.reason.contains(expected_field),
                "{case} returned unexpected error: {err}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_malformed_previous_proof_opening_archives() {
        let previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
        let canonical_archive =
            sample_previous_recursive_proof_open_envelopes_archive_for_js_host(&previous_bundle);
        let previous_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&canonical_archive)
                .expect("decode JS host previous proof open envelopes");
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
                    .expect("encode JS host empty previous proof open archive"),
            ),
            (
                "over-count previous-proof opening vector",
                norito::to_bytes(&vec![
                    previous_open_envelope.clone(),
                    previous_open_envelope.clone(),
                ])
                .expect("encode JS host over-count previous proof open archive"),
            ),
        ] {
            let request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
                previous_bundle: previous_bundle.clone(),
                previous_lineage_verifier_record: Some(
                    sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
                ),
                record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                    bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                        chain_id: previous_bundle.accumulator.chain_id.clone(),
                        asset: previous_bundle.accumulator.asset.clone(),
                        steps: Vec::new(),
                    },
                    verifier_records: Vec::new(),
                },
                pallas_open_envelopes_archive: Vec::new(),
                current_note: previous_bundle.accumulator.current_note.clone(),
                output_proof_circuit_id:
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                        .to_owned(),
                previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
                lineage_verifier_key: None,
                lineage_proving_key_archive: None,
                block_height: None,
            };
            let archive = norito::to_bytes(&request).expect("encode JS host append request");

            let err = match kagemusha_recursive_spend_append(Uint8Array::from(archive)) {
                Ok(_) => panic!("JS host must reject {case}"),
                Err(err) => err,
            };
            assert!(
                err.reason
                    .contains("previous_recursive_proof_open_envelopes_archive"),
                "{case} returned unexpected error: {err}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_stale_previous_proof_payload_opening() {
        let mut previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
        let previous_proof_open_archive =
            sample_previous_recursive_proof_open_envelopes_archive_for_js_host(&previous_bundle);
        let mut previous_proof_envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&previous_bundle.recursive_proof.proof.bytes)
                .expect("decode JS host previous recursive proof envelope");
        previous_proof_envelope.proof_bytes.push(0x42);
        previous_bundle.recursive_proof.proof.bytes = norito::to_bytes(&previous_proof_envelope)
            .expect("encode JS host stale previous recursive proof envelope");

        let request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle: previous_bundle.clone(),
            previous_lineage_verifier_record: Some(
                sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
            ),
            record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                    chain_id: previous_bundle.accumulator.chain_id.clone(),
                    asset: previous_bundle.accumulator.asset.clone(),
                    steps: Vec::new(),
                },
                verifier_records: Vec::new(),
            },
            pallas_open_envelopes_archive: Vec::new(),
            current_note: previous_bundle.accumulator.current_note.clone(),
            output_proof_circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
            previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        let archive = norito::to_bytes(&request).expect("encode JS host append request");

        let err = match kagemusha_recursive_spend_append(Uint8Array::from(archive)) {
            Ok(_) => panic!("JS host must reject stale previous-proof payload opening"),
            Err(err) => err,
        };
        assert!(
            err.reason
                .contains("previous_recursive_proof_open_envelopes_archive.domain_tag"),
            "stale previous-proof payload returned unexpected error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_forged_previous_proof_circuit_id() {
        let mut previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
        let previous_proof_open_archive =
            sample_previous_recursive_proof_open_envelopes_archive_for_js_host(&previous_bundle);
        let mut previous_proof_envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&previous_bundle.recursive_proof.proof.bytes)
                .expect("decode JS host previous recursive proof envelope");
        previous_proof_envelope.circuit_id =
            "forged-js-host-previous-recursive-proof-circuit-id".to_owned();
        previous_bundle.recursive_proof.proof.bytes = norito::to_bytes(&previous_proof_envelope)
            .expect("encode JS host previous recursive proof envelope with forged circuit id");

        let request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle: previous_bundle.clone(),
            previous_lineage_verifier_record: Some(
                sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
            ),
            record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                    chain_id: previous_bundle.accumulator.chain_id.clone(),
                    asset: previous_bundle.accumulator.asset.clone(),
                    steps: Vec::new(),
                },
                verifier_records: Vec::new(),
            },
            pallas_open_envelopes_archive: Vec::new(),
            current_note: previous_bundle.accumulator.current_note.clone(),
            output_proof_circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
            previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        let archive = norito::to_bytes(&request).expect("encode JS host append request");

        let err = match kagemusha_recursive_spend_append(Uint8Array::from(archive)) {
            Ok(_) => panic!("JS host must reject forged previous recursive proof circuit id"),
            Err(err) => err,
        };
        assert!(
            err.reason
                .contains("previous_bundle.recursive_proof.proof.circuit_id"),
            "forged previous proof circuit-id returned unexpected error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_transition_profile_append_binds_append_opening_preflight() {
        let (record_bundle, pallas_open_envelopes_archive) =
            sample_real_current_hop_record_bundle_for_js_host();
        let pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&pallas_open_envelopes_archive)
                .expect("decode JS host real current-hop Pallas archive");
        let evidence =
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes(
                &record_bundle,
                &pallas_open_envelopes,
            )
            .expect("derive JS host real current-hop recursive aggregation evidence");
        let step = record_bundle
            .bundle
            .steps
            .first()
            .expect("real current-hop record bundle has one hop");

        let mut previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
        previous_bundle.accumulator.chain_id = record_bundle.bundle.chain_id.clone();
        previous_bundle.accumulator.asset = record_bundle.bundle.asset.clone();
        previous_bundle.accumulator.final_root = step.root_before;
        previous_bundle.accumulator.current_note =
            iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: sample_hash(0xC1),
                spend_nullifier: step.input_nullifiers[0],
                amount: Numeric::new(7, 0),
            };
        previous_bundle.accumulator.verifier_opening_len = evidence.verifier_opening_len;
        previous_bundle.accumulator.verifier_params_fingerprint =
            evidence.verifier_params_fingerprint;
        previous_bundle
            .accumulator
            .fixed_window_table_schedule_digest = evidence.fixed_window_table_schedule_digest;
        previous_bundle
            .accumulator
            .fixed_window_shared_table_manifest_digest =
            evidence.fixed_window_shared_table_manifest_digest;
        refresh_reserved_lineage_previous_bundle_public_inputs_for_js_host(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope_for_js_host(
            &mut previous_bundle,
            sample_hash(0xC2),
        );

        let previous_proof_open_archive =
            sample_previous_recursive_proof_open_envelopes_archive_for_js_host(&previous_bundle);
        let current_note = iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step.output_commitments[0],
            spend_nullifier: sample_hash(0xC3),
            amount: Numeric::new(7, 0),
        };
        let mut request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            previous_lineage_verifier_record: Some(
                sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
            ),
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
            output_proof_circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
            previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        request
            .validate_public_binding()
            .expect("JS host append request with previous proof openings is well formed");

        let profile_archive =
            kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
                norito::to_bytes(&request)
                    .expect("encode JS host append transition-profile request"),
            ))
            .expect("JS host append transition profile with previous proof openings");
        let profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(profile_archive.as_ref())
                .expect("decode JS host append transition profile");
        let append_opening_preflight_digest = profile
            .append_opening_preflight_digest
            .expect("JS host append profile binds append opening preflight digest");
        assert_ne!(
            append_opening_preflight_digest,
            [0u8; Hash::LENGTH],
            "JS host append opening preflight digest must be non-zero"
        );
        assert!(
            profile
                .previous_recursive_proof_open_envelopes_archive_digest
                .is_some(),
            "JS host append profile must retain the previous-proof opening archive digest"
        );
        let append_opening_preflight = profile
            .append_opening_preflight
            .as_ref()
            .expect("JS host append profile binds full append opening preflight contract");
        assert_eq!(
            append_opening_preflight.append_opening_preflight_digest,
            append_opening_preflight_digest,
            "JS host append profile contract digest must match the profile digest field"
        );
        assert_eq!(
            Some(append_opening_preflight.previous_recursive_proof_open_envelopes_archive_digest),
            profile.previous_recursive_proof_open_envelopes_archive_digest,
            "JS host append profile contract must bind the previous opening archive digest"
        );
        assert_eq!(
            append_opening_preflight.current_hop_proof_hash,
            profile.current_hop_statement.proof_hash,
            "JS host append profile contract must bind the current-hop proof hash"
        );

        let mut forged_current_hop_opening = request.clone();
        let mut forged_current_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&forged_current_hop_opening.pallas_open_envelopes_archive)
                .expect("decode JS host current-hop Pallas archive");
        forged_current_envelopes[0].domain_tag = Some(sample_hash(0xC4));
        forged_current_hop_opening.pallas_open_envelopes_archive =
            norito::to_bytes(&forged_current_envelopes)
                .expect("encode JS host forged current-hop Pallas archive");
        let err = match kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
            norito::to_bytes(&forged_current_hop_opening)
                .expect("encode JS host forged current-hop transition request"),
        )) {
            Ok(_) => {
                panic!("JS host append profile must reject forged current-hop opening metadata")
            }
            Err(err) => err,
        };
        assert!(
            err.reason.contains("hop domain metadata mismatch"),
            "JS host forged current-hop opening returned unexpected error: {err}"
        );

        request
            .previous_recursive_proof_open_envelopes_archive
            .clear();
        let legacy_profile_archive =
            kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
                norito::to_bytes(&request)
                    .expect("encode JS host legacy append transition-profile request"),
            ))
            .expect("JS host legacy append transition profile without previous proof openings");
        let legacy_profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(legacy_profile_archive.as_ref())
                .expect("decode JS host legacy append transition profile");
        assert_eq!(
            legacy_profile.append_opening_preflight_digest, None,
            "JS host legacy append profiles must not synthesize append opening preflight bytes"
        );
        assert_eq!(
            legacy_profile.append_opening_preflight, None,
            "JS host legacy append profiles must not synthesize append opening preflight contracts"
        );
        assert_eq!(
            legacy_profile.previous_recursive_proof_open_envelopes_archive_digest, None,
            "JS host legacy append profiles must not bind absent previous proof opening bytes"
        );
        let profile_digest =
            iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_digest(
                &profile,
            )
            .expect("JS host append opening profile digest");
        let legacy_profile_digest =
            iroha_data_model::offline::kagemusha_recursive_spend_transition_profile_digest(
                &legacy_profile,
            )
            .expect("JS host legacy append profile digest");
        assert_ne!(
            profile_digest, legacy_profile_digest,
            "JS host append opening preflight must change the profile digest"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_append_boundary_rejects_duplicate_current_outputs() {
        let request =
            sample_kagemusha_recursive_spend_transition_profile_append_request_for_js_host();
        let profile_archive =
            kagemusha_recursive_spend_transition_profile_append(Uint8Array::from(
                norito::to_bytes(&request)
                    .expect("encode JS host append transition-profile request"),
            ))
            .expect("JS host append transition profile should build");
        let mut profile: iroha_data_model::offline::KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(profile_archive.as_ref())
                .expect("decode JS host append transition profile");
        profile
            .current_hop_statement
            .output_commitments
            .push(profile.current_hop_statement.output_commitments[0]);

        let duplicate_output_profile_archive = norito::to_bytes(&profile)
            .expect("encode JS host append transition profile with duplicate output");
        let err = match kagemusha_recursive_spend_lineage_append_boundary(Uint8Array::from(
            duplicate_output_profile_archive,
        )) {
            Ok(_) => {
                panic!("JS host append-boundary helper must reject duplicate current-hop outputs")
            }
            Err(err) => err,
        };
        assert!(
            err.reason.contains("repeats an output commitment"),
            "JS host duplicate-output append-boundary error lost context: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_instruction_rejects_semantic_profile_after_public_binding()
    {
        let request = sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        request
            .validate_public_binding()
            .expect("semantic recursive spend redeem request has valid public bindings");
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(request) {
            Ok(_) => panic!("semantic recursive spend redeem request must reject"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("private-hop lineage"),
            "unexpected semantic-profile error: {err}"
        );

        let wrong_amount = sample_kagemusha_recursive_spend_redeem_request_for_js_host(41);
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(wrong_amount) {
            Ok(_) => panic!("wrong recursive spend redeem amount must reject"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("public_amount"),
            "unexpected wrong-amount error: {err}"
        );

        let mut missing_anchor = sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        missing_anchor
            .bundle
            .accumulator
            .topup_anchor_nullifiers
            .clear();
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(missing_anchor) {
            Ok(_) => panic!("missing recursive spend top-up anchor must reject"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("topup_anchor_nullifiers"),
            "unexpected missing-anchor error: {err}"
        );

        let mut missing_vk_commitment =
            sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        missing_vk_commitment.redeem_proof.vk_commitment = None;
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(
            missing_vk_commitment,
        ) {
            Ok(_) => panic!("missing recursive spend redeem VK commitment must reject"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("vk_commitment"),
            "unexpected missing-VK-commitment error: {err}"
        );

        let mut zero_vk_commitment =
            sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        zero_vk_commitment.redeem_proof.vk_commitment = Some([0u8; Hash::LENGTH]);
        let err =
            match kagemusha_recursive_spend_redeem_instruction_from_request(zero_vk_commitment) {
                Ok(_) => panic!("zero recursive spend redeem VK commitment must reject"),
                Err(err) => err,
            };
        assert!(
            err.to_string().contains("vk_commitment"),
            "unexpected zero-VK-commitment error: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_instruction_requires_lineage_record_for_reserved_previous_proof()
     {
        let request =
            sample_kagemusha_recursive_spend_mixed_reserved_previous_redeem_request_for_js_host();
        let err = request
            .validate_public_binding()
            .expect_err("semantic final redeem with reserved previous proof requires record");
        assert!(
            err.to_string().contains("lineage_verifier_record"),
            "unexpected missing lineage-record error: {err}"
        );
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(request.clone()) {
            Ok(_) => panic!("JS host must reject mixed lineage redeem without lineage record"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("lineage_verifier_record"),
            "unexpected mixed-lineage rejection: {err}"
        );

        let mut with_record = request;
        with_record.lineage_verifier_record =
            Some(sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host());
        with_record
            .validate_public_binding()
            .expect("semantic final redeem accepts reserved previous proof with lineage record");

        let mut wrong_record_circuit = with_record.clone();
        wrong_record_circuit
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .circuit_id =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
                .to_owned();
        let err = wrong_record_circuit
            .validate_public_binding()
            .expect_err("semantic final redeem must reject mismatched previous lineage record");
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.circuit_id"),
            "unexpected wrong-circuit lineage-record public-binding error: {err}"
        );
        let err =
            match kagemusha_recursive_spend_redeem_instruction_from_request(wrong_record_circuit) {
                Ok(_) => {
                    panic!("JS host must reject lineage verifier-record circuit-id mismatch")
                }
                Err(err) => err,
            };
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.circuit_id"),
            "unexpected wrong-circuit lineage-record rejection: {err}"
        );

        let mut forged_record = with_record.clone();
        forged_record
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .commitment = sample_hash(0xE7);
        let err = forged_record
            .validate_public_binding()
            .expect_err("semantic final redeem must reject forged lineage verifier record");
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.commitment"),
            "unexpected forged lineage-record public-binding error: {err}"
        );
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(forged_record) {
            Ok(_) => panic!("JS host must reject forged lineage verifier-record commitment"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.commitment"),
            "unexpected forged lineage-record rejection: {err}"
        );
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_instruction_rejects_backend_invalid_lineage() {
        let mut request = sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        attach_strict_reserved_lineage_envelope_for_js_host(&mut request);
        let mut lineage_record =
            sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host();
        lineage_record.max_proof_bytes = u32::try_from(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
        )
        .expect("recursive proof envelope byte cap fits u32");
        request.lineage_verifier_record = Some(lineage_record);
        request.validate_public_binding().expect(
            "witnessless reserved-lineage redeem validates before backend proof verification",
        );

        let mut wrong_record_circuit = request.clone();
        wrong_record_circuit
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .circuit_id =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
                .to_owned();
        let err = wrong_record_circuit
            .validate_public_binding()
            .expect_err("reserved-lineage redeem must reject mismatched final lineage record");
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.circuit_id"),
            "unexpected wrong-circuit final lineage-record error: {err}"
        );
        let err =
            match kagemusha_recursive_spend_redeem_instruction_from_request(wrong_record_circuit) {
                Ok(_) => {
                    panic!("JS host must reject final lineage verifier-record circuit-id mismatch")
                }
                Err(err) => err,
            };
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.circuit_id"),
            "unexpected wrong-circuit final lineage-record rejection: {err}"
        );

        let mut forged_record = request.clone();
        forged_record
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .commitment = sample_hash(0xE8);
        let err = forged_record
            .validate_public_binding()
            .expect_err("reserved-lineage redeem must reject forged verifier record");
        assert!(
            err.to_string()
                .contains("lineage_verifier_record.commitment"),
            "unexpected forged reserved-lineage verifier-record error: {err}"
        );

        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(request.clone()) {
            Ok(_) => {
                panic!("JS host must reject backend-invalid witnessless reserved-lineage redeem")
            }
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("inline key"),
            "unexpected backend-invalid lineage rejection: {err}"
        );

        let mut missing_lineage_slice =
            sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        attach_reserved_lineage_envelope_for_js_host(&mut missing_lineage_slice, false);
        missing_lineage_slice.lineage_verifier_record =
            Some(sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host());
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(
            missing_lineage_slice,
        ) {
            Ok(_) => panic!("reserved-lineage redeem without verifier-slice columns must reject"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("verifier-slice")
                || err.to_string().contains("public instance columns"),
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
        let err = match kagemusha_recursive_spend_redeem_instruction_from_request(missing_scalar) {
            Ok(_) => panic!("zero lineage scalar projection must reject"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("recursive_verifier_scalar_projection_digest"),
            "unexpected zero-lineage-scalar error: {err}"
        );

        let mut malformed_envelope = request;
        malformed_envelope.bundle.recursive_proof.proof =
            ProofBox::new("halo2/ipa".to_owned(), vec![0xA5; 64]);
        let err =
            match kagemusha_recursive_spend_redeem_instruction_from_request(malformed_envelope) {
                Ok(_) => panic!("malformed reserved lineage proof envelope must reject"),
                Err(err) => err,
            };
        assert!(
            err.to_string()
                .contains("failed to decode recursive spend lineage proof envelope"),
            "unexpected malformed-lineage-envelope error: {err}"
        );
    }

    fn sample_kagemusha_recursive_spend_lineage_witness_for_js_host(
        bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV1,
    ) -> iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 {
        let vk_id = VerifyingKeyId::new("halo2/ipa", "js-host-recursive-lineage-hop");
        let verifier_key =
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".to_owned(), vec![0xC9; 32]);
        let vk_commitment = iroha_core::zk::hash_vk(&verifier_key);
        let proof_schema = b"js-host-recursive-lineage-hop-public-inputs-v1".to_vec();
        let proof_envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
                .to_owned(),
            vk_hash: vk_commitment,
            public_inputs: proof_schema.clone(),
            proof_bytes: vec![0xA1; 16],
            aux: Vec::new(),
        };
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            iroha_data_model::proof::ProofBox::new(
                "halo2/ipa".to_owned(),
                norito::to_bytes(&proof_envelope).expect("encode JS host lineage hop proof"),
            ),
            vk_id.clone(),
        );
        attachment.vk_commitment = Some(vk_commitment);
        let step = iroha_data_model::offline::KagemushaVerifiedFoldStep {
            root_before: bundle.accumulator.initial_root,
            input_nullifiers: vec![bundle.accumulator.topup_anchor_nullifiers[0]],
            output_commitments: vec![bundle.accumulator.current_note.note_commitment],
            root_after: bundle.accumulator.final_root,
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pallas",
            Hash::new(proof_schema.as_slice()).into(),
            vk_commitment,
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 {
            record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                    chain_id: bundle.accumulator.chain_id.clone(),
                    asset: bundle.accumulator.asset.clone(),
                    steps: vec![step],
                },
                verifier_records: vec![
                    iroha_data_model::offline::KagemushaVerifiedFoldVerifierRecord {
                        id: vk_id,
                        record,
                    },
                ],
            },
            pallas_open_envelopes_archive: vec![0xFF, 0x00, 0x01],
            current_notes: vec![bundle.accumulator.current_note.clone()],
            previous_recursive_proofs: Vec::new(),
        }
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_instruction_rejects_malformed_lineage_witnesses() {
        fn assert_rejects(
            request: iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1,
            label: &str,
        ) {
            let err = match kagemusha_recursive_spend_redeem_instruction_from_request(request) {
                Ok(_) => panic!("JS host recursive redeem builder must reject {label}"),
                Err(err) => err,
            };
            assert!(
                !err.to_string().is_empty(),
                "JS host recursive redeem builder must report a reason for {label}"
            );
        }

        let base_request = {
            let mut request = sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
            request.lineage_witness =
                Some(sample_kagemusha_recursive_spend_lineage_witness_for_js_host(&request.bundle));
            request
        };

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
        extra.id = VerifyingKeyId::new("halo2/ipa", "unused-js-host-lineage-hop");
        unreferenced_record
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .verifier_records
            .push(extra);
        assert_rejects(unreferenced_record, "unreferenced verifier record");

        let mut note_commitment_mismatch = base_request.clone();
        note_commitment_mismatch
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[0]
            .note_commitment = sample_hash(0xDB);
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
        let sibling_output = sample_hash(0xDC);
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
        attach_strict_reserved_lineage_envelope_for_js_host(
            &mut reserved_lineage_with_record_witness,
        );
        reserved_lineage_with_record_witness.lineage_verifier_record =
            Some(sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host());
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

        assert_rejects(base_request, "malformed Pallas envelope archive");
    }

    fn sample_hash(byte: u8) -> [u8; Hash::LENGTH] {
        let mut buf = [byte; Hash::LENGTH];
        buf[buf.len() - 1] |= 1;
        buf
    }

    fn sample_pallas_coeffs_for_js_host(n: usize) -> Vec<iroha_zkp_halo2::pallas::Scalar> {
        (0..n)
            .map(|index| iroha_zkp_halo2::pallas::Scalar::from((index + 1) as u64))
            .collect()
    }

    fn sample_pallas_open_envelope_with_metadata_for_js_host(
        n: usize,
        label: &str,
        metadata: iroha_zkp_halo2::PolyOpenTranscriptMetadata,
    ) -> iroha_zkp_halo2::OpenVerifyEnvelope {
        let params = iroha_zkp_halo2::pallas::Params::new(n).expect("Pallas params");
        let poly =
            iroha_zkp_halo2::pallas::Polynomial::from_coeffs(sample_pallas_coeffs_for_js_host(n));
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

    fn sample_real_current_hop_record_bundle_for_js_host() -> (
        iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
        Vec<u8>,
    ) {
        static FIXTURE: OnceLock<(
            iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
            Vec<u8>,
        )> = OnceLock::new();

        FIXTURE
            .get_or_init(|| {
                let chain_id: iroha_data_model::ChainId =
                    "kagemusha-js-host-real-transition".parse().expect("chain id");
                let asset = AssetDefinitionId::new(
                    DomainId::try_new("offline", "universal").expect("domain id"),
                    "kgm-js-real-transition"
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
                        b"kagemusha-js-host-real-input",
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
                            b"kagemusha-js-host-real-output",
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
                    iroha_core::zk::ZK_BACKEND_HALO2_IPA.into(),
                    proof.proof,
                    VerifyingKeyId::new(
                        iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                        "kagemusha-js-host-confidential-transfer-v2",
                    ),
                );
                attachment.vk_commitment = Some(iroha_core::zk::hash_vk(&verifier_key));
                let step = iroha_data_model::offline::KagemushaVerifiedFoldStep {
                    root_before,
                    input_nullifiers: proof.nullifiers,
                    output_commitments: proof.output_commitments,
                    root_after,
                    attachment,
                    verifier_key,
                };
                let id = step.attachment.vk_ref.clone();
                let record_bundle =
                    iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                        bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                            chain_id,
                            asset,
                            steps: vec![step],
                        },
                        verifier_records: vec![
                            iroha_data_model::offline::KagemushaVerifiedFoldVerifierRecord {
                                id,
                                record,
                            },
                        ],
                    };
                let metadata =
                    iroha_core::zk::kagemusha_pallas_open_envelope_metadata_for_verified_hop(
                        &record_bundle.bundle.chain_id,
                        &record_bundle.bundle.asset,
                        0,
                        &record_bundle.bundle.steps[0],
                    )
                    .expect("Pallas open-envelope hop metadata");
                let envelope = sample_pallas_open_envelope_with_metadata_for_js_host(
                    4,
                    "js-host-transition-profile-current-hop-open",
                    metadata,
                );
                let envelope_archive =
                    norito::to_bytes(&vec![envelope]).expect("encode Pallas envelope archive");
                (record_bundle, envelope_archive)
            })
            .clone()
    }

    fn sample_two_hop_real_record_bundle_for_js_host() -> (
        iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
        Vec<u8>,
    ) {
        static FIXTURE: OnceLock<(
            iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
            Vec<u8>,
        )> = OnceLock::new();

        FIXTURE
            .get_or_init(|| {
                let (mut record_bundle, _) = sample_real_current_hop_record_bundle_for_js_host();
                let chain_id = record_bundle.bundle.chain_id.clone();
                let asset = record_bundle.bundle.asset.clone();
                let record = record_bundle
                    .verifier_records
                    .first()
                    .expect("one-hop JS host fixture has verifier record")
                    .record
                    .clone();
                let verifier_key = record_bundle.bundle.steps[0].verifier_key.clone();
                let input_diversifier =
                    iroha_core::zk::confidential_v2::derive_confidential_diversifier_v2(
                        b"kagemusha-js-host-real-input",
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
                        b"kagemusha-js-host-real-output",
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
                            b"kagemusha-js-host-second-output",
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
                    iroha_core::zk::ZK_BACKEND_HALO2_IPA.into(),
                    second_proof.proof,
                    VerifyingKeyId::new(
                        iroha_core::zk::ZK_BACKEND_HALO2_IPA,
                        "kagemusha-js-host-confidential-transfer-v2",
                    ),
                );
                attachment.vk_commitment = Some(iroha_core::zk::hash_vk(&verifier_key));
                record_bundle
                    .bundle
                    .steps
                    .push(iroha_data_model::offline::KagemushaVerifiedFoldStep {
                        root_before: second_root_before,
                        input_nullifiers: second_proof.nullifiers,
                        output_commitments: second_proof.output_commitments,
                        root_after: second_root_after,
                        attachment,
                        verifier_key,
                    });
                let pallas_open_envelopes_archive =
                    pallas_open_envelopes_archive_for_record_bundle_js_host(
                        &record_bundle,
                        "js-host-recursive-compact-multi-hop-open",
                    );
                (record_bundle, pallas_open_envelopes_archive)
            })
            .clone()
    }

    fn pallas_open_envelopes_archive_for_record_bundle_js_host(
        record_bundle: &iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
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
                sample_pallas_open_envelope_with_metadata_for_js_host(4, &envelope_label, metadata)
            })
            .collect::<Vec<_>>();
        norito::to_bytes(&envelopes).expect("encode Pallas envelope archive")
    }

    fn sample_kagemusha_recursive_spend_transition_profile_init_request_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 {
        let (record_bundle, pallas_open_envelopes_archive) =
            sample_real_current_hop_record_bundle_for_js_host();
        let step = record_bundle
            .bundle
            .steps
            .first()
            .expect("real current-hop record bundle has one hop");
        let output_commitment = step.output_commitments[0];
        iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 {
            record_bundle,
            pallas_open_envelopes_archive,
            current_note: iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: sample_hash(0xC8),
                amount: Numeric::new(7, 0),
            },
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        }
    }

    fn sample_kagemusha_recursive_spend_transition_profile_append_request_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
        let (record_bundle, pallas_open_envelopes_archive) =
            sample_real_current_hop_record_bundle_for_js_host();
        let pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&pallas_open_envelopes_archive)
                .expect("decode JS host real current-hop Pallas archive");
        let evidence =
            iroha_core::zk::kagemusha_verified_recursive_aggregation_evidence_from_record_bundle_and_pallas_open_envelopes(
                &record_bundle,
                &pallas_open_envelopes,
            )
            .expect("derive JS host real current-hop recursive aggregation evidence");
        let step = record_bundle
            .bundle
            .steps
            .first()
            .expect("real current-hop record bundle has one hop");
        let root_before = step.root_before;
        let previous_note_nullifier = step.input_nullifiers[0];
        let output_commitment = step.output_commitments[0];

        let mut previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
        previous_bundle.accumulator.chain_id = record_bundle.bundle.chain_id.clone();
        previous_bundle.accumulator.asset = record_bundle.bundle.asset.clone();
        previous_bundle.accumulator.final_root = root_before;
        previous_bundle.accumulator.current_note =
            iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: sample_hash(0xC9),
                spend_nullifier: previous_note_nullifier,
                amount: Numeric::new(7, 0),
            };
        previous_bundle.accumulator.verifier_opening_len = evidence.verifier_opening_len;
        previous_bundle.accumulator.verifier_params_fingerprint =
            evidence.verifier_params_fingerprint;
        previous_bundle
            .accumulator
            .fixed_window_table_schedule_digest = evidence.fixed_window_table_schedule_digest;
        previous_bundle
            .accumulator
            .fixed_window_shared_table_manifest_digest =
            evidence.fixed_window_shared_table_manifest_digest;
        refresh_reserved_lineage_previous_bundle_public_inputs_for_js_host(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope_for_js_host(
            &mut previous_bundle,
            sample_hash(0xCA),
        );

        let previous_proof_open_archive =
            sample_previous_recursive_proof_open_envelopes_archive_for_js_host(&previous_bundle);
        let request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            previous_lineage_verifier_record: Some(
                sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
            ),
            record_bundle,
            pallas_open_envelopes_archive,
            current_note: iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: sample_hash(0xCB),
                amount: Numeric::new(7, 0),
            },
            output_proof_circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
            previous_recursive_proof_open_envelopes_archive: previous_proof_open_archive,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        request
            .validate_public_binding()
            .expect("JS host append transition-profile request is well formed");
        request
    }

    fn window_first_recursive_spend_hop_record_for_js_host(
        record_bundle: &mut iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle,
    ) {
        let record = &mut record_bundle
            .verifier_records
            .first_mut()
            .expect("sample record bundle has a verifier record")
            .record;
        record.activation_height = Some(2);
        record.withdraw_height = Some(4);
    }

    fn sample_kagemusha_recursive_spend_init_request_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 {
        let chain_id: iroha_data_model::ChainId =
            "kagemusha-js-host-current-hop".parse().expect("chain id");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "kgm-js-current-hop".parse().expect("asset definition name"),
        );
        let vk_id = VerifyingKeyId::new("halo2/ipa", "js-host-current-hop");
        let verifier_key =
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".to_owned(), vec![0xC4; 48]);
        let vk_commitment = iroha_core::zk::hash_vk(&verifier_key);
        let proof_schema = b"js-host-current-hop-public-inputs-v1".to_vec();
        let proof_envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: "js-host-current-hop-circuit".to_owned(),
            vk_hash: vk_commitment,
            public_inputs: proof_schema.clone(),
            proof_bytes: vec![0xA1; 16],
            aux: Vec::new(),
        };
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            iroha_data_model::proof::ProofBox::new(
                "halo2/ipa".to_owned(),
                norito::to_bytes(&proof_envelope).expect("encode JS host hop proof envelope"),
            ),
            vk_id.clone(),
        );
        attachment.vk_commitment = Some(vk_commitment);
        let output_commitment = sample_hash(0xE4);
        let step = iroha_data_model::offline::KagemushaVerifiedFoldStep {
            root_before: sample_hash(0xE1),
            input_nullifiers: vec![sample_hash(0xE2)],
            output_commitments: vec![output_commitment],
            root_after: sample_hash(0xE3),
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            "js-host-current-hop-circuit",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pallas",
            Hash::new(proof_schema.as_slice()).into(),
            vk_commitment,
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        record.max_proof_bytes = 4096;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.key = Some(verifier_key);
        let record_bundle = iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
            bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                chain_id,
                asset,
                steps: vec![step],
            },
            verifier_records: vec![
                iroha_data_model::offline::KagemushaVerifiedFoldVerifierRecord {
                    id: vk_id,
                    record,
                },
            ],
        };
        let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> = norito::decode_from_bytes(
            &sample_kagemusha_recursive_spend_pallas_archive_for_js_host(1),
        )
        .expect("decode JS host Pallas archive");
        let envelope = envelopes
            .first_mut()
            .expect("Pallas archive contains one envelope");
        envelope.vk_commitment = Some(vk_commitment);
        envelope.public_inputs_schema_hash = Some(Hash::new(proof_schema.as_slice()).into());
        iroha_data_model::offline::KagemushaRecursiveSpendInitRequestV1 {
            record_bundle,
            pallas_open_envelopes_archive: norito::to_bytes(&envelopes)
                .expect("encode JS host current-hop Pallas archive"),
            current_note: iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: sample_hash(0xE5),
                amount: Numeric::new(42, 0),
            },
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        }
    }

    fn recursive_spend_lineage_scalar_projection(byte: u8) -> [u8; Hash::LENGTH] {
        let mut bytes = [byte; Hash::LENGTH];
        bytes[Hash::LENGTH - 1] &= 0x1f;
        bytes
    }

    fn attach_recursive_spend_previous_proof_open_verify_envelope_for_js_host(
        bundle: &mut iroha_data_model::offline::KagemushaRecursiveSpendBundleV1,
        vk_hash: [u8; Hash::LENGTH],
    ) {
        let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id: bundle.recursive_proof.verifier_key_id.name.clone(),
            vk_hash,
            public_inputs:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
                    .to_vec(),
            proof_bytes: vec![0xA5; 64],
            aux: Vec::new(),
        };
        bundle.recursive_proof.proof = ProofBox::new(
            "halo2/ipa".to_owned(),
            norito::to_bytes(&envelope).expect("encode previous recursive proof envelope"),
        );
    }

    fn sample_account(_domain: &str) -> AccountId {
        let keypair = KeyPair::random();
        AccountId::new(keypair.public_key().clone())
    }

    fn sample_kagemusha_recursive_spend_bundle_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 {
        let chain_id: iroha_data_model::ChainId =
            "kagemusha-js-host-recursive".parse().expect("chain id");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "kgm-js-host".parse().expect("asset definition name"),
        );
        let current_note = iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
            note_commitment: sample_hash(0x61),
            spend_nullifier: sample_hash(0x71),
            amount: Numeric::new(42, 0),
        };
        let lineage_digest = sample_hash(0x91);
        let accumulator = iroha_data_model::offline::KagemushaRecursiveSpendAccumulatorV1 {
            domain: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN
                .to_owned(),
            chain_id,
            asset,
            initial_root: sample_hash(0x11),
            final_root: sample_hash(0x12),
            topup_anchor_nullifiers: vec![sample_hash(0x21), sample_hash(0x31)],
            hop_count: 1,
            lineage_digest,
            aggregation_transcript_digest: lineage_digest,
            nullifier_digest: Hash::new(b"js-host-recursive-nullifier-digest"),
            output_commitment_digest: Hash::new(b"js-host-recursive-output-digest"),
            fold_digest: Hash::new(b"js-host-recursive-fold-digest"),
            recursive_proof_chain_digest: sample_hash(0x96),
            transition_profile_binding_digest: sample_hash(0x97),
            append_opening_preflight_digest: [0u8; 32],
            append_boundary_digest: [0u8; 32],
            verifier_params_fingerprint: sample_hash(0xA1),
            fixed_window_table_schedule_digest: sample_hash(0xA2),
            fixed_window_shared_table_manifest_digest: sample_hash(0xA3),
            fixed_window_table_base_digest: sample_hash(0xA4),
            verifier_witness_batch_digest: sample_hash(0xA5),
            verifier_opening_len: 4,
            current_note,
        };
        let public_inputs =
            iroha_data_model::offline::kagemusha_recursive_spend_public_inputs_from_accumulator(
                &accumulator,
            )
            .expect("recursive spend public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive spend public-input hash");
        iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 {
            accumulator,
            recursive_proof: iroha_data_model::offline::KagemushaRecursiveAggregationProof {
                verifier_key_id: VerifyingKeyId::new(
                    "halo2/ipa",
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                ),
                public_inputs,
                public_inputs_hash,
                proof: iroha_data_model::proof::ProofBox::new(
                    "halo2/ipa".to_owned(),
                    vec![0xA5; 64],
                ),
            },
        }
    }

    fn sample_reserved_lineage_previous_bundle_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendBundleV1 {
        let mut previous_bundle = sample_kagemusha_recursive_spend_bundle_for_js_host();
        previous_bundle.recursive_proof.verifier_key_id.name =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                .to_owned();
        refresh_reserved_lineage_previous_bundle_public_inputs_for_js_host(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope_for_js_host(
            &mut previous_bundle,
            sample_hash(0x6D),
        );
        previous_bundle
    }

    fn sample_reserved_lineage_append_request_missing_key_artifacts_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
        let init_request = sample_kagemusha_recursive_spend_init_request_for_js_host();
        let mut previous_bundle = sample_reserved_lineage_previous_bundle_for_js_host();
        let step = init_request
            .record_bundle
            .bundle
            .steps
            .first()
            .expect("JS host append record bundle has one hop");
        let root_before = step.root_before;
        let previous_note_nullifier = step.input_nullifiers[0];
        let output_commitment = step.output_commitments[0];
        previous_bundle.accumulator.chain_id = init_request.record_bundle.bundle.chain_id.clone();
        previous_bundle.accumulator.asset = init_request.record_bundle.bundle.asset.clone();
        if previous_bundle.accumulator.initial_root == root_before {
            previous_bundle.accumulator.initial_root = sample_hash(0xBC);
        }
        previous_bundle.accumulator.topup_anchor_nullifiers = vec![sample_hash(0xC0)];
        previous_bundle.accumulator.final_root = root_before;
        previous_bundle.accumulator.current_note =
            iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: sample_hash(0xBD),
                spend_nullifier: previous_note_nullifier,
                amount: Numeric::new(42, 0),
            };
        refresh_reserved_lineage_previous_bundle_public_inputs_for_js_host(&mut previous_bundle);
        attach_recursive_spend_previous_proof_open_verify_envelope_for_js_host(
            &mut previous_bundle,
            sample_hash(0xBE),
        );
        let previous_recursive_proof_open_envelopes_archive =
            sample_previous_recursive_proof_open_envelopes_archive_for_js_host(&previous_bundle);

        let request = iroha_data_model::offline::KagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle,
            previous_lineage_verifier_record: Some(
                sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host(),
            ),
            record_bundle: init_request.record_bundle,
            pallas_open_envelopes_archive: init_request.pallas_open_envelopes_archive,
            current_note: iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
                note_commitment: output_commitment,
                spend_nullifier: sample_hash(0xBF),
                amount: Numeric::new(42, 0),
            },
            output_proof_circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
            previous_recursive_proof_open_envelopes_archive,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        request
            .validate_public_binding()
            .expect("JS host missing-key-artifact append request is otherwise well formed");
        request
    }

    fn refresh_reserved_lineage_previous_bundle_public_inputs_for_js_host(
        previous_bundle: &mut iroha_data_model::offline::KagemushaRecursiveSpendBundleV1,
    ) {
        previous_bundle.recursive_proof.public_inputs =
            iroha_data_model::offline::kagemusha_recursive_spend_public_inputs_from_accumulator(
                &previous_bundle.accumulator,
            )
            .expect("JS host reserved lineage previous public inputs");
        previous_bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            recursive_spend_lineage_scalar_projection(0x6C);
        previous_bundle.recursive_proof.public_inputs_hash = previous_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("JS host reserved lineage previous public-input hash");
    }

    fn sample_kagemusha_recursive_spend_redeem_request_for_js_host(
        public_amount: u128,
    ) -> iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1 {
        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".to_owned(), vec![0x5A; 64]),
            VerifyingKeyId::new("halo2/ipa", "js-host-recursive-unshield"),
        );
        redeem_proof.vk_commitment = Some(sample_hash(0xB7));
        iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: sample_kagemusha_recursive_spend_bundle_for_js_host(),
            recipient: sample_account("kagemusha-js-host"),
            public_amount,
            redeem_proof,
            lineage_witness: None,
            lineage_verifier_record: None,
            block_height: None,
        }
    }

    fn sample_kagemusha_recursive_spend_lineage_verifier_record_for_js_host()
    -> iroha_data_model::proof::VerifyingKeyRecord {
        let verifier_key =
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".to_owned(), vec![0xC7; 48]);
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pallas",
            iroha_data_model::offline::kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
            iroha_core::zk::hash_vk(&verifier_key),
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        record.max_proof_bytes = 4096;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.key = Some(verifier_key);
        record
    }

    fn sample_previous_recursive_proof_open_envelopes_archive_for_js_host(
        previous_bundle: &iroha_data_model::offline::KagemushaRecursiveSpendBundleV1,
    ) -> Vec<u8> {
        let expected =
            iroha_data_model::offline::kagemusha_recursive_previous_proof_open_envelope_metadata(
                previous_bundle,
            )
            .expect("JS host previous proof opening metadata");
        let envelope = sample_pallas_open_envelope_with_metadata_for_js_host(
            4,
            "js-host-recursive-spend-previous-proof-open-envelope",
            expected,
        );
        norito::to_bytes(&vec![envelope])
            .expect("encode JS host previous proof open-envelope archive")
    }

    fn sample_kagemusha_recursive_spend_pallas_archive_for_js_host(hop_count: usize) -> Vec<u8> {
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
                    transcript_label: format!("js-host-mixed-lineage-open-envelope-{hop_index}"),
                    vk_commitment: Some([label.wrapping_add(12); Hash::LENGTH]),
                    public_inputs_schema_hash: Some([label.wrapping_add(13); Hash::LENGTH]),
                    domain_tag: Some([label.wrapping_add(14); Hash::LENGTH]),
                }
            })
            .collect::<Vec<_>>();
        norito::to_bytes(&envelopes).expect("encode JS host Pallas envelope archive")
    }

    fn sample_kagemusha_recursive_spend_mixed_reserved_previous_redeem_request_for_js_host()
    -> iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1 {
        let mut request = sample_kagemusha_recursive_spend_redeem_request_for_js_host(42);
        request.bundle.accumulator.hop_count = 2;
        request.bundle.recursive_proof.public_inputs =
            iroha_data_model::offline::kagemusha_recursive_spend_public_inputs_from_accumulator(
                &request.bundle.accumulator,
            )
            .expect("two-hop JS host recursive spend public inputs");
        request.bundle.recursive_proof.public_inputs_hash = request
            .bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("two-hop JS host recursive spend public-input hash");

        let vk_id = VerifyingKeyId::new("halo2/ipa", "js-host-mixed-lineage-hop");
        let verifier_key =
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".to_owned(), vec![0xE7; 32]);
        let vk_commitment = iroha_core::zk::hash_vk(&verifier_key);
        let proof_schema = b"js-host-mixed-lineage-hop-public-inputs-v1".to_vec();
        let proof_schema_hash: [u8; Hash::LENGTH] = Hash::new(proof_schema.as_slice()).into();
        let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pallas",
            proof_schema_hash,
            vk_commitment,
        );
        record.namespace = iroha_core::zk::KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        record.max_proof_bytes = 4096;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.key = Some(verifier_key.clone());

        let intermediate_root = sample_hash(0xEA);
        let intermediate_note = iroha_data_model::offline::KagemushaSpendableNoteDescriptorV1 {
            note_commitment: sample_hash(0xEB),
            spend_nullifier: sample_hash(0xEC),
            amount: request.bundle.accumulator.current_note.amount.clone(),
        };
        let step0 = iroha_data_model::offline::KagemushaVerifiedFoldStep {
            root_before: request.bundle.accumulator.initial_root,
            input_nullifiers: request.bundle.accumulator.topup_anchor_nullifiers.clone(),
            output_commitments: vec![intermediate_note.note_commitment],
            root_after: intermediate_root,
            attachment: {
                let proof_envelope = iroha_data_model::zk::OpenVerifyEnvelope {
                    backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
                    circuit_id:
                        iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
                            .to_owned(),
                    vk_hash: vk_commitment,
                    public_inputs: proof_schema.clone(),
                    proof_bytes: vec![0xA1; 16],
                    aux: Vec::new(),
                };
                let mut attachment = ProofAttachment::new_ref(
                    "halo2/ipa".to_owned(),
                    iroha_data_model::proof::ProofBox::new(
                        "halo2/ipa".to_owned(),
                        norito::to_bytes(&proof_envelope)
                            .expect("encode first JS host mixed lineage hop proof"),
                    ),
                    vk_id.clone(),
                );
                attachment.vk_commitment = Some(vk_commitment);
                attachment
            },
            verifier_key: verifier_key.clone(),
        };
        let step1 = iroha_data_model::offline::KagemushaVerifiedFoldStep {
            root_before: intermediate_root,
            input_nullifiers: vec![intermediate_note.spend_nullifier],
            output_commitments: vec![request.bundle.accumulator.current_note.note_commitment],
            root_after: request.bundle.accumulator.final_root,
            attachment: {
                let proof_envelope = iroha_data_model::zk::OpenVerifyEnvelope {
                    backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
                    circuit_id:
                        iroha_core::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
                            .to_owned(),
                    vk_hash: vk_commitment,
                    public_inputs: proof_schema,
                    proof_bytes: vec![0xA2; 16],
                    aux: Vec::new(),
                };
                let mut attachment = ProofAttachment::new_ref(
                    "halo2/ipa".to_owned(),
                    iroha_data_model::proof::ProofBox::new(
                        "halo2/ipa".to_owned(),
                        norito::to_bytes(&proof_envelope)
                            .expect("encode second JS host mixed lineage hop proof"),
                    ),
                    vk_id.clone(),
                );
                attachment.vk_commitment = Some(vk_commitment);
                attachment
            },
            verifier_key,
        };
        let previous_accumulator =
            iroha_data_model::offline::KagemushaRecursiveSpendAccumulatorV1 {
                domain: request.bundle.accumulator.domain.clone(),
                chain_id: request.bundle.accumulator.chain_id.clone(),
                asset: request.bundle.accumulator.asset.clone(),
                initial_root: request.bundle.accumulator.initial_root,
                final_root: intermediate_root,
                topup_anchor_nullifiers: request.bundle.accumulator.topup_anchor_nullifiers.clone(),
                hop_count: 1,
                lineage_digest: sample_hash(0xED),
                aggregation_transcript_digest: sample_hash(0xED),
                nullifier_digest: Hash::new(b"js-host-mixed-lineage-nullifier-digest"),
                output_commitment_digest: Hash::new(b"js-host-mixed-lineage-output-digest"),
                fold_digest: Hash::new(b"js-host-mixed-lineage-fold-digest"),
                recursive_proof_chain_digest: sample_hash(0xEE),
                transition_profile_binding_digest: sample_hash(0xF1),
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
                fixed_window_table_base_digest: sample_hash(0xEF),
                verifier_witness_batch_digest: sample_hash(0xF0),
                verifier_opening_len: request.bundle.accumulator.verifier_opening_len,
                current_note: intermediate_note.clone(),
            };
        let mut previous_public_inputs =
            iroha_data_model::offline::kagemusha_recursive_spend_public_inputs_from_accumulator(
                &previous_accumulator,
            )
            .expect("JS host reserved previous public inputs");
        previous_public_inputs.recursive_verifier_scalar_projection_digest =
            recursive_spend_lineage_scalar_projection(0xF1);
        let previous_public_inputs_hash = previous_public_inputs
            .public_inputs_hash()
            .expect("JS host reserved previous public-input hash");
        let previous_recursive_proof =
            iroha_data_model::offline::KagemushaRecursiveAggregationProof {
                verifier_key_id: VerifyingKeyId::new(
                    "halo2/ipa",
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                ),
                public_inputs: previous_public_inputs,
                public_inputs_hash: previous_public_inputs_hash,
                proof: iroha_data_model::proof::ProofBox::new(
                    "halo2/ipa".to_owned(),
                    vec![0xA3; 64],
                ),
            };
        let mut pallas_open_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(
                &sample_kagemusha_recursive_spend_pallas_archive_for_js_host(2),
            )
            .expect("decode JS host mixed lineage Pallas archive");
        for envelope in &mut pallas_open_envelopes {
            envelope.vk_commitment = Some(vk_commitment);
            envelope.public_inputs_schema_hash = Some(proof_schema_hash);
        }
        request.lineage_witness = Some(
            iroha_data_model::offline::KagemushaRecursiveSpendLineageWitnessV1 {
                record_bundle: iroha_data_model::offline::KagemushaVerifiedFoldRecordBundle {
                    bundle: iroha_data_model::offline::KagemushaVerifiedFoldBundle {
                        chain_id: request.bundle.accumulator.chain_id.clone(),
                        asset: request.bundle.accumulator.asset.clone(),
                        steps: vec![step0, step1],
                    },
                    verifier_records: vec![
                        iroha_data_model::offline::KagemushaVerifiedFoldVerifierRecord {
                            id: vk_id,
                            record,
                        },
                    ],
                },
                pallas_open_envelopes_archive: norito::to_bytes(&pallas_open_envelopes)
                    .expect("encode JS host mixed lineage Pallas archive"),
                current_notes: vec![
                    intermediate_note,
                    request.bundle.accumulator.current_note.clone(),
                ],
                previous_recursive_proofs: vec![previous_recursive_proof],
            },
        );
        request
    }

    fn append_zk1_raw_instance_columns_for_js_host(
        buf: &mut Vec<u8>,
        columns: Vec<Vec<[u8; Hash::LENGTH]>>,
    ) {
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
        zk1_append_tlv(buf, *b"I10P", &payload);
    }

    fn attach_reserved_lineage_envelope_for_js_host(
        request: &mut iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1,
        include_lineage_slice: bool,
    ) {
        request.bundle.recursive_proof.verifier_key_id.name =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                .to_owned();
        request
            .bundle
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            recursive_spend_lineage_scalar_projection(0x1C);
        request.bundle.recursive_proof.public_inputs_hash = request
            .bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("lineage recursive spend public-input hash");

        let mut proof_bytes = ZK1_ENVELOPE_PREFIX.to_vec();
        zk1_append_proof(&mut proof_bytes, &[0xB1; 64]);
        let mut instance_columns =
            iroha_core::zk::kagemusha_recursive_spend_bundle_instance_values(&request.bundle)
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
        append_zk1_raw_instance_columns_for_js_host(&mut proof_bytes, instance_columns);
        let envelope = iroha_data_model::zk::OpenVerifyEnvelope {
            backend: iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            circuit_id:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
                    .to_owned(),
            vk_hash: sample_hash(0xC2),
            public_inputs:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
                    .to_vec(),
            proof_bytes,
            aux: Vec::new(),
        };
        request.bundle.recursive_proof.proof = ProofBox::new(
            "halo2/ipa".to_owned(),
            norito::to_bytes(&envelope).expect("encode recursive spend lineage envelope"),
        );
    }

    fn attach_strict_reserved_lineage_envelope_for_js_host(
        request: &mut iroha_data_model::offline::KagemushaRecursiveSpendRedeemRequestV1,
    ) {
        attach_reserved_lineage_envelope_for_js_host(request, true);
    }

    fn account_json_literal(account: &AccountId) -> String {
        json::to_value(account)
            .expect("serialize account id")
            .as_str()
            .expect("account id json literal should be string")
            .to_owned()
    }

    fn canonical_owner_literal(_domain: &str) -> String {
        account_json_literal(&sample_account("wonderland"))
    }

    fn noncanonical_owner_literal(domain: &str) -> String {
        let account = sample_account(domain);
        format!("{}@{domain}", account_json_literal(&account))
    }

    fn sample_rwa_id(domain: &str, byte: u8) -> RwaId {
        RwaId::generated(
            DomainId::try_new(domain, "universal").expect("valid domain id"),
            Hash::prehashed(sample_hash(byte)),
        )
    }

    fn sample_kaigi_id(domain: &str, call_name: &str) -> KaigiId {
        let domain_id = DomainId::try_new(domain, "universal").expect("valid domain id");
        let call = Name::from_str(call_name).expect("valid kaigi name");
        KaigiId::new(domain_id, call)
    }

    fn sample_agenda_proposal() -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: 1,
            proposal_id: "AC-2026-001".to_owned(),
            submitted_at_unix_ms: 1_770_000_000_000,
            language: "en".to_owned(),
            action: AgendaProposalAction::AddToDenylist,
            summary: AgendaProposalSummary {
                title: "Blacklist proposal for bafy-test".to_owned(),
                motivation: "Evidence review requested for the published CID.".to_owned(),
                expected_impact:
                    "Participating gateways would restrict delivery while the case is reviewed."
                        .to_owned(),
            },
            tags: vec!["spam".to_owned()],
            targets: vec![AgendaProposalTarget {
                label: "bafy-test".to_owned(),
                hash_family: "sorafs-root-cid".to_owned(),
                hash_hex: "11".repeat(32),
                reason: "spam moderation report".to_owned(),
            }],
            evidence: vec![AgendaEvidenceAttachment {
                kind: AgendaEvidenceKind::Url,
                uri: "https://example.invalid/case/1".to_owned(),
                digest_blake3_hex: Some("22".repeat(32)),
                description: Some("Captured gateway evidence".to_owned()),
            }],
            submitter: AgendaProposalSubmitter {
                name: "Explorer Moderator".to_owned(),
                contact: "moderation@example.invalid".to_owned(),
                organization: Some("Sora Ops".to_owned()),
                pgp_fingerprint: None,
            },
            duplicates: vec!["AC-2025-014".to_owned()],
        }
    }

    fn sample_taikai_cache_options() -> JsTaikaiCacheConfig {
        JsTaikaiCacheConfig {
            hot_capacity_bytes: JsU64(8_388_608),
            hot_retention_secs: JsU64(45),
            warm_capacity_bytes: JsU64(33_554_432),
            warm_retention_secs: JsU64(180),
            cold_capacity_bytes: JsU64(268_435_456),
            cold_retention_secs: JsU64(3_600),
            qos: JsTaikaiQosConfig {
                priority_rate_bps: JsU64(83_886_080),
                standard_rate_bps: JsU64(41_943_040),
                bulk_rate_bps: JsU64(12_582_912),
                burst_multiplier: 4,
            },
            reliability: None,
        }
    }

    #[test]
    fn gateway_options_apply_taikai_cache_config() {
        let mut config = OrchestratorConfig::default();
        let opts = JsGatewayFetchOptions {
            taikai_cache: Some(sample_taikai_cache_options()),
            ..Default::default()
        };
        apply_gateway_options(&mut config, &opts).expect("taikai cache applies");
        let cache = config.taikai_cache.expect("cache configured");
        assert_eq!(cache.hot_capacity_bytes, 8_388_608);
        assert_eq!(cache.cold_retention.as_secs(), 3_600);
        assert_eq!(cache.qos.burst_multiplier, 4);
    }

    #[test]
    fn taikai_cache_validation_rejects_invalid_values() {
        let mut config = OrchestratorConfig::default();
        let mut invalid = sample_taikai_cache_options();
        invalid.qos.burst_multiplier = 0;
        let opts = JsGatewayFetchOptions {
            taikai_cache: Some(invalid),
            ..Default::default()
        };
        let err =
            apply_gateway_options(&mut config, &opts).expect_err("burst multiplier validation");
        assert!(
            err.to_string().contains("burstMultiplier"),
            "unexpected error: {err}"
        );
    }

    fn make_stream_token_b64(
        manifest_id_hex: &str,
        provider_id_hex: &str,
        profile: &str,
        max_streams: u16,
    ) -> String {
        let mut provider_id = [0u8; 32];
        provider_id
            .copy_from_slice(&hex::decode(provider_id_hex).expect("decode provider identifier"));
        let token = StreamTokenV1 {
            body: StreamTokenBodyV1 {
                token_id: "01TESTTOKEN0000000000000000".to_string(),
                manifest_cid: hex::decode(manifest_id_hex).expect("decode manifest id"),
                provider_id,
                profile_handle: profile.to_string(),
                max_streams,
                ttl_epoch: 9_999_999_999,
                rate_limit_bytes: 8 * 1024 * 1024,
                issued_at: 1_735_000_000,
                requests_per_minute: 120,
                token_pk_version: 1,
            },
            signature: vec![0; 64],
        };
        let bytes = norito::to_bytes(&token).expect("encode stream token");
        BASE64.encode(bytes)
    }

    fn da_fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/da/reconstruct/rs_parity_v1")
            .join(name)
    }

    fn load_da_manifest_fixture() -> Vec<u8> {
        let hex_str =
            fs::read_to_string(da_fixture_path("manifest.norito.hex")).expect("read manifest hex");
        hex::decode(hex_str.trim()).expect("decode manifest hex")
    }

    fn load_da_payload_fixture() -> Vec<u8> {
        fs::read(da_fixture_path("payload.bin")).expect("read payload fixture")
    }

    #[test]
    fn da_fixtures_are_readable() {
        let manifest = load_da_manifest_fixture();
        assert!(
            !manifest.is_empty(),
            "DA manifest fixture should not be empty"
        );
        let payload = load_da_payload_fixture();
        assert!(
            !payload.is_empty(),
            "DA payload fixture should not be empty"
        );
    }

    fn empty_gateway_options() -> JsGatewayFetchOptions {
        JsGatewayFetchOptions {
            manifest_envelope_b64: None,
            manifest_cid_hex: None,
            cache_version: None,
            moderation_token_key: None,
            client_id: None,
            telemetry_region: None,
            rollout_phase: None,
            max_peers: None,
            retry_budget: None,
            transport_policy: None,
            anonymity_policy: None,
            write_mode: None,
            local_proxy: None,
            taikai_cache: None,
            scoreboard_out_path: None,
            scoreboard_now_unix_secs: None,
            scoreboard_telemetry_label: None,
            scoreboard_allow_implicit_metadata: None,
            allow_single_source_fallback: None,
        }
    }

    #[test]
    fn gateway_rollout_phase_updates_default_anonymity() {
        let mut config = OrchestratorConfig {
            anonymity_policy: AnonymityPolicy::StrictPq,
            ..OrchestratorConfig::default()
        };
        assert!(config.anonymity_policy_override.is_none());

        let mut opts = empty_gateway_options();
        opts.rollout_phase = Some("ramp".to_string());
        apply_gateway_options(&mut config, &opts).expect("apply rollout phase");

        assert_eq!(config.rollout_phase, RolloutPhase::Ramp);
        assert_eq!(
            config.anonymity_policy,
            RolloutPhase::Ramp.default_anonymity_policy()
        );
        assert!(config.anonymity_policy_override.is_none());
    }

    #[test]
    fn gateway_rollout_phase_respects_anonymity_override() {
        let mut config = OrchestratorConfig {
            anonymity_policy: AnonymityPolicy::StrictPq,
            anonymity_policy_override: Some(AnonymityPolicy::StrictPq),
            ..OrchestratorConfig::default()
        };

        let mut opts = empty_gateway_options();
        opts.rollout_phase = Some("canary".to_string());
        apply_gateway_options(&mut config, &opts).expect("apply rollout phase");

        assert_eq!(config.rollout_phase, RolloutPhase::Canary);
        assert_eq!(config.anonymity_policy, AnonymityPolicy::StrictPq);
        assert_eq!(
            config.anonymity_policy_override,
            Some(AnonymityPolicy::StrictPq)
        );
    }

    #[test]
    fn gateway_retry_budget_zero_disables_cap() {
        let mut config = OrchestratorConfig::default();
        let mut opts = empty_gateway_options();
        opts.retry_budget = Some(0);
        apply_gateway_options(&mut config, &opts).expect("apply retry budget");

        assert!(config.fetch.per_chunk_retry_limit.is_none());
    }

    #[test]
    fn gateway_retry_budget_sets_positive_limit() {
        let mut config = OrchestratorConfig::default();
        let mut opts = empty_gateway_options();
        opts.retry_budget = Some(3);
        apply_gateway_options(&mut config, &opts).expect("apply retry budget");
        assert_eq!(config.fetch.per_chunk_retry_limit, Some(3));
    }

    #[test]
    fn gateway_write_mode_parses_upload_hint() {
        let mut config = OrchestratorConfig::default();
        let mut opts = empty_gateway_options();
        opts.write_mode = Some("upload-pq-only".to_string());
        apply_gateway_options(&mut config, &opts).expect("apply write mode");
        assert_eq!(config.write_mode, WriteModeHint::UploadPqOnly);
    }

    #[test]
    fn scoreboard_metadata_records_effective_policy_labels() {
        let config = OrchestratorConfig {
            transport_policy: TransportPolicy::SoranetPreferred,
            anonymity_policy: AnonymityPolicy::GuardPq,
            ..OrchestratorConfig::default()
        };
        let metadata = build_scoreboard_metadata_value(
            2,
            2,
            &config,
            ScoreboardMetadataInputs {
                allow_implicit_metadata: false,
                telemetry_label: Some("sdk:js"),
                telemetry_region: Some("iad-prod"),
                gateway_manifest_provided: true,
                gateway_manifest_id: Some("feedface"),
                gateway_manifest_cid: Some("c0ffee"),
                allow_single_source_fallback: false,
            },
        )
        .expect("metadata");
        let map = metadata
            .as_object()
            .expect("scoreboard metadata should be an object");
        assert_eq!(
            map.get("transport_policy").and_then(Value::as_str),
            Some("soranet-first")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            map.get("transport_policy_override_label")
                .is_some_and(Value::is_null)
        );
        assert_eq!(
            map.get("anonymity_policy").and_then(Value::as_str),
            Some("anon-guard-pq")
        );
        assert_eq!(
            map.get("anonymity_policy_override")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            map.get("anonymity_policy_override_label")
                .is_some_and(Value::is_null)
        );
        assert_eq!(
            map.get("write_mode").and_then(Value::as_str),
            Some("read-only")
        );
        assert_eq!(
            map.get("write_mode_enforces_pq").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            map.get("gateway_manifest_id").and_then(Value::as_str),
            Some("feedface")
        );
        assert_eq!(
            map.get("gateway_manifest_cid").and_then(Value::as_str),
            Some("c0ffee")
        );
        assert_eq!(
            map.get("telemetry_region").and_then(Value::as_str),
            Some("iad-prod")
        );
    }

    #[test]
    fn scoreboard_metadata_records_policy_overrides() {
        let config = OrchestratorConfig {
            policy_override: PolicyOverride::new(
                Some(TransportPolicy::SoranetStrict),
                Some(AnonymityPolicy::StrictPq),
            ),
            ..OrchestratorConfig::default()
        };
        let metadata = build_scoreboard_metadata_value(
            1,
            1,
            &config,
            ScoreboardMetadataInputs {
                allow_implicit_metadata: true,
                telemetry_label: None,
                telemetry_region: None,
                gateway_manifest_provided: false,
                gateway_manifest_id: None,
                gateway_manifest_cid: None,
                allow_single_source_fallback: true,
            },
        )
        .expect("metadata");
        let map = metadata
            .as_object()
            .expect("scoreboard metadata should be an object");
        assert_eq!(
            map.get("transport_policy").and_then(Value::as_str),
            Some("soranet-strict")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("transport_policy_override_label")
                .and_then(Value::as_str),
            Some("soranet-strict")
        );
        assert_eq!(
            map.get("anonymity_policy").and_then(Value::as_str),
            Some("anon-strict-pq")
        );
        assert_eq!(
            map.get("anonymity_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("anonymity_policy_override_label")
                .and_then(Value::as_str),
            Some("anon-strict-pq")
        );
        assert_eq!(
            map.get("write_mode").and_then(Value::as_str),
            Some("read-only")
        );
        assert_eq!(
            map.get("write_mode_enforces_pq").and_then(Value::as_bool),
            Some(false)
        );
        assert!(map.get("gateway_manifest_id").is_some_and(Value::is_null));
        assert!(map.get("gateway_manifest_cid").is_some_and(Value::is_null));
        assert!(map.get("telemetry_region").is_some_and(Value::is_null));
    }

    #[test]
    fn mint_asset_instruction_json_roundtrip() {
        let account_id = sample_account("wonderland");
        let asset_definition: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_definition, account_id.clone());

        let mint_box: MintBox =
            Mint::asset_numeric(Numeric::from_str("10").expect("valid numeric"), asset_id).into();
        let instruction = InstructionBox::from(mint_box);

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize instruction to json");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Mint"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize instruction from json");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn kaigi_commitment_option_roundtrip() {
        disable_packed_struct_once();
        let mut buf = [0x11u8; Hash::LENGTH];
        buf[buf.len() - 1] |= 1;
        let commitment = KaigiParticipantCommitment {
            commitment: Hash::prehashed(buf),
            alias_tag: Some("alice".to_owned()),
        };
        let option = Some(commitment.clone());
        let bytes = option.encode();
        let mut cursor = Cursor::new(bytes.as_slice());
        let decoded: Option<KaigiParticipantCommitment> =
            NoritoDecode::decode(&mut cursor).expect("decode option bytes");
        assert_eq!(decoded, Some(commitment));
    }

    #[test]
    fn burn_asset_instruction_json_roundtrip() {
        let account_id = sample_account("wonderland");
        let asset_definition: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_definition, account_id.clone());

        let burn_box: BurnBox =
            Burn::asset_numeric(Numeric::from_str("5").expect("valid numeric"), asset_id).into();
        let instruction = InstructionBox::from(burn_box);

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize instruction to json");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Burn"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize instruction from json");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn custom_instruction_json_roundtrip() {
        let account = sample_account("wonderland");
        let mut propose = json::Map::new();
        propose.insert(
            "account".to_owned(),
            json::Value::String(account_json_literal(&account)),
        );
        propose.insert(
            "instructions".to_owned(),
            json::Value::Array(vec![norito_json!({
                "Log": "hello-custom"
            })]),
        );
        propose.insert(
            "transaction_ttl_ms".to_owned(),
            json::Value::Number(json::Number::from(30_000_u64)),
        );
        let mut payload = json::Map::new();
        payload.insert("Propose".to_owned(), json::Value::Object(propose));
        let instruction_json = custom_json_value(json::Value::Object(payload));

        let instruction =
            value_to_instruction(instruction_json.clone()).expect("parse Custom instruction");
        assert!(
            instruction
                .as_any()
                .downcast_ref::<CustomInstruction>()
                .is_some(),
            "instruction must decode as CustomInstruction"
        );

        let rendered =
            instruction_to_json_value(&instruction).expect("render custom instruction to JSON");
        assert_eq!(rendered, instruction_json);
    }

    #[test]
    fn multisig_alias_payloads_decode_as_custom_instruction() {
        let account = sample_account("wonderland");
        let account_literal = account_json_literal(&account);

        let mut propose_fields = json::Map::new();
        propose_fields.insert(
            "account".to_owned(),
            json::Value::String(account_literal.clone()),
        );
        propose_fields.insert(
            "instructions".to_owned(),
            json::Value::Array(vec![norito_json!({
                "Log": "multisig-propose"
            })]),
        );
        let mut propose_outer = json::Map::new();
        propose_outer.insert(
            "MultisigPropose".to_owned(),
            json::Value::Object(propose_fields),
        );
        let propose_instruction = value_to_instruction(json::Value::Object(propose_outer))
            .expect("parse MultisigPropose alias");
        let propose_rendered =
            instruction_to_json_value(&propose_instruction).expect("render MultisigPropose alias");
        assert!(
            propose_rendered
                .get("Custom")
                .and_then(|value| value.get("payload"))
                .and_then(|value| value.get("Propose"))
                .is_some(),
            "MultisigPropose alias must map to Custom.payload.Propose"
        );

        let mut cancel_fields = json::Map::new();
        cancel_fields.insert(
            "account".to_owned(),
            json::Value::String(account_literal.clone()),
        );
        cancel_fields.insert(
            "instructions_hash".to_owned(),
            json::Value::String(hash_literal(0xBB)),
        );
        let mut cancel_outer = json::Map::new();
        cancel_outer.insert(
            "MultisigCancel".to_owned(),
            json::Value::Object(cancel_fields),
        );
        let cancel_instruction = value_to_instruction(json::Value::Object(cancel_outer))
            .expect("parse MultisigCancel alias");
        let cancel_rendered =
            instruction_to_json_value(&cancel_instruction).expect("render MultisigCancel alias");
        assert!(
            cancel_rendered
                .get("Custom")
                .and_then(|value| value.get("payload"))
                .and_then(|value| value.get("Cancel"))
                .and_then(|value| value.get("instructions_hash"))
                .is_some(),
            "MultisigCancel alias must map to Custom.payload.Cancel"
        );

        let mut approve_fields = json::Map::new();
        approve_fields.insert("account".to_owned(), json::Value::String(account_literal));
        approve_fields.insert(
            "instructions_hash".to_owned(),
            json::Value::String(hash_literal(0xCC)),
        );
        let mut multisig_payload = json::Map::new();
        multisig_payload.insert("Approve".to_owned(), json::Value::Object(approve_fields));
        let mut approve_outer = json::Map::new();
        approve_outer.insert("Multisig".to_owned(), json::Value::Object(multisig_payload));
        let approve_instruction =
            value_to_instruction(json::Value::Object(approve_outer)).expect("parse Multisig alias");
        let approve_rendered =
            instruction_to_json_value(&approve_instruction).expect("render Multisig alias");
        assert!(
            approve_rendered
                .get("Custom")
                .and_then(|value| value.get("payload"))
                .and_then(|value| value.get("Approve"))
                .is_some(),
            "Multisig alias must map to Custom.payload.Approve"
        );
    }

    #[test]
    fn sorafs_multi_fetch_local_executes_plan() {
        let tempdir = tempdir().expect("tempdir");
        let payload: Vec<u8> = (0..(6 * 1024_usize))
            .map(|idx| u8::try_from(idx % 241).expect("modulo fits in u8"))
            .collect();
        let alpha_path = tempdir.path().join("alpha.bin");
        let beta_path = tempdir.path().join("beta.bin");
        fs::write(&alpha_path, &payload).expect("write alpha payload");
        fs::write(&beta_path, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialize plan");

        let providers = vec![
            JsLocalProviderSpec {
                name: "alpha".to_owned(),
                path: alpha_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
            JsLocalProviderSpec {
                name: "beta".to_owned(),
                path: beta_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
        ];

        let result = sorafs_multi_fetch_local(
            plan_json,
            providers,
            Some(JsMultiFetchOptions {
                max_peers: Some(1),
                ..Default::default()
            }),
        )
        .expect("multi-fetch succeeds");

        assert_eq!(result.chunk_count as usize, plan.chunk_fetch_specs().len());
        assert_eq!(result.provider_reports.len(), 1);
        assert_eq!(result.provider_reports[0].provider, "alpha");
        assert_eq!(result.provider_reports[0].failures, 0);
        assert!(!result.provider_reports[0].disabled);

        assert_eq!(result.chunk_receipts.len(), plan.chunk_fetch_specs().len());
        assert!(
            result
                .chunk_receipts
                .iter()
                .all(|receipt| receipt.provider == "alpha")
        );

        let payload_bytes = result.payload.to_vec();
        assert_eq!(payload_bytes, payload);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn sorafs_gateway_fetch_returns_manifest_and_car_verification() {
        use sorafs_car::multi_fetch::{ChunkReceipt, FetchOutcome, FetchProvider, ProviderReport};

        ensure_packed_struct_disabled();

        let payload_len = 32 * 1024;
        let payload: Vec<u8> = (0..payload_len)
            .map(|idx| u8::try_from(idx % 251).expect("payload byte fits in u8"))
            .collect();

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let descriptor = chunker_registry::lookup_by_profile(
            ChunkProfile::DEFAULT,
            chunker_registry::DEFAULT_MULTIHASH_CODE,
        )
        .expect("lookup chunker profile");
        let chunker_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );

        let writer = CarWriter::new(&plan, &payload).expect("car writer");
        let car_stats = writer.write_to(std::io::sink()).expect("write car bytes");

        let manifest = ManifestBuilder::new()
            .root_cid(car_stats.root_cids[0].clone())
            .dag_codec(sorafs_manifest::DagCodecId(car_stats.dag_codec))
            .chunking_profile(ChunkingProfileV1::from_profile(
                plan.chunk_profile,
                chunker_registry::DEFAULT_MULTIHASH_CODE,
            ))
            .content_length(plan.content_length)
            .car_digest(car_stats.car_archive_digest.into())
            .car_size(car_stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 0,
            })
            .governance(GovernanceProofs {
                council_signatures: vec![CouncilSignature {
                    signer: [0x11; 32],
                    signature: vec![0x22; 64],
                }],
            })
            .build()
            .expect("manifest");
        let manifest_bytes = manifest.encode().expect("encode manifest");
        let manifest_digest = manifest.digest().expect("manifest digest");
        let manifest_id_hex = hex::encode(manifest_digest.as_bytes());
        let manifest_governance = manifest.governance.clone();
        let manifest_digest_copy = manifest_digest;

        let provider_id_hex = "aa".repeat(32);
        let stream_token_b64 = make_stream_token_b64(
            &manifest_id_hex,
            &provider_id_hex,
            &chunker_handle,
            u16::try_from(plan.chunks.len()).expect("chunk count fits in u16"),
        );

        let plan_json =
            chunk_fetch_specs_to_string(&plan.chunk_fetch_specs()).expect("serialize plan");
        let tempdir = tempdir().expect("tempdir");
        let norito_dir = tempdir.path().join("norito");
        let car_dir = tempdir.path().join("car");
        let scoreboard_path = tempdir.path().join("scoreboard").join("scoreboard.json");
        fs::create_dir_all(&norito_dir).expect("create norito directory");
        fs::create_dir_all(&car_dir).expect("create car directory");

        let options = JsGatewayFetchOptions {
            manifest_envelope_b64: None,
            manifest_cid_hex: None,
            cache_version: None,
            moderation_token_key: None,
            client_id: Some("ci-test".into()),
            telemetry_region: Some("ci-region".into()),
            rollout_phase: Some("canary".into()),
            max_peers: Some(2),
            retry_budget: Some(3),
            transport_policy: Some("soranet-first".into()),
            anonymity_policy: None,
            write_mode: None,
            local_proxy: Some(JsLocalProxyConfig {
                bind_addr: Some("127.0.0.1:0".into()),
                telemetry_label: Some("test-proxy".into()),
                guard_cache_key_hex: None,
                emit_browser_manifest: Some(true),
                proxy_mode: Some("bridge".into()),
                prewarm_circuits: Some(true),
                max_streams_per_circuit: Some(4),
                circuit_ttl_hint_secs: Some(180),
                norito_bridge: Some(JsProxyNoritoBridgeConfig {
                    spool_dir: norito_dir.to_string_lossy().into_owned(),
                    extension: Some("norito".into()),
                }),
                car_bridge: Some(JsProxyCarBridgeConfig {
                    cache_dir: car_dir.to_string_lossy().into_owned(),
                    extension: Some("car".into()),
                    allow_zst: Some(false),
                }),
                kaigi_bridge: None,
            }),
            taikai_cache: None,
            scoreboard_out_path: Some(scoreboard_path.to_string_lossy().into_owned()),
            scoreboard_now_unix_secs: Some(JsU64(1_700_000_000)),
            scoreboard_telemetry_label: Some("ci-gateway".into()),
            scoreboard_allow_implicit_metadata: Some(true),
            allow_single_source_fallback: Some(false),
        };

        let manifest_bytes_clone = manifest_bytes.clone();
        let car_stats_clone = car_stats.clone();
        let payload_clone = payload.clone();
        let chunker_handle_clone = chunker_handle.clone();
        let override_guard = set_fetch_via_gateway_override(move |_, plan_override, _, _, _, _| {
            let provider = Arc::new(FetchProvider::new("alpha"));

            let chunk_specs = plan_override.chunk_fetch_specs();
            let mut chunks = Vec::with_capacity(chunk_specs.len());
            let mut receipts = Vec::with_capacity(chunk_specs.len());
            for spec in &chunk_specs {
                let offset = usize::try_from(spec.offset).expect("offset fits in usize");
                let length = usize::try_from(spec.length).expect("length fits in usize");
                let upper = offset
                    .checked_add(length)
                    .expect("chunk slice within payload bounds");
                assert!(
                    upper <= payload_clone.len(),
                    "chunk slice exceeds payload (offset={offset} length={length})"
                );
                let bytes = payload_clone[offset..upper].to_vec();
                chunks.push(bytes);
                receipts.push(ChunkReceipt {
                    chunk_index: spec.chunk_index,
                    provider: provider.id().clone(),
                    attempts: 1,
                    latency_ms: 12.5,
                    bytes: spec.length,
                });
            }

            let outcome = FetchOutcome {
                chunks,
                chunk_receipts: receipts,
                provider_reports: vec![ProviderReport {
                    provider: provider.clone(),
                    successes: chunk_specs.len(),
                    failures: 0,
                    disabled: false,
                }],
            };

            let policy_report = PolicyReport {
                policy: AnonymityPolicy::GuardPq,
                effective_policy: AnonymityPolicy::GuardPq,
                total_candidates: 1,
                pq_candidates: 1,
                selected_soranet_total: 1,
                selected_pq: 1,
                status: PolicyStatus::Met,
                fallback_reason: None,
            };

            let mut manifest_car_digest = [0u8; 32];
            manifest_car_digest.copy_from_slice(car_stats_clone.car_archive_digest.as_bytes());

            let car_verification = GatewayCarVerification {
                manifest_digest: manifest_digest_copy,
                manifest_payload_digest: car_stats_clone.car_payload_digest,
                manifest_content_length: plan_override.content_length,
                manifest_chunk_count: u64::try_from(chunk_specs.len())
                    .expect("chunk count fits in u64"),
                manifest_car_digest,
                manifest_governance: manifest_governance.clone(),
                chunk_profile_handle: chunker_handle_clone.clone(),
                car_stats: car_stats_clone.clone(),
                por_leaf_count: 0,
            };

            let manifest_stub = BrowserExtensionManifest {
                version: 1,
                authority: "127.0.0.1:9000".into(),
                certificate_pem: BASE64.encode(&manifest_bytes_clone),
                cert_fingerprint_hex: Some("DEADBEEF".into()),
                alpn: Some("h3".into()),
                capabilities: vec!["raw-stream".into()],
                proxy_mode: ProxyMode::Bridge,
                session_id: Some("session".into()),
                telemetry_label: Some("test-proxy".into()),
                guard_cache_key_hex: None,
                circuit: None,
                guard_selection: None,
                route_hints: Vec::new(),
                cache_tagging: None,
                telemetry_v2: None,
            };

            Ok(FetchSession {
                outcome,
                policy_report,
                local_proxy_manifest: Some(manifest_stub),
                car_verification: Some(car_verification),
                taikai_cache_stats: None,
                taikai_cache_queue: None,
            })
        });

        let result = sorafs_gateway_fetch(
            manifest_id_hex.clone(),
            chunker_handle.clone(),
            plan_json,
            vec![JsGatewayProviderSpec {
                name: "alpha".to_string(),
                provider_id_hex: provider_id_hex.clone(),
                base_url: "https://stub".into(),
                stream_token_b64,
                privacy_events_url: None,
            }],
            Some(options),
        )
        .expect("gateway fetch result");
        drop(override_guard);

        assert_eq!(result.manifest_id_hex, manifest_id_hex);
        assert_eq!(result.chunker_handle, chunker_handle);
        assert_eq!(
            usize::try_from(result.chunk_count).expect("chunk count fits in usize"),
            plan.chunks.len()
        );
        assert_eq!(result.payload.len(), payload.len());
        assert_eq!(result.payload.as_ref(), payload.as_slice());
        assert_eq!(result.telemetry_region.as_deref(), Some("ci-region"));

        let manifest_json = result
            .local_proxy_manifest_json
            .as_ref()
            .expect("local proxy manifest");
        let manifest_value: Value = json::from_str(manifest_json).expect("parse manifest json");
        let authority = manifest_value
            .get("authority")
            .and_then(Value::as_str)
            .expect("manifest authority");
        assert!(
            authority.starts_with("127.0.0.1:"),
            "expected loopback authority, got {authority}"
        );

        let verification = result
            .car_verification
            .expect("car verification should be present");
        assert_eq!(
            verification.manifest_digest_hex,
            hex::encode_upper(manifest_digest.as_bytes())
        );
        assert_eq!(verification.manifest_content_length.0, plan.content_length);
        assert_eq!(
            verification.manifest_chunk_count.0,
            u64::try_from(plan.chunks.len()).expect("chunk count fits in u64")
        );
        assert_eq!(verification.manifest_governance.council_signatures.len(), 1);
        assert_eq!(
            verification.car_archive.payload_digest_hex,
            hex::encode_upper(car_stats.car_payload_digest.as_bytes())
        );
        assert_eq!(verification.car_archive.size.0, car_stats.car_size);
    }

    fn sample_js_range_capability() -> JsRangeCapability {
        let profile = ChunkProfile::DEFAULT;
        JsRangeCapability {
            max_chunk_span: u32::try_from(profile.max_size).expect("chunk span fits in u32"),
            min_granularity: u32::try_from(profile.min_size).expect("granularity fits in u32"),
            supports_sparse_offsets: Some(true),
            requires_alignment: Some(false),
            supports_merkle_proof: Some(true),
        }
    }

    fn sample_js_stream_budget() -> JsStreamBudget {
        JsStreamBudget {
            max_in_flight: 4,
            max_bytes_per_sec: JsU64(5_000_000),
            burst_bytes: Some(JsU64(5_000_000)),
        }
    }

    fn sample_provider_metadata(name: &str, issued_at: u64) -> JsProviderMetadata {
        JsProviderMetadata {
            provider_id: Some(name.to_string()),
            profile_id: Some("sorafs.sf1@1.0.0".into()),
            profile_aliases: Some(vec![name.to_string()]),
            availability: Some("hot".into()),
            stake_amount: Some("1000000".into()),
            max_streams: Some(4),
            refresh_deadline: Some(JsU64(issued_at + 1_800)),
            expires_at: Some(JsU64(issued_at + 3_600)),
            ttl_secs: Some(JsU64(3_600)),
            allow_unknown_capabilities: Some(false),
            capability_names: Some(vec!["torii_gateway".into(), "chunk_range_fetch".into()]),
            rendezvous_topics: None,
            notes: Some(format!("{name} provider")),
            range_capability: Some(sample_js_range_capability()),
            stream_budget: Some(sample_js_stream_budget()),
            transport_hints: None,
        }
    }

    struct DaManifestFixture {
        manifest_bytes: Vec<u8>,
        payload: Vec<u8>,
        blob_hash: [u8; 32],
        chunk_root: [u8; 32],
        leaf_count: usize,
    }

    #[allow(clippy::too_many_lines)]
    fn build_da_manifest_fixture() -> DaManifestFixture {
        let payload: Vec<u8> = (0..16 * 1024)
            .map(|idx| u8::try_from(idx % 197).expect("payload byte fits in u8"))
            .collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut store = ChunkStore::with_profile(plan.chunk_profile);
        let mut source = InMemoryPayload::new(&payload);
        store
            .ingest_plan_source(&plan, &mut source)
            .expect("ingest payload");
        let chunk_root = *store.por_tree().root();
        let blob_hash = *store.payload_digest().as_bytes();
        let shard_span = usize::from(
            ErasureProfile::default()
                .data_shards
                .saturating_add(ErasureProfile::default().parity_shards),
        );
        let chunk_commitments = plan
            .chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let chunk_index =
                    u32::try_from(index).expect("chunk index must fit within u32 range");
                let stripe_id =
                    u32::try_from(index / shard_span).expect("stripe index fits in u32");
                ChunkCommitment::new_with_role(
                    chunk_index,
                    chunk.offset,
                    chunk.length,
                    BlobDigest::new(chunk.digest),
                    ChunkRole::Data,
                    stripe_id,
                )
            })
            .collect();
        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::from_hash(blake3::hash(b"client")),
            lane_id: LaneId::new(0),
            epoch: 0,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec("application/octet-stream".into()),
            blob_hash: BlobDigest::new(blob_hash),
            chunk_root: BlobDigest::new(chunk_root),
            storage_ticket: StorageTicketId::from_hash(blake3::hash(b"ticket")),
            total_size: payload.len() as u64,
            chunk_size: plan
                .chunks
                .first()
                .map(|chunk| chunk.length)
                .expect("chunks present"),
            total_stripes: u32::try_from(
                plan.chunks
                    .len()
                    .div_ceil(usize::from(ErasureProfile::default().data_shards)),
            )
            .expect("stripe count fits in u32"),
            shards_per_stripe: u32::from(
                ErasureProfile::default()
                    .data_shards
                    .saturating_add(ErasureProfile::default().parity_shards),
            ),
            erasure_profile: ErasureProfile::default(),
            retention_policy: RetentionPolicy::default(),
            rent_quote: DaRentQuote::default(),
            chunks: chunk_commitments,
            ipa_commitment: BlobDigest::new(chunk_root),
            metadata: ExtraMetadata {
                items: vec![
                    MetadataEntry::new(
                        "taikai.event_id",
                        b"demo-event".to_vec(),
                        MetadataVisibility::Public,
                    ),
                    MetadataEntry::new(
                        "taikai.stream_id",
                        b"demo-stream".to_vec(),
                        MetadataVisibility::Public,
                    ),
                    MetadataEntry::new(
                        "taikai.rendition_id",
                        b"demo-rendition".to_vec(),
                        MetadataVisibility::Public,
                    ),
                    MetadataEntry::new(
                        "taikai.segment.sequence",
                        b"1".to_vec(),
                        MetadataVisibility::Public,
                    ),
                ],
            },
            issued_at_unix: 0,
        };
        let manifest_bytes = norito::to_bytes(&manifest).expect("encode manifest");
        DaManifestFixture {
            manifest_bytes,
            payload,
            blob_hash,
            chunk_root,
            leaf_count: store.por_tree().leaf_count(),
        }
    }

    fn unix_time_now() -> Option<u64> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs())
    }

    #[test]
    fn sorafs_multi_fetch_local_scoreboard_prefers_eligible_provider() {
        let tempdir = tempdir().expect("tempdir");
        let payload: Vec<u8> = (0..(4 * 1024_usize))
            .map(|idx| u8::try_from(idx % 211).expect("modulo fits in u8"))
            .collect();
        let alpha_path = tempdir.path().join("alpha.bin");
        let beta_path = tempdir.path().join("beta.bin");
        fs::write(&alpha_path, &payload).expect("write alpha payload");
        fs::write(&beta_path, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialize plan");

        let issued_at = unix_time_now().unwrap_or(1_700_000_000);
        let providers = vec![
            JsLocalProviderSpec {
                name: "alpha".to_owned(),
                path: alpha_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: Some(sample_provider_metadata("alpha", issued_at)),
            },
            JsLocalProviderSpec {
                name: "beta".to_owned(),
                path: beta_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: Some(sample_provider_metadata("beta", issued_at)),
            },
        ];

        let telemetry = vec![
            JsTelemetryEntry {
                provider_id: "alpha".into(),
                qos_score: Some(40.0),
                latency_p95_ms: Some(900.0),
                failure_rate_ewma: Some(0.3),
                token_health: Some(0.6),
                staking_weight: Some(0.8),
                penalty: Some(true),
                last_updated_unix: Some(JsU64(issued_at)),
            },
            JsTelemetryEntry {
                provider_id: "beta".into(),
                qos_score: Some(95.0),
                latency_p95_ms: Some(140.0),
                failure_rate_ewma: Some(0.05),
                token_health: Some(0.98),
                staking_weight: Some(1.2),
                penalty: Some(false),
                last_updated_unix: Some(JsU64(issued_at)),
            },
        ];

        let result = sorafs_multi_fetch_local(
            plan_json,
            providers,
            Some(JsMultiFetchOptions {
                use_scoreboard: Some(true),
                telemetry: Some(telemetry),
                return_scoreboard: Some(true),
                ..Default::default()
            }),
        )
        .expect("multi-fetch succeeds with scoreboard");

        assert_eq!(result.chunk_count as usize, plan.chunk_fetch_specs().len());
        assert_eq!(result.provider_reports.len(), 1);
        let beta_report = &result.provider_reports[0];
        assert_eq!(beta_report.provider, "beta");
        assert!(!beta_report.disabled);
        assert!(
            result
                .chunk_receipts
                .iter()
                .all(|receipt| receipt.provider == "beta")
        );

        let scoreboard = result
            .scoreboard
            .expect("scoreboard entries should be returned");
        assert!(
            scoreboard
                .iter()
                .any(|entry| entry.alias == "beta" && entry.eligibility == "eligible")
        );
        assert!(scoreboard.iter().any(|entry| entry.alias == "alpha"
            && entry.eligibility.to_ascii_lowercase().contains("penalty")));
    }

    #[test]
    fn sorafs_multi_fetch_local_policy_denies_provider() {
        let tempdir = tempdir().expect("tempdir");
        let payload: Vec<u8> = (0..(4 * 1024_usize))
            .map(|idx| u8::try_from(idx % 197).expect("modulo fits in u8"))
            .collect();
        let alpha_path = tempdir.path().join("alpha.bin");
        let beta_path = tempdir.path().join("beta.bin");
        fs::write(&alpha_path, &payload).expect("write alpha payload");
        fs::write(&beta_path, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json =
            sorafs_car::fetch_plan::chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
                .expect("serialize plan");

        let providers = vec![
            JsLocalProviderSpec {
                name: "alpha".to_owned(),
                path: alpha_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
            JsLocalProviderSpec {
                name: "beta".to_owned(),
                path: beta_path.to_string_lossy().into_owned(),
                max_concurrent: Some(1),
                weight: None,
                metadata: None,
            },
        ];

        let result = sorafs_multi_fetch_local(
            plan_json,
            providers,
            Some(JsMultiFetchOptions {
                deny_providers: Some(vec!["alpha".into()]),
                ..Default::default()
            }),
        )
        .expect("multi-fetch succeeds with deny policy");

        assert_eq!(result.provider_reports.len(), 2);
        let alpha_report = result
            .provider_reports
            .iter()
            .find(|report| report.provider == "alpha")
            .expect("alpha report present");
        assert_eq!(alpha_report.successes, 0);
        assert_eq!(alpha_report.failures, 0);
        let beta_report = result
            .provider_reports
            .iter()
            .find(|report| report.provider == "beta")
            .expect("beta report present");
        assert_eq!(
            beta_report.successes as usize,
            plan.chunk_fetch_specs().len()
        );
        assert!(!beta_report.disabled);
        assert!(
            result
                .chunk_receipts
                .iter()
                .all(|receipt| receipt.provider == "beta")
        );
    }

    #[test]
    fn transfer_asset_instruction_json_roundtrip() {
        let source_account = sample_account("wonderland");
        let destination = sample_account("wonderland");
        let asset_definition: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_definition, source_account.clone());

        let transfer_box: TransferBox = Transfer::asset_numeric(
            asset_id,
            Numeric::from_str("25").expect("valid numeric"),
            destination,
        )
        .into();
        let instruction = InstructionBox::from(transfer_box);

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize instruction to json");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Transfer"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize instruction from json");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // End-to-end JSON roundtrip coverage is easier to read as one consolidated case table.
    fn rwa_instruction_json_roundtrip() {
        disable_packed_struct_once();
        let source_account = sample_account("wonderland");
        let destination = sample_account("wonderland");
        let rwa_id = sample_rwa_id("commodities", 0x31);
        let parent = RwaParentRef::new(
            sample_rwa_id("commodities", 0x32),
            Numeric::from_str("1.25").expect("valid numeric"),
        );
        let controls = RwaControlPolicy {
            controller_accounts: vec![source_account.clone()],
            controller_roles: Vec::new(),
            freeze_enabled: true,
            hold_enabled: true,
            force_transfer_enabled: true,
            redeem_enabled: false,
        };
        let new_rwa = NewRwa::new(
            DomainId::try_new("commodities", "universal").expect("valid domain id"),
            Numeric::from_str("10.5").expect("valid numeric"),
            iroha_primitives::numeric::NumericSpec::fractional(1),
            "vault-cert-001".to_owned(),
            Some(Name::from_str("Active").expect("valid status")),
            Metadata::default(),
            vec![parent.clone()],
            controls.clone(),
        );
        let cases = vec![
            norito_json!({
                "RegisterRwa": norito_json!({
                    "rwa": new_rwa_to_json(&new_rwa).expect("render new rwa"),
                })
            }),
            norito_json!({
                "TransferRwa": norito_json!({
                    "source": source_account.canonical_i105().expect("canonical I105 source"),
                    "rwa": rwa_id.to_string(),
                    "quantity": Numeric::from_str("2.5").expect("valid numeric"),
                    "destination": destination
                        .canonical_i105()
                        .expect("canonical I105 destination"),
                })
            }),
            norito_json!({
                "MergeRwas": norito_json!({
                    "parents": rwa_parent_refs_to_json(std::slice::from_ref(&parent)),
                    "primary_reference": "blend-001".to_owned(),
                    "status": Value::Null,
                    "metadata": Metadata::default(),
                })
            }),
            norito_json!({
                "RedeemRwa": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "quantity": Numeric::from_str("1").expect("valid numeric"),
                })
            }),
            norito_json!({ "FreezeRwa": norito_json!({ "rwa": rwa_id.to_string() }) }),
            norito_json!({ "UnfreezeRwa": norito_json!({ "rwa": rwa_id.to_string() }) }),
            norito_json!({
                "HoldRwa": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "quantity": Numeric::from_str("0.5").expect("valid numeric"),
                })
            }),
            norito_json!({
                "ReleaseRwa": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "quantity": Numeric::from_str("0.25").expect("valid numeric"),
                })
            }),
            norito_json!({
                "ForceTransferRwa": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "quantity": Numeric::from_str("1.5").expect("valid numeric"),
                    "destination": destination
                        .canonical_i105()
                        .expect("canonical I105 destination"),
                })
            }),
            norito_json!({
                "SetRwaControls": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "controls": controls.clone(),
                })
            }),
            norito_json!({
                "SetRwaKeyValue": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "key": Name::from_str("grade").expect("valid key"),
                    "value": norito_json!({
                        "origin": "AE",
                        "score": Numeric::from_str("9").expect("valid numeric"),
                    }),
                })
            }),
            norito_json!({
                "RemoveRwaKeyValue": norito_json!({
                    "rwa": rwa_id.to_string(),
                    "key": Name::from_str("grade").expect("valid key"),
                })
            }),
        ];

        for json_value in cases {
            let instruction =
                value_to_instruction(json_value.clone()).expect("deserialize RWA instruction");
            let rendered =
                instruction_to_json_value(&instruction).expect("serialize RWA instruction");
            assert_eq!(rendered, json_value);
        }
    }

    #[test]
    fn kaigi_join_instruction_json_roundtrip() {
        disable_packed_struct_once();
        let mut call_id = json::Map::new();
        call_id.insert("domain_id".into(), Value::String("wonderland.sora".into()));
        call_id.insert("call_name".into(), Value::String("weekly-sync".into()));

        let mut commitment = json::Map::new();
        commitment.insert("commitment".into(), Value::String(hash_literal(0x11)));
        commitment.insert("alias_tag".into(), Value::String("alice".into()));

        let mut nullifier = json::Map::new();
        nullifier.insert("digest".into(), Value::String(hash_literal(0x22)));
        nullifier.insert("issued_at_ms".into(), Value::Number(json::Number::U64(99)));

        let participant = sample_account("wonderland");

        let mut join = json::Map::new();
        join.insert("call_id".into(), Value::Object(call_id));
        join.insert(
            "participant".into(),
            Value::String(account_json_literal(&participant)),
        );
        join.insert("commitment".into(), Value::Object(commitment));
        join.insert("nullifier".into(), Value::Object(nullifier));
        join.insert("roster_root".into(), Value::String(hash_literal(0x33)));
        join.insert("proof".into(), Value::String("qrvM".into()));

        let mut kaigi = json::Map::new();
        kaigi.insert("JoinKaigi".into(), Value::Object(join));

        let mut outer = json::Map::new();
        outer.insert("Kaigi".into(), Value::Object(kaigi));
        let json_value = Value::Object(outer);

        let instruction = value_to_instruction(json_value.clone()).expect("parse join json");
        let bytes = norito::to_bytes(&instruction).expect("encode join json");
        let decoded: InstructionBox = decode_from_bytes(&bytes).expect("decode join instruction");
        let rendered = instruction_to_json_value(&decoded).expect("render join json");
        assert_eq!(rendered, json_value);
    }

    #[test]
    fn mint_trigger_instruction_json_roundtrip() {
        let trigger_id: TriggerId = "notify-users".parse().expect("valid trigger id");

        let mint_box: MintBox = Mint::trigger_repetitions(3, trigger_id.clone()).into();
        let instruction = InstructionBox::from(mint_box);

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize instruction to json");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Mint"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize instruction from json");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn transfer_domain_instruction_json_roundtrip() {
        let source_account = sample_account("wonderland");
        let destination = sample_account("wonderland");
        let domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("valid domain id");

        let transfer_box: TransferBox = Transfer::domain(
            source_account.clone(),
            domain_id.clone(),
            destination.clone(),
        )
        .into();
        let instruction = InstructionBox::from(transfer_box);

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize instruction to json");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Transfer"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize instruction from json");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn register_peer_instruction_json_roundtrip() {
        let keypair = KeyPair::random();
        let peer_id = PeerId::from(keypair.public_key().clone());
        let register = RegisterPeerWithPop::new(peer_id.clone(), vec![0xAA, 0xBB]);
        let instruction = InstructionBox::from(RegisterBox::Peer(register));

        let json_value = instruction_to_json_value(&instruction).expect("serialize register peer");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Register"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize register peer");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn register_nft_instruction_json_roundtrip() {
        let nft_id: NftId = "collectible$wonderland".parse().expect("valid nft id");

        let mut nft_fields = json::Map::new();
        nft_fields.insert(
            "id".to_owned(),
            json::to_value(&nft_id).expect("serialize nft id"),
        );
        nft_fields.insert(
            "content".to_owned(),
            json::to_value(&Metadata::default()).expect("serialize metadata"),
        );
        let mut register_map = json::Map::new();
        register_map.insert("Nft".to_owned(), json::Value::Object(nft_fields));

        let mut outer = json::Map::new();
        outer.insert("Register".to_owned(), json::Value::Object(register_map));
        let json_value = json::Value::Object(outer);

        let instruction =
            value_to_instruction(json_value.clone()).expect("parse register nft instruction");
        let rendered =
            instruction_to_json_value(&instruction).expect("render register nft instruction");
        assert_eq!(rendered, json_value);
    }

    #[test]
    fn unregister_peer_instruction_json_roundtrip() {
        let keypair = KeyPair::random();
        let peer_id = PeerId::from(keypair.public_key().clone());
        let unregister = Unregister::<Peer>::peer(peer_id);
        let instruction = InstructionBox::from(UnregisterBox::Peer(unregister));

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize unregister peer");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Unregister"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize unregister peer");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn transfer_nft_instruction_json_roundtrip() {
        let source_account = sample_account("wonderland");
        let destination = sample_account("wonderland");
        let nft_id: NftId = "dragon$wonderland".parse().expect("valid nft id");

        let transfer_box: TransferBox =
            Transfer::nft(source_account.clone(), nft_id.clone(), destination.clone()).into();
        let instruction = InstructionBox::from(transfer_box);

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize instruction to json");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Transfer"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize instruction from json");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn create_kaigi_instruction_json_roundtrip() {
        let host = sample_account("wonderland");
        let relay = sample_account("wonderland");
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let manifest = KaigiRelayManifest {
            hops: vec![KaigiRelayHop {
                relay_id: relay.clone(),
                hpke_public_key: vec![0x01, 0x02, 0x03, 0x04],
                weight: 7,
            }],
            expiry_ms: 1_700_000_500_000,
        };
        let call = NewKaigi {
            id: call_id.clone(),
            host: host.clone(),
            title: Some("Weekly Sync".to_owned()),
            description: Some("Roadmap alignment".to_owned()),
            max_participants: Some(16),
            gas_rate_per_minute: 120,
            metadata: Metadata::default(),
            scheduled_start_ms: Some(1_700_000_000_000),
            billing_account: Some(host.clone()),
            privacy_mode: KaigiPrivacyMode::Transparent,
            room_policy: KaigiRoomPolicy::Authenticated,
            relay_manifest: Some(manifest),
        };
        let commitment = KaigiParticipantCommitment {
            commitment: Hash::new(b"commitment::host"),
            alias_tag: Some("host".to_owned()),
        };
        let nullifier = KaigiParticipantNullifier {
            digest: Hash::new(b"nullifier::host"),
            issued_at_ms: 7,
        };
        let instruction: InstructionBox = Box::new(CreateKaigi {
            call,
            commitment: Some(commitment),
            nullifier: Some(nullifier),
            roster_root: Some(Hash::new(b"roster-root")),
            proof: Some(vec![0xFA, 0xCE]),
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize Kaigi instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn join_kaigi_instruction_json_roundtrip() {
        disable_packed_struct_once();
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let participant = sample_account("wonderland");
        let commitment = KaigiParticipantCommitment {
            commitment: Hash::new(b"commitment::alice"),
            alias_tag: Some("alice".to_owned()),
        };
        let nullifier = KaigiParticipantNullifier {
            digest: Hash::new(b"nullifier::alice"),
            issued_at_ms: 42,
        };
        let roster_root = Hash::new(b"roster-root");
        let proof = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let join = JoinKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: Some(commitment),
            nullifier: Some(nullifier),
            roster_root: Some(roster_root),
            proof: Some(proof),
        };
        let instruction: InstructionBox = Box::new(join).into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi join instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize Kaigi join instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn join_kaigi_instruction_norito_roundtrip_from_json() {
        disable_packed_struct_once();
        let participant = account_json_literal(&sample_account("wonderland"));
        let payload = r#"{
            "Kaigi": {
                "JoinKaigi": {
                    "call_id": {
                        "domain_id": "wonderland.sora",
                        "call_name": "weekly-sync"
                    },
                    "participant": "__PARTICIPANT__",
                    "commitment": {
                        "commitment": "hash:1111111111111111111111111111111111111111111111111111111111111111#4667",
                        "alias_tag": null
                    },
                    "nullifier": {
                        "digest": "hash:2222222222222222222222222222222222222222222222222222222222222223#F3BF",
                        "issued_at_ms": 1700000000000
                    },
                    "roster_root": "hash:3333333333333333333333333333333333333333333333333333333333333333#70D6",
                    "proof": "qrvM"
                }
            }
        }"#;
        let payload = payload.replace("__PARTICIPANT__", &participant);

        let instruction =
            instruction_from_json(&payload).expect("builder JSON must translate into instruction");
        if let Some(join) = instruction.as_any().downcast_ref::<JoinKaigi>() {
            assert!(
                join.commitment().is_some(),
                "JSON builder should supply a commitment"
            );
            assert!(
                join.nullifier().is_some(),
                "JSON builder should supply a nullifier"
            );
            assert!(
                join.roster_root().is_some(),
                "JSON builder should supply a roster root"
            );
        } else {
            panic!("expected JoinKaigi instruction");
        }
        let encoded =
            norito::to_bytes(&instruction).expect("encoding JoinKaigi instruction to bytes");
        let decoded: InstructionBox =
            decode_from_bytes(&encoded).expect("deserialize JoinKaigi instruction");
        assert_eq!(decoded, instruction);
    }

    #[test]
    fn leave_kaigi_instruction_json_roundtrip() {
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let participant = sample_account("wonderland");
        let commitment = KaigiParticipantCommitment {
            commitment: Hash::new(b"commitment::leave"),
            alias_tag: None,
        };
        let nullifier = KaigiParticipantNullifier {
            digest: Hash::new(b"nullifier::leave"),
            issued_at_ms: 84,
        };
        let roster_root = Hash::new(b"leave-root");
        let proof = vec![0xDE, 0xAD];
        let leave = LeaveKaigi {
            call_id: call_id.clone(),
            participant: participant.clone(),
            commitment: Some(commitment),
            nullifier: Some(nullifier),
            roster_root: Some(roster_root),
            proof: Some(proof),
        };
        let instruction: InstructionBox = Box::new(leave).into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi leave instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize Kaigi leave instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn end_kaigi_instruction_json_roundtrip() {
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let commitment = KaigiParticipantCommitment {
            commitment: Hash::new(b"commitment::host"),
            alias_tag: Some("host".to_owned()),
        };
        let nullifier = KaigiParticipantNullifier {
            digest: Hash::new(b"nullifier::end"),
            issued_at_ms: 99,
        };
        let end = EndKaigi {
            call_id: call_id.clone(),
            ended_at_ms: Some(1_700_222_000_000),
            commitment: Some(commitment),
            nullifier: Some(nullifier),
            roster_root: Some(Hash::new(b"roster-root")),
            proof: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let instruction: InstructionBox = Box::new(end).into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi end instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize Kaigi end instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn record_kaigi_usage_instruction_json_roundtrip() {
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let usage_commitment = Hash::new(b"usage::commitment");
        let proof = vec![0xAB, 0xCD];
        let usage = RecordKaigiUsage {
            call_id: call_id.clone(),
            duration_ms: 60_000,
            billed_gas: 512,
            usage_commitment: Some(usage_commitment),
            proof: Some(proof),
        };
        let instruction: InstructionBox = Box::new(usage).into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi usage instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize Kaigi usage instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn report_kaigi_relay_health_instruction_json_roundtrip() {
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let relay_id = sample_account("wonderland");
        let report = ReportKaigiRelayHealth {
            call_id: call_id.clone(),
            relay_id: relay_id.clone(),
            status: KaigiRelayHealthStatus::Degraded,
            reported_at_ms: 1_701_123_456_789,
            notes: Some("latency spike observed".to_owned()),
        };
        let instruction: InstructionBox = Box::new(report).into_instruction_box();

        let json_value = instruction_to_json_value(&instruction)
            .expect("serialize Kaigi relay health instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed = value_to_instruction(json_value.clone())
            .expect("deserialize Kaigi relay health instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn set_kaigi_relay_manifest_instruction_json_roundtrip() {
        let call_id = sample_kaigi_id("wonderland", "weekly-sync");
        let relay = sample_account("wonderland");
        let manifest = KaigiRelayManifest {
            hops: vec![KaigiRelayHop {
                relay_id: relay,
                hpke_public_key: vec![0x10, 0x11, 0x12],
                weight: 3,
            }],
            expiry_ms: 1_700_111_000_000,
        };
        let instruction: InstructionBox = Box::new(SetKaigiRelayManifest {
            call_id: call_id.clone(),
            relay_manifest: Some(manifest),
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi manifest instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed = value_to_instruction(json_value.clone())
            .expect("deserialize Kaigi manifest instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn register_kaigi_relay_instruction_json_roundtrip() {
        let relay = sample_account("wonderland");
        let instruction: InstructionBox = Box::new(RegisterKaigiRelay {
            relay: KaigiRelayRegistration {
                relay_id: relay,
                hpke_public_key: vec![0xAA, 0xBB, 0xCC, 0xDD],
                bandwidth_class: 9,
            },
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize Kaigi relay instruction");
        let outer = json_value
            .as_object()
            .expect("instruction JSON should be an object");
        assert!(outer.contains_key("Kaigi"));

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize Kaigi relay instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn governance_propose_deploy_contract_instruction_json_roundtrip() {
        let instruction: InstructionBox = Box::new(ProposeDeployContract {
            contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address"),
            code_hash_hex: "aa".repeat(32),
            abi_hash_hex: "bb".repeat(32),
            abi_version: "1".to_owned(),
            window: Some(AtWindow {
                lower: 10,
                upper: 20,
            }),
            mode: Some(VotingMode::Plain),
            manifest_provenance: None,
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize governance instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("ProposeDeployContract"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize governance instruction");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn governance_cast_zk_ballot_instruction_json_roundtrip() {
        let instruction: InstructionBox = Box::new(CastZkBallot {
            election_id: "ref-1".to_owned(),
            proof_b64: STANDARD.encode([0x01, 0x02, 0x03]),
            public_inputs_json: r#"{"tally":"aye"}"#.to_owned(),
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize CastZkBallot instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("CastZkBallot"))
                .is_some()
        );

        let reconstructed = value_to_instruction(json_value).expect("deserialize CastZkBallot");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_rejects_deprecated_keys() {
        let mut inner = json::Map::new();
        let owner = canonical_owner_literal("wonderland");
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01, 0x02])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(format!(
                r#"{{"owner":"{owner}","amount":"10","durationBlocks":64}}"#
            )),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        assert!(value_to_instruction(json::Value::Object(outer)).is_err());
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_canonicalizes_hex_hints() {
        let mut inner = json::Map::new();
        let owner = canonical_owner_literal("wonderland");
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01, 0x02])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(
                format!(
                    r#"{{"owner":"{owner}","amount":"10","duration_blocks":64,"root_hint":"0x{}","nullifier":"blake2b32:{}"}}"#,
                    "Aa".repeat(32),
                    "BB".repeat(32)
                ),
            ),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        let instruction =
            value_to_instruction(json::Value::Object(outer)).expect("deserialize CastZkBallot");
        let ballot = instruction
            .as_any()
            .downcast_ref::<CastZkBallot>()
            .expect("CastZkBallot");
        let parsed: json::Value =
            json::from_str(&ballot.public_inputs_json).expect("parse public inputs");
        let root_hint = parsed
            .get("root_hint")
            .and_then(json::Value::as_str)
            .expect("root_hint");
        let nullifier = parsed
            .get("nullifier")
            .and_then(json::Value::as_str)
            .expect("nullifier");
        assert_eq!(root_hint, "aa".repeat(32));
        assert_eq!(nullifier, "bb".repeat(32));
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_rejects_noncanonical_owner() {
        let mut inner = json::Map::new();
        let owner = noncanonical_owner_literal("wonderland");
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(format!(
                r#"{{"owner":"{owner}","amount":"10","duration_blocks":64}}"#
            )),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        assert!(value_to_instruction(json::Value::Object(outer)).is_err());
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_rejects_partial_hints() {
        let mut inner = json::Map::new();
        let owner = canonical_owner_literal("wonderland");
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(format!(r#"{{"owner":"{owner}"}}"#)),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        assert!(value_to_instruction(json::Value::Object(outer)).is_err());
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_rejects_deprecated_aliases() {
        let mut inner = json::Map::new();
        let owner = canonical_owner_literal("wonderland");
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(format!(
                r#"{{"owner":"{owner}","amount":"10","duration_blocks":64,"rootHint":"aa"}}"#
            )),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        assert!(value_to_instruction(json::Value::Object(outer)).is_err());
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_rejects_invalid_hex() {
        let mut inner = json::Map::new();
        let owner = canonical_owner_literal("wonderland");
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(format!(
                r#"{{"owner":"{owner}","amount":"10","duration_blocks":64,"root_hint":"not-hex"}}"#
            )),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        assert!(value_to_instruction(json::Value::Object(outer)).is_err());
    }

    #[test]
    fn governance_cast_zk_ballot_public_inputs_rejects_non_object_json() {
        let mut inner = json::Map::new();
        inner.insert(
            "election_id".to_owned(),
            json::Value::String("ref-1".to_owned()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(BASE64.encode([0x01])),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String("[1,2]".to_owned()),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));

        assert!(value_to_instruction(json::Value::Object(outer)).is_err());
    }

    #[test]
    fn governance_cast_plain_ballot_instruction_json_roundtrip() {
        let owner = sample_account("wonderland");
        let instruction: InstructionBox = Box::new(CastPlainBallot {
            referendum_id: "ref-plain".to_owned(),
            owner: owner.clone(),
            amount: 1_000,
            duration_blocks: 42,
            direction: 1,
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize CastPlainBallot instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("CastPlainBallot"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize CastPlainBallot");
        assert_eq!(reconstructed, instruction);

        // Ensure owner round-tripped correctly.
        let owner_json = json_value
            .as_object()
            .unwrap()
            .get("CastPlainBallot")
            .and_then(|value| value.get("owner"))
            .and_then(|value| value.as_str())
            .expect("owner string present");
        assert_eq!(owner_json, account_json_literal(&owner));
    }

    #[test]
    fn governance_register_citizen_instruction_json_roundtrip() {
        let owner = sample_account("wonderland");
        let instruction: InstructionBox = Box::new(RegisterCitizen {
            owner: owner.clone(),
            amount: 10_000,
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize RegisterCitizen instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("RegisterCitizen"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize RegisterCitizen");
        assert_eq!(reconstructed, instruction);

        let owner_json = json_value
            .as_object()
            .unwrap()
            .get("RegisterCitizen")
            .and_then(|value| value.get("owner"))
            .and_then(|value| value.as_str())
            .expect("owner string present");
        assert_eq!(owner_json, account_json_literal(&owner));
    }

    #[test]
    fn governance_enact_referendum_instruction_json_roundtrip() {
        let instruction: InstructionBox = Box::new(EnactReferendum {
            referendum_id: sample_hash(0x11),
            preimage_hash: sample_hash(0x22),
            at_window: AtWindow {
                lower: 0,
                upper: 100,
            },
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize EnactReferendum");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("EnactReferendum"))
                .is_some()
        );

        let reconstructed = value_to_instruction(json_value).expect("deserialize EnactReferendum");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn governance_finalize_referendum_instruction_json_roundtrip() {
        let instruction: InstructionBox = Box::new(FinalizeReferendum {
            referendum_id: "ref-final".to_owned(),
            proposal_id: sample_hash(0x33),
        })
        .into_instruction_box();

        let json_value =
            instruction_to_json_value(&instruction).expect("serialize FinalizeReferendum");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("FinalizeReferendum"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value).expect("deserialize FinalizeReferendum");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn governance_persist_council_instruction_json_roundtrip() {
        let member = sample_account("wonderland");
        let instruction: InstructionBox = Box::new(PersistCouncilForEpoch {
            epoch: 10,
            members: vec![member.clone()],
            alternates: vec![member.clone()],
            verified: 2,
            candidates_count: 5,
            derived_by: CouncilDerivationKind::Fallback,
        })
        .into_instruction_box();

        let json_value = instruction_to_json_value(&instruction)
            .expect("serialize PersistCouncilForEpoch instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("PersistCouncilForEpoch"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize PersistCouncilForEpoch");
        assert_eq!(reconstructed, instruction);

        let derived = json_value
            .as_object()
            .unwrap()
            .get("PersistCouncilForEpoch")
            .and_then(|value| value.get("derived_by"))
            .and_then(|value| value.as_str())
            .expect("derived_by string present");
        assert_eq!(derived, "Fallback");

        let member_json = json_value
            .as_object()
            .unwrap()
            .get("PersistCouncilForEpoch")
            .and_then(|value| value.get("members"))
            .and_then(|value| value.as_array())
            .and_then(|arr| arr.first())
            .and_then(|value| value.as_str())
            .expect("member string present");
        assert_eq!(member_json, account_json_literal(&member));
    }

    #[test]
    fn governance_submit_agenda_proposal_instruction_json_roundtrip() {
        let instruction: InstructionBox = Box::new(SubmitAgendaProposal {
            proposal: sample_agenda_proposal(),
        })
        .into_instruction_box();

        let json_value = instruction_to_json_value(&instruction)
            .expect("serialize SubmitAgendaProposal instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("SubmitAgendaProposal"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize SubmitAgendaProposal");
        assert_eq!(reconstructed, instruction);

        let proposal_id = json_value
            .as_object()
            .unwrap()
            .get("SubmitAgendaProposal")
            .and_then(|value| value.get("proposal"))
            .and_then(|value| value.get("proposal_id"))
            .and_then(|value| value.as_str())
            .expect("proposal id present");
        assert_eq!(proposal_id, "AC-2026-001");
    }

    #[test]
    fn smart_contract_code_instruction_json_roundtrip() {
        let signing_key = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
        let manifest = ContractManifest {
            code_hash: Some(Hash::prehashed(sample_hash(0xAA))),
            abi_hash: Some(Hash::prehashed(sample_hash(0xBB))),
            compiler_fingerprint: Some("rustc-1.79".to_owned()),
            features_bitmap: Some(42),
            access_set_hints: Some(AccessSetHints {
                read_keys: vec!["account:alice".to_owned()],
                write_keys: vec!["contract:foo".to_owned()],
                dynamic_reads: Vec::new(),
                dynamic_writes: Vec::new(),
            }),
            entrypoints: Some(vec![EntrypointDescriptor {
                name: "upgrade_ledger".to_owned(),
                kind: EntryPointKind::Kaizen,
                params: vec![EntrypointParamDescriptor {
                    name: "reason".to_owned(),
                    type_name: "String".to_owned(),
                }],
                return_type: Some("bool".to_owned()),
                permission: Some("can_upgrade".to_owned()),
                read_keys: vec!["contract:ledger".to_owned()],
                write_keys: vec!["contract:ledger".to_owned()],
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
            }]),
            states: None,
            kotoba: Some(vec![KotobaTranslationEntry {
                msg_id: "contract.title".to_owned(),
                translations: vec![KotobaTranslation {
                    lang: "en".to_owned(),
                    text: "Ledger Contract".to_owned(),
                }],
            }]),
            provenance: None,
        }
        .signed(&signing_key);
        let instruction: InstructionBox = Box::new(RegisterSmartContractCode {
            manifest: manifest.clone(),
        })
        .into_instruction_box();

        let json_value = instruction_to_json_value(&instruction)
            .expect("serialize RegisterSmartContractCode instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("RegisterSmartContractCode"))
                .is_some()
        );

        let reconstructed = value_to_instruction(json_value.clone())
            .expect("deserialize RegisterSmartContractCode");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn decode_signed_transaction_accepts_supported_norito_rpc_fixture_subset() {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/norito_rpc/transaction_fixtures.manifest.json");
        let manifest_bytes = fs::read(&manifest_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
        let manifest: Value = json::from_slice(&manifest_bytes)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", manifest_path.display()));
        let names = [
            "ivm_transfer",
            "grant_revoke_role_permission",
            "set_parameter_next_mode",
            "executor_upgrade_demo",
            "register_peer_with_pop_demo",
            "register_nft_demo",
            "trigger_repetitions_demo",
        ];

        for name in names {
            let fixture = manifest
                .get("fixtures")
                .and_then(Value::as_array)
                .and_then(|fixtures| {
                    fixtures
                        .iter()
                        .find(|fixture| fixture.get("name").and_then(Value::as_str) == Some(name))
                })
                .unwrap_or_else(|| panic!("fixture {name} missing from norito fixture manifest"));
            let signed_base64 = fixture
                .get("signed_base64")
                .and_then(Value::as_str)
                .expect("fixture signed_base64");
            let signed_bytes = BASE64
                .decode(signed_base64)
                .unwrap_or_else(|err| panic!("failed to decode {name} signed payload: {err}"));
            decode_signed_transaction(&signed_bytes)
                .unwrap_or_else(|err| panic!("failed to decode fixture {name}: {err}"));
        }
    }

    #[test]
    fn decode_signed_transaction_accepts_versioned_bytes() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(1));
        let signed = builder.sign(keypair.private_key());
        let mut versioned = vec![1];
        versioned.extend(norito::codec::encode_adaptive(&signed));

        let decoded = decode_signed_transaction(&versioned)
            .expect("versioned signed transaction must decode");

        assert_eq!(decoded, signed);
    }

    #[test]
    fn sign_js_transaction_checked_signing_verifies() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let asset_definition: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_definition, authority.clone());
        let instruction: InstructionBox =
            Mint::asset_numeric(Numeric::from_str("10").expect("valid numeric"), asset_id).into();

        let tx = sign_js_transaction(
            TransactionBuilder::new(chain_id, authority.clone()).with_instructions([instruction]),
            keypair.private_key(),
            "test",
        )
        .expect("checked signing should succeed");

        assert_eq!(tx.authority(), &authority);
        tx.verify_signature()
            .expect("checked signed JS transaction should verify");
    }

    #[test]
    fn smart_contract_bytes_instruction_json_roundtrip() {
        let code_bytes = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let instruction: InstructionBox = Box::new(RegisterSmartContractBytes {
            code_hash: Hash::prehashed(sample_hash(0xCC)),
            code: code_bytes.clone(),
        })
        .into_instruction_box();

        let json_value = instruction_to_json_value(&instruction)
            .expect("serialize RegisterSmartContractBytes instruction");
        let payload = json_value
            .as_object()
            .and_then(|map| map.get("RegisterSmartContractBytes"))
            .and_then(|value| value.as_object())
            .expect("bytes payload present");
        assert_eq!(
            payload.get("code_hash"),
            Some(&json::Value::String(hash_literal(0xCC))),
        );
        assert_eq!(
            payload.get("code"),
            Some(&json::Value::String(STANDARD.encode(&code_bytes))),
        );

        let reconstructed = value_to_instruction(json_value.clone())
            .expect("deserialize RegisterSmartContractBytes");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn activate_contract_instance_instruction_json_roundtrip() {
        let authority = AccountId::new(KeyPair::random().public_key().clone());
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            0,
            &authority,
            1,
            iroha_data_model::nexus::DataSpaceId::new(0),
        )
        .expect("contract address");
        let instruction: InstructionBox = Box::new(ActivateContractInstance {
            contract_address,
            code_hash: Hash::prehashed(sample_hash(0x44)),
        })
        .into_instruction_box();

        let json_value = instruction_to_json_value(&instruction)
            .expect("serialize ActivateContractInstance instruction");
        assert!(
            json_value
                .as_object()
                .and_then(|map| map.get("ActivateContractInstance"))
                .is_some()
        );

        let reconstructed =
            value_to_instruction(json_value.clone()).expect("deserialize ActivateContractInstance");
        assert_eq!(reconstructed, instruction);
    }

    #[test]
    fn js_builder_create_kaigi_payload_matches() {
        // Mirrors the payload assembled by the JavaScript builders/tests.
        let host = account_json_literal(&sample_account("wonderland"));
        let billing_account = account_json_literal(&sample_account("wonderland"));
        let relay_id = account_json_literal(&sample_account("wonderland"));
        let payload = norito_json!({
            "Kaigi": norito_json!({
                "CreateKaigi": norito_json!({
                    "call": norito_json!({
                        "id": norito_json!({
                            "domain_id": "wonderland.sora",
                            "call_name": "weekly-sync"
                        }),
                        "host": host,
                        "title": "Weekly Sync",
                        "description": "Roadmap alignment",
                        "max_participants": 16,
                        "gas_rate_per_minute": 120,
                        "metadata": norito_json!({
                            "topic": "status"
                        }),
                        "scheduled_start_ms": 1_700_000_000_000_u64,
                        "billing_account": billing_account,
                        "privacy_mode": norito_json!({
                            "mode": "ZkRosterV1",
                            "state": json::Value::Null
                        }),
                        "relay_manifest": norito_json!({
                            "hops": vec![norito_json!({
                                "relay_id": relay_id,
                                "hpke_public_key": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
                                "weight": 5
                            })],
                            "expiry_ms": 1_700_111_000_000_u64
                        })
                    })
                })
            })
        });
        let json_payload = json::to_json(&payload).expect("serialize payload to json");

        let value: json::Value =
            norito::json::from_json(&json_payload).expect("parse builder json into Value");
        if let Some(host) = value
            .get("Kaigi")
            .and_then(|v| v.get("CreateKaigi"))
            .and_then(|v| v.get("call"))
            .and_then(|v| v.get("host"))
            .and_then(|v| v.as_str())
        {
            AccountId::parse_encoded(host)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("host account id");
        }
        if let Some(billing) = value
            .get("Kaigi")
            .and_then(|v| v.get("CreateKaigi"))
            .and_then(|v| v.get("call"))
            .and_then(|v| v.get("billing_account"))
            .and_then(|v| v.as_str())
        {
            AccountId::parse_encoded(billing)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("billing account id");
        }
        if let Some(relay_id) = value
            .get("Kaigi")
            .and_then(|v| v.get("CreateKaigi"))
            .and_then(|v| v.get("call"))
            .and_then(|v| v.get("relay_manifest"))
            .and_then(|v| v.get("hops"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.get("relay_id"))
            .and_then(|v| v.as_str())
        {
            AccountId::parse_encoded(relay_id)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("relay account id");
        }

        instruction_from_json(&json_payload).expect("JS builder payload must be parsable");
    }

    #[test]
    fn build_transaction_from_instructions_json_roundtrip() {
        disable_packed_struct_once();
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let authority = AccountId::new(keypair.public_key().clone());

        let asset_definition: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_definition, authority.clone());

        let instruction_box: InstructionBox = Mint::asset_numeric(
            Numeric::from_str("10").expect("valid numeric"),
            asset_id.clone(),
        )
        .into();

        let instruction_json = json::to_json(&instruction_to_json_value(&instruction_box).unwrap())
            .expect("instruction json");

        let (_, secret_bytes) = keypair.private_key().to_bytes();

        let result = build_transaction_from_instructions_json(
            chain_id.clone(),
            authority.clone(),
            vec![instruction_json],
            None,
            Some(1_700_000_000_000),
            Some(5_000),
            Some(42),
            &secret_bytes,
        )
        .expect("transaction built");

        let tx = decode_signed_transaction(result.signed_transaction.as_ref()).expect("decode");
        assert_eq!(tx.authority(), &authority);
        assert_eq!(tx.chain(), &chain_id);
        tx.verify_signature()
            .expect("assembled transaction signature should verify");
        match tx.instructions() {
            Executable::Instructions(batch) => {
                assert_eq!(batch.len(), 1);
                assert_eq!(batch.iter().next().unwrap(), &instruction_box);
            }
            other => panic!("expected instruction batch, got {other:?}"),
        }
        assert_eq!(
            result.hash.as_ref(),
            tx.hash().as_ref(),
            "hash must match signed transaction hash"
        );
    }

    #[test]
    fn build_transaction_from_instructions_json_accepts_kagemusha_instruction_archive() {
        disable_packed_struct_once();
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let instruction = sample_kagemusha_transfer_instruction_for_js_host();
        let archive = norito::to_bytes(&instruction).expect("encode Kagemusha transfer");
        let instruction_json =
            kagemusha_instruction_archive_json("KagemushaTransfer", &STANDARD.encode(&archive), "");
        let (_, secret_bytes) = keypair.private_key().to_bytes();

        let result = build_transaction_from_instructions_json(
            chain_id,
            authority,
            vec![instruction_json],
            None,
            Some(1_700_000_000_000),
            Some(5_000),
            Some(42),
            &secret_bytes,
        )
        .expect("transaction built");

        let tx = decode_signed_transaction(result.signed_transaction.as_ref()).expect("decode");
        match tx.instructions() {
            Executable::Instructions(batch) => {
                assert_eq!(batch.len(), 1);
                let decoded = batch
                    .iter()
                    .next()
                    .expect("instruction")
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::offline::KagemushaTransfer>()
                    .expect("KagemushaTransfer instruction");
                assert_eq!(decoded, &instruction);
            }
            other => panic!("expected instruction batch, got {other:?}"),
        }
    }

    #[test]
    fn kagemusha_instruction_archive_json_rejects_adversarial_inputs() {
        let transfer = sample_kagemusha_transfer_instruction_for_js_host();
        let transfer_archive = norito::to_bytes(&transfer).expect("encode Kagemusha transfer");
        let cases = [
            (
                kagemusha_instruction_archive_json("KagemushaTransfer", "", ""),
                "Kagemusha instruction archive must not be empty",
            ),
            (
                kagemusha_instruction_archive_json(
                    "KagemushaTransfer",
                    &STANDARD.encode(&[0x01_u8, 0x02]),
                    "",
                ),
                "invalid KagemushaTransfer instruction archive",
            ),
            (
                kagemusha_instruction_archive_json(
                    "RedeemOfflineNoteV2",
                    &STANDARD.encode(&[0x01_u8, 0x02]),
                    "",
                ),
                "unsupported KagemushaInstructionArchive.type",
            ),
            (
                kagemusha_instruction_archive_json(
                    "RedeemKagemushaRecursive",
                    &STANDARD.encode(&transfer_archive),
                    "",
                ),
                "invalid RedeemKagemushaRecursive instruction archive",
            ),
            (
                kagemusha_instruction_archive_json(
                    "KagemushaTransfer",
                    &STANDARD.encode(&transfer_archive),
                    r#","extra":true"#,
                ),
                "KagemushaInstructionArchive contains unexpected field",
            ),
            (
                kagemusha_instruction_archive_json(
                    "KagemushaTransfer",
                    &STANDARD.encode(&transfer_archive).trim_end_matches('='),
                    "",
                ),
                "invalid KagemushaInstructionArchive.bytes_base64",
            ),
            (
                kagemusha_instruction_archive_json(
                    "KagemushaTransfer",
                    &format!(" {}", STANDARD.encode(&transfer_archive)),
                    "",
                ),
                "invalid KagemushaInstructionArchive.bytes_base64",
            ),
        ];

        for (instruction_json, expected) in cases {
            let err = instruction_from_json(&instruction_json)
                .expect_err("adversarial Kagemusha archive payload must fail");
            assert!(
                err.to_string().contains(expected),
                "expected {expected:?} in error, got {err}",
            );
        }
    }

    fn kagemusha_instruction_archive_json(
        instruction_type: &str,
        bytes_base64: &str,
        extra_fields: &str,
    ) -> String {
        format!(
            r#"{{"KagemushaInstructionArchive":{{"type":"{instruction_type}","bytes_base64":"{bytes_base64}"{extra_fields}}}}}"#
        )
    }

    fn sample_kagemusha_transfer_instruction_for_js_host()
    -> iroha_data_model::isi::offline::KagemushaTransfer {
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain"),
            "kgm".parse().expect("asset name"),
        );
        iroha_data_model::isi::offline::KagemushaTransfer::new(
            asset_definition,
            vec![[0x11; 32]],
            vec![[0x22; 32]],
            ProofAttachment::new_ref(
                "halo2/ipa".parse().expect("backend ident"),
                ProofBox::new(
                    "halo2/ipa".parse().expect("proof backend ident"),
                    vec![0xAA, 0xBB, 0xCC],
                ),
                VerifyingKeyId::new("halo2/ipa", "js-host-kagemusha-transfer"),
            ),
            Some([0x33; 32]),
        )
    }

    #[test]
    fn build_ivm_proved_transaction_roundtrip() {
        disable_packed_struct_once();
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let authority = AccountId::new(keypair.public_key().clone());
        let proved = IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x4e, 0x52, 0x54, 0x30]),
            overlay: Vec::<InstructionBox>::new().into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas-policy"),
        };
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".parse().expect("backend ident"),
            ProofBox::new(
                "halo2/ipa".parse().expect("proof backend ident"),
                vec![0xAA, 0xBB, 0xCC],
            ),
            VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1"),
        );
        let proved_json = json::to_json(&proved).expect("proved json");
        let attachment_json = json::to_json(&attachment).expect("attachment json");
        let (_, secret_bytes) = keypair.private_key().to_bytes();

        let result = build_ivm_proved_transaction(
            chain_id.to_string(),
            account_json_literal(&authority),
            proved_json,
            attachment_json,
            Some(r#"{"gas_limit":1000}"#.to_owned()),
            Some(1_700_000_000_000),
            Some(5_000),
            Some(42),
            Uint8Array::from(secret_bytes.to_vec()),
        )
        .expect("transaction built");

        let tx = decode_signed_transaction(result.signed_transaction.as_ref()).expect("decode");
        assert_eq!(tx.authority(), &authority);
        assert_eq!(tx.chain(), &chain_id);
        match tx.instructions() {
            Executable::IvmProved(decoded) => {
                assert_eq!(decoded, &proved);
            }
            other => panic!("expected IvmProved executable, got {other:?}"),
        }
        let attachments = tx.attachments().expect("proof attachments");
        assert_eq!(attachments.0, vec![attachment]);
        assert_eq!(
            result.hash.as_ref(),
            tx.hash().as_ref(),
            "hash must match signed transaction hash"
        );
    }

    #[test]
    fn parse_account_id_accepts_taira_i105_literals() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let authority_i105 = AccountAddress::from_account_id(&authority)
            .expect("account address")
            .to_i105_for_discriminant(369)
            .expect("taira i105");

        let parsed = parse_account_id(&authority_i105, "authority account id")
            .expect("parse Taira I105 account id");

        assert_eq!(parsed, authority);
    }

    #[test]
    fn build_transaction_accepts_taira_i105_shield_fields() {
        disable_packed_struct_once();
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let authority_i105 = AccountAddress::from_account_id(&authority)
            .expect("account address")
            .to_i105_for_discriminant(369)
            .expect("taira i105");
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let asset_definition: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        );
        let instruction = Shield::new(
            asset_definition,
            authority.clone(),
            7,
            [0x11; 32],
            iroha_data_model::confidential::ConfidentialEncryptedPayload::new(
                [0x22; 32],
                [0x33; 24],
                b"ciphertext".to_vec(),
            ),
        );
        let instruction_box: InstructionBox = instruction.into();
        let mut instruction_json =
            instruction_to_json_value(&instruction_box).expect("instruction json");
        instruction_json
            .get_mut("zk")
            .and_then(json::Value::as_object_mut)
            .and_then(|zk| zk.get_mut("Shield"))
            .and_then(json::Value::as_object_mut)
            .expect("shield payload")
            .insert(
                "from".to_owned(),
                json::Value::String(authority_i105.clone()),
            );
        let instruction_json =
            json::to_json(&instruction_json).expect("serialized instruction json");
        let (_, secret_bytes) = keypair.private_key().to_bytes();

        let result = build_transaction(
            chain_id.to_string(),
            authority_i105,
            vec![instruction_json],
            None,
            None,
            None,
            None,
            Uint8Array::from(secret_bytes.to_vec()),
        )
        .expect("transaction built");

        let tx = decode_signed_transaction(result.signed_transaction.as_ref()).expect("decode");
        assert_eq!(tx.authority(), &authority);
        match tx.instructions() {
            Executable::Instructions(batch) => {
                assert_eq!(batch.len(), 1);
                let first = batch.iter().next().expect("shield instruction");
                let shield = first
                    .as_any()
                    .downcast_ref::<Shield>()
                    .expect("shield instruction");
                assert_eq!(shield.from, authority);
            }
            other => panic!("expected instruction batch, got {other:?}"),
        }
    }

    #[test]
    fn decode_transaction_receipt_json_roundtrip() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
            signed_transaction_hash: None,
            submitted_at_ms: 42,
            submitted_at_height: 7,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let decoded =
            decode_transaction_receipt_json(bytes.into()).expect("decode receipt into json");
        let expected = json::to_json(&receipt).expect("serialize receipt json");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn build_transaction_with_empty_instructions_fails() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let chain_id: ChainId = "test-chain".parse().expect("valid chain id");
        let (_, secret_bytes) = keypair.private_key().to_bytes();

        let result = build_transaction_from_instructions_json(
            chain_id,
            authority,
            Vec::new(),
            None,
            None,
            None,
            None,
            &secret_bytes,
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_time_trigger_action_encodes_expected_schedule() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority_id = AccountId::new(keypair.public_key().clone());
        let encoded = build_time_trigger_action(
            account_json_literal(&authority_id),
            vec![
                "{\"Mint\":{\"TriggerRepetitions\":{\"object\":1,\"destination\":\"demo::trigger\"}}}"
                    .to_owned(),
            ],
            1_735_000_000_000i64,
            Some(60_000i64),
            Some(2),
            Some("{\"label\":\"demo\"}".to_owned()),
        )
        .expect("time trigger action");
        let bytes = STANDARD.decode(encoded).expect("base64");
        let archived = from_bytes::<Action>(&bytes).expect("decode action");
        let action = Action::try_deserialize(archived).expect("action value");
        assert_eq!(action.authority(), &authority_id);
        assert!(matches!(action.repeats(), Repeats::Exactly(2)));
        match action.filter() {
            EventFilterBox::Time(TimeEventFilter(ExecutionTime::Schedule(schedule))) => {
                assert_eq!(schedule.start_ms, 1_735_000_000_000);
                assert_eq!(schedule.period_ms, Some(60_000));
            }
            other => panic!("unexpected filter: {other:?}"),
        }
        assert!(
            action
                .metadata()
                .contains(&"label".parse().expect("label key"))
        );
    }

    #[test]
    fn build_precommit_trigger_action_encodes_filter() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority_id = AccountId::new(keypair.public_key().clone());
        let encoded = build_precommit_trigger_action(
            account_json_literal(&authority_id),
            vec![
                "{\"Mint\":{\"TriggerRepetitions\":{\"object\":1,\"destination\":\"demo::trigger\"}}}"
                    .to_owned(),
            ],
            None,
            None,
        )
        .expect("pre-commit action");
        let bytes = STANDARD.decode(encoded).expect("base64");
        let archived = from_bytes::<Action>(&bytes).expect("decode action");
        let action = Action::try_deserialize(archived).expect("action value");
        assert_eq!(action.authority(), &authority_id);
        assert!(matches!(action.repeats(), Repeats::Indefinitely));
        match action.filter() {
            EventFilterBox::Time(TimeEventFilter(ExecutionTime::PreCommit)) => {}
            other => panic!("unexpected filter: {other:?}"),
        }
    }

    #[test]
    fn da_manifest_chunker_handle_binding_resolves_profile() {
        ensure_packed_struct_disabled();
        let fixture = build_da_manifest_fixture();
        let handle =
            da_manifest_chunker_handle(Buffer::from(fixture.manifest_bytes.clone()).into())
                .expect("chunker handle");
        assert_eq!(handle, "sorafs.sf1@1.0.0");
    }

    #[test]
    fn da_proof_summary_binding_verifies_payload() {
        ensure_packed_struct_disabled();
        let fixture = build_da_manifest_fixture();
        let summary = da_generate_proof_summary(
            Buffer::from(fixture.manifest_bytes.clone()),
            Buffer::from(fixture.payload.clone()),
            Some(JsDaProofOptions {
                sample_count: Some(3),
                sample_seed: Some(JsU64(99)),
                leaf_indexes: Some(vec![0, 1]),
            }),
        )
        .expect("proof summary");
        assert_eq!(summary.blob_hash_hex, hex::encode(fixture.blob_hash));
        assert_eq!(summary.chunk_root_hex, hex::encode(fixture.chunk_root));
        assert_eq!(summary.leaf_count.0, fixture.leaf_count as u64);
        assert_eq!(summary.sample_count, 3);
        assert!(!summary.proofs.is_empty());
        assert!(summary.proofs.iter().all(|proof| proof.verified));
    }

    #[test]
    fn taikai_cache_stats_conversion_populates_js_struct() {
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

        let js = JsTaikaiCacheStats::from(stats);
        assert_eq!(js.hits.hot.0, 7);
        assert_eq!(js.evictions.warm.capacity.0, 4);
        assert_eq!(js.promotions.cold_to_hot.0, 16);
        assert_eq!(js.qos_denials.standard.0, 18);
        assert_eq!(js.misses.0, 10);
    }

    #[test]
    fn taikai_queue_stats_conversion_populates_js_struct() {
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

        let js = JsTaikaiQueueStats::from(queue);
        assert_eq!(js.pending_segments.0, 2);
        assert_eq!(js.shaper_denials.bulk.0, 3);
        assert_eq!(js.hedged_batches.0, 6);
        assert_eq!(js.open_circuits.0, 9);
    }
}

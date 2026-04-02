//! Native bindings exposed to the JavaScript SDK.
#![allow(clippy::missing_errors_doc)]

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
    collections::HashSet,
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
use iroha_core::zk::{hash_vk as hash_verifying_key_box, test_utils::halo2_fixture_envelope};
use iroha_crypto::{
    Algorithm, Hash, HashOf, KeyPair, PrivateKey, PublicKey, Signature, derive_keyset_from_slice,
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature, encode_sm2_public_key_payload},
};
#[cfg(test)]
use iroha_data_model::da::types::DaRentQuote;
use iroha_data_model::{
    ChainId,
    account::{
        Account, AccountId, NewAccount,
        address::{AccountAddress, AccountAddressError},
    },
    asset::{
        definition::{AssetDefinition, NewAssetDefinition},
        id::{AssetDefinitionId, AssetId},
    },
    block::{
        BlockHeader,
        consensus::{LaneBlockCommitment, PERMISSIONED_TAG},
    },
    consensus::{CertPhase, Qc, QcAggregate},
    da::manifest::DaManifestV1,
    domain::{Domain, DomainId, NewDomain},
    events::time::{ExecutionTime, Schedule as TimeSchedule, TimeEventFilter},
    isi::{
        Burn, BurnBox, CreateKaigi, CustomInstruction, EndKaigi, ExecuteTrigger,
        Instruction as InstructionTrait, InstructionBox, JoinKaigi, LeaveKaigi, Mint, MintBox, RecordKaigiUsage, Register,
        RegisterBox, RegisterKaigiRelay, RegisterPeerWithPop, RemoveKeyValue,
        ReportKaigiRelayHealth, SetKaigiRelayManifest, SetKeyValue, Transfer, TransferBox,
        Unregister, UnregisterBox,
        governance::{
            CastPlainBallot, CastZkBallot, CouncilDerivationKind, EnactReferendum,
            FinalizeReferendum, PersistCouncilForEpoch, ProposeDeployContract, RegisterCitizen,
            VotingMode,
        },
        rwa::{
            ForceTransferRwa, FreezeRwa, HoldRwa, MergeRwas, RedeemRwa, RegisterRwa, ReleaseRwa,
            RwaInstructionBox, SetRwaControls, TransferRwa, UnfreezeRwa,
        },
        smart_contract_code::{
            ActivateContractInstance, DeactivateContractInstance, RegisterSmartContractBytes,
            RegisterSmartContractCode, RemoveSmartContractBytes,
        },
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
    name::Name,
    nexus::{
        AxtDescriptorBuilder, AxtTouchFragment, DataSpaceId, LaneId, LaneRelayEnvelope,
        TouchManifest, compute_descriptor_binding, compute_settlement_hash, validate_descriptor,
    },
    nft::{NewNft, Nft, NftId},
    oracle::KeyedHash,
    peer::{Peer, PeerId},
    role::{NewRole, Role, RoleId},
    rwa::{NewRwa, RwaControlPolicy, RwaId, RwaParentRef},
    smart_contract::manifest::ContractManifest,
    transaction::{
        Executable, PrivateCreateKaigi, PrivateEndKaigi, PrivateJoinKaigi, PrivateKaigiAction,
        PrivateKaigiArtifacts, PrivateKaigiFeeSpend, PrivateKaigiTemplate, PrivateKaigiTransaction,
        TransactionSubmissionReceipt,
        signed::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
    },
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_primitives::{
    json::Json,
    numeric::Numeric,
    soradns::{GatewayHostBindings, derive_gateway_hosts},
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
    codec::{Decode, DecodeAll, Encode},
    core::{self, DecodeFromSlice},
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
    AccountId::parse_encoded(input)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| {
            napi::Error::new(napi::Status::InvalidArg, format!("invalid {label}: {err}"))
        })
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
#[allow(clippy::unnecessary_wraps)]
pub fn ed25519_keypair(seed: Option<Uint8Array>) -> napi::Result<JsKeyPair> {
    let keypair = seed.map_or_else(
        || KeyPair::random_with_algorithm(Algorithm::Ed25519),
        |seed| {
            let bytes = seed.to_vec();
            KeyPair::from_seed(bytes, Algorithm::Ed25519)
        },
    );

    let (_, public_bytes) = keypair.public_key().to_bytes();
    let (_, private_bytes) = keypair.private_key().to_bytes();

    Ok(JsKeyPair {
        algorithm: "ed25519".to_owned(),
        public_key: Buffer::from(public_bytes.to_vec()),
        private_key: Buffer::from(private_bytes),
        distid: None,
    })
}

/// Derive an Ed25519 public key from a private key seed or keypair payload.
#[napi]
#[allow(clippy::needless_pass_by_value)] // napi-rs typed arrays require owned values at the boundary
pub fn ed25519_public_key_from_private(private_key: Uint8Array) -> napi::Result<Buffer> {
    let secret =
        PrivateKey::from_bytes(Algorithm::Ed25519, private_key.as_ref()).map_err(norito_to_napi)?;
    let keypair = KeyPair::from_private_key(secret).map_err(norito_to_napi)?;
    let (_, public_bytes) = keypair.public_key().to_bytes();
    Ok(Buffer::from(public_bytes.to_vec()))
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
    let mut rng = OsRng;
    let private = Sm2PrivateKey::random(distid.clone(), &mut rng).map_err(|err| {
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
        .map(|pk| pk.to_string())
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
    let encoded = core::to_bytes(&instruction).map_err(norito_to_napi)?;
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
    let validator_set = vec![PeerId::from(KeyPair::random().public_key().clone())];
    let qc = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: header.hash(),
        parent_state_root: Hash::new([0xBA; 4]),
        post_state_root: Hash::new([0xBB; 4]),
        height: header.height().get(),
        view: 1,
        epoch: 0,
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

#[allow(unsafe_code)]
fn decode_instruction_aligned(bytes: &[u8]) -> Result<InstructionBox, core::Error> {
    if let Ok(instruction) = decode_from_bytes::<InstructionBox>(bytes) {
        return Ok(instruction);
    }
    let view = core::from_bytes_view(bytes)?;
    let payload = view.as_bytes();
    let (instruction, used) = <InstructionBox as DecodeFromSlice>::decode_from_slice(payload)?;
    if used != payload.len() {
        return Err(core::Error::LengthMismatch);
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
    let multihash = public_key.to_string();
    let prefixed = public_key.to_prefixed_string();
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
                let namespace = parse_string_value(
                    required_value(&mut fields, "namespace", "ProposeDeployContract")?,
                    "ProposeDeployContract.namespace",
                )?;
                let contract_id = parse_string_value(
                    required_value(&mut fields, "contract_id", "ProposeDeployContract")?,
                    "ProposeDeployContract.contract_id",
                )?;
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
                let window = fields
                    .remove("window")
                    .map(|value| json::from_value(value).map_err(norito_to_napi))
                    .transpose()?;
                let mode =
                    parse_optional_voting_mode(fields.remove("mode"), "ProposeDeployContract")?;
                let manifest_provenance = fields
                    .remove("manifest_provenance")
                    .map(|value| json::from_value(value).map_err(norito_to_napi))
                    .transpose()?;
                let instruction = ProposeDeployContract {
                    namespace,
                    contract_id,
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
                let namespace = parse_string_value(
                    required_value(&mut fields, "namespace", "ActivateContractInstance")?,
                    "ActivateContractInstance.namespace",
                )?;
                let contract_id = parse_string_value(
                    required_value(&mut fields, "contract_id", "ActivateContractInstance")?,
                    "ActivateContractInstance.contract_id",
                )?;
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "ActivateContractInstance")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "ActivateContractInstance.code_hash")?;
                let instruction = ActivateContractInstance {
                    namespace,
                    contract_id,
                    code_hash,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }

            if let Some(json::Value::Object(mut fields)) = map.remove("DeactivateContractInstance")
            {
                let namespace = parse_string_value(
                    required_value(&mut fields, "namespace", "DeactivateContractInstance")?,
                    "DeactivateContractInstance.namespace",
                )?;
                let contract_id = parse_string_value(
                    required_value(&mut fields, "contract_id", "DeactivateContractInstance")?,
                    "DeactivateContractInstance.contract_id",
                )?;
                let reason = parse_optional_string_value(
                    fields.remove("reason"),
                    "DeactivateContractInstance.reason",
                )?;
                let instruction = DeactivateContractInstance {
                    namespace,
                    contract_id,
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

    if let Some(propose) = instruction_ref
        .as_any()
        .downcast_ref::<ProposeDeployContract>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "namespace".to_owned(),
            json::Value::String(propose.namespace.clone()),
        );
        inner.insert(
            "contract_id".to_owned(),
            json::Value::String(propose.contract_id.clone()),
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
            "namespace".to_owned(),
            json::Value::String(activate.namespace.clone()),
        );
        inner.insert(
            "contract_id".to_owned(),
            json::Value::String(activate.contract_id.clone()),
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
            "namespace".to_owned(),
            json::Value::String(deactivate.namespace.clone()),
        );
        inner.insert(
            "contract_id".to_owned(),
            json::Value::String(deactivate.contract_id.clone()),
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

fn decode_signed_transaction(bytes: &[u8]) -> napi::Result<SignedTransaction> {
    let mut cursor = bytes;
    SignedTransaction::decode(&mut cursor).map_err(norito_to_napi)
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

    let mut builder = TransactionBuilder::new(chain_id, authority).with_instructions(instructions);

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

    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, secret).map_err(norito_to_napi)?;
    let signed = builder.sign(&private_key);
    let signed_bytes = Encode::encode(&signed);
    let hash = Buffer::from(signed.hash().as_ref().to_vec());

    Ok(JsSignedTransaction {
        signed_transaction: Buffer::from(signed_bytes),
        hash,
    })
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
    let signed = builder.sign(&private_key);
    Ok(Buffer::from(Encode::encode(&signed)))
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
            format!("{context} must be in `domain:callName` format"),
        ));
    };
    let domain_id = DomainId::from_str(domain).map_err(|err| {
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
    let domain_id: DomainId = domain_id.parse().map_err(|err| {
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
    use std::{fs, io::Cursor, path::PathBuf, str::FromStr, sync::Arc};

    use base64::engine::general_purpose::STANDARD as BASE64;
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
        name::Name,
        nexus::LaneId,
        nft::NftId,
        peer::{Peer, PeerId},
        rwa::{NewRwa, RwaControlPolicy, RwaId, RwaParentRef},
        smart_contract::manifest::{AccessSetHints, ContractManifest},
        transaction::{
            Executable, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
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

    fn sample_hash(byte: u8) -> [u8; Hash::LENGTH] {
        let mut buf = [byte; Hash::LENGTH];
        buf[buf.len() - 1] |= 1;
        buf
    }

    fn sample_account(_domain: &str) -> AccountId {
        let keypair = KeyPair::random();
        AccountId::new(keypair.public_key().clone())
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
            domain.parse().expect("valid domain id"),
            Hash::prehashed(sample_hash(byte)),
        )
    }

    fn sample_kaigi_id(domain: &str, call_name: &str) -> KaigiId {
        let domain_id: DomainId = domain.parse().expect("valid domain id");
        let call = Name::from_str(call_name).expect("valid kaigi name");
        KaigiId::new(domain_id, call)
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
        let asset_definition: AssetDefinitionId =
            AssetDefinitionId::new("wonderland".parse().unwrap(), "rose".parse().unwrap());
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
        let asset_definition: AssetDefinitionId =
            AssetDefinitionId::new("wonderland".parse().unwrap(), "rose".parse().unwrap());
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
        let asset_definition: AssetDefinitionId =
            AssetDefinitionId::new("wonderland".parse().unwrap(), "rose".parse().unwrap());
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
            "commodities".parse().expect("valid domain id"),
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
        call_id.insert("domain_id".into(), Value::String("wonderland".into()));
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
        let domain_id: DomainId = "wonderland".parse().expect("valid domain id");

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
                        "domain_id": "wonderland",
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
            namespace: "apps".to_owned(),
            contract_id: "ledger".to_owned(),
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
    fn smart_contract_code_instruction_json_roundtrip() {
        let manifest = ContractManifest {
            code_hash: Some(Hash::prehashed(sample_hash(0xAA))),
            abi_hash: Some(Hash::prehashed(sample_hash(0xBB))),
            compiler_fingerprint: Some("rustc-1.79".to_owned()),
            features_bitmap: Some(42),
            access_set_hints: Some(AccessSetHints {
                read_keys: vec!["account:alice".to_owned()],
                write_keys: vec!["contract:foo".to_owned()],
            }),
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };
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
        let instruction: InstructionBox = Box::new(ActivateContractInstance {
            namespace: "apps".to_owned(),
            contract_id: "ledger".to_owned(),
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
                            "domain_id": "wonderland",
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

        let asset_definition: AssetDefinitionId =
            AssetDefinitionId::new("wonderland".parse().unwrap(), "rose".parse().unwrap());
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
    fn decode_transaction_receipt_json_roundtrip() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
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

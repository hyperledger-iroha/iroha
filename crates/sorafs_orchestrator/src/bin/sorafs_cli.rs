//! Aggregated CLI entry point for SoraFS packaging helpers.
#![allow(unexpected_cfgs)]
#[path = "sorafs_cli/pdp.rs"]
mod pdp;
use base64::{
    Engine,
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD,
    },
};
use blake3::hash as blake3_hash;
use ed25519_dalek::{Signer, SigningKey};
use hex::encode as hex_encode;
use iroha_config::parameters::defaults::streaming::soranet::PROVISION_SPOOL_DIR;
use iroha_crypto::{KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::AccountAddress},
    da::types::{BlobDigest, StorageTicketId},
    id::ChainId,
    isi::sorafs::RegisterPinManifest,
    metadata::Metadata,
    name::Name,
    sorafs::{
        moderation::{
            AdversarialCorpusManifestV1, MODERATION_MODEL_MAX_INPUT_BYTES_V1,
            ModerationCommitteeAggregateV1, ModerationModelScoreV1, ModerationReproManifestV1,
            ModerationSignedScreeningResultV1, ModerationThresholdsV1, ModerationTrustPolicyV1,
        },
        pin_registry::{
            ChunkerProfileHandle, ManifestAliasBinding, ManifestDigest, ManifestRootCid,
            PinManifestFinalizedRecordV1, PinPolicy as RegistryPinPolicy, PinStatus,
            StorageClass as RegistryStorageClass,
        },
    },
    taikai::{
        TaikaiAudioLayout, TaikaiCodec, TaikaiEventId, TaikaiRenditionId, TaikaiResolution,
        TaikaiStreamId, TaikaiTrackMetadata,
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_primitives::numeric::Quantity;
use iroha_version::codec::EncodeVersioned;
use ivm::kotodama::session::{CompileRequest, CompilerSession};
use norito::{
    decode_from_bytes,
    derive::{NoritoDeserialize, NoritoSerialize},
    json::{Map, Number, Value, from_slice, to_string_pretty, to_value, to_vec},
    to_bytes,
};
use reqwest::{
    StatusCode,
    blocking::Client as HttpClient,
    header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
    redirect::Policy as RedirectPolicy,
};
use sha3::{Digest, Sha3_256};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarStreamingWriter, CarVerifier, CarWriteError, ChunkFetchSpec,
    FileEntry, FilePlan, StoredChunk,
    chunker_registry::{self, ChunkerProfileDescriptor},
    compute_por_root,
    fetch_plan::{chunk_fetch_plan_from_json, chunk_fetch_plan_to_string},
    gateway::{GatewayFetchConfig, GatewayFetchContext, GatewayProviderInput},
    multi_fetch::{ProviderMetadata, RangeCapability, StreamBudget},
    policy::{PolicyEvidenceValidator, run_honey_probe},
    proof_stream::{
        ProofKind, ProofStreamItem, ProofStreamMetrics, ProofStreamVerificationContext, ProofTier,
    },
    proof_stream_transport::ProofStreamNdjsonReader,
    scoreboard::{Eligibility, TelemetrySnapshot},
    taikai::{BundleRequest, BundleSummary, bundle_segment, load_extra_metadata},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::por::{
    POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1, POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
    POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1, POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1,
    PorStatusCursorV1, decode_por_weekly_report_v1,
};
use sorafs_manifest::{
    ChunkingProfileV1, DagCodecId, GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1,
    GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
    GovernanceLogSignatureV1, GovernanceSignatureAlgorithm, MANIFEST_DAG_CODEC,
    MAX_MANIFEST_ENCODED_BYTES, MAX_PROOF_STREAM_SAMPLE_COUNT, ManifestBuildError, ManifestBuilder,
    ManifestV1, PinPolicy, PorChallengeOutcome, PorChallengeStatusV1, PorReportIsoWeek,
    PorWeeklyReportV1, ProofStreamHttpRequestV1, ProofStreamRequestV1, ReputationMerkleProofV1,
    ReputationSnapshotV1, StorageClass, ValidationOutcomeV1,
    chunker_registry as manifest_chunker_registry, decode_manifest_v1_canonical,
    governance_dag_block_cid_v1, validate_governance_dag_head_against_chain_v1,
    validate_governance_log_node_bytes,
};
use sorafs_orchestrator::{
    AnonymityPolicy, FetchSession, OrchestratorConfig, RolloutPhase, TransportPolicy,
    WriteModeHint,
    appeals::{
        AppealClass, AppealClassConfig, AppealDisbursementError, AppealDisbursementInput,
        AppealDisbursementPlan, AppealPricingConfig, AppealQuote, AppealQuoteInput,
        AppealSettlementBreakdown, AppealSettlementConfig, AppealSettlementError, AppealUrgency,
        AppealVerdict, parse_appeal_quantity_literal,
    },
    bindings::{
        config_from_json as orchestrator_config_from_json,
        config_to_json as orchestrator_config_to_json,
    },
    fetch_via_gateway,
    moderation_provenance::{ModerationProvenanceStoreError, ModerationProvenanceStoreV1},
    moderation_runner::{
        LoadedModerationRunnerV1, LoadedModerationSigningRunnerV1, ModerationInferenceV1,
        ModerationRunnerError,
    },
    proxy::{ProxyKaigiBridgeConfig, ProxyMode, ProxyNoritoBridgeConfig},
    taikai_cache::{TaikaiCacheConfig, TaikaiCacheStatsSnapshot, TaikaiPullQueueStats},
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
    env,
    fmt::Write as FmtWrite,
    fs::{self, File, Metadata as FsMetadata, OpenOptions},
    io::{self, BufReader, BufWriter, Cursor, Read, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::runtime::Runtime;
macro_rules! insert_value {
    ($map:ident[$key:literal] = $value:expr) => {
        $map.insert($key.into(), Value::from($value))
    };
}
macro_rules! insert_json {
    ($map:ident[$key:literal] = $value:expr) => {
        $map.insert($key.into(), $value)
    };
}
const SORAFS_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");
use url::{Url, form_urlencoded::Serializer};
const DEFAULT_CHUNKER_HANDLE: &str = "sorafs.sf1@1.0.0";
const CONTEXT_APPEAL_QUOTE: &str = "sorafs_cli appeal quote";
const CONTEXT_APPEAL_SETTLE: &str = "sorafs_cli appeal settle";
const CONTEXT_APPEAL_DISBURSE: &str = "sorafs_cli appeal disburse";
const MODERATION_LOCAL_RUNNER_POLICY_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local-runner.policy-digest.v1";
const MODERATION_LOCAL_RUNNER_EVIDENCE_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.local-runner.evidence-digest.v1";
const MODERATION_RUNNER_DEFAULT_LISTEN: &str = "127.0.0.1:9194";
const MODERATION_RUNNER_GRPC_DEFAULT_LISTEN: &str = "127.0.0.1:9199";
const MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const MODERATION_RUNNER_HARD_MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const MODERATION_TRUST_POLICY_MAX_BYTES: u64 = 1024 * 1024;
const MODERATION_SIGNED_RESULT_MAX_BYTES: u64 = 512 * 1024;
const MODERATION_AUTHENTICATED_AGGREGATE_MAX_BYTES: u64 = 1024 * 1024;
const MODERATION_SIGNING_KEY_MAX_BYTES: u64 = 64 * 1024;
const MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES: u32 = MODERATION_MODEL_MAX_INPUT_BYTES_V1;
const MODERATION_RUNNER_MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MODERATION_RUNNER_MAX_SUBJECT_BYTES: usize = 1024;
const MODERATION_RUNNER_MAX_NOTES_BYTES: usize = 4096;
const MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS: usize = 64;
const MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT: usize = 32;
const MODERATION_RUNNER_MAX_GRPC_RESPONSE_BYTES: usize = 1024 * 1024;
const MODERATION_RUNNER_MAX_GRPC_ENVELOPE_BYTES: usize = 8 * 1024;
const MODERATION_RUNNER_MAX_HEADER_BYTES: usize = 16 * 1024;
const MODERATION_CANARY_MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MODERATION_COMMITTEE_MAX_RESULTS: usize = 64;
const MODERATION_COMMITTEE_MAX_RESULT_BYTES: u64 = 128 * 1024;
const MODERATION_COMMITTEE_DEFAULT_LISTEN: &str = "127.0.0.1:9196";
const MODERATION_REGISTRY_DEFAULT_LISTEN: &str = "127.0.0.1:9198";
const REPUTATION_AUTH_PRIVATE_KEY_MAX_BYTES: u64 = 64 * 1024;
const REPUTATION_PROVIDER_ID_MAX_BYTES: usize = 256;
const REPUTATION_RESPONSE_MAX_BYTES: u64 = 4 * 1024 * 1024;
const REPUTATION_HEADER_ACCOUNT: &str = "X-Iroha-Account";
const REPUTATION_HEADER_SIGNATURE: &str = "X-Iroha-Signature";
const REPUTATION_HEADER_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
const REPUTATION_HEADER_NONCE: &str = "X-Iroha-Nonce";
fn parse_u32_arg(flag: &str, raw: &str, context: &str) -> Result<u32, String> {
    require_canonical_unsigned_decimal(flag, raw, context)?;
    raw.parse::<u32>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}
fn parse_u64_arg(flag: &str, raw: &str, context: &str) -> Result<u64, String> {
    require_canonical_unsigned_decimal(flag, raw, context)?;
    raw.parse::<u64>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}
fn parse_u16_arg(flag: &str, raw: &str, context: &str) -> Result<u16, String> {
    require_canonical_unsigned_decimal(flag, raw, context)?;
    raw.parse::<u16>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}
fn parse_i32_arg(flag: &str, raw: &str, context: &str) -> Result<i32, String> {
    require_canonical_signed_decimal(flag, raw, context)?;
    raw.parse::<i32>()
        .map_err(|err| format!("failed to parse `{flag}` for `{context}`: {err}"))
}
#[cfg(test)]
fn parse_decimal_arg(flag: &str, raw: &str, ctx: &str) -> Result<rust_decimal::Decimal, String> {
    require_canonical_decimal_token(flag, raw, ctx)?;
    let value = raw
        .parse::<rust_decimal::Decimal>()
        .map_err(|err| format!("failed to parse `{flag}` for `{ctx}`: {err}"))?;
    if value.to_string() != raw {
        return Err(format!(
            "failed to parse `{flag}` for `{ctx}`: value must be a canonical decimal"
        ));
    }
    Ok(value)
}
fn require_canonical_unsigned_decimal(flag: &str, raw: &str, context: &str) -> Result<(), String> {
    let digits = raw.as_bytes();
    if digits.is_empty()
        || !digits.iter().all(u8::is_ascii_digit)
        || (digits.len() > 1 && digits[0] == b'0')
    {
        return Err(format!(
            "failed to parse `{flag}` for `{context}`: value must be a canonical unsigned decimal integer"
        ));
    }
    Ok(())
}
fn require_canonical_signed_decimal(flag: &str, raw: &str, context: &str) -> Result<(), String> {
    if raw.is_empty() || raw.trim() != raw || raw.starts_with('+') {
        return Err(format!(
            "failed to parse `{flag}` for `{context}`: value must be a canonical signed decimal integer"
        ));
    }
    let negative = raw.starts_with('-');
    let digits = raw.strip_prefix('-').unwrap_or(raw).as_bytes();
    if digits.is_empty()
        || !digits.iter().all(u8::is_ascii_digit)
        || (digits.len() > 1 && digits[0] == b'0')
        || (negative && digits == b"0")
    {
        return Err(format!(
            "failed to parse `{flag}` for `{context}`: value must be a canonical signed decimal integer"
        ));
    }
    Ok(())
}
#[cfg(test)]
fn require_canonical_decimal_token(flag: &str, raw: &str, context: &str) -> Result<(), String> {
    if raw.is_empty() || raw.trim() != raw || raw.starts_with('+') {
        return Err(format!(
            "failed to parse `{flag}` for `{context}`: value must be a canonical decimal"
        ));
    }
    let body = raw.strip_prefix('-').unwrap_or(raw);
    let (integer, fractional) = match body.split_once('.') {
        Some((integer, fractional)) => {
            if body.matches('.').count() != 1 || fractional.is_empty() || fractional.ends_with('0')
            {
                return Err(format!(
                    "failed to parse `{flag}` for `{context}`: value must be a canonical decimal"
                ));
            }
            (integer, Some(fractional))
        }
        None => (body, None),
    };
    if integer.is_empty()
        || !integer.as_bytes().iter().all(u8::is_ascii_digit)
        || (integer.len() > 1 && integer.as_bytes()[0] == b'0')
        || fractional.is_some_and(|digits| !digits.as_bytes().iter().all(u8::is_ascii_digit))
        || (raw.starts_with('-') && integer == "0" && fractional.is_none())
    {
        return Err(format!(
            "failed to parse `{flag}` for `{context}`: value must be a canonical decimal"
        ));
    }
    Ok(())
}
fn infer_i105_network_prefix(raw: &str) -> Option<u16> {
    let trimmed = raw.trim();
    if trimmed.starts_with("sora") {
        return Some(753);
    }
    if trimmed.starts_with("test") {
        return Some(369);
    }
    if trimmed.starts_with("dev") {
        return Some(0);
    }
    trimmed
        .strip_prefix('n')
        .and_then(|digits| digits.parse::<u16>().ok())
}
fn parse_account_id_arg(flag: &str, raw: &str, context: &str) -> Result<AccountId, String> {
    let trimmed = raw.trim();
    if let Some(prefix) = infer_i105_network_prefix(trimmed)
        && let Ok(address) = AccountAddress::from_i105_for_discriminant(trimmed, Some(prefix))
    {
        return address.to_account_id().map_err(|err| {
            format!("failed to decode `{flag}` for `{context}` as account id: {err}")
        });
    }
    if let Ok(address) = AccountAddress::from_i105(trimmed) {
        return address.to_account_id().map_err(|err| {
            format!("failed to decode `{flag}` for `{context}` as account id: {err}")
        });
    }
    AccountId::parse_encoded(trimmed)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| format!("failed to parse `{flag}` for `{context}` as account id: {err}"))
}
fn parse_account_id_arg_with_prefix(
    flag: &str,
    raw: &str,
    context: &str,
    expected_prefix: Option<u16>,
) -> Result<AccountId, String> {
    let trimmed = raw.trim();
    let address = match expected_prefix.or_else(|| infer_i105_network_prefix(trimmed)) {
        Some(prefix) => AccountAddress::from_i105_for_discriminant(trimmed, Some(prefix)),
        None => AccountAddress::from_i105(trimmed),
    };
    if let Ok(address) = address {
        return address.to_account_id().map_err(|err| {
            format!("failed to decode `{flag}` for `{context}` as account id: {err}")
        });
    }
    if expected_prefix.is_none() {
        return parse_account_id_arg(flag, trimmed, context);
    }
    AccountId::parse_encoded(trimmed)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| format!("failed to parse `{flag}` for `{context}` as account id: {err}"))
}
fn authority_payload_literal(
    authority: &AccountId,
    network_prefix: Option<u16>,
) -> Result<String, String> {
    match network_prefix {
        Some(prefix) => authority
            .to_i105_for_discriminant(prefix)
            .map_err(|err| format!("failed to encode authority for payload: {err}")),
        None => authority
            .canonical_i105()
            .map_err(|err| format!("failed to encode authority for payload: {err}")),
    }
}
fn parse_appeal_verdict(value: &str) -> Result<AppealVerdict, String> {
    value.parse::<AppealVerdict>().map_err(|_| {
        format!(
            "unsupported `--outcome={value}` for `{CONTEXT_APPEAL_SETTLE}`; expected uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated"
        )
    })
}
fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        return Err(usage());
    };
    match first.as_str() {
        "car" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "pack" => car_pack(args.collect()),
                _ => Err(usage()),
            }
        }
        "manifest" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "build" => manifest_build(args.collect()),
                "submit" => manifest_submit(args.collect()),
                "proposal" => manifest_proposal(args.collect()),
                _ => Err(usage()),
            }
        }
        "norito" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "build" => norito_build(args.collect()),
                _ => Err(usage()),
            }
        }
        "deploy" => deploy(args.collect()),
        "fetch" => fetch_gateway(args.collect()),
        "pdp" => pdp::run(args.collect()),
        "proof" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "stream" => proof_stream(args.collect()),
                "verify" => proof_verify(args.collect()),
                _ => Err(usage()),
            }
        }
        "reputation" => {
            let Some(sub) = args.next() else {
                return Err(reputation_usage());
            };
            match sub.as_str() {
                "fetch" => reputation_fetch(args.collect()),
                "snapshot" => reputation_snapshot(args.collect()),
                "watch" => reputation_watch(args.collect()),
                "verify" => reputation_verify(args.collect()),
                _ => Err(reputation_usage()),
            }
        }
        "storage" => {
            let Some(sub) = args.next() else {
                return Err(usage());
            };
            match sub.as_str() {
                "prepare" => storage_prepare(args.collect()),
                _ => Err(usage()),
            }
        }
        "por" => {
            let Some(sub) = args.next() else {
                return Err(por_usage());
            };
            match sub.as_str() {
                "status" => por_status(args.collect()),
                "export" => por_export(args.collect()),
                "report" => por_report(args.collect()),
                _ => Err(por_usage()),
            }
        }
        "proxy" => {
            let Some(sub) = args.next() else {
                return Err(proxy_usage());
            };
            match sub.as_str() {
                "set-mode" => proxy_set_mode(args.collect()),
                _ => Err(proxy_usage()),
            }
        }
        "taikai" => {
            let Some(sub) = args.next() else {
                return Err(taikai_usage());
            };
            match sub.as_str() {
                "bundle" => taikai_bundle(args.collect()),
                _ => Err(taikai_usage()),
            }
        }
        "moderation" => {
            let Some(sub) = args.next() else {
                return Err(moderation_usage());
            };
            match sub.as_str() {
                "validate-repro" => moderation_validate_repro(args.collect()),
                "validate-corpus" => moderation_validate_corpus(args.collect()),
                "registry-serve" => moderation_registry_serve(args.collect()),
                "run-local" => moderation_run_local(args.collect()),
                "run-signed-local" => moderation_run_signed_local(args.collect()),
                "runner-serve" => moderation_runner_serve(args.collect()),
                "runner-signed-serve" => moderation_runner_signed_serve(args.collect()),
                "runner-grpc-serve" => moderation_runner_grpc_serve(args.collect()),
                "runner-bundle" => moderation_runner_bundle(args.collect()),
                "runner-canary" => moderation_runner_canary(args.collect()),
                "committee-run" => moderation_committee_run(args.collect()),
                "committee-authenticated-run" => {
                    moderation_committee_authenticated_run(args.collect())
                }
                "committee-serve" => moderation_committee_serve(args.collect()),
                "committee-authenticated-serve" => {
                    moderation_committee_authenticated_serve(args.collect())
                }
                "committee-bundle" => moderation_committee_bundle(args.collect()),
                "committee-canary" => moderation_committee_canary(args.collect()),
                "honey-audit" => moderation_honey_audit(args.collect()),
                _ => Err(moderation_usage()),
            }
        }
        "appeal" => {
            let Some(sub) = args.next() else {
                return Err(appeal_usage());
            };
            match sub.as_str() {
                "quote" => appeal_quote(args.collect()),
                "settle" => appeal_settle(args.collect()),
                "disburse" => appeal_disburse(args.collect()),
                _ => Err(appeal_usage()),
            }
        }
        "governance" => {
            let Some(sub) = args.next() else {
                return Err(governance_usage());
            };
            match sub.as_str() {
                "dag" => {
                    let Some(dag_sub) = args.next() else {
                        return Err(governance_usage());
                    };
                    match dag_sub.as_str() {
                        "list" => governance_dag_list(args.collect()),
                        "show" => governance_dag_show(args.collect()),
                        "verify" => governance_dag_verify(args.collect()),
                        "export" => governance_dag_export(args.collect()),
                        "build" => governance_dag_build(args.collect()),
                        "verify-build" => governance_dag_verify_build(args.collect()),
                        "rebuild-head" => governance_dag_rebuild_head(args.collect()),
                        "checkpoint" => governance_dag_checkpoint(args.collect()),
                        "checkpoint-verify" => governance_dag_checkpoint_verify(args.collect()),
                        "checkpoint-recover" => governance_dag_checkpoint_recover(args.collect()),
                        "mirror-build" => governance_dag_mirror_build(args.collect()),
                        "mirror-query" => governance_dag_mirror_query(args.collect()),
                        _ => Err(governance_usage()),
                    }
                }
                _ => Err(governance_usage()),
            }
        }
        "--help" | "-h" | "help" => Err(usage()),
        _ => Err(usage()),
    }
}
fn car_pack(raw_args: Vec<String>) -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut chunker_handle: Option<String> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut plan_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--input" => input = Some(PathBuf::from(value)),
            "--chunker-handle" => chunker_handle = Some(value.to_string()),
            "--car-out" => car_out = Some(PathBuf::from(value)),
            "--plan-out" => plan_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli car pack`"
                ));
            }
        }
    }
    let input_path = input
        .ok_or_else(|| "missing required `--input=PATH` for `sorafs_cli car pack`".to_string())?;
    let car_path = car_out
        .ok_or_else(|| "missing required `--car-out=PATH` for `sorafs_cli car pack`".to_string())?;
    let handle = chunker_handle
        .as_deref()
        .unwrap_or(DEFAULT_CHUNKER_HANDLE)
        .trim();
    let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
        format!("unknown chunker profile handle `{handle}`; see `sorafs_manifest_builder --list-chunker-profiles` for options")
    })?;
    let metadata =
        fs::metadata(&input_path).map_err(|err| format!("failed to stat input: {err}"))?;
    if metadata.is_dir() {
        build_from_directory(
            &input_path,
            descriptor,
            &car_path,
            plan_out.as_ref(),
            summary_out.as_ref(),
            handle,
        )
    } else if metadata.is_file() {
        build_from_file(
            &input_path,
            descriptor,
            &car_path,
            plan_out.as_ref(),
            summary_out.as_ref(),
            handle,
        )
    } else {
        Err(format!(
            "input `{}` is neither a regular file nor directory",
            input_path.display()
        ))
    }
}
struct DeployClientConfig {
    torii_url: Option<String>,
    public_key: PublicKey,
    private_key: PrivateKey,
    network_id: NetworkId,
    chain_discriminant: u16,
}
struct DeployPackArtifacts {
    manifest: ManifestV1,
    manifest_digest_hex: String,
    root_cid_hex: String,
    root_cid_base32: String,
    gateway_expectations: Vec<GatewayExpectation>,
    payload_kind: &'static str,
    payload_digest_hex: String,
}
#[derive(Clone, Debug)]
struct GatewayExpectation {
    path: Option<Vec<String>>,
    bytes: u64,
    blake3_hex: String,
}
struct PublishPeerDiscovery {
    gateway_base_url: Option<String>,
    status: Option<u16>,
    error: Option<String>,
}
struct ManifestRegisterSubmission {
    endpoint_requested: String,
    endpoint_used: String,
    status: StatusCode,
    response_bytes: Vec<u8>,
    response_value: Value,
    submission_mode: &'static str,
}
struct ManifestSubmitRequest<'a> {
    client: &'a HttpClient,
    torii_base_url: &'a Url,
    network_id: &'a NetworkId,
    authority: &'a AccountId,
    private_key: &'a PrivateKey,
    alias_inputs: Option<&'a AliasInputs>,
}
fn deploy(raw_args: Vec<String>) -> Result<(), String> {
    let mut payload_path: Option<PathBuf> = None;
    let mut client_config_path: Option<PathBuf> = None;
    let mut torii_url_override: Option<String> = None;
    let mut name: Option<String> = None;
    let mut out_dir_override: Option<PathBuf> = None;
    let mut gateway_base_url_override: Option<String> = None;
    let mut peer_discovery_enabled = true;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--no-peer-discovery" {
            peer_discovery_enabled = false;
            continue;
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--client-config" => client_config_path = Some(PathBuf::from(value)),
            "--torii-url" => torii_url_override = Some(value.to_string()),
            "--name" => name = Some(value.to_string()),
            "--out-dir" => out_dir_override = Some(PathBuf::from(value)),
            "--gateway-base-url" => gateway_base_url_override = Some(value.to_string()),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli deploy`"
                ));
            }
        }
    }
    let payload_path = payload_path
        .ok_or_else(|| "missing required `--payload=PATH` for `sorafs_cli deploy`".to_string())?;
    let client_config_path = client_config_path.ok_or_else(|| {
        "missing required `--client-config=PATH` for `sorafs_cli deploy`".to_string()
    })?;
    let client_config = load_deploy_client_config(&client_config_path)?;
    let torii_url = torii_url_override
        .or(client_config.torii_url.clone())
        .ok_or_else(|| {
            "`--torii-url=URL` is required when client config does not define top-level `torii_url`"
                .to_string()
        })?;
    let torii_base_url =
        Url::parse(&torii_url).map_err(|err| format!("invalid Torii URL `{torii_url}`: {err}"))?;
    let deploy_name = sanitize_deploy_name(
        name.as_deref()
            .or_else(|| payload_path.file_name().and_then(|name| name.to_str()))
            .unwrap_or("payload"),
    );
    let out_dir =
        out_dir_override.unwrap_or_else(|| PathBuf::from(".sorafs/deploy").join(&deploy_name));
    fs::create_dir_all(&out_dir).map_err(|err| {
        format!(
            "failed to create deploy directory `{}`: {err}",
            out_dir.display()
        )
    })?;
    let receipt_path = summary_out
        .clone()
        .unwrap_or_else(|| out_dir.join(format!("{deploy_name}.deploy.receipt.json")));
    let car_path = out_dir.join(format!("{deploy_name}.car"));
    let plan_path = out_dir.join(format!("{deploy_name}.plan.json"));
    let pack_summary_path = out_dir.join(format!("{deploy_name}.pack.json"));
    let manifest_path = out_dir.join(format!("{deploy_name}.manifest.to"));
    let manifest_json_path = out_dir.join(format!("{deploy_name}.manifest.json"));
    let register_response_path = out_dir.join(format!("{deploy_name}.pin-register.response.json"));
    let mut errors: Vec<String> = Vec::new();
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let artifacts = build_deploy_artifacts(
        &payload_path,
        &car_path,
        &plan_path,
        &pack_summary_path,
        &manifest_path,
        &manifest_json_path,
    );
    let mut receipt = Map::new();
    insert_value!(receipt["name"] = deploy_name.clone());
    insert_value!(receipt["payload_path"] = payload_path.display().to_string());
    insert_value!(receipt["client_config_path"] = client_config_path.display().to_string());
    insert_value!(receipt["torii_url"] = torii_url.clone());
    insert_value!(receipt["out_dir"] = out_dir.display().to_string());
    insert_value!(receipt["receipt_path"] = receipt_path.display().to_string());
    let artifacts = match artifacts {
        Ok(artifacts) => artifacts,
        Err(err) => {
            errors.push(err);
            insert_value!(receipt["success"] = false);
            insert_json!(
                receipt["errors"] = Value::Array(errors.iter().cloned().map(Value::from).collect())
            );
            write_deploy_receipt_and_stdout(&receipt_path, &Value::Object(receipt))?;
            return Err("deploy failed while building local artifacts".to_string());
        }
    };
    insert_value!(receipt["cid_hex"] = artifacts.root_cid_hex.clone());
    insert_value!(receipt["cid_base32"] = artifacts.root_cid_base32.clone());
    insert_value!(receipt["manifest_digest_hex"] = artifacts.manifest_digest_hex.clone());
    insert_value!(receipt["payload_digest_blake3_hex"] = artifacts.payload_digest_hex.clone());
    insert_value!(receipt["payload_bytes"] = artifacts.manifest.content_length);
    insert_value!(receipt["payload_kind"] = artifacts.payload_kind);
    let mut artifact_paths = Map::new();
    insert_value!(artifact_paths["car"] = car_path.display().to_string());
    insert_value!(artifact_paths["plan_json"] = plan_path.display().to_string());
    insert_value!(artifact_paths["pack_summary_json"] = pack_summary_path.display().to_string());
    insert_value!(artifact_paths["manifest_to"] = manifest_path.display().to_string());
    insert_value!(artifact_paths["manifest_json"] = manifest_json_path.display().to_string());
    insert_value!(
        artifact_paths["pin_register_response"] = register_response_path.display().to_string()
    );
    insert_json!(receipt["artifacts"] = Value::Object(artifact_paths));
    let authority = AccountId::new(client_config.public_key.clone());
    let authority_literal =
        authority_payload_literal(&authority, Some(client_config.chain_discriminant))?;
    let submit_request = ManifestSubmitRequest {
        client: &client,
        torii_base_url: &torii_base_url,
        network_id: &client_config.network_id,
        authority: &authority,
        private_key: &client_config.private_key,
        alias_inputs: None,
    };
    let registration = submit_pin_register(&submit_request, &artifacts.manifest, None);
    let mut registration_summary = Map::new();
    insert_value!(registration_summary["authority"] = authority_literal);
    insert_value!(
        registration_summary["response_path"] = register_response_path.display().to_string()
    );
    let paid_pin_fee = match registration {
        Ok(response) => {
            write_bytes(&register_response_path, &response.response_bytes)?;
            let registration_ok = response.status.is_success();
            insert_value!(registration_summary["status"] = response.status.as_u16() as u64);
            insert_value!(registration_summary["endpoint"] = response.endpoint_used);
            insert_value!(registration_summary["endpoint_requested"] = response.endpoint_requested);
            insert_value!(registration_summary["submission_mode"] = response.submission_mode);
            insert_value!(registration_summary["success"] = registration_ok);
            let paid_pin_fee = paid_pin_fee_from_register_response(&response.response_value);
            if !registration_ok {
                let body = String::from_utf8_lossy(&response.response_bytes);
                errors.push(format!(
                    "paid pin registration failed with status {}: {body}",
                    response.status
                ));
            }
            paid_pin_fee
        }
        Err(err) => {
            insert_value!(registration_summary["success"] = false);
            insert_value!(registration_summary["error"] = err.clone());
            errors.push(err);
            Value::Null
        }
    };
    insert_json!(receipt["registration"] = Value::Object(registration_summary));
    insert_json!(receipt["paid_pin_fee"] = paid_pin_fee);
    let discovery = if peer_discovery_enabled {
        discover_publish_peers(&client, &torii_base_url)
    } else {
        PublishPeerDiscovery {
            gateway_base_url: None,
            status: None,
            error: Some("peer discovery disabled by --no-peer-discovery".to_string()),
        }
    };
    let gateway_base_url = gateway_base_url_override
        .or(discovery.gateway_base_url.clone())
        .unwrap_or_else(|| torii_url.clone());
    let mut discovery_json = Map::new();
    insert_value!(discovery_json["enabled"] = peer_discovery_enabled);
    insert_json!(
        discovery_json["gateway_base_url"] = discovery
            .gateway_base_url
            .as_ref()
            .map_or(Value::Null, |url| Value::from(url.clone()))
    );
    if let Some(status) = discovery.status {
        insert_value!(discovery_json["status"] = status as u64);
    }
    if let Some(err) = discovery.error.as_ref() {
        insert_value!(discovery_json["warning"] = err.clone());
    }
    insert_json!(receipt["peer_discovery"] = Value::Object(discovery_json));
    insert_json!(
        receipt["provider_ingest"] = Value::Object(Map::from_iter([
            (
                "state".into(),
                Value::from("awaiting_finalized_provider_assignment"),
            ),
            ("queued".into(), Value::from(false)),
            ("direct_http_ingest".into(), Value::from(false)),
        ]))
    );
    let gateway_verification = verify_gateway_deploy(
        &client,
        &gateway_base_url,
        &artifacts.root_cid_base32,
        &artifacts.gateway_expectations,
    );
    let gateway_success = gateway_verification
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let cid_url = gateway_url_for_cid(&gateway_base_url, &artifacts.root_cid_base32)?;
    insert_value!(receipt["gateway_base_url"] = gateway_base_url);
    insert_value!(receipt["cid_base32_url"] = cid_url);
    insert_json!(receipt["gateway_verification"] = gateway_verification);
    if !gateway_success {
        errors.push("gateway verification failed".to_string());
    }
    let success = errors.is_empty();
    insert_value!(receipt["success"] = success);
    insert_json!(
        receipt["errors"] = Value::Array(errors.iter().cloned().map(Value::from).collect())
    );
    write_deploy_receipt_and_stdout(&receipt_path, &Value::Object(receipt))?;
    if success {
        Ok(())
    } else {
        Err("deploy failed; see receipt for details".to_string())
    }
}
fn load_deploy_client_config(path: &Path) -> Result<DeployClientConfig, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read client config `{}`: {err}", path.display()))?;
    let root: toml::Table = raw.parse().map_err(|err| {
        format!(
            "failed to parse client config TOML `{}`: {err}",
            path.display()
        )
    })?;
    let torii_url = root
        .get("torii_url")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let account = root
        .get("account")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "client config must contain an `[account]` table".to_string())?;
    let public_key_raw = account
        .get("public_key")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "client config `[account]` must define `public_key`".to_string())?;
    let private_key_raw = account
        .get("private_key")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "client config `[account]` must define `private_key`".to_string())?;
    let chain_discriminant = resolve_deploy_chain_discriminant(&root, account)?;
    let _display_chain_id = resolve_deploy_chain_id(&root, chain_discriminant)?;
    let network_id = root
        .get("network_id")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "client config must define exact `network_id`".to_string())?
        .parse()
        .map_err(|err| format!("failed to parse client config network_id: {err}"))?;
    let public_key = PublicKey::from_str(public_key_raw)
        .map_err(|err| format!("failed to parse client config public_key: {err}"))?;
    let private_key = parse_private_key_inline(private_key_raw)
        .map_err(|err| format!("failed to parse client config private_key: {err}"))?;
    Ok(DeployClientConfig {
        torii_url,
        public_key,
        private_key,
        network_id,
        chain_discriminant,
    })
}
fn resolve_deploy_chain_id(root: &toml::Table, chain_discriminant: u16) -> Result<ChainId, String> {
    let literal = root
        .get("chain")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| known_chain_id_for_discriminant(chain_discriminant).map(str::to_owned))
        .ok_or_else(|| {
            format!(
                "client config must define top-level `chain` for network discriminant \
                 {chain_discriminant}"
            )
        })?;
    literal
        .parse()
        .map_err(|err| format!("failed to parse client config chain `{literal}`: {err}"))
}
fn known_chain_id_for_discriminant(chain_discriminant: u16) -> Option<&'static str> {
    match chain_discriminant {
        369 => Some("fc56984b-2be7-431d-840e-21514d1883f0"),
        753 => Some("00000000-0000-0000-0000-000000000753"),
        discriminant
            if discriminant == iroha_config::parameters::defaults::common::chain_discriminant() =>
        {
            Some("00000000-0000-0000-0000-000000000000")
        }
        _ => None,
    }
}
fn resolve_deploy_chain_discriminant(
    root: &toml::Table,
    account: &toml::Table,
) -> Result<u16, String> {
    if let Some(value) = account.get("chain_discriminant") {
        let value = value.as_integer().ok_or_else(|| {
            "client config `[account].chain_discriminant` must be an integer".to_string()
        })?;
        return u16::try_from(value).map_err(|_| {
            "client config `[account].chain_discriminant` must fit in u16".to_string()
        });
    }
    let chain = root
        .get("chain")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            "client config must define integer `[account].chain_discriminant` or a known top-level `chain`"
                .to_string()
        })?;
    known_deploy_chain_discriminant(chain).ok_or_else(|| {
        format!(
            "client config top-level `chain` `{chain}` is not known; define integer `[account].chain_discriminant`"
        )
    })
}
fn known_deploy_chain_discriminant(chain: &str) -> Option<u16> {
    match chain.trim() {
        "fc56984b-2be7-431d-840e-21514d1883f0" => Some(369),
        "iroha3-nexus" | "00000000-0000-0000-0000-000000000753" => Some(753),
        "00000000-0000-0000-0000-000000000000" => {
            Some(iroha_config::parameters::defaults::common::chain_discriminant())
        }
        _ => None,
    }
}
fn build_deploy_artifacts(
    payload_path: &Path,
    car_path: &Path,
    plan_path: &Path,
    pack_summary_path: &Path,
    manifest_path: &Path,
    manifest_json_path: &Path,
) -> Result<DeployPackArtifacts, String> {
    let handle = DEFAULT_CHUNKER_HANDLE;
    let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
        format!("unknown chunker profile handle `{handle}`; refresh the SoraFS chunker registry")
    })?;
    let metadata = fs::metadata(payload_path)
        .map_err(|err| format!("failed to stat payload `{}`: {err}", payload_path.display()))?;
    let (input_summary, mut plan, payload_cursor): (InputSummary, CarBuildPlan, Cursor<Vec<u8>>) =
        if metadata.is_dir() {
            let (plan, payload) =
                CarBuildPlan::from_directory_with_profile(payload_path, descriptor.profile)
                    .map_err(|err| format!("failed to build directory payload plan: {err}"))?;
            (
                InputSummary::Directory {
                    path: payload_path.to_path_buf(),
                    file_count: plan.files.len() as u64,
                },
                plan,
                Cursor::new(payload),
            )
        } else if metadata.is_file() {
            let payload = fs::read(payload_path).map_err(|err| {
                format!("failed to read payload `{}`: {err}", payload_path.display())
            })?;
            let plan = CarBuildPlan::single_file_with_profile(&payload, descriptor.profile)
                .map_err(|err| format!("failed to chunk payload: {err}"))?;
            (
                InputSummary::File {
                    path: payload_path.to_path_buf(),
                    bytes: payload.len() as u64,
                },
                plan,
                Cursor::new(payload),
            )
        } else {
            return Err(format!(
                "payload `{}` is neither a regular file nor directory",
                payload_path.display()
            ));
        };
    let car_file = open_output_file(car_path)?;
    let mut writer = BufWriter::new(car_file);
    let mut payload_reader = payload_cursor;
    let por_root = compute_por_root(payload_reader.get_ref(), &plan)
        .map_err(|err| format!("failed to derive payload PoR root: {err}"))?;
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload_reader, &mut writer)
        .map_err(format_car_error)?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush `{}`: {err}", car_path.display()))?;
    let plan_specs = plan
        .try_chunk_fetch_specs()
        .map_err(|err| format!("failed to derive chunk fetch plan: {err}"))?;
    let plan_json = chunk_fetch_plan_to_string(&plan)
        .map_err(|err| format!("failed to render chunk plan JSON: {err}"))?;
    write_text(plan_path, plan_json.as_bytes())?;
    let pack_summary = render_summary(
        &input_summary,
        descriptor,
        handle,
        &plan,
        &stats,
        por_root,
        car_path,
    )?;
    let pack_rendered = to_string_pretty(&pack_summary)
        .map_err(|err| format!("failed to render pack summary JSON: {err}"))?;
    write_text(pack_summary_path, pack_rendered.as_bytes())?;
    let root_cid = stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| "CAR build did not emit a root CID".to_string())?;
    let car_digest: [u8; 32] = *stats.car_archive_digest.as_bytes();
    let chunk_digest_sha3 = chunk_digest_sha3_from_specs(&plan_specs);
    let manifest_descriptor =
        manifest_chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
            format!("unknown manifest chunker profile handle `{handle}`; refresh the registry")
        })?;
    let chunking_profile = ChunkingProfileV1::from_descriptor(manifest_descriptor);
    let manifest = ManifestBuilder::new()
        .root_cid(root_cid.clone())
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_profile(chunking_profile)
        .chunk_digest_sha3_256(chunk_digest_sha3)
        .por_root(por_root)
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 86_400,
        })
        .build()
        .map_err(format_manifest_error)?;
    let manifest_bytes = manifest
        .encode()
        .map_err(|err| format!("failed to encode manifest: {err}"))?;
    write_bytes(manifest_path, &manifest_bytes)?;
    let manifest_json = to_string_pretty(
        &to_value(&manifest).map_err(|err| format!("failed to serialise manifest JSON: {err}"))?,
    )
    .map_err(|err| format!("failed to render manifest JSON: {err}"))?;
    write_text(manifest_json_path, manifest_json.as_bytes())?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let (payload_bytes, storage_files, payload_kind) =
        load_storage_pin_payload(payload_path, &manifest)?;
    let gateway_expectations =
        build_gateway_expectations(payload_path, storage_files.as_deref(), &payload_bytes)?;
    let payload_digest_hex = hex_encode(blake3_hash(&payload_bytes).as_bytes());
    let root_cid_hex = hex_encode(&root_cid);
    let root_cid_base32 = encode_content_cid_base32(&root_cid);
    plan.chunks.shrink_to_fit();
    Ok(DeployPackArtifacts {
        manifest,
        manifest_digest_hex: hex_encode(manifest_digest.as_bytes()),
        root_cid_hex,
        root_cid_base32,
        gateway_expectations,
        payload_kind,
        payload_digest_hex,
    })
}
fn build_gateway_expectations(
    payload_path: &Path,
    files: Option<&[StorageFileEntryOwned]>,
    payload_bytes: &[u8],
) -> Result<Vec<GatewayExpectation>, String> {
    if let Some(entries) = files {
        let mut expectations = Vec::new();
        if let Some(index) = entries
            .iter()
            .find(|entry| entry.path.len() == 1 && entry.path[0] == "index.html")
        {
            expectations.push(read_gateway_expectation(payload_path, None, index)?);
        }
        let mut ordered = entries.to_vec();
        ordered.sort_by(|left, right| left.path.cmp(&right.path));
        for entry in ordered.iter().take(32) {
            expectations.push(read_gateway_expectation(
                payload_path,
                Some(entry.path.clone()),
                entry,
            )?);
        }
        return Ok(expectations);
    }
    Ok(vec![GatewayExpectation {
        path: None,
        bytes: payload_bytes.len() as u64,
        blake3_hex: hex_encode(blake3_hash(payload_bytes).as_bytes()),
    }])
}
fn read_gateway_expectation(
    root: &Path,
    gateway_path: Option<Vec<String>>,
    entry: &StorageFileEntryOwned,
) -> Result<GatewayExpectation, String> {
    let file_path = entry
        .path
        .iter()
        .fold(root.to_path_buf(), |acc, component| acc.join(component));
    let bytes = fs::read(&file_path).map_err(|err| {
        format!(
            "failed to read gateway verification file `{}`: {err}",
            file_path.display()
        )
    })?;
    Ok(GatewayExpectation {
        path: gateway_path,
        bytes: entry.size,
        blake3_hex: hex_encode(blake3_hash(&bytes).as_bytes()),
    })
}
fn submit_pin_register(
    request: &ManifestSubmitRequest<'_>,
    manifest: &ManifestV1,
    successor_digest: Option<[u8; 32]>,
) -> Result<ManifestRegisterSubmission, String> {
    let endpoint = request
        .torii_base_url
        .join("v1/sorafs/pin/register")
        .map_err(|err| format!("failed to build Torii pin-register endpoint URL: {err}"))?;
    let requested_endpoint = endpoint.as_str().to_string();
    let transaction = build_pin_register_transaction(
        request.network_id,
        request.authority,
        request.private_key,
        manifest,
        request.alias_inputs,
        successor_digest,
    )?;
    let body_bytes = transaction.encode_versioned();
    let response = request
        .client
        .post(endpoint.as_str())
        .header(CONTENT_TYPE, "application/x-norito")
        .header("Accept", "application/json")
        .body(body_bytes)
        .send()
        .map_err(|err| format!("failed to submit manifest to Torii: {err}"))?;
    let status = response.status();
    let response_bytes = response
        .bytes()
        .map_err(|err| format!("failed to read Torii response: {err}"))?
        .to_vec();
    if status.is_success() {
        return Ok(ManifestRegisterSubmission {
            endpoint_requested: requested_endpoint.clone(),
            endpoint_used: requested_endpoint,
            status,
            response_value: decode_response_value_or_text(&response_bytes),
            response_bytes,
            submission_mode: "pin_register_http",
        });
    }
    let body_text = String::from_utf8_lossy(&response_bytes);
    Err(format!(
        "Torii pin-register route returned {status}; generic transaction fallback is not supported: {body_text}"
    ))
}
fn discover_publish_peers(client: &HttpClient, torii_base_url: &Url) -> PublishPeerDiscovery {
    let endpoint = match torii_base_url.join("v1/sorafs/storage/peers") {
        Ok(endpoint) => endpoint,
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                status: None,
                error: Some(format!(
                    "failed to build peer discovery endpoint URL: {err}"
                )),
            };
        }
    };
    let response = match client
        .get(endpoint.as_str())
        .header("Accept", "application/json")
        .send()
    {
        Ok(response) => response,
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                status: None,
                error: Some(format!("peer discovery unavailable: {err}")),
            };
        }
    };
    let status = response.status();
    let response_bytes = match response.bytes() {
        Ok(bytes) => bytes.to_vec(),
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                status: Some(status.as_u16()),
                error: Some(format!("failed to read peer discovery response: {err}")),
            };
        }
    };
    if !status.is_success() {
        return PublishPeerDiscovery {
            gateway_base_url: None,
            status: Some(status.as_u16()),
            error: Some(format!(
                "peer discovery returned {status}; falling back to primary Torii URL"
            )),
        };
    }
    let value: Value = match from_slice(&response_bytes) {
        Ok(value) => value,
        Err(err) => {
            return PublishPeerDiscovery {
                gateway_base_url: None,
                status: Some(status.as_u16()),
                error: Some(format!("failed to parse peer discovery JSON: {err}")),
            };
        }
    };
    let gateway_base_url = value
        .get("gateway_base_url")
        .and_then(Value::as_str)
        .map(str::to_owned);
    PublishPeerDiscovery {
        gateway_base_url,
        status: Some(status.as_u16()),
        error: None,
    }
}
fn verify_gateway_deploy(
    client: &HttpClient,
    gateway_base_url: &str,
    cid_base32: &str,
    expectations: &[GatewayExpectation],
) -> Value {
    let root_url = match gateway_url_for_cid(gateway_base_url, cid_base32) {
        Ok(url) => url,
        Err(err) => {
            return Value::Object(Map::from_iter([
                ("success".into(), Value::from(false)),
                (
                    "gateway_base_url".into(),
                    Value::from(gateway_base_url.to_string()),
                ),
                ("error".into(), Value::from(err)),
            ]));
        }
    };
    let mut checks = Vec::new();
    for expectation in expectations {
        let (url, label) = match expectation.path.as_ref() {
            Some(path) => match gateway_url_for_file(gateway_base_url, cid_base32, path) {
                Ok(url) => (url, path.join("/")),
                Err(err) => {
                    checks.push(Value::Object(Map::from_iter([
                        ("success".into(), Value::from(false)),
                        ("path".into(), Value::from(path.join("/"))),
                        ("error".into(), Value::from(err)),
                    ])));
                    continue;
                }
            },
            None => (root_url.clone(), "/".to_string()),
        };
        checks.push(fetch_gateway_check(client, &url, &label, expectation));
    }
    if checks.is_empty() {
        checks.push(Value::Object(Map::from_iter([
            ("success".into(), Value::from(false)),
            (
                "error".into(),
                Value::from("no gateway verification expectations were generated"),
            ),
        ])));
    }
    let success = checks
        .iter()
        .all(|check| check.get("success").and_then(Value::as_bool) == Some(true));
    let mut map = Map::new();
    insert_value!(map["success"] = success);
    insert_value!(map["gateway_base_url"] = gateway_base_url.to_string());
    insert_value!(map["root_url"] = root_url);
    insert_json!(map["checks"] = Value::Array(checks));
    Value::Object(map)
}
fn fetch_gateway_check(
    client: &HttpClient,
    url: &str,
    label: &str,
    expectation: &GatewayExpectation,
) -> Value {
    let mut map = Map::new();
    insert_value!(map["path"] = label.to_string());
    insert_value!(map["url"] = url.to_string());
    insert_value!(map["expected_bytes"] = expectation.bytes);
    insert_value!(map["expected_blake3_hex"] = expectation.blake3_hex.clone());
    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            let bytes = match response.bytes() {
                Ok(bytes) => bytes,
                Err(err) => {
                    insert_value!(map["status"] = status.as_u16() as u64);
                    insert_value!(map["success"] = false);
                    insert_value!(map["error"] = format!("failed to read gateway response: {err}"));
                    return Value::Object(map);
                }
            };
            let actual = bytes.len() as u64;
            let actual_hash = hex_encode(blake3_hash(&bytes).as_bytes());
            let length_ok = expectation.bytes == actual;
            let hash_ok = expectation.blake3_hex == actual_hash;
            insert_value!(map["status"] = status.as_u16() as u64);
            insert_value!(map["actual_bytes"] = actual);
            insert_value!(map["actual_blake3_hex"] = actual_hash);
            insert_value!(map["length_ok"] = length_ok);
            insert_value!(map["hash_ok"] = hash_ok);
            insert_value!(map["success"] = status == StatusCode::OK && length_ok && hash_ok);
        }
        Err(err) => {
            insert_value!(map["success"] = false);
            insert_value!(map["error"] = err.to_string());
        }
    }
    Value::Object(map)
}
fn gateway_url_for_cid(gateway_base_url: &str, cid_base32: &str) -> Result<String, String> {
    let base = Url::parse(gateway_base_url)
        .map_err(|err| format!("invalid gateway base URL `{gateway_base_url}`: {err}"))?;
    Ok(base
        .join(&format!("sorafs/cid/{cid_base32}"))
        .map_err(|err| format!("failed to build gateway CID URL: {err}"))?
        .to_string())
}
fn gateway_url_for_file(
    gateway_base_url: &str,
    cid_base32: &str,
    path: &[String],
) -> Result<String, String> {
    let root = gateway_url_for_cid(gateway_base_url, cid_base32)?;
    let mut url = Url::parse(&root)
        .map_err(|err| format!("failed to parse gateway CID root URL `{root}`: {err}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "gateway URL cannot be a base for path segments".to_string())?;
        for component in path {
            segments.push(component);
        }
    }
    Ok(url.to_string())
}
fn paid_pin_fee_from_register_response(response: &Value) -> Value {
    let mut map = Map::new();
    if let Some(value) = response.get("pin_fee_nano") {
        insert_json!(map["pin_fee_nano"] = value.clone());
    }
    if let Some(value) = response.get("pin_fee_asset_id") {
        insert_json!(map["pin_fee_asset_id"] = value.clone());
    }
    if let Some(value) = response.get("pin_fee_treasury_account_id") {
        insert_json!(map["pin_fee_treasury_account_id"] = value.clone());
    }
    if map.is_empty() {
        Value::Null
    } else {
        Value::Object(map)
    }
}
fn write_deploy_receipt_and_stdout(path: &Path, value: &Value) -> Result<(), String> {
    let rendered =
        to_string_pretty(value).map_err(|err| format!("failed to render deploy receipt: {err}"))?;
    println!("{rendered}");
    write_text(path, rendered.as_bytes())
}
fn sanitize_deploy_name(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches(['.', '-']).trim();
    if trimmed.is_empty() {
        "payload".to_string()
    } else {
        trimmed.to_string()
    }
}
fn encode_content_cid_base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if bytes.is_empty() {
        return "b".to_string();
    }
    let mut acc = 0_u32;
    let mut bits = 0_u32;
    let mut out = Vec::with_capacity((bytes.len() * 8).div_ceil(5) + 1);
    out.push(b'b');
    for byte in bytes {
        acc = (acc << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1f) as usize;
            out.push(ALPHABET[index]);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[index]);
    }
    String::from_utf8(out).expect("CID base32 alphabet is valid UTF-8")
}
fn taikai_bundle(raw_args: Vec<String>) -> Result<(), String> {
    let mut payload: Option<PathBuf> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut envelope_out: Option<PathBuf> = None;
    let mut indexes_out: Option<PathBuf> = None;
    let mut ingest_metadata_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut event_id: Option<String> = None;
    let mut stream_id: Option<String> = None;
    let mut rendition_id: Option<String> = None;
    let mut track_kind: Option<String> = None;
    let mut codec: Option<String> = None;
    let mut bitrate_kbps: Option<u32> = None;
    let mut resolution: Option<String> = None;
    let mut audio_layout: Option<String> = None;
    let mut segment_sequence: Option<u64> = None;
    let mut segment_start_pts: Option<u64> = None;
    let mut segment_duration: Option<u32> = None;
    let mut wallclock_unix_ms: Option<u64> = None;
    let mut manifest_hash_hex: Option<String> = None;
    let mut storage_ticket_hex: Option<String> = None;
    let mut ingest_latency_ms: Option<u32> = None;
    let mut live_edge_drift_ms: Option<i32> = None;
    let mut ingest_node_id: Option<String> = None;
    let mut metadata_json: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(taikai_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--payload" => payload = Some(PathBuf::from(value)),
            "--car-out" => car_out = Some(PathBuf::from(value)),
            "--envelope-out" => envelope_out = Some(PathBuf::from(value)),
            "--indexes-out" => indexes_out = Some(PathBuf::from(value)),
            "--ingest-metadata-out" => ingest_metadata_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--event-id" => event_id = Some(value.to_string()),
            "--stream-id" => stream_id = Some(value.to_string()),
            "--rendition-id" => rendition_id = Some(value.to_string()),
            "--track-kind" => track_kind = Some(value.to_string()),
            "--codec" => codec = Some(value.to_string()),
            "--bitrate-kbps" => {
                let parsed = parse_u32_arg("--bitrate-kbps", value, "sorafs_cli taikai bundle")?;
                if parsed == 0 {
                    return Err(
                        "`--bitrate-kbps` for `sorafs_cli taikai bundle` must be greater than zero"
                            .to_string(),
                    );
                }
                bitrate_kbps = Some(parsed);
            }
            "--resolution" => resolution = Some(value.to_string()),
            "--audio-layout" => audio_layout = Some(value.to_string()),
            "--segment-sequence" => {
                segment_sequence = Some(parse_u64_arg(
                    "--segment-sequence",
                    value,
                    "sorafs_cli taikai bundle",
                )?);
            }
            "--segment-start-pts" => {
                segment_start_pts = Some(parse_u64_arg(
                    "--segment-start-pts",
                    value,
                    "sorafs_cli taikai bundle",
                )?);
            }
            "--segment-duration" => {
                let parsed =
                    parse_u32_arg("--segment-duration", value, "sorafs_cli taikai bundle")?;
                if parsed == 0 {
                    return Err(
                        "`--segment-duration` for `sorafs_cli taikai bundle` must be greater than zero"
                            .to_string(),
                    );
                }
                segment_duration = Some(parsed);
            }
            "--wallclock-unix-ms" => {
                wallclock_unix_ms = Some(parse_u64_arg(
                    "--wallclock-unix-ms",
                    value,
                    "sorafs_cli taikai bundle",
                )?);
            }
            "--manifest-hash" => manifest_hash_hex = Some(value.to_string()),
            "--storage-ticket" => storage_ticket_hex = Some(value.to_string()),
            "--ingest-latency-ms" => {
                ingest_latency_ms = Some(parse_u32_arg(
                    "--ingest-latency-ms",
                    value,
                    "sorafs_cli taikai bundle",
                )?)
            }
            "--live-edge-drift-ms" => {
                live_edge_drift_ms = Some(parse_i32_arg(
                    "--live-edge-drift-ms",
                    value,
                    "sorafs_cli taikai bundle",
                )?);
            }
            "--ingest-node-id" => ingest_node_id = Some(value.to_string()),
            "--metadata-json" => metadata_json = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli taikai bundle`"
                ));
            }
        }
    }
    let payload_path = payload.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let car_path = car_out.ok_or_else(|| {
        "missing required `--car-out=PATH` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let envelope_path = envelope_out.ok_or_else(|| {
        "missing required `--envelope-out=PATH` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let event_raw = event_id.ok_or_else(|| {
        "missing required `--event-id=NAME` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let stream_raw = stream_id.ok_or_else(|| {
        "missing required `--stream-id=NAME` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let rendition_raw = rendition_id.ok_or_else(|| {
        "missing required `--rendition-id=NAME` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let track_kind_label = track_kind.ok_or_else(|| {
        "missing required `--track-kind=video|audio|data` for `sorafs_cli taikai bundle`"
            .to_string()
    })?;
    let codec_label = codec.ok_or_else(|| {
        "missing required `--codec=<identifier>` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let bitrate = bitrate_kbps.ok_or_else(|| {
        "missing required `--bitrate-kbps=KBPS` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let sequence = segment_sequence.ok_or_else(|| {
        "missing required `--segment-sequence=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let pts = segment_start_pts.ok_or_else(|| {
        "missing required `--segment-start-pts=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let duration = segment_duration.ok_or_else(|| {
        "missing required `--segment-duration=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let wallclock = wallclock_unix_ms.ok_or_else(|| {
        "missing required `--wallclock-unix-ms=N` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let manifest_hex = manifest_hash_hex.ok_or_else(|| {
        "missing required `--manifest-hash=HEX` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let storage_ticket_hex = storage_ticket_hex.ok_or_else(|| {
        "missing required `--storage-ticket=HEX` for `sorafs_cli taikai bundle`".to_string()
    })?;
    let event_name = Name::from_str(&event_raw)
        .map_err(|err| format!("invalid `--event-id` value `{event_raw}`: {err}"))?;
    let stream_name = Name::from_str(&stream_raw)
        .map_err(|err| format!("invalid `--stream-id` value `{stream_raw}`: {err}"))?;
    let rendition_name = Name::from_str(&rendition_raw)
        .map_err(|err| format!("invalid `--rendition-id` value `{rendition_raw}`: {err}"))?;
    let manifest_digest = parse_blob_digest_field(&manifest_hex, "--manifest-hash")?;
    let storage_ticket = parse_storage_ticket_field(&storage_ticket_hex, "--storage-ticket")?;
    let parsed_kind = parse_taikai_track_kind(&track_kind_label)?;
    let metadata = build_taikai_track_metadata(
        parsed_kind,
        &codec_label,
        bitrate,
        resolution.as_deref(),
        audio_layout.as_deref(),
    )?;
    let extra_metadata =
        if let Some(path) = metadata_json.as_ref() {
            Some(load_extra_metadata(path).map_err(|err| {
                format!("failed to load metadata JSON `{}`: {err}", path.display())
            })?)
        } else {
            None
        };
    let bundle_inputs = TaikaiBundleInputs {
        event_id: event_raw,
        stream_id: stream_raw,
        rendition_id: rendition_raw,
        track_kind: parsed_kind.as_str().to_string(),
        codec: codec_label.clone(),
        bitrate_kbps: bitrate,
        resolution: resolution.clone(),
        audio_layout: audio_layout.clone(),
        segment_sequence: sequence,
        segment_start_pts: pts,
        segment_duration: duration,
        wallclock_unix_ms: wallclock,
        manifest_hash_hex: manifest_hex.clone(),
        storage_ticket_hex: storage_ticket_hex.clone(),
        ingest_latency_ms,
        live_edge_drift_ms,
        ingest_node_id: ingest_node_id.clone(),
    };
    let summary = bundle_segment(&BundleRequest {
        payload_path: &payload_path,
        payload_bytes: None,
        car_out: &car_path,
        envelope_out: &envelope_path,
        indexes_out: indexes_out.as_deref(),
        ingest_metadata_out: ingest_metadata_out.as_deref(),
        manifest_hash: manifest_digest,
        storage_ticket,
        event_id: TaikaiEventId::new(event_name),
        stream_id: TaikaiStreamId::new(stream_name),
        rendition_id: TaikaiRenditionId::new(rendition_name),
        track: metadata,
        segment_sequence: sequence,
        segment_start_pts: pts,
        segment_duration: duration,
        wallclock_unix_ms: wallclock,
        ingest_latency_ms,
        live_edge_drift_ms,
        ingest_node_id,
        extra_metadata,
    })
    .map_err(|err| format!("failed to bundle Taikai segment: {err}"))?;
    println!("Taikai segment bundle generated");
    println!("car_cid (multibase): {}", summary.car_pointer.cid_multibase);
    println!(
        "car_digest (blake3-256 hex): {}",
        hex::encode(summary.car_pointer.car_digest.as_bytes())
    );
    println!("car_size_bytes: {}", summary.car_pointer.car_size_bytes);
    println!(
        "chunk_root (blake3-256 hex): {}",
        hex::encode(summary.chunk_root.as_bytes())
    );
    println!("chunk_count: {}", summary.chunk_count);
    println!("car_out: {}", summary.car_out.display());
    println!("envelope_out: {}", summary.envelope_out.display());
    if let Some(path) = summary.indexes_out.as_ref() {
        println!("indexes_out: {}", path.display());
    }
    if let Some(path) = summary.ingest_metadata_out.as_ref() {
        println!("ingest_metadata_out: {}", path.display());
    }
    if let Some(path) = summary_out.as_ref() {
        let summary_value = render_taikai_summary_value(&bundle_inputs, &summary);
        write_summary_json(path, &summary_value)?;
        println!("summary_out: {}", path.display());
    }
    Ok(())
}
struct TaikaiBundleInputs {
    event_id: String,
    stream_id: String,
    rendition_id: String,
    track_kind: String,
    codec: String,
    bitrate_kbps: u32,
    resolution: Option<String>,
    audio_layout: Option<String>,
    segment_sequence: u64,
    segment_start_pts: u64,
    segment_duration: u32,
    wallclock_unix_ms: u64,
    manifest_hash_hex: String,
    storage_ticket_hex: String,
    ingest_latency_ms: Option<u32>,
    live_edge_drift_ms: Option<i32>,
    ingest_node_id: Option<String>,
}
#[derive(Clone, Copy, Debug)]
enum TaikaiCliTrackKind {
    Video,
    Audio,
    Data,
}
impl TaikaiCliTrackKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Data => "data",
        }
    }
}
fn parse_taikai_track_kind(value: &str) -> Result<TaikaiCliTrackKind, String> {
    match value {
        "video" => Ok(TaikaiCliTrackKind::Video),
        "audio" => Ok(TaikaiCliTrackKind::Audio),
        "data" => Ok(TaikaiCliTrackKind::Data),
        other if other.trim().to_ascii_lowercase() == other => Err(format!(
            "invalid `--track-kind` value `{other}`; expected video|audio|data"
        )),
        other => Err(format!(
            "`--track-kind` value `{other}` must be canonical lowercase video|audio|data"
        )),
    }
}
fn build_taikai_track_metadata(
    kind: TaikaiCliTrackKind,
    codec_label: &str,
    bitrate_kbps: u32,
    resolution: Option<&str>,
    audio_layout: Option<&str>,
) -> Result<TaikaiTrackMetadata, String> {
    let codec = TaikaiCodec::from_str(codec_label)
        .map_err(|err| format!("invalid `--codec` value `{codec_label}`: {err}"))?;
    match kind {
        TaikaiCliTrackKind::Video => {
            let value = resolution
                .ok_or_else(|| "`--resolution` is required for `--track-kind=video`".to_string())?;
            let parsed = TaikaiResolution::from_str(value).map_err(|err| {
                format!("invalid `--resolution` value `{value}` for video track: {err}")
            })?;
            Ok(TaikaiTrackMetadata::video(codec, bitrate_kbps, parsed))
        }
        TaikaiCliTrackKind::Audio => {
            let value = audio_layout.ok_or_else(|| {
                "`--audio-layout` is required for `--track-kind=audio`".to_string()
            })?;
            let parsed = TaikaiAudioLayout::from_str(value)
                .map_err(|err| format!("invalid `--audio-layout` value `{value}`: {err}"))?;
            Ok(TaikaiTrackMetadata::audio(codec, bitrate_kbps, parsed))
        }
        TaikaiCliTrackKind::Data => Ok(TaikaiTrackMetadata::data(codec, bitrate_kbps)),
    }
}
fn parse_blob_digest_field(value: &str, flag: &str) -> Result<BlobDigest, String> {
    let digest = parse_taikai_digest_hex(value, flag)
        .map_err(|err| format!("invalid `{flag}` value `{value}`: {err}"))?;
    Ok(BlobDigest::new(digest))
}
fn parse_storage_ticket_field(value: &str, flag: &str) -> Result<StorageTicketId, String> {
    let bytes = parse_taikai_digest_hex(value, flag)
        .map_err(|err| format!("invalid `{flag}` value `{value}`: {err}"))?;
    Ok(StorageTicketId::new(bytes))
}
fn parse_taikai_digest_hex(value: &str, flag: &str) -> Result<[u8; 32], String> {
    if value.is_empty() {
        return Err(format!("`{flag}` must not be empty"));
    }
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        return Err(format!("`{flag}` must not contain ASCII whitespace"));
    }
    if value.starts_with("0x") || value.starts_with("0X") {
        return Err(format!("`{flag}` must not use a hex prefix"));
    }
    if value.len() != 64 {
        return Err(format!(
            "`{flag}` must contain exactly 64 lowercase hex characters"
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(format!(
            "`{flag}` must contain only lowercase hex characters"
        ));
    }
    let digest = parse_digest_hex(value)?;
    if digest.iter().all(|&byte| byte == 0) {
        return Err(format!("`{flag}` must not be all zero"));
    }
    Ok(digest)
}
fn render_taikai_summary_value(inputs: &TaikaiBundleInputs, summary: &BundleSummary) -> Value {
    let mut ingest = Map::new();
    insert_value!(ingest["event_id"] = inputs.event_id.clone());
    insert_value!(ingest["stream_id"] = inputs.stream_id.clone());
    insert_value!(ingest["rendition_id"] = inputs.rendition_id.clone());
    insert_value!(ingest["segment_sequence"] = inputs.segment_sequence);
    insert_value!(ingest["segment_start_pts"] = inputs.segment_start_pts);
    insert_value!(ingest["segment_duration"] = inputs.segment_duration);
    insert_value!(ingest["wallclock_unix_ms"] = inputs.wallclock_unix_ms);
    insert_value!(ingest["manifest_hash"] = inputs.manifest_hash_hex.clone());
    insert_value!(ingest["storage_ticket"] = inputs.storage_ticket_hex.clone());
    if let Some(latency) = inputs.ingest_latency_ms {
        insert_value!(ingest["ingest_latency_ms"] = latency);
    }
    if let Some(drift) = inputs.live_edge_drift_ms {
        insert_value!(ingest["live_edge_drift_ms"] = drift);
    }
    if let Some(node) = inputs.ingest_node_id.as_ref() {
        insert_value!(ingest["ingest_node_id"] = node.clone());
    }
    let mut track = Map::new();
    insert_value!(track["kind"] = inputs.track_kind.clone());
    insert_value!(track["codec"] = inputs.codec.clone());
    insert_value!(track["bitrate_kbps"] = inputs.bitrate_kbps);
    if let Some(resolution) = inputs.resolution.as_ref() {
        insert_value!(track["resolution"] = resolution.clone());
    }
    if let Some(layout) = inputs.audio_layout.as_ref() {
        insert_value!(track["audio_layout"] = layout.clone());
    }
    let mut car = Map::new();
    insert_value!(car["cid_multibase"] = summary.car_pointer.cid_multibase.clone());
    insert_value!(
        car["digest_blake3_hex"] = hex::encode(summary.car_pointer.car_digest.as_bytes())
    );
    insert_value!(car["size_bytes"] = summary.car_pointer.car_size_bytes);
    let mut chunk = Map::new();
    insert_value!(chunk["root_blake3_hex"] = hex::encode(summary.chunk_root.as_bytes()));
    insert_value!(chunk["count"] = summary.chunk_count);
    let mut outputs = Map::new();
    insert_value!(outputs["car_out"] = summary.car_out.display().to_string());
    insert_value!(outputs["envelope_out"] = summary.envelope_out.display().to_string());
    if let Some(path) = summary.indexes_out.as_ref() {
        insert_value!(outputs["indexes_out"] = path.display().to_string());
    }
    if let Some(path) = summary.ingest_metadata_out.as_ref() {
        insert_value!(outputs["ingest_metadata_out"] = path.display().to_string());
    }
    let mut root = Map::new();
    insert_json!(root["ingest"] = Value::Object(ingest));
    insert_json!(root["track"] = Value::Object(track));
    insert_json!(root["car"] = Value::Object(car));
    insert_json!(root["chunk"] = Value::Object(chunk));
    insert_json!(root["outputs"] = Value::Object(outputs));
    if let Ok(value) = to_value(&summary.indexes) {
        insert_json!(root["indexes"] = value);
    }
    Value::Object(root)
}
fn write_summary_json(path: &Path, value: &Value) -> Result<(), String> {
    let rendered =
        to_string_pretty(value).map_err(|err| format!("failed to render summary JSON: {err}"))?;
    write_text(path, rendered.as_bytes())
}
enum StatusOutputFormat {
    Table,
    Json,
}
enum ReportOutputFormat {
    Markdown,
    Json,
}
const POR_STATUS_RESPONSE_ENVELOPE_MAX_BYTES_V1: usize = 64 * 1024;
const POR_STATUS_PAGE_MAX_INSPECTED_CANDIDATES_V1: usize = 512;
const POR_STATUS_DECODE_MAX_TOTAL_ELEMENTS_V1: usize =
    POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 * 64;
const POR_STATUS_DECODE_ALLOCATION_MULTIPLIER_V1: usize = 4;
const POR_STATUS_DECODE_MAX_NESTING_DEPTH_V1: usize = 32;
#[derive(Clone, Copy)]
struct PorStatusResponseBoundsV1 {
    response_max_bytes: usize,
    response_max_bytes_u64: u64,
    response_read_limit: u64,
    decode_limits: norito::DecodeLimits,
}
fn por_status_response_bounds(canonical_record_bytes: usize) -> Option<PorStatusResponseBoundsV1> {
    if canonical_record_bytes == 0
        || canonical_record_bytes > POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1
    {
        return None;
    }
    let response_max_bytes =
        canonical_record_bytes.checked_add(POR_STATUS_RESPONSE_ENVELOPE_MAX_BYTES_V1)?;
    let response_max_bytes_u64 = u64::try_from(response_max_bytes).ok()?;
    let response_read_limit = response_max_bytes_u64.checked_add(1)?;
    let max_total_allocated_bytes =
        response_max_bytes.checked_mul(POR_STATUS_DECODE_ALLOCATION_MULTIPLIER_V1)?;
    let decode_limits = norito::DecodeLimits::new(
        POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
        response_max_bytes,
        POR_STATUS_DECODE_MAX_TOTAL_ELEMENTS_V1,
        max_total_allocated_bytes,
        POR_STATUS_DECODE_MAX_NESTING_DEPTH_V1,
    );
    Some(PorStatusResponseBoundsV1 {
        response_max_bytes,
        response_max_bytes_u64,
        response_read_limit,
        decode_limits,
    })
}
#[derive(Debug, NoritoSerialize, NoritoDeserialize)]
struct ToriiPorStatusPageV1 {
    version: u8,
    snapshot_generation: u64,
    record_limit: u32,
    canonical_byte_limit: u64,
    canonical_bytes: u64,
    inspected_candidates: u32,
    has_more: bool,
    #[norito(default)]
    next_cursor: Option<String>,
    statuses: Vec<PorChallengeStatusV1>,
}
#[derive(Debug, NoritoSerialize, NoritoDeserialize)]
struct ToriiPorStatusExportPageV1 {
    version: u8,
    #[norito(default)]
    start_epoch: Option<u64>,
    #[norito(default)]
    end_epoch: Option<u64>,
    page: ToriiPorStatusPageV1,
}
#[derive(Debug, Clone, Copy, Default)]
struct RequestedPorStatusFilter {
    manifest_digest: Option<[u8; 32]>,
    provider_id: Option<[u8; 32]>,
    epoch_id: Option<u64>,
    outcome: Option<PorChallengeOutcome>,
}
fn validate_sorafs_por_cursor(cursor: &str, context: &str) -> Result<PorStatusCursorV1, String> {
    PorStatusCursorV1::decode_opaque(cursor)
        .map_err(|error| {
            format!(
                "{context} must be a bounded canonical PoR cursor (maximum {POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1} bytes): {error}"
            )
        })
}
fn validate_torii_por_status_page(
    page: &ToriiPorStatusPageV1,
    expected_limit: usize,
    expected_max_bytes: usize,
) -> Result<(), String> {
    if page.version != 1
        || page.snapshot_generation == 0
        || usize::try_from(page.record_limit).ok() != Some(expected_limit)
        || usize::try_from(page.canonical_byte_limit).ok() != Some(expected_max_bytes)
        || page.statuses.len() > expected_limit
        || usize::try_from(page.canonical_bytes)
            .ok()
            .is_none_or(|bytes| bytes > expected_max_bytes)
        || usize::try_from(page.inspected_candidates)
            .ok()
            .is_none_or(|count| {
                count < page.statuses.len() || count > POR_STATUS_PAGE_MAX_INSPECTED_CANDIDATES_V1
            })
        || page.has_more != page.next_cursor.is_some()
        || (page.has_more && page.inspected_candidates == 0)
    {
        return Err("PoR status page metadata violates the requested bounds".into());
    }
    if let Some(cursor) = page.next_cursor.as_deref() {
        let cursor = validate_sorafs_por_cursor(cursor, "PoR status page next_cursor")?;
        if cursor.snapshot_generation != page.snapshot_generation {
            return Err("PoR status page next_cursor does not bind the response generation".into());
        }
    }
    let mut canonical_bytes = 0usize;
    for (index, status) in page.statuses.iter().enumerate() {
        status
            .validate()
            .map_err(|error| format!("PoR status record #{index} is invalid: {error}"))?;
        canonical_bytes = canonical_bytes
            .checked_add(
                to_bytes(status)
                    .map_err(|error| format!("failed to encode PoR status #{index}: {error}"))?
                    .len(),
            )
            .ok_or_else(|| "PoR status canonical-byte accounting overflowed".to_owned())?;
    }
    if u64::try_from(canonical_bytes).ok() != Some(page.canonical_bytes) {
        return Err("PoR status canonical-byte total does not match its page envelope".into());
    }
    Ok(())
}
fn validate_por_status_order(
    statuses: &[PorChallengeStatusV1],
    epoch_ordered: bool,
) -> Result<(), String> {
    let strictly_ordered = statuses.windows(2).all(|pair| {
        if epoch_ordered {
            (pair[0].epoch_id, pair[0].issued_at, pair[0].challenge_id)
                < (pair[1].epoch_id, pair[1].issued_at, pair[1].challenge_id)
        } else {
            (pair[0].issued_at, pair[0].challenge_id) < (pair[1].issued_at, pair[1].challenge_id)
        }
    });
    if !strictly_ordered {
        return Err("PoR status records are not in strict canonical order".into());
    }
    Ok(())
}
fn validate_por_status_filter_membership(
    statuses: &[PorChallengeStatusV1],
    filter: RequestedPorStatusFilter,
) -> Result<(), String> {
    for (index, status) in statuses.iter().enumerate() {
        if filter
            .manifest_digest
            .is_some_and(|manifest| status.manifest_digest != manifest)
        {
            return Err(format!(
                "PoR status record #{index} does not match the requested manifest filter"
            ));
        }
        if filter
            .provider_id
            .is_some_and(|provider| status.provider_id != provider)
        {
            return Err(format!(
                "PoR status record #{index} does not match the requested provider filter"
            ));
        }
        if filter
            .epoch_id
            .is_some_and(|epoch| status.epoch_id != epoch)
        {
            return Err(format!(
                "PoR status record #{index} does not match the requested epoch filter"
            ));
        }
        if filter
            .outcome
            .is_some_and(|outcome| status.status != outcome)
        {
            return Err(format!(
                "PoR status record #{index} does not match the requested outcome filter"
            ));
        }
    }
    Ok(())
}
fn por_status(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut manifest_hex: Option<String> = None;
    let mut provider_hex: Option<String> = None;
    let mut epoch: Option<u64> = None;
    let mut status_filter: Option<String> = None;
    let mut limit: Option<u32> = None;
    let mut max_bytes: Option<usize> = None;
    let mut cursor: Option<String> = None;
    let mut format_label: String = "table".to_string();
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--manifest" => manifest_hex = Some(value.to_ascii_lowercase()),
            "--provider" => provider_hex = Some(value.to_ascii_lowercase()),
            "--epoch" => {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|err| format!("invalid `--epoch` value: {err}"))?;
                epoch = Some(parsed);
            }
            "--status" => status_filter = Some(value.to_string()),
            "--limit" => {
                let parsed = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|err| format!("invalid `--limit` value: {err}"))?;
                if parsed == 0 {
                    return Err("`--limit` must be greater than zero".into());
                }
                limit = Some(parsed);
            }
            "--max-bytes" => {
                let parsed = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|err| format!("invalid `--max-bytes` value: {err}"))?;
                if parsed == 0 || parsed > POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1 {
                    return Err(format!(
                        "`--max-bytes` must be in 1..={POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1}"
                    ));
                }
                max_bytes = Some(parsed);
            }
            "--cursor" => {
                let value = value.trim();
                validate_sorafs_por_cursor(value, "`--cursor`")?;
                cursor = Some(value.to_owned());
            }
            "--format" => format_label = value.to_string(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por status`"
                ));
            }
        }
    }
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por status`".to_string()
    })?;
    let output_format = match format_label.trim().to_ascii_lowercase().as_str() {
        "table" => StatusOutputFormat::Table,
        "json" => StatusOutputFormat::Json,
        other => {
            return Err(format!(
                "unsupported `--format` value `{other}`; expected table|json"
            ));
        }
    };
    let manifest_filter = manifest_hex
        .as_deref()
        .map(|hex| {
            parse_digest_hex(hex).map_err(|err| {
                format!("invalid `--manifest` digest `{hex}` supplied to `por status`: {err}")
            })
        })
        .transpose()?;
    let provider_filter = provider_hex
        .as_deref()
        .map(|hex| {
            parse_digest_hex(hex).map_err(|err| {
                format!("invalid `--provider` digest `{hex}` supplied to `por status`: {err}")
            })
        })
        .transpose()?;
    let outcome_filter = status_filter
        .as_deref()
        .map(|label| {
            PorChallengeOutcome::parse(label).map_err(|err| {
                format!("invalid `--status` value `{label}` supplied to `por status`: {err}")
            })
        })
        .transpose()?;
    let status_param = outcome_filter.map(|outcome| outcome.as_str().to_owned());
    let response_filter = RequestedPorStatusFilter {
        manifest_digest: manifest_filter,
        provider_id: provider_filter,
        epoch_id: epoch,
        outcome: outcome_filter,
    };
    let effective_limit = match limit {
        Some(limit) => usize::try_from(limit)
            .map_err(|_| format!("`--limit={limit}` cannot be represented on this platform"))?,
        None => POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
    };
    if effective_limit > POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 {
        return Err(format!(
            "`--limit={effective_limit}` exceeds the PoR status page maximum of {POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1}"
        ));
    }
    let effective_max_bytes = max_bytes.unwrap_or(POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1);
    let mut endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join("v1/sorafs/por/status")
        .map_err(|err| format!("failed to resolve PoR status endpoint: {err}"))?;
    let mut serializer = Serializer::new(String::new());
    if let Some(hex) = manifest_hex {
        serializer.append_pair("manifest", hex.trim());
    }
    if let Some(hex) = provider_hex {
        serializer.append_pair("provider", hex.trim());
    }
    if let Some(epoch) = epoch {
        serializer.append_pair("epoch", &epoch.to_string());
    }
    if let Some(status) = status_param {
        serializer.append_pair("status", status.as_str());
    }
    serializer.append_pair("limit", &effective_limit.to_string());
    serializer.append_pair("max_bytes", &effective_max_bytes.to_string());
    if let Some(cursor) = cursor {
        serializer.append_pair("cursor", &cursor);
    }
    let query = serializer.finish();
    if !query.is_empty() {
        endpoint.set_query(Some(&query));
    }
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .get(endpoint.clone())
        .header("Accept", "application/x-norito, application/json")
        .send()
        .map_err(|err| format!("failed to request PoR status from `{endpoint}`: {err}"))?;
    let status = response.status();
    let response_bounds = por_status_response_bounds(effective_max_bytes)
        .ok_or_else(|| "PoR status response bound overflowed".to_owned())?;
    let response_max_bytes = response_bounds.response_max_bytes;
    if response
        .content_length()
        .is_some_and(|length| length > response_bounds.response_max_bytes_u64)
    {
        return Err(format!(
            "PoR status response exceeds the {response_max_bytes}-byte envelope limit"
        ));
    }
    let mut body = Vec::new();
    response
        .take(response_bounds.response_read_limit)
        .read_to_end(&mut body)
        .map_err(|err| format!("failed to read PoR status response: {err}"))?;
    if body.len() > response_max_bytes {
        return Err(format!(
            "PoR status response exceeds the {response_max_bytes}-byte envelope limit"
        ));
    }
    if !status.is_success() {
        return Err(format!(
            "Torii responded with status {status} for `por status`: {}",
            body_snippet(&body)
        ));
    }
    let page: ToriiPorStatusPageV1 =
        norito::decode_from_bytes_with_limits(&body, response_bounds.decode_limits)
            .map_err(|err| format!("failed to decode PoR status page: {err}"))?;
    if to_bytes(&page).map_err(|err| format!("failed to re-encode PoR status page: {err}"))? != body
    {
        return Err("PoR status page is not canonical Norito".into());
    }
    validate_torii_por_status_page(&page, effective_limit, effective_max_bytes)?;
    validate_por_status_order(&page.statuses, false)?;
    validate_por_status_filter_membership(&page.statuses, response_filter)?;
    let next_cursor = page.next_cursor;
    let statuses = page.statuses;
    match output_format {
        StatusOutputFormat::Table => {
            let rendered = render_status_table(&statuses);
            println!("{rendered}");
        }
        StatusOutputFormat::Json => {
            let value = to_value(&statuses)
                .map_err(|err| format!("failed to serialise status JSON: {err}"))?;
            let pretty = to_string_pretty(&value)
                .map_err(|err| format!("failed to pretty-print status JSON: {err}"))?;
            println!("{pretty}");
        }
    }
    if let Some(cursor) = next_cursor {
        eprintln!("next_cursor={cursor}");
    }
    Ok(())
}
fn por_export(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut start_epoch: Option<u64> = None;
    let mut end_epoch: Option<u64> = None;
    let mut limit: usize = POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1;
    let mut max_bytes: usize = POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1;
    let mut cursor: Option<String> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--out" => out_path = Some(PathBuf::from(value)),
            "--start-epoch" => {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|err| format!("invalid `--start-epoch` value: {err}"))?;
                start_epoch = Some(parsed);
            }
            "--end-epoch" => {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|err| format!("invalid `--end-epoch` value: {err}"))?;
                end_epoch = Some(parsed);
            }
            "--limit" => {
                limit = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|err| format!("invalid `--limit` value: {err}"))?;
                if limit == 0 || limit > POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 {
                    return Err(format!(
                        "`--limit` must be in 1..={POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1}"
                    ));
                }
            }
            "--max-bytes" => {
                max_bytes = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|err| format!("invalid `--max-bytes` value: {err}"))?;
                if max_bytes == 0 || max_bytes > POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1 {
                    return Err(format!(
                        "`--max-bytes` must be in 1..={POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1}"
                    ));
                }
            }
            "--cursor" => {
                let value = value.trim();
                validate_sorafs_por_cursor(value, "`--cursor`")?;
                cursor = Some(value.to_owned());
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por export`"
                ));
            }
        }
    }
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por export`".to_string()
    })?;
    let out_path = out_path
        .ok_or_else(|| "missing required `--out=PATH` for `sorafs_cli por export`".to_string())?;
    if start_epoch.is_some() != end_epoch.is_some() {
        return Err("`--start-epoch` and `--end-epoch` must be supplied together".into());
    }
    if let (Some(start), Some(end)) = (start_epoch, end_epoch)
        && start > end
    {
        return Err("`--start-epoch` must not exceed `--end-epoch`".into());
    }
    let mut endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join("v1/sorafs/por/export")
        .map_err(|err| format!("failed to resolve PoR export endpoint: {err}"))?;
    let mut serializer = Serializer::new(String::new());
    if let Some(start) = start_epoch {
        serializer.append_pair("start_epoch", &start.to_string());
    }
    if let Some(end) = end_epoch {
        serializer.append_pair("end_epoch", &end.to_string());
    }
    serializer.append_pair("limit", &limit.to_string());
    serializer.append_pair("max_bytes", &max_bytes.to_string());
    if let Some(cursor) = cursor {
        serializer.append_pair("cursor", &cursor);
    }
    let query = serializer.finish();
    if !query.is_empty() {
        endpoint.set_query(Some(&query));
    }
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .get(endpoint.clone())
        .header("Accept", "application/octet-stream")
        .send()
        .map_err(|err| format!("failed to request PoR export from `{endpoint}`: {err}"))?;
    let status = response.status();
    let response_bounds = por_status_response_bounds(max_bytes)
        .ok_or_else(|| "PoR export response bound overflowed".to_owned())?;
    let response_max_bytes = response_bounds.response_max_bytes;
    if response
        .content_length()
        .is_some_and(|length| length > response_bounds.response_max_bytes_u64)
    {
        return Err(format!(
            "PoR export response exceeds the {response_max_bytes}-byte envelope limit"
        ));
    }
    let mut body = Vec::new();
    response
        .take(response_bounds.response_read_limit)
        .read_to_end(&mut body)
        .map_err(|err| format!("failed to read PoR export response: {err}"))?;
    if body.len() > response_max_bytes {
        return Err(format!(
            "PoR export response exceeds the {response_max_bytes}-byte envelope limit"
        ));
    }
    if !status.is_success() {
        return Err(format!(
            "PoR export failed with status {status}: {}",
            body_snippet(&body)
        ));
    }
    let export: ToriiPorStatusExportPageV1 =
        norito::decode_from_bytes_with_limits(&body, response_bounds.decode_limits)
            .map_err(|err| format!("failed to decode PoR export page: {err}"))?;
    if to_bytes(&export).map_err(|err| format!("failed to re-encode PoR export page: {err}"))?
        != body
    {
        return Err("PoR export page is not canonical Norito".into());
    }
    if export.version != 1 || export.start_epoch != start_epoch || export.end_epoch != end_epoch {
        return Err("PoR export page does not match the requested epoch range".into());
    }
    validate_torii_por_status_page(&export.page, limit, max_bytes)?;
    let epoch_ordered = start_epoch.is_some();
    validate_por_status_order(&export.page.statuses, epoch_ordered)?;
    if let (Some(start), Some(end)) = (start_epoch, end_epoch)
        && export
            .page
            .statuses
            .iter()
            .any(|status| !(start..=end).contains(&status.epoch_id))
    {
        return Err("PoR export page contains a status outside the requested epoch range".into());
    }
    write_bytes(&out_path, &body)?;
    println!("exported {} bytes to `{}`.", body.len(), out_path.display());
    if let Some(cursor) = export.page.next_cursor {
        println!("next_cursor={cursor}");
    }
    Ok(())
}
fn por_report(raw_args: Vec<String>) -> Result<(), String> {
    let mut torii_url: Option<String> = None;
    let mut week_label: Option<String> = None;
    let mut format_label: String = "markdown".to_string();
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(por_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--week" => week_label = Some(value.to_string()),
            "--format" => format_label = value.to_string(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli por report`"
                ));
            }
        }
    }
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli por report`".to_string()
    })?;
    let week_label = week_label.ok_or_else(|| {
        "missing required `--week=YYYY-Www` for `sorafs_cli por report`".to_string()
    })?;
    let iso_week = parse_iso_week_arg(&week_label)?;
    let output_format = match format_label.trim().to_ascii_lowercase().as_str() {
        "markdown" => ReportOutputFormat::Markdown,
        "json" => ReportOutputFormat::Json,
        other => {
            return Err(format!(
                "unsupported `--format` value `{other}`; expected markdown|json"
            ));
        }
    };
    let endpoint = Url::parse(&torii_url)
        .map_err(|err| format!("invalid `--torii-url` value `{torii_url}`: {err}"))?
        .join(&format!("v1/sorafs/por/report/{iso_week}"))
        .map_err(|err| format!("failed to resolve PoR report endpoint: {err}"))?;
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let response = client
        .get(endpoint.clone())
        .header("Accept", "application/x-norito, application/json")
        .send()
        .map_err(|err| format!("failed to request PoR report from `{endpoint}`: {err}"))?;
    let status = response.status();
    if response.content_length().is_some_and(|length| {
        length
            > u64::try_from(POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1)
                .expect("weekly PoR report bound fits u64")
    }) {
        return Err(format!(
            "PoR report response exceeds the {}-byte canonical limit",
            POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1
        ));
    }
    let response_read_limit = u64::try_from(
        POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1
            .checked_add(1)
            .expect("weekly PoR report bound can be incremented"),
    )
    .expect("weekly PoR report bound fits u64");
    let mut body = Vec::new();
    response
        .take(response_read_limit)
        .read_to_end(&mut body)
        .map_err(|err| format!("failed to read PoR report response: {err}"))?;
    if body.len() > POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1 {
        return Err(format!(
            "PoR report response exceeds the {}-byte canonical limit",
            POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1
        ));
    }
    if !status.is_success() {
        return Err(format!(
            "PoR report fetch failed with status {status}: {}",
            body_snippet(&body)
        ));
    }
    let report: PorWeeklyReportV1 = decode_por_weekly_report_v1(&body)
        .map_err(|err| format!("failed to decode PoR weekly report: {err}"))?;
    report
        .validate()
        .map_err(|err| format!("weekly report failed validation: {err}"))?;
    if report.cycle != iso_week {
        return Err("PoR weekly report cycle does not match the requested week".into());
    }
    match output_format {
        ReportOutputFormat::Markdown => {
            let rendered = render_report_markdown(&report);
            println!("{rendered}");
        }
        ReportOutputFormat::Json => {
            let value = to_value(&report)
                .map_err(|err| format!("failed to serialise report JSON: {err}"))?;
            let pretty = to_string_pretty(&value)
                .map_err(|err| format!("failed to pretty-print report JSON: {err}"))?;
            println!("{pretty}");
        }
    }
    Ok(())
}
fn build_from_file(
    input: &Path,
    descriptor: &ChunkerProfileDescriptor,
    car_out: &Path,
    plan_out: Option<&PathBuf>,
    summary_out: Option<&PathBuf>,
    handle: &str,
) -> Result<(), String> {
    let payload = fs::read(input)
        .map_err(|err| format!("failed to read payload `{}`: {err}", input.display()))?;
    let plan = CarBuildPlan::single_file_with_profile(&payload, descriptor.profile)
        .map_err(|err| format!("failed to chunk payload: {err}"))?;
    emit_car_and_artifacts(
        InputSummary::File {
            path: input.to_path_buf(),
            bytes: payload.len() as u64,
        },
        descriptor,
        handle,
        plan,
        Cursor::new(payload),
        car_out,
        plan_out,
        summary_out,
    )
}
fn build_from_directory(
    input: &Path,
    descriptor: &ChunkerProfileDescriptor,
    car_out: &Path,
    plan_out: Option<&PathBuf>,
    summary_out: Option<&PathBuf>,
    handle: &str,
) -> Result<(), String> {
    let (plan, payload) = CarBuildPlan::from_directory_with_profile(input, descriptor.profile)
        .map_err(|err| format!("failed to build directory plan: {err}"))?;
    emit_car_and_artifacts(
        InputSummary::Directory {
            path: input.to_path_buf(),
            file_count: plan.files.len() as u64,
        },
        descriptor,
        handle,
        plan,
        Cursor::new(payload),
        car_out,
        plan_out,
        summary_out,
    )
}
#[allow(clippy::too_many_arguments)]
fn emit_car_and_artifacts(
    input: InputSummary,
    descriptor: &ChunkerProfileDescriptor,
    handle: &str,
    mut plan: CarBuildPlan,
    mut payload: Cursor<Vec<u8>>,
    car_out: &Path,
    plan_out: Option<&PathBuf>,
    summary_out: Option<&PathBuf>,
) -> Result<(), String> {
    let por_root = compute_por_root(payload.get_ref(), &plan)
        .map_err(|err| format!("failed to derive payload PoR root: {err}"))?;
    let car_file = open_output_file(car_out)?;
    let mut writer = BufWriter::new(car_file);
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload, &mut writer)
        .map_err(format_car_error)?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush `{}`: {err}", car_out.display()))?;
    if stats.chunk_profile != descriptor.profile {
        return Err("emitted CAR used unexpected chunk profile".to_string());
    }
    if let Some(plan_path) = plan_out {
        let spec_json = chunk_fetch_plan_to_string(&plan)
            .map_err(|err| format!("failed to render chunk plan: {err}"))?;
        write_text(plan_path, spec_json.as_bytes())?;
    }
    let summary = render_summary(&input, descriptor, handle, &plan, &stats, por_root, car_out)?;
    let rendered =
        to_string_pretty(&summary).map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(summary_path) = summary_out {
        write_text(summary_path, rendered.as_bytes())?;
    }
    // Drop payload bytes held in the plan to free memory before returning.
    plan.chunks.shrink_to_fit();
    Ok(())
}
fn render_summary(
    input: &InputSummary,
    descriptor: &ChunkerProfileDescriptor,
    handle: &str,
    plan: &CarBuildPlan,
    stats: &sorafs_car::CarWriteStats,
    por_root: [u8; 32],
    car_path: &Path,
) -> Result<Value, String> {
    let chunk_fetch_specs = plan
        .try_chunk_fetch_specs()
        .map_err(|err| format!("failed to validate chunk fetch plan: {err}"))?;
    let mut obj = Map::new();
    insert_value!(obj["chunker_handle"] = handle);
    insert_value!(obj["chunker_profile_id"] = descriptor.id.0 as u64);
    insert_value!(
        obj["chunker_profile_canonical"] = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )
    );
    insert_value!(obj["payload_bytes"] = plan.content_length);
    insert_value!(obj["chunk_count"] = plan.chunks.len() as u64);
    insert_value!(obj["file_count"] = plan.files.len() as u64);
    insert_value!(obj["car_size"] = stats.car_size);
    insert_value!(obj["car_payload_digest_hex"] = hex_encode(stats.car_payload_digest.as_bytes()));
    insert_value!(obj["car_digest_hex"] = hex_encode(stats.car_archive_digest.as_bytes()));
    insert_value!(
        obj["chunk_digest_sha3_256_hex"] =
            hex_encode(chunk_digest_sha3_from_specs(&chunk_fetch_specs))
    );
    insert_value!(obj["por_root_hex"] = hex_encode(por_root));
    insert_value!(obj["car_cid_hex"] = hex_encode(&stats.car_cid));
    insert_json!(
        obj["root_cids_hex"] = Value::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| Value::from(hex_encode(cid)))
                .collect(),
        )
    );
    insert_value!(obj["output_car"] = car_path.display().to_string());
    match input {
        InputSummary::File { path, bytes } => {
            insert_value!(obj["input_kind"] = "file");
            insert_value!(obj["input_path"] = path.display().to_string());
            insert_value!(obj["input_bytes"] = *bytes);
        }
        InputSummary::Directory { path, file_count } => {
            insert_value!(obj["input_kind"] = "directory");
            insert_value!(obj["input_path"] = path.display().to_string());
            insert_value!(obj["input_file_count"] = *file_count);
        }
    }
    Ok(Value::Object(obj))
}
fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create `{}`: {err}", parent.display()))?;
    }
    Ok(())
}
fn anonymity_policy_label(policy: AnonymityPolicy) -> &'static str {
    match policy {
        AnonymityPolicy::GuardPq => "anon-guard-pq",
        AnonymityPolicy::MajorityPq => "anon-majority-pq",
        AnonymityPolicy::StrictPq => "anon-strict-pq",
    }
}
fn write_text(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_bytes(path, bytes)
}
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = open_output_file(path)?;
    file.write_all(bytes)
        .map_err(|err| format!("failed to write `{}`: {err}", path.display()))
}
fn open_output_file(path: &Path) -> Result<File, String> {
    validate_output_path(path)?;
    ensure_parent_dir(path)?;
    validate_output_path(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .map_err(|err| format!("failed to open `{}` for writing: {err}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to inspect `{}` after open: {err}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "failed to write `{}`: output must be a regular file",
            path.display()
        ));
    }
    Ok(file)
}
fn validate_output_path(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!("output `{}` must not be a symlink", path.display()));
            }
            if metadata.is_dir() {
                return Err(format!(
                    "output `{}` must not be a directory",
                    path.display()
                ));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "failed to inspect output `{}`: {err}",
                path.display()
            ));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(format!(
                            "output parent `{}` must not be a symlink",
                            ancestor.display()
                        ));
                    }
                    if !metadata.is_dir() {
                        return Err(format!(
                            "output parent `{}` must be a directory",
                            ancestor.display()
                        ));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!(
                        "failed to inspect output parent `{}`: {err}",
                        ancestor.display()
                    ));
                }
            }
        }
    }
    Ok(())
}
#[cfg(unix)]
fn set_no_follow_flag(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut OpenOptions) {}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}
fn format_car_error(err: CarWriteError) -> String {
    match err {
        CarWriteError::Io(io_err) => format!("streaming payload failed: {io_err}"),
        other => format!("failed to emit CAR: {other}"),
    }
}
fn usage() -> String {
    "Usage:
  sorafs_cli norito build --source=PATH --bytecode-out=PATH [--summary-out=PATH]
  sorafs_cli deploy --payload=PATH --client-config=PATH [--torii-url=URL] [--name=NAME] [--out-dir=PATH] [--gateway-base-url=URL] [--no-peer-discovery] [--summary-out=PATH]
  sorafs_cli car pack --input=PATH --car-out=PATH [--chunker-handle=HANDLE] [--plan-out=PATH] [--summary-out=PATH]
  sorafs_cli manifest build --summary=PATH --manifest-out=PATH [--manifest-json-out=PATH] [--pin-min-replicas=N] [--pin-storage-class=hot|warm|cold] [--pin-retention-epoch=EPOCH] [--metadata key=value]
  sorafs_cli manifest submit --manifest=PATH --torii-url=URL --network-id=NETWORK_ID (--chunk-plan=PATH | --chunk-digest-sha3=HEX) --authority=ACCOUNT [--network-prefix=U16] (--private-key=KEY | --private-key-file=PATH) [--alias-namespace=NS --alias-name=NAME --alias-proof=PATH] [--successor-of=HEX] [--summary-out=PATH] [--response-out=PATH]
  sorafs_cli manifest proposal --manifest=PATH (--chunk-plan=PATH | --chunk-digest-sha3=HEX) --proposal-out=PATH [--successor-of=HEX] [--alias-hint=TEXT]
  sorafs_cli storage prepare --manifest=PATH --payload=PATH --payload-out=PATH --files-out=PATH [--summary-out=PATH]
  sorafs_cli fetch --plan=PATH --manifest-id=HEX [--chunker-handle=HANDLE] [--manifest-envelope=BASE64] [--manifest-report=PATH|-] [--manifest-cid=HEX] [--client-id=ID] [--telemetry-region=REGION] [--rollout-phase=canary|ramp|default] [--transport-policy=soranet-first|soranet-strict|direct-only] [--transport-policy-override=soranet-first|soranet-strict|direct-only] [--anonymity-policy=anon-guard-pq|anon-majority-pq|anon-strict-pq] [--anonymity-policy-override=anon-guard-pq|anon-majority-pq|anon-strict-pq] [--write-mode=read-only|upload-pq-only] [--scoreboard-out=PATH] [--scoreboard-now=UNIX_SECS] [--telemetry-source-label=LABEL] [--profile=hot|warm|cold] [--orchestrator-config=PATH] [--taikai-cache-config=PATH] [--output=PATH] [--json-out=PATH] [--local-proxy-mode=bridge|metadata-only] [--local-proxy-norito-spool=PATH] [--max-peers=N] [--retry-budget=N] [--expected-cache-version=VERSION] --provider name=ALIAS,provider-id=HEX,gateway-key=HEX,base-url=URL,stream-token=BASE64 [...]
  sorafs_cli proof stream --manifest=PATH (--torii-url=HTTPS_ORIGIN | --gateway-url=HTTPS_URL) --provider-id-hex=HEX32 --bearer-token-env=VAR [--proof-kind=por|pdp|potr] [--challenge-id-hex=HEX32] [--samples=N] [--sample-seed=SEED] [--deadline-ms=N] [--tier=hot|warm|archive] [--nonce-b64=BASE64] [--orchestrator-job-id-hex=HEX16] [--summary-out=PATH] [--governance-evidence-dir=DIR] [--emit-events=true|false]
  sorafs_cli proof verify --manifest=PATH --car=PATH [--chunk-plan=PATH] [--summary-out=PATH]
  sorafs_cli pdp enqueue|next|submit|status|export --torii-url=HTTPS_ORIGIN --network-id=NETWORK_ID --operator-private-key-file=PATH [operation options; run `sorafs_cli pdp` for details]
  sorafs_cli reputation snapshot --torii-url=URL --network-id=NETWORK_ID --auth-account=I105 --auth-private-key-file=PATH [--output=PATH] [--summary-out=PATH]
  sorafs_cli reputation fetch --torii-url=URL --network-id=NETWORK_ID --provider-id=ID --auth-account=I105 --auth-private-key-file=PATH [--format=table|json] [--summary-out=PATH]
  sorafs_cli reputation watch --torii-url=URL --network-id=NETWORK_ID --auth-account=I105 --auth-private-key-file=PATH [--since=N] [--limit=N] [--max-polls=N] [--poll-interval-ms=N] [--summary-out=PATH]
  sorafs_cli reputation verify --snapshot=PATH [--provider-id=ID --proof=PATH] [--summary-out=PATH]
  sorafs_cli por status --torii-url=URL [--manifest=HEX32] [--provider=HEX32] [--epoch=N] [--status=awaiting_proof|proof_submitted|verified|failed|repaired] [--limit=N] [--max-bytes=N] [--cursor=OPAQUE] [--format=table|json]
  sorafs_cli por export --torii-url=URL --out=PATH [--start-epoch=N --end-epoch=N] [--limit=N] [--max-bytes=N] [--cursor=OPAQUE]
  sorafs_cli por report --torii-url=URL --week=YYYY-Www [--format=markdown|json]
  sorafs_cli proxy set-mode --orchestrator-config=PATH --mode=bridge|metadata-only [--json-out=PATH] [--config-out=PATH] [--dry-run]
  sorafs_cli taikai bundle --payload=PATH --car-out=PATH --envelope-out=PATH --event-id=NAME --stream-id=NAME --rendition-id=NAME --track-kind=video|audio|data --codec=CODEC --bitrate-kbps=KBPS --segment-sequence=N --segment-start-pts=N --segment-duration=N --wallclock-unix-ms=N --manifest-hash=HEX --storage-ticket=HEX [--indexes-out=PATH] [--ingest-metadata-out=PATH] [--summary-out=PATH] [--resolution=WxH] [--audio-layout=mono|stereo|5.1|7.1|custom:<label>] [--ingest-latency-ms=N] [--live-edge-drift-ms=N] [--ingest-node-id=ID] [--metadata-json=PATH]
  sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]
  sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]
  sorafs_cli moderation registry-serve --state=PATH [--listen=HOST:PORT] [--max-body-bytes=N] [--snapshot-limit=N]
  sorafs_cli moderation run-local --manifest=PATH --artifact-root=DIR [--format=json|norito] --payload=PATH --subject=ID --screened-at=UNIX_SECS [--max-payload-bytes=N] [--notes=TEXT] [--json-out=PATH]
  sorafs_cli moderation run-signed-local --manifest=PATH --artifact-root=DIR [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --signing-key=PATH --payload=PATH --subject=ID --provenance=PATH --provenance-log-id=HEX16 [--max-payload-bytes=N] [--notes=TEXT] [--norito-out=PATH] [--json-out=PATH]
  sorafs_cli moderation runner-serve --manifest=PATH --artifact-root=DIR [--format=json|norito] [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N]
  sorafs_cli moderation runner-signed-serve --manifest=PATH --artifact-root=DIR [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --signing-key=PATH --provenance=PATH --provenance-log-id=HEX16 [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N]
  sorafs_cli moderation runner-grpc-serve --manifest=PATH --artifact-root=DIR [--format=json|norito] [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N]
  sorafs_cli moderation runner-bundle --manifest=PATH --artifact-root=DIR [--format=json|norito] --bundle-out=DIR [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N] [--binary=PATH] [--service-name=NAME] [--service-user=USER] [--service-group=GROUP]
  sorafs_cli moderation runner-canary --manifest=PATH [--format=json|norito] --runner-url=URL --payload=PATH --subject=ID --screened-at=UNIX_SECS --generated-at-unix=UNIX_SECS --deployment-id=ID --environment=prod|production|release|staging --deployment-context-reviewed=true --process-isolation-enforcement=systemd_ip_filter|container_network_policy|host_firewall --process-isolation-attestation-digest=HEX32 --process-isolation-verified-at=UNIX_SECS --process-isolation-reviewed=true [--checked-at=UNIX_SECS] [--notes=TEXT] [--timeout-ms=N] [--json-out=PATH]
  sorafs_cli moderation committee-run --manifest=PATH [--format=json|norito] --quorum=N --result=PATH [--result=PATH...] [--notes=TEXT] [--json-out=PATH]
  sorafs_cli moderation committee-authenticated-run --manifest=PATH [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --result=SIGNED_RESULT.to [--result=SIGNED_RESULT.to...] --provenance=PATH --provenance-log-id=HEX16 [--norito-out=PATH] [--json-out=PATH]
  sorafs_cli moderation committee-serve --manifest=PATH [--format=json|norito] --quorum=N [--listen=HOST:PORT] [--max-body-bytes=N]
  sorafs_cli moderation committee-authenticated-serve --manifest=PATH [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --provenance=PATH --provenance-log-id=HEX16 [--listen=HOST:PORT] [--max-body-bytes=N]
  sorafs_cli moderation committee-bundle --manifest=PATH [--format=json|norito] --quorum=N --bundle-out=DIR [--listen=HOST:PORT] [--max-body-bytes=N] [--binary=PATH] [--service-name=NAME] [--service-user=USER] [--service-group=GROUP]
  sorafs_cli moderation committee-canary --manifest=PATH [--format=json|norito] --committee-url=URL --quorum=N --result=PATH [--result=PATH...] --generated-at-unix=UNIX_SECS --deployment-id=ID --environment=prod|production|release|staging --deployment-context-reviewed=true --process-isolation-enforcement=systemd_ip_filter|container_network_policy|host_firewall --process-isolation-attestation-digest=HEX32 --process-isolation-verified-at=UNIX_SECS --process-isolation-reviewed=true [--checked-at=UNIX_SECS] [--notes=TEXT] [--timeout-ms=N] [--json-out=PATH]
  sorafs_cli moderation honey-audit --manifest-id=HEX --honey=HEX [--honey=HEX...] --provider name=ALIAS,provider-id=HEX,gateway-key=HEX,base-url=URL,stream-token=BASE64 [...] [--chunker-handle=HANDLE] [--expected-catalog-digest=HEX] [--json-out=PATH] [--markdown-out=PATH]
  sorafs_cli appeal quote --class=content|access|fraud|other [--backlog=N] [--evidence-mb=N] [--urgency=normal|high] [--panel-size=N] [--format=table|json] [--config=PATH|-]
  sorafs_cli governance dag list --root=DIR [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag show --node=PATH [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag verify --root=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag export --root=DIR --out=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag build --root=DIR --out=DIR --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH] [--car-out=PATH [--car-plan-out=PATH] [--car-chunker-handle=HANDLE]]
  sorafs_cli governance dag verify-build --root=DIR [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag rebuild-head --root=DIR --head-out=PATH --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX] [--car=PATH] [--mirror-index=PATH] [--generated-at=UNIX_SECS]
  sorafs_cli governance dag checkpoint-verify --checkpoint=PATH [--root=DIR] [--car=PATH] [--mirror-index=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint-recover --checkpoint=PATH --root=DIR --out=PATH [--car=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag mirror-build --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag mirror-query --index=PATH (--head | --block-cid=CID|hex:HEX | --node-cid=CID|hex:HEX) [--format=table|json]"
        .to_string()
}
fn reputation_usage() -> String {
    "Usage:
  sorafs_cli reputation snapshot --torii-url=URL --network-id=NETWORK_ID --auth-account=I105 --auth-private-key-file=PATH [--output=PATH] [--summary-out=PATH]
  sorafs_cli reputation fetch --torii-url=URL --network-id=NETWORK_ID --provider-id=ID --auth-account=I105 --auth-private-key-file=PATH [--format=table|json] [--summary-out=PATH]
  sorafs_cli reputation watch --torii-url=URL --network-id=NETWORK_ID --auth-account=I105 --auth-private-key-file=PATH [--since=N] [--limit=N] [--max-polls=N] [--poll-interval-ms=N] [--summary-out=PATH]
  sorafs_cli reputation verify --snapshot=PATH [--provider-id=ID --proof=PATH] [--summary-out=PATH]"
        .to_string()
}
fn fetch_usage() -> String {
    "Usage:
  sorafs_cli fetch --plan=PATH --manifest-id=HEX --provider name=ALIAS,provider-id=HEX,gateway-key=HEX,base-url=URL,stream-token=BASE64 [additional --provider entries...] [--chunker-handle=HANDLE] [--manifest-envelope=BASE64] [--manifest-report=PATH|-] [--manifest-cid=HEX] [--client-id=ID] [--telemetry-region=REGION] [--rollout-phase=canary|ramp|default] [--transport-policy=soranet-first|soranet-strict|direct-only] [--transport-policy-override=soranet-first|soranet-strict|direct-only] [--anonymity-policy=anon-guard-pq|anon-majority-pq|anon-strict-pq] [--anonymity-policy-override=anon-guard-pq|anon-majority-pq|anon-strict-pq] [--write-mode=read-only|upload-pq-only] [--scoreboard-out=PATH] [--scoreboard-now=UNIX_SECS] [--telemetry-source-label=LABEL] [--profile=hot|warm|cold] [--orchestrator-config=PATH] [--taikai-cache-config=PATH] [--output=PATH] [--json-out=PATH] [--local-proxy-mode=bridge|metadata-only] [--local-proxy-norito-spool=PATH] [--local-proxy-manifest-out=PATH] [--max-peers=N] [--retry-budget=N] [--expected-cache-version=VERSION]"
        .to_string()
}
fn taikai_usage() -> String {
    "Usage:
  sorafs_cli taikai bundle --payload=PATH --car-out=PATH --envelope-out=PATH --event-id=NAME --stream-id=NAME --rendition-id=NAME --track-kind=video|audio|data --codec=CODEC --bitrate-kbps=KBPS --segment-sequence=N --segment-start-pts=N --segment-duration=N --wallclock-unix-ms=N --manifest-hash=HEX --storage-ticket=HEX [--indexes-out=PATH] [--ingest-metadata-out=PATH] [--summary-out=PATH] [--resolution=WxH] [--audio-layout=mono|stereo|5.1|7.1|custom:<label>] [--ingest-latency-ms=N] [--live-edge-drift-ms=N] [--ingest-node-id=ID] [--metadata-json=PATH]"
        .to_string()
}
#[derive(Clone, Copy)]
struct GatewayProviderCounts {
    direct: usize,
    gateway: usize,
}
impl GatewayProviderCounts {
    const fn new(direct: usize, gateway: usize) -> Self {
        Self { direct, gateway }
    }
    fn direct_u64(self) -> u64 {
        u64::try_from(self.direct).unwrap_or(u64::MAX)
    }
    fn gateway_u64(self) -> u64 {
        u64::try_from(self.gateway).unwrap_or(u64::MAX)
    }
    fn mix_label(self) -> &'static str {
        match (self.direct > 0, self.gateway > 0) {
            (true, true) => "mixed",
            (true, false) => "direct-only",
            (false, true) => "gateway-only",
            (false, false) => "none",
        }
    }
}
struct GatewayScoreboardMetadataInput<'a> {
    provider_counts: GatewayProviderCounts,
    max_peers: Option<usize>,
    retry_budget: Option<usize>,
    manifest_envelope_present: bool,
    gateway_manifest_id: Option<&'a str>,
    gateway_manifest_cid: Option<&'a str>,
    transport_policy: Option<TransportPolicy>,
    transport_policy_override: Option<TransportPolicy>,
    anonymity_policy: Option<AnonymityPolicy>,
    anonymity_policy_override: Option<AnonymityPolicy>,
    write_mode: WriteModeHint,
    scoreboard_now: Option<u64>,
    telemetry_source: Option<&'a str>,
}
#[derive(Clone, Copy)]
enum FetchCacheProfile {
    Warm,
    Cold,
}
impl FetchCacheProfile {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "hot" | "warm" => Some(Self::Warm),
            "cold" => Some(Self::Cold),
            _ => None,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Cold => "cold",
        }
    }
}
struct CliPolicyLabels {
    effective_label: &'static str,
    override_flag: bool,
    override_label: Option<&'static str>,
}
fn summarise_policy<T>(
    requested: Option<T>,
    override_policy: Option<T>,
    label_fn: impl Fn(T) -> &'static str,
) -> CliPolicyLabels
where
    T: Copy + Default,
{
    let effective = override_policy.or(requested).unwrap_or_default();
    CliPolicyLabels {
        effective_label: label_fn(effective),
        override_flag: override_policy.is_some(),
        override_label: override_policy.map(label_fn),
    }
}
fn summarise_transport_policy(
    requested: Option<TransportPolicy>,
    override_policy: Option<TransportPolicy>,
) -> CliPolicyLabels {
    summarise_policy(requested, override_policy, TransportPolicy::label)
}
fn summarise_anonymity_policy(
    requested: Option<AnonymityPolicy>,
    override_policy: Option<AnonymityPolicy>,
) -> CliPolicyLabels {
    summarise_policy(requested, override_policy, AnonymityPolicy::label)
}
fn option_usize_to_value(value: Option<usize>) -> Value {
    value
        .and_then(|val| u64::try_from(val).ok())
        .map(Value::from)
        .unwrap_or(Value::Null)
}
fn build_gateway_scoreboard_metadata(input: &GatewayScoreboardMetadataInput<'_>) -> Value {
    let mut metadata = Map::new();
    insert_value!(metadata["version"] = SORAFS_CLI_VERSION);
    insert_value!(metadata["use_scoreboard"] = true);
    insert_value!(metadata["allow_implicit_metadata"] = false);
    insert_value!(metadata["provider_count"] = input.provider_counts.direct_u64());
    insert_value!(metadata["gateway_provider_count"] = input.provider_counts.gateway_u64());
    insert_value!(metadata["provider_mix"] = input.provider_counts.mix_label());
    insert_json!(metadata["max_parallel"] = Value::Null);
    insert_json!(metadata["max_peers"] = option_usize_to_value(input.max_peers));
    insert_json!(metadata["retry_budget"] = option_usize_to_value(input.retry_budget));
    insert_json!(metadata["provider_failure_threshold"] = Value::Null);
    insert_json!(metadata["assume_now"] = input.scoreboard_now.map_or(Value::Null, Value::from));
    insert_json!(
        metadata["telemetry_source"] = input.telemetry_source.map_or(Value::Null, Value::from)
    );
    insert_json!(
        metadata["gateway_manifest_id"] =
            input.gateway_manifest_id.map_or(Value::Null, Value::from)
    );
    insert_json!(
        metadata["gateway_manifest_cid"] =
            input.gateway_manifest_cid.map_or(Value::Null, Value::from)
    );
    insert_value!(metadata["gateway_manifest_provided"] = input.manifest_envelope_present);
    let transport_labels =
        summarise_transport_policy(input.transport_policy, input.transport_policy_override);
    insert_value!(metadata["transport_policy"] = transport_labels.effective_label);
    insert_value!(metadata["transport_policy_override"] = transport_labels.override_flag);
    insert_json!(
        metadata["transport_policy_override_label"] = transport_labels
            .override_label
            .map_or(Value::Null, Value::from)
    );
    let anonymity_labels =
        summarise_anonymity_policy(input.anonymity_policy, input.anonymity_policy_override);
    insert_value!(metadata["anonymity_policy"] = anonymity_labels.effective_label);
    insert_value!(metadata["anonymity_policy_override"] = anonymity_labels.override_flag);
    insert_json!(
        metadata["anonymity_policy_override_label"] = anonymity_labels
            .override_label
            .map_or(Value::Null, Value::from)
    );
    insert_value!(metadata["write_mode"] = input.write_mode.label());
    insert_value!(metadata["write_mode_enforces_pq"] = input.write_mode.enforces_pq_only());
    Value::Object(metadata)
}
fn insert_telemetry_source(summary: &mut Value, telemetry_source: Option<&str>) {
    if let Some(label) = telemetry_source
        && let Some(obj) = summary.as_object_mut()
    {
        insert_value!(obj["telemetry_source"] = label);
    }
}
fn por_usage() -> String {
    "Usage:
  sorafs_cli por status --torii-url=URL [--manifest=HEX32] [--provider=HEX32] [--epoch=N] [--status=awaiting_proof|proof_submitted|verified|failed|repaired] [--limit=N] [--max-bytes=N] [--cursor=OPAQUE] [--format=table|json]
  sorafs_cli por export --torii-url=URL --out=PATH [--start-epoch=N --end-epoch=N] [--limit=N] [--max-bytes=N] [--cursor=OPAQUE]
  sorafs_cli por report --torii-url=URL --week=YYYY-Www [--format=markdown|json]"
        .to_string()
}
fn proxy_usage() -> String {
    "Usage:
  sorafs_cli proxy set-mode --orchestrator-config=PATH --mode=bridge|metadata-only [--json-out=PATH] [--config-out=PATH] [--dry-run]"
        .to_string()
}
fn appeal_usage() -> String {
    "Usage:
  sorafs_cli appeal quote --class=content|access|fraud|other [--backlog=N] [--evidence-mb=N] [--urgency=normal|high] [--panel-size=N] [--format=table|json] [--config=PATH|-]
  sorafs_cli appeal settle --deposit=XOR --outcome=uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated [--panel-size=N] [--format=table|json] [--config=PATH|-]
  sorafs_cli appeal disburse --deposit=XOR --outcome=uphold|overturn|modify|withdrawn_before_panel|withdrawn_after_panel|frivolous|escalated --refund-account=ID --treasury-account=ID --escrow-account=ID --juror=ID [--juror=ID...] [--no-show=ID...] [--panel-size=N] [--format=table|json] [--config=PATH|-]"
        .to_string()
}
fn governance_usage() -> String {
    "Usage:
  sorafs_cli governance dag list --root=DIR [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag show --node=PATH [--format=table|json] [--summary-out=PATH]
  sorafs_cli governance dag verify --root=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag export --root=DIR --out=DIR [--require-chain] [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag build --root=DIR --out=DIR --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH] [--car-out=PATH [--car-plan-out=PATH] [--car-chunker-handle=HANDLE]]
  sorafs_cli governance dag verify-build --root=DIR [--require-sidecars] [--head-cid=CID|hex:HEX] [--summary-out=PATH]
  sorafs_cli governance dag rebuild-head --root=DIR --head-out=PATH --publisher-peer-id=ID (--key-hex=HEX | --key=PATH) [--generated-at=UNIX_SECS] [--checkpoint-cid=CID|hex:HEX] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX] [--car=PATH] [--mirror-index=PATH] [--generated-at=UNIX_SECS]
  sorafs_cli governance dag checkpoint-verify --checkpoint=PATH [--root=DIR] [--car=PATH] [--mirror-index=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag checkpoint-recover --checkpoint=PATH --root=DIR --out=PATH [--car=PATH] [--require-sidecars] [--summary-out=PATH]
  sorafs_cli governance dag mirror-build --root=DIR --out=PATH [--require-sidecars] [--head-cid=CID|hex:HEX]
  sorafs_cli governance dag mirror-query --index=PATH (--head | --block-cid=CID|hex:HEX | --node-cid=CID|hex:HEX) [--format=table|json]"
        .to_string()
}
fn moderation_usage() -> String {
    "Usage:
  sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]
  sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]
  sorafs_cli moderation registry-serve --state=PATH [--listen=HOST:PORT] [--max-body-bytes=N] [--snapshot-limit=N]
  sorafs_cli moderation run-local --manifest=PATH --artifact-root=DIR [--format=json|norito] --payload=PATH --subject=ID --screened-at=UNIX_SECS [--max-payload-bytes=N] [--notes=TEXT] [--json-out=PATH]
  sorafs_cli moderation run-signed-local --manifest=PATH --artifact-root=DIR [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --signing-key=PATH --payload=PATH --subject=ID --provenance=PATH --provenance-log-id=HEX16 [--max-payload-bytes=N] [--notes=TEXT] [--norito-out=PATH] [--json-out=PATH]
  sorafs_cli moderation runner-serve --manifest=PATH --artifact-root=DIR [--format=json|norito] [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N]
  sorafs_cli moderation runner-signed-serve --manifest=PATH --artifact-root=DIR [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --signing-key=PATH --provenance=PATH --provenance-log-id=HEX16 [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N]
  sorafs_cli moderation runner-grpc-serve --manifest=PATH --artifact-root=DIR [--format=json|norito] [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N]
  sorafs_cli moderation runner-bundle --manifest=PATH --artifact-root=DIR [--format=json|norito] --bundle-out=DIR [--listen=HOST:PORT] [--max-body-bytes=N] [--max-payload-bytes=N] [--binary=PATH] [--service-name=NAME] [--service-user=USER] [--service-group=GROUP]
  sorafs_cli moderation runner-canary --manifest=PATH [--format=json|norito] --runner-url=URL --payload=PATH --subject=ID --screened-at=UNIX_SECS --generated-at-unix=UNIX_SECS --deployment-id=ID --environment=prod|production|release|staging --deployment-context-reviewed=true --process-isolation-enforcement=systemd_ip_filter|container_network_policy|host_firewall --process-isolation-attestation-digest=HEX32 --process-isolation-verified-at=UNIX_SECS --process-isolation-reviewed=true [--checked-at=UNIX_SECS] [--notes=TEXT] [--timeout-ms=N] [--json-out=PATH]
  sorafs_cli moderation committee-run --manifest=PATH [--format=json|norito] --quorum=N --result=PATH [--result=PATH...] [--notes=TEXT] [--json-out=PATH]
  sorafs_cli moderation committee-authenticated-run --manifest=PATH [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --result=SIGNED_RESULT.to [--result=SIGNED_RESULT.to...] --provenance=PATH --provenance-log-id=HEX16 [--norito-out=PATH] [--json-out=PATH]
  sorafs_cli moderation committee-serve --manifest=PATH [--format=json|norito] --quorum=N [--listen=HOST:PORT] [--max-body-bytes=N]
  sorafs_cli moderation committee-authenticated-serve --manifest=PATH [--format=json|norito] --trust-policy=PATH [--trust-policy-format=json|norito] --trust-anchor=PUBLIC_KEY [--trust-anchor=PUBLIC_KEY...] --minimum-governance-quorum=N --provenance=PATH --provenance-log-id=HEX16 [--listen=HOST:PORT] [--max-body-bytes=N]
  sorafs_cli moderation committee-bundle --manifest=PATH [--format=json|norito] --quorum=N --bundle-out=DIR [--listen=HOST:PORT] [--max-body-bytes=N] [--binary=PATH] [--service-name=NAME] [--service-user=USER] [--service-group=GROUP]
  sorafs_cli moderation committee-canary --manifest=PATH [--format=json|norito] --committee-url=URL --quorum=N --result=PATH [--result=PATH...] --generated-at-unix=UNIX_SECS --deployment-id=ID --environment=prod|production|release|staging --deployment-context-reviewed=true --process-isolation-enforcement=systemd_ip_filter|container_network_policy|host_firewall --process-isolation-attestation-digest=HEX32 --process-isolation-verified-at=UNIX_SECS --process-isolation-reviewed=true [--checked-at=UNIX_SECS] [--notes=TEXT] [--timeout-ms=N] [--json-out=PATH]
  sorafs_cli moderation honey-audit --manifest-id=HEX --honey=HEX [--honey=HEX...] --provider name=ALIAS,provider-id=HEX,gateway-key=HEX,base-url=URL,stream-token=BASE64 [...] [--chunker-handle=HANDLE] [--expected-catalog-digest=HEX] [--json-out=PATH] [--markdown-out=PATH]

Validates internally signed AI moderation reproducibility manifests and adversarial corpus registries before a separate governance trust policy admits them. `run-signed-local` is the production trust-boundary path: it verifies external governance anchors, signs a fresh result with a policy-authorized runner key, and atomically appends the complete result to a tamper-evident provenance segment. `committee-authenticated-run` verifies distinct authorized runner signatures, freshness, revocation, and policy quorum before persisting the deterministic aggregate. `run-local`, `runner-serve`, `runner-grpc-serve`, `committee-run`, and `committee-serve` are unsigned diagnostic/foundation paths and must not be used as production trust boundaries."
        .to_string()
}
fn fetch_gateway(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(fetch_usage());
    }
    let mut plan_source: Option<JsonSource> = None;
    let mut manifest_id_hex: Option<String> = None;
    let mut chunker_handle_hint: Option<String> = None;
    let mut manifest_envelope: Option<String> = None;
    let mut manifest_report_source: Option<JsonSource> = None;
    let mut manifest_cid_hex: Option<String> = None;
    let mut expected_cache_version: Option<String> = None;
    let mut client_id: Option<String> = None;
    let mut telemetry_region: Option<String> = None;
    let mut rollout_phase: Option<RolloutPhase> = None;
    let mut orchestrator_config_source: Option<JsonSource> = None;
    let mut taikai_cache_source: Option<JsonSource> = None;
    let mut transport_policy: Option<TransportPolicy> = None;
    let mut anonymity_policy: Option<AnonymityPolicy> = None;
    let mut transport_policy_override: Option<TransportPolicy> = None;
    let mut anonymity_policy_override: Option<AnonymityPolicy> = None;
    let mut write_mode: Option<WriteModeHint> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut local_proxy_manifest_out: Option<PathBuf> = None;
    let mut local_proxy_mode_override: Option<ProxyMode> = None;
    let mut local_proxy_spool_override: Option<String> = None;
    let mut local_proxy_kaigi_spool_override: Option<String> = None;
    let mut local_proxy_kaigi_policy_override: Option<String> = None;
    let mut max_peers: Option<usize> = None;
    let mut retry_budget: Option<usize> = None;
    let mut provider_specs: Vec<GatewayProviderSpec> = Vec::new();
    let mut scoreboard_out: Option<PathBuf> = None;
    let mut scoreboard_now: Option<u64> = None;
    let mut telemetry_source_label: Option<String> = None;
    let mut cache_profile: Option<FetchCacheProfile> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(fetch_usage());
        }
        if let Some(rest) = arg.strip_prefix("--plan=") {
            plan_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--manifest-id=") {
            manifest_id_hex = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--chunker-handle=") {
            chunker_handle_hint = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--manifest-envelope=") {
            manifest_envelope = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--manifest-report=") {
            manifest_report_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--manifest-cid=") {
            manifest_cid_hex = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--expected-cache-version=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--expected-cache-version` must not be empty".into());
            }
            expected_cache_version = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--client-id=") {
            client_id = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--telemetry-region=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--telemetry-region` must not be empty".into());
            }
            telemetry_region = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--rollout-phase=") {
            if rest.is_empty() {
                return Err("`--rollout-phase` must not be empty".into());
            }
            let parsed = RolloutPhase::parse(rest).ok_or_else(|| {
                "`--rollout-phase` must be one of canary|ramp|default".to_string()
            })?;
            rollout_phase = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--transport-policy=") {
            if rest.is_empty() {
                return Err("`--transport-policy` must not be empty".into());
            }
            let parsed = TransportPolicy::parse(rest).ok_or_else(|| {
                "`--transport-policy` must be one of soranet-first|soranet-strict|direct-only"
                    .to_string()
            })?;
            transport_policy = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--anonymity-policy=") {
            if rest.is_empty() {
                return Err("`--anonymity-policy` must not be empty".into());
            }
            let parsed = AnonymityPolicy::parse(rest).ok_or_else(|| {
                "`--anonymity-policy` must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq"
                    .to_string()
            })?;
            anonymity_policy = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--write-mode=") {
            if rest.is_empty() {
                return Err("`--write-mode` must not be empty".into());
            }
            let parsed = WriteModeHint::parse(rest).ok_or_else(|| {
                "`--write-mode` must be one of read-only|upload-pq-only".to_string()
            })?;
            write_mode = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--transport-policy-override=") {
            if rest.is_empty() {
                return Err("`--transport-policy-override` must not be empty".into());
            }
            let parsed = TransportPolicy::parse(rest).ok_or_else(|| {
                "`--transport-policy-override` must be one of soranet-first|soranet-strict|direct-only"
                    .to_string()
            })?;
            transport_policy_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--anonymity-policy-override=") {
            if rest.is_empty() {
                return Err("`--anonymity-policy-override` must not be empty".into());
            }
            let parsed = AnonymityPolicy::parse(rest).ok_or_else(|| {
                "`--anonymity-policy-override` must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq"
                    .to_string()
            })?;
            anonymity_policy_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--scoreboard-out=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--scoreboard-out` must not be empty".into());
            }
            scoreboard_out = Some(PathBuf::from(trimmed));
        } else if let Some(rest) = arg.strip_prefix("--scoreboard-now=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--scoreboard-now` must not be empty".into());
            }
            let parsed = trimmed
                .parse::<u64>()
                .map_err(|err| format!("`--scoreboard-now` must be an unsigned integer: {err}"))?;
            scoreboard_now = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--telemetry-source-label=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--telemetry-source-label` must not be empty".into());
            }
            telemetry_source_label = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--profile=") {
            let normalized = rest.trim().to_ascii_lowercase().replace('-', "_");
            let parsed = FetchCacheProfile::parse(&normalized).ok_or_else(|| {
                "`--profile` must be one of hot|warm|cold for `sorafs_cli fetch`".to_string()
            })?;
            cache_profile = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--orchestrator-config=") {
            orchestrator_config_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--policy=") {
            orchestrator_config_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--taikai-cache-config=") {
            taikai_cache_source = Some(JsonSource::from_arg(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--output=") {
            output_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-manifest-out=") {
            local_proxy_manifest_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-mode=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-mode` must not be empty".into());
            }
            let parsed = ProxyMode::parse(trimmed).ok_or_else(|| {
                "`--local-proxy-mode` must be one of bridge|metadata-only".to_string()
            })?;
            local_proxy_mode_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-norito-spool=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-norito-spool` must not be empty".into());
            }
            local_proxy_spool_override = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-kaigi-spool=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-kaigi-spool` must not be empty".into());
            }
            local_proxy_kaigi_spool_override = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--local-proxy-kaigi-policy=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--local-proxy-kaigi-policy` must not be empty".into());
            }
            let normalized = trimmed.to_ascii_lowercase();
            match normalized.as_str() {
                "public" | "authenticated" => {
                    local_proxy_kaigi_policy_override = Some(normalized);
                }
                _ => {
                    return Err(
                        "`--local-proxy-kaigi-policy` must be `public` or `authenticated`".into(),
                    );
                }
            }
        } else if let Some(rest) = arg.strip_prefix("--max-peers=") {
            max_peers = Some(parse_usize(rest, "--max-peers")?);
        } else if let Some(rest) = arg.strip_prefix("--retry-budget=") {
            retry_budget = Some(parse_usize(rest, "--retry-budget")?);
        } else if let Some(rest) = arg.strip_prefix("--provider=") {
            provider_specs.push(parse_gateway_provider_spec(rest)?);
        } else {
            return Err(format!(
                "unrecognised option `{arg}` for `sorafs_cli fetch`"
            ));
        }
    }
    let plan_source = plan_source
        .ok_or_else(|| "missing required `--plan=PATH` for `sorafs_cli fetch`".to_string())?;
    let manifest_id_hex = manifest_id_hex
        .ok_or_else(|| "missing required `--manifest-id=HEX` for `sorafs_cli fetch`".to_string())?;
    if manifest_id_hex.len() != 64 || !manifest_id_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("`--manifest-id` must be a 32-byte hex string".to_string());
    }
    if let Some(cid) = &manifest_cid_hex
        && (cid.len() != 64 || !cid.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("`--manifest-cid` must be a 32-byte hex string".to_string());
    }
    if provider_specs.is_empty() {
        return Err("provide at least one `--provider` entry".to_string());
    }
    let manifest_report = if let Some(source) = manifest_report_source.take() {
        Some(source.read()?)
    } else {
        None
    };
    if let Some(report) = manifest_report.as_ref() {
        if manifest_envelope.is_none()
            && let Some(encoded) = report.get("manifest_b64").and_then(Value::as_str)
            && !encoded.trim().is_empty()
        {
            manifest_envelope = Some(encoded.trim().to_string());
        }
        if manifest_cid_hex.is_none()
            && let Some(cid) = report.get("manifest_id_hex").and_then(Value::as_str)
        {
            manifest_cid_hex = Some(cid.trim().to_ascii_lowercase());
        }
    }
    let plan_json = plan_source.read()?;
    let plan_with_handle = build_plan_from_specs(&plan_json, chunker_handle_hint.as_deref())?;
    let plan = plan_with_handle.plan;
    let chunker_handle = plan_with_handle.chunker_handle;
    let gateway_config = GatewayFetchConfig {
        manifest_id_hex: manifest_id_hex.clone(),
        chunker_handle: chunker_handle.clone(),
        manifest_envelope_b64: manifest_envelope.clone(),
        client_id: client_id.clone(),
        expected_manifest_cid_hex: manifest_cid_hex.clone(),
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: expected_cache_version.clone(),
    };
    let provider_inputs: Vec<GatewayProviderInput> = provider_specs
        .iter()
        .map(|spec| GatewayProviderInput {
            name: spec.name.clone(),
            provider_id_hex: spec.provider_id_hex.clone(),
            gateway_public_key_hex: spec.gateway_public_key_hex.clone(),
            base_url: spec.base_url.clone(),
            stream_token_b64: spec.stream_token_b64.clone(),
            privacy_events_url: spec.privacy_events_url.clone(),
        })
        .collect();
    let context = GatewayFetchContext::new(gateway_config.clone(), provider_inputs.clone())
        .map_err(|err| format!("failed to construct gateway context: {err}"))?;
    let context_providers = context.providers();
    if context_providers.is_empty() {
        return Err("gateway context did not expose any providers".to_string());
    }
    let metadata: Vec<ProviderMetadata> = context_providers
        .iter()
        .map(|provider| {
            let alias = provider.id().as_str().to_string();
            let mut meta = provider
                .metadata()
                .cloned()
                .unwrap_or_else(ProviderMetadata::new);
            meta.provider_id = Some(alias.clone());
            if !meta.profile_aliases.iter().any(|entry| entry == &alias) {
                meta.profile_aliases.push(alias.clone());
            }
            if meta.range_capability.is_none() {
                meta.range_capability = Some(RangeCapability {
                    max_chunk_span: u32::MAX,
                    min_granularity: 1,
                    supports_sparse_offsets: true,
                    requires_alignment: false,
                    supports_merkle_proof: true,
                });
            }
            if meta.stream_budget.is_none()
                && let Some(max_streams) = meta.max_streams
            {
                meta.stream_budget = Some(StreamBudget {
                    max_in_flight: max_streams,
                    max_bytes_per_sec: 0,
                    burst_bytes: None,
                });
            }
            meta
        })
        .collect();
    let telemetry_snapshot = TelemetrySnapshot::default();
    let mut orchestrator_config = if let Some(source) = orchestrator_config_source {
        let value = source.read()?;
        orchestrator_config_from_json(&value)
            .map_err(|err| format!("failed to parse orchestrator config JSON: {err}"))?
    } else {
        OrchestratorConfig::default()
    };
    if let Some(path) = scoreboard_out {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create scoreboard directory `{}`: {err}",
                    parent.display()
                )
            })?;
        }
        orchestrator_config.scoreboard.persist_path = Some(path);
    }
    if let Some(now) = scoreboard_now {
        orchestrator_config.scoreboard.now_unix_secs = now;
    } else if orchestrator_config.scoreboard.persist_path.is_some()
        && orchestrator_config.scoreboard.now_unix_secs == 0
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_secs();
        orchestrator_config.scoreboard.now_unix_secs = now;
    }
    if let Some(mode) = write_mode {
        orchestrator_config = orchestrator_config.with_write_mode(mode);
    }
    let provider_counts = GatewayProviderCounts::new(0, provider_inputs.len());
    let scoreboard_metadata = build_gateway_scoreboard_metadata(&GatewayScoreboardMetadataInput {
        provider_counts,
        max_peers,
        retry_budget,
        manifest_envelope_present: manifest_envelope.is_some(),
        gateway_manifest_id: Some(manifest_id_hex.as_str()),
        gateway_manifest_cid: manifest_cid_hex.as_deref(),
        transport_policy,
        transport_policy_override,
        anonymity_policy,
        anonymity_policy_override,
        write_mode: orchestrator_config.write_mode,
        scoreboard_now,
        telemetry_source: telemetry_source_label.as_deref(),
    });
    orchestrator_config.scoreboard.persist_metadata = Some(scoreboard_metadata);
    if let Some(phase) = rollout_phase {
        orchestrator_config = orchestrator_config.with_rollout_phase(phase);
    }
    if let Some(source) = taikai_cache_source {
        let value = source.read()?;
        match parse_taikai_cache_override(value)? {
            Some(cache) => {
                orchestrator_config.taikai_cache = Some(cache);
            }
            None => {
                orchestrator_config.taikai_cache = None;
            }
        }
    }
    if let Some(proxy_cfg) = orchestrator_config.local_proxy.as_mut() {
        if let Some(mode) = local_proxy_mode_override {
            proxy_cfg.proxy_mode = mode;
        }
        if let Some(spool) = local_proxy_spool_override.clone() {
            proxy_cfg.norito_bridge = Some(ProxyNoritoBridgeConfig {
                spool_dir: spool,
                extension: Some("norito".to_string()),
            });
        }
        if matches!(proxy_cfg.proxy_mode, ProxyMode::Bridge) && proxy_cfg.norito_bridge.is_none() {
            proxy_cfg.norito_bridge = Some(ProxyNoritoBridgeConfig {
                spool_dir: PROVISION_SPOOL_DIR.to_string(),
                extension: Some("norito".to_string()),
            });
        }
        if matches!(proxy_cfg.proxy_mode, ProxyMode::Bridge) && proxy_cfg.kaigi_bridge.is_none() {
            proxy_cfg.kaigi_bridge = Some(ProxyKaigiBridgeConfig {
                spool_dir: PROVISION_SPOOL_DIR.to_string(),
                extension: Some("norito".to_string()),
                room_policy: Some("public".to_string()),
            });
        }
        if let Some(policy) = local_proxy_kaigi_policy_override.clone() {
            let bridge = proxy_cfg
                .kaigi_bridge
                .get_or_insert_with(|| ProxyKaigiBridgeConfig {
                    spool_dir: PROVISION_SPOOL_DIR.to_string(),
                    extension: Some("norito".to_string()),
                    room_policy: None,
                });
            bridge.room_policy = Some(policy);
            if bridge.extension.is_none() {
                bridge.extension = Some("norito".to_string());
            }
        }
        if let Some(spool) = local_proxy_kaigi_spool_override.clone() {
            let bridge = proxy_cfg
                .kaigi_bridge
                .get_or_insert_with(|| ProxyKaigiBridgeConfig {
                    spool_dir: spool.clone(),
                    extension: Some("norito".to_string()),
                    room_policy: None,
                });
            bridge.spool_dir = spool;
            if bridge.extension.is_none() {
                bridge.extension = Some("norito".to_string());
            }
        }
    } else if local_proxy_mode_override.is_some() || local_proxy_spool_override.is_some() {
        return Err("`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, and `--local-proxy-kaigi-policy` require `local_proxy` in the orchestrator config".to_string());
    } else if local_proxy_kaigi_spool_override.is_some()
        || local_proxy_kaigi_policy_override.is_some()
    {
        return Err("`--local-proxy-kaigi-spool` and `--local-proxy-kaigi-policy` require `local_proxy` in the orchestrator config".to_string());
    }
    let local_proxy_snapshot = orchestrator_config.local_proxy.clone();
    if local_proxy_manifest_out.is_some() {
        let Some(proxy_cfg) = local_proxy_snapshot.as_ref() else {
            return Err(
                "--local-proxy-manifest-out requires `local_proxy` in the orchestrator config"
                    .to_string(),
            );
        };
        if !proxy_cfg.emit_browser_manifest {
            return Err(
                "--local-proxy-manifest-out requires `local_proxy.emit_browser_manifest` to be true"
                    .to_string(),
            );
        }
        #[cfg(not(feature = "local-quic-proxy"))]
        {
            return Err(
                "--local-proxy-manifest-out requires local QUIC proxy runtime support; rebuild `sorafs_cli` with the `local-quic-proxy` feature"
                    .to_string(),
            );
        }
    }
    let mut fetch_options = orchestrator_config.fetch.clone();
    let scoreboard = sorafs_car::scoreboard::build_scoreboard(
        &plan,
        &metadata,
        &telemetry_snapshot,
        &orchestrator_config.scoreboard,
    )
    .map_err(|err| format!("failed to build provider scoreboard: {err}"))?;
    let eligible_count = scoreboard
        .entries()
        .iter()
        .filter(|entry| matches!(entry.eligibility, Eligibility::Eligible))
        .count();
    if eligible_count == 0 {
        return Err("no eligible providers available after capability checks".to_string());
    }
    let ineligible_providers: Vec<Value> = scoreboard
        .entries()
        .iter()
        .filter_map(|entry| match &entry.eligibility {
            Eligibility::Ineligible(reason) => {
                let mut obj = Map::new();
                insert_value!(obj["provider"] = entry.provider.id().as_str().to_string());
                insert_value!(obj["reason"] = reason.to_string());
                Some(Value::Object(obj))
            }
            Eligibility::Eligible => None,
        })
        .collect();
    if let Some(limit) = max_peers {
        let limit = limit.max(1);
        fetch_options.global_parallel_limit = Some(
            fetch_options
                .global_parallel_limit
                .map_or(limit, |existing| existing.min(limit)),
        );
    }
    if let Some(budget) = retry_budget {
        fetch_options.per_chunk_retry_limit = Some(budget);
    }
    if let Some(limit) = max_peers {
        let limit = limit.max(1);
        orchestrator_config.max_providers = std::num::NonZeroUsize::new(limit);
    }
    let mut telemetry_region_effective = telemetry_region
        .map(|region| region.trim().to_string())
        .filter(|value| !value.is_empty());
    if telemetry_region_effective.is_none() {
        telemetry_region_effective = orchestrator_config.telemetry_region.clone();
    }
    orchestrator_config.telemetry_region = telemetry_region_effective.clone();
    orchestrator_config.fetch = fetch_options;
    if let Some(policy) = transport_policy {
        orchestrator_config.transport_policy = policy;
    }
    if let Some(policy) = anonymity_policy {
        orchestrator_config.anonymity_policy = policy;
        orchestrator_config.anonymity_policy_override = Some(policy);
    }
    if let Some(policy) = transport_policy_override {
        orchestrator_config.policy_override.transport_policy = Some(policy);
    }
    let requested_anonymity_override = anonymity_policy_override;
    let rollout_phase = orchestrator_config.rollout_phase;
    let write_mode = orchestrator_config.write_mode;
    let runtime =
        Runtime::new().map_err(|err| format!("failed to initialise Tokio runtime: {err}"))?;
    let session = runtime
        .block_on(fetch_via_gateway(
            orchestrator_config,
            &plan,
            gateway_config,
            provider_inputs,
            Some(&telemetry_snapshot),
            max_peers,
        ))
        .map_err(|err| format!("fetch failed: {err}"))?;
    let outcome = &session.outcome;
    if let Some(path) = output_path {
        let assembled = outcome.assemble_payload();
        write_bytes(&path, &assembled)?;
    }
    let mut summary = build_fetch_summary(
        manifest_id_hex.as_str(),
        &chunker_handle,
        &plan,
        &session,
        FetchSummaryOptions {
            client_id: client_id.as_deref(),
            rollout_phase,
            write_mode,
            cache_profile,
        },
    );
    if let Some(region) = telemetry_region_effective.as_deref()
        && let Some(obj) = summary.as_object_mut()
    {
        insert_value!(obj["telemetry_region"] = region);
    }
    insert_telemetry_source(&mut summary, telemetry_source_label.as_deref());
    if let Some(obj) = summary.as_object_mut() {
        insert_json!(obj["ineligible_providers"] = Value::Array(ineligible_providers));
        if let Some(policy) = requested_anonymity_override {
            insert_value!(obj["anonymity_policy"] = policy.label());
            insert_value!(obj["anonymity_policy_override"] = true);
            insert_value!(obj["anonymity_policy_override_label"] = policy.label());
        }
        if let Some(proxy_cfg) = local_proxy_snapshot.as_ref() {
            insert_value!(obj["local_proxy_mode"] = proxy_cfg.proxy_mode.as_str());
            if let Some(bridge) = proxy_cfg.norito_bridge.as_ref() {
                insert_value!(obj["local_proxy_norito_spool"] = bridge.spool_dir.clone());
            }
            if let Some(bridge) = proxy_cfg.kaigi_bridge.as_ref() {
                insert_value!(obj["local_proxy_kaigi_spool"] = bridge.spool_dir.clone());
                let policy = bridge
                    .room_policy
                    .as_deref()
                    .unwrap_or("public")
                    .to_string();
                insert_value!(obj["local_proxy_kaigi_policy"] = policy);
            }
        }
    }
    if let Some(path) = local_proxy_manifest_out {
        let manifest = session.local_proxy_manifest.as_ref().ok_or_else(|| {
            "--local-proxy-manifest-out requires `local_proxy.emit_browser_manifest = true` and an active local proxy runtime".to_string()
        })?;
        let manifest_value =
            to_value(manifest).expect("local proxy manifest should serialise to JSON");
        let manifest_json =
            to_string_pretty(&manifest_value).expect("local proxy manifest should emit valid JSON");
        write_text(&path, manifest_json.as_bytes())?;
    }
    let summary_text = to_string_pretty(&summary).expect("fetch summary should be serialisable");
    println!("{summary_text}");
    if let Some(path) = json_out {
        write_text(&path, summary_text.as_bytes())?;
    }
    Ok(())
}
fn proxy_set_mode(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(proxy_usage());
    }
    let mut config_path: Option<PathBuf> = None;
    let mut requested_mode: Option<ProxyMode> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut config_out: Option<PathBuf> = None;
    let mut dry_run = false;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(proxy_usage());
        }
        if let Some(rest) = arg.strip_prefix("--orchestrator-config=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--orchestrator-config` must not be empty".into());
            }
            if trimmed == "-" {
                return Err("`--orchestrator-config` requires a file path; stdin is not supported for remediation".into());
            }
            config_path = Some(PathBuf::from(trimmed));
        } else if let Some(rest) = arg.strip_prefix("--mode=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--mode` must not be empty".into());
            }
            let parsed = ProxyMode::parse(trimmed)
                .ok_or_else(|| "`--mode` must be one of bridge|metadata-only".to_string())?;
            requested_mode = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--json-out` must not be empty".into());
            }
            json_out = Some(PathBuf::from(trimmed));
        } else if let Some(rest) = arg.strip_prefix("--config-out=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--config-out` must not be empty".into());
            }
            config_out = Some(PathBuf::from(trimmed));
        } else if arg == "--dry-run" {
            dry_run = true;
        } else {
            return Err(format!(
                "unrecognised option `{arg}` for `sorafs_cli proxy set-mode`"
            ));
        }
    }
    let config_path =
        config_path.ok_or_else(|| "missing required `--orchestrator-config=PATH`".to_string())?;
    let requested_mode = requested_mode
        .ok_or_else(|| "missing required `--mode=bridge|metadata-only`".to_string())?;
    let config_bytes = fs::read_to_string(&config_path)
        .map_err(|err| format!("failed to read `{}`: {err}", config_path.display()))?;
    let config_value: Value = norito::json::from_str(&config_bytes)
        .map_err(|err| format!("failed to parse orchestrator config JSON: {err}"))?;
    let mut orchestrator_config = orchestrator_config_from_json(&config_value)
        .map_err(|err| format!("failed to decode orchestrator config structure: {err}"))?;
    let (previous_mode, telemetry_label, bind_addr, guard_cache_key_hex) = {
        let proxy_cfg = orchestrator_config.local_proxy.as_mut().ok_or_else(|| {
            "orchestrator config does not enable `local_proxy`; remediation is unavailable"
                .to_string()
        })?;
        let prev = proxy_cfg.proxy_mode.clone();
        proxy_cfg.proxy_mode = requested_mode.clone();
        let label = proxy_cfg.telemetry_label.clone();
        let addr = proxy_cfg.bind_addr.clone();
        let guard_key = proxy_cfg.guard_cache_key_hex.clone();
        (prev, label, addr, guard_key)
    };
    let effective_mode = orchestrator_config
        .local_proxy
        .as_ref()
        .expect("local proxy must be present")
        .proxy_mode
        .clone();
    let target_config_path = config_out.as_ref().unwrap_or(&config_path);
    if !dry_run {
        let config_value = orchestrator_config_to_json(&orchestrator_config);
        let config_json = norito::json::to_json_pretty(&config_value)
            .map_err(|err| format!("failed to render orchestrator config JSON: {err}"))?;
        write_text(target_config_path, config_json.as_bytes())?;
    }
    let mut summary = Map::new();
    insert_json!(summary["mode_previous"] = Value::String(previous_mode.as_str().to_string()));
    insert_json!(summary["mode_effective"] = Value::String(effective_mode.as_str().to_string()));
    insert_json!(summary["mode_requested"] = Value::String(requested_mode.as_str().to_string()));
    insert_value!(summary["dry_run"] = dry_run);
    insert_json!(summary["config_path"] = Value::String(config_path.to_string_lossy().into()));
    if !dry_run {
        insert_json!(
            summary["config_written"] = Value::String(target_config_path.to_string_lossy().into())
        );
    } else {
        insert_json!(summary["config_written"] = Value::Null);
    }
    if let Some(label) = telemetry_label {
        insert_json!(summary["telemetry_label"] = Value::String(label));
    }
    insert_json!(summary["bind_addr"] = Value::String(bind_addr));
    if let Some(guard_key) = guard_cache_key_hex {
        insert_json!(summary["guard_cache_key_hex"] = Value::String(guard_key));
    }
    let summary_json = norito::json::to_json_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary JSON: {err}"))?;
    if let Some(path) = json_out {
        write_text(&path, summary_json.as_bytes())?;
    } else {
        println!("{summary_json}");
    }
    Ok(())
}
fn moderation_validate_repro(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation validate-repro`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation validate-repro`".to_string()
    })?;
    let bytes = fs::read(&manifest_path)
        .map_err(|err| format!("failed to read `{}`: {err}", manifest_path.display()))?;
    let manifest: ModerationReproManifestV1 = match format.as_str() {
        "json" => norito::json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse JSON reproducibility manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).map_err(|err| {
            format!(
                "failed to decode Norito reproducibility manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        other => {
            return Err(format!(
                "unsupported `--format={other}` for `sorafs_cli moderation validate-repro` (expected `json` or `norito`)"
            ));
        }
    };
    let summary = manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    println!(
        "reproducibility manifest {} validated (models={}, signers={}, issued_at={})",
        hex_encode(summary.manifest_id),
        summary.model_count,
        summary.signer_count,
        summary.issued_at_unix
    );
    Ok(())
}
fn moderation_validate_corpus(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation validate-corpus`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation validate-corpus`".to_string()
    })?;
    let bytes = fs::read(&manifest_path)
        .map_err(|err| format!("failed to read `{}`: {err}", manifest_path.display()))?;
    let manifest: AdversarialCorpusManifestV1 = match format.as_str() {
        "json" => norito::json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse JSON adversarial corpus manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        "norito" => decode_from_bytes(&bytes).map_err(|err| {
            format!(
                "failed to decode Norito adversarial corpus manifest `{}`: {err}",
                manifest_path.display()
            )
        })?,
        other => {
            return Err(format!(
                "unsupported `--format={other}` for `sorafs_cli moderation validate-corpus` (expected `json` or `norito`)"
            ));
        }
    };
    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    let family_count = manifest.families.len();
    let variant_count: usize = manifest
        .families
        .iter()
        .map(|family| family.variants.len())
        .sum();
    let cohort = manifest.cohort_label.as_deref().unwrap_or("-");
    println!(
        "adversarial corpus manifest validated (issued_at={}, cohort={}, families={}, variants={})",
        manifest.issued_at_unix, cohort, family_count, variant_count
    );
    Ok(())
}
#[derive(Clone, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationRegistryPersistedSnapshot {
    schema_version: u16,
    repro_manifests: Vec<ModerationRegistryReproRecord>,
    adversarial_corpora: Vec<ModerationRegistryCorpusRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationRegistryReproRecord {
    manifest_id: [u8; 16],
    manifest_digest: [u8; 32],
    runner_hash: [u8; 32],
    runtime_version: String,
    issued_at_unix: u64,
    model_count: u32,
    signer_count: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationRegistryCorpusRecord {
    corpus_digest: [u8; 32],
    issued_at_unix: u64,
    cohort_label: Option<String>,
    family_count: u32,
    variant_count: u32,
}
struct ModerationRegistryService {
    state_path: PathBuf,
    state: Mutex<ModerationRegistryPersistedSnapshot>,
    max_body_bytes: usize,
    snapshot_limit: usize,
}
fn moderation_registry_serve(raw_args: Vec<String>) -> Result<(), String> {
    let mut state_path: Option<PathBuf> = None;
    let mut listen = String::from(MODERATION_REGISTRY_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    let mut snapshot_limit = 500_usize;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--state" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--state` must not be empty".to_string());
                }
                state_path = Some(PathBuf::from(trimmed));
            }
            "--listen" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--listen` must not be empty".to_string());
                }
                listen = trimmed.to_string();
            }
            "--max-body-bytes" => {
                max_body_bytes =
                    parse_moderation_max_body_bytes(value, "sorafs_cli moderation registry-serve")?;
            }
            "--snapshot-limit" => {
                let parsed = parse_u64_arg(
                    "--snapshot-limit",
                    value,
                    "sorafs_cli moderation registry-serve",
                )?;
                snapshot_limit = usize::try_from(parsed).map_err(|_| {
                    "`--snapshot-limit` does not fit into this platform's usize".to_string()
                })?;
                if snapshot_limit == 0 {
                    return Err("`--snapshot-limit` must be greater than zero".to_string());
                }
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation registry-serve`"
                ));
            }
        }
    }
    let state_path = state_path.ok_or_else(|| {
        "missing required `--state=PATH` for `sorafs_cli moderation registry-serve`".to_string()
    })?;
    let state = moderation_registry_load_state(&state_path)?;
    moderation_registry_save_state(&state_path, &state)?;
    let listen_addr =
        validate_moderation_loopback_listen(&listen, "sorafs_cli moderation registry-serve")?;
    let listener = TcpListener::bind(listen_addr).map_err(|err| {
        format!("failed to bind moderation model registry service at `{listen}`: {err}")
    })?;
    let local_addr = listener
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| listen.clone());
    let service = Arc::new(ModerationRegistryService {
        state_path,
        state: Mutex::new(state),
        max_body_bytes,
        snapshot_limit,
    });
    let status = moderation_registry_status_json(&service, "listening", Some(&local_addr))?;
    let rendered = to_string_pretty(&status)
        .map_err(|err| format!("failed to render model registry service status JSON: {err}"))?;
    println!("{rendered}");
    let active_connections = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let Some(active_permit) = moderation_try_acquire_permit(
                    &active_connections,
                    MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS,
                ) else {
                    let response = moderation_registry_error_response(
                        503,
                        "Service Unavailable",
                        "moderation registry connection limit reached",
                    );
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    continue;
                };
                let service = Arc::clone(&service);
                thread::spawn(move || {
                    let _active_permit = active_permit;
                    if let Err(err) =
                        moderation_registry_handle_stream(stream, &service, max_body_bytes)
                    {
                        eprintln!("sorafs moderation model registry connection failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("sorafs moderation model registry accept failed: {err}"),
        }
    }
    Ok(())
}
fn moderation_registry_load_state(
    path: &Path,
) -> Result<ModerationRegistryPersistedSnapshot, String> {
    if !path.exists() {
        return Ok(ModerationRegistryPersistedSnapshot {
            schema_version: 1,
            ..Default::default()
        });
    }
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read model registry state `{}`: {err}",
            path.display()
        )
    })?;
    if bytes.is_empty() {
        return Ok(ModerationRegistryPersistedSnapshot {
            schema_version: 1,
            ..Default::default()
        });
    }
    let mut snapshot =
        decode_from_bytes::<ModerationRegistryPersistedSnapshot>(&bytes).map_err(|err| {
            format!(
                "failed to decode Norito model registry state `{}`: {err}",
                path.display()
            )
        })?;
    moderation_registry_normalize_snapshot(&mut snapshot)?;
    Ok(snapshot)
}
fn moderation_registry_save_state(
    path: &Path,
    state: &ModerationRegistryPersistedSnapshot,
) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let mut normalized = state.clone();
    moderation_registry_normalize_snapshot(&mut normalized)?;
    let bytes = to_bytes(&normalized)
        .map_err(|err| format!("failed to encode model registry state as Norito: {err}"))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &bytes)
        .map_err(|err| format!("failed to write `{}`: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        format!(
            "failed to replace model registry state `{}` with `{}`: {err}",
            path.display(),
            tmp_path.display()
        )
    })
}
fn moderation_registry_normalize_snapshot(
    snapshot: &mut ModerationRegistryPersistedSnapshot,
) -> Result<(), String> {
    if snapshot.schema_version == 0 {
        snapshot.schema_version = 1;
    }
    if snapshot.schema_version != 1 {
        return Err(format!(
            "unsupported model registry snapshot version {}",
            snapshot.schema_version
        ));
    }
    snapshot
        .repro_manifests
        .sort_by_key(|record| record.manifest_id);
    snapshot
        .adversarial_corpora
        .sort_by_key(|record| record.corpus_digest);
    let mut seen_repro = BTreeSet::new();
    for record in &snapshot.repro_manifests {
        if !seen_repro.insert(record.manifest_id) {
            return Err(
                "duplicate reproducibility manifest id in model registry state".to_string(),
            );
        }
        if record.runtime_version.trim().is_empty() {
            return Err(format!(
                "reproducibility manifest `{}` has an empty runtime version",
                hex_encode(record.manifest_id)
            ));
        }
        if record.model_count == 0 {
            return Err(format!(
                "reproducibility manifest `{}` has no model fingerprints",
                hex_encode(record.manifest_id)
            ));
        }
        if record.signer_count == 0 {
            return Err(format!(
                "reproducibility manifest `{}` has no governance signers",
                hex_encode(record.manifest_id)
            ));
        }
    }
    let mut seen_corpus = BTreeSet::new();
    for record in &snapshot.adversarial_corpora {
        if !seen_corpus.insert(record.corpus_digest) {
            return Err("duplicate adversarial corpus digest in model registry state".to_string());
        }
        if record.family_count == 0 {
            return Err(format!(
                "adversarial corpus `{}` has no perceptual families",
                hex_encode(record.corpus_digest)
            ));
        }
        if record.variant_count == 0 {
            return Err(format!(
                "adversarial corpus `{}` has no variants",
                hex_encode(record.corpus_digest)
            ));
        }
    }
    Ok(())
}
fn moderation_http_hard_limit(max_body_bytes: usize, service: &str) -> io::Result<usize> {
    max_body_bytes
        .checked_add(MODERATION_RUNNER_MAX_HEADER_BYTES)
        .and_then(|limit| limit.checked_add(4))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{service} HTTP body and header limits overflow usize"),
            )
        })
}
fn moderation_registry_handle_stream(
    mut stream: TcpStream,
    service: &ModerationRegistryService,
    max_body_bytes: usize,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let hard_limit = moderation_http_hard_limit(max_body_bytes, "registry")?;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = hard_limit.saturating_sub(request.len());
        let accepted = count.min(remaining);
        request.try_reserve(accepted).map_err(|_| {
            io::Error::new(io::ErrorKind::OutOfMemory, "HTTP request allocation failed")
        })?;
        request.extend_from_slice(&buffer[..accepted]);
        if accepted < count || request.len() >= hard_limit {
            break;
        }
        if find_http_header_end(&request).is_none()
            && request.len() > MODERATION_RUNNER_MAX_HEADER_BYTES
        {
            break;
        }
        if let Some((header_len, content_len)) = moderation_runner_request_lengths(&request)
            && (content_len > max_body_bytes
                || header_len
                    .checked_add(content_len)
                    .is_some_and(|body_end| request.len() >= body_end))
        {
            break;
        }
    }
    let response = moderation_registry_http_response(service, &request, max_body_bytes);
    stream.write_all(&response)?;
    stream.flush()
}
fn moderation_registry_http_response(
    service: &ModerationRegistryService,
    request: &[u8],
    max_body_bytes: usize,
) -> Vec<u8> {
    match moderation_runner_parse_http_request(request, max_body_bytes) {
        Ok(parsed) => moderation_registry_route_request(service, &parsed),
        Err(response) => response,
    }
}
fn moderation_registry_route_request(
    service: &ModerationRegistryService,
    request: &ModerationRunnerHttpRequest<'_>,
) -> Vec<u8> {
    match (request.method, request.path) {
        ("GET", "/healthz") | ("GET", "/v1/sorafs/moderation/model-registry/status") => {
            match moderation_registry_status_json(service, "ready", None) {
                Ok(value) => moderation_registry_json_response(200, "OK", &value),
                Err(message) => {
                    moderation_registry_error_response(500, "Internal Server Error", &message)
                }
            }
        }
        ("GET", "/v1/sorafs/moderation/model-registry") => {
            match moderation_registry_snapshot_response_json(service) {
                Ok(value) => moderation_registry_json_response(200, "OK", &value),
                Err(message) => {
                    moderation_registry_error_response(500, "Internal Server Error", &message)
                }
            }
        }
        ("POST", "/v1/sorafs/moderation/model-registry/repro-manifests") => {
            match moderation_registry_admit_repro_request_json(service, request.body) {
                Ok(value) => moderation_registry_json_response(200, "OK", &value),
                Err(message) => moderation_registry_error_response(400, "Bad Request", &message),
            }
        }
        ("POST", "/v1/sorafs/moderation/model-registry/corpora") => {
            match moderation_registry_admit_corpus_request_json(service, request.body) {
                Ok(value) => moderation_registry_json_response(200, "OK", &value),
                Err(message) => moderation_registry_error_response(400, "Bad Request", &message),
            }
        }
        ("GET", _) | ("POST", _) => moderation_registry_error_response(
            404,
            "Not Found",
            "unknown SoraFS moderation model registry endpoint",
        ),
        _ => moderation_registry_error_response(
            405,
            "Method Not Allowed",
            "SoraFS moderation model registry supports GET and POST only",
        ),
    }
}
fn moderation_registry_admit_repro_request_json(
    service: &ModerationRegistryService,
    body: &[u8],
) -> Result<Value, String> {
    let bytes = moderation_registry_manifest_bytes_from_request(
        body,
        "moderation model registry reproducibility manifest admission",
    )?;
    let manifest = decode_from_bytes::<ModerationReproManifestV1>(&bytes)
        .map_err(|err| format!("failed to decode Norito reproducibility manifest: {err}"))?;
    let record = moderation_registry_repro_record_from_manifest(&manifest)?;
    let (record, created) = moderation_registry_insert_repro(service, record)?;
    Ok(moderation_registry_repro_admission_json(&record, created))
}
fn moderation_registry_admit_corpus_request_json(
    service: &ModerationRegistryService,
    body: &[u8],
) -> Result<Value, String> {
    let bytes = moderation_registry_manifest_bytes_from_request(
        body,
        "moderation model registry adversarial corpus admission",
    )?;
    let manifest = decode_from_bytes::<AdversarialCorpusManifestV1>(&bytes)
        .map_err(|err| format!("failed to decode Norito adversarial corpus manifest: {err}"))?;
    let record = moderation_registry_corpus_record_from_manifest(&manifest)?;
    let (record, created) = moderation_registry_insert_corpus(service, record)?;
    Ok(moderation_registry_corpus_admission_json(&record, created))
}
fn moderation_registry_manifest_bytes_from_request(
    body: &[u8],
    context: &str,
) -> Result<Vec<u8>, String> {
    if body.is_empty() {
        return Err(format!("{context} body must not be empty"));
    }
    let value: Value =
        from_slice(body).map_err(|err| format!("failed to parse {context} JSON: {err}"))?;
    if json_contains_key(&value, "payload_b64") {
        return Err(format!("{context} must not contain `payload_b64`"));
    }
    let fields = value
        .as_object()
        .ok_or_else(|| format!("{context} body must be a JSON object"))?;
    let manifest_b64 = required_json_string(fields, "manifest_b64", context)?;
    let bytes = BASE64_STANDARD
        .decode(manifest_b64)
        .map_err(|err| format!("{context} `manifest_b64` is not valid base64: {err}"))?;
    if bytes.is_empty() {
        return Err(format!("{context} `manifest_b64` decoded to empty bytes"));
    }
    Ok(bytes)
}
fn moderation_registry_repro_record_from_manifest(
    manifest: &ModerationReproManifestV1,
) -> Result<ModerationRegistryReproRecord, String> {
    let summary = manifest
        .validate()
        .map_err(|err| format!("reproducibility manifest validation failed: {err}"))?;
    Ok(ModerationRegistryReproRecord {
        manifest_id: summary.manifest_id,
        manifest_digest: manifest.body.manifest_digest,
        runner_hash: manifest.body.runner_hash,
        runtime_version: manifest.body.runtime_version.clone(),
        issued_at_unix: summary.issued_at_unix,
        model_count: summary.model_count,
        signer_count: summary.signer_count,
    })
}
fn moderation_registry_corpus_record_from_manifest(
    manifest: &AdversarialCorpusManifestV1,
) -> Result<ModerationRegistryCorpusRecord, String> {
    manifest
        .validate()
        .map_err(|err| format!("adversarial corpus manifest validation failed: {err}"))?;
    let encoded = to_bytes(manifest)
        .map_err(|err| format!("failed to encode adversarial corpus manifest: {err}"))?;
    let family_count = manifest.families.len().min(u32::MAX as usize) as u32;
    let variant_count = manifest
        .families
        .iter()
        .map(|family| family.variants.len())
        .sum::<usize>()
        .min(u32::MAX as usize) as u32;
    Ok(ModerationRegistryCorpusRecord {
        corpus_digest: *blake3_hash(&encoded).as_bytes(),
        issued_at_unix: manifest.issued_at_unix,
        cohort_label: manifest.cohort_label.clone(),
        family_count,
        variant_count,
    })
}
fn moderation_registry_insert_repro(
    service: &ModerationRegistryService,
    record: ModerationRegistryReproRecord,
) -> Result<(ModerationRegistryReproRecord, bool), String> {
    let mut state = service
        .state
        .lock()
        .map_err(|_| "moderation model registry state lock poisoned".to_string())?;
    match state
        .repro_manifests
        .iter()
        .position(|existing| existing.manifest_id == record.manifest_id)
    {
        Some(index) if state.repro_manifests[index] != record => Err(format!(
            "moderation reproducibility manifest `{}` conflicts with registry",
            hex_encode(record.manifest_id)
        )),
        Some(index) => Ok((state.repro_manifests[index].clone(), false)),
        None => {
            state.repro_manifests.push(record.clone());
            moderation_registry_normalize_snapshot(&mut state)?;
            moderation_registry_save_state(&service.state_path, &state)?;
            Ok((record, true))
        }
    }
}
fn moderation_registry_insert_corpus(
    service: &ModerationRegistryService,
    record: ModerationRegistryCorpusRecord,
) -> Result<(ModerationRegistryCorpusRecord, bool), String> {
    let mut state = service
        .state
        .lock()
        .map_err(|_| "moderation model registry state lock poisoned".to_string())?;
    match state
        .adversarial_corpora
        .iter()
        .position(|existing| existing.corpus_digest == record.corpus_digest)
    {
        Some(index) => Ok((state.adversarial_corpora[index].clone(), false)),
        None => {
            state.adversarial_corpora.push(record.clone());
            moderation_registry_normalize_snapshot(&mut state)?;
            moderation_registry_save_state(&service.state_path, &state)?;
            Ok((record, true))
        }
    }
}
fn moderation_registry_status_json(
    service: &ModerationRegistryService,
    status: &str,
    listen: Option<&str>,
) -> Result<Value, String> {
    let state = service
        .state
        .lock()
        .map_err(|_| "moderation model registry state lock poisoned".to_string())?;
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.model_registry.service_status.v1");
    insert_value!(output["status"] = status.to_string());
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["state_path"] = service.state_path.display().to_string());
    insert_value!(output["state_digest_hex"] = moderation_registry_state_digest_hex(&state)?);
    insert_value!(output["repro_manifest_count"] = state.repro_manifests.len() as u64);
    insert_value!(output["corpus_count"] = state.adversarial_corpora.len() as u64);
    insert_value!(output["max_body_bytes"] = service.max_body_bytes as u64);
    insert_value!(output["snapshot_limit"] = service.snapshot_limit as u64);
    insert_value!(output["outbound_network"] = "disabled");
    insert_json!(
        output["listen"] = listen
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    Ok(Value::Object(output))
}
fn moderation_registry_snapshot_response_json(
    service: &ModerationRegistryService,
) -> Result<Value, String> {
    let state = service
        .state
        .lock()
        .map_err(|_| "moderation model registry state lock poisoned".to_string())?;
    moderation_registry_snapshot_json(&state, service.snapshot_limit)
}
fn moderation_registry_state_digest_hex(
    state: &ModerationRegistryPersistedSnapshot,
) -> Result<String, String> {
    let bytes = to_bytes(state)
        .map_err(|err| format!("failed to encode model registry state for digest: {err}"))?;
    Ok(hex_encode(blake3_hash(&bytes).as_bytes()))
}
fn moderation_registry_snapshot_json(
    state: &ModerationRegistryPersistedSnapshot,
    limit: usize,
) -> Result<Value, String> {
    let repro_count = state.repro_manifests.len();
    let corpus_count = state.adversarial_corpora.len();
    let repro_manifests = state
        .repro_manifests
        .iter()
        .take(limit)
        .map(moderation_registry_repro_record_json)
        .collect::<Vec<_>>();
    let adversarial_corpora = state
        .adversarial_corpora
        .iter()
        .take(limit)
        .map(moderation_registry_corpus_record_json)
        .collect::<Vec<_>>();
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.model_registry.snapshot.v1");
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["state_digest_hex"] = moderation_registry_state_digest_hex(state)?);
    insert_value!(output["repro_manifest_count"] = repro_count as u64);
    insert_value!(output["returned_repro_manifest_count"] = repro_manifests.len() as u64);
    insert_value!(output["truncated_repro_manifests"] = repro_count > limit);
    insert_value!(output["corpus_count"] = corpus_count as u64);
    insert_value!(output["returned_corpus_count"] = adversarial_corpora.len() as u64);
    insert_value!(output["truncated_corpora"] = corpus_count > limit);
    insert_value!(output["limit"] = limit as u64);
    insert_json!(output["repro_manifests"] = Value::Array(repro_manifests));
    insert_json!(output["adversarial_corpora"] = Value::Array(adversarial_corpora));
    Ok(Value::Object(output))
}
fn moderation_registry_repro_admission_json(
    record: &ModerationRegistryReproRecord,
    created: bool,
) -> Value {
    let mut output = Map::new();
    insert_value!(
        output["schema"] = "sorafs.moderation.model_registry.repro_manifest_admission.v1"
    );
    insert_value!(output["status"] = "accepted");
    insert_value!(output["created"] = created);
    insert_json!(output["record"] = moderation_registry_repro_record_json(record));
    Value::Object(output)
}
fn moderation_registry_corpus_admission_json(
    record: &ModerationRegistryCorpusRecord,
    created: bool,
) -> Value {
    let mut output = Map::new();
    insert_value!(
        output["schema"] = "sorafs.moderation.model_registry.corpus_manifest_admission.v1"
    );
    insert_value!(output["status"] = "accepted");
    insert_value!(output["created"] = created);
    insert_json!(output["record"] = moderation_registry_corpus_record_json(record));
    Value::Object(output)
}
fn moderation_registry_repro_record_json(record: &ModerationRegistryReproRecord) -> Value {
    let mut output = Map::new();
    insert_value!(output["manifest_id_hex"] = hex_encode(record.manifest_id));
    insert_value!(output["manifest_digest_hex"] = hex_encode(record.manifest_digest));
    insert_value!(output["runner_hash_hex"] = hex_encode(record.runner_hash));
    insert_value!(output["runtime_version"] = record.runtime_version.clone());
    insert_value!(output["issued_at_unix"] = record.issued_at_unix);
    insert_value!(output["model_count"] = u64::from(record.model_count));
    insert_value!(output["signer_count"] = u64::from(record.signer_count));
    Value::Object(output)
}
fn moderation_registry_corpus_record_json(record: &ModerationRegistryCorpusRecord) -> Value {
    let mut output = Map::new();
    insert_value!(output["corpus_digest_hex"] = hex_encode(record.corpus_digest));
    insert_value!(output["issued_at_unix"] = record.issued_at_unix);
    insert_json!(
        output["cohort_label"] = record
            .cohort_label
            .as_deref()
            .map(Value::from)
            .unwrap_or(Value::Null)
    );
    insert_value!(output["family_count"] = u64::from(record.family_count));
    insert_value!(output["variant_count"] = u64::from(record.variant_count));
    Value::Object(output)
}
fn moderation_registry_json_response(status: u16, reason: &str, value: &Value) -> Vec<u8> {
    let body = to_vec(value).unwrap_or_else(|_| b"{\"error\":\"json_render_failed\"}".to_vec());
    moderation_runner_http_response_bytes(status, reason, "application/json", &body)
}
fn moderation_registry_error_response(status: u16, reason: &str, message: &str) -> Vec<u8> {
    let mut body = Map::new();
    insert_value!(body["schema"] = "sorafs.moderation.model_registry.error.v1");
    insert_value!(body["status"] = "error");
    insert_value!(body["message"] = message.to_string());
    moderation_registry_json_response(status, reason, &Value::Object(body))
}
fn moderation_run_local(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut artifact_root: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut payload_path: Option<PathBuf> = None;
    let mut subject: Option<String> = None;
    let mut screened_at_unix: Option<u64> = None;
    let mut notes: Option<String> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut max_payload_bytes = MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--artifact-root" => artifact_root = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--subject" => {
                validate_moderation_request_text(
                    value,
                    MODERATION_RUNNER_MAX_SUBJECT_BYTES,
                    "`--subject`",
                )?;
                subject = Some(value.to_string());
            }
            "--screened-at" => {
                screened_at_unix = Some(parse_u64_arg(
                    "--screened-at",
                    value,
                    "sorafs_cli moderation run-local",
                )?);
            }
            "--notes" => {
                validate_moderation_request_text(
                    value,
                    MODERATION_RUNNER_MAX_NOTES_BYTES,
                    "`--notes`",
                )?;
                notes = Some(value.to_string());
            }
            "--json-out" => json_out = Some(PathBuf::from(value)),
            "--max-payload-bytes" => {
                max_payload_bytes =
                    parse_moderation_max_payload_bytes(value, "sorafs_cli moderation run-local")?;
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation run-local`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation run-local`".to_string()
    })?;
    let payload_path = payload_path.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli moderation run-local`".to_string()
    })?;
    let artifact_root = artifact_root.ok_or_else(|| {
        "missing required `--artifact-root=DIR` for `sorafs_cli moderation run-local`".to_string()
    })?;
    let subject = subject.ok_or_else(|| {
        "missing required `--subject=ID` for `sorafs_cli moderation run-local`".to_string()
    })?;
    let screened_at_unix = screened_at_unix.ok_or_else(|| {
        "missing required `--screened-at=UNIX_SECS` for `sorafs_cli moderation run-local`"
            .to_string()
    })?;
    if screened_at_unix == 0 {
        return Err("`--screened-at` must be greater than zero".to_string());
    }
    let manifest =
        load_moderation_repro_manifest(&manifest_path, &format, "sorafs_cli moderation run-local")?;
    let runner = load_moderation_runner_for_current_executable(manifest, &artifact_root)?;
    let payload = read_file_bounded(
        &payload_path,
        u64::from(max_payload_bytes),
        "moderation payload",
    )?;
    if payload.is_empty() {
        return Err("`--payload` file must not be empty".to_string());
    }
    let output = moderation_local_runner_screening_json(
        &runner,
        &payload,
        &subject,
        screened_at_unix,
        notes.as_deref(),
        max_payload_bytes,
    )?;
    let rendered = to_string_pretty(&output)
        .map_err(|err| format!("failed to render local runner JSON: {err}"))?;
    if let Some(path) = json_out {
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
fn moderation_run_signed_local(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let context = "sorafs_cli moderation run-signed-local";
    let mut manifest_path = None;
    let mut artifact_root = None;
    let mut format = String::from("json");
    let mut trust_policy_path = None;
    let mut trust_policy_format = String::from("norito");
    let mut trust_anchors = BTreeSet::new();
    let mut minimum_governance_quorum = None;
    let mut signing_key_path = None;
    let mut payload_path = None;
    let mut subject = None;
    let mut provenance_path = None;
    let mut provenance_log_id = None;
    let mut max_payload_bytes = MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES;
    let mut notes = None;
    let mut norito_out = None;
    let mut json_out = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--artifact-root" => artifact_root = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--trust-policy" => trust_policy_path = Some(PathBuf::from(value)),
            "--trust-policy-format" => trust_policy_format = value.to_ascii_lowercase(),
            "--trust-anchor" => {
                let anchor = parse_moderation_trust_anchor(value, context)?;
                if !trust_anchors.insert(anchor) {
                    return Err("duplicate `--trust-anchor` is forbidden".to_string());
                }
            }
            "--minimum-governance-quorum" => {
                let parsed = parse_u64_arg(key, value, context)?;
                let parsed = u16::try_from(parsed)
                    .map_err(|_| "`--minimum-governance-quorum` exceeds u16".to_string())?;
                if parsed == 0 {
                    return Err(
                        "`--minimum-governance-quorum` must be greater than zero".to_string()
                    );
                }
                minimum_governance_quorum = Some(parsed);
            }
            "--signing-key" => signing_key_path = Some(PathBuf::from(value)),
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--subject" => {
                validate_moderation_request_text(
                    value,
                    MODERATION_RUNNER_MAX_SUBJECT_BYTES,
                    "signed moderation `--subject`",
                )?;
                subject = Some(value.to_string());
            }
            "--provenance" => provenance_path = Some(PathBuf::from(value)),
            "--provenance-log-id" => {
                provenance_log_id = Some(parse_fixed_hex::<16>(
                    value,
                    "--provenance-log-id",
                    context,
                )?);
            }
            "--max-payload-bytes" => {
                max_payload_bytes = parse_moderation_max_payload_bytes(value, context)?;
            }
            "--notes" => {
                validate_moderation_request_text(
                    value,
                    MODERATION_RUNNER_MAX_NOTES_BYTES,
                    "signed moderation `--notes`",
                )?;
                notes = Some(value.to_string());
            }
            "--norito-out" => norito_out = Some(PathBuf::from(value)),
            "--json-out" => json_out = Some(PathBuf::from(value)),
            _ => return Err(format!("unrecognised option `{key}` for `{context}`")),
        }
    }
    let manifest_path = manifest_path
        .ok_or_else(|| format!("missing required `--manifest=PATH` for `{context}`"))?;
    let artifact_root = artifact_root
        .ok_or_else(|| format!("missing required `--artifact-root=DIR` for `{context}`"))?;
    let trust_policy_path = trust_policy_path
        .ok_or_else(|| format!("missing required `--trust-policy=PATH` for `{context}`"))?;
    if trust_anchors.is_empty() {
        return Err(format!(
            "provide at least one external `--trust-anchor=PUBLIC_KEY` for `{context}`"
        ));
    }
    let minimum_governance_quorum = minimum_governance_quorum.ok_or_else(|| {
        format!("missing required `--minimum-governance-quorum=N` for `{context}`")
    })?;
    let signing_key_path = signing_key_path
        .ok_or_else(|| format!("missing required `--signing-key=PATH` for `{context}`"))?;
    let payload_path =
        payload_path.ok_or_else(|| format!("missing required `--payload=PATH` for `{context}`"))?;
    let subject =
        subject.ok_or_else(|| format!("missing required `--subject=ID` for `{context}`"))?;
    let provenance_path = provenance_path
        .ok_or_else(|| format!("missing required `--provenance=PATH` for `{context}`"))?;
    let provenance_log_id = provenance_log_id
        .ok_or_else(|| format!("missing required `--provenance-log-id=HEX16` for `{context}`"))?;
    if provenance_log_id == [0; 16] {
        return Err("`--provenance-log-id` must be non-zero".to_string());
    }
    let now_unix = moderation_trusted_now_unix()?;
    let manifest = load_moderation_repro_manifest(&manifest_path, &format, context)?;
    let policy = load_moderation_trust_policy(&trust_policy_path, &trust_policy_format, context)?;
    let signing_key = load_moderation_signing_key(&signing_key_path, context)?;
    let runner = load_moderation_runner_for_current_executable(manifest.clone(), &artifact_root)?;
    let signing_runner = LoadedModerationSigningRunnerV1::from_verified(
        runner,
        policy.clone(),
        trust_anchors.clone(),
        minimum_governance_quorum,
        signing_key,
        now_unix,
    )
    .map_err(|error| format!("failed to initialize signed moderation runner: {error}"))?;
    let payload = read_file_bounded(
        &payload_path,
        u64::from(max_payload_bytes),
        "signed moderation payload",
    )?;
    if payload.is_empty() {
        return Err("signed moderation payload must not be empty".to_string());
    }
    let result = signing_runner
        .screen_signed(&payload, max_payload_bytes, &subject, notes, now_unix)
        .map_err(|error| format!("signed moderation screening failed: {error}"))?;
    ensure_parent_dir(&provenance_path)?;
    let provenance = ModerationProvenanceStoreV1::open(&provenance_path, provenance_log_id)
        .map_err(|error| format!("failed to open moderation provenance: {error}"))?;
    let provenance_head = provenance
        .append_signed_result(
            &manifest,
            &policy,
            &trust_anchors,
            minimum_governance_quorum,
            result.clone(),
            now_unix,
        )
        .map_err(|error| format!("failed to persist signed moderation result: {error}"))?;
    let canonical = to_bytes(&result)
        .map_err(|error| format!("failed to encode signed moderation result: {error}"))?;
    if u64::try_from(canonical.len())
        .ok()
        .is_none_or(|length| length > MODERATION_SIGNED_RESULT_MAX_BYTES)
    {
        return Err("signed moderation result exceeds its hard encoded bound".to_string());
    }
    if let Some(path) = norito_out {
        write_bytes(&path, &canonical)?;
    }
    let output = moderation_signed_result_summary_json(&result, &canonical, provenance_head)?;
    let rendered = to_string_pretty(&output)
        .map_err(|error| format!("failed to render signed moderation summary: {error}"))?;
    if let Some(path) = json_out {
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
fn moderation_trusted_now_unix() -> Result<u64, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_secs();
    if now == 0 {
        return Err("trusted system clock returned zero Unix time".to_string());
    }
    Ok(now)
}
fn moderation_runner_serve(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut artifact_root: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut listen = String::from(MODERATION_RUNNER_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    let mut max_payload_bytes = MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--artifact-root" => artifact_root = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--listen" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--listen` must not be empty".to_string());
                }
                listen = trimmed.to_string();
            }
            "--max-body-bytes" => {
                max_body_bytes =
                    parse_moderation_max_body_bytes(value, "sorafs_cli moderation runner-serve")?;
            }
            "--max-payload-bytes" => {
                max_payload_bytes = parse_moderation_max_payload_bytes(
                    value,
                    "sorafs_cli moderation runner-serve",
                )?;
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation runner-serve`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation runner-serve`".to_string()
    })?;
    let artifact_root = artifact_root.ok_or_else(|| {
        "missing required `--artifact-root=DIR` for `sorafs_cli moderation runner-serve`"
            .to_string()
    })?;
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation runner-serve",
    )?;
    let runner = load_moderation_runner_for_current_executable(manifest, &artifact_root)?;
    let listen_addr =
        validate_moderation_loopback_listen(&listen, "sorafs_cli moderation runner-serve")?;
    let listener = TcpListener::bind(listen_addr)
        .map_err(|err| format!("failed to bind moderation runner service at `{listen}`: {err}"))?;
    let local_addr = listener
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| listen.clone());
    let service = Arc::new(ModerationRunnerService {
        runner,
        signed: None,
        manifest_source: manifest_path.display().to_string(),
        max_body_bytes,
        max_payload_bytes,
    });
    let status = moderation_runner_status_json(&service, "listening", Some(&local_addr));
    let rendered = to_string_pretty(&status)
        .map_err(|err| format!("failed to render runner service status JSON: {err}"))?;
    println!("{rendered}");
    let active_connections = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let Some(active_permit) = moderation_try_acquire_permit(
                    &active_connections,
                    MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS,
                ) else {
                    let response = moderation_runner_error_response(
                        503,
                        "Service Unavailable",
                        "moderation runner connection limit reached",
                    );
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    continue;
                };
                let service = Arc::clone(&service);
                thread::spawn(move || {
                    let _active_permit = active_permit;
                    if let Err(err) =
                        moderation_runner_handle_stream(stream, &service, max_body_bytes)
                    {
                        eprintln!("sorafs moderation runner connection failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("sorafs moderation runner accept failed: {err}"),
        }
    }
    Ok(())
}
fn moderation_runner_signed_serve(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let context = "sorafs_cli moderation runner-signed-serve";
    let mut manifest_path = None;
    let mut artifact_root = None;
    let mut format = String::from("json");
    let mut trust_policy_path = None;
    let mut trust_policy_format = String::from("norito");
    let mut trust_anchors = BTreeSet::new();
    let mut minimum_governance_quorum = None;
    let mut signing_key_path = None;
    let mut provenance_path = None;
    let mut provenance_log_id = None;
    let mut listen = String::from(MODERATION_RUNNER_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    let mut max_payload_bytes = MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--artifact-root" => artifact_root = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--trust-policy" => trust_policy_path = Some(PathBuf::from(value)),
            "--trust-policy-format" => trust_policy_format = value.to_ascii_lowercase(),
            "--trust-anchor" => {
                let anchor = parse_moderation_trust_anchor(value, context)?;
                if !trust_anchors.insert(anchor) {
                    return Err("duplicate `--trust-anchor` is forbidden".to_string());
                }
            }
            "--minimum-governance-quorum" => {
                let parsed = parse_u64_arg(key, value, context)?;
                let parsed = u16::try_from(parsed)
                    .map_err(|_| "`--minimum-governance-quorum` exceeds u16".to_string())?;
                if parsed == 0 {
                    return Err(
                        "`--minimum-governance-quorum` must be greater than zero".to_string()
                    );
                }
                minimum_governance_quorum = Some(parsed);
            }
            "--signing-key" => signing_key_path = Some(PathBuf::from(value)),
            "--provenance" => provenance_path = Some(PathBuf::from(value)),
            "--provenance-log-id" => {
                provenance_log_id = Some(parse_fixed_hex::<16>(
                    value,
                    "--provenance-log-id",
                    context,
                )?);
            }
            "--listen" => {
                if value.trim().is_empty() {
                    return Err("`--listen` must not be empty".to_string());
                }
                listen = value.to_string();
            }
            "--max-body-bytes" => max_body_bytes = parse_moderation_max_body_bytes(value, context)?,
            "--max-payload-bytes" => {
                max_payload_bytes = parse_moderation_max_payload_bytes(value, context)?;
            }
            _ => return Err(format!("unrecognised option `{key}` for `{context}`")),
        }
    }
    let manifest_path = manifest_path
        .ok_or_else(|| format!("missing required `--manifest=PATH` for `{context}`"))?;
    let artifact_root = artifact_root
        .ok_or_else(|| format!("missing required `--artifact-root=DIR` for `{context}`"))?;
    let trust_policy_path = trust_policy_path
        .ok_or_else(|| format!("missing required `--trust-policy=PATH` for `{context}`"))?;
    if trust_anchors.is_empty() {
        return Err(format!(
            "provide at least one external `--trust-anchor=PUBLIC_KEY` for `{context}`"
        ));
    }
    let minimum_governance_quorum = minimum_governance_quorum.ok_or_else(|| {
        format!("missing required `--minimum-governance-quorum=N` for `{context}`")
    })?;
    let signing_key_path = signing_key_path
        .ok_or_else(|| format!("missing required `--signing-key=PATH` for `{context}`"))?;
    let provenance_path = provenance_path
        .ok_or_else(|| format!("missing required `--provenance=PATH` for `{context}`"))?;
    let provenance_log_id = provenance_log_id
        .ok_or_else(|| format!("missing required `--provenance-log-id=HEX16` for `{context}`"))?;
    if provenance_log_id == [0; 16] {
        return Err("`--provenance-log-id` must be non-zero".to_string());
    }
    let listen_addr = validate_moderation_loopback_listen(&listen, context)?;
    let now_unix = moderation_trusted_now_unix()?;
    let manifest = load_moderation_repro_manifest(&manifest_path, &format, context)?;
    let policy = load_moderation_trust_policy(&trust_policy_path, &trust_policy_format, context)?;
    let signing_key = load_moderation_signing_key(&signing_key_path, context)?;
    let runner = load_moderation_runner_for_current_executable(manifest, &artifact_root)?;
    let signing_runner = LoadedModerationSigningRunnerV1::from_verified(
        runner.clone(),
        policy,
        trust_anchors.clone(),
        minimum_governance_quorum,
        signing_key,
        now_unix,
    )
    .map_err(|error| format!("failed to initialize signed moderation runner: {error}"))?;
    ensure_parent_dir(&provenance_path)?;
    let provenance = ModerationProvenanceStoreV1::open(&provenance_path, provenance_log_id)
        .map_err(|error| format!("failed to open moderation provenance: {error}"))?;
    let listener = TcpListener::bind(listen_addr).map_err(|error| {
        format!("failed to bind signed moderation runner at `{listen}`: {error}")
    })?;
    let local_addr = listener
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_else(|_| listen.clone());
    let service = Arc::new(ModerationRunnerService {
        runner,
        signed: Some(ModerationSignedRunnerState {
            signing_runner,
            provenance,
            trust_anchors,
            minimum_governance_quorum,
            transaction_guard: Mutex::new(()),
        }),
        manifest_source: manifest_path.display().to_string(),
        max_body_bytes,
        max_payload_bytes,
    });
    let status = moderation_runner_status_json(&service, "listening", Some(&local_addr));
    let rendered = to_string_pretty(&status)
        .map_err(|error| format!("failed to render signed runner status: {error}"))?;
    println!("{rendered}");
    let active_connections = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let Some(active_permit) = moderation_try_acquire_permit(
                    &active_connections,
                    MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS,
                ) else {
                    let response = moderation_runner_error_response(
                        503,
                        "Service Unavailable",
                        "signed moderation runner connection limit reached",
                    );
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    continue;
                };
                let service = Arc::clone(&service);
                thread::spawn(move || {
                    let _active_permit = active_permit;
                    if let Err(error) =
                        moderation_runner_handle_stream(stream, &service, max_body_bytes)
                    {
                        eprintln!("signed moderation runner connection failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("signed moderation runner accept failed: {error}"),
        }
    }
    Ok(())
}
struct ModerationActivePermit {
    active: Arc<AtomicUsize>,
}
impl Drop for ModerationActivePermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, AtomicOrdering::AcqRel);
    }
}
fn moderation_try_acquire_permit(
    active: &Arc<AtomicUsize>,
    limit: usize,
) -> Option<ModerationActivePermit> {
    active
        .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |current| {
            (current < limit).then_some(current + 1)
        })
        .ok()?;
    Some(ModerationActivePermit {
        active: Arc::clone(active),
    })
}
fn moderation_runner_grpc_serve(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut artifact_root: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut listen = String::from(MODERATION_RUNNER_GRPC_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    let mut max_payload_bytes = MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--artifact-root" => artifact_root = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--listen" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--listen` must not be empty".to_string());
                }
                listen = trimmed.to_string();
            }
            "--max-body-bytes" => {
                max_body_bytes = parse_moderation_max_body_bytes(
                    value,
                    "sorafs_cli moderation runner-grpc-serve",
                )?;
            }
            "--max-payload-bytes" => {
                max_payload_bytes = parse_moderation_max_payload_bytes(
                    value,
                    "sorafs_cli moderation runner-grpc-serve",
                )?;
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation runner-grpc-serve`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation runner-grpc-serve`"
            .to_string()
    })?;
    let artifact_root = artifact_root.ok_or_else(|| {
        "missing required `--artifact-root=DIR` for `sorafs_cli moderation runner-grpc-serve`"
            .to_string()
    })?;
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation runner-grpc-serve",
    )?;
    let runner = load_moderation_runner_for_current_executable(manifest, &artifact_root)?;
    let addr =
        validate_moderation_loopback_listen(&listen, "sorafs_cli moderation runner-grpc-serve")?;
    let service = Arc::new(ModerationRunnerService {
        runner,
        signed: None,
        manifest_source: manifest_path.display().to_string(),
        max_body_bytes,
        max_payload_bytes,
    });
    let status = moderation_runner_status_json(&service, "listening", Some(&listen));
    let rendered = to_string_pretty(&status)
        .map_err(|err| format!("failed to render runner gRPC service status JSON: {err}"))?;
    println!("{rendered}");
    let max_decoding_message_size = max_body_bytes
        .checked_add(MODERATION_RUNNER_MAX_GRPC_ENVELOPE_BYTES)
        .ok_or_else(|| "runner gRPC decoding limit overflows usize".to_owned())?;
    let handler = ModerationRunnerGrpcHandler {
        service,
        listen: listen.clone(),
        in_flight: Arc::new(AtomicUsize::new(0)),
    };
    let grpc_service = moderation_runner_grpc::runner_server::RunnerServer::new(handler)
        .max_decoding_message_size(max_decoding_message_size)
        .max_encoding_message_size(MODERATION_RUNNER_MAX_GRPC_RESPONSE_BYTES);
    Runtime::new()
        .map_err(|err| format!("failed to start Tokio runtime for runner gRPC: {err}"))?
        .block_on(async move {
            tonic::transport::Server::builder()
                .concurrency_limit_per_connection(MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT)
                .add_service(grpc_service)
                .serve(addr)
                .await
                .map_err(|err| format!("runner gRPC service failed: {err}"))
        })
}
fn moderation_runner_bundle(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut artifact_root: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut bundle_out: Option<PathBuf> = None;
    let mut listen = String::from(MODERATION_RUNNER_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    let mut max_payload_bytes = MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES;
    let mut binary = String::from("sorafs_cli");
    let mut service_name = String::from("sorafs-moderation-runner");
    let mut service_user = String::from("sorafs");
    let mut service_group = String::from("sorafs");
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--artifact-root" => artifact_root = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--bundle-out" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--bundle-out` must not be empty".to_string());
                }
                bundle_out = Some(PathBuf::from(trimmed));
            }
            "--listen" => {
                let trimmed = validate_runner_bundle_value("--listen", value)?;
                listen = trimmed.to_string();
            }
            "--max-body-bytes" => {
                max_body_bytes =
                    parse_moderation_max_body_bytes(value, "sorafs_cli moderation runner-bundle")?;
            }
            "--max-payload-bytes" => {
                max_payload_bytes = parse_moderation_max_payload_bytes(
                    value,
                    "sorafs_cli moderation runner-bundle",
                )?;
            }
            "--binary" => {
                let trimmed = validate_runner_bundle_value("--binary", value)?;
                binary = trimmed.to_string();
            }
            "--service-name" => {
                let trimmed = validate_runner_bundle_value("--service-name", value)?;
                validate_runner_bundle_service_name(trimmed)?;
                service_name = trimmed.to_string();
            }
            "--service-user" => {
                let trimmed = validate_runner_bundle_value("--service-user", value)?;
                service_user = trimmed.to_string();
            }
            "--service-group" => {
                let trimmed = validate_runner_bundle_value("--service-group", value)?;
                service_group = trimmed.to_string();
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation runner-bundle`"
                ));
            }
        }
    }
    validate_runner_bundle_service_name(&service_name)?;
    validate_moderation_loopback_listen(&listen, "sorafs_cli moderation runner-bundle")?;
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation runner-bundle`".to_string()
    })?;
    let bundle_out = bundle_out.ok_or_else(|| {
        "missing required `--bundle-out=DIR` for `sorafs_cli moderation runner-bundle`".to_string()
    })?;
    let artifact_root = artifact_root.ok_or_else(|| {
        "missing required `--artifact-root=DIR` for `sorafs_cli moderation runner-bundle`"
            .to_string()
    })?;
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation runner-bundle",
    )?;
    let verified_runner =
        load_moderation_runner_for_current_executable(manifest.clone(), &artifact_root)?;
    let verified_artifacts = verified_runner
        .canonical_artifacts()
        .map_err(|error| format!("failed to prepare verified bundle artefacts: {error}"))?;
    write_moderation_runner_bundle(ModerationRunnerBundleSpec {
        manifest,
        manifest_source: manifest_path,
        verified_artifacts,
        manifest_format: format,
        bundle_out,
        listen,
        max_body_bytes,
        max_payload_bytes,
        binary,
        service_name,
        service_user,
        service_group,
    })
}
struct ModerationRunnerBundleSpec {
    manifest: ModerationReproManifestV1,
    manifest_source: PathBuf,
    verified_artifacts: Vec<(String, Vec<u8>)>,
    manifest_format: String,
    bundle_out: PathBuf,
    listen: String,
    max_body_bytes: usize,
    max_payload_bytes: u32,
    binary: String,
    service_name: String,
    service_user: String,
    service_group: String,
}
fn encode_moderation_manifest_for_bundle(
    manifest: &ModerationReproManifestV1,
    format: &str,
    context: &str,
) -> Result<Vec<u8>, String> {
    match format {
        "json" => norito::json::to_json_pretty(manifest)
            .map(String::into_bytes)
            .map_err(|error| format!("failed to encode {context} JSON manifest: {error}")),
        "norito" => norito::to_bytes(manifest)
            .map_err(|error| format!("failed to encode {context} Norito manifest: {error}")),
        other => Err(format!(
            "unsupported manifest format `{other}` while encoding {context}"
        )),
    }
}
fn write_moderation_runner_bundle(spec: ModerationRunnerBundleSpec) -> Result<(), String> {
    fs::create_dir_all(&spec.bundle_out).map_err(|err| {
        format!(
            "failed to create runner bundle directory `{}`: {err}",
            spec.bundle_out.display()
        )
    })?;
    let bundle_dir = spec
        .bundle_out
        .canonicalize()
        .unwrap_or_else(|_| spec.bundle_out.clone());
    let manifest_copy_name = match spec.manifest_format.as_str() {
        "json" => "manifest.json",
        "norito" => "manifest.to",
        other => {
            return Err(format!(
                "unsupported `--format={other}` for `sorafs_cli moderation runner-bundle` (expected `json` or `norito`)"
            ));
        }
    };
    let manifest_copy_path = bundle_dir.join(manifest_copy_name);
    let manifest_bytes = encode_moderation_manifest_for_bundle(
        &spec.manifest,
        &spec.manifest_format,
        "runner bundle",
    )?;
    write_text(&manifest_copy_path, &manifest_bytes)?;
    let artifact_copy_root = bundle_dir.join("artifacts");
    fs::create_dir_all(&artifact_copy_root).map_err(|error| {
        format!(
            "failed to create runner artefact directory `{}`: {error}",
            artifact_copy_root.display()
        )
    })?;
    for (artifact_path, bytes) in &spec.verified_artifacts {
        let destination = artifact_copy_root.join(artifact_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create bundled model directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }
        write_text(&destination, bytes)?;
    }
    let observed_runner_hash = moderation_runner_current_executable_hash()?;
    LoadedModerationRunnerV1::load_verified(
        spec.manifest.clone(),
        &artifact_copy_root,
        observed_runner_hash,
    )
    .map_err(|error| format!("bundled moderation artefacts failed verification: {error}"))?;
    let env_path = bundle_dir.join("runner.env");
    let run_path = bundle_dir.join("run.sh");
    let systemd_unit_name = format!("{}.service", spec.service_name);
    let systemd_path = bundle_dir.join(&systemd_unit_name);
    let launchd_plist_name = format!("{}.plist", spec.service_name);
    let launchd_path = bundle_dir.join(&launchd_plist_name);
    let metadata_path = bundle_dir.join("bundle.json");
    let readme_path = bundle_dir.join("README.md");
    let env = moderation_runner_bundle_env(&spec);
    write_text(&env_path, env.as_bytes())?;
    let run_script = moderation_runner_bundle_run_script(manifest_copy_name, &spec.manifest_format);
    write_text(&run_path, run_script.as_bytes())?;
    set_executable_if_supported(&run_path)?;
    let systemd = moderation_runner_bundle_systemd_unit(&spec, &bundle_dir, &run_path, &env_path);
    write_text(&systemd_path, systemd.as_bytes())?;
    let launchd = moderation_runner_bundle_launchd_plist(&spec, &bundle_dir, &run_path);
    write_text(&launchd_path, launchd.as_bytes())?;
    let readme = moderation_runner_bundle_readme(
        &spec,
        manifest_copy_name,
        &systemd_unit_name,
        &launchd_plist_name,
    );
    write_text(&readme_path, readme.as_bytes())?;
    let summary = moderation_runner_bundle_summary_json(
        &spec,
        &bundle_dir,
        manifest_copy_name,
        &[
            manifest_copy_name,
            "artifacts/",
            "runner.env",
            "run.sh",
            &systemd_unit_name,
            &launchd_plist_name,
            "bundle.json",
            "README.md",
        ],
    );
    let rendered = to_string_pretty(&summary)
        .map_err(|err| format!("failed to render runner bundle summary JSON: {err}"))?;
    write_text(&metadata_path, format!("{rendered}\n").as_bytes())?;
    println!("{rendered}");
    Ok(())
}
fn moderation_runner_bundle_env(spec: &ModerationRunnerBundleSpec) -> String {
    format!(
        "SORAFS_CLI={}\nSORAFS_RUNNER_LISTEN={}\nSORAFS_RUNNER_MAX_BODY_BYTES={}\nSORAFS_RUNNER_MAX_PAYLOAD_BYTES={}\n",
        shell_single_quote(&spec.binary),
        shell_single_quote(&spec.listen),
        shell_single_quote(&spec.max_body_bytes.to_string()),
        shell_single_quote(&spec.max_payload_bytes.to_string())
    )
}
fn moderation_runner_bundle_run_script(manifest_copy_name: &str, format: &str) -> String {
    format!(
        "#!/usr/bin/env sh\nset -eu\nSCRIPT_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nif [ -f \"$SCRIPT_DIR/runner.env\" ]; then\n  . \"$SCRIPT_DIR/runner.env\"\nfi\n: \"${{SORAFS_CLI:=sorafs_cli}}\"\n: \"${{SORAFS_RUNNER_LISTEN:={}}}\"\n: \"${{SORAFS_RUNNER_MAX_BODY_BYTES:={}}}\"\n: \"${{SORAFS_RUNNER_MAX_PAYLOAD_BYTES:={}}}\"\nexec \"$SORAFS_CLI\" moderation runner-serve \\\n  --manifest=\"$SCRIPT_DIR/{}\" \\\n  --artifact-root=\"$SCRIPT_DIR/artifacts\" \\\n  --format={} \\\n  --listen=\"$SORAFS_RUNNER_LISTEN\" \\\n  --max-body-bytes=\"$SORAFS_RUNNER_MAX_BODY_BYTES\" \\\n  --max-payload-bytes=\"$SORAFS_RUNNER_MAX_PAYLOAD_BYTES\"\n",
        MODERATION_RUNNER_DEFAULT_LISTEN,
        MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES,
        MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES,
        manifest_copy_name,
        format
    )
}
fn moderation_runner_bundle_systemd_unit(
    spec: &ModerationRunnerBundleSpec,
    bundle_dir: &Path,
    run_path: &Path,
    env_path: &Path,
) -> String {
    format!(
        "[Unit]\nDescription=SoraFS moderation runner ({})\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=simple\nUser={}\nGroup={}\nWorkingDirectory={}\nEnvironmentFile={}\nExecStart={}\nRestart=on-failure\nRestartSec=5s\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectControlGroups=true\nMemoryDenyWriteExecute=true\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\nIPAddressDeny=any\nIPAddressAllow=localhost\nReadOnlyPaths={}\n\n[Install]\nWantedBy=multi-user.target\n",
        spec.service_name,
        spec.service_user,
        spec.service_group,
        systemd_quote(&bundle_dir.display().to_string()),
        systemd_quote(&env_path.display().to_string()),
        systemd_quote(&run_path.display().to_string()),
        systemd_quote(&bundle_dir.display().to_string())
    )
}
fn moderation_runner_bundle_launchd_plist(
    spec: &ModerationRunnerBundleSpec,
    bundle_dir: &Path,
    run_path: &Path,
) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n  </array>\n  <key>WorkingDirectory</key>\n  <string>{}</string>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <true/>\n  <key>StandardOutPath</key>\n  <string>{}</string>\n  <key>StandardErrorPath</key>\n  <string>{}</string>\n</dict>\n</plist>\n",
        xml_escape(&spec.service_name),
        xml_escape(&run_path.display().to_string()),
        xml_escape(&bundle_dir.display().to_string()),
        xml_escape(&bundle_dir.join("runner.out.log").display().to_string()),
        xml_escape(&bundle_dir.join("runner.err.log").display().to_string())
    )
}
fn moderation_runner_bundle_readme(
    spec: &ModerationRunnerBundleSpec,
    manifest_copy_name: &str,
    systemd_unit_name: &str,
    launchd_plist_name: &str,
) -> String {
    format!(
        "# SoraFS Moderation Runner Bundle\n\nThis bundle starts a manifest- and artefact-locked SoraFS moderation runner. The integer model engine performs no outbound I/O and the CLI accepts loopback listen addresses only. The generated systemd unit additionally denies non-loopback IP traffic. Direct `run.sh` and launchd execution cannot impose a kernel network sandbox, so operators must supply an equivalent process policy.\n\n- Manifest copy: `{}`\n- Model artefacts: `artifacts/`\n- Manifest id: `{}`\n- Runner hash: `{}`\n- Listen address: `{}`\n- Maximum body bytes: `{}`\n- Maximum decoded payload bytes: `{}`\n\nRun directly:\n\n```sh\n./run.sh\n```\n\nInstall with systemd:\n\n```sh\nsudo cp {} /etc/systemd/system/\nsudo systemctl daemon-reload\nsudo systemctl enable --now {}\n```\n\nInstall with launchd:\n\n```sh\ncp {} ~/Library/LaunchAgents/\nlaunchctl load ~/Library/LaunchAgents/{}\n```\n\nKeep `runner.env`, `run.sh`, the manifest copy, and `artifacts/` together. The executable at `SORAFS_CLI` must hash to the signed `runner_hash`; otherwise startup fails before binding.\n",
        manifest_copy_name,
        hex_encode(spec.manifest.body.manifest_id),
        hex_encode(spec.manifest.body.runner_hash),
        spec.listen,
        spec.max_body_bytes,
        spec.max_payload_bytes,
        systemd_unit_name,
        systemd_unit_name,
        launchd_plist_name,
        launchd_plist_name
    )
}
fn moderation_runner_bundle_summary_json(
    spec: &ModerationRunnerBundleSpec,
    bundle_dir: &Path,
    manifest_copy_name: &str,
    files: &[&str],
) -> Value {
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.moderation.runner.bundle.v1");
    insert_value!(summary["source"] = "sorafs_cli");
    insert_value!(summary["bundle_dir"] = bundle_dir.display().to_string());
    insert_value!(summary["manifest_source"] = spec.manifest_source.display().to_string());
    insert_value!(summary["manifest_copy"] = manifest_copy_name);
    insert_value!(summary["manifest_format"] = spec.manifest_format.clone());
    insert_value!(summary["manifest_id_hex"] = hex_encode(spec.manifest.body.manifest_id));
    insert_value!(summary["manifest_digest_hex"] = hex_encode(spec.manifest.body.manifest_digest));
    insert_value!(summary["runner_hash_hex"] = hex_encode(spec.manifest.body.runner_hash));
    insert_value!(summary["runtime_version"] = spec.manifest.body.runtime_version.clone());
    insert_value!(summary["listen"] = spec.listen.clone());
    insert_value!(summary["max_body_bytes"] = spec.max_body_bytes as u64);
    insert_value!(summary["max_payload_bytes"] = u64::from(spec.max_payload_bytes));
    insert_value!(summary["binary"] = spec.binary.clone());
    insert_value!(summary["service_name"] = spec.service_name.clone());
    insert_value!(summary["service_user"] = spec.service_user.clone());
    insert_value!(summary["service_group"] = spec.service_group.clone());
    insert_value!(summary["outbound_network"] = "process_policy_required");
    insert_json!(
        summary["files"] = Value::Array(files.iter().map(|file| Value::from(*file)).collect())
    );
    Value::Object(summary)
}
fn moderation_committee_bundle(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut quorum: Option<usize> = None;
    let mut bundle_out: Option<PathBuf> = None;
    let mut listen = String::from(MODERATION_COMMITTEE_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    let mut binary = String::from("sorafs_cli");
    let mut service_name = String::from("sorafs-moderation-committee");
    let mut service_user = String::from("sorafs");
    let mut service_group = String::from("sorafs");
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--quorum" => {
                let parsed =
                    parse_u64_arg("--quorum", value, "sorafs_cli moderation committee-bundle")?;
                let parsed = usize::try_from(parsed).map_err(|_| {
                    "`--quorum` does not fit into this platform's usize".to_string()
                })?;
                if parsed == 0 {
                    return Err("`--quorum` must be greater than zero".to_string());
                }
                quorum = Some(parsed);
            }
            "--bundle-out" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--bundle-out` must not be empty".to_string());
                }
                bundle_out = Some(PathBuf::from(trimmed));
            }
            "--listen" => {
                let trimmed = validate_runner_bundle_value("--listen", value)?;
                listen = trimmed.to_string();
            }
            "--max-body-bytes" => {
                max_body_bytes = parse_moderation_max_body_bytes(
                    value,
                    "sorafs_cli moderation committee-bundle",
                )?;
            }
            "--binary" => {
                let trimmed = validate_runner_bundle_value("--binary", value)?;
                binary = trimmed.to_string();
            }
            "--service-name" => {
                let trimmed = validate_runner_bundle_value("--service-name", value)?;
                validate_runner_bundle_service_name(trimmed)?;
                service_name = trimmed.to_string();
            }
            "--service-user" => {
                let trimmed = validate_runner_bundle_value("--service-user", value)?;
                service_user = trimmed.to_string();
            }
            "--service-group" => {
                let trimmed = validate_runner_bundle_value("--service-group", value)?;
                service_group = trimmed.to_string();
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation committee-bundle`"
                ));
            }
        }
    }
    validate_runner_bundle_service_name(&service_name)?;
    validate_moderation_loopback_listen(&listen, "sorafs_cli moderation committee-bundle")?;
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation committee-bundle`"
            .to_string()
    })?;
    let quorum = quorum.ok_or_else(|| {
        "missing required `--quorum=N` for `sorafs_cli moderation committee-bundle`".to_string()
    })?;
    let bundle_out = bundle_out.ok_or_else(|| {
        "missing required `--bundle-out=DIR` for `sorafs_cli moderation committee-bundle`"
            .to_string()
    })?;
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation committee-bundle",
    )?;
    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    validate_moderation_local_runner_manifest(&manifest)?;
    write_moderation_committee_bundle(ModerationCommitteeBundleSpec {
        manifest,
        manifest_source: manifest_path,
        manifest_format: format,
        quorum,
        bundle_out,
        listen,
        max_body_bytes,
        binary,
        service_name,
        service_user,
        service_group,
    })
}
struct ModerationCommitteeBundleSpec {
    manifest: ModerationReproManifestV1,
    manifest_source: PathBuf,
    manifest_format: String,
    quorum: usize,
    bundle_out: PathBuf,
    listen: String,
    max_body_bytes: usize,
    binary: String,
    service_name: String,
    service_user: String,
    service_group: String,
}
fn write_moderation_committee_bundle(spec: ModerationCommitteeBundleSpec) -> Result<(), String> {
    fs::create_dir_all(&spec.bundle_out).map_err(|err| {
        format!(
            "failed to create committee bundle directory `{}`: {err}",
            spec.bundle_out.display()
        )
    })?;
    let bundle_dir = spec
        .bundle_out
        .canonicalize()
        .unwrap_or_else(|_| spec.bundle_out.clone());
    let manifest_copy_name = match spec.manifest_format.as_str() {
        "json" => "manifest.json",
        "norito" => "manifest.to",
        other => {
            return Err(format!(
                "unsupported `--format={other}` for `sorafs_cli moderation committee-bundle` (expected `json` or `norito`)"
            ));
        }
    };
    let manifest_copy_path = bundle_dir.join(manifest_copy_name);
    let manifest_bytes = encode_moderation_manifest_for_bundle(
        &spec.manifest,
        &spec.manifest_format,
        "committee bundle",
    )?;
    write_text(&manifest_copy_path, &manifest_bytes)?;
    let env_path = bundle_dir.join("committee.env");
    let run_path = bundle_dir.join("run.sh");
    let systemd_unit_name = format!("{}.service", spec.service_name);
    let systemd_path = bundle_dir.join(&systemd_unit_name);
    let launchd_plist_name = format!("{}.plist", spec.service_name);
    let launchd_path = bundle_dir.join(&launchd_plist_name);
    let metadata_path = bundle_dir.join("bundle.json");
    let readme_path = bundle_dir.join("README.md");
    let env = moderation_committee_bundle_env(&spec);
    write_text(&env_path, env.as_bytes())?;
    let run_script =
        moderation_committee_bundle_run_script(manifest_copy_name, &spec.manifest_format);
    write_text(&run_path, run_script.as_bytes())?;
    set_executable_if_supported(&run_path)?;
    let systemd =
        moderation_committee_bundle_systemd_unit(&spec, &bundle_dir, &run_path, &env_path);
    write_text(&systemd_path, systemd.as_bytes())?;
    let launchd = moderation_committee_bundle_launchd_plist(&spec, &bundle_dir, &run_path);
    write_text(&launchd_path, launchd.as_bytes())?;
    let readme = moderation_committee_bundle_readme(
        &spec,
        manifest_copy_name,
        &systemd_unit_name,
        &launchd_plist_name,
    );
    write_text(&readme_path, readme.as_bytes())?;
    let summary = moderation_committee_bundle_summary_json(
        &spec,
        &bundle_dir,
        manifest_copy_name,
        &[
            manifest_copy_name,
            "committee.env",
            "run.sh",
            &systemd_unit_name,
            &launchd_plist_name,
            "bundle.json",
            "README.md",
        ],
    );
    let rendered = to_string_pretty(&summary)
        .map_err(|err| format!("failed to render committee bundle summary JSON: {err}"))?;
    write_text(&metadata_path, format!("{rendered}\n").as_bytes())?;
    println!("{rendered}");
    Ok(())
}
fn moderation_committee_bundle_env(spec: &ModerationCommitteeBundleSpec) -> String {
    format!(
        "SORAFS_CLI={}\nSORAFS_COMMITTEE_LISTEN={}\nSORAFS_COMMITTEE_QUORUM={}\nSORAFS_COMMITTEE_MAX_BODY_BYTES={}\n",
        shell_single_quote(&spec.binary),
        shell_single_quote(&spec.listen),
        shell_single_quote(&spec.quorum.to_string()),
        shell_single_quote(&spec.max_body_bytes.to_string())
    )
}
fn moderation_committee_bundle_run_script(manifest_copy_name: &str, format: &str) -> String {
    format!(
        "#!/usr/bin/env sh\nset -eu\nSCRIPT_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nif [ -f \"$SCRIPT_DIR/committee.env\" ]; then\n  . \"$SCRIPT_DIR/committee.env\"\nfi\n: \"${{SORAFS_CLI:=sorafs_cli}}\"\n: \"${{SORAFS_COMMITTEE_LISTEN:={}}}\"\n: \"${{SORAFS_COMMITTEE_QUORUM:=1}}\"\n: \"${{SORAFS_COMMITTEE_MAX_BODY_BYTES:={}}}\"\nexec \"$SORAFS_CLI\" moderation committee-serve \\\n  --manifest=\"$SCRIPT_DIR/{}\" \\\n  --format={} \\\n  --quorum=\"$SORAFS_COMMITTEE_QUORUM\" \\\n  --listen=\"$SORAFS_COMMITTEE_LISTEN\" \\\n  --max-body-bytes=\"$SORAFS_COMMITTEE_MAX_BODY_BYTES\"\n",
        MODERATION_COMMITTEE_DEFAULT_LISTEN,
        MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES,
        manifest_copy_name,
        format
    )
}
fn moderation_committee_bundle_systemd_unit(
    spec: &ModerationCommitteeBundleSpec,
    bundle_dir: &Path,
    run_path: &Path,
    env_path: &Path,
) -> String {
    format!(
        "[Unit]\nDescription=SoraFS moderation committee ({})\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=simple\nUser={}\nGroup={}\nWorkingDirectory={}\nEnvironmentFile={}\nExecStart={}\nRestart=on-failure\nRestartSec=5s\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectControlGroups=true\nMemoryDenyWriteExecute=true\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\nIPAddressDeny=any\nIPAddressAllow=localhost\nReadOnlyPaths={}\n\n[Install]\nWantedBy=multi-user.target\n",
        spec.service_name,
        spec.service_user,
        spec.service_group,
        systemd_quote(&bundle_dir.display().to_string()),
        systemd_quote(&env_path.display().to_string()),
        systemd_quote(&run_path.display().to_string()),
        systemd_quote(&bundle_dir.display().to_string())
    )
}
fn moderation_committee_bundle_launchd_plist(
    spec: &ModerationCommitteeBundleSpec,
    bundle_dir: &Path,
    run_path: &Path,
) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n  </array>\n  <key>WorkingDirectory</key>\n  <string>{}</string>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <true/>\n  <key>StandardOutPath</key>\n  <string>{}</string>\n  <key>StandardErrorPath</key>\n  <string>{}</string>\n</dict>\n</plist>\n",
        xml_escape(&spec.service_name),
        xml_escape(&run_path.display().to_string()),
        xml_escape(&bundle_dir.display().to_string()),
        xml_escape(&bundle_dir.join("committee.out.log").display().to_string()),
        xml_escape(&bundle_dir.join("committee.err.log").display().to_string())
    )
}
fn moderation_committee_bundle_readme(
    spec: &ModerationCommitteeBundleSpec,
    manifest_copy_name: &str,
    systemd_unit_name: &str,
    launchd_plist_name: &str,
) -> String {
    format!(
        "# SoraFS Moderation Committee Bundle\n\nThis bundle starts a locked-manifest SoraFS moderation committee service. The CLI accepts loopback listen addresses only. The generated systemd unit denies non-loopback IP traffic; direct `run.sh` and launchd execution require an equivalent external runtime policy. The service accepts payload-free runner result JSON and returns deterministic median-score quorum aggregates.\n\n- Manifest copy: `{}`\n- Manifest id: `{}`\n- Runner hash: `{}`\n- Quorum: `{}`\n- Listen address: `{}`\n- Maximum body bytes: `{}`\n\nRun directly:\n\n```sh\n./run.sh\n```\n\nInstall with systemd:\n\n```sh\nsudo cp {} /etc/systemd/system/\nsudo systemctl daemon-reload\nsudo systemctl enable --now {}\n```\n\nInstall with launchd:\n\n```sh\ncp {} ~/Library/LaunchAgents/\nlaunchctl load ~/Library/LaunchAgents/{}\n```\n\nKeep `committee.env`, `run.sh`, and the manifest copy together. Replace the `SORAFS_CLI` value in `committee.env` with the absolute path to the audited `sorafs_cli` binary on the target host before installing.\n",
        manifest_copy_name,
        hex_encode(spec.manifest.body.manifest_id),
        hex_encode(spec.manifest.body.runner_hash),
        spec.quorum,
        spec.listen,
        spec.max_body_bytes,
        systemd_unit_name,
        systemd_unit_name,
        launchd_plist_name,
        launchd_plist_name
    )
}
fn moderation_committee_bundle_summary_json(
    spec: &ModerationCommitteeBundleSpec,
    bundle_dir: &Path,
    manifest_copy_name: &str,
    files: &[&str],
) -> Value {
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.moderation.committee.bundle.v1");
    insert_value!(summary["source"] = "sorafs_cli");
    insert_value!(summary["bundle_dir"] = bundle_dir.display().to_string());
    insert_value!(summary["manifest_source"] = spec.manifest_source.display().to_string());
    insert_value!(summary["manifest_copy"] = manifest_copy_name);
    insert_value!(summary["manifest_format"] = spec.manifest_format.clone());
    insert_value!(summary["manifest_id_hex"] = hex_encode(spec.manifest.body.manifest_id));
    insert_value!(summary["manifest_digest_hex"] = hex_encode(spec.manifest.body.manifest_digest));
    insert_value!(summary["runner_hash_hex"] = hex_encode(spec.manifest.body.runner_hash));
    insert_value!(summary["runtime_version"] = spec.manifest.body.runtime_version.clone());
    insert_value!(summary["quorum"] = spec.quorum as u64);
    insert_value!(summary["aggregation"] = "median_score_bps");
    insert_value!(summary["listen"] = spec.listen.clone());
    insert_value!(summary["max_body_bytes"] = spec.max_body_bytes as u64);
    insert_value!(summary["binary"] = spec.binary.clone());
    insert_value!(summary["service_name"] = spec.service_name.clone());
    insert_value!(summary["service_user"] = spec.service_user.clone());
    insert_value!(summary["service_group"] = spec.service_group.clone());
    insert_value!(summary["outbound_network"] = "network_capable_process_policy_required");
    insert_json!(
        summary["files"] = Value::Array(files.iter().map(|file| Value::from(*file)).collect())
    );
    Value::Object(summary)
}
fn validate_runner_bundle_value<'a>(flag: &str, raw: &'a str) -> Result<&'a str, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("`{flag}` must not be empty"));
    }
    if trimmed.chars().any(|ch| matches!(ch, '\n' | '\r' | '\0')) {
        return Err(format!("`{flag}` must not contain control line separators"));
    }
    Ok(trimmed)
}
fn validate_runner_bundle_service_name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '@'))
    {
        return Err(
            "`--service-name` may only contain ASCII letters, digits, '.', '_', '-', or '@'"
                .to_string(),
        );
    }
    Ok(())
}
fn shell_single_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn systemd_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}
fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
fn set_executable_if_supported(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = fs::metadata(path)
            .map_err(|err| format!("failed to stat `{}`: {err}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|err| format!("failed to chmod `{}`: {err}", path.display()))?;
    }
    Ok(())
}
#[derive(Clone, Debug)]
struct ModerationCanaryDeploymentContext {
    generated_at_unix: u64,
    deployment_id: String,
    environment: String,
}
fn moderation_canary_deployment_id(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if trimmed != value
        || bytes.is_empty()
        || bytes.len() > 128
        || !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(
            "`--deployment-id` must be 1-128 ASCII letters, digits, '.', '_' or '-' and start/end with a letter or digit"
                .to_string(),
        );
    }
    Ok(trimmed.to_string())
}
fn moderation_canary_environment(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed != value || !matches!(trimmed, "prod" | "production" | "release" | "staging") {
        return Err(
            "`--environment` must be one of prod, production, release, or staging".to_string(),
        );
    }
    Ok(trimmed.to_string())
}
#[derive(Debug)]
struct ModerationCanaryHttpProbe {
    method: &'static str,
    url: String,
    status_code: u16,
    request_bytes: u64,
    request_body_blake3: [u8; 32],
    response_bytes: u64,
    response_body_blake3: [u8; 32],
    response: Value,
}
fn moderation_canary_probe_json(name: &str, probe: &ModerationCanaryHttpProbe) -> Value {
    let mut output = Map::new();
    insert_value!(output["name"] = name.to_string());
    insert_value!(output["method"] = probe.method);
    insert_value!(output["url"] = probe.url.clone());
    insert_value!(output["status_code"] = u64::from(probe.status_code));
    insert_value!(output["request_bytes"] = probe.request_bytes);
    insert_value!(output["request_body_blake3"] = hex_encode(probe.request_body_blake3));
    insert_value!(output["response_bytes"] = probe.response_bytes);
    insert_value!(output["response_body_blake3"] = hex_encode(probe.response_body_blake3));
    insert_value!(output["passed"] = true);
    insert_value!(output["payload_bytes_included"] = false);
    insert_value!(output["private_payloads_included"] = false);
    Value::Object(output)
}
fn moderation_runner_canary(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut runner_url: Option<String> = None;
    let mut payload_path: Option<PathBuf> = None;
    let mut subject: Option<String> = None;
    let mut screened_at_unix: Option<u64> = None;
    let mut generated_at_unix: Option<u64> = None;
    let mut deployment_id: Option<String> = None;
    let mut environment: Option<String> = None;
    let mut deployment_context_reviewed = false;
    let mut process_isolation_enforcement: Option<&'static str> = None;
    let mut process_isolation_attestation_digest: Option<[u8; 32]> = None;
    let mut process_isolation_verified_at: Option<u64> = None;
    let mut process_isolation_reviewed = false;
    let mut checked_at_unix: Option<u64> = None;
    let mut notes: Option<String> = None;
    let mut timeout_ms = 30_000_u64;
    let mut json_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--runner-url" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--runner-url` must not be empty".to_string());
                }
                runner_url = Some(trimmed.to_string());
            }
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--subject" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--subject` must not be empty".to_string());
                }
                subject = Some(trimmed.to_string());
            }
            "--screened-at" => {
                screened_at_unix = Some(parse_u64_arg(
                    "--screened-at",
                    value,
                    "sorafs_cli moderation runner-canary",
                )?);
            }
            "--generated-at-unix" => {
                let generated = parse_u64_arg(
                    "--generated-at-unix",
                    value,
                    "sorafs_cli moderation runner-canary",
                )?;
                if generated == 0 {
                    return Err("`--generated-at-unix` must be greater than zero".to_string());
                }
                generated_at_unix = Some(generated);
            }
            "--deployment-id" => {
                deployment_id = Some(moderation_canary_deployment_id(value)?);
            }
            "--environment" => {
                environment = Some(moderation_canary_environment(value)?);
            }
            "--deployment-context-reviewed" => {
                if value != "true" {
                    return Err(
                        "`--deployment-context-reviewed` must be exactly `true`".to_string()
                    );
                }
                deployment_context_reviewed = true;
            }
            "--process-isolation-enforcement" => {
                process_isolation_enforcement = Some(match value {
                    "systemd_ip_filter" => "systemd_ip_filter",
                    "container_network_policy" => "container_network_policy",
                    "host_firewall" => "host_firewall",
                    _ => {
                        return Err("`--process-isolation-enforcement` must be one of `systemd_ip_filter`, `container_network_policy`, or `host_firewall`".to_string());
                    }
                });
            }
            "--process-isolation-attestation-digest" => {
                if value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err("`--process-isolation-attestation-digest` must be exactly 64 lowercase hexadecimal characters".to_string());
                }
                let digest = parse_fixed_hex::<32>(
                    value,
                    "--process-isolation-attestation-digest",
                    "sorafs_cli moderation runner-canary",
                )?;
                if moderation_digest_is_placeholder(&digest) {
                    return Err("`--process-isolation-attestation-digest` must not be a zero/repeated placeholder digest".to_string());
                }
                process_isolation_attestation_digest = Some(digest);
            }
            "--process-isolation-verified-at" => {
                let verified_at = parse_u64_arg(
                    "--process-isolation-verified-at",
                    value,
                    "sorafs_cli moderation runner-canary",
                )?;
                if verified_at == 0 {
                    return Err(
                        "`--process-isolation-verified-at` must be greater than zero".to_string(),
                    );
                }
                process_isolation_verified_at = Some(verified_at);
            }
            "--process-isolation-reviewed" => {
                if value != "true" {
                    return Err("`--process-isolation-reviewed` must be exactly `true`".to_string());
                }
                process_isolation_reviewed = true;
            }
            "--checked-at" => {
                checked_at_unix = Some(parse_u64_arg(
                    "--checked-at",
                    value,
                    "sorafs_cli moderation runner-canary",
                )?);
            }
            "--notes" => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    notes = Some(trimmed.to_string());
                }
            }
            "--timeout-ms" => {
                timeout_ms =
                    parse_u64_arg("--timeout-ms", value, "sorafs_cli moderation runner-canary")?;
                if timeout_ms == 0 {
                    return Err("`--timeout-ms` must be greater than zero".to_string());
                }
            }
            "--json-out" => json_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation runner-canary`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation runner-canary`".to_string()
    })?;
    let runner_url = runner_url.ok_or_else(|| {
        "missing required `--runner-url=URL` for `sorafs_cli moderation runner-canary`".to_string()
    })?;
    let payload_path = payload_path.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli moderation runner-canary`".to_string()
    })?;
    let subject = subject.ok_or_else(|| {
        "missing required `--subject=ID` for `sorafs_cli moderation runner-canary`".to_string()
    })?;
    let screened_at_unix = screened_at_unix.ok_or_else(|| {
        "missing required `--screened-at=UNIX_SECS` for `sorafs_cli moderation runner-canary`"
            .to_string()
    })?;
    let generated_at_unix = generated_at_unix.ok_or_else(|| {
        "missing required `--generated-at-unix=UNIX_SECS` for `sorafs_cli moderation runner-canary`"
            .to_string()
    })?;
    let deployment_id = deployment_id.ok_or_else(|| {
        "missing required `--deployment-id=ID` for `sorafs_cli moderation runner-canary`"
            .to_string()
    })?;
    let environment = environment.ok_or_else(|| {
        "missing required `--environment=ENV` for `sorafs_cli moderation runner-canary`".to_string()
    })?;
    if !deployment_context_reviewed {
        return Err(
            "missing required `--deployment-context-reviewed=true` for `sorafs_cli moderation runner-canary`"
                .to_string(),
        );
    }
    let process_isolation_enforcement = process_isolation_enforcement.ok_or_else(|| {
        "missing required `--process-isolation-enforcement=KIND` for `sorafs_cli moderation runner-canary`"
            .to_string()
    })?;
    let process_isolation_attestation_digest = process_isolation_attestation_digest.ok_or_else(|| {
        "missing required `--process-isolation-attestation-digest=HEX` for `sorafs_cli moderation runner-canary`"
            .to_string()
    })?;
    let process_isolation_verified_at = process_isolation_verified_at.ok_or_else(|| {
        "missing required `--process-isolation-verified-at=UNIX_SECS` for `sorafs_cli moderation runner-canary`"
            .to_string()
    })?;
    if !process_isolation_reviewed {
        return Err(
            "missing required `--process-isolation-reviewed=true` for `sorafs_cli moderation runner-canary`"
                .to_string(),
        );
    }
    let checked_at_unix = checked_at_unix.unwrap_or(generated_at_unix);
    if checked_at_unix != generated_at_unix {
        return Err("`--checked-at` must equal `--generated-at-unix`".to_string());
    }
    if screened_at_unix > checked_at_unix {
        return Err("`--screened-at` must not be after `--checked-at`".to_string());
    }
    if process_isolation_verified_at > generated_at_unix {
        return Err(
            "`--process-isolation-verified-at` must not be after `--generated-at-unix`".to_string(),
        );
    }
    let process_isolation = ModerationProcessIsolationEvidence {
        enforcement: process_isolation_enforcement,
        attestation_digest: process_isolation_attestation_digest,
        verified_at_unix: process_isolation_verified_at,
    };
    let deployment_context = ModerationCanaryDeploymentContext {
        generated_at_unix,
        deployment_id,
        environment,
    };
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation runner-canary",
    )?;
    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    validate_moderation_local_runner_manifest(&manifest)?;
    let payload = read_file_bounded(
        &payload_path,
        u64::from(MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES),
        "runner canary payload",
    )?;
    if payload.is_empty() {
        return Err("`--payload` file must not be empty".to_string());
    }
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| format!("failed to construct runner canary HTTP client: {err}"))?;
    let base_url = Url::parse(&runner_url)
        .map_err(|err| format!("invalid `--runner-url` `{runner_url}`: {err}"))?;
    let status_url =
        moderation_runner_canary_endpoint(&base_url, "/v1/sorafs/moderation/runner/status")?;
    let screen_url =
        moderation_runner_canary_endpoint(&base_url, "/v1/sorafs/moderation/runner/screen")?;
    let runner_base_url = base_url.as_str().trim_end_matches('/');
    let status_probe = moderation_runner_canary_get_json(&client, &status_url)?;
    let screen_request = moderation_runner_canary_screen_request_json(
        &payload,
        &subject,
        screened_at_unix,
        notes.as_deref(),
    );
    let screening_probe =
        moderation_runner_canary_post_json(&client, &screen_url, &screen_request)?;
    let evidence = moderation_runner_canary_evidence_json(ModerationRunnerCanaryEvidenceInput {
        manifest: &manifest,
        runner_url: runner_base_url,
        status_url: status_url.as_str(),
        screen_url: screen_url.as_str(),
        subject: &subject,
        payload: &payload,
        screened_at_unix,
        checked_at_unix,
        deployment_context,
        process_isolation,
        notes: notes.as_deref(),
        status_probe,
        screening_probe,
    })?;
    let rendered = to_string_pretty(&evidence)
        .map_err(|err| format!("failed to render runner canary evidence JSON: {err}"))?;
    if let Some(path) = json_out {
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
struct ModerationRunnerCanaryEvidenceInput<'a> {
    manifest: &'a ModerationReproManifestV1,
    runner_url: &'a str,
    status_url: &'a str,
    screen_url: &'a str,
    subject: &'a str,
    payload: &'a [u8],
    screened_at_unix: u64,
    checked_at_unix: u64,
    deployment_context: ModerationCanaryDeploymentContext,
    process_isolation: ModerationProcessIsolationEvidence,
    notes: Option<&'a str>,
    status_probe: ModerationCanaryHttpProbe,
    screening_probe: ModerationCanaryHttpProbe,
}
#[derive(Clone, Copy)]
struct ModerationProcessIsolationEvidence {
    enforcement: &'static str,
    attestation_digest: [u8; 32],
    verified_at_unix: u64,
}
impl ModerationProcessIsolationEvidence {
    fn validate(self, generated_at_unix: u64, context: &str) -> Result<(), String> {
        if !matches!(
            self.enforcement,
            "systemd_ip_filter" | "container_network_policy" | "host_firewall"
        ) {
            return Err(format!(
                "{context} process isolation enforcement `{}` is unsupported",
                self.enforcement
            ));
        }
        if moderation_digest_is_placeholder(&self.attestation_digest) {
            return Err(format!(
                "{context} process isolation attestation digest is a placeholder"
            ));
        }
        if self.verified_at_unix == 0 || self.verified_at_unix > generated_at_unix {
            return Err(format!(
                "{context} process isolation verification timestamp {} is outside 1..={generated_at_unix}",
                self.verified_at_unix
            ));
        }
        Ok(())
    }
}
fn moderation_digest_is_placeholder(digest: &[u8; 32]) -> bool {
    digest.iter().all(|byte| *byte == digest[0]) || digest[..16] == digest[16..]
}
fn moderation_runner_canary_endpoint(base_url: &Url, path: &str) -> Result<Url, String> {
    let mut endpoint = base_url.as_str().trim_end_matches('/').to_string();
    endpoint.push_str(path);
    Url::parse(&endpoint)
        .map_err(|err| format!("failed to build runner endpoint `{endpoint}`: {err}"))
}
fn read_moderation_canary_response_bounded(
    response: reqwest::blocking::Response,
    context: &str,
) -> Result<(StatusCode, Vec<u8>), String> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MODERATION_CANARY_MAX_RESPONSE_BYTES)
    {
        return Err(format!(
            "{context} declared a response larger than {MODERATION_CANARY_MAX_RESPONSE_BYTES} bytes"
        ));
    }
    let initial_capacity = response
        .content_length()
        .unwrap_or(0)
        .min(MODERATION_CANARY_MAX_RESPONSE_BYTES);
    let initial_capacity = usize::try_from(initial_capacity)
        .map_err(|_| format!("{context} response length does not fit usize"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial_capacity)
        .map_err(|error| format!("failed to reserve bounded {context} response: {error}"))?;
    let mut limited = response.take(MODERATION_CANARY_MAX_RESPONSE_BYTES + 1);
    limited
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{context} failed to read body: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MODERATION_CANARY_MAX_RESPONSE_BYTES {
        return Err(format!(
            "{context} response exceeded {MODERATION_CANARY_MAX_RESPONSE_BYTES} bytes"
        ));
    }
    Ok((status, bytes))
}
fn moderation_runner_canary_get_json(
    client: &HttpClient,
    url: &Url,
) -> Result<ModerationCanaryHttpProbe, String> {
    let response = client
        .get(url.as_str())
        .send()
        .map_err(|err| format!("runner canary GET `{url}` failed: {err}"))?;
    let (status, bytes) =
        read_moderation_canary_response_bounded(response, &format!("runner canary GET `{url}`"))?;
    if !status.is_success() {
        return Err(format!(
            "runner canary GET `{url}` returned HTTP {status}: {}",
            body_snippet(&bytes)
        ));
    }
    let response_bytes = u64::try_from(bytes.len())
        .map_err(|_| "runner canary GET response length exceeds u64".to_string())?;
    let response = from_slice(&bytes)
        .map_err(|err| format!("runner canary GET `{url}` returned invalid JSON: {err}"))?;
    Ok(ModerationCanaryHttpProbe {
        method: "GET",
        url: url.as_str().to_string(),
        status_code: status.as_u16(),
        request_bytes: 0,
        request_body_blake3: *blake3_hash(&[]).as_bytes(),
        response_bytes,
        response_body_blake3: *blake3_hash(&bytes).as_bytes(),
        response,
    })
}
fn moderation_runner_canary_post_json(
    client: &HttpClient,
    url: &Url,
    value: &Value,
) -> Result<ModerationCanaryHttpProbe, String> {
    let body = to_vec(value)
        .map_err(|err| format!("failed to encode runner canary request JSON: {err}"))?;
    if body.len() > MODERATION_RUNNER_HARD_MAX_BODY_BYTES {
        return Err(format!(
            "runner canary request has {} bytes; maximum is {MODERATION_RUNNER_HARD_MAX_BODY_BYTES}",
            body.len()
        ));
    }
    let request_bytes = u64::try_from(body.len())
        .map_err(|_| "runner canary POST request length exceeds u64".to_string())?;
    let request_body_blake3 = *blake3_hash(&body).as_bytes();
    let response = client
        .post(url.as_str())
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .map_err(|err| format!("runner canary POST `{url}` failed: {err}"))?;
    let (status, bytes) =
        read_moderation_canary_response_bounded(response, &format!("runner canary POST `{url}`"))?;
    if !status.is_success() {
        return Err(format!(
            "runner canary POST `{url}` returned HTTP {status}: {}",
            body_snippet(&bytes)
        ));
    }
    let response_bytes = u64::try_from(bytes.len())
        .map_err(|_| "runner canary POST response length exceeds u64".to_string())?;
    let response = from_slice(&bytes)
        .map_err(|err| format!("runner canary POST `{url}` returned invalid JSON: {err}"))?;
    Ok(ModerationCanaryHttpProbe {
        method: "POST",
        url: url.as_str().to_string(),
        status_code: status.as_u16(),
        request_bytes,
        request_body_blake3,
        response_bytes,
        response_body_blake3: *blake3_hash(&bytes).as_bytes(),
        response,
    })
}
fn moderation_runner_canary_screen_request_json(
    payload: &[u8],
    subject: &str,
    screened_at_unix: u64,
    notes: Option<&str>,
) -> Value {
    let mut request = Map::new();
    insert_value!(request["subject"] = subject.to_string());
    insert_value!(request["payload_b64"] = BASE64_STANDARD.encode(payload));
    insert_value!(request["screened_at_unix"] = screened_at_unix);
    insert_json!(
        request["notes"] = notes
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    Value::Object(request)
}
fn moderation_runner_canary_evidence_json(
    input: ModerationRunnerCanaryEvidenceInput<'_>,
) -> Result<Value, String> {
    input.process_isolation.validate(
        input.deployment_context.generated_at_unix,
        "runner canary evidence",
    )?;
    validate_moderation_runner_status_response(input.manifest, &input.status_probe.response)?;
    let subject_digest = *blake3_hash(input.payload).as_bytes();
    let screening = validate_moderation_runner_screening_response(
        input.manifest,
        input.subject,
        &subject_digest,
        &input.screening_probe.response,
    )?;
    if json_contains_key(&input.status_probe.response, "payload_b64")
        || json_contains_key(&input.screening_probe.response, "payload_b64")
    {
        return Err("runner canary evidence responses must not contain `payload_b64`".to_string());
    }
    let probes = Value::Array(vec![
        moderation_canary_probe_json("status", &input.status_probe),
        moderation_canary_probe_json("screen", &input.screening_probe),
    ]);
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.runner.rollout_evidence.v1");
    insert_value!(output["status"] = "verified");
    insert_value!(output["synthetic"] = false);
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["generated_at_unix"] = input.deployment_context.generated_at_unix);
    insert_value!(output["deployment_id"] = input.deployment_context.deployment_id);
    insert_value!(output["environment"] = input.deployment_context.environment);
    insert_value!(output["deployment_context_reviewed"] = true);
    insert_value!(output["outbound_network"] = "model_engine_none_process_policy_required");
    insert_json!(
        output["process_isolation_evidence"] = Value::Object(Map::from_iter([
            ("required".into(), Value::from(true)),
            ("status".into(), Value::from("runtime_verified")),
            (
                "enforcement".into(),
                Value::from(input.process_isolation.enforcement),
            ),
            (
                "attestation_digest_hex".into(),
                Value::from(hex_encode(input.process_isolation.attestation_digest)),
            ),
            (
                "verified_at_unix".into(),
                Value::from(input.process_isolation.verified_at_unix),
            ),
            ("reviewed".into(), Value::from(true)),
            ("synthetic".into(), Value::from(false)),
        ]))
    );
    insert_value!(output["runner_url"] = input.runner_url.to_string());
    insert_value!(output["status_url"] = input.status_url.to_string());
    insert_value!(output["screen_url"] = input.screen_url.to_string());
    insert_value!(output["manifest_id_hex"] = hex_encode(input.manifest.body.manifest_id));
    insert_value!(output["runner_hash_hex"] = hex_encode(input.manifest.body.runner_hash));
    insert_value!(output["subject"] = input.subject.to_string());
    insert_value!(output["subject_digest_hex"] = hex_encode(subject_digest));
    insert_value!(output["screened_at_unix"] = input.screened_at_unix);
    insert_value!(output["checked_at_unix"] = input.checked_at_unix);
    insert_value!(output["combined_score_bps"] = u64::from(screening.combined_score_bps));
    insert_value!(output["verdict"] = screening.verdict);
    insert_value!(output["evidence_digest_hex"] = hex_encode(screening.evidence_digest));
    insert_value!(output["policy_digest_hex"] = hex_encode(screening.policy_digest));
    insert_value!(output["probe_count"] = 2_u64);
    insert_value!(output["passed_probe_count"] = 2_u64);
    insert_json!(output["probes"] = probes);
    insert_json!(
        output["notes"] = input
            .notes
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    insert_json!(output["runner_status"] = input.status_probe.response);
    insert_json!(output["screening_result"] = input.screening_probe.response);
    Ok(Value::Object(output))
}
struct ModerationRunnerCanaryScreening {
    combined_score_bps: u16,
    verdict: String,
    evidence_digest: [u8; 32],
    policy_digest: [u8; 32],
}
fn validate_moderation_runner_status_response(
    manifest: &ModerationReproManifestV1,
    value: &Value,
) -> Result<(), String> {
    let fields = value
        .as_object()
        .ok_or_else(|| "runner status response must be a JSON object".to_string())?;
    let context = "runner status response";
    let schema = required_json_string(fields, "schema", context)?;
    if schema != "sorafs.moderation.runner.status.v1" {
        return Err(format!("{context} has unexpected schema `{schema}`"));
    }
    let status = required_json_string(fields, "status", context)?;
    if status != "ready" {
        return Err(format!("{context} status `{status}` is not `ready`"));
    }
    let manifest_id = parse_fixed_hex::<16>(
        required_json_string(fields, "manifest_id_hex", context)?,
        "manifest_id_hex",
        context,
    )?;
    if manifest_id != manifest.body.manifest_id {
        return Err(format!(
            "{context} manifest id {} does not match locked manifest {}",
            hex_encode(manifest_id),
            hex_encode(manifest.body.manifest_id)
        ));
    }
    let runner_hash = parse_fixed_hex::<32>(
        required_json_string(fields, "runner_hash_hex", context)?,
        "runner_hash_hex",
        context,
    )?;
    if runner_hash != manifest.body.runner_hash {
        return Err(format!(
            "{context} runner hash {} does not match locked runner {}",
            hex_encode(runner_hash),
            hex_encode(manifest.body.runner_hash)
        ));
    }
    let outbound = required_json_string(fields, "outbound_network", context)?;
    if outbound != "model_engine_none_process_policy_required" {
        return Err(format!(
            "{context} outbound_network `{outbound}` is not `model_engine_none_process_policy_required`"
        ));
    }
    let isolation = required_json_string(fields, "process_isolation", context)?;
    if isolation != "external_runtime_attestation_required" {
        return Err(format!(
            "{context} process_isolation `{isolation}` is not `external_runtime_attestation_required`"
        ));
    }
    if fields
        .get("process_isolation_verified")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(format!(
            "{context} must report process_isolation_verified=false; runtime isolation requires external evidence"
        ));
    }
    Ok(())
}
fn validate_moderation_runner_screening_response(
    manifest: &ModerationReproManifestV1,
    subject: &str,
    subject_digest: &[u8; 32],
    value: &Value,
) -> Result<ModerationRunnerCanaryScreening, String> {
    let fields = value
        .as_object()
        .ok_or_else(|| "runner screening response must be a JSON object".to_string())?;
    let context = "runner screening response";
    let actual_subject = required_json_string(fields, "subject", context)?;
    if actual_subject != subject {
        return Err(format!(
            "{context} subject `{actual_subject}` does not match `{subject}`"
        ));
    }
    let actual_digest = parse_fixed_hex::<32>(
        required_json_string(fields, "subject_digest_hex", context)?,
        "subject_digest_hex",
        context,
    )?;
    if &actual_digest != subject_digest {
        return Err(format!(
            "{context} subject digest does not match payload digest"
        ));
    }
    let manifest_id = parse_fixed_hex::<16>(
        required_json_string(fields, "manifest_id_hex", context)?,
        "manifest_id_hex",
        context,
    )?;
    if manifest_id != manifest.body.manifest_id {
        return Err(format!(
            "{context} manifest id {} does not match locked manifest {}",
            hex_encode(manifest_id),
            hex_encode(manifest.body.manifest_id)
        ));
    }
    let runner_hash = parse_fixed_hex::<32>(
        required_json_string(fields, "runner_hash_hex", context)?,
        "runner_hash_hex",
        context,
    )?;
    if runner_hash != manifest.body.runner_hash {
        return Err(format!(
            "{context} runner hash {} does not match locked runner {}",
            hex_encode(runner_hash),
            hex_encode(manifest.body.runner_hash)
        ));
    }
    let score = fields
        .get("combined_score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} requires numeric `combined_score_bps`"))?;
    if score > 10_000 {
        return Err(format!(
            "{context} combined_score_bps {score} exceeds 10000 bps"
        ));
    }
    let combined_score_bps =
        u16::try_from(score).map_err(|_| format!("{context} combined_score_bps overflowed u16"))?;
    let verdict = required_json_string(fields, "verdict", context)?.to_string();
    let expected_verdict = moderation_score_verdict(combined_score_bps, manifest.body.thresholds);
    if verdict != expected_verdict {
        return Err(format!(
            "{context} verdict `{verdict}` does not match score-derived verdict `{expected_verdict}`"
        ));
    }
    let evidence_digest = optional_json_fixed_hex::<32>(fields, "evidence_digest_hex", context)?
        .ok_or_else(|| format!("{context} requires `evidence_digest_hex`"))?;
    let policy_digest = optional_json_fixed_hex::<32>(fields, "policy_digest_hex", context)?
        .ok_or_else(|| format!("{context} requires `policy_digest_hex`"))?;
    Ok(ModerationRunnerCanaryScreening {
        combined_score_bps,
        verdict,
        evidence_digest,
        policy_digest,
    })
}
fn json_contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(fields) => fields
            .iter()
            .any(|(field, nested)| field == key || json_contains_key(nested, key)),
        Value::Array(values) => values.iter().any(|nested| json_contains_key(nested, key)),
        _ => false,
    }
}
#[derive(Clone, Debug)]
struct ModerationCommitteeInput {
    source_path: PathBuf,
    subject: String,
    subject_digest: [u8; 32],
    manifest_id: [u8; 16],
    runner_hash: [u8; 32],
    combined_score_bps: u16,
    verdict: String,
    screened_at_unix: Option<u64>,
    evidence_digest: Option<[u8; 32]>,
    policy_digest: Option<[u8; 32]>,
    notes: Option<String>,
}
fn moderation_committee_run(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut quorum: Option<usize> = None;
    let mut result_paths: Vec<PathBuf> = Vec::new();
    let mut notes: Option<String> = None;
    let mut json_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--quorum" => {
                let parsed =
                    parse_u64_arg("--quorum", value, "sorafs_cli moderation committee-run")?;
                let parsed = usize::try_from(parsed).map_err(|_| {
                    "`--quorum` does not fit into this platform's usize".to_string()
                })?;
                if parsed == 0 {
                    return Err("`--quorum` must be greater than zero".to_string());
                }
                quorum = Some(parsed);
            }
            "--result" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--result` path must not be empty".to_string());
                }
                result_paths.push(PathBuf::from(trimmed));
            }
            "--notes" => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    notes = Some(trimmed.to_string());
                }
            }
            "--json-out" => json_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation committee-run`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation committee-run`".to_string()
    })?;
    let quorum = quorum.ok_or_else(|| {
        "missing required `--quorum=N` for `sorafs_cli moderation committee-run`".to_string()
    })?;
    if result_paths.is_empty() {
        return Err("provide at least one `--result=PATH` for committee aggregation".to_string());
    }
    if result_paths.len() > MODERATION_COMMITTEE_MAX_RESULTS {
        return Err(format!(
            "committee aggregation accepts at most {MODERATION_COMMITTEE_MAX_RESULTS} result files"
        ));
    }
    if quorum > result_paths.len() {
        return Err(format!(
            "committee aggregation requires quorum {quorum} but only {} result file(s) were provided",
            result_paths.len()
        ));
    }
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation committee-run",
    )?;
    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    validate_moderation_local_runner_manifest(&manifest)?;
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(result_paths.len())
        .map_err(|error| format!("failed to reserve bounded committee inputs: {error}"))?;
    for path in &result_paths {
        inputs.push(load_moderation_committee_input(path, &manifest)?);
    }
    inputs.sort_by_key(|input| input.source_path.display().to_string());
    let output = moderation_committee_aggregate_json(&manifest, &inputs, quorum, notes.as_deref())?;
    let rendered = to_string_pretty(&output)
        .map_err(|err| format!("failed to render committee aggregate JSON: {err}"))?;
    if let Some(path) = json_out {
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
fn moderation_committee_authenticated_run(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let context = "sorafs_cli moderation committee-authenticated-run";
    let mut manifest_path = None;
    let mut format = String::from("json");
    let mut trust_policy_path = None;
    let mut trust_policy_format = String::from("norito");
    let mut trust_anchors = BTreeSet::new();
    let mut minimum_governance_quorum = None;
    let mut result_paths = Vec::new();
    let mut provenance_path = None;
    let mut provenance_log_id = None;
    let mut norito_out = None;
    let mut json_out = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--trust-policy" => trust_policy_path = Some(PathBuf::from(value)),
            "--trust-policy-format" => trust_policy_format = value.to_ascii_lowercase(),
            "--trust-anchor" => {
                let anchor = parse_moderation_trust_anchor(value, context)?;
                if !trust_anchors.insert(anchor) {
                    return Err("duplicate `--trust-anchor` is forbidden".to_string());
                }
            }
            "--minimum-governance-quorum" => {
                let parsed = parse_u64_arg(key, value, context)?;
                let parsed = u16::try_from(parsed)
                    .map_err(|_| "`--minimum-governance-quorum` exceeds u16".to_string())?;
                if parsed == 0 {
                    return Err(
                        "`--minimum-governance-quorum` must be greater than zero".to_string()
                    );
                }
                minimum_governance_quorum = Some(parsed);
            }
            "--result" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--result` path must not be empty".to_string());
                }
                if result_paths.len() >= MODERATION_COMMITTEE_MAX_RESULTS {
                    return Err(format!(
                        "authenticated committee accepts at most {MODERATION_COMMITTEE_MAX_RESULTS} results"
                    ));
                }
                result_paths.push(PathBuf::from(trimmed));
            }
            "--provenance" => provenance_path = Some(PathBuf::from(value)),
            "--provenance-log-id" => {
                provenance_log_id = Some(parse_fixed_hex::<16>(
                    value,
                    "--provenance-log-id",
                    context,
                )?);
            }
            "--norito-out" => norito_out = Some(PathBuf::from(value)),
            "--json-out" => json_out = Some(PathBuf::from(value)),
            _ => return Err(format!("unrecognised option `{key}` for `{context}`")),
        }
    }
    let manifest_path = manifest_path
        .ok_or_else(|| format!("missing required `--manifest=PATH` for `{context}`"))?;
    let trust_policy_path = trust_policy_path
        .ok_or_else(|| format!("missing required `--trust-policy=PATH` for `{context}`"))?;
    if trust_anchors.is_empty() {
        return Err(format!(
            "provide at least one external `--trust-anchor=PUBLIC_KEY` for `{context}`"
        ));
    }
    let minimum_governance_quorum = minimum_governance_quorum.ok_or_else(|| {
        format!("missing required `--minimum-governance-quorum=N` for `{context}`")
    })?;
    if result_paths.is_empty() {
        return Err(format!(
            "provide at least one canonical `--result=SIGNED_RESULT.to` for `{context}`"
        ));
    }
    let provenance_path = provenance_path
        .ok_or_else(|| format!("missing required `--provenance=PATH` for `{context}`"))?;
    let provenance_log_id = provenance_log_id
        .ok_or_else(|| format!("missing required `--provenance-log-id=HEX16` for `{context}`"))?;
    if provenance_log_id == [0; 16] {
        return Err("`--provenance-log-id` must be non-zero".to_string());
    }
    let now_unix = moderation_trusted_now_unix()?;
    let manifest = load_moderation_repro_manifest(&manifest_path, &format, context)?;
    let policy = load_moderation_trust_policy(&trust_policy_path, &trust_policy_format, context)?;
    let mut results = Vec::new();
    results
        .try_reserve_exact(result_paths.len())
        .map_err(|error| format!("failed to reserve bounded signed result set: {error}"))?;
    for path in &result_paths {
        results.push(load_moderation_signed_result(path, context)?);
    }
    let aggregate = ModerationCommitteeAggregateV1::aggregate_authenticated(
        &manifest,
        &policy,
        &trust_anchors,
        minimum_governance_quorum,
        &results,
        now_unix,
    )
    .map_err(|error| format!("authenticated moderation aggregation failed: {error}"))?;
    ensure_parent_dir(&provenance_path)?;
    let provenance = ModerationProvenanceStoreV1::open(&provenance_path, provenance_log_id)
        .map_err(|error| format!("failed to open moderation provenance: {error}"))?;
    let provenance_head = provenance
        .append_authenticated_aggregate(
            &manifest,
            &policy,
            &trust_anchors,
            minimum_governance_quorum,
            &results,
            aggregate.clone(),
            now_unix,
        )
        .map_err(|error| format!("failed to persist authenticated aggregate: {error}"))?;
    let canonical = to_bytes(&aggregate)
        .map_err(|error| format!("failed to encode authenticated aggregate: {error}"))?;
    if u64::try_from(canonical.len())
        .ok()
        .is_none_or(|length| length > MODERATION_AUTHENTICATED_AGGREGATE_MAX_BYTES)
    {
        return Err("authenticated aggregate exceeds its hard encoded bound".to_string());
    }
    if let Some(path) = norito_out {
        write_bytes(&path, &canonical)?;
    }
    let output =
        moderation_authenticated_aggregate_summary_json(&aggregate, &canonical, provenance_head)?;
    let rendered = to_string_pretty(&output)
        .map_err(|error| format!("failed to render authenticated aggregate: {error}"))?;
    if let Some(path) = json_out {
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
fn load_moderation_committee_input(
    path: &Path,
    manifest: &ModerationReproManifestV1,
) -> Result<ModerationCommitteeInput, String> {
    let bytes = read_file_bounded(
        path,
        MODERATION_COMMITTEE_MAX_RESULT_BYTES,
        "moderation committee result",
    )?;
    let value: Value = from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse committee result JSON `{}`: {err}",
            path.display()
        )
    })?;
    parse_moderation_committee_input_value(&path.display().to_string(), &value, manifest)
}
fn parse_moderation_committee_input_value(
    source_label: &str,
    value: &Value,
    manifest: &ModerationReproManifestV1,
) -> Result<ModerationCommitteeInput, String> {
    let fields = value
        .as_object()
        .ok_or_else(|| format!("committee result `{source_label}` must be a JSON object"))?;
    let context = format!("committee result `{source_label}`");
    if json_contains_key(value, "payload_b64") {
        return Err(format!(
            "{context} must be payload-free screening-result JSON, found `payload_b64`"
        ));
    }
    let subject = required_json_string(fields, "subject", &context)?.to_string();
    validate_moderation_request_text(
        &subject,
        MODERATION_RUNNER_MAX_SUBJECT_BYTES,
        "moderation committee result `subject`",
    )?;
    let subject_digest_hex = required_json_string(fields, "subject_digest_hex", &context)?;
    let subject_digest = parse_fixed_hex::<32>(subject_digest_hex, "subject_digest_hex", &context)?;
    let manifest_id_hex = required_json_string(fields, "manifest_id_hex", &context)?;
    let manifest_id = parse_fixed_hex::<16>(manifest_id_hex, "manifest_id_hex", &context)?;
    if manifest_id != manifest.body.manifest_id {
        return Err(format!(
            "{context} `manifest_id_hex` {} does not match locked manifest {}",
            hex_encode(manifest_id),
            hex_encode(manifest.body.manifest_id)
        ));
    }
    let runner_hash_hex = required_json_string(fields, "runner_hash_hex", &context)?;
    let runner_hash = parse_fixed_hex::<32>(runner_hash_hex, "runner_hash_hex", &context)?;
    if runner_hash != manifest.body.runner_hash {
        return Err(format!(
            "{context} `runner_hash_hex` {} does not match locked runner {}",
            hex_encode(runner_hash),
            hex_encode(manifest.body.runner_hash)
        ));
    }
    let combined_score = fields
        .get("combined_score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} requires numeric `combined_score_bps`"))?;
    if combined_score > 10_000 {
        return Err(format!(
            "{context} `combined_score_bps` {combined_score} exceeds 10000 bps"
        ));
    }
    let combined_score_bps = u16::try_from(combined_score)
        .map_err(|_| format!("{context} `combined_score_bps` overflowed u16"))?;
    let verdict = required_json_string(fields, "verdict", &context)?.to_string();
    if !matches!(verdict.as_str(), "pass" | "quarantine" | "escalate") {
        return Err(format!("{context} has unsupported verdict `{verdict}`"));
    }
    let expected_verdict = moderation_score_verdict(combined_score_bps, manifest.body.thresholds);
    if verdict != expected_verdict {
        return Err(format!(
            "{context} verdict `{verdict}` does not match score-derived verdict `{expected_verdict}`"
        ));
    }
    let notes = optional_json_string(fields, "notes", &context)?.map(ToOwned::to_owned);
    if let Some(notes) = notes.as_deref() {
        validate_moderation_request_text(
            notes,
            MODERATION_RUNNER_MAX_NOTES_BYTES,
            "moderation committee result `notes`",
        )?;
    }
    Ok(ModerationCommitteeInput {
        source_path: PathBuf::from(source_label),
        subject,
        subject_digest,
        manifest_id,
        runner_hash,
        combined_score_bps,
        verdict,
        screened_at_unix: optional_json_u64(fields, "screened_at_unix", &context)?,
        evidence_digest: optional_json_fixed_hex::<32>(fields, "evidence_digest_hex", &context)?,
        policy_digest: optional_json_fixed_hex::<32>(fields, "policy_digest_hex", &context)?,
        notes,
    })
}
fn moderation_committee_aggregate_json(
    manifest: &ModerationReproManifestV1,
    inputs: &[ModerationCommitteeInput],
    quorum: usize,
    notes: Option<&str>,
) -> Result<Value, String> {
    if inputs.is_empty() {
        return Err("committee aggregation requires at least one result".to_string());
    }
    if inputs.len() > MODERATION_COMMITTEE_MAX_RESULTS {
        return Err(format!(
            "committee aggregation accepts at most {MODERATION_COMMITTEE_MAX_RESULTS} results"
        ));
    }
    if quorum == 0 {
        return Err("committee aggregation quorum must be greater than zero".to_string());
    }
    if inputs.len() < quorum {
        return Err(format!(
            "committee aggregation requires quorum {quorum} but only {} result file(s) were provided",
            inputs.len()
        ));
    }
    if let Some(notes) = notes {
        validate_moderation_request_text(
            notes,
            MODERATION_RUNNER_MAX_NOTES_BYTES,
            "moderation committee `notes`",
        )?;
    }
    let first = &inputs[0];
    for input in inputs.iter().skip(1) {
        if input.subject != first.subject {
            return Err(format!(
                "committee result `{}` subject `{}` does not match `{}`",
                input.source_path.display(),
                input.subject,
                first.subject
            ));
        }
        if input.subject_digest != first.subject_digest {
            return Err(format!(
                "committee result `{}` subject digest does not match the first result",
                input.source_path.display()
            ));
        }
        if input.manifest_id != manifest.body.manifest_id {
            return Err(format!(
                "committee result `{}` manifest id does not match the locked manifest",
                input.source_path.display()
            ));
        }
        if input.runner_hash != manifest.body.runner_hash {
            return Err(format!(
                "committee result `{}` runner hash does not match the locked manifest",
                input.source_path.display()
            ));
        }
    }
    let median_score_bps = moderation_committee_median_score(
        inputs
            .iter()
            .map(|input| input.combined_score_bps)
            .collect(),
    );
    let verdict = moderation_score_verdict(median_score_bps, manifest.body.thresholds);
    let screened_min = inputs
        .iter()
        .filter_map(|input| input.screened_at_unix)
        .min();
    let screened_max = inputs
        .iter()
        .filter_map(|input| input.screened_at_unix)
        .max();
    let member_results = inputs
        .iter()
        .map(moderation_committee_member_result_json)
        .collect();
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.committee.aggregate.v1");
    insert_value!(output["status"] = "quorum_satisfied");
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["manifest_id_hex"] = hex_encode(manifest.body.manifest_id));
    insert_value!(output["runner_hash_hex"] = hex_encode(manifest.body.runner_hash));
    insert_value!(output["subject"] = first.subject.clone());
    insert_value!(output["subject_digest_hex"] = hex_encode(first.subject_digest));
    insert_value!(output["result_count"] = inputs.len() as u64);
    insert_value!(output["quorum"] = quorum as u64);
    insert_value!(output["aggregation"] = "median_score_bps");
    insert_value!(output["aggregated_score_bps"] = u64::from(median_score_bps));
    insert_value!(output["verdict"] = verdict);
    insert_json!(
        output["screened_at_unix_min"] = screened_min.map(Value::from).unwrap_or(Value::Null)
    );
    insert_json!(
        output["screened_at_unix_max"] = screened_max.map(Value::from).unwrap_or(Value::Null)
    );
    insert_json!(
        output["notes"] = notes
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    insert_json!(output["member_results"] = Value::Array(member_results));
    Ok(Value::Object(output))
}
fn moderation_committee_median_score(mut scores: Vec<u16>) -> u16 {
    debug_assert!(!scores.is_empty());
    scores.sort_unstable();
    let mid = scores.len() / 2;
    if scores.len() % 2 == 1 {
        scores[mid]
    } else {
        (u32::from(scores[mid - 1]) + u32::from(scores[mid])).div_ceil(2) as u16
    }
}
fn moderation_committee_member_result_json(input: &ModerationCommitteeInput) -> Value {
    let mut result = Map::new();
    insert_value!(result["source_path"] = input.source_path.display().to_string());
    insert_value!(result["combined_score_bps"] = u64::from(input.combined_score_bps));
    insert_value!(result["verdict"] = input.verdict.clone());
    insert_json!(
        result["screened_at_unix"] = input
            .screened_at_unix
            .map(Value::from)
            .unwrap_or(Value::Null)
    );
    insert_json!(
        result["evidence_digest_hex"] = input
            .evidence_digest
            .map(hex_encode)
            .map(Value::from)
            .unwrap_or(Value::Null)
    );
    insert_json!(
        result["policy_digest_hex"] = input
            .policy_digest
            .map(hex_encode)
            .map(Value::from)
            .unwrap_or(Value::Null)
    );
    insert_json!(
        result["notes"] = input
            .notes
            .as_ref()
            .map(|value| Value::from(value.clone()))
            .unwrap_or(Value::Null)
    );
    Value::Object(result)
}
fn moderation_committee_serve(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut quorum: Option<usize> = None;
    let mut listen = String::from(MODERATION_COMMITTEE_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--quorum" => {
                let parsed =
                    parse_u64_arg("--quorum", value, "sorafs_cli moderation committee-serve")?;
                let parsed = usize::try_from(parsed).map_err(|_| {
                    "`--quorum` does not fit into this platform's usize".to_string()
                })?;
                if parsed == 0 {
                    return Err("`--quorum` must be greater than zero".to_string());
                }
                quorum = Some(parsed);
            }
            "--listen" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--listen` must not be empty".to_string());
                }
                listen = trimmed.to_string();
            }
            "--max-body-bytes" => {
                max_body_bytes = parse_moderation_max_body_bytes(
                    value,
                    "sorafs_cli moderation committee-serve",
                )?;
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation committee-serve`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation committee-serve`".to_string()
    })?;
    let quorum = quorum.ok_or_else(|| {
        "missing required `--quorum=N` for `sorafs_cli moderation committee-serve`".to_string()
    })?;
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation committee-serve",
    )?;
    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    validate_moderation_local_runner_manifest(&manifest)?;
    let listen_addr =
        validate_moderation_loopback_listen(&listen, "sorafs_cli moderation committee-serve")?;
    let listener = TcpListener::bind(listen_addr).map_err(|err| {
        format!("failed to bind moderation committee service at `{listen}`: {err}")
    })?;
    let local_addr = listener
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| listen.clone());
    let service = Arc::new(ModerationCommitteeService {
        manifest,
        authenticated: None,
        manifest_source: manifest_path.display().to_string(),
        quorum,
        max_body_bytes,
    });
    let status = moderation_committee_status_json(&service, "listening", Some(&local_addr));
    let rendered = to_string_pretty(&status)
        .map_err(|err| format!("failed to render committee service status JSON: {err}"))?;
    println!("{rendered}");
    let active_connections = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let Some(active_permit) = moderation_try_acquire_permit(
                    &active_connections,
                    MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS,
                ) else {
                    let response = moderation_committee_error_response(
                        503,
                        "Service Unavailable",
                        "moderation committee connection limit reached",
                    );
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    continue;
                };
                let service = Arc::clone(&service);
                thread::spawn(move || {
                    let _active_permit = active_permit;
                    if let Err(err) =
                        moderation_committee_handle_stream(stream, &service, max_body_bytes)
                    {
                        eprintln!("sorafs moderation committee connection failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("sorafs moderation committee accept failed: {err}"),
        }
    }
    Ok(())
}
fn moderation_committee_authenticated_serve(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let context = "sorafs_cli moderation committee-authenticated-serve";
    let mut manifest_path = None;
    let mut format = String::from("json");
    let mut trust_policy_path = None;
    let mut trust_policy_format = String::from("norito");
    let mut trust_anchors = BTreeSet::new();
    let mut minimum_governance_quorum = None;
    let mut provenance_path = None;
    let mut provenance_log_id = None;
    let mut listen = String::from(MODERATION_COMMITTEE_DEFAULT_LISTEN);
    let mut max_body_bytes = MODERATION_RUNNER_DEFAULT_MAX_BODY_BYTES;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--trust-policy" => trust_policy_path = Some(PathBuf::from(value)),
            "--trust-policy-format" => trust_policy_format = value.to_ascii_lowercase(),
            "--trust-anchor" => {
                let anchor = parse_moderation_trust_anchor(value, context)?;
                if !trust_anchors.insert(anchor) {
                    return Err("duplicate `--trust-anchor` is forbidden".to_string());
                }
            }
            "--minimum-governance-quorum" => {
                let parsed = parse_u64_arg(key, value, context)?;
                let parsed = u16::try_from(parsed)
                    .map_err(|_| "`--minimum-governance-quorum` exceeds u16".to_string())?;
                if parsed == 0 {
                    return Err(
                        "`--minimum-governance-quorum` must be greater than zero".to_string()
                    );
                }
                minimum_governance_quorum = Some(parsed);
            }
            "--provenance" => provenance_path = Some(PathBuf::from(value)),
            "--provenance-log-id" => {
                provenance_log_id = Some(parse_fixed_hex::<16>(
                    value,
                    "--provenance-log-id",
                    context,
                )?);
            }
            "--listen" => {
                if value.trim().is_empty() {
                    return Err("`--listen` must not be empty".to_string());
                }
                listen = value.to_string();
            }
            "--max-body-bytes" => max_body_bytes = parse_moderation_max_body_bytes(value, context)?,
            _ => return Err(format!("unrecognised option `{key}` for `{context}`")),
        }
    }
    let manifest_path = manifest_path
        .ok_or_else(|| format!("missing required `--manifest=PATH` for `{context}`"))?;
    let trust_policy_path = trust_policy_path
        .ok_or_else(|| format!("missing required `--trust-policy=PATH` for `{context}`"))?;
    if trust_anchors.is_empty() {
        return Err(format!(
            "provide at least one external `--trust-anchor=PUBLIC_KEY` for `{context}`"
        ));
    }
    let minimum_governance_quorum = minimum_governance_quorum.ok_or_else(|| {
        format!("missing required `--minimum-governance-quorum=N` for `{context}`")
    })?;
    let provenance_path = provenance_path
        .ok_or_else(|| format!("missing required `--provenance=PATH` for `{context}`"))?;
    let provenance_log_id = provenance_log_id
        .ok_or_else(|| format!("missing required `--provenance-log-id=HEX16` for `{context}`"))?;
    if provenance_log_id == [0; 16] {
        return Err("`--provenance-log-id` must be non-zero".to_string());
    }
    let listen_addr = validate_moderation_loopback_listen(&listen, context)?;
    let now_unix = moderation_trusted_now_unix()?;
    let manifest = load_moderation_repro_manifest(&manifest_path, &format, context)?;
    let policy = load_moderation_trust_policy(&trust_policy_path, &trust_policy_format, context)?;
    policy
        .validate_with_trust_anchors(
            &manifest,
            &trust_anchors,
            minimum_governance_quorum,
            now_unix,
        )
        .map_err(|error| format!("moderation trust policy validation failed: {error}"))?;
    ensure_parent_dir(&provenance_path)?;
    let provenance = ModerationProvenanceStoreV1::open(&provenance_path, provenance_log_id)
        .map_err(|error| format!("failed to open moderation provenance: {error}"))?;
    let quorum = usize::from(policy.body.result_quorum);
    let listener = TcpListener::bind(listen_addr).map_err(|error| {
        format!("failed to bind authenticated moderation committee at `{listen}`: {error}")
    })?;
    let local_addr = listener
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_else(|_| listen.clone());
    let service = Arc::new(ModerationCommitteeService {
        manifest,
        authenticated: Some(ModerationAuthenticatedCommitteeState {
            trust_policy: policy,
            trust_anchors,
            minimum_governance_quorum,
            provenance,
            transaction_guard: Mutex::new(()),
        }),
        manifest_source: manifest_path.display().to_string(),
        quorum,
        max_body_bytes,
    });
    let status = moderation_committee_status_json(&service, "listening", Some(&local_addr));
    let rendered = to_string_pretty(&status)
        .map_err(|error| format!("failed to render authenticated committee status: {error}"))?;
    println!("{rendered}");
    let active_connections = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let Some(active_permit) = moderation_try_acquire_permit(
                    &active_connections,
                    MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS,
                ) else {
                    let response = moderation_committee_error_response(
                        503,
                        "Service Unavailable",
                        "authenticated moderation committee connection limit reached",
                    );
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    continue;
                };
                let service = Arc::clone(&service);
                thread::spawn(move || {
                    let _active_permit = active_permit;
                    if let Err(error) =
                        moderation_committee_handle_stream(stream, &service, max_body_bytes)
                    {
                        eprintln!("authenticated moderation committee connection failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("authenticated moderation committee accept failed: {error}"),
        }
    }
    Ok(())
}
struct ModerationCommitteeService {
    manifest: ModerationReproManifestV1,
    authenticated: Option<ModerationAuthenticatedCommitteeState>,
    manifest_source: String,
    quorum: usize,
    max_body_bytes: usize,
}
struct ModerationAuthenticatedCommitteeState {
    trust_policy: ModerationTrustPolicyV1,
    trust_anchors: BTreeSet<PublicKey>,
    minimum_governance_quorum: u16,
    provenance: ModerationProvenanceStoreV1,
    transaction_guard: Mutex<()>,
}
fn moderation_committee_handle_stream(
    mut stream: TcpStream,
    service: &ModerationCommitteeService,
    max_body_bytes: usize,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let hard_limit = moderation_http_hard_limit(max_body_bytes, "committee")?;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = hard_limit.saturating_sub(request.len());
        let accepted = count.min(remaining);
        request.try_reserve(accepted).map_err(|_| {
            io::Error::new(io::ErrorKind::OutOfMemory, "HTTP request allocation failed")
        })?;
        request.extend_from_slice(&buffer[..accepted]);
        if accepted < count || request.len() >= hard_limit {
            break;
        }
        if let Some((header_len, content_len)) = moderation_runner_request_lengths(&request)
            && (content_len > max_body_bytes
                || header_len
                    .checked_add(content_len)
                    .is_some_and(|body_end| request.len() >= body_end))
        {
            break;
        }
    }
    let response = moderation_committee_http_response(service, &request, max_body_bytes);
    stream.write_all(&response)?;
    stream.flush()
}
fn moderation_committee_http_response(
    service: &ModerationCommitteeService,
    request: &[u8],
    max_body_bytes: usize,
) -> Vec<u8> {
    match moderation_runner_parse_http_request(request, max_body_bytes) {
        Ok(parsed) => moderation_committee_route_request(service, &parsed),
        Err(response) => response,
    }
}
fn moderation_committee_route_request(
    service: &ModerationCommitteeService,
    request: &ModerationRunnerHttpRequest<'_>,
) -> Vec<u8> {
    match (request.method, request.path) {
        ("GET", "/healthz") | ("GET", "/v1/sorafs/moderation/committee/status") => {
            if let Some(state) = &service.authenticated
                && let Err(error) = state.provenance.snapshot()
            {
                return moderation_committee_error_response(
                    503,
                    "Service Unavailable",
                    &format!("authenticated committee provenance is unhealthy: {error}"),
                );
            }
            moderation_committee_json_response(
                200,
                "OK",
                &moderation_committee_status_json(service, "ready", None),
            )
        }
        ("POST", "/v1/sorafs/moderation/committee/aggregate") => {
            if service.authenticated.is_some() {
                return moderation_committee_error_response(
                    409,
                    "Conflict",
                    "unsigned aggregation is disabled on the authenticated committee; use /v1/sorafs/moderation/committee/aggregate-authenticated",
                );
            }
            match moderation_committee_aggregate_request_json(service, request.body) {
                Ok(value) => moderation_committee_json_response(200, "OK", &value),
                Err(message) => moderation_committee_error_response(400, "Bad Request", &message),
            }
        }
        ("POST", "/v1/sorafs/moderation/committee/aggregate-authenticated") => {
            if service.authenticated.is_none() {
                return moderation_committee_error_response(
                    404,
                    "Not Found",
                    "authenticated aggregation is not configured on this diagnostic committee",
                );
            }
            match moderation_committee_authenticated_request_json(service, request.body) {
                Ok(value) => moderation_committee_json_response(200, "OK", &value),
                Err(ModerationAuthenticatedCommitteeRequestError::BadRequest(message)) => {
                    moderation_committee_error_response(400, "Bad Request", &message)
                }
                Err(ModerationAuthenticatedCommitteeRequestError::Unavailable(message)) => {
                    moderation_committee_error_response(503, "Service Unavailable", &message)
                }
                Err(ModerationAuthenticatedCommitteeRequestError::Internal(message)) => {
                    moderation_committee_error_response(500, "Internal Server Error", &message)
                }
            }
        }
        ("GET", _) | ("POST", _) => moderation_committee_error_response(
            404,
            "Not Found",
            "unknown SoraFS moderation committee endpoint",
        ),
        _ => moderation_committee_error_response(
            405,
            "Method Not Allowed",
            "SoraFS moderation committee supports GET and POST only",
        ),
    }
}
#[derive(Debug)]
enum ModerationAuthenticatedCommitteeRequestError {
    BadRequest(String),
    Unavailable(String),
    Internal(String),
}
fn moderation_committee_authenticated_request_json(
    service: &ModerationCommitteeService,
    body: &[u8],
) -> Result<Value, ModerationAuthenticatedCommitteeRequestError> {
    let bad_request = ModerationAuthenticatedCommitteeRequestError::BadRequest;
    if body.is_empty() {
        return Err(bad_request(
            "authenticated committee request body must not be empty".to_string(),
        ));
    }
    let value: Value = from_slice(body).map_err(|error| {
        bad_request(format!(
            "failed to parse authenticated committee request JSON: {error}"
        ))
    })?;
    if json_contains_key(&value, "payload_b64") {
        return Err(bad_request(
            "authenticated committee requests must not contain payload bytes".to_string(),
        ));
    }
    let fields = value.as_object().ok_or_else(|| {
        bad_request("authenticated committee request must be a JSON object".to_string())
    })?;
    if let Some(field) = fields
        .keys()
        .find(|field| field.as_str() != "signed_results_norito_b64")
    {
        return Err(bad_request(format!(
            "authenticated committee request contains unsupported field `{field}`"
        )));
    }
    let encoded_results = fields
        .get("signed_results_norito_b64")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            bad_request(
                "authenticated committee request requires array `signed_results_norito_b64`"
                    .to_string(),
            )
        })?;
    if encoded_results.is_empty() || encoded_results.len() > MODERATION_COMMITTEE_MAX_RESULTS {
        return Err(bad_request(format!(
            "authenticated committee accepts 1..={MODERATION_COMMITTEE_MAX_RESULTS} signed results"
        )));
    }
    let result_limit = usize::try_from(MODERATION_SIGNED_RESULT_MAX_BYTES).map_err(|_| {
        ModerationAuthenticatedCommitteeRequestError::Internal(
            "signed result bound does not fit usize".to_string(),
        )
    })?;
    let padded_limit = result_limit.checked_add(2).ok_or_else(|| {
        ModerationAuthenticatedCommitteeRequestError::Internal(
            "signed result bound overflows usize".to_string(),
        )
    })?;
    let mut results = Vec::new();
    results
        .try_reserve_exact(encoded_results.len())
        .map_err(|error| {
            ModerationAuthenticatedCommitteeRequestError::Unavailable(format!(
                "failed to reserve bounded authenticated result set: {error}"
            ))
        })?;
    for (index, encoded) in encoded_results.iter().enumerate() {
        let encoded = encoded.as_str().ok_or_else(|| {
            bad_request(format!(
                "signed_results_norito_b64[{index}] must be a base64 string"
            ))
        })?;
        let decoded_capacity = base64::decoded_len_estimate(encoded.len());
        if decoded_capacity > padded_limit {
            return Err(bad_request(format!(
                "signed_results_norito_b64[{index}] can decode beyond the {result_limit}-byte bound"
            )));
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(decoded_capacity).map_err(|error| {
            ModerationAuthenticatedCommitteeRequestError::Unavailable(format!(
                "failed to reserve signed result {index}: {error}"
            ))
        })?;
        bytes.resize(decoded_capacity, 0);
        let decoded = BASE64_STANDARD
            .decode_slice(encoded, &mut bytes)
            .map_err(|error| {
                bad_request(format!(
                    "signed_results_norito_b64[{index}] is invalid base64: {error}"
                ))
            })?;
        bytes.truncate(decoded);
        results.push(
            decode_moderation_signed_result(&bytes, &format!("at request index {index}"))
                .map_err(bad_request)?,
        );
    }
    let state = service.authenticated.as_ref().ok_or_else(|| {
        ModerationAuthenticatedCommitteeRequestError::Internal(
            "authenticated committee state disappeared after route selection".to_string(),
        )
    })?;
    let _transaction = state
        .transaction_guard
        .try_lock()
        .map_err(|error| match error {
            std::sync::TryLockError::WouldBlock => {
                ModerationAuthenticatedCommitteeRequestError::Unavailable(
                    "authenticated committee already has an in-flight transaction".to_string(),
                )
            }
            std::sync::TryLockError::Poisoned(_) => {
                ModerationAuthenticatedCommitteeRequestError::Internal(
                    "authenticated committee transaction lock is poisoned".to_string(),
                )
            }
        })?;
    let now_unix = moderation_trusted_now_unix()
        .map_err(ModerationAuthenticatedCommitteeRequestError::Internal)?;
    state
        .trust_policy
        .validate_with_trust_anchors(
            &service.manifest,
            &state.trust_anchors,
            state.minimum_governance_quorum,
            now_unix,
        )
        .map_err(|error| {
            ModerationAuthenticatedCommitteeRequestError::Unavailable(error.to_string())
        })?;
    let aggregate = ModerationCommitteeAggregateV1::aggregate_authenticated(
        &service.manifest,
        &state.trust_policy,
        &state.trust_anchors,
        state.minimum_governance_quorum,
        &results,
        now_unix,
    )
    .map_err(|error| ModerationAuthenticatedCommitteeRequestError::BadRequest(error.to_string()))?;
    let provenance_head = state
        .provenance
        .append_authenticated_aggregate(
            &service.manifest,
            &state.trust_policy,
            &state.trust_anchors,
            state.minimum_governance_quorum,
            &results,
            aggregate.clone(),
            now_unix,
        )
        .map_err(|error| match error {
            ModerationProvenanceStoreError::Locked(_) => {
                ModerationAuthenticatedCommitteeRequestError::Unavailable(error.to_string())
            }
            _ => ModerationAuthenticatedCommitteeRequestError::Internal(error.to_string()),
        })?;
    let canonical = to_bytes(&aggregate).map_err(|error| {
        ModerationAuthenticatedCommitteeRequestError::Internal(format!(
            "failed to encode authenticated aggregate: {error}"
        ))
    })?;
    if u64::try_from(canonical.len())
        .ok()
        .is_none_or(|length| length > MODERATION_AUTHENTICATED_AGGREGATE_MAX_BYTES)
    {
        return Err(ModerationAuthenticatedCommitteeRequestError::Internal(
            "authenticated aggregate exceeds its hard encoded bound".to_string(),
        ));
    }
    moderation_authenticated_aggregate_summary_json(&aggregate, &canonical, provenance_head)
        .map_err(ModerationAuthenticatedCommitteeRequestError::Internal)
}
fn moderation_committee_aggregate_request_json(
    service: &ModerationCommitteeService,
    body: &[u8],
) -> Result<Value, String> {
    if body.is_empty() {
        return Err("moderation committee aggregate request body must not be empty".to_string());
    }
    let value: Value = from_slice(body).map_err(|err| {
        format!("failed to parse moderation committee aggregate request JSON: {err}")
    })?;
    if json_contains_key(&value, "payload_b64") {
        return Err(
            "moderation committee aggregate request must be payload-free; found `payload_b64`"
                .to_string(),
        );
    }
    let fields = value.as_object().ok_or_else(|| {
        "moderation committee aggregate request must be a JSON object".to_string()
    })?;
    let results = fields
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "moderation committee aggregate request requires array `results`".to_string()
        })?;
    if results.is_empty() {
        return Err(
            "moderation committee aggregate request `results` must not be empty".to_string(),
        );
    }
    if results.len() > MODERATION_COMMITTEE_MAX_RESULTS {
        return Err(format!(
            "moderation committee aggregate request accepts at most {MODERATION_COMMITTEE_MAX_RESULTS} results"
        ));
    }
    let notes = optional_json_string(fields, "notes", "moderation committee aggregate request")?;
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(results.len())
        .map_err(|error| format!("failed to reserve bounded committee inputs: {error}"))?;
    for (idx, result) in results.iter().enumerate() {
        inputs.push(parse_moderation_committee_input_value(
            &format!("request.results[{idx}]"),
            result,
            &service.manifest,
        )?);
    }
    inputs.sort_by_key(|input| input.source_path.display().to_string());
    moderation_committee_aggregate_json(&service.manifest, &inputs, service.quorum, notes)
}
fn moderation_committee_status_json(
    service: &ModerationCommitteeService,
    status: &str,
    listen: Option<&str>,
) -> Value {
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.committee.status.v1");
    insert_value!(output["status"] = status.to_string());
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["manifest_source"] = service.manifest_source.clone());
    insert_value!(output["manifest_id_hex"] = hex_encode(service.manifest.body.manifest_id));
    insert_value!(output["runner_hash_hex"] = hex_encode(service.manifest.body.runner_hash));
    insert_value!(output["quorum"] = service.quorum as u64);
    insert_value!(output["aggregation"] = "median_score_bps");
    insert_value!(output["model_count"] = service.manifest.body.models.len() as u64);
    insert_value!(output["max_body_bytes"] = service.max_body_bytes as u64);
    insert_value!(output["outbound_network"] = "network_capable_process_policy_required");
    insert_value!(output["process_isolation"] = "external_runtime_attestation_required");
    insert_value!(output["process_isolation_verified"] = false);
    if let Some(state) = &service.authenticated {
        let policy = &state.trust_policy;
        let snapshot = state.provenance.snapshot().ok();
        insert_value!(output["trust_boundary"] = "externally_anchored_authenticated_committee");
        insert_value!(output["authenticated_results"] = true);
        insert_value!(output["unsigned_aggregation_enabled"] = false);
        insert_value!(
            output["authenticated_aggregation_endpoint"] =
                "/v1/sorafs/moderation/committee/aggregate-authenticated"
        );
        insert_value!(output["trust_policy_id_hex"] = hex_encode(policy.body.policy_id));
        insert_value!(output["trust_policy_digest_hex"] = hex_encode(policy.body.policy_digest));
        insert_value!(
            output["minimum_governance_quorum"] = u64::from(state.minimum_governance_quorum)
        );
        insert_value!(
            output["trusted_governance_anchor_count"] = u64::try_from(state.trust_anchors.len())
                .expect("bounded governance anchor count fits u64")
        );
        insert_value!(output["provenance_path"] = state.provenance.path().display().to_string());
        insert_value!(output["provenance_verified"] = snapshot.is_some());
        insert_json!(
            output["provenance_entry_count"] = snapshot
                .as_ref()
                .map(|value| {
                    Value::from(
                        u64::try_from(value.entries.len())
                            .expect("bounded provenance entry count fits u64"),
                    )
                })
                .unwrap_or(Value::Null)
        );
        insert_json!(
            output["provenance_head_digest_hex"] = snapshot
                .map(|value| Value::from(hex_encode(value.head_digest)))
                .unwrap_or(Value::Null)
        );
        insert_value!(output["max_authenticated_in_flight"] = 1_u64);
    } else {
        insert_value!(output["trust_boundary"] = "unsigned_diagnostic_only");
        insert_value!(output["authenticated_results"] = false);
        insert_value!(output["unsigned_aggregation_enabled"] = true);
        insert_value!(output["provenance_verified"] = false);
    }
    insert_json!(
        output["listen"] = listen
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    Value::Object(output)
}
fn moderation_committee_json_response(status: u16, reason: &str, value: &Value) -> Vec<u8> {
    let body = to_vec(value).unwrap_or_else(|_| b"{\"error\":\"json_render_failed\"}".to_vec());
    moderation_runner_http_response_bytes(status, reason, "application/json", &body)
}
fn moderation_committee_error_response(status: u16, reason: &str, message: &str) -> Vec<u8> {
    let mut body = Map::new();
    insert_value!(body["schema"] = "sorafs.moderation.committee.error.v1");
    insert_value!(body["status"] = "error");
    insert_value!(body["message"] = message.to_string());
    moderation_committee_json_response(status, reason, &Value::Object(body))
}
fn moderation_committee_canary(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_path: Option<PathBuf> = None;
    let mut format = String::from("json");
    let mut committee_url: Option<String> = None;
    let mut quorum: Option<usize> = None;
    let mut result_paths: Vec<PathBuf> = Vec::new();
    let mut generated_at_unix: Option<u64> = None;
    let mut deployment_id: Option<String> = None;
    let mut environment: Option<String> = None;
    let mut deployment_context_reviewed = false;
    let mut process_isolation_enforcement: Option<&'static str> = None;
    let mut process_isolation_attestation_digest: Option<[u8; 32]> = None;
    let mut process_isolation_verified_at: Option<u64> = None;
    let mut process_isolation_reviewed = false;
    let mut checked_at_unix: Option<u64> = None;
    let mut notes: Option<String> = None;
    let mut timeout_ms = 30_000_u64;
    let mut json_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--format" => format = value.to_ascii_lowercase(),
            "--committee-url" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--committee-url` must not be empty".to_string());
                }
                committee_url = Some(trimmed.to_string());
            }
            "--quorum" => {
                let parsed =
                    parse_u64_arg("--quorum", value, "sorafs_cli moderation committee-canary")?;
                let parsed = usize::try_from(parsed).map_err(|_| {
                    "`--quorum` does not fit into this platform's usize".to_string()
                })?;
                if parsed == 0 {
                    return Err("`--quorum` must be greater than zero".to_string());
                }
                quorum = Some(parsed);
            }
            "--result" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--result` path must not be empty".to_string());
                }
                result_paths.push(PathBuf::from(trimmed));
            }
            "--generated-at-unix" => {
                let generated = parse_u64_arg(
                    "--generated-at-unix",
                    value,
                    "sorafs_cli moderation committee-canary",
                )?;
                if generated == 0 {
                    return Err("`--generated-at-unix` must be greater than zero".to_string());
                }
                generated_at_unix = Some(generated);
            }
            "--deployment-id" => {
                deployment_id = Some(moderation_canary_deployment_id(value)?);
            }
            "--environment" => {
                environment = Some(moderation_canary_environment(value)?);
            }
            "--deployment-context-reviewed" => {
                if value != "true" {
                    return Err(
                        "`--deployment-context-reviewed` must be exactly `true`".to_string()
                    );
                }
                deployment_context_reviewed = true;
            }
            "--process-isolation-enforcement" => {
                process_isolation_enforcement = Some(match value {
                    "systemd_ip_filter" => "systemd_ip_filter",
                    "container_network_policy" => "container_network_policy",
                    "host_firewall" => "host_firewall",
                    _ => {
                        return Err("`--process-isolation-enforcement` must be one of `systemd_ip_filter`, `container_network_policy`, or `host_firewall`".to_string());
                    }
                });
            }
            "--process-isolation-attestation-digest" => {
                if value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err("`--process-isolation-attestation-digest` must be exactly 64 lowercase hexadecimal characters".to_string());
                }
                let digest = parse_fixed_hex::<32>(
                    value,
                    "--process-isolation-attestation-digest",
                    "sorafs_cli moderation committee-canary",
                )?;
                if moderation_digest_is_placeholder(&digest) {
                    return Err("`--process-isolation-attestation-digest` must not be a zero/repeated placeholder digest".to_string());
                }
                process_isolation_attestation_digest = Some(digest);
            }
            "--process-isolation-verified-at" => {
                let verified_at = parse_u64_arg(
                    "--process-isolation-verified-at",
                    value,
                    "sorafs_cli moderation committee-canary",
                )?;
                if verified_at == 0 {
                    return Err(
                        "`--process-isolation-verified-at` must be greater than zero".to_string(),
                    );
                }
                process_isolation_verified_at = Some(verified_at);
            }
            "--process-isolation-reviewed" => {
                if value != "true" {
                    return Err("`--process-isolation-reviewed` must be exactly `true`".to_string());
                }
                process_isolation_reviewed = true;
            }
            "--checked-at" => {
                checked_at_unix = Some(parse_u64_arg(
                    "--checked-at",
                    value,
                    "sorafs_cli moderation committee-canary",
                )?);
            }
            "--notes" => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    notes = Some(trimmed.to_string());
                }
            }
            "--timeout-ms" => {
                timeout_ms = parse_u64_arg(
                    "--timeout-ms",
                    value,
                    "sorafs_cli moderation committee-canary",
                )?;
                if timeout_ms == 0 {
                    return Err("`--timeout-ms` must be greater than zero".to_string());
                }
            }
            "--json-out" => json_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli moderation committee-canary`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    let committee_url = committee_url.ok_or_else(|| {
        "missing required `--committee-url=URL` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    let quorum = quorum.ok_or_else(|| {
        "missing required `--quorum=N` for `sorafs_cli moderation committee-canary`".to_string()
    })?;
    if result_paths.is_empty() {
        return Err("provide at least one `--result=PATH` for committee canary".to_string());
    }
    if result_paths.len() > MODERATION_COMMITTEE_MAX_RESULTS {
        return Err(format!(
            "committee canary accepts at most {MODERATION_COMMITTEE_MAX_RESULTS} result files"
        ));
    }
    if quorum > result_paths.len() {
        return Err(format!(
            "committee canary requires quorum {quorum} but only {} result file(s) were provided",
            result_paths.len()
        ));
    }
    let generated_at_unix = generated_at_unix.ok_or_else(|| {
        "missing required `--generated-at-unix=UNIX_SECS` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    let deployment_id = deployment_id.ok_or_else(|| {
        "missing required `--deployment-id=ID` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    let environment = environment.ok_or_else(|| {
        "missing required `--environment=ENV` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    if !deployment_context_reviewed {
        return Err(
            "missing required `--deployment-context-reviewed=true` for `sorafs_cli moderation committee-canary`"
                .to_string(),
        );
    }
    let process_isolation_enforcement = process_isolation_enforcement.ok_or_else(|| {
        "missing required `--process-isolation-enforcement=KIND` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    let process_isolation_attestation_digest = process_isolation_attestation_digest.ok_or_else(|| {
        "missing required `--process-isolation-attestation-digest=HEX` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    let process_isolation_verified_at = process_isolation_verified_at.ok_or_else(|| {
        "missing required `--process-isolation-verified-at=UNIX_SECS` for `sorafs_cli moderation committee-canary`"
            .to_string()
    })?;
    if !process_isolation_reviewed {
        return Err(
            "missing required `--process-isolation-reviewed=true` for `sorafs_cli moderation committee-canary`"
                .to_string(),
        );
    }
    let checked_at_unix = checked_at_unix.unwrap_or(generated_at_unix);
    if checked_at_unix != generated_at_unix {
        return Err("`--checked-at` must equal `--generated-at-unix`".to_string());
    }
    if process_isolation_verified_at > generated_at_unix {
        return Err(
            "`--process-isolation-verified-at` must not be after `--generated-at-unix`".to_string(),
        );
    }
    let process_isolation = ModerationProcessIsolationEvidence {
        enforcement: process_isolation_enforcement,
        attestation_digest: process_isolation_attestation_digest,
        verified_at_unix: process_isolation_verified_at,
    };
    let deployment_context = ModerationCanaryDeploymentContext {
        generated_at_unix,
        deployment_id,
        environment,
    };
    let manifest = load_moderation_repro_manifest(
        &manifest_path,
        &format,
        "sorafs_cli moderation committee-canary",
    )?;
    manifest
        .validate()
        .map_err(|err| format!("manifest validation failed: {err}"))?;
    validate_moderation_local_runner_manifest(&manifest)?;
    let mut result_values = Vec::new();
    result_values
        .try_reserve_exact(result_paths.len())
        .map_err(|error| format!("failed to reserve bounded canary result values: {error}"))?;
    let mut result_fingerprints = Vec::new();
    result_fingerprints
        .try_reserve_exact(result_paths.len())
        .map_err(|error| format!("failed to reserve bounded canary fingerprints: {error}"))?;
    let mut seen_result_digests = BTreeSet::new();
    for path in &result_paths {
        let result = load_moderation_committee_result_value(path, &manifest)?;
        if !seen_result_digests.insert(result.body_blake3) {
            return Err(
                "committee canary result files must have unique body fingerprints".to_string(),
            );
        }
        result_values.push(result.value);
        result_fingerprints.push(result.fingerprint);
    }
    let expected_aggregate = moderation_committee_expected_aggregate_from_values(
        &manifest,
        &result_values,
        quorum,
        notes.as_deref(),
    )?;
    let request =
        moderation_committee_canary_aggregate_request_json(&result_values, notes.as_deref());
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| format!("failed to construct committee canary HTTP client: {err}"))?;
    let base_url = Url::parse(&committee_url)
        .map_err(|err| format!("invalid `--committee-url` `{committee_url}`: {err}"))?;
    let status_url =
        moderation_runner_canary_endpoint(&base_url, "/v1/sorafs/moderation/committee/status")?;
    let aggregate_url =
        moderation_runner_canary_endpoint(&base_url, "/v1/sorafs/moderation/committee/aggregate")?;
    let committee_base_url = base_url.as_str().trim_end_matches('/');
    let status_probe = moderation_committee_canary_get_json(&client, &status_url)?;
    let aggregate_probe = moderation_committee_canary_post_json(&client, &aggregate_url, &request)?;
    let evidence =
        moderation_committee_canary_evidence_json(ModerationCommitteeCanaryEvidenceInput {
            manifest: &manifest,
            committee_url: committee_base_url,
            status_url: status_url.as_str(),
            aggregate_url: aggregate_url.as_str(),
            quorum,
            checked_at_unix,
            deployment_context,
            process_isolation,
            notes: notes.as_deref(),
            result_fingerprints,
            expected_aggregate,
            status_probe,
            aggregate_probe,
        })?;
    let rendered = to_string_pretty(&evidence)
        .map_err(|err| format!("failed to render committee canary evidence JSON: {err}"))?;
    if let Some(path) = json_out {
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
struct ModerationCommitteeCanaryEvidenceInput<'a> {
    manifest: &'a ModerationReproManifestV1,
    committee_url: &'a str,
    status_url: &'a str,
    aggregate_url: &'a str,
    quorum: usize,
    checked_at_unix: u64,
    deployment_context: ModerationCanaryDeploymentContext,
    process_isolation: ModerationProcessIsolationEvidence,
    notes: Option<&'a str>,
    result_fingerprints: Vec<Value>,
    expected_aggregate: Value,
    status_probe: ModerationCanaryHttpProbe,
    aggregate_probe: ModerationCanaryHttpProbe,
}
struct ModerationCommitteeCanaryResult {
    value: Value,
    body_blake3: [u8; 32],
    fingerprint: Value,
}
fn load_moderation_committee_result_value(
    path: &Path,
    manifest: &ModerationReproManifestV1,
) -> Result<ModerationCommitteeCanaryResult, String> {
    let bytes = read_file_bounded(
        path,
        MODERATION_COMMITTEE_MAX_RESULT_BYTES,
        "moderation committee canary result",
    )?;
    let value: Value = from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse committee result JSON `{}`: {err}",
            path.display()
        )
    })?;
    if json_contains_key(&value, "payload_b64") {
        return Err(format!(
            "committee canary result `{}` must be payload-free; found `payload_b64`",
            path.display()
        ));
    }
    parse_moderation_committee_input_value(&path.display().to_string(), &value, manifest)?;
    let body_blake3 = *blake3_hash(&bytes).as_bytes();
    let bytes_len = u64::try_from(bytes.len())
        .map_err(|_| "committee canary result length exceeds u64".to_string())?;
    let mut fingerprint = Map::new();
    insert_value!(
        fingerprint["name"] = format!("ai-prescreen-committee-result-{}", hex_encode(body_blake3))
    );
    insert_value!(fingerprint["bytes"] = bytes_len);
    insert_value!(fingerprint["body_blake3_hex"] = hex_encode(body_blake3));
    insert_value!(fingerprint["payload_bytes_included"] = false);
    insert_value!(fingerprint["private_payloads_included"] = false);
    Ok(ModerationCommitteeCanaryResult {
        value,
        body_blake3,
        fingerprint: Value::Object(fingerprint),
    })
}
fn moderation_committee_expected_aggregate_from_values(
    manifest: &ModerationReproManifestV1,
    result_values: &[Value],
    quorum: usize,
    notes: Option<&str>,
) -> Result<Value, String> {
    if result_values.len() > MODERATION_COMMITTEE_MAX_RESULTS {
        return Err(format!(
            "committee canary accepts at most {MODERATION_COMMITTEE_MAX_RESULTS} results"
        ));
    }
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(result_values.len())
        .map_err(|error| format!("failed to reserve bounded committee canary inputs: {error}"))?;
    for (idx, value) in result_values.iter().enumerate() {
        inputs.push(parse_moderation_committee_input_value(
            &format!("request.results[{idx}]"),
            value,
            manifest,
        )?);
    }
    inputs.sort_by_key(|input| input.source_path.display().to_string());
    moderation_committee_aggregate_json(manifest, &inputs, quorum, notes)
}
fn moderation_committee_canary_aggregate_request_json(
    result_values: &[Value],
    notes: Option<&str>,
) -> Value {
    let mut request = Map::new();
    insert_json!(request["results"] = Value::Array(result_values.to_vec()));
    insert_json!(
        request["notes"] = notes
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    Value::Object(request)
}
fn moderation_committee_canary_get_json(
    client: &HttpClient,
    url: &Url,
) -> Result<ModerationCanaryHttpProbe, String> {
    let response = client
        .get(url.as_str())
        .send()
        .map_err(|err| format!("committee canary GET `{url}` failed: {err}"))?;
    let (status, bytes) = read_moderation_canary_response_bounded(
        response,
        &format!("committee canary GET `{url}`"),
    )?;
    if !status.is_success() {
        return Err(format!(
            "committee canary GET `{url}` returned HTTP {status}: {}",
            body_snippet(&bytes)
        ));
    }
    let response_bytes = u64::try_from(bytes.len())
        .map_err(|_| "committee canary GET response length exceeds u64".to_string())?;
    let response = from_slice(&bytes)
        .map_err(|err| format!("committee canary GET `{url}` returned invalid JSON: {err}"))?;
    Ok(ModerationCanaryHttpProbe {
        method: "GET",
        url: url.as_str().to_string(),
        status_code: status.as_u16(),
        request_bytes: 0,
        request_body_blake3: *blake3_hash(&[]).as_bytes(),
        response_bytes,
        response_body_blake3: *blake3_hash(&bytes).as_bytes(),
        response,
    })
}
fn moderation_committee_canary_post_json(
    client: &HttpClient,
    url: &Url,
    value: &Value,
) -> Result<ModerationCanaryHttpProbe, String> {
    let body = to_vec(value)
        .map_err(|err| format!("failed to encode committee canary request JSON: {err}"))?;
    if body.len() > MODERATION_RUNNER_HARD_MAX_BODY_BYTES {
        return Err(format!(
            "committee canary request has {} bytes; maximum is {MODERATION_RUNNER_HARD_MAX_BODY_BYTES}",
            body.len()
        ));
    }
    let request_bytes = u64::try_from(body.len())
        .map_err(|_| "committee canary POST request length exceeds u64".to_string())?;
    let request_body_blake3 = *blake3_hash(&body).as_bytes();
    let response = client
        .post(url.as_str())
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .map_err(|err| format!("committee canary POST `{url}` failed: {err}"))?;
    let (status, bytes) = read_moderation_canary_response_bounded(
        response,
        &format!("committee canary POST `{url}`"),
    )?;
    if !status.is_success() {
        return Err(format!(
            "committee canary POST `{url}` returned HTTP {status}: {}",
            body_snippet(&bytes)
        ));
    }
    let response_bytes = u64::try_from(bytes.len())
        .map_err(|_| "committee canary POST response length exceeds u64".to_string())?;
    let response = from_slice(&bytes)
        .map_err(|err| format!("committee canary POST `{url}` returned invalid JSON: {err}"))?;
    Ok(ModerationCanaryHttpProbe {
        method: "POST",
        url: url.as_str().to_string(),
        status_code: status.as_u16(),
        request_bytes,
        request_body_blake3,
        response_bytes,
        response_body_blake3: *blake3_hash(&bytes).as_bytes(),
        response,
    })
}
fn moderation_committee_canary_evidence_json(
    input: ModerationCommitteeCanaryEvidenceInput<'_>,
) -> Result<Value, String> {
    input.process_isolation.validate(
        input.deployment_context.generated_at_unix,
        "committee canary evidence",
    )?;
    validate_moderation_committee_status_response(
        input.manifest,
        input.quorum,
        &input.status_probe.response,
    )?;
    validate_moderation_committee_aggregate_response(
        input.manifest,
        input.quorum,
        &input.expected_aggregate,
        &input.aggregate_probe.response,
    )?;
    if json_contains_key(&input.status_probe.response, "payload_b64")
        || json_contains_key(&input.aggregate_probe.response, "payload_b64")
    {
        return Err(
            "committee canary evidence responses must not contain `payload_b64`".to_string(),
        );
    }
    let aggregate = input
        .aggregate_probe
        .response
        .as_object()
        .ok_or_else(|| "committee aggregate response must be a JSON object".to_string())?;
    let subject = required_json_string(aggregate, "subject", "committee aggregate response")?;
    let subject_digest_hex = required_json_string(
        aggregate,
        "subject_digest_hex",
        "committee aggregate response",
    )?;
    let score = aggregate
        .get("aggregated_score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "committee aggregate response requires numeric `aggregated_score_bps`".to_string()
        })?;
    let verdict = required_json_string(aggregate, "verdict", "committee aggregate response")?;
    let result_count = aggregate
        .get("result_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "committee aggregate response requires numeric `result_count`".to_string()
        })?;
    if result_count
        != u64::try_from(input.result_fingerprints.len())
            .map_err(|_| "committee result fingerprint count exceeds u64".to_string())?
    {
        return Err(
            "committee result fingerprint count does not match aggregate result_count".to_string(),
        );
    }
    let probes = Value::Array(vec![
        moderation_canary_probe_json("status", &input.status_probe),
        moderation_canary_probe_json("aggregate", &input.aggregate_probe),
    ]);
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.committee.rollout_evidence.v1");
    insert_value!(output["status"] = "verified");
    insert_value!(output["synthetic"] = false);
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["generated_at_unix"] = input.deployment_context.generated_at_unix);
    insert_value!(output["deployment_id"] = input.deployment_context.deployment_id);
    insert_value!(output["environment"] = input.deployment_context.environment);
    insert_value!(output["deployment_context_reviewed"] = true);
    insert_value!(output["outbound_network"] = "network_capable_process_policy_required");
    insert_json!(
        output["process_isolation_evidence"] = Value::Object(Map::from_iter([
            ("required".into(), Value::from(true)),
            ("status".into(), Value::from("runtime_verified")),
            (
                "enforcement".into(),
                Value::from(input.process_isolation.enforcement),
            ),
            (
                "attestation_digest_hex".into(),
                Value::from(hex_encode(input.process_isolation.attestation_digest)),
            ),
            (
                "verified_at_unix".into(),
                Value::from(input.process_isolation.verified_at_unix),
            ),
            ("reviewed".into(), Value::from(true)),
            ("synthetic".into(), Value::from(false)),
        ]))
    );
    insert_value!(output["committee_url"] = input.committee_url.to_string());
    insert_value!(output["status_url"] = input.status_url.to_string());
    insert_value!(output["aggregate_url"] = input.aggregate_url.to_string());
    insert_value!(output["manifest_id_hex"] = hex_encode(input.manifest.body.manifest_id));
    insert_value!(output["runner_hash_hex"] = hex_encode(input.manifest.body.runner_hash));
    insert_value!(output["quorum"] = input.quorum as u64);
    insert_value!(output["aggregation"] = "median_score_bps");
    insert_value!(output["result_count"] = result_count);
    insert_json!(output["results"] = Value::Array(input.result_fingerprints));
    insert_value!(output["subject"] = subject.to_string());
    insert_value!(output["subject_digest_hex"] = subject_digest_hex.to_string());
    insert_value!(output["aggregated_score_bps"] = score);
    insert_value!(output["verdict"] = verdict.to_string());
    insert_value!(output["checked_at_unix"] = input.checked_at_unix);
    insert_value!(output["probe_count"] = 2_u64);
    insert_value!(output["passed_probe_count"] = 2_u64);
    insert_json!(output["probes"] = probes);
    insert_json!(
        output["notes"] = input
            .notes
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    insert_json!(output["committee_status"] = input.status_probe.response);
    insert_json!(output["committee_aggregate"] = input.aggregate_probe.response);
    Ok(Value::Object(output))
}
fn validate_moderation_committee_status_response(
    manifest: &ModerationReproManifestV1,
    quorum: usize,
    value: &Value,
) -> Result<(), String> {
    let fields = value
        .as_object()
        .ok_or_else(|| "committee status response must be a JSON object".to_string())?;
    let context = "committee status response";
    let schema = required_json_string(fields, "schema", context)?;
    if schema != "sorafs.moderation.committee.status.v1" {
        return Err(format!("{context} has unexpected schema `{schema}`"));
    }
    let status = required_json_string(fields, "status", context)?;
    if status != "ready" {
        return Err(format!("{context} status `{status}` is not `ready`"));
    }
    let manifest_id = parse_fixed_hex::<16>(
        required_json_string(fields, "manifest_id_hex", context)?,
        "manifest_id_hex",
        context,
    )?;
    if manifest_id != manifest.body.manifest_id {
        return Err(format!(
            "{context} manifest id {} does not match locked manifest {}",
            hex_encode(manifest_id),
            hex_encode(manifest.body.manifest_id)
        ));
    }
    let runner_hash = parse_fixed_hex::<32>(
        required_json_string(fields, "runner_hash_hex", context)?,
        "runner_hash_hex",
        context,
    )?;
    if runner_hash != manifest.body.runner_hash {
        return Err(format!(
            "{context} runner hash {} does not match locked runner {}",
            hex_encode(runner_hash),
            hex_encode(manifest.body.runner_hash)
        ));
    }
    let actual_quorum = fields
        .get("quorum")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} requires numeric `quorum`"))?;
    if actual_quorum != quorum as u64 {
        return Err(format!(
            "{context} quorum {actual_quorum} does not match expected quorum {quorum}"
        ));
    }
    let aggregation = required_json_string(fields, "aggregation", context)?;
    if aggregation != "median_score_bps" {
        return Err(format!(
            "{context} aggregation `{aggregation}` is not `median_score_bps`"
        ));
    }
    let outbound = required_json_string(fields, "outbound_network", context)?;
    if outbound != "network_capable_process_policy_required" {
        return Err(format!(
            "{context} outbound_network `{outbound}` is not `network_capable_process_policy_required`"
        ));
    }
    let isolation = required_json_string(fields, "process_isolation", context)?;
    if isolation != "external_runtime_attestation_required" {
        return Err(format!(
            "{context} process_isolation `{isolation}` is not `external_runtime_attestation_required`"
        ));
    }
    if fields
        .get("process_isolation_verified")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(format!(
            "{context} must report process_isolation_verified=false; runtime isolation requires external evidence"
        ));
    }
    Ok(())
}
fn validate_moderation_committee_aggregate_response(
    manifest: &ModerationReproManifestV1,
    quorum: usize,
    expected: &Value,
    value: &Value,
) -> Result<(), String> {
    if value != expected {
        return Err(
            "committee aggregate response does not match deterministic local aggregation"
                .to_string(),
        );
    }
    let fields = value
        .as_object()
        .ok_or_else(|| "committee aggregate response must be a JSON object".to_string())?;
    let context = "committee aggregate response";
    let schema = required_json_string(fields, "schema", context)?;
    if schema != "sorafs.moderation.committee.aggregate.v1" {
        return Err(format!("{context} has unexpected schema `{schema}`"));
    }
    let status = required_json_string(fields, "status", context)?;
    if status != "quorum_satisfied" {
        return Err(format!(
            "{context} status `{status}` is not `quorum_satisfied`"
        ));
    }
    let manifest_id = parse_fixed_hex::<16>(
        required_json_string(fields, "manifest_id_hex", context)?,
        "manifest_id_hex",
        context,
    )?;
    if manifest_id != manifest.body.manifest_id {
        return Err(format!(
            "{context} manifest id {} does not match locked manifest {}",
            hex_encode(manifest_id),
            hex_encode(manifest.body.manifest_id)
        ));
    }
    let runner_hash = parse_fixed_hex::<32>(
        required_json_string(fields, "runner_hash_hex", context)?,
        "runner_hash_hex",
        context,
    )?;
    if runner_hash != manifest.body.runner_hash {
        return Err(format!(
            "{context} runner hash {} does not match locked runner {}",
            hex_encode(runner_hash),
            hex_encode(manifest.body.runner_hash)
        ));
    }
    let actual_quorum = fields
        .get("quorum")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} requires numeric `quorum`"))?;
    if actual_quorum != quorum as u64 {
        return Err(format!(
            "{context} quorum {actual_quorum} does not match expected quorum {quorum}"
        ));
    }
    let aggregation = required_json_string(fields, "aggregation", context)?;
    if aggregation != "median_score_bps" {
        return Err(format!(
            "{context} aggregation `{aggregation}` is not `median_score_bps`"
        ));
    }
    let score = fields
        .get("aggregated_score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{context} requires numeric `aggregated_score_bps`"))?;
    if score > 10_000 {
        return Err(format!(
            "{context} aggregated_score_bps {score} exceeds 10000 bps"
        ));
    }
    let score = u16::try_from(score)
        .map_err(|_| format!("{context} `aggregated_score_bps` overflowed u16"))?;
    let verdict = required_json_string(fields, "verdict", context)?;
    let expected_verdict = moderation_score_verdict(score, manifest.body.thresholds);
    if verdict != expected_verdict {
        return Err(format!(
            "{context} verdict `{verdict}` does not match score-derived verdict `{expected_verdict}`"
        ));
    }
    Ok(())
}
struct ModerationRunnerService {
    runner: LoadedModerationRunnerV1,
    signed: Option<ModerationSignedRunnerState>,
    manifest_source: String,
    max_body_bytes: usize,
    max_payload_bytes: u32,
}
struct ModerationSignedRunnerState {
    signing_runner: LoadedModerationSigningRunnerV1,
    provenance: ModerationProvenanceStoreV1,
    trust_anchors: BTreeSet<PublicKey>,
    minimum_governance_quorum: u16,
    transaction_guard: Mutex<()>,
}
impl ModerationRunnerService {
    fn manifest(&self) -> &ModerationReproManifestV1 {
        self.runner.manifest()
    }
}
#[derive(Clone, PartialEq, prost::Message)]
struct ModerationRunnerStatusRequest {}
#[derive(Clone, PartialEq, prost::Message)]
struct ModerationRunnerStatusResponse {
    #[prost(string, tag = "1")]
    schema: String,
    #[prost(string, tag = "2")]
    status: String,
    #[prost(string, tag = "3")]
    source: String,
    #[prost(string, tag = "4")]
    manifest_source: String,
    #[prost(string, tag = "5")]
    manifest_id_hex: String,
    #[prost(string, tag = "6")]
    manifest_digest_hex: String,
    #[prost(string, tag = "7")]
    runner_hash_hex: String,
    #[prost(string, tag = "8")]
    runtime_version: String,
    #[prost(uint64, tag = "9")]
    model_count: u64,
    #[prost(uint64, tag = "10")]
    max_body_bytes: u64,
    #[prost(string, tag = "11")]
    outbound_network: String,
    #[prost(string, tag = "12")]
    listen: String,
    #[prost(uint64, tag = "13")]
    max_payload_bytes: u64,
    #[prost(uint64, tag = "14")]
    max_active_connections: u64,
    #[prost(string, tag = "15")]
    process_isolation: String,
    #[prost(bool, tag = "16")]
    process_isolation_verified: bool,
    #[prost(uint64, tag = "17")]
    max_grpc_in_flight: u64,
    #[prost(uint64, tag = "18")]
    max_grpc_response_bytes: u64,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ModerationRunnerScreenRequest {
    #[prost(string, tag = "1")]
    subject: String,
    #[prost(bytes = "vec", tag = "2")]
    payload: Vec<u8>,
    #[prost(uint64, tag = "3")]
    screened_at_unix: u64,
    #[prost(string, optional, tag = "4")]
    notes: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ModerationRunnerScreenResponse {
    #[prost(string, tag = "1")]
    subject: String,
    #[prost(string, tag = "2")]
    subject_digest_hex: String,
    #[prost(string, tag = "3")]
    manifest_id_hex: String,
    #[prost(string, tag = "4")]
    runner_hash_hex: String,
    #[prost(uint32, tag = "5")]
    combined_score_bps: u32,
    #[prost(string, tag = "6")]
    verdict: String,
    #[prost(uint64, tag = "7")]
    screened_at_unix: u64,
    #[prost(string, tag = "8")]
    evidence_digest_hex: String,
    #[prost(string, tag = "9")]
    policy_digest_hex: String,
    #[prost(string, optional, tag = "10")]
    notes: Option<String>,
    #[prost(message, repeated, tag = "11")]
    model_scores: Vec<ModerationRunnerModelScore>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ModerationRunnerModelScore {
    #[prost(string, tag = "1")]
    model_id_hex: String,
    #[prost(string, tag = "2")]
    artifact_digest_hex: String,
    #[prost(uint32, tag = "3")]
    score_bps: u32,
}
#[derive(Clone)]
struct ModerationRunnerGrpcHandler {
    service: Arc<ModerationRunnerService>,
    listen: String,
    in_flight: Arc<AtomicUsize>,
}
#[tonic::async_trait]
impl moderation_runner_grpc::runner_server::Runner for ModerationRunnerGrpcHandler {
    async fn status(
        &self,
        _request: tonic::Request<ModerationRunnerStatusRequest>,
    ) -> Result<tonic::Response<ModerationRunnerStatusResponse>, tonic::Status> {
        Ok(tonic::Response::new(moderation_runner_status_proto(
            &self.service,
            "ready",
            Some(&self.listen),
        )))
    }
    async fn screen(
        &self,
        request: tonic::Request<ModerationRunnerScreenRequest>,
    ) -> Result<tonic::Response<ModerationRunnerScreenResponse>, tonic::Status> {
        let _permit =
            moderation_try_acquire_permit(&self.in_flight, MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT)
                .ok_or_else(|| {
                    tonic::Status::resource_exhausted("moderation runner is overloaded")
                })?;
        moderation_runner_screen_request_proto(&self.service, request.into_inner())
            .map(tonic::Response::new)
            .map_err(tonic::Status::invalid_argument)
    }
}
mod moderation_runner_grpc {
    use super::{
        ModerationRunnerScreenRequest, ModerationRunnerScreenResponse,
        ModerationRunnerStatusRequest, ModerationRunnerStatusResponse,
    };
    use std::{convert::Infallible, sync::Arc, task::Poll};
    use tonic::codegen::*;
    pub mod runner_server {
        use super::*;
        #[tonic::async_trait]
        pub trait Runner: Send + Sync + 'static {
            async fn status(
                &self,
                request: tonic::Request<ModerationRunnerStatusRequest>,
            ) -> Result<tonic::Response<ModerationRunnerStatusResponse>, tonic::Status>;
            async fn screen(
                &self,
                request: tonic::Request<ModerationRunnerScreenRequest>,
            ) -> Result<tonic::Response<ModerationRunnerScreenResponse>, tonic::Status>;
        }
        #[derive(Debug)]
        pub struct RunnerServer<T> {
            inner: Arc<T>,
            max_decoding_message_size: Option<usize>,
            max_encoding_message_size: Option<usize>,
        }
        impl<T> RunnerServer<T> {
            pub fn new(inner: T) -> Self {
                Self {
                    inner: Arc::new(inner),
                    max_decoding_message_size: None,
                    max_encoding_message_size: None,
                }
            }
            pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
                self.max_decoding_message_size = Some(limit);
                self
            }
            pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
                self.max_encoding_message_size = Some(limit);
                self
            }
        }
        impl<T: Runner> Clone for RunnerServer<T> {
            fn clone(&self) -> Self {
                Self {
                    inner: Arc::clone(&self.inner),
                    max_decoding_message_size: self.max_decoding_message_size,
                    max_encoding_message_size: self.max_encoding_message_size,
                }
            }
        }
        impl<T, B> Service<http::Request<B>> for RunnerServer<T>
        where
            T: Runner,
            B: Body + Send + 'static,
            B::Error: Into<StdError> + Send + 'static,
        {
            type Response = http::Response<tonic::body::Body>;
            type Error = Infallible;
            type Future = BoxFuture<Self::Response, Self::Error>;
            fn poll_ready(
                &mut self,
                _cx: &mut std::task::Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }
            fn call(&mut self, req: http::Request<B>) -> Self::Future {
                match req.uri().path() {
                    "/sorafs.moderation.runner.v1.Runner/Status" => {
                        #[allow(non_camel_case_types)]
                        struct StatusSvc<T: Runner>(pub Arc<T>);
                        impl<T: Runner> tonic::server::UnaryService<ModerationRunnerStatusRequest> for StatusSvc<T> {
                            type Response = ModerationRunnerStatusResponse;
                            type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                            fn call(
                                &mut self,
                                request: tonic::Request<ModerationRunnerStatusRequest>,
                            ) -> Self::Future {
                                let inner = Arc::clone(&self.0);
                                let fut = async move { (*inner).status(request).await };
                                Box::pin(fut)
                            }
                        }
                        let inner = Arc::clone(&self.inner);
                        let max_decoding_message_size = self.max_decoding_message_size;
                        let max_encoding_message_size = self.max_encoding_message_size;
                        let fut = async move {
                            let method = StatusSvc(inner);
                            let codec = tonic_prost::ProstCodec::default();
                            let mut grpc = tonic::server::Grpc::new(codec);
                            if let Some(limit) = max_decoding_message_size {
                                grpc = grpc.max_decoding_message_size(limit);
                            }
                            if let Some(limit) = max_encoding_message_size {
                                grpc = grpc.max_encoding_message_size(limit);
                            }
                            let response = grpc.unary(method, req).await;
                            Ok(response)
                        };
                        Box::pin(fut)
                    }
                    "/sorafs.moderation.runner.v1.Runner/Screen" => {
                        #[allow(non_camel_case_types)]
                        struct ScreenSvc<T: Runner>(pub Arc<T>);
                        impl<T: Runner> tonic::server::UnaryService<ModerationRunnerScreenRequest> for ScreenSvc<T> {
                            type Response = ModerationRunnerScreenResponse;
                            type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                            fn call(
                                &mut self,
                                request: tonic::Request<ModerationRunnerScreenRequest>,
                            ) -> Self::Future {
                                let inner = Arc::clone(&self.0);
                                let fut = async move { (*inner).screen(request).await };
                                Box::pin(fut)
                            }
                        }
                        let inner = Arc::clone(&self.inner);
                        let max_decoding_message_size = self.max_decoding_message_size;
                        let max_encoding_message_size = self.max_encoding_message_size;
                        let fut = async move {
                            let method = ScreenSvc(inner);
                            let codec = tonic_prost::ProstCodec::default();
                            let mut grpc = tonic::server::Grpc::new(codec);
                            if let Some(limit) = max_decoding_message_size {
                                grpc = grpc.max_decoding_message_size(limit);
                            }
                            if let Some(limit) = max_encoding_message_size {
                                grpc = grpc.max_encoding_message_size(limit);
                            }
                            let response = grpc.unary(method, req).await;
                            Ok(response)
                        };
                        Box::pin(fut)
                    }
                    _ => Box::pin(async move {
                        Ok(http::Response::builder()
                            .status(200)
                            .header("grpc-status", "12")
                            .header("content-type", "application/grpc")
                            .body(tonic::body::Body::empty())
                            .expect("valid unimplemented gRPC response"))
                    }),
                }
            }
        }
        impl<T: Runner> tonic::server::NamedService for RunnerServer<T> {
            const NAME: &'static str = "sorafs.moderation.runner.v1.Runner";
        }
    }
}
fn moderation_runner_handle_stream(
    mut stream: TcpStream,
    service: &ModerationRunnerService,
    max_body_bytes: usize,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let hard_limit = moderation_http_hard_limit(max_body_bytes, "runner")?;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = hard_limit.saturating_sub(request.len());
        let accepted = count.min(remaining);
        request.try_reserve(accepted).map_err(|_| {
            io::Error::new(io::ErrorKind::OutOfMemory, "HTTP request allocation failed")
        })?;
        request.extend_from_slice(&buffer[..accepted]);
        if accepted < count || request.len() >= hard_limit {
            break;
        }
        if let Some((header_len, content_len)) = moderation_runner_request_lengths(&request)
            && (content_len > max_body_bytes
                || header_len
                    .checked_add(content_len)
                    .is_some_and(|body_end| request.len() >= body_end))
        {
            break;
        }
    }
    let response = moderation_runner_http_response(service, &request, max_body_bytes);
    stream.write_all(&response)?;
    stream.flush()
}
fn moderation_runner_http_response(
    service: &ModerationRunnerService,
    request: &[u8],
    max_body_bytes: usize,
) -> Vec<u8> {
    match moderation_runner_parse_http_request(request, max_body_bytes) {
        Ok(parsed) => moderation_runner_route_request(service, &parsed),
        Err(response) => response,
    }
}
struct ModerationRunnerHttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    body: &'a [u8],
}
fn moderation_runner_parse_http_request<'a>(
    request: &'a [u8],
    max_body_bytes: usize,
) -> Result<ModerationRunnerHttpRequest<'a>, Vec<u8>> {
    let Some(header_end) = find_http_header_end(request) else {
        if request.len() > MODERATION_RUNNER_MAX_HEADER_BYTES {
            return Err(moderation_runner_error_response(
                431,
                "Request Header Fields Too Large",
                "moderation runner HTTP headers exceed the configured maximum",
            ));
        }
        return Err(moderation_runner_error_response(
            400,
            "Bad Request",
            "missing HTTP header terminator",
        ));
    };
    if header_end > MODERATION_RUNNER_MAX_HEADER_BYTES {
        return Err(moderation_runner_error_response(
            431,
            "Request Header Fields Too Large",
            "moderation runner HTTP headers exceed the configured maximum",
        ));
    }
    let header_text = match std::str::from_utf8(&request[..header_end]) {
        Ok(text) => text,
        Err(_) => {
            return Err(moderation_runner_error_response(
                400,
                "Bad Request",
                "HTTP headers must be UTF-8",
            ));
        }
    };
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();
    if method.is_empty()
        || raw_path.is_empty()
        || version != "HTTP/1.1"
        || parts.next().is_some()
        || !is_valid_http_header_name(method)
        || !raw_path.starts_with('/')
        || !raw_path.is_ascii()
        || raw_path
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || raw_path.contains('?')
        || raw_path.contains('#')
    {
        return Err(moderation_runner_error_response(
            400,
            "Bad Request",
            "malformed HTTP request line",
        ));
    }
    let mut content_length = None;
    let mut host_seen = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(moderation_runner_error_response(
                400,
                "Bad Request",
                "malformed HTTP header line",
            ));
        };
        if !is_valid_http_header_name(name)
            || value
                .chars()
                .any(|character| character.is_control() && character != '\t')
        {
            return Err(moderation_runner_error_response(
                400,
                "Bad Request",
                "malformed HTTP header name or value",
            ));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(moderation_runner_error_response(
                400,
                "Bad Request",
                "Transfer-Encoding is not supported",
            ));
        }
        if name.eq_ignore_ascii_case("host") {
            if host_seen || value.trim().is_empty() {
                return Err(moderation_runner_error_response(
                    400,
                    "Bad Request",
                    "HTTP/1.1 requires exactly one non-empty Host header",
                ));
            }
            host_seen = true;
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(moderation_runner_error_response(
                    400,
                    "Bad Request",
                    "duplicate Content-Length headers are forbidden",
                ));
            }
            let canonical = value.trim();
            if canonical.is_empty()
                || !canonical.bytes().all(|byte| byte.is_ascii_digit())
                || canonical.len() > 1 && canonical.starts_with('0')
            {
                return Err(moderation_runner_error_response(
                    400,
                    "Bad Request",
                    "Content-Length must be canonical unsigned decimal",
                ));
            }
            content_length = Some(canonical.parse::<usize>().map_err(|err| {
                moderation_runner_error_response(
                    400,
                    "Bad Request",
                    &format!("invalid Content-Length header: {err}"),
                )
            })?);
        }
    }
    if !host_seen {
        return Err(moderation_runner_error_response(
            400,
            "Bad Request",
            "HTTP/1.1 requires exactly one non-empty Host header",
        ));
    }
    let content_length = content_length.unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(moderation_runner_error_response(
            413,
            "Payload Too Large",
            "moderation runner request body exceeds configured maximum",
        ));
    }
    let body_start = header_end + 4;
    let body_end = body_start.checked_add(content_length).ok_or_else(|| {
        moderation_runner_error_response(413, "Payload Too Large", "HTTP body length overflow")
    })?;
    if request.len() < body_end {
        return Err(moderation_runner_error_response(
            400,
            "Bad Request",
            "incomplete HTTP request body",
        ));
    }
    if request.len() != body_end {
        return Err(moderation_runner_error_response(
            400,
            "Bad Request",
            "trailing bytes after the declared HTTP request body are forbidden",
        ));
    }
    Ok(ModerationRunnerHttpRequest {
        method,
        path: raw_path,
        body: &request[body_start..body_end],
    })
}
fn is_valid_http_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.is_ascii()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}
fn moderation_runner_request_lengths(request: &[u8]) -> Option<(usize, usize)> {
    let header_end = find_http_header_end(request)?;
    if header_end > MODERATION_RUNNER_MAX_HEADER_BYTES {
        return Some((header_end + 4, 0));
    }
    let header_text = std::str::from_utf8(&request[..header_end]).ok()?;
    let mut content_length = None;
    for line in header_text.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            return Some((header_end + 4, 0));
        };
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Some((header_end + 4, 0));
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Some((header_end + 4, 0));
            }
            content_length = value.trim().parse().ok();
            if content_length.is_none() {
                return Some((header_end + 4, 0));
            }
        }
    }
    Some((header_end + 4, content_length.unwrap_or(0)))
}
fn find_http_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n")
}
fn moderation_runner_route_request(
    service: &ModerationRunnerService,
    request: &ModerationRunnerHttpRequest<'_>,
) -> Vec<u8> {
    match (request.method, request.path) {
        ("GET", "/healthz") | ("GET", "/v1/sorafs/moderation/runner/status") => {
            if let Some(state) = &service.signed
                && let Err(error) = state.provenance.snapshot()
            {
                return moderation_runner_error_response(
                    503,
                    "Service Unavailable",
                    &format!("signed moderation provenance is unhealthy: {error}"),
                );
            }
            moderation_runner_json_response(
                200,
                "OK",
                &moderation_runner_status_json(service, "ready", None),
            )
        }
        ("POST", "/v1/sorafs/moderation/runner/screen") => {
            if service.signed.is_some() {
                return moderation_runner_error_response(
                    409,
                    "Conflict",
                    "unsigned screening is disabled on the signed moderation runner; use /v1/sorafs/moderation/runner/screen-signed",
                );
            }
            match moderation_runner_screen_request_json(service, request.body) {
                Ok(value) => moderation_runner_json_response(200, "OK", &value),
                Err(message) => moderation_runner_error_response(400, "Bad Request", &message),
            }
        }
        ("POST", "/v1/sorafs/moderation/runner/screen-signed") => {
            if service.signed.is_none() {
                return moderation_runner_error_response(
                    404,
                    "Not Found",
                    "signed screening is not configured on this diagnostic runner",
                );
            }
            match moderation_runner_signed_screen_request_json(service, request.body) {
                Ok(value) => moderation_runner_json_response(200, "OK", &value),
                Err(ModerationSignedRunnerRequestError::BadRequest(message)) => {
                    moderation_runner_error_response(400, "Bad Request", &message)
                }
                Err(ModerationSignedRunnerRequestError::Unavailable(message)) => {
                    moderation_runner_error_response(503, "Service Unavailable", &message)
                }
                Err(ModerationSignedRunnerRequestError::Internal(message)) => {
                    moderation_runner_error_response(500, "Internal Server Error", &message)
                }
            }
        }
        ("GET", _) | ("POST", _) => moderation_runner_error_response(
            404,
            "Not Found",
            "unknown SoraFS moderation runner endpoint",
        ),
        _ => moderation_runner_error_response(
            405,
            "Method Not Allowed",
            "SoraFS moderation runner supports GET and POST only",
        ),
    }
}
#[derive(Debug)]
enum ModerationSignedRunnerRequestError {
    BadRequest(String),
    Unavailable(String),
    Internal(String),
}
fn moderation_runner_signed_screen_request_json(
    service: &ModerationRunnerService,
    body: &[u8],
) -> Result<Value, ModerationSignedRunnerRequestError> {
    let bad_request = ModerationSignedRunnerRequestError::BadRequest;
    if body.is_empty() {
        return Err(bad_request(
            "signed moderation screen request body must not be empty".to_string(),
        ));
    }
    let value: Value = from_slice(body).map_err(|error| {
        bad_request(format!(
            "failed to parse signed moderation screen request JSON: {error}"
        ))
    })?;
    let fields = value.as_object().ok_or_else(|| {
        bad_request("signed moderation screen request must be a JSON object".to_string())
    })?;
    if let Some(field) = fields
        .keys()
        .find(|field| !matches!(field.as_str(), "subject" | "payload_b64" | "notes"))
    {
        return Err(bad_request(format!(
            "signed moderation screen request contains unsupported field `{field}`"
        )));
    }
    let subject = required_json_string(fields, "subject", "signed moderation screen request")
        .map_err(bad_request)?;
    validate_moderation_request_text(
        subject,
        MODERATION_RUNNER_MAX_SUBJECT_BYTES,
        "signed moderation runner `subject`",
    )
    .map_err(bad_request)?;
    let payload_b64 =
        required_json_string(fields, "payload_b64", "signed moderation screen request")
            .map_err(bad_request)?;
    let notes = match fields.get("notes") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => {
            validate_moderation_request_text(
                value,
                MODERATION_RUNNER_MAX_NOTES_BYTES,
                "signed moderation runner `notes`",
            )
            .map_err(bad_request)?;
            Some(value.clone())
        }
        Some(_) => {
            return Err(bad_request(
                "signed moderation runner optional `notes` must be a string".to_string(),
            ));
        }
    };
    let decoded_capacity = base64::decoded_len_estimate(payload_b64.len());
    let maximum_payload = usize::try_from(service.max_payload_bytes).map_err(|_| {
        ModerationSignedRunnerRequestError::Internal(
            "signed moderation payload bound does not fit usize".to_string(),
        )
    })?;
    let padded_maximum = maximum_payload.checked_add(2).ok_or_else(|| {
        ModerationSignedRunnerRequestError::Internal(
            "signed moderation payload bound overflows usize".to_string(),
        )
    })?;
    if decoded_capacity > padded_maximum {
        return Err(bad_request(format!(
            "signed moderation `payload_b64` can decode beyond the configured {maximum_payload}-byte maximum"
        )));
    }
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(decoded_capacity)
        .map_err(|error| {
            ModerationSignedRunnerRequestError::Unavailable(format!(
                "failed to reserve bounded signed moderation payload: {error}"
            ))
        })?;
    payload.resize(decoded_capacity, 0);
    let decoded = BASE64_STANDARD
        .decode_slice(payload_b64, &mut payload)
        .map_err(|error| {
            bad_request(format!("invalid signed moderation `payload_b64`: {error}"))
        })?;
    payload.truncate(decoded);
    if payload.is_empty() {
        return Err(bad_request(
            "signed moderation `payload_b64` must decode to non-empty bytes".to_string(),
        ));
    }
    let state = service.signed.as_ref().ok_or_else(|| {
        ModerationSignedRunnerRequestError::Internal(
            "signed runner state disappeared after route selection".to_string(),
        )
    })?;
    let _transaction = state
        .transaction_guard
        .try_lock()
        .map_err(|error| match error {
            std::sync::TryLockError::WouldBlock => ModerationSignedRunnerRequestError::Unavailable(
                "signed moderation runner already has an in-flight signing transaction".to_string(),
            ),
            std::sync::TryLockError::Poisoned(_) => ModerationSignedRunnerRequestError::Internal(
                "signed moderation transaction lock is poisoned".to_string(),
            ),
        })?;
    let now_unix =
        moderation_trusted_now_unix().map_err(ModerationSignedRunnerRequestError::Internal)?;
    let result = state
        .signing_runner
        .screen_signed(
            &payload,
            service.max_payload_bytes,
            subject,
            notes,
            now_unix,
        )
        .map_err(|error| match error {
            ModerationRunnerError::EmptyPayload | ModerationRunnerError::PayloadTooLarge { .. } => {
                ModerationSignedRunnerRequestError::BadRequest(error.to_string())
            }
            ModerationRunnerError::InvalidTrustPolicy(_)
            | ModerationRunnerError::InvalidSigningKey(_)
            | ModerationRunnerError::ResultExpiryOverflow => {
                ModerationSignedRunnerRequestError::Unavailable(error.to_string())
            }
            _ => ModerationSignedRunnerRequestError::Internal(error.to_string()),
        })?;
    let provenance_head = state
        .provenance
        .append_signed_result(
            service.manifest(),
            state.signing_runner.trust_policy(),
            &state.trust_anchors,
            state.minimum_governance_quorum,
            result.clone(),
            now_unix,
        )
        .map_err(|error| match error {
            ModerationProvenanceStoreError::Locked(_) => {
                ModerationSignedRunnerRequestError::Unavailable(error.to_string())
            }
            _ => ModerationSignedRunnerRequestError::Internal(error.to_string()),
        })?;
    let canonical = to_bytes(&result).map_err(|error| {
        ModerationSignedRunnerRequestError::Internal(format!(
            "failed to encode signed moderation result: {error}"
        ))
    })?;
    if u64::try_from(canonical.len())
        .ok()
        .is_none_or(|length| length > MODERATION_SIGNED_RESULT_MAX_BYTES)
    {
        return Err(ModerationSignedRunnerRequestError::Internal(
            "signed moderation result exceeds its hard encoded bound".to_string(),
        ));
    }
    moderation_signed_result_summary_json(&result, &canonical, provenance_head)
        .map_err(ModerationSignedRunnerRequestError::Internal)
}
fn moderation_runner_screen_request_json(
    service: &ModerationRunnerService,
    body: &[u8],
) -> Result<Value, String> {
    if body.is_empty() {
        return Err("moderation runner screen request body must not be empty".to_string());
    }
    let value: Value = from_slice(body)
        .map_err(|err| format!("failed to parse moderation runner screen request JSON: {err}"))?;
    let fields = value
        .as_object()
        .ok_or_else(|| "moderation runner screen request must be a JSON object".to_string())?;
    if let Some(field) = fields.keys().find(|field| {
        !matches!(
            field.as_str(),
            "subject" | "payload_b64" | "screened_at_unix" | "notes"
        )
    }) {
        return Err(format!(
            "moderation runner screen request contains unsupported field `{field}`"
        ));
    }
    let subject = fields
        .get("subject")
        .and_then(Value::as_str)
        .ok_or_else(|| "moderation runner screen request requires string `subject`".to_string())?;
    validate_moderation_request_text(
        subject,
        MODERATION_RUNNER_MAX_SUBJECT_BYTES,
        "moderation runner `subject`",
    )?;
    let payload_b64 =
        required_json_string(fields, "payload_b64", "moderation runner screen request")?;
    let screened_at_unix = fields
        .get("screened_at_unix")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "moderation runner screen request requires numeric `screened_at_unix`".to_string()
        })?;
    if screened_at_unix == 0 {
        return Err("moderation runner `screened_at_unix` must be greater than zero".to_string());
    }
    let notes = match fields.get("notes") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => {
            validate_moderation_request_text(
                value,
                MODERATION_RUNNER_MAX_NOTES_BYTES,
                "moderation runner `notes`",
            )?;
            Some(value.as_str())
        }
        Some(_) => return Err("moderation runner optional `notes` must be a string".to_string()),
    };
    let decoded_capacity = base64::decoded_len_estimate(payload_b64.len());
    let maximum_payload = usize::try_from(service.max_payload_bytes)
        .map_err(|_| "moderation runner payload bound does not fit usize".to_owned())?;
    let padded_maximum = maximum_payload
        .checked_add(2)
        .ok_or_else(|| "moderation runner payload bound overflows usize".to_owned())?;
    if decoded_capacity > padded_maximum {
        return Err(format!(
            "moderation runner `payload_b64` can decode beyond the configured {maximum_payload}-byte maximum"
        ));
    }
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(decoded_capacity)
        .map_err(|error| format!("failed to reserve bounded moderation payload: {error}"))?;
    payload.resize(decoded_capacity, 0);
    let decoded = BASE64_STANDARD
        .decode_slice(payload_b64, &mut payload)
        .map_err(|err| format!("invalid moderation runner `payload_b64`: {err}"))?;
    payload.truncate(decoded);
    if payload.is_empty() {
        return Err("moderation runner `payload_b64` must decode to non-empty bytes".to_string());
    }
    moderation_local_runner_screening_json(
        &service.runner,
        &payload,
        subject,
        screened_at_unix,
        notes,
        service.max_payload_bytes,
    )
}
fn moderation_runner_screen_request_proto(
    service: &ModerationRunnerService,
    request: ModerationRunnerScreenRequest,
) -> Result<ModerationRunnerScreenResponse, String> {
    let subject = request.subject.as_str();
    validate_moderation_request_text(
        subject,
        MODERATION_RUNNER_MAX_SUBJECT_BYTES,
        "moderation runner gRPC `subject`",
    )?;
    if request.payload.is_empty() {
        return Err(
            "moderation runner gRPC screen request `payload` must not be empty".to_string(),
        );
    }
    if request.payload.len() > service.max_body_bytes {
        return Err(
            "moderation runner gRPC screen request payload exceeds configured maximum".to_string(),
        );
    }
    if request.screened_at_unix == 0 {
        return Err(
            "moderation runner gRPC `screened_at_unix` must be greater than zero".to_string(),
        );
    }
    let notes = match request.notes.as_deref() {
        None => None,
        Some(value) => {
            validate_moderation_request_text(
                value,
                MODERATION_RUNNER_MAX_NOTES_BYTES,
                "moderation runner gRPC `notes`",
            )?;
            Some(value)
        }
    };
    let value = moderation_local_runner_screening_json(
        &service.runner,
        &request.payload,
        subject,
        request.screened_at_unix,
        notes,
        service.max_payload_bytes,
    )?;
    moderation_runner_screen_proto_from_json(&value)
}
fn moderation_runner_status_json(
    service: &ModerationRunnerService,
    status: &str,
    listen: Option<&str>,
) -> Value {
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.runner.status.v1");
    insert_value!(output["status"] = status.to_string());
    insert_value!(output["source"] = "sorafs_cli");
    insert_value!(output["manifest_source"] = service.manifest_source.clone());
    insert_value!(output["manifest_id_hex"] = hex_encode(service.manifest().body.manifest_id));
    insert_value!(
        output["manifest_digest_hex"] = hex_encode(service.manifest().body.manifest_digest)
    );
    insert_value!(output["runner_hash_hex"] = hex_encode(service.manifest().body.runner_hash));
    insert_value!(output["runtime_version"] = service.manifest().body.runtime_version.clone());
    insert_value!(
        output["model_count"] = u64::try_from(service.manifest().body.models.len())
            .expect("validated runner model count fits u64")
    );
    insert_value!(output["max_body_bytes"] = service.max_body_bytes as u64);
    insert_value!(output["max_payload_bytes"] = u64::from(service.max_payload_bytes));
    insert_value!(
        output["max_active_connections"] = u64::try_from(MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS)
            .expect("runner connection limit fits u64")
    );
    insert_value!(
        output["max_grpc_in_flight"] = u64::try_from(MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT)
            .expect("runner gRPC in-flight limit fits u64")
    );
    insert_value!(
        output["max_grpc_response_bytes"] =
            u64::try_from(MODERATION_RUNNER_MAX_GRPC_RESPONSE_BYTES)
                .expect("runner gRPC response limit fits u64")
    );
    insert_value!(output["outbound_network"] = "model_engine_none_process_policy_required");
    insert_value!(output["process_isolation"] = "external_runtime_attestation_required");
    insert_value!(output["process_isolation_verified"] = false);
    if let Some(state) = &service.signed {
        let policy = state.signing_runner.trust_policy();
        let snapshot = state.provenance.snapshot().ok();
        insert_value!(output["trust_boundary"] = "externally_anchored_signed_results");
        insert_value!(output["signed_results"] = true);
        insert_value!(output["unsigned_screening_enabled"] = false);
        insert_value!(
            output["signed_screening_endpoint"] = "/v1/sorafs/moderation/runner/screen-signed"
        );
        insert_value!(output["trust_policy_id_hex"] = hex_encode(policy.body.policy_id));
        insert_value!(output["trust_policy_digest_hex"] = hex_encode(policy.body.policy_digest));
        insert_value!(
            output["result_signer_public_key"] =
                state.signing_runner.signer_public_key().to_string()
        );
        insert_value!(
            output["minimum_governance_quorum"] = u64::from(state.minimum_governance_quorum)
        );
        insert_value!(
            output["trusted_governance_anchor_count"] = u64::try_from(state.trust_anchors.len())
                .expect("bounded governance anchor count fits u64")
        );
        insert_value!(output["provenance_path"] = state.provenance.path().display().to_string());
        insert_value!(output["provenance_verified"] = snapshot.is_some());
        insert_json!(
            output["provenance_entry_count"] = snapshot
                .as_ref()
                .map(|value| {
                    Value::from(
                        u64::try_from(value.entries.len())
                            .expect("bounded provenance entry count fits u64"),
                    )
                })
                .unwrap_or(Value::Null)
        );
        insert_json!(
            output["provenance_head_digest_hex"] = snapshot
                .map(|value| Value::from(hex_encode(value.head_digest)))
                .unwrap_or(Value::Null)
        );
        insert_value!(output["max_signed_in_flight"] = 1_u64);
    } else {
        insert_value!(output["trust_boundary"] = "unsigned_diagnostic_only");
        insert_value!(output["signed_results"] = false);
        insert_value!(output["unsigned_screening_enabled"] = true);
        insert_value!(output["provenance_verified"] = false);
    }
    insert_json!(
        output["listen"] = listen
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    Value::Object(output)
}
fn moderation_runner_status_proto(
    service: &ModerationRunnerService,
    status: &str,
    listen: Option<&str>,
) -> ModerationRunnerStatusResponse {
    ModerationRunnerStatusResponse {
        schema: "sorafs.moderation.runner.status.v1".to_string(),
        status: status.to_string(),
        source: "sorafs_cli".to_string(),
        manifest_source: service.manifest_source.clone(),
        manifest_id_hex: hex_encode(service.manifest().body.manifest_id),
        manifest_digest_hex: hex_encode(service.manifest().body.manifest_digest),
        runner_hash_hex: hex_encode(service.manifest().body.runner_hash),
        runtime_version: service.manifest().body.runtime_version.clone(),
        model_count: u64::try_from(service.manifest().body.models.len())
            .expect("validated runner model count fits u64"),
        max_body_bytes: service.max_body_bytes as u64,
        outbound_network: "model_engine_none_process_policy_required".to_string(),
        listen: listen.unwrap_or_default().to_string(),
        max_payload_bytes: u64::from(service.max_payload_bytes),
        max_active_connections: u64::try_from(MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS)
            .expect("runner connection limit fits u64"),
        process_isolation: "external_runtime_attestation_required".to_string(),
        process_isolation_verified: false,
        max_grpc_in_flight: u64::try_from(MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT)
            .expect("runner gRPC in-flight limit fits u64"),
        max_grpc_response_bytes: u64::try_from(MODERATION_RUNNER_MAX_GRPC_RESPONSE_BYTES)
            .expect("runner gRPC response limit fits u64"),
    }
}
fn moderation_runner_screen_proto_from_json(
    value: &Value,
) -> Result<ModerationRunnerScreenResponse, String> {
    let fields = value
        .as_object()
        .ok_or_else(|| "runner screening result was not a JSON object".to_string())?;
    let combined_score_bps = fields
        .get("combined_score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| "runner screening result is missing `combined_score_bps`".to_string())?;
    let notes = match fields.get("notes") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            return Err("runner screening result `notes` field is not a string".to_string());
        }
    };
    let model_scores = fields
        .get("model_scores")
        .and_then(Value::as_array)
        .ok_or_else(|| "runner screening result is missing `model_scores`".to_string())?
        .iter()
        .map(|value| {
            let score = value.as_object().ok_or_else(|| {
                "runner screening result model score must be an object".to_string()
            })?;
            let score_bps = score
                .get("score_bps")
                .and_then(Value::as_u64)
                .ok_or_else(|| "runner model score is missing `score_bps`".to_string())?;
            Ok(ModerationRunnerModelScore {
                model_id_hex: required_json_string(score, "model_id_hex", "runner model score")?
                    .to_string(),
                artifact_digest_hex: required_json_string(
                    score,
                    "artifact_digest_hex",
                    "runner model score",
                )?
                .to_string(),
                score_bps: u32::try_from(score_bps)
                    .map_err(|_| "runner model score does not fit into u32".to_string())?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ModerationRunnerScreenResponse {
        subject: required_json_string(fields, "subject", "runner screening result")?.to_string(),
        subject_digest_hex: required_json_string(
            fields,
            "subject_digest_hex",
            "runner screening result",
        )?
        .to_string(),
        manifest_id_hex: required_json_string(
            fields,
            "manifest_id_hex",
            "runner screening result",
        )?
        .to_string(),
        runner_hash_hex: required_json_string(
            fields,
            "runner_hash_hex",
            "runner screening result",
        )?
        .to_string(),
        combined_score_bps: u32::try_from(combined_score_bps)
            .map_err(|_| "runner screening result score does not fit into u32".to_string())?,
        verdict: required_json_string(fields, "verdict", "runner screening result")?.to_string(),
        screened_at_unix: fields
            .get("screened_at_unix")
            .and_then(Value::as_u64)
            .ok_or_else(|| "runner screening result is missing `screened_at_unix`".to_string())?,
        evidence_digest_hex: required_json_string(
            fields,
            "evidence_digest_hex",
            "runner screening result",
        )?
        .to_string(),
        policy_digest_hex: required_json_string(
            fields,
            "policy_digest_hex",
            "runner screening result",
        )?
        .to_string(),
        notes,
        model_scores,
    })
}
fn moderation_runner_json_response(status: u16, reason: &str, value: &Value) -> Vec<u8> {
    let body = to_vec(value).unwrap_or_else(|_| b"{\"error\":\"json_render_failed\"}".to_vec());
    moderation_runner_http_response_bytes(status, reason, "application/json", &body)
}
fn moderation_runner_error_response(status: u16, reason: &str, message: &str) -> Vec<u8> {
    let mut body = Map::new();
    insert_value!(body["schema"] = "sorafs.moderation.runner.error.v1");
    insert_value!(body["status"] = "error");
    insert_value!(body["message"] = message.to_string());
    moderation_runner_json_response(status, reason, &Value::Object(body))
}
fn moderation_runner_http_response_bytes(
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = Vec::with_capacity(header.len() + body.len());
    response.extend_from_slice(header.as_bytes());
    response.extend_from_slice(body);
    response
}
fn required_json_string<'a>(fields: &'a Map, key: &str, context: &str) -> Result<&'a str, String> {
    let value = fields
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} requires string `{key}`"))?
        .trim();
    if value.is_empty() {
        return Err(format!("{context} string `{key}` must not be empty"));
    }
    Ok(value)
}
fn optional_json_string<'a>(
    fields: &'a Map,
    key: &str,
    context: &str,
) -> Result<Option<&'a str>, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(format!("{context} string `{key}` must not be empty"));
            }
            Ok(Some(trimmed))
        }
        Some(_) => Err(format!("{context} optional `{key}` must be a string")),
    }
}
fn optional_json_u64(fields: &Map, key: &str, context: &str) -> Result<Option<u64>, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| format!("{context} optional `{key}` must be numeric"))
            .map(Some),
    }
}
fn optional_json_fixed_hex<const N: usize>(
    fields: &Map,
    key: &str,
    context: &str,
) -> Result<Option<[u8; N]>, String> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => parse_fixed_hex::<N>(value, key, context).map(Some),
        Some(_) => Err(format!("{context} optional `{key}` must be a string")),
    }
}
fn parse_fixed_hex<const N: usize>(
    raw: &str,
    label: &str,
    context: &str,
) -> Result<[u8; N], String> {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    let bytes = parse_hex_vec(stripped)
        .map_err(|err| format!("{context} has invalid `{label}` hex: {err}"))?;
    if bytes.len() != N {
        return Err(format!(
            "{context} `{label}` must decode to {N} bytes, found {} bytes",
            bytes.len()
        ));
    }
    let mut out = [0_u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}
fn parse_moderation_max_payload_bytes(raw: &str, context: &str) -> Result<u32, String> {
    let parsed = parse_u32_arg("--max-payload-bytes", raw, context)?;
    if parsed == 0 || parsed > MODERATION_MODEL_MAX_INPUT_BYTES_V1 {
        return Err(format!(
            "`--max-payload-bytes` must be in 1..={MODERATION_MODEL_MAX_INPUT_BYTES_V1}"
        ));
    }
    Ok(parsed)
}
fn parse_moderation_max_body_bytes(raw: &str, context: &str) -> Result<usize, String> {
    let parsed = parse_u64_arg("--max-body-bytes", raw, context)?;
    let parsed = usize::try_from(parsed)
        .map_err(|_| "`--max-body-bytes` does not fit into this platform's usize".to_owned())?;
    if parsed == 0 || parsed > MODERATION_RUNNER_HARD_MAX_BODY_BYTES {
        return Err(format!(
            "`--max-body-bytes` must be in 1..={MODERATION_RUNNER_HARD_MAX_BODY_BYTES}"
        ));
    }
    Ok(parsed)
}
fn validate_moderation_loopback_listen(value: &str, context: &str) -> Result<SocketAddr, String> {
    let address = value.parse::<SocketAddr>().map_err(|error| {
        format!("`--listen={value}` is not a socket address for {context}: {error}")
    })?;
    if !address.ip().is_loopback() {
        return Err(format!(
            "`--listen={value}` must use a loopback IP; expose the unauthenticated runner only through an authenticated local proxy"
        ));
    }
    Ok(address)
}
fn validate_moderation_request_text(
    value: &str,
    maximum: usize,
    field: &str,
) -> Result<(), String> {
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{field} must be non-empty canonical text without padding/control characters and at most {maximum} bytes"
        ));
    }
    Ok(())
}
fn read_file_bounded(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!(
            "{label} `{}` must be a regular non-symlink file",
            path.display()
        ));
    }
    if before.len() > maximum {
        return Err(format!(
            "{label} `{}` has {} bytes; maximum is {maximum}",
            path.display(),
            before.len()
        ));
    }
    let identity = moderation_file_identity(&before);
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .map_err(|error| format!("failed to open {label} `{}`: {error}", path.display()))?;
    let opened = file.metadata().map_err(|error| {
        format!(
            "failed to inspect opened {label} `{}`: {error}",
            path.display()
        )
    })?;
    if !opened.is_file() || moderation_file_identity(&opened) != identity {
        return Err(format!(
            "{label} `{}` changed identity while opening",
            path.display()
        ));
    }
    let capacity = usize::try_from(before.len())
        .map_err(|_| format!("{label} size does not fit into this platform's usize"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|error| format!("failed to reserve bounded {label} buffer: {error}"))?;
    let read_limit = maximum
        .checked_add(1)
        .ok_or_else(|| format!("{label} read limit overflows u64"))?;
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {label} `{}`: {error}", path.display()))?;
    let observed_len =
        u64::try_from(bytes.len()).map_err(|_| format!("{label} length does not fit into u64"))?;
    if observed_len > maximum {
        return Err(format!(
            "{label} `{}` grew beyond the {maximum}-byte limit while reading",
            path.display()
        ));
    }
    let after = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to re-inspect {label} `{}`: {error}", path.display()))?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || moderation_file_identity(&after) != identity
        || observed_len != before.len()
    {
        return Err(format!(
            "{label} `{}` changed while it was being read",
            path.display()
        ));
    }
    Ok(bytes)
}
fn moderation_runner_current_executable_hash() -> Result<[u8; 32], String> {
    let executable = env::current_exe()
        .map_err(|error| format!("failed to locate current moderation runner binary: {error}"))?;
    let canonical = fs::canonicalize(&executable).map_err(|error| {
        format!(
            "failed to canonicalize moderation runner binary `{}`: {error}",
            executable.display()
        )
    })?;
    let before = fs::symlink_metadata(&canonical).map_err(|error| {
        format!(
            "failed to inspect moderation runner binary `{}`: {error}",
            canonical.display()
        )
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!(
            "moderation runner binary `{}` must be a regular non-symlink file",
            canonical.display()
        ));
    }
    let identity = moderation_file_identity(&before);
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(&canonical).map_err(|error| {
        format!(
            "failed to open moderation runner binary `{}`: {error}",
            canonical.display()
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        format!(
            "failed to inspect opened moderation runner binary `{}`: {error}",
            canonical.display()
        )
    })?;
    if !opened.is_file() || moderation_file_identity(&opened) != identity {
        return Err("moderation runner binary changed identity while opening".to_string());
    }
    let mut hasher = blake3::Hasher::new();
    let mut chunk = [0_u8; 64 * 1024];
    let mut total_bytes = 0_u64;
    loop {
        let read = file.read(&mut chunk).map_err(|error| {
            format!(
                "failed to hash moderation runner binary `{}`: {error}",
                canonical.display()
            )
        })?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes
            .checked_add(
                u64::try_from(read)
                    .map_err(|_| "moderation runner read length does not fit u64".to_string())?,
            )
            .ok_or_else(|| "moderation runner binary length overflow".to_string())?;
        hasher.update(&chunk[..read]);
    }
    let after = fs::symlink_metadata(&canonical).map_err(|error| {
        format!(
            "failed to re-inspect moderation runner binary `{}`: {error}",
            canonical.display()
        )
    })?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || moderation_file_identity(&after) != identity
        || total_bytes != before.len()
    {
        return Err("moderation runner binary changed while hashing".to_string());
    }
    Ok(*hasher.finalize().as_bytes())
}
#[cfg(unix)]
fn moderation_file_identity(metadata: &FsMetadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}
#[cfg(not(unix))]
fn moderation_file_identity(metadata: &FsMetadata) -> (u64, Option<SystemTime>) {
    (metadata.len(), metadata.modified().ok())
}
fn load_moderation_runner_for_current_executable(
    manifest: ModerationReproManifestV1,
    artifact_root: &Path,
) -> Result<LoadedModerationRunnerV1, String> {
    let observed_runner_hash = moderation_runner_current_executable_hash()?;
    LoadedModerationRunnerV1::load_verified(manifest, artifact_root, observed_runner_hash)
        .map_err(|error| format!("failed to load verified moderation runner: {error}"))
}
fn load_moderation_repro_manifest(
    manifest_path: &Path,
    format: &str,
    context: &str,
) -> Result<ModerationReproManifestV1, String> {
    let bytes = read_file_bounded(
        manifest_path,
        MODERATION_RUNNER_MAX_MANIFEST_BYTES,
        "moderation manifest",
    )?;
    match format {
        "json" => norito::json::from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse JSON reproducibility manifest `{}` for `{context}`: {err}",
                manifest_path.display()
            )
        }),
        "norito" => {
            let byte_limit = usize::try_from(MODERATION_RUNNER_MAX_MANIFEST_BYTES)
                .map_err(|_| "moderation manifest limit does not fit usize".to_string())?;
            norito::decode_from_bytes_with_limits(
                &bytes,
                norito::DecodeLimits::new(1024, byte_limit, 4096, byte_limit, 32),
            )
            .map_err(|err| {
                format!(
                    "failed to decode Norito reproducibility manifest `{}` for `{context}`: {err}",
                    manifest_path.display()
                )
            })
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{context}` (expected `json` or `norito`)"
        )),
    }
}
fn load_moderation_trust_policy(
    policy_path: &Path,
    format: &str,
    context: &str,
) -> Result<ModerationTrustPolicyV1, String> {
    let bytes = read_file_bounded(
        policy_path,
        MODERATION_TRUST_POLICY_MAX_BYTES,
        "moderation trust policy",
    )?;
    match format {
        "json" => norito::json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse JSON moderation trust policy `{}` for `{context}`: {error}",
                policy_path.display()
            )
        }),
        "norito" => {
            let byte_limit = usize::try_from(MODERATION_TRUST_POLICY_MAX_BYTES)
                .map_err(|_| "moderation trust-policy limit does not fit usize".to_string())?;
            let policy: ModerationTrustPolicyV1 = norito::decode_from_bytes_with_limits(
                &bytes,
                norito::DecodeLimits::new(4096, byte_limit, 4096, byte_limit, 32),
            )
            .map_err(|error| {
                format!(
                    "failed to decode Norito moderation trust policy `{}` for `{context}`: {error}",
                    policy_path.display()
                )
            })?;
            let canonical = to_bytes(&policy).map_err(|error| {
                format!("failed to re-encode moderation trust policy canonically: {error}")
            })?;
            if canonical != bytes {
                return Err(format!(
                    "Norito moderation trust policy `{}` is not canonically encoded",
                    policy_path.display()
                ));
            }
            Ok(policy)
        }
        other => Err(format!(
            "unsupported `--trust-policy-format={other}` for `{context}` (expected `json` or `norito`)"
        )),
    }
}
fn parse_moderation_trust_anchor(value: &str, context: &str) -> Result<PublicKey, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "`--trust-anchor` must not be empty for `{context}`"
        ));
    }
    PublicKey::from_str(trimmed)
        .map_err(|error| format!("invalid `--trust-anchor` for `{context}`: {error}"))
}
fn load_moderation_signing_key(path: &Path, context: &str) -> Result<PrivateKey, String> {
    #[cfg(not(unix))]
    {
        let _ = (path, context);
        return Err(
            "moderation result signing fails closed on non-Unix platforms until equivalent private-key file controls are implemented"
                .to_string(),
        );
    }
    #[cfg(unix)]
    let canonical = {
        let configured = fs::symlink_metadata(path).map_err(|error| {
            format!(
                "failed to inspect moderation result-signing key `{}`: {error}",
                path.display()
            )
        })?;
        if configured.file_type().is_symlink() || !configured.is_file() || configured.nlink() != 1 {
            return Err(format!(
                "moderation result-signing key `{}` must be a regular non-symlink with exactly one hard link",
                path.display()
            ));
        }
        if configured.mode() & 0o077 != 0 {
            return Err(format!(
                "moderation result-signing key `{}` must not grant group or world permissions",
                path.display()
            ));
        }
        fs::canonicalize(path).map_err(|error| {
            format!(
                "failed to canonicalize moderation result-signing key `{}`: {error}",
                path.display()
            )
        })?
    };
    #[cfg(not(unix))]
    let canonical = path.to_path_buf();
    let bytes = read_file_bounded(
        &canonical,
        MODERATION_SIGNING_KEY_MAX_BYTES,
        "moderation result-signing key",
    )?;
    #[cfg(unix)]
    if fs::canonicalize(path).ok().as_ref() != Some(&canonical) {
        return Err(format!(
            "moderation result-signing key `{}` changed while loading",
            path.display()
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        format!(
            "moderation result-signing key `{}` for `{context}` is not UTF-8: {error}",
            path.display()
        )
    })?;
    parse_private_key_inline(text)
}
fn load_moderation_signed_result(
    path: &Path,
    context: &str,
) -> Result<ModerationSignedScreeningResultV1, String> {
    let bytes = read_file_bounded(
        path,
        MODERATION_SIGNED_RESULT_MAX_BYTES,
        "signed moderation result",
    )?;
    decode_moderation_signed_result(&bytes, &format!("`{}` for `{context}`", path.display()))
}
fn decode_moderation_signed_result(
    bytes: &[u8],
    context: &str,
) -> Result<ModerationSignedScreeningResultV1, String> {
    if bytes.is_empty()
        || u64::try_from(bytes.len())
            .ok()
            .is_none_or(|length| length > MODERATION_SIGNED_RESULT_MAX_BYTES)
    {
        return Err(format!(
            "signed moderation result {context} is outside the 1..={MODERATION_SIGNED_RESULT_MAX_BYTES}-byte bound"
        ));
    }
    let byte_limit = usize::try_from(MODERATION_SIGNED_RESULT_MAX_BYTES)
        .map_err(|_| "signed moderation result limit does not fit usize".to_string())?;
    let result: ModerationSignedScreeningResultV1 = norito::decode_from_bytes_with_limits(
        bytes,
        norito::DecodeLimits::new(4096, byte_limit, 4096, byte_limit, 32),
    )
    .map_err(|error| format!("failed to decode signed moderation result {context}: {error}"))?;
    let canonical = to_bytes(&result)
        .map_err(|error| format!("failed to re-encode signed moderation result: {error}"))?;
    if canonical != bytes {
        return Err(format!(
            "signed moderation result {context} is not canonically encoded"
        ));
    }
    Ok(result)
}
fn moderation_signed_result_summary_json(
    result: &ModerationSignedScreeningResultV1,
    canonical_bytes: &[u8],
    provenance_head: [u8; 32],
) -> Result<Value, String> {
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.signed_runner_output.v1");
    insert_json!(
        output["signed_result"] = to_value(result)
            .map_err(|error| format!("failed to render signed result JSON: {error}"))?
    );
    insert_value!(output["signed_result_norito_b64"] = BASE64_STANDARD.encode(canonical_bytes));
    insert_value!(output["manifest_id_hex"] = hex_encode(result.body.manifest_id));
    insert_value!(output["trust_policy_id_hex"] = hex_encode(result.body.trust_policy_id));
    insert_value!(output["trust_policy_digest_hex"] = hex_encode(result.body.trust_policy_digest));
    insert_value!(output["signer_public_key"] = result.signer_public_key.to_string());
    insert_value!(output["evidence_digest_hex"] = hex_encode(result.body.evidence_digest));
    insert_value!(output["provenance_head_digest_hex"] = hex_encode(provenance_head));
    insert_value!(output["payload_bytes_included"] = false);
    Ok(Value::Object(output))
}
fn moderation_authenticated_aggregate_summary_json(
    aggregate: &ModerationCommitteeAggregateV1,
    canonical_bytes: &[u8],
    provenance_head: [u8; 32],
) -> Result<Value, String> {
    let mut output = Map::new();
    insert_value!(output["schema"] = "sorafs.moderation.authenticated_committee_output.v1");
    insert_json!(
        output["aggregate"] = to_value(aggregate)
            .map_err(|error| format!("failed to render committee aggregate JSON: {error}"))?
    );
    insert_value!(output["aggregate_norito_b64"] = BASE64_STANDARD.encode(canonical_bytes));
    insert_value!(output["aggregate_digest_hex"] = hex_encode(aggregate.aggregate_digest));
    insert_value!(output["trust_policy_digest_hex"] = hex_encode(aggregate.trust_policy_digest));
    insert_value!(
        output["distinct_signer_count"] = u64::try_from(aggregate.members.len())
            .expect("bounded committee member count fits u64")
    );
    insert_value!(output["provenance_head_digest_hex"] = hex_encode(provenance_head));
    insert_value!(output["payload_bytes_included"] = false);
    Ok(Value::Object(output))
}
fn validate_moderation_local_runner_manifest(
    manifest: &ModerationReproManifestV1,
) -> Result<(), String> {
    let thresholds = manifest.body.thresholds;
    if thresholds.quarantine > 10_000 {
        return Err(format!(
            "manifest quarantine threshold {} exceeds 10000 bps",
            thresholds.quarantine
        ));
    }
    if thresholds.escalate > 10_000 {
        return Err(format!(
            "manifest escalate threshold {} exceeds 10000 bps",
            thresholds.escalate
        ));
    }
    if thresholds.quarantine > thresholds.escalate {
        return Err(format!(
            "manifest quarantine threshold {} exceeds escalate threshold {}",
            thresholds.quarantine, thresholds.escalate
        ));
    }
    let mut has_positive_weight = false;
    for model in &manifest.body.models {
        let weight = model.weight.unwrap_or(10_000);
        if weight > 10_000 {
            return Err(format!(
                "model {} has weight {} above 10000 bps",
                hex_encode(model.model_id),
                weight
            ));
        }
        has_positive_weight |= weight > 0;
    }
    if !has_positive_weight {
        return Err("manifest must include at least one positive model weight".to_string());
    }
    Ok(())
}
fn moderation_local_runner_screening_json(
    runner: &LoadedModerationRunnerV1,
    payload: &[u8],
    subject: &str,
    screened_at_unix: u64,
    notes: Option<&str>,
    max_payload_bytes: u32,
) -> Result<Value, String> {
    if screened_at_unix == 0 {
        return Err("moderation runner `screened_at_unix` must be greater than zero".to_string());
    }
    let manifest = runner.manifest();
    let subject_digest = *blake3_hash(payload).as_bytes();
    let ModerationInferenceV1 {
        combined_score_bps: score,
        model_scores,
    } = runner
        .infer(payload, max_payload_bytes)
        .map_err(|error| format!("moderation inference failed: {error}"))?;
    let verdict = moderation_score_verdict(score, manifest.body.thresholds);
    let policy_digest = moderation_local_runner_policy_digest(manifest)?;
    let evidence_digest =
        moderation_local_runner_evidence_digest(ModerationLocalRunnerEvidenceInput {
            manifest,
            subject,
            subject_digest: &subject_digest,
            score,
            verdict,
            screened_at_unix,
            policy_digest: &policy_digest,
            model_scores: &model_scores,
        });
    let mut output = Map::new();
    insert_value!(output["subject"] = subject.to_string());
    insert_value!(output["subject_digest_hex"] = hex_encode(subject_digest));
    insert_value!(output["manifest_id_hex"] = hex_encode(manifest.body.manifest_id));
    insert_value!(output["runner_hash_hex"] = hex_encode(manifest.body.runner_hash));
    insert_value!(output["combined_score_bps"] = u64::from(score));
    insert_json!(
        output["model_scores"] = Value::Array(
            model_scores
                .iter()
                .map(moderation_model_score_json)
                .collect(),
        )
    );
    insert_value!(output["verdict"] = verdict.to_string());
    insert_value!(output["screened_at_unix"] = screened_at_unix);
    insert_value!(output["evidence_digest_hex"] = hex_encode(evidence_digest));
    insert_value!(output["policy_digest_hex"] = hex_encode(policy_digest));
    insert_json!(
        output["notes"] = notes
            .map(|value| Value::from(value.to_string()))
            .unwrap_or(Value::Null)
    );
    Ok(Value::Object(output))
}
fn moderation_model_score_json(score: &ModerationModelScoreV1) -> Value {
    let mut output = Map::new();
    insert_value!(output["model_id_hex"] = hex_encode(score.model_id));
    insert_value!(output["artifact_digest_hex"] = hex_encode(score.artifact_digest));
    insert_value!(output["score_bps"] = u64::from(score.score_bps));
    Value::Object(output)
}
fn moderation_score_verdict(score: u16, thresholds: ModerationThresholdsV1) -> &'static str {
    if score >= thresholds.escalate {
        "escalate"
    } else if score >= thresholds.quarantine {
        "quarantine"
    } else {
        "pass"
    }
}
fn moderation_local_runner_policy_digest(
    manifest: &ModerationReproManifestV1,
) -> Result<[u8; 32], String> {
    let body_bytes = to_bytes(&manifest.body)
        .map_err(|err| format!("failed to encode reproducibility manifest body: {err}"))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_LOCAL_RUNNER_POLICY_DOMAIN_V1);
    hasher.update(&body_bytes);
    Ok(*hasher.finalize().as_bytes())
}
struct ModerationLocalRunnerEvidenceInput<'a> {
    manifest: &'a ModerationReproManifestV1,
    subject: &'a str,
    subject_digest: &'a [u8; 32],
    score: u16,
    verdict: &'a str,
    screened_at_unix: u64,
    policy_digest: &'a [u8; 32],
    model_scores: &'a [ModerationModelScoreV1],
}
fn moderation_local_runner_evidence_digest(
    input: ModerationLocalRunnerEvidenceInput<'_>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MODERATION_LOCAL_RUNNER_EVIDENCE_DOMAIN_V1);
    hasher.update(&input.manifest.body.manifest_id);
    hasher.update(&input.manifest.body.runner_hash);
    update_hash_string(&mut hasher, input.subject);
    hasher.update(input.subject_digest);
    hasher.update(&input.score.to_le_bytes());
    update_hash_string(&mut hasher, input.verdict);
    hasher.update(&input.screened_at_unix.to_le_bytes());
    hasher.update(input.policy_digest);
    hasher.update(
        &u64::try_from(input.model_scores.len())
            .expect("validated runner model count fits u64")
            .to_le_bytes(),
    );
    for model_score in input.model_scores {
        hasher.update(&model_score.model_id);
        hasher.update(&model_score.artifact_digest);
        hasher.update(&model_score.score_bps.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}
fn update_hash_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}
fn moderation_honey_audit(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        return Err(moderation_usage());
    }
    let mut manifest_id_hex: Option<String> = None;
    let mut chunker_handle = DEFAULT_CHUNKER_HANDLE.to_string();
    let mut expected_catalog_digest_hex: Option<String> = None;
    let mut honey_digests: Vec<String> = Vec::new();
    let mut provider_specs: Vec<GatewayProviderSpec> = Vec::new();
    let mut json_out: Option<PathBuf> = None;
    let mut markdown_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(moderation_usage());
        }
        if let Some(rest) = arg.strip_prefix("--manifest-id=") {
            manifest_id_hex = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--chunker-handle=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--chunker-handle` must not be empty".into());
            }
            chunker_handle = trimmed.to_string();
        } else if let Some(rest) = arg.strip_prefix("--expected-catalog-digest=") {
            let trimmed = rest.trim();
            if trimmed != rest
                || trimmed.len() != 64
                || !trimmed
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err("`--expected-catalog-digest` must be lowercase 32-byte hex".into());
            }
            expected_catalog_digest_hex = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--honey=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--honey` digests must not be empty".into());
            }
            honey_digests.push(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--markdown-out=") {
            markdown_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--provider") {
            if let Some(spec) = rest.strip_prefix('=') {
                provider_specs.push(parse_gateway_provider_spec(spec)?);
            } else {
                return Err("expected `--provider name=ALIAS,provider-id=HEX,gateway-key=HEX,base-url=URL,stream-token=BASE64`".to_string());
            }
        } else {
            return Err(moderation_usage());
        }
    }
    let manifest_id_hex = manifest_id_hex.ok_or_else(|| {
        "missing required `--manifest-id` for `sorafs_cli moderation honey-audit`".to_string()
    })?;
    if manifest_id_hex.len() != 64 || !manifest_id_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("`--manifest-id` must be a 32-byte hex string".to_string());
    }
    if honey_digests.is_empty() {
        return Err("provide at least one `--honey=HEX` digest to probe".to_string());
    }
    if provider_specs.is_empty() {
        return Err("provide at least one `--provider` entry".to_string());
    }
    let mut specs = Vec::with_capacity(honey_digests.len());
    for (idx, digest_hex) in honey_digests.iter().enumerate() {
        let digest = hex::decode(digest_hex.trim())
            .map_err(|err| format!("invalid `--honey` digest `{digest_hex}`: {err}"))?;
        if digest.len() != 32 {
            return Err(format!(
                "`--honey` digest `{digest_hex}` must decode to 32 bytes"
            ));
        }
        let mut digest_bytes = [0u8; 32];
        digest_bytes.copy_from_slice(&digest);
        specs.push(ChunkFetchSpec {
            chunk_index: idx,
            offset: 0,
            length: 0,
            digest: digest_bytes,
            taikai_segment_hint: None,
        });
    }
    let gateway_config = GatewayFetchConfig {
        manifest_id_hex: manifest_id_hex.clone(),
        chunker_handle: chunker_handle.clone(),
        manifest_envelope_b64: None,
        client_id: None,
        expected_manifest_cid_hex: None,
        blinded_cid_b64: None,
        salt_epoch: None,
        expected_cache_version: None,
    };
    let provider_inputs: Vec<GatewayProviderInput> = provider_specs
        .iter()
        .map(|spec| GatewayProviderInput {
            name: spec.name.clone(),
            provider_id_hex: spec.provider_id_hex.clone(),
            gateway_public_key_hex: spec.gateway_public_key_hex.clone(),
            base_url: spec.base_url.clone(),
            stream_token_b64: spec.stream_token_b64.clone(),
            privacy_events_url: spec.privacy_events_url.clone(),
        })
        .collect();
    let context = GatewayFetchContext::new(gateway_config, provider_inputs)
        .map_err(|err| format!("failed to construct gateway context: {err}"))?;
    let providers = context.providers();
    if providers.is_empty() {
        return Err("gateway context did not expose any providers".to_string());
    }
    let validator = expected_catalog_digest_hex
        .as_deref()
        .map_or_else(PolicyEvidenceValidator::new, |digest| {
            PolicyEvidenceValidator::new().with_expected_catalog_digest(digest)
        });
    let runtime =
        Runtime::new().map_err(|err| format!("failed to initialise Tokio runtime: {err}"))?;
    let mut digest_reports = Vec::new();
    for spec in &specs {
        let reports = runtime
            .block_on(run_honey_probe(
                &context.fetcher(),
                &providers,
                spec,
                &validator,
            ))
            .map_err(|err| {
                format!(
                    "honey probe failed for digest {}: {err}",
                    hex::encode(spec.digest)
                )
            })?;
        digest_reports.push((spec.clone(), reports));
    }
    for (spec, reports) in &digest_reports {
        println!("digest {}:", hex::encode(spec.digest));
        for report in reports {
            let evidence = &report.policy.evidence;
            println!(
                "  - provider {} status={} code={} source={} catalog_digest_hex={}",
                report.provider_id,
                evidence.observed_status,
                evidence.code,
                evidence.source,
                evidence.catalog_digest_hex,
            );
        }
    }
    if let Some(path) = json_out {
        let digests: Vec<Value> = digest_reports
            .iter()
            .map(|(spec, reports)| {
                let providers: Vec<Value> = reports
                    .iter()
                    .map(|report| {
                        let evidence = &report.policy.evidence;
                        let mut map = Map::new();
                        insert_value!(map["provider"] = report.provider_id.clone());
                        insert_value!(map["observed_status"] = evidence.observed_status.as_u16());
                        insert_value!(map["code"] = evidence.code.clone());
                        insert_value!(map["source"] = evidence.source.clone());
                        insert_value!(
                            map["catalog_digest_hex"] = evidence.catalog_digest_hex.clone()
                        );
                        Value::Object(map)
                    })
                    .collect();
                let mut digest_map = Map::new();
                insert_value!(digest_map["digest_hex"] = hex::encode(spec.digest));
                insert_json!(digest_map["reports"] = Value::Array(providers));
                Value::Object(digest_map)
            })
            .collect();
        let mut summary = Map::new();
        insert_value!(summary["manifest_id_hex"] = manifest_id_hex.clone());
        insert_value!(summary["chunker_handle"] = chunker_handle.clone());
        insert_json!(
            summary["expected_catalog_digest_hex"] = expected_catalog_digest_hex
                .as_ref()
                .map(|value| Value::from(value.clone()))
                .unwrap_or(Value::Null)
        );
        insert_value!(summary["provider_count"] = providers.len() as u64);
        insert_json!(summary["digests"] = Value::Array(digests));
        let summary = Value::Object(summary);
        let rendered =
            to_string_pretty(&summary).map_err(|err| format!("failed to render JSON: {err}"))?;
        write_text(&path, format!("{rendered}\n").as_bytes())?;
    }
    if let Some(path) = markdown_out {
        let mut md = String::from("# Honey Audit Report\n\n");
        md.push_str(&format!(
            "- manifest: `{}`\n- chunker: `{}`\n- expected catalog digest: `{}`\n- providers: {}\n\n",
            manifest_id_hex,
            chunker_handle,
            expected_catalog_digest_hex
                .as_deref()
                .unwrap_or("unspecified"),
            providers.len()
        ));
        for (spec, reports) in &digest_reports {
            md.push_str(&format!("## digest `{}`\n", hex::encode(spec.digest)));
            for report in reports {
                let evidence = &report.policy.evidence;
                md.push_str(&format!(
                    "- {}: status={} code={} source={} catalog_digest_hex={}\n",
                    report.provider_id,
                    evidence.observed_status,
                    evidence.code,
                    evidence.source,
                    evidence.catalog_digest_hex,
                ));
            }
            md.push('\n');
        }
        write_text(&path, md.as_bytes())?;
    }
    Ok(())
}
fn appeal_quote(raw_args: Vec<String>) -> Result<(), String> {
    let mut class: Option<AppealClass> = None;
    let mut backlog: u32 = 0;
    let mut evidence_size_mb: u32 = 0;
    let mut panel_size_override: Option<u32> = None;
    let mut urgency = AppealUrgency::Normal;
    let mut format = String::from("table");
    let mut config_source: Option<JsonSource> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(appeal_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--class" => {
                class = Some(
                    value
                        .parse::<AppealClass>()
                        .map_err(|err| err.to_string())?,
                );
            }
            "--backlog" => backlog = parse_u32_arg("backlog", value, CONTEXT_APPEAL_QUOTE)?,
            "--evidence-mb" => {
                evidence_size_mb = parse_u32_arg("evidence-mb", value, CONTEXT_APPEAL_QUOTE)?;
            }
            "--panel-size" => {
                panel_size_override =
                    Some(parse_u32_arg("panel-size", value, CONTEXT_APPEAL_QUOTE)?);
            }
            "--urgency" => {
                urgency = value
                    .parse::<AppealUrgency>()
                    .map_err(|err| err.to_string())?;
            }
            "--format" => format = value.trim().to_ascii_lowercase(),
            "--config" => {
                config_source = Some(JsonSource::from_arg(value).map_err(|err| {
                    format!("failed to parse --config for `{CONTEXT_APPEAL_QUOTE}`: {err}")
                })?)
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `{CONTEXT_APPEAL_QUOTE}`"
                ));
            }
        }
    }
    let config = if let Some(source) = config_source {
        let value = source.read()?;
        AppealPricingConfig::from_manifest_value(&value)
            .map_err(|err| format!("failed to parse appeal pricing config: {err}"))?
    } else {
        AppealPricingConfig::baseline_v1()
    };
    let class = class.ok_or_else(|| {
        format!(
            "missing required `--class=content|access|fraud|other` for `{CONTEXT_APPEAL_QUOTE}`"
        )
    })?;
    let panel_size = panel_size_override.unwrap_or_else(|| config.default_panel_size());
    let quote = config
        .quote(AppealQuoteInput {
            class,
            backlog,
            evidence_size_mb,
            urgency,
            panel_size,
        })
        .map_err(|err| err.to_string())?;
    let class_cfg = config
        .class_config(class)
        .expect("quoted class must have a configuration entry");
    let valid_until_unix = compute_valid_until(config.quote_ttl_secs());
    let context = AppealQuoteInputs {
        config: &config,
        class,
        urgency,
        backlog,
        evidence_size_mb,
        panel_size,
        quote: &quote,
        valid_until_unix,
    };
    match format.as_str() {
        "json" => print_appeal_quote_json(&context),
        "table" | "text" | "" => {
            print_appeal_quote_table(class_cfg, &context);
            Ok(())
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{CONTEXT_APPEAL_QUOTE}` (expected table|json)"
        )),
    }
}
fn appeal_settle(raw_args: Vec<String>) -> Result<(), String> {
    let mut deposit: Option<Quantity> = None;
    let mut verdict: Option<AppealVerdict> = None;
    let mut panel_size_override: Option<u32> = None;
    let mut format = String::from("table");
    let mut config_source: Option<JsonSource> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(appeal_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--deposit" => {
                deposit = Some(parse_appeal_quantity_literal("deposit", value).map_err(
                    |error| {
                        format!("failed to parse `deposit` for `{CONTEXT_APPEAL_SETTLE}`: {error}")
                    },
                )?);
            }
            "--outcome" => {
                verdict = Some(parse_appeal_verdict(value)?);
            }
            "--panel-size" => {
                panel_size_override =
                    Some(parse_u32_arg("panel-size", value, CONTEXT_APPEAL_SETTLE)?);
            }
            "--format" => format = value.trim().to_ascii_lowercase(),
            "--config" => {
                config_source = Some(JsonSource::from_arg(value).map_err(|err| {
                    format!("failed to parse --config for `{CONTEXT_APPEAL_SETTLE}`: {err}")
                })?)
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `{CONTEXT_APPEAL_SETTLE}`"
                ));
            }
        }
    }
    let config = if let Some(source) = config_source {
        let value = source.read()?;
        AppealSettlementConfig::from_manifest_value(&value)
            .map_err(|err| format!("failed to parse appeal settlement config: {err}"))?
    } else {
        AppealSettlementConfig::baseline_v1()
    };
    let deposit = deposit.ok_or_else(|| {
        format!("missing required `--deposit` (XOR) for `{CONTEXT_APPEAL_SETTLE}`")
    })?;
    let verdict = verdict
        .ok_or_else(|| format!("missing required `--outcome` for `{CONTEXT_APPEAL_SETTLE}`"))?;
    let panel_size = panel_size_override.unwrap_or_else(|| config.default_panel_size());
    let breakdown = config
        .settle(deposit.clone(), panel_size, verdict)
        .map_err(|err| match err {
            AppealSettlementError::MissingDecisionRule { decision } => {
                format!("settlement config is missing a rule for `{decision}`")
            }
            AppealSettlementError::InvalidPanelSize
            | AppealSettlementError::InvalidXorQuantity { .. }
            | AppealSettlementError::DecimalProduct(_)
            | AppealSettlementError::Arithmetic(_) => err.to_string(),
        })?;
    let context = AppealSettlementInputs {
        config: &config,
        deposit_xor: deposit,
        panel_size,
        verdict,
        breakdown,
    };
    match format.as_str() {
        "json" => print_appeal_settlement_json(&context),
        "table" | "text" | "" => {
            print_appeal_settlement_table(&context);
            Ok(())
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{CONTEXT_APPEAL_SETTLE}` (expected table|json)"
        )),
    }
}
fn appeal_disburse(raw_args: Vec<String>) -> Result<(), String> {
    let mut deposit: Option<Quantity> = None;
    let mut verdict: Option<AppealVerdict> = None;
    let mut panel_size_override: Option<u32> = None;
    let mut format = String::from("table");
    let mut config_source: Option<JsonSource> = None;
    let mut refund_account: Option<AccountId> = None;
    let mut treasury_account: Option<AccountId> = None;
    let mut escrow_account: Option<AccountId> = None;
    let mut jurors: Vec<AccountId> = Vec::new();
    let mut no_shows: Vec<AccountId> = Vec::new();
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(appeal_usage());
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--deposit" => {
                deposit = Some(parse_appeal_quantity_literal("deposit", value).map_err(
                    |error| {
                        format!(
                            "failed to parse `deposit` for `{CONTEXT_APPEAL_DISBURSE}`: {error}"
                        )
                    },
                )?);
            }
            "--outcome" => verdict = Some(parse_appeal_verdict(value)?),
            "--panel-size" => {
                panel_size_override =
                    Some(parse_u32_arg("panel-size", value, CONTEXT_APPEAL_DISBURSE)?);
            }
            "--format" => format = value.trim().to_ascii_lowercase(),
            "--config" => {
                config_source = Some(JsonSource::from_arg(value).map_err(|err| {
                    format!("failed to parse --config for `{CONTEXT_APPEAL_DISBURSE}`: {err}")
                })?)
            }
            "--refund-account" => {
                refund_account = Some(parse_account_id_arg(
                    "refund-account",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--treasury-account" => {
                treasury_account = Some(parse_account_id_arg(
                    "treasury-account",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--escrow-account" => {
                escrow_account = Some(parse_account_id_arg(
                    "escrow-account",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--juror" => {
                jurors.push(parse_account_id_arg(
                    "juror",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            "--no-show" => {
                no_shows.push(parse_account_id_arg(
                    "no-show",
                    value,
                    CONTEXT_APPEAL_DISBURSE,
                )?);
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `{CONTEXT_APPEAL_DISBURSE}`"
                ));
            }
        }
    }
    let config = if let Some(source) = config_source {
        let value = source.read()?;
        AppealSettlementConfig::from_manifest_value(&value)
            .map_err(|err| format!("failed to parse appeal settlement config: {err}"))?
    } else {
        AppealSettlementConfig::baseline_v1()
    };
    let deposit = deposit.ok_or_else(|| {
        format!("missing required `--deposit` (XOR) for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    let verdict = verdict
        .ok_or_else(|| format!("missing required `--outcome` for `{CONTEXT_APPEAL_DISBURSE}`"))?;
    let refund_account = refund_account.ok_or_else(|| {
        format!("missing required `--refund-account` for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    let treasury_account = treasury_account.ok_or_else(|| {
        format!("missing required `--treasury-account` for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    let escrow_account = escrow_account.ok_or_else(|| {
        format!("missing required `--escrow-account` for `{CONTEXT_APPEAL_DISBURSE}`")
    })?;
    if jurors.is_empty() {
        return Err(format!(
            "missing required `--juror` entries for `{CONTEXT_APPEAL_DISBURSE}` (supply at least one juror id)"
        ));
    }
    let panel_size = panel_size_override.unwrap_or_else(|| config.default_panel_size());
    let plan = config
        .disburse(AppealDisbursementInput {
            deposit_xor: deposit,
            panel_size,
            verdict,
            jurors: &jurors,
            no_shows: &no_shows,
            refund_account: &refund_account,
            treasury_account: &treasury_account,
            escrow_account: &escrow_account,
        })
        .map_err(|err| match err {
            AppealDisbursementError::Settlement(AppealSettlementError::MissingDecisionRule {
                decision,
            }) => format!("settlement config is missing a rule for `{decision}`"),
            other => other.to_string(),
        })?;
    let context = AppealDisbursementInputs {
        config: &config,
        plan,
    };
    match format.as_str() {
        "json" => print_appeal_disbursement_json(&context),
        "table" | "text" | "" => {
            print_appeal_disbursement_table(&context);
            Ok(())
        }
        other => Err(format!(
            "unsupported `--format={other}` for `{CONTEXT_APPEAL_DISBURSE}` (expected table|json)"
        )),
    }
}
fn compute_valid_until(ttl_secs: u64) -> Option<u64> {
    if ttl_secs == 0 {
        return None;
    }
    let expiry = SystemTime::now().checked_add(Duration::from_secs(ttl_secs))?;
    Some(expiry.duration_since(UNIX_EPOCH).ok()?.as_secs())
}
struct AppealQuoteInputs<'a> {
    config: &'a AppealPricingConfig,
    class: AppealClass,
    urgency: AppealUrgency,
    backlog: u32,
    evidence_size_mb: u32,
    panel_size: u32,
    quote: &'a AppealQuote,
    valid_until_unix: Option<u64>,
}
struct AppealSettlementInputs<'a> {
    config: &'a AppealSettlementConfig,
    deposit_xor: Quantity,
    panel_size: u32,
    verdict: AppealVerdict,
    breakdown: AppealSettlementBreakdown,
}
struct AppealDisbursementInputs<'a> {
    config: &'a AppealSettlementConfig,
    plan: AppealDisbursementPlan,
}
fn print_appeal_quote_table(class_cfg: &AppealClassConfig, ctx: &AppealQuoteInputs<'_>) {
    println!("Appeal deposit quote ({})", ctx.config.version());
    println!("  class: {:<8} urgency: {}", ctx.class, ctx.urgency);
    println!(
        "  deposit: {} XOR (raw {} XOR, min {}, max {})",
        format_exact(&ctx.quote.deposit_xor),
        format_exact(&ctx.quote.breakdown.raw_deposit_xor),
        format_exact(&class_cfg.min_deposit_xor),
        format_exact(&class_cfg.max_deposit_xor),
    );
    println!(
        "  backlog: {} (target {}), factor {}",
        ctx.backlog,
        class_cfg.backlog_target,
        format_exact(&ctx.quote.breakdown.backlog_factor)
    );
    println!(
        "  evidence_size_mb: {} (divisor {}), size multiplier {}",
        ctx.evidence_size_mb,
        format_exact(&class_cfg.size_divisor_mb),
        format_exact(&ctx.quote.breakdown.size_multiplier),
    );
    println!(
        "  urgency multiplier: {}",
        format_exact(&ctx.quote.breakdown.urgency_multiplier)
    );
    println!(
        "  panel multiplier: {} (panel {} / default {})",
        format_exact(&ctx.quote.breakdown.panel_multiplier),
        ctx.panel_size,
        ctx.config.default_panel_size()
    );
    println!(
        "  surge multiplier: {}",
        format_exact(&ctx.quote.breakdown.surge_multiplier)
    );
    if let Some(expiry) = ctx.valid_until_unix {
        println!("  valid until (unix): {expiry}");
    }
}
fn print_appeal_quote_json(ctx: &AppealQuoteInputs<'_>) -> Result<(), String> {
    let mut breakdown = Map::new();
    insert_json!(
        breakdown["base_rate_xor"] =
            Value::String(format_exact(&ctx.quote.breakdown.base_rate_xor))
    );
    insert_json!(
        breakdown["backlog_factor"] =
            Value::String(format_exact(&ctx.quote.breakdown.backlog_factor))
    );
    insert_json!(
        breakdown["size_multiplier"] =
            Value::String(format_exact(&ctx.quote.breakdown.size_multiplier))
    );
    insert_json!(
        breakdown["urgency_multiplier"] =
            Value::String(format_exact(&ctx.quote.breakdown.urgency_multiplier))
    );
    insert_json!(
        breakdown["panel_multiplier"] =
            Value::String(format_exact(&ctx.quote.breakdown.panel_multiplier))
    );
    insert_json!(
        breakdown["surge_multiplier"] =
            Value::String(format_exact(&ctx.quote.breakdown.surge_multiplier))
    );
    insert_json!(
        breakdown["raw_deposit_xor"] =
            Value::String(format_exact(&ctx.quote.breakdown.raw_deposit_xor))
    );
    insert_json!(
        breakdown["min_deposit_xor"] =
            Value::String(format_exact(&ctx.quote.breakdown.min_deposit_xor))
    );
    insert_json!(
        breakdown["max_deposit_xor"] =
            Value::String(format_exact(&ctx.quote.breakdown.max_deposit_xor))
    );
    let mut root = Map::new();
    insert_json!(root["version"] = Value::String(ctx.config.version().to_string()));
    insert_json!(root["class"] = Value::String(ctx.class.as_str().to_string()));
    insert_json!(root["urgency"] = Value::String(ctx.urgency.as_str().to_string()));
    insert_json!(root["deposit_xor"] = Value::String(format_exact(&ctx.quote.deposit_xor)));
    insert_json!(root["backlog_open_cases"] = Value::Number(Number::from(ctx.backlog as u64)));
    insert_json!(
        root["evidence_size_mb"] = Value::Number(Number::from(ctx.evidence_size_mb as u64))
    );
    insert_json!(root["panel_size"] = Value::Number(Number::from(u64::from(ctx.panel_size))));
    insert_json!(
        root["default_panel_size"] =
            Value::Number(Number::from(ctx.config.default_panel_size() as u64))
    );
    insert_json!(root["quote_ttl_secs"] = Value::Number(Number::from(ctx.config.quote_ttl_secs())));
    if let Some(expiry) = ctx.valid_until_unix {
        insert_json!(root["valid_until_unix"] = Value::Number(Number::from(expiry)));
    }
    insert_json!(root["breakdown"] = Value::Object(breakdown));
    let json = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to render JSON quote: {err}"))?;
    println!("{json}");
    Ok(())
}
fn print_appeal_settlement_table(ctx: &AppealSettlementInputs<'_>) {
    println!("Appeal settlement ({})", ctx.config.version());
    println!("  outcome: {}", ctx.verdict);
    println!("  deposit: {} XOR", format_exact(&ctx.deposit_xor));
    println!("  refund: {} XOR", format_exact(&ctx.breakdown.refund_xor));
    println!(
        "  treasury transfer: {} XOR",
        format_exact(&ctx.breakdown.treasury_xor)
    );
    println!(
        "  held in escrow: {} XOR",
        format_exact(&ctx.breakdown.held_xor)
    );
    println!(
        "  panel reward: {} jurors × {} XOR + bonus = {} XOR",
        ctx.panel_size,
        format_exact(&ctx.breakdown.panel_reward_per_juror_xor),
        format_exact(&ctx.breakdown.panel_reward_total_xor)
    );
}
fn print_appeal_settlement_json(ctx: &AppealSettlementInputs<'_>) -> Result<(), String> {
    let mut root = Map::new();
    insert_json!(root["version"] = Value::String(ctx.config.version().to_string()));
    insert_json!(root["outcome"] = Value::String(ctx.verdict.to_string()));
    insert_json!(root["deposit_xor"] = Value::String(format_exact(&ctx.deposit_xor)));
    insert_json!(root["refund_xor"] = Value::String(format_exact(&ctx.breakdown.refund_xor)));
    insert_json!(root["treasury_xor"] = Value::String(format_exact(&ctx.breakdown.treasury_xor)));
    insert_json!(root["held_xor"] = Value::String(format_exact(&ctx.breakdown.held_xor)));
    insert_json!(root["panel_size"] = Value::Number(Number::from(u64::from(ctx.panel_size))));
    insert_json!(
        root["panel_reward_per_juror_xor"] =
            Value::String(format_exact(&ctx.breakdown.panel_reward_per_juror_xor))
    );
    insert_json!(
        root["panel_reward_total_xor"] =
            Value::String(format_exact(&ctx.breakdown.panel_reward_total_xor))
    );
    let json = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to render JSON settlement: {err}"))?;
    println!("{json}");
    Ok(())
}
fn print_appeal_disbursement_table(ctx: &AppealDisbursementInputs<'_>) {
    println!("Appeal disbursement ({})", ctx.config.version());
    println!("  outcome: {}", ctx.plan.verdict);
    println!("  deposit: {} XOR", format_exact(&ctx.plan.deposit_xor));
    println!(
        "  refund -> {}: {} XOR",
        ctx.plan.refund_account,
        format_exact(&ctx.plan.settlement.refund_xor)
    );
    println!(
        "  treasury -> {}: {} XOR (deposit) + {} XOR (forfeited rewards) = {} XOR",
        ctx.plan.treasury_account,
        format_exact(&ctx.plan.settlement.treasury_xor),
        format_exact(&ctx.plan.rewards_forfeited_treasury_xor),
        format_exact(&ctx.plan.total_treasury_xor)
    );
    println!(
        "  held in escrow -> {}: {} XOR",
        ctx.plan.escrow_account,
        format_exact(&ctx.plan.settlement.held_xor)
    );
    println!(
        "  attendance: {}/{} jurors paid",
        ctx.plan.attending_count(),
        ctx.plan.panel_size
    );
    println!(
        "  panel rewards: {} XOR available; {} XOR paid; {} XOR forfeited to treasury",
        format_exact(&ctx.plan.rewards_available_xor),
        format_exact(&ctx.plan.rewards_paid_total_xor),
        format_exact(&ctx.plan.rewards_forfeited_treasury_xor)
    );
    for payout in &ctx.plan.juror_payouts {
        println!(
            "    - {}: stipend {} XOR + bonus {} XOR = {} XOR",
            payout.juror,
            format_exact(&payout.stipend_xor),
            format_exact(&payout.bonus_xor),
            format_exact(&payout.total().expect("validated payout arithmetic"))
        );
    }
    if !ctx.plan.no_show_accounts.is_empty() {
        println!("  no-shows (forfeited rewards):");
        for account in &ctx.plan.no_show_accounts {
            println!("    - {}", account);
        }
    }
}
fn print_appeal_disbursement_json(ctx: &AppealDisbursementInputs<'_>) -> Result<(), String> {
    let mut root = Map::new();
    insert_json!(root["version"] = Value::String(ctx.config.version().to_string()));
    insert_json!(root["outcome"] = Value::String(ctx.plan.verdict.to_string()));
    insert_json!(root["deposit_xor"] = Value::String(format_exact(&ctx.plan.deposit_xor)));
    insert_json!(root["panel_size"] = Value::Number(Number::from(u64::from(ctx.plan.panel_size))));
    let mut refund = Map::new();
    insert_json!(refund["account"] = Value::String(ctx.plan.refund_account.to_string()));
    insert_json!(
        refund["amount_xor"] = Value::String(format_exact(&ctx.plan.settlement.refund_xor))
    );
    insert_json!(root["refund"] = Value::Object(refund));
    let mut treasury = Map::new();
    insert_json!(treasury["account"] = Value::String(ctx.plan.treasury_account.to_string()));
    insert_json!(
        treasury["deposit_component_xor"] =
            Value::String(format_exact(&ctx.plan.settlement.treasury_xor))
    );
    insert_json!(
        treasury["forfeited_rewards_xor"] =
            Value::String(format_exact(&ctx.plan.rewards_forfeited_treasury_xor))
    );
    insert_json!(treasury["total_xor"] = Value::String(format_exact(&ctx.plan.total_treasury_xor)));
    insert_json!(root["treasury"] = Value::Object(treasury));
    let mut held = Map::new();
    insert_json!(held["account"] = Value::String(ctx.plan.escrow_account.to_string()));
    insert_json!(held["amount_xor"] = Value::String(format_exact(&ctx.plan.settlement.held_xor)));
    insert_json!(root["held"] = Value::Object(held));
    let mut rewards = Map::new();
    insert_json!(
        rewards["available_xor"] = Value::String(format_exact(&ctx.plan.rewards_available_xor))
    );
    insert_json!(
        rewards["paid_xor"] = Value::String(format_exact(&ctx.plan.rewards_paid_total_xor))
    );
    insert_json!(
        rewards["forfeited_xor"] =
            Value::String(format_exact(&ctx.plan.rewards_forfeited_treasury_xor))
    );
    insert_json!(
        rewards["attending"] = Value::Number(Number::from(ctx.plan.attending_count() as u64))
    );
    insert_json!(
        rewards["no_shows"] = Value::Array(
            ctx.plan
                .no_show_accounts
                .iter()
                .map(|acct| Value::String(acct.to_string()))
                .collect(),
        )
    );
    let participants: Vec<Value> = ctx
        .plan
        .juror_payouts
        .iter()
        .map(|payout| {
            let mut entry = Map::new();
            insert_json!(entry["account"] = Value::String(payout.juror.to_string()));
            insert_json!(entry["stipend_xor"] = Value::String(format_exact(&payout.stipend_xor)));
            insert_json!(entry["bonus_xor"] = Value::String(format_exact(&payout.bonus_xor)));
            insert_json!(
                entry["total_xor"] = Value::String(format_exact(
                    &payout.total().expect("validated payout arithmetic"),
                ))
            );
            Value::Object(entry)
        })
        .collect();
    insert_json!(rewards["participants"] = Value::Array(participants));
    insert_json!(root["rewards"] = Value::Object(rewards));
    let json = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to render JSON disbursement: {err}"))?;
    println!("{json}");
    Ok(())
}
fn format_exact(value: &impl std::fmt::Display) -> String {
    value.to_string()
}
#[cfg(test)]
mod manifest_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use iroha_crypto::{Algorithm, PublicKey};
    use iroha_data_model::account::AccountId;
    use iroha_data_model::sorafs::moderation::ModerationModelFingerprintV1;
    use norito::json::{Map, Value};
    use sorafs_orchestrator::proxy::LocalQuicProxyConfig;
    use tempfile::TempDir;
    fn account_string(label: u8) -> String {
        let seed = [label; ed25519_dalek::SECRET_KEY_LENGTH];
        let signer = SigningKey::from_bytes(&seed);
        let pk_bytes = signer.verifying_key().to_bytes();
        let pk =
            PublicKey::from_bytes(Algorithm::Ed25519, pk_bytes.as_slice()).expect("public key");
        AccountId::new(pk).to_string()
    }
    fn canonical_temp_path(temp: &TempDir) -> std::path::PathBuf {
        temp.path().canonicalize().expect("canonical tempdir")
    }
    macro_rules! assert_json_str {
        ($object:ident, $field:literal, $expected:expr) => {
            assert_eq!($object.get($field).and_then(Value::as_str), Some($expected));
        };
    }
    macro_rules! assert_json_u64 {
        ($object:ident, $field:literal, $expected:expr) => {
            assert_eq!($object.get($field).and_then(Value::as_u64), Some($expected));
        };
    }
    macro_rules! assert_json_bool {
        ($object:ident, $field:literal, $expected:expr) => {
            assert_eq!(
                $object.get($field).and_then(Value::as_bool),
                Some($expected)
            );
        };
    }
    macro_rules! write_manifest_fixture {
        ($path:ident, $manifest:ident) => {
            fs::write(
                &$path,
                norito::json::to_json_pretty(&$manifest).expect("render manifest json"),
            )
            .expect("write manifest");
        };
    }
    #[test]
    fn write_text_creates_parent_and_writes_bytes() {
        let temp = TempDir::new().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let output_path = temp_path.join("nested").join("summary.json");
        write_text(&output_path, br#"{"ok":true}"#).expect("write text");
        assert_eq!(
            fs::read(&output_path).expect("read output"),
            br#"{"ok":true}"#
        );
    }
    #[cfg(unix)]
    #[test]
    fn write_text_rejects_symlink_output() {
        let temp = TempDir::new().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let target_path = temp_path.join("target.txt");
        fs::write(&target_path, b"unchanged").expect("write target");
        let output_path = temp_path.join("output.txt");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");
        let err = write_text(&output_path, b"changed").expect_err("reject symlink output");
        assert!(
            err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged");
    }
    #[cfg(unix)]
    #[test]
    fn write_text_rejects_symlink_parent() {
        let temp = TempDir::new().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("output.txt");
        let err = write_text(&output_path, b"changed").expect_err("reject symlink parent");
        assert!(
            err.contains("parent") && err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert!(
            !real_dir.join("output.txt").exists(),
            "symlink parent should not receive output"
        );
    }
    #[cfg(unix)]
    #[test]
    fn moderation_bounded_reader_rejects_symlink_input() {
        let temp = TempDir::new().expect("tempdir");
        let target = temp.path().join("target");
        let linked = temp.path().join("linked");
        fs::write(&target, b"manifest").expect("write target");
        std::os::unix::fs::symlink(&target, &linked).expect("create symlink");
        let error = read_file_bounded(&linked, 1024, "test input")
            .expect_err("symlink input must be rejected");
        assert!(error.contains("non-symlink"), "unexpected error: {error}");
    }
    #[test]
    fn proxy_set_mode_updates_config_file() {
        let temp = TempDir::new().expect("tempdir");
        let root = canonical_temp_path(&temp);
        let config_path = root.join("orchestrator.json");
        let json_out_path = root.join("summary.json");
        let config = OrchestratorConfig {
            local_proxy: Some(LocalQuicProxyConfig {
                bind_addr: "127.0.0.1:0".into(),
                telemetry_label: Some("test-proxy".into()),
                proxy_mode: ProxyMode::Bridge,
                ..LocalQuicProxyConfig::default()
            }),
            ..OrchestratorConfig::default()
        };
        let config_value = orchestrator_config_to_json(&config);
        let config_json = norito::json::to_json_pretty(&config_value).expect("render config json");
        fs::write(&config_path, config_json).expect("write config");
        proxy_set_mode(vec![
            format!("--orchestrator-config={}", config_path.display()),
            "--mode=metadata-only".into(),
            format!("--json-out={}", json_out_path.display()),
        ])
        .expect("proxy set mode succeeds");
        let updated_json = fs::read_to_string(&config_path).expect("read updated config file");
        let updated_value: Value =
            norito::json::from_str(&updated_json).expect("parse updated config");
        let updated_config =
            orchestrator_config_from_json(&updated_value).expect("decode updated config");
        assert_eq!(
            updated_config
                .local_proxy
                .as_ref()
                .expect("local proxy")
                .proxy_mode,
            ProxyMode::MetadataOnly
        );
        let summary_json = fs::read_to_string(&json_out_path).expect("read summary json");
        let summary_value: Value =
            norito::json::from_str(&summary_json).expect("parse summary json");
        let summary_map = summary_value.as_object().expect("summary to be an object");
        assert_json_str!(summary_map, "mode_effective", "metadata-only");
        assert_json_str!(summary_map, "mode_previous", "bridge");
    }
    #[test]
    fn moderation_validate_corpus_accepts_valid_manifest() {
        let manifest = adversarial_corpus_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("corpus.json");
        let manifest_json = norito::json::to_json_pretty(&manifest).expect("render manifest json");
        fs::write(&manifest_path, manifest_json).expect("write manifest json");
        moderation_validate_corpus(vec![format!("--manifest={}", manifest_path.display())])
            .expect("corpus manifest validated");
    }
    fn adversarial_corpus_manifest_fixture() -> AdversarialCorpusManifestV1 {
        use iroha_data_model::sorafs::moderation::{
            ADVERSARIAL_CORPUS_VERSION_V1, AdversarialPerceptualFamilyV1,
            AdversarialPerceptualVariantV1,
        };
        AdversarialCorpusManifestV1 {
            schema_version: ADVERSARIAL_CORPUS_VERSION_V1,
            issued_at_unix: 1_740_000_000,
            cohort_label: Some("test-cohort".to_string()),
            families: vec![AdversarialPerceptualFamilyV1 {
                family_id: [0x11; 16],
                description: "jpeg jitter corpus".to_string(),
                variants: vec![AdversarialPerceptualVariantV1 {
                    variant_id: [0x22; 16],
                    attack_vector: "jpeg_jitter".to_string(),
                    reference_cid_b64: None,
                    perceptual_hash: Some([0x33; 32]),
                    hamming_radius: 8,
                    embedding_digest: None,
                    notes: Some("sample variant".to_string()),
                }],
            }],
        }
    }
    fn signed_moderation_repro_manifest_fixture() -> ModerationReproManifestV1 {
        use iroha_data_model::sorafs::moderation::{
            MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationReproBodyV1,
            ModerationReproSignatureV1, ModerationSeedMaterialV1,
        };
        let (models, _) = moderation_model_artifacts_fixture();
        let mut body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [0xA1; 16],
            manifest_digest: [0; 32],
            runner_hash: moderation_runner_current_executable_hash()
                .expect("hash fixture runner executable"),
            runtime_version: "sorafs-ai-runner local-test".to_string(),
            issued_at_unix: 1_800_000_000,
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sfm4a:test".to_string(),
                seed_version: 1,
                run_nonce: [0xD4; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 6_000,
                escalate: 8_500,
            },
            models,
            notes: Some("local runner fixture".to_string()),
        };
        body.refresh_manifest_digest()
            .expect("refresh fixture manifest digest");
        let keypair = iroha_crypto::KeyPair::try_from_seed(vec![0xE5; 32], Algorithm::Ed25519)
            .expect("derive moderation fixture keypair");
        let signature = iroha_crypto::SignatureOf::try_new(keypair.private_key(), &body)
            .expect("sign moderation fixture body");
        ModerationReproManifestV1 {
            body,
            signatures: vec![ModerationReproSignatureV1 {
                role: "council".to_string(),
                public_key: keypair.public_key().clone(),
                signature,
            }],
        }
    }
    fn moderation_model_artifacts_fixture()
    -> (Vec<ModerationModelFingerprintV1>, Vec<(String, Vec<u8>)>) {
        use iroha_data_model::sorafs::moderation::{
            MODERATION_MODEL_ARTIFACT_VERSION_V1, MODERATION_MODEL_FEATURE_COUNT_V1,
            MODERATION_MODEL_WORKING_MEMORY_BYTES_V1, ModerationCalibrationKnotV1,
            ModerationFeatureProfileV1, ModerationModelArtifactV1, ModerationModelEngineV1,
            moderation_model_required_operations_v1,
        };
        use sorafs_orchestrator::moderation_runner::fingerprint_model_artifact;
        let mut fingerprints = Vec::new();
        let mut files = Vec::new();
        for (model_id, filename, feature, signed_weight, ensemble_weight) in [
            ([0x11; 16], "model-11.norito", b'a', 1, 7_000),
            ([0x44; 16], "model-44.norito", b'z', -1, 3_000),
        ] {
            let calibration = vec![
                ModerationCalibrationKnotV1 {
                    input: -10_000,
                    score_bps: 0,
                },
                ModerationCalibrationKnotV1 {
                    input: 10_000,
                    score_bps: 10_000,
                },
            ];
            let mut weights = vec![0; MODERATION_MODEL_FEATURE_COUNT_V1];
            weights[usize::from(feature)] = signed_weight;
            let artifact = ModerationModelArtifactV1 {
                schema_version: MODERATION_MODEL_ARTIFACT_VERSION_V1,
                engine: ModerationModelEngineV1::DeterministicLinearV1,
                feature_profile: ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
                model_id,
                max_input_bytes: 4096,
                max_operations: moderation_model_required_operations_v1(4096, calibration.len())
                    .expect("fixture operation budget"),
                working_memory_bytes: MODERATION_MODEL_WORKING_MEMORY_BYTES_V1,
                bias: 0,
                weights,
                calibration,
            };
            let (fingerprint, bytes) =
                fingerprint_model_artifact(filename, &artifact, Some(ensemble_weight))
                    .expect("fixture model fingerprint");
            fingerprints.push(fingerprint);
            files.push((filename.to_owned(), bytes));
        }
        (fingerprints, files)
    }
    fn write_moderation_model_artifacts_fixture(root: &Path) {
        fs::create_dir_all(root).expect("create fixture artifact root");
        for (path, bytes) in moderation_model_artifacts_fixture().1 {
            fs::write(root.join(path), bytes).expect("write fixture model artifact");
        }
    }
    fn resign_moderation_repro_manifest(manifest: &mut ModerationReproManifestV1) {
        manifest
            .body
            .refresh_manifest_digest()
            .expect("refresh fixture manifest digest");
        let keypair = iroha_crypto::KeyPair::try_from_seed(vec![0xE5; 32], Algorithm::Ed25519)
            .expect("derive moderation fixture keypair");
        let signature = iroha_crypto::SignatureOf::try_new(keypair.private_key(), &manifest.body)
            .expect("re-sign moderation fixture body");
        manifest.signatures[0].signature = signature;
    }
    fn moderation_registry_fixture_service(state_path: &Path) -> ModerationRegistryService {
        let state = moderation_registry_load_state(state_path).expect("load registry state");
        ModerationRegistryService {
            state_path: state_path.to_path_buf(),
            state: Mutex::new(state),
            max_body_bytes: 8192,
            snapshot_limit: 25,
        }
    }
    fn moderation_registry_manifest_request<T: norito::core::NoritoSerialize>(
        manifest: &T,
    ) -> Vec<u8> {
        let manifest_bytes = to_bytes(manifest).expect("encode manifest as Norito");
        norito::json::to_vec(&norito::json!({
            "manifest_b64": (BASE64_STANDARD.encode(manifest_bytes))
        }))
        .expect("registry request JSON")
    }
    #[test]
    fn moderation_run_local_emits_torii_screening_request_json() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let root = canonical_temp_path(&temp);
        let manifest_path = root.join("repro.json");
        let payload_path = root.join("payload.bin");
        let out_path = root.join("screening.json");
        let artifact_root = root.join("artifacts");
        write_moderation_model_artifacts_fixture(&artifact_root);
        let payload = b"moderation payload bytes";
        write_manifest_fixture!(manifest_path, manifest);
        fs::write(&payload_path, payload).expect("write payload");
        moderation_run_local(vec![
            format!("--manifest={}", manifest_path.display()),
            format!("--artifact-root={}", artifact_root.display()),
            format!("--payload={}", payload_path.display()),
            "--subject=cid:bafy-local-runner".to_string(),
            "--screened-at=1800001234".to_string(),
            "--notes=local deterministic run".to_string(),
            format!("--json-out={}", out_path.display()),
        ])
        .expect("local runner succeeds");
        let rendered = fs::read_to_string(&out_path).expect("read runner output");
        let value: Value = norito::json::from_str(&rendered).expect("parse runner output");
        let expected_runner = LoadedModerationRunnerV1::load_verified(
            manifest.clone(),
            &artifact_root,
            manifest.body.runner_hash,
        )
        .expect("load expected fixture runner");
        let expected = moderation_local_runner_screening_json(
            &expected_runner,
            payload,
            "cid:bafy-local-runner",
            1_800_001_234,
            Some("local deterministic run"),
            MODERATION_RUNNER_DEFAULT_MAX_PAYLOAD_BYTES,
        )
        .expect("direct local runner output");
        assert_eq!(value, expected);
        let object = value.as_object().expect("runner output object");
        assert_json_str!(object, "subject", "cid:bafy-local-runner");
        assert_json_str!(
            object,
            "subject_digest_hex",
            hex_encode(blake3_hash(payload).as_bytes()).as_str()
        );
        assert_json_str!(
            object,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_json_str!(
            object,
            "runner_hash_hex",
            hex_encode(manifest.body.runner_hash).as_str()
        );
        let score = object
            .get("combined_score_bps")
            .and_then(Value::as_u64)
            .expect("score present");
        assert!(score <= 10_000);
        assert!(matches!(
            object.get("verdict").and_then(Value::as_str),
            Some("pass" | "quarantine" | "escalate")
        ));
        assert_eq!(
            object
                .get("policy_digest_hex")
                .and_then(Value::as_str)
                .expect("policy digest")
                .len(),
            64
        );
        assert_eq!(
            object
                .get("evidence_digest_hex")
                .and_then(Value::as_str)
                .expect("evidence digest")
                .len(),
            64
        );
    }
    fn moderation_runner_fixture_service(
        manifest: ModerationReproManifestV1,
    ) -> ModerationRunnerService {
        let artifact_root = TempDir::new().expect("fixture artifact root");
        write_moderation_model_artifacts_fixture(artifact_root.path());
        let observed_runner_hash = manifest.body.runner_hash;
        let runner = LoadedModerationRunnerV1::load_verified(
            manifest,
            artifact_root.path(),
            observed_runner_hash,
        )
        .expect("load fixture moderation runner");
        ModerationRunnerService {
            runner,
            signed: None,
            manifest_source: "fixture-repro.json".to_string(),
            max_body_bytes: 4096,
            max_payload_bytes: 4096,
        }
    }
    fn signed_trust_policy_fixture(
        manifest: &ModerationReproManifestV1,
        governance: &iroha_crypto::KeyPair,
        runner_keys: &[&iroha_crypto::KeyPair],
        result_quorum: u16,
        now_unix: u64,
    ) -> ModerationTrustPolicyV1 {
        use iroha_data_model::sorafs::moderation::{
            MODERATION_TRUST_POLICY_VERSION_V1, ModerationTrustPolicyBodyV1,
            ModerationTrustPolicySignatureV1, ModerationTrustedSignerV1,
        };
        let mut trusted_signers = runner_keys
            .iter()
            .enumerate()
            .map(|(index, keypair)| ModerationTrustedSignerV1 {
                role: format!("runner-{index}"),
                public_key: keypair.public_key().clone(),
                valid_from_unix: now_unix - 60,
                valid_until_unix: now_unix + 3_600,
                revoked_at_unix: None,
            })
            .collect::<Vec<_>>();
        trusted_signers.sort_by(|left, right| left.public_key.cmp(&right.public_key));
        let mut body = ModerationTrustPolicyBodyV1 {
            schema_version: MODERATION_TRUST_POLICY_VERSION_V1,
            policy_id: [0xC1; 16],
            policy_digest: [0; 32],
            manifest_id: manifest.body.manifest_id,
            manifest_digest: manifest.body.manifest_digest,
            runner_hash: manifest.body.runner_hash,
            issued_at_unix: now_unix - 120,
            valid_from_unix: now_unix - 60,
            valid_until_unix: now_unix + 3_600,
            result_quorum,
            governance_quorum: 1,
            max_result_age_secs: 600,
            max_result_ttl_secs: 300,
            max_clock_skew_secs: 5,
            trusted_signers,
            notes: Some("externally anchored test policy".to_string()),
        };
        body.refresh_policy_digest().expect("policy digest");
        ModerationTrustPolicyV1 {
            signatures: vec![ModerationTrustPolicySignatureV1 {
                role: "governance".to_string(),
                public_key: governance.public_key().clone(),
                signature: iroha_crypto::SignatureOf::try_new(governance.private_key(), &body)
                    .expect("policy signature"),
            }],
            body,
        }
    }
    fn moderation_signed_runner_fixture_service()
    -> (ModerationRunnerService, TempDir, ModerationReproManifestV1) {
        let manifest = signed_moderation_repro_manifest_fixture();
        let mut service = moderation_runner_fixture_service(manifest.clone());
        let now_unix = moderation_trusted_now_unix().expect("trusted time");
        let governance = iroha_crypto::KeyPair::try_random().expect("governance key");
        let runner_key = iroha_crypto::KeyPair::try_random().expect("runner key");
        let policy =
            signed_trust_policy_fixture(&manifest, &governance, &[&runner_key], 1, now_unix);
        let trust_anchors: BTreeSet<PublicKey> =
            std::iter::once(governance.public_key().clone()).collect();
        let signing_runner = LoadedModerationSigningRunnerV1::from_verified(
            service.runner.clone(),
            policy,
            trust_anchors.clone(),
            1,
            runner_key.private_key().clone(),
            now_unix,
        )
        .expect("signing runner");
        let provenance_root = TempDir::new().expect("provenance root");
        let provenance = ModerationProvenanceStoreV1::open(
            provenance_root.path().join("runner-provenance.to"),
            [0xD1; 16],
        )
        .expect("provenance store");
        service.signed = Some(ModerationSignedRunnerState {
            signing_runner,
            provenance,
            trust_anchors,
            minimum_governance_quorum: 1,
            transaction_guard: Mutex::new(()),
        });
        (service, provenance_root, manifest)
    }
    fn moderation_runner_http_request(method: &str, path: &str, body: &[u8]) -> Vec<u8> {
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: runner.local\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        request.extend_from_slice(body);
        request
    }
    fn moderation_runner_response_parts(response: &[u8]) -> (&str, Value) {
        let header_end = find_http_header_end(response).expect("response header terminator");
        let header = std::str::from_utf8(&response[..header_end]).expect("response headers");
        let body = &response[header_end + 4..];
        let json: Value = norito::json::from_slice(body).expect("response body JSON");
        (header, json)
    }
    #[test]
    fn moderation_registry_service_admits_and_persists_manifests() {
        let repro_manifest = signed_moderation_repro_manifest_fixture();
        let corpus_manifest = adversarial_corpus_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let state_path = temp.path().join("registry-state.to");
        let service = moderation_registry_fixture_service(&state_path);
        moderation_registry_save_state(
            &state_path,
            &service.state.lock().expect("registry lock").clone(),
        )
        .expect("write empty registry state");
        let repro_body = moderation_registry_manifest_request(&repro_manifest);
        let repro_request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/model-registry/repro-manifests",
            &repro_body,
        );
        let repro_response =
            moderation_registry_http_response(&service, &repro_request, service.max_body_bytes);
        let (header, repro_json) = moderation_runner_response_parts(&repro_response);
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_json_str!(
            repro_json,
            "schema",
            "sorafs.moderation.model_registry.repro_manifest_admission.v1"
        );
        assert_json_bool!(repro_json, "created", true);
        let corpus_body = moderation_registry_manifest_request(&corpus_manifest);
        let corpus_request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/model-registry/corpora",
            &corpus_body,
        );
        let corpus_response =
            moderation_registry_http_response(&service, &corpus_request, service.max_body_bytes);
        let (header, corpus_json) = moderation_runner_response_parts(&corpus_response);
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_json_str!(
            corpus_json,
            "schema",
            "sorafs.moderation.model_registry.corpus_manifest_admission.v1"
        );
        assert_json_bool!(corpus_json, "created", true);
        let snapshot_request =
            moderation_runner_http_request("GET", "/v1/sorafs/moderation/model-registry", &[]);
        let snapshot_response =
            moderation_registry_http_response(&service, &snapshot_request, service.max_body_bytes);
        let (header, snapshot) = moderation_runner_response_parts(&snapshot_response);
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_json_str!(
            snapshot,
            "schema",
            "sorafs.moderation.model_registry.snapshot.v1"
        );
        assert_json_u64!(snapshot, "repro_manifest_count", 1);
        assert_json_u64!(snapshot, "corpus_count", 1);
        assert!(state_path.exists(), "registry service should persist state");
        let reloaded = moderation_registry_fixture_service(&state_path);
        let reloaded_response = moderation_registry_http_response(
            &reloaded,
            &snapshot_request,
            reloaded.max_body_bytes,
        );
        let (_, reloaded_snapshot) = moderation_runner_response_parts(&reloaded_response);
        assert_eq!(
            reloaded_snapshot
                .get("repro_manifest_count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            reloaded_snapshot
                .get("corpus_count")
                .and_then(Value::as_u64),
            Some(1)
        );
    }
    #[test]
    fn moderation_registry_service_rejects_conflicting_manifest_id() {
        let repro_manifest = signed_moderation_repro_manifest_fixture();
        let mut conflicting_manifest = repro_manifest.clone();
        conflicting_manifest.body.notes = Some("conflicting release metadata".to_string());
        resign_moderation_repro_manifest(&mut conflicting_manifest);
        let temp = TempDir::new().expect("tempdir");
        let state_path = temp.path().join("registry-state.to");
        let service = moderation_registry_fixture_service(&state_path);
        let original_body = moderation_registry_manifest_request(&repro_manifest);
        let original_request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/model-registry/repro-manifests",
            &original_body,
        );
        let original_response =
            moderation_registry_http_response(&service, &original_request, service.max_body_bytes);
        let (header, _) = moderation_runner_response_parts(&original_response);
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        let conflict_body = moderation_registry_manifest_request(&conflicting_manifest);
        let conflict_request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/model-registry/repro-manifests",
            &conflict_body,
        );
        let conflict_response =
            moderation_registry_http_response(&service, &conflict_request, service.max_body_bytes);
        let (header, body) = moderation_runner_response_parts(&conflict_response);
        assert!(header.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(
            body.get("message")
                .and_then(Value::as_str)
                .expect("error message")
                .contains("conflicts with registry")
        );
    }
    fn moderation_committee_fixture_service(
        manifest: ModerationReproManifestV1,
        quorum: usize,
    ) -> ModerationCommitteeService {
        ModerationCommitteeService {
            manifest,
            authenticated: None,
            manifest_source: "fixture-repro.json".to_string(),
            quorum,
            max_body_bytes: 4096,
        }
    }
    #[test]
    fn signed_runner_endpoint_uses_server_time_and_persists_verified_result() {
        let (service, _provenance_root, manifest) = moderation_signed_runner_fixture_service();
        let request_body = to_vec(&Value::Object(Map::from_iter([
            ("subject".into(), Value::from("cid:production-subject")),
            (
                "payload_b64".into(),
                Value::from(BASE64_STANDARD.encode(b"signed payload")),
            ),
            ("notes".into(), Value::from("operator-reviewed")),
        ])))
        .expect("request JSON");
        let request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/runner/screen-signed",
            &request_body,
        );
        let response = moderation_runner_http_response(&service, &request, service.max_body_bytes);
        let (header, output) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 200 OK"), "{header}");
        assert_eq!(
            output
                .get("payload_bytes_included")
                .and_then(Value::as_bool),
            Some(false)
        );
        let encoded = output
            .get("signed_result_norito_b64")
            .and_then(Value::as_str)
            .expect("signed result bytes");
        let bytes = BASE64_STANDARD.decode(encoded).expect("base64 result");
        let result = decode_moderation_signed_result(&bytes, "test response")
            .expect("canonical signed result");
        assert_eq!(result.body.subject, "cid:production-subject");
        assert_eq!(
            result.body.subject_digest,
            *blake3_hash(b"signed payload").as_bytes()
        );
        let state = service.signed.as_ref().expect("signed state");
        result
            .validate(
                &manifest,
                state.signing_runner.trust_policy(),
                moderation_trusted_now_unix().expect("trusted time"),
            )
            .expect("independent signed result validation");
        let provenance = state.provenance.snapshot().expect("provenance snapshot");
        assert_eq!(provenance.entries.len(), 1);
        provenance.validate_chain().expect("valid provenance chain");
        let status = moderation_runner_status_json(&service, "ready", None);
        assert_json_bool!(status, "signed_results", true);
        assert_json_u64!(status, "provenance_entry_count", 1);
    }
    #[test]
    fn signed_runner_rejects_client_time_unsigned_route_and_unknown_fields() {
        let (service, _provenance_root, _) = moderation_signed_runner_fixture_service();
        let with_client_time = to_vec(&Value::Object(Map::from_iter([
            ("subject".into(), Value::from("cid:subject")),
            (
                "payload_b64".into(),
                Value::from(BASE64_STANDARD.encode(b"payload")),
            ),
            ("screened_at_unix".into(), Value::from(1_u64)),
        ])))
        .expect("request JSON");
        let request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/runner/screen-signed",
            &with_client_time,
        );
        let response = moderation_runner_http_response(&service, &request, service.max_body_bytes);
        assert!(
            moderation_runner_response_parts(&response)
                .0
                .starts_with("HTTP/1.1 400 Bad Request")
        );
        let unsigned_request =
            moderation_runner_http_request("POST", "/v1/sorafs/moderation/runner/screen", b"{}");
        let unsigned_response =
            moderation_runner_http_response(&service, &unsigned_request, service.max_body_bytes);
        assert!(
            moderation_runner_response_parts(&unsigned_response)
                .0
                .starts_with("HTTP/1.1 409 Conflict")
        );
        assert_eq!(
            service
                .signed
                .as_ref()
                .expect("signed state")
                .provenance
                .snapshot()
                .expect("snapshot")
                .entries
                .len(),
            0
        );
    }
    #[test]
    fn signed_runner_fails_fast_when_signing_transaction_is_busy() {
        let (service, _provenance_root, _) = moderation_signed_runner_fixture_service();
        let state = service.signed.as_ref().expect("signed state");
        let _held = state.transaction_guard.lock().expect("hold transaction");
        let request_body = to_vec(&Value::Object(Map::from_iter([
            ("subject".into(), Value::from("cid:subject")),
            (
                "payload_b64".into(),
                Value::from(BASE64_STANDARD.encode(b"payload")),
            ),
        ])))
        .expect("request JSON");
        assert!(matches!(
            moderation_runner_signed_screen_request_json(&service, &request_body)
                .expect_err("busy signer fails fast"),
            ModerationSignedRunnerRequestError::Unavailable(_)
        ));
    }
    fn moderation_authenticated_committee_fixture_service() -> (
        ModerationCommitteeService,
        TempDir,
        Vec<ModerationSignedScreeningResultV1>,
    ) {
        let manifest = signed_moderation_repro_manifest_fixture();
        let runner_service = moderation_runner_fixture_service(manifest.clone());
        let now_unix = moderation_trusted_now_unix().expect("trusted time");
        let governance = iroha_crypto::KeyPair::try_random().expect("governance key");
        let runner_a = iroha_crypto::KeyPair::try_random().expect("runner a");
        let runner_b = iroha_crypto::KeyPair::try_random().expect("runner b");
        let policy = signed_trust_policy_fixture(
            &manifest,
            &governance,
            &[&runner_a, &runner_b],
            2,
            now_unix,
        );
        let trust_anchors: BTreeSet<PublicKey> =
            std::iter::once(governance.public_key().clone()).collect();
        let signer_a = LoadedModerationSigningRunnerV1::from_verified(
            runner_service.runner.clone(),
            policy.clone(),
            trust_anchors.clone(),
            1,
            runner_a.private_key().clone(),
            now_unix,
        )
        .expect("signer a");
        let signer_b = LoadedModerationSigningRunnerV1::from_verified(
            runner_service.runner,
            policy.clone(),
            trust_anchors.clone(),
            1,
            runner_b.private_key().clone(),
            now_unix,
        )
        .expect("signer b");
        let payload = b"committee payload";
        let result_a = signer_a
            .screen_signed(payload, 4096, "cid:committee-subject", None, now_unix)
            .expect("result a");
        let result_b = signer_b
            .screen_signed(payload, 4096, "cid:committee-subject", None, now_unix)
            .expect("result b");
        let provenance_root = TempDir::new().expect("provenance root");
        let provenance = ModerationProvenanceStoreV1::open(
            provenance_root.path().join("committee-provenance.to"),
            [0xD2; 16],
        )
        .expect("provenance store");
        (
            ModerationCommitteeService {
                manifest,
                authenticated: Some(ModerationAuthenticatedCommitteeState {
                    trust_policy: policy,
                    trust_anchors,
                    minimum_governance_quorum: 1,
                    provenance,
                    transaction_guard: Mutex::new(()),
                }),
                manifest_source: "fixture-repro.json".to_string(),
                quorum: 2,
                max_body_bytes: 1024 * 1024,
            },
            provenance_root,
            vec![result_a, result_b],
        )
    }
    fn authenticated_committee_request(results: &[ModerationSignedScreeningResultV1]) -> Vec<u8> {
        let encoded = results
            .iter()
            .map(|result| {
                Value::from(
                    BASE64_STANDARD.encode(to_bytes(result).expect("canonical signed result")),
                )
            })
            .collect();
        to_vec(&Value::Object(Map::from_iter([(
            "signed_results_norito_b64".into(),
            Value::Array(encoded),
        )])))
        .expect("committee request JSON")
    }
    #[test]
    fn authenticated_committee_endpoint_verifies_quorum_and_persists_full_provenance() {
        let (service, _provenance_root, results) =
            moderation_authenticated_committee_fixture_service();
        let body = authenticated_committee_request(&results);
        let request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/committee/aggregate-authenticated",
            &body,
        );
        let response =
            moderation_committee_http_response(&service, &request, service.max_body_bytes);
        let (header, output) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 200 OK"), "{header}");
        assert_json_u64!(output, "distinct_signer_count", 2);
        assert_eq!(
            output
                .get("payload_bytes_included")
                .and_then(Value::as_bool),
            Some(false)
        );
        let encoded = output
            .get("aggregate_norito_b64")
            .and_then(Value::as_str)
            .expect("aggregate bytes");
        let bytes = BASE64_STANDARD.decode(encoded).expect("base64 aggregate");
        let aggregate: ModerationCommitteeAggregateV1 =
            decode_from_bytes(&bytes).expect("decode aggregate");
        assert_eq!(aggregate.members.len(), 2);
        assert_eq!(
            aggregate.computed_aggregate_digest().unwrap(),
            aggregate.aggregate_digest
        );
        let state = service.authenticated.as_ref().expect("authenticated state");
        let provenance = state.provenance.snapshot().expect("snapshot");
        assert_eq!(provenance.entries.len(), 3);
        provenance.validate_chain().expect("valid chain");
        assert!(matches!(
            &provenance.entries[2].payload,
            iroha_data_model::sorafs::moderation::ModerationProvenancePayloadV1::CommitteeAggregate(
                _
            )
        ));
    }
    #[test]
    fn authenticated_committee_rejects_duplicate_signer_payload_bytes_and_legacy_route() {
        let (service, _provenance_root, results) =
            moderation_authenticated_committee_fixture_service();
        let duplicate_body =
            authenticated_committee_request(&[results[0].clone(), results[0].clone()]);
        let duplicate_request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/committee/aggregate-authenticated",
            &duplicate_body,
        );
        let duplicate_response = moderation_committee_http_response(
            &service,
            &duplicate_request,
            service.max_body_bytes,
        );
        assert!(
            moderation_runner_response_parts(&duplicate_response)
                .0
                .starts_with("HTTP/1.1 400 Bad Request")
        );
        let payload_body = to_vec(&Value::Object(Map::from_iter([
            ("signed_results_norito_b64".into(), Value::Array(Vec::new())),
            ("payload_b64".into(), Value::from("c2VjcmV0")),
        ])))
        .expect("payload request");
        assert!(matches!(
            moderation_committee_authenticated_request_json(&service, &payload_body)
                .expect_err("payload bytes forbidden"),
            ModerationAuthenticatedCommitteeRequestError::BadRequest(_)
        ));
        let legacy_request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/committee/aggregate",
            b"{}",
        );
        let legacy_response =
            moderation_committee_http_response(&service, &legacy_request, service.max_body_bytes);
        assert!(
            moderation_runner_response_parts(&legacy_response)
                .0
                .starts_with("HTTP/1.1 409 Conflict")
        );
        assert_eq!(
            service
                .authenticated
                .as_ref()
                .expect("authenticated state")
                .provenance
                .snapshot()
                .expect("snapshot")
                .entries
                .len(),
            0
        );
    }
    fn moderation_runner_canary_fixture_server(
        manifest: ModerationReproManifestV1,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind runner canary fixture");
        let addr = listener.local_addr().expect("runner canary fixture addr");
        let service = moderation_runner_fixture_service(manifest);
        let handle = std::thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().expect("accept runner canary request");
                moderation_runner_handle_stream(stream, &service, service.max_body_bytes)
                    .expect("handle runner canary request");
            }
        });
        (format!("http://{addr}"), handle)
    }
    fn moderation_committee_canary_fixture_server(
        manifest: ModerationReproManifestV1,
        quorum: usize,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind committee canary fixture");
        let addr = listener
            .local_addr()
            .expect("committee canary fixture addr");
        let service = moderation_committee_fixture_service(manifest, quorum);
        let handle = std::thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().expect("accept committee canary request");
                moderation_committee_handle_stream(stream, &service, service.max_body_bytes)
                    .expect("handle committee canary request");
            }
        });
        (format!("http://{addr}"), handle)
    }
    fn moderation_canary_test_context(generated_at_unix: u64) -> ModerationCanaryDeploymentContext {
        ModerationCanaryDeploymentContext {
            generated_at_unix,
            deployment_id: "ai-prescreen-production-20260701".to_string(),
            environment: "production".to_string(),
        }
    }
    fn moderation_canary_test_probe(
        method: &'static str,
        url: &str,
        response: Value,
    ) -> ModerationCanaryHttpProbe {
        let request_body = if method == "GET" { &[][..] } else { b"{}" };
        let response_body = to_vec(&response).expect("encode test canary response");
        ModerationCanaryHttpProbe {
            method,
            url: url.to_string(),
            status_code: 200,
            request_bytes: u64::try_from(request_body.len()).expect("request length"),
            request_body_blake3: *blake3_hash(request_body).as_bytes(),
            response_bytes: u64::try_from(response_body.len()).expect("response length"),
            response_body_blake3: *blake3_hash(&response_body).as_bytes(),
            response,
        }
    }
    fn moderation_canary_test_result_fingerprint(index: u8) -> Value {
        let digest = [index; 32];
        Value::Object(Map::from_iter([
            (
                "name".into(),
                Value::from(format!(
                    "ai-prescreen-committee-result-{}",
                    hex_encode(digest)
                )),
            ),
            ("bytes".into(), Value::from(256_u64 + u64::from(index))),
            ("body_blake3_hex".into(), Value::from(hex_encode(digest))),
            ("payload_bytes_included".into(), Value::from(false)),
            ("private_payloads_included".into(), Value::from(false)),
        ]))
    }
    fn moderation_committee_result_fixture(
        manifest: &ModerationReproManifestV1,
        payload: &[u8],
        subject: &str,
        score: u16,
        screened_at_unix: u64,
    ) -> Value {
        let service = moderation_runner_fixture_service(manifest.clone());
        let inference = service
            .runner
            .infer(payload, service.max_payload_bytes)
            .expect("fixture inference");
        let verdict = moderation_score_verdict(score, manifest.body.thresholds);
        let subject_digest = *blake3_hash(payload).as_bytes();
        let policy_digest = moderation_local_runner_policy_digest(manifest).expect("policy digest");
        let evidence_digest =
            moderation_local_runner_evidence_digest(ModerationLocalRunnerEvidenceInput {
                manifest,
                subject,
                subject_digest: &subject_digest,
                score,
                verdict,
                screened_at_unix,
                policy_digest: &policy_digest,
                model_scores: &inference.model_scores,
            });
        let mut value = moderation_local_runner_screening_json(
            &service.runner,
            payload,
            subject,
            screened_at_unix,
            Some("committee fixture"),
            service.max_payload_bytes,
        )
        .expect("runner output");
        let object = value.as_object_mut().expect("runner output object");
        insert_value!(object["combined_score_bps"] = u64::from(score));
        insert_value!(object["verdict"] = verdict);
        insert_value!(object["screened_at_unix"] = screened_at_unix);
        insert_value!(object["evidence_digest_hex"] = hex_encode(evidence_digest));
        value
    }
    fn write_moderation_json(path: &Path, value: &Value) {
        let rendered = to_string_pretty(value).expect("render moderation JSON");
        fs::write(path, format!("{rendered}\n")).expect("write moderation JSON");
    }
    #[test]
    fn moderation_runner_status_endpoint_reports_locked_manifest() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_runner_fixture_service(manifest.clone());
        let request =
            moderation_runner_http_request("GET", "/v1/sorafs/moderation/runner/status", &[]);
        let response = moderation_runner_http_response(&service, &request, service.max_body_bytes);
        let (header, body) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_json_str!(body, "schema", "sorafs.moderation.runner.status.v1");
        assert_json_str!(
            body,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_json_str!(
            body,
            "outbound_network",
            "model_engine_none_process_policy_required"
        );
    }
    #[test]
    fn moderation_runner_screen_endpoint_matches_local_runner() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_runner_fixture_service(manifest.clone());
        let payload = b"runner service moderation payload";
        let body = norito::json::to_vec(&norito::json!({
            "subject": "cid:bafy-runner-service",
            "payload_b64": (BASE64_STANDARD.encode(payload)),
            "screened_at_unix": 1_800_002_000_u64,
            "notes": "service fixture"
        }))
        .expect("screen request JSON");
        let request =
            moderation_runner_http_request("POST", "/v1/sorafs/moderation/runner/screen", &body);
        let response = moderation_runner_http_response(&service, &request, service.max_body_bytes);
        let (header, actual) = moderation_runner_response_parts(&response);
        let expected = moderation_local_runner_screening_json(
            &service.runner,
            payload,
            "cid:bafy-runner-service",
            1_800_002_000,
            Some("service fixture"),
            service.max_payload_bytes,
        )
        .expect("expected runner output");
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(actual, expected);
        assert_json_str!(
            actual,
            "subject_digest_hex",
            hex_encode(blake3_hash(payload).as_bytes()).as_str()
        );
    }
    #[test]
    fn moderation_runner_grpc_status_reports_locked_manifest() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_runner_fixture_service(manifest.clone());
        let response = moderation_runner_status_proto(&service, "ready", Some("127.0.0.1:9199"));
        assert_eq!(response.schema, "sorafs.moderation.runner.status.v1");
        assert_eq!(response.status, "ready");
        assert_eq!(
            response.manifest_id_hex,
            hex_encode(manifest.body.manifest_id)
        );
        assert_eq!(
            response.runner_hash_hex,
            hex_encode(manifest.body.runner_hash)
        );
        assert_eq!(
            response.outbound_network,
            "model_engine_none_process_policy_required"
        );
        assert_eq!(response.listen, "127.0.0.1:9199");
        assert_eq!(
            response.max_grpc_in_flight,
            u64::try_from(MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT).expect("limit fits u64")
        );
        assert_eq!(
            response.max_grpc_response_bytes,
            u64::try_from(MODERATION_RUNNER_MAX_GRPC_RESPONSE_BYTES).expect("limit fits u64")
        );
    }
    #[test]
    fn moderation_runner_grpc_screen_matches_local_runner() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_runner_fixture_service(manifest.clone());
        let payload = b"runner grpc moderation payload".to_vec();
        let response = moderation_runner_screen_request_proto(
            &service,
            ModerationRunnerScreenRequest {
                subject: "cid:bafy-runner-grpc".to_string(),
                payload: payload.clone(),
                screened_at_unix: 1_800_002_100,
                notes: Some("grpc fixture".to_string()),
            },
        )
        .expect("gRPC screen succeeds");
        let expected = moderation_runner_screen_proto_from_json(
            &moderation_local_runner_screening_json(
                &service.runner,
                &payload,
                "cid:bafy-runner-grpc",
                1_800_002_100,
                Some("grpc fixture"),
                service.max_payload_bytes,
            )
            .expect("local runner output"),
        )
        .expect("expected proto output");
        assert_eq!(response.subject, expected.subject);
        assert_eq!(response.subject_digest_hex, expected.subject_digest_hex);
        assert_eq!(response.manifest_id_hex, expected.manifest_id_hex);
        assert_eq!(response.runner_hash_hex, expected.runner_hash_hex);
        assert_eq!(response.combined_score_bps, expected.combined_score_bps);
        assert_eq!(response.verdict, expected.verdict);
        assert_eq!(response.screened_at_unix, expected.screened_at_unix);
        assert_eq!(response.evidence_digest_hex, expected.evidence_digest_hex);
        assert_eq!(response.policy_digest_hex, expected.policy_digest_hex);
        assert_eq!(response.notes, expected.notes);
        assert_eq!(response.model_scores, expected.model_scores);
        assert_eq!(
            response.subject_digest_hex,
            hex_encode(blake3_hash(&payload).as_bytes())
        );
        assert_eq!(response.notes.as_deref(), Some("grpc fixture"));
    }
    #[test]
    fn moderation_runner_grpc_screen_rejects_payload_over_limit() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let mut service = moderation_runner_fixture_service(manifest);
        service.max_body_bytes = 4;
        let error = match moderation_runner_screen_request_proto(
            &service,
            ModerationRunnerScreenRequest {
                subject: "cid:bafy-runner-grpc".to_string(),
                payload: b"oversized".to_vec(),
                screened_at_unix: 1_800_002_101,
                notes: None,
            },
        ) {
            Ok(_) => panic!("oversized payload should fail"),
            Err(error) => error,
        };
        assert!(error.contains("payload exceeds configured maximum"));
    }
    #[test]
    fn moderation_runner_grpc_maximum_text_response_fits_the_transport_cap() {
        let service = moderation_runner_fixture_service(signed_moderation_repro_manifest_fixture());
        let response = moderation_runner_screen_request_proto(
            &service,
            ModerationRunnerScreenRequest {
                subject: "s".repeat(MODERATION_RUNNER_MAX_SUBJECT_BYTES),
                payload: b"payload".to_vec(),
                screened_at_unix: 1,
                notes: Some("n".repeat(MODERATION_RUNNER_MAX_NOTES_BYTES)),
            },
        )
        .expect("maximum bounded text response");
        assert!(
            <ModerationRunnerScreenResponse as prost::Message>::encoded_len(&response)
                < MODERATION_RUNNER_MAX_GRPC_RESPONSE_BYTES
        );
    }
    #[test]
    fn moderation_runner_screen_requires_explicit_timestamp() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_runner_fixture_service(manifest);
        let body = norito::json::to_vec(&norito::json!({
            "subject": "cid:bafy-runner-service",
            "payload_b64": (BASE64_STANDARD.encode(b"payload")),
        }))
        .expect("screen request JSON");
        let request =
            moderation_runner_http_request("POST", "/v1/sorafs/moderation/runner/screen", &body);
        let response = moderation_runner_http_response(&service, &request, service.max_body_bytes);
        let (header, body) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(
            body.get("message")
                .and_then(Value::as_str)
                .expect("error message")
                .contains("screened_at_unix")
        );
    }
    #[test]
    fn moderation_runner_rejects_body_over_limit() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_runner_fixture_service(manifest);
        let request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/runner/screen",
            b"0123456789",
        );
        let response = moderation_runner_http_response(&service, &request, 4);
        let (header, body) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 413 Payload Too Large"));
        assert_eq!(body.get("status").and_then(Value::as_str), Some("error"));
    }
    #[test]
    fn moderation_http_limits_are_exact_bounded_and_overflow_safe() {
        assert_eq!(
            moderation_http_hard_limit(4, "test").expect("small hard limit"),
            4 + MODERATION_RUNNER_MAX_HEADER_BYTES + 4
        );
        assert!(moderation_http_hard_limit(usize::MAX, "test").is_err());
        assert_eq!(
            parse_moderation_max_body_bytes(
                &MODERATION_RUNNER_HARD_MAX_BODY_BYTES.to_string(),
                "test",
            )
            .expect("exact hard maximum"),
            MODERATION_RUNNER_HARD_MAX_BODY_BYTES
        );
        assert!(parse_moderation_max_body_bytes("0", "test").is_err());
        assert!(
            parse_moderation_max_body_bytes(
                &(MODERATION_RUNNER_HARD_MAX_BODY_BYTES + 1).to_string(),
                "test",
            )
            .is_err()
        );
    }
    #[test]
    fn moderation_runner_http_parser_rejects_smuggling_and_malformed_frames() {
        let service = moderation_runner_fixture_service(signed_moderation_repro_manifest_fixture());
        let malformed = [
            b"GET /healthz HTTP/1.1\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nHost: second.local\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: \r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nContent-Length: 00\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nContent-Length: +0\r\n\r\n".as_slice(),
            b"POST /v1/sorafs/moderation/runner/screen HTTP/1.1\r\nHost: runner.local\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nMalformed\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\n folded: value\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1 extra\r\nHost: runner.local\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.0\r\nHost: runner.local\r\n\r\n".as_slice(),
            b"GE\tT /healthz HTTP/1.1\r\nHost: runner.local\r\n\r\n".as_slice(),
            b"GET /healthz?ignored=true HTTP/1.1\r\nHost: runner.local\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nContent-Length: 0\r\n\r\nX".as_slice(),
            b"GET /healthz HTTP/1.1\r\nBad Header: value\r\nHost: runner.local\r\n\r\n".as_slice(),
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nX-Null: bad\0value\r\n\r\n".as_slice(),
        ];
        for request in malformed {
            let response = moderation_runner_http_response(&service, request, 4096);
            assert!(
                response.starts_with(b"HTTP/1.1 400 Bad Request"),
                "malformed request was not rejected: {}",
                String::from_utf8_lossy(request)
            );
        }
        let mut oversized_header =
            b"GET /healthz HTTP/1.1\r\nHost: runner.local\r\nX-Fill: ".to_vec();
        oversized_header.extend(vec![b'a'; MODERATION_RUNNER_MAX_HEADER_BYTES]);
        oversized_header.extend_from_slice(b"\r\n\r\n");
        let response = moderation_runner_http_response(&service, &oversized_header, 4096);
        assert!(response.starts_with(b"HTTP/1.1 431 Request Header Fields Too Large"));
    }
    #[test]
    fn moderation_runner_rejects_noncanonical_request_text() {
        let service = moderation_runner_fixture_service(signed_moderation_repro_manifest_fixture());
        for (subject, notes) in [
            (" padded", Some("notes")),
            ("subject", Some("notes\nwith-control")),
        ] {
            let body = norito::json::to_vec(&norito::json!({
                "subject": subject,
                "payload_b64": (BASE64_STANDARD.encode(b"payload")),
                "screened_at_unix": 1_u64,
                "notes": notes,
            }))
            .expect("request JSON");
            assert!(moderation_runner_screen_request_json(&service, &body).is_err());
        }
        let oversized_subject = "x".repeat(MODERATION_RUNNER_MAX_SUBJECT_BYTES + 1);
        let body = norito::json::to_vec(&norito::json!({
            "subject": oversized_subject,
            "payload_b64": (BASE64_STANDARD.encode(b"payload")),
            "screened_at_unix": 1_u64,
        }))
        .expect("request JSON");
        assert!(moderation_runner_screen_request_json(&service, &body).is_err());
        let body = norito::json::to_vec(&norito::json!({
            "subject": "subject",
            "payload_b64": (BASE64_STANDARD.encode(b"payload")),
            "screened_at_unix": 1_u64,
            "unexpected": true,
        }))
        .expect("request JSON");
        assert!(moderation_runner_screen_request_json(&service, &body).is_err());
        let body = norito::json::to_vec(&norito::json!({
            "subject": "subject",
            "payload_b64": ("A".repeat(8192)),
            "screened_at_unix": 1_u64,
        }))
        .expect("request JSON");
        let error = moderation_runner_screen_request_json(&service, &body)
            .expect_err("oversized decoded payload estimate must fail before allocation");
        assert!(error.contains("can decode beyond"));
        let body = norito::json::to_vec(&norito::json!({
            "subject": "subject",
            "payload_b64": (BASE64_STANDARD.encode(b"payload")),
            "screened_at_unix": 0_u64,
        }))
        .expect("request JSON");
        assert!(moderation_runner_screen_request_json(&service, &body).is_err());
        let body = norito::json::to_vec(&norito::json!({
            "subject": "subject",
            "payload_b64": (BASE64_STANDARD.encode(b"payload")),
            "screened_at_unix": 1_u64,
            "notes": ("x".repeat(MODERATION_RUNNER_MAX_NOTES_BYTES + 1)),
        }))
        .expect("request JSON");
        assert!(moderation_runner_screen_request_json(&service, &body).is_err());
        assert!(
            moderation_runner_screen_request_proto(
                &service,
                ModerationRunnerScreenRequest {
                    subject: " padded".to_owned(),
                    payload: b"payload".to_vec(),
                    screened_at_unix: 1,
                    notes: None,
                },
            )
            .is_err()
        );
    }
    #[test]
    fn moderation_runner_connection_admission_is_bounded() {
        let active = Arc::new(AtomicUsize::new(0));
        let mut permits = Vec::new();
        for _ in 0..MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS {
            permits.push(
                moderation_try_acquire_permit(&active, MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS)
                    .expect("connection within the hard limit must be admitted"),
            );
        }
        assert!(
            moderation_try_acquire_permit(&active, MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS)
                .is_none()
        );
        assert_eq!(
            active.load(AtomicOrdering::Acquire),
            MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS
        );
        permits.pop();
        assert_eq!(
            active.load(AtomicOrdering::Acquire),
            MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS - 1
        );
        let replacement =
            moderation_try_acquire_permit(&active, MODERATION_RUNNER_MAX_ACTIVE_CONNECTIONS)
                .expect("dropping a permit must release capacity");
        drop(replacement);
        drop(permits);
        assert_eq!(active.load(AtomicOrdering::Acquire), 0);
    }
    #[test]
    fn moderation_runner_grpc_rejects_work_above_the_in_flight_limit() {
        let in_flight = Arc::new(AtomicUsize::new(MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT));
        let handler = ModerationRunnerGrpcHandler {
            service: Arc::new(moderation_runner_fixture_service(
                signed_moderation_repro_manifest_fixture(),
            )),
            listen: MODERATION_RUNNER_GRPC_DEFAULT_LISTEN.to_owned(),
            in_flight: Arc::clone(&in_flight),
        };
        let request = tonic::Request::new(ModerationRunnerScreenRequest {
            subject: "cid:bafy-overload".to_owned(),
            payload: b"must not be evaluated".to_vec(),
            screened_at_unix: 1,
            notes: None,
        });
        let result = Runtime::new().expect("Tokio runtime").block_on(
            moderation_runner_grpc::runner_server::Runner::screen(&handler, request),
        );
        let error = match result {
            Ok(_) => panic!("saturated gRPC runner must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.code(), tonic::Code::ResourceExhausted);
        assert_eq!(
            in_flight.load(AtomicOrdering::Acquire),
            MODERATION_RUNNER_MAX_GRPC_IN_FLIGHT,
            "a rejected call must not perturb the active-work counter"
        );
    }
    #[test]
    fn moderation_runner_bundle_emits_supervised_artifacts() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("repro.json");
        let bundle_dir = temp.path().join("runner-bundle");
        let artifact_root = temp.path().join("source-artifacts");
        write_moderation_model_artifacts_fixture(&artifact_root);
        write_manifest_fixture!(manifest_path, manifest);
        moderation_runner_bundle(vec![
            format!("--manifest={}", manifest_path.display()),
            format!("--artifact-root={}", artifact_root.display()),
            format!("--bundle-out={}", bundle_dir.display()),
            "--listen=127.0.0.1:9195".to_string(),
            "--max-body-bytes=8192".to_string(),
            "--binary=/opt/sora/bin/sorafs_cli".to_string(),
            "--service-name=org.sora.sorafs.runner-test".to_string(),
            "--service-user=sorafs-runner".to_string(),
            "--service-group=sorafs-runner".to_string(),
        ])
        .expect("runner bundle succeeds");
        let manifest_copy = bundle_dir.join("manifest.json");
        let env = fs::read_to_string(bundle_dir.join("runner.env")).expect("runner env");
        let run_script = fs::read_to_string(bundle_dir.join("run.sh")).expect("run script");
        let systemd = fs::read_to_string(bundle_dir.join("org.sora.sorafs.runner-test.service"))
            .expect("systemd unit");
        let launchd = fs::read_to_string(bundle_dir.join("org.sora.sorafs.runner-test.plist"))
            .expect("launchd plist");
        let readme = fs::read_to_string(bundle_dir.join("README.md")).expect("readme");
        let metadata: Value = norito::json::from_str(
            &fs::read_to_string(bundle_dir.join("bundle.json")).expect("bundle metadata"),
        )
        .expect("parse bundle metadata");
        assert!(manifest_copy.exists());
        assert!(bundle_dir.join("artifacts/model-11.norito").exists());
        assert!(bundle_dir.join("artifacts/model-44.norito").exists());
        assert!(env.contains("SORAFS_CLI='/opt/sora/bin/sorafs_cli'"));
        assert!(env.contains("SORAFS_RUNNER_LISTEN='127.0.0.1:9195'"));
        assert!(run_script.contains("moderation runner-serve"));
        assert!(run_script.contains("--manifest=\"$SCRIPT_DIR/manifest.json\""));
        assert!(run_script.contains("--format=json"));
        assert!(systemd.contains("NoNewPrivileges=true"));
        assert!(systemd.contains("IPAddressDeny=any"));
        assert!(systemd.contains("IPAddressAllow=localhost"));
        assert!(systemd.contains("EnvironmentFile="));
        assert!(systemd.contains("User=sorafs-runner"));
        assert!(launchd.contains("<key>KeepAlive</key>"));
        assert!(readme.contains("SoraFS Moderation Runner Bundle"));
        let object = metadata.as_object().expect("metadata object");
        assert_json_str!(object, "schema", "sorafs.moderation.runner.bundle.v1");
        assert_json_str!(
            object,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_json_str!(object, "listen", "127.0.0.1:9195");
        let files = match object.get("files") {
            Some(Value::Array(values)) => values,
            other => panic!("expected files array, got {other:?}"),
        };
        assert!(
            files
                .iter()
                .any(|value| value.as_str() == Some("manifest.json"))
        );
        assert!(files.iter().any(|value| value.as_str() == Some("run.sh")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(bundle_dir.join("run.sh"))
                .expect("run script metadata")
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "run.sh should be executable");
        }
    }
    #[test]
    fn moderation_runner_bundle_rejects_invalid_service_name() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("repro.json");
        write_manifest_fixture!(manifest_path, manifest);
        let err = moderation_runner_bundle(vec![
            format!("--manifest={}", manifest_path.display()),
            format!(
                "--bundle-out={}",
                temp.path().join("runner-bundle").display()
            ),
            "--service-name=bad/name".to_string(),
        ])
        .expect_err("invalid service name rejected");
        assert!(err.contains("--service-name"), "unexpected error: {err}");
    }
    #[test]
    fn moderation_runner_canary_emits_payload_free_rollout_evidence() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let root = canonical_temp_path(&temp);
        let manifest_path = root.join("repro.json");
        let payload_path = root.join("payload.bin");
        let out_path = root.join("runner-canary.json");
        let payload = b"runner canary payload bytes";
        write_manifest_fixture!(manifest_path, manifest);
        fs::write(&payload_path, payload).expect("write payload");
        let (runner_url, handle) = moderation_runner_canary_fixture_server(manifest.clone());
        moderation_runner_canary(vec![
            format!("--manifest={}", manifest_path.display()),
            format!("--runner-url={runner_url}"),
            format!("--payload={}", payload_path.display()),
            "--subject=cid:bafy-runner-canary".to_string(),
            "--screened-at=1800004000".to_string(),
            "--generated-at-unix=1800004999".to_string(),
            "--deployment-id=ai-prescreen-production-20260701".to_string(),
            "--environment=production".to_string(),
            "--deployment-context-reviewed=true".to_string(),
            "--process-isolation-enforcement=systemd_ip_filter".to_string(),
            "--process-isolation-attestation-digest=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_string(),
            "--process-isolation-verified-at=1800004998".to_string(),
            "--process-isolation-reviewed=true".to_string(),
            "--checked-at=1800004999".to_string(),
            "--notes=fixture rollout canary".to_string(),
            "--timeout-ms=5000".to_string(),
            format!("--json-out={}", out_path.display()),
        ])
        .expect("runner canary succeeds");
        handle.join().expect("runner canary fixture exits");
        let rendered = fs::read_to_string(&out_path).expect("read canary output");
        assert!(!rendered.contains("payload_b64"));
        let value: Value = norito::json::from_str(&rendered).expect("parse canary output");
        let object = value.as_object().expect("canary output object");
        assert_json_str!(
            object,
            "schema",
            "sorafs.moderation.runner.rollout_evidence.v1"
        );
        assert_json_str!(object, "status", "verified");
        assert_json_str!(
            object,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_json_str!(
            object,
            "runner_hash_hex",
            hex_encode(manifest.body.runner_hash).as_str()
        );
        assert_json_str!(
            object,
            "subject_digest_hex",
            hex_encode(blake3_hash(payload).as_bytes()).as_str()
        );
        assert_json_u64!(object, "checked_at_unix", 1_800_004_999);
        assert_json_u64!(object, "generated_at_unix", 1_800_004_999);
        assert_json_str!(object, "deployment_id", "ai-prescreen-production-20260701");
        assert_json_str!(object, "environment", "production");
        assert_json_bool!(object, "synthetic", false);
        let isolation = object
            .get("process_isolation_evidence")
            .and_then(Value::as_object)
            .expect("runner isolation evidence");
        assert_json_str!(isolation, "status", "runtime_verified");
        assert_json_str!(isolation, "enforcement", "systemd_ip_filter");
        assert_json_bool!(isolation, "reviewed", true);
        assert_json_bool!(isolation, "synthetic", false);
        assert_eq!(object.get("probe_count").and_then(Value::as_u64), Some(2));
        let probes = object
            .get("probes")
            .and_then(Value::as_array)
            .expect("runner canary probes");
        assert_eq!(probes.len(), 2);
        assert_eq!(
            probes[0].get("name").and_then(Value::as_str),
            Some("status")
        );
        assert_eq!(
            probes[1].get("name").and_then(Value::as_str),
            Some("screen")
        );
        assert!(object.get("runner_status").is_some());
        assert!(object.get("screening_result").is_some());
    }
    fn runner_canary_args_without_process_isolation() -> Vec<String> {
        vec![
            "--manifest=/nonexistent/repro.json".to_owned(),
            "--runner-url=http://127.0.0.1:9194".to_owned(),
            "--payload=/nonexistent/payload.bin".to_owned(),
            "--subject=cid:bafy-isolation-negative".to_owned(),
            "--screened-at=99".to_owned(),
            "--generated-at-unix=100".to_owned(),
            "--deployment-id=isolation-negative".to_owned(),
            "--environment=production".to_owned(),
            "--deployment-context-reviewed=true".to_owned(),
        ]
    }
    #[test]
    fn moderation_runner_canary_rejects_missing_or_forged_isolation_evidence() {
        let error = moderation_runner_canary(runner_canary_args_without_process_isolation())
            .expect_err("missing isolation enforcement must fail before I/O");
        assert!(error.contains("--process-isolation-enforcement"));
        let error = moderation_runner_canary(vec![
            "--process-isolation-enforcement=application_claim".to_owned(),
        ])
        .expect_err("unsupported isolation enforcement must fail");
        assert!(error.contains("must be one of"));
        let error = moderation_runner_canary(vec![
            "--process-isolation-attestation-digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        ])
        .expect_err("repeated placeholder attestation must fail");
        assert!(error.contains("placeholder"));
        let mut args = runner_canary_args_without_process_isolation();
        args.extend([
            "--process-isolation-enforcement=systemd_ip_filter".to_owned(),
            "--process-isolation-attestation-digest=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_owned(),
            "--process-isolation-verified-at=101".to_owned(),
            "--process-isolation-reviewed=true".to_owned(),
        ]);
        let error = moderation_runner_canary(args)
            .expect_err("future-dated isolation attestation must fail before I/O");
        assert!(error.contains("must not be after"));
    }
    #[test]
    fn moderation_canary_response_reader_enforces_declared_and_streamed_caps() {
        fn response_with_body(
            content_length: Option<u64>,
            body_len: usize,
        ) -> (reqwest::blocking::Response, thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
            let address = listener.local_addr().expect("fixture address");
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept fixture request");
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).expect("read fixture request");
                let length_header = content_length
                    .map(|length| format!("Content-Length: {length}\r\n"))
                    .unwrap_or_default();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\n{length_header}Connection: close\r\n\r\n"
                )
                .expect("write fixture headers");
                if body_len > 0 {
                    stream
                        .write_all(&vec![b'x'; body_len])
                        .expect("write fixture body");
                }
            });
            let response = HttpClient::new()
                .get(format!("http://{address}/"))
                .send()
                .expect("fixture response");
            (response, handle)
        }
        let (response, handle) =
            response_with_body(Some(MODERATION_CANARY_MAX_RESPONSE_BYTES + 1), 0);
        let error = read_moderation_canary_response_bounded(response, "declared oversized")
            .expect_err("oversized Content-Length must fail before body allocation");
        handle.join().expect("fixture server exits");
        assert!(error.contains("declared a response larger"));
        let streamed_len = usize::try_from(MODERATION_CANARY_MAX_RESPONSE_BYTES + 1)
            .expect("fixture cap fits usize");
        let (response, handle) = response_with_body(None, streamed_len);
        let error = read_moderation_canary_response_bounded(response, "streamed oversized")
            .expect_err("connection-close response above the cap must fail");
        handle.join().expect("fixture server exits");
        assert!(error.contains("response exceeded"));
    }
    #[test]
    fn moderation_runner_canary_rejects_manifest_mismatch() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let mut expected_manifest = manifest.clone();
        expected_manifest.body.runner_hash = [0xDD; 32];
        let payload = b"runner canary mismatch payload";
        let subject = "cid:bafy-runner-canary";
        let service = moderation_runner_fixture_service(manifest.clone());
        let status_response = moderation_runner_status_json(&service, "ready", None);
        let screen_request =
            moderation_runner_canary_screen_request_json(payload, subject, 1_800_004_000, None);
        let screen_request_body = to_vec(&screen_request).expect("screen request JSON");
        let screening_response =
            moderation_runner_screen_request_json(&service, &screen_request_body)
                .expect("screening response");
        let status_probe = moderation_canary_test_probe(
            "GET",
            "http://127.0.0.1:9194/v1/sorafs/moderation/runner/status",
            status_response,
        );
        let screening_probe = moderation_canary_test_probe(
            "POST",
            "http://127.0.0.1:9194/v1/sorafs/moderation/runner/screen",
            screening_response,
        );
        let err = moderation_runner_canary_evidence_json(ModerationRunnerCanaryEvidenceInput {
            manifest: &expected_manifest,
            runner_url: "http://127.0.0.1:9194",
            status_url: "http://127.0.0.1:9194/v1/sorafs/moderation/runner/status",
            screen_url: "http://127.0.0.1:9194/v1/sorafs/moderation/runner/screen",
            subject,
            payload,
            screened_at_unix: 1_800_004_000,
            checked_at_unix: 1_800_004_999,
            deployment_context: moderation_canary_test_context(1_800_004_999),
            process_isolation: ModerationProcessIsolationEvidence {
                enforcement: "systemd_ip_filter",
                attestation_digest: core::array::from_fn(|index| {
                    u8::try_from(index).expect("fixture digest index fits u8")
                }),
                verified_at_unix: 1_800_004_998,
            },
            notes: None,
            status_probe,
            screening_probe,
        })
        .expect_err("runner hash mismatch rejected");
        assert!(err.contains("runner hash"), "unexpected error: {err}");
    }
    #[test]
    fn moderation_committee_status_endpoint_reports_locked_manifest() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_committee_fixture_service(manifest.clone(), 2);
        let request =
            moderation_runner_http_request("GET", "/v1/sorafs/moderation/committee/status", &[]);
        let response =
            moderation_committee_http_response(&service, &request, service.max_body_bytes);
        let (header, body) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_json_str!(body, "schema", "sorafs.moderation.committee.status.v1");
        assert_json_str!(
            body,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_eq!(body.get("quorum").and_then(Value::as_u64), Some(2));
        assert_json_str!(body, "aggregation", "median_score_bps");
        assert_json_str!(
            body,
            "outbound_network",
            "network_capable_process_policy_required"
        );
    }
    #[test]
    fn moderation_committee_aggregate_endpoint_matches_local_aggregation() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_committee_fixture_service(manifest.clone(), 2);
        let payload = b"committee service payload bytes";
        let subject = "cid:bafy-committee-service";
        let result_a =
            moderation_committee_result_fixture(&manifest, payload, subject, 5_900, 1_800_005_001);
        let result_b =
            moderation_committee_result_fixture(&manifest, payload, subject, 6_100, 1_800_005_002);
        let result_c =
            moderation_committee_result_fixture(&manifest, payload, subject, 8_700, 1_800_005_003);
        let mut body = Map::new();
        insert_json!(
            body["results"] =
                Value::Array(vec![result_b.clone(), result_a.clone(), result_c.clone()])
        );
        insert_value!(body["notes"] = "service aggregate");
        let body = to_vec(&Value::Object(body)).expect("committee request JSON");
        let request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/committee/aggregate",
            &body,
        );
        let response =
            moderation_committee_http_response(&service, &request, service.max_body_bytes);
        let (header, actual) = moderation_runner_response_parts(&response);
        let expected_inputs = vec![
            parse_moderation_committee_input_value("request.results[0]", &result_b, &manifest)
                .expect("result b"),
            parse_moderation_committee_input_value("request.results[1]", &result_a, &manifest)
                .expect("result a"),
            parse_moderation_committee_input_value("request.results[2]", &result_c, &manifest)
                .expect("result c"),
        ];
        let expected = moderation_committee_aggregate_json(
            &manifest,
            &expected_inputs,
            2,
            Some("service aggregate"),
        )
        .expect("expected aggregate");
        assert!(header.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(actual, expected);
        assert_json_u64!(actual, "aggregated_score_bps", 6_100);
        assert_json_str!(actual, "verdict", "quarantine");
    }
    #[test]
    fn moderation_committee_aggregate_endpoint_rejects_payload_bytes() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_committee_fixture_service(manifest.clone(), 1);
        let result = moderation_committee_result_fixture(
            &manifest,
            b"committee service payload bytes",
            "cid:bafy-committee-service",
            6_100,
            1_800_005_010,
        );
        let mut body = Map::new();
        insert_json!(body["results"] = Value::Array(vec![result]));
        insert_value!(body["payload_b64"] = BASE64_STANDARD.encode(b"payload"));
        let body = to_vec(&Value::Object(body)).expect("committee request JSON");
        let request = moderation_runner_http_request(
            "POST",
            "/v1/sorafs/moderation/committee/aggregate",
            &body,
        );
        let response =
            moderation_committee_http_response(&service, &request, service.max_body_bytes);
        let (header, body) = moderation_runner_response_parts(&response);
        assert!(header.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(
            body.get("message")
                .and_then(Value::as_str)
                .expect("error message")
                .contains("payload-free")
        );
    }
    #[test]
    fn moderation_committee_bundle_emits_supervised_artifacts() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("repro.json");
        let bundle_dir = temp.path().join("committee-bundle");
        write_manifest_fixture!(manifest_path, manifest);
        moderation_committee_bundle(vec![
            format!("--manifest={}", manifest_path.display()),
            "--quorum=2".to_string(),
            format!("--bundle-out={}", bundle_dir.display()),
            "--listen=127.0.0.1:9197".to_string(),
            "--max-body-bytes=8192".to_string(),
            "--binary=/opt/sora/bin/sorafs_cli".to_string(),
            "--service-name=org.sora.sorafs.committee-test".to_string(),
            "--service-user=sorafs-committee".to_string(),
            "--service-group=sorafs-committee".to_string(),
        ])
        .expect("committee bundle succeeds");
        let manifest_copy = bundle_dir.join("manifest.json");
        let env = fs::read_to_string(bundle_dir.join("committee.env")).expect("committee env");
        let run_script = fs::read_to_string(bundle_dir.join("run.sh")).expect("run script");
        let systemd = fs::read_to_string(bundle_dir.join("org.sora.sorafs.committee-test.service"))
            .expect("systemd unit");
        let launchd = fs::read_to_string(bundle_dir.join("org.sora.sorafs.committee-test.plist"))
            .expect("launchd plist");
        let readme = fs::read_to_string(bundle_dir.join("README.md")).expect("readme");
        let metadata: Value = norito::json::from_str(
            &fs::read_to_string(bundle_dir.join("bundle.json")).expect("bundle metadata"),
        )
        .expect("parse bundle metadata");
        assert!(manifest_copy.exists());
        assert!(env.contains("SORAFS_CLI='/opt/sora/bin/sorafs_cli'"));
        assert!(env.contains("SORAFS_COMMITTEE_LISTEN='127.0.0.1:9197'"));
        assert!(env.contains("SORAFS_COMMITTEE_QUORUM='2'"));
        assert!(run_script.contains("moderation committee-serve"));
        assert!(run_script.contains("--manifest=\"$SCRIPT_DIR/manifest.json\""));
        assert!(run_script.contains("--quorum=\"$SORAFS_COMMITTEE_QUORUM\""));
        assert!(systemd.contains("NoNewPrivileges=true"));
        assert!(systemd.contains("IPAddressDeny=any"));
        assert!(systemd.contains("IPAddressAllow=localhost"));
        assert!(systemd.contains("EnvironmentFile="));
        assert!(systemd.contains("User=sorafs-committee"));
        assert!(launchd.contains("<key>KeepAlive</key>"));
        assert!(readme.contains("SoraFS Moderation Committee Bundle"));
        let object = metadata.as_object().expect("metadata object");
        assert_json_str!(object, "schema", "sorafs.moderation.committee.bundle.v1");
        assert_json_str!(
            object,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_eq!(object.get("quorum").and_then(Value::as_u64), Some(2));
        assert_json_str!(object, "aggregation", "median_score_bps");
        assert_json_str!(object, "listen", "127.0.0.1:9197");
        let files = match object.get("files") {
            Some(Value::Array(values)) => values,
            other => panic!("expected files array, got {other:?}"),
        };
        assert!(
            files
                .iter()
                .any(|value| value.as_str() == Some("manifest.json"))
        );
        assert!(
            files
                .iter()
                .any(|value| value.as_str() == Some("committee.env"))
        );
        assert!(files.iter().any(|value| value.as_str() == Some("run.sh")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(bundle_dir.join("run.sh"))
                .expect("run script metadata")
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "run.sh should be executable");
        }
    }
    #[test]
    fn moderation_committee_canary_emits_payload_free_rollout_evidence() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let root = canonical_temp_path(&temp);
        let manifest_path = root.join("repro.json");
        let out_path = root.join("committee-canary.json");
        let result_a = root.join("a.json");
        let result_b = root.join("b.json");
        let result_c = root.join("c.json");
        let payload = b"committee canary payload bytes";
        let subject = "cid:bafy-committee-canary";
        write_manifest_fixture!(manifest_path, manifest);
        write_moderation_json(
            &result_a,
            &moderation_committee_result_fixture(&manifest, payload, subject, 5_900, 1_800_006_001),
        );
        write_moderation_json(
            &result_b,
            &moderation_committee_result_fixture(&manifest, payload, subject, 6_100, 1_800_006_002),
        );
        write_moderation_json(
            &result_c,
            &moderation_committee_result_fixture(&manifest, payload, subject, 8_700, 1_800_006_003),
        );
        let (committee_url, handle) =
            moderation_committee_canary_fixture_server(manifest.clone(), 2);
        moderation_committee_canary(vec![
            format!("--manifest={}", manifest_path.display()),
            format!("--committee-url={committee_url}"),
            "--quorum=2".to_string(),
            format!("--result={}", result_b.display()),
            format!("--result={}", result_a.display()),
            format!("--result={}", result_c.display()),
            "--generated-at-unix=1800006999".to_string(),
            "--deployment-id=ai-prescreen-production-20260701".to_string(),
            "--environment=production".to_string(),
            "--deployment-context-reviewed=true".to_string(),
            "--process-isolation-enforcement=systemd_ip_filter".to_string(),
            "--process-isolation-attestation-digest=202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f".to_string(),
            "--process-isolation-verified-at=1800006998".to_string(),
            "--process-isolation-reviewed=true".to_string(),
            "--checked-at=1800006999".to_string(),
            "--notes=fixture committee rollout canary".to_string(),
            "--timeout-ms=5000".to_string(),
            format!("--json-out={}", out_path.display()),
        ])
        .expect("committee canary succeeds");
        handle.join().expect("committee canary fixture exits");
        let rendered = fs::read_to_string(&out_path).expect("read canary output");
        assert!(!rendered.contains("payload_b64"));
        let value: Value = norito::json::from_str(&rendered).expect("parse canary output");
        let object = value.as_object().expect("canary output object");
        assert_json_str!(
            object,
            "schema",
            "sorafs.moderation.committee.rollout_evidence.v1"
        );
        assert_json_str!(object, "status", "verified");
        assert_json_str!(
            object,
            "manifest_id_hex",
            hex_encode(manifest.body.manifest_id).as_str()
        );
        assert_json_str!(
            object,
            "runner_hash_hex",
            hex_encode(manifest.body.runner_hash).as_str()
        );
        assert_eq!(object.get("quorum").and_then(Value::as_u64), Some(2));
        assert_eq!(object.get("result_count").and_then(Value::as_u64), Some(3));
        let results = object
            .get("results")
            .and_then(Value::as_array)
            .expect("results array");
        assert_eq!(results.len(), 3);
        let first_result = results
            .first()
            .and_then(Value::as_object)
            .expect("first result fingerprint");
        assert!(
            first_result
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.starts_with("ai-prescreen-committee-result-"))
        );
        assert!(
            first_result
                .get("body_blake3_hex")
                .and_then(Value::as_str)
                .is_some_and(|digest| digest.len() == 64)
        );
        assert_json_str!(
            object,
            "subject_digest_hex",
            hex_encode(blake3_hash(payload).as_bytes()).as_str()
        );
        assert_json_u64!(object, "aggregated_score_bps", 6_100);
        assert_json_str!(object, "verdict", "quarantine");
        assert_json_u64!(object, "checked_at_unix", 1_800_006_999);
        assert_json_u64!(object, "generated_at_unix", 1_800_006_999);
        assert_json_bool!(object, "synthetic", false);
        let isolation = object
            .get("process_isolation_evidence")
            .and_then(Value::as_object)
            .expect("committee isolation evidence");
        assert_json_str!(isolation, "status", "runtime_verified");
        assert_json_str!(isolation, "enforcement", "systemd_ip_filter");
        assert_json_bool!(isolation, "reviewed", true);
        assert_json_bool!(isolation, "synthetic", false);
        assert_eq!(object.get("probe_count").and_then(Value::as_u64), Some(2));
        assert!(object.get("committee_status").is_some());
        assert!(object.get("committee_aggregate").is_some());
    }
    #[test]
    fn moderation_committee_rejects_excessive_result_count_and_file_size() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let service = moderation_committee_fixture_service(manifest.clone(), 1);
        let body = norito::json::to_vec(&norito::json!({
            "results": (vec![Value::Null; MODERATION_COMMITTEE_MAX_RESULTS + 1]),
        }))
        .expect("oversized result inventory JSON");
        let error = moderation_committee_aggregate_request_json(&service, &body)
            .expect_err("committee result inventory above the cap must fail");
        assert!(error.contains("accepts at most"));
        let temp = TempDir::new().expect("tempdir");
        let oversized = temp.path().join("oversized-result.json");
        fs::write(
            &oversized,
            vec![
                b'x';
                usize::try_from(MODERATION_COMMITTEE_MAX_RESULT_BYTES + 1)
                    .expect("fixture cap fits usize")
            ],
        )
        .expect("write oversized result");
        let error = load_moderation_committee_input(&oversized, &manifest)
            .expect_err("oversized committee result file must fail before parsing");
        assert!(error.contains("maximum"));
    }
    fn committee_canary_args_without_process_isolation() -> Vec<String> {
        vec![
            "--manifest=/nonexistent/repro.json".to_owned(),
            "--committee-url=http://127.0.0.1:9196".to_owned(),
            "--quorum=1".to_owned(),
            "--result=/nonexistent/result.json".to_owned(),
            "--generated-at-unix=100".to_owned(),
            "--deployment-id=isolation-negative".to_owned(),
            "--environment=production".to_owned(),
            "--deployment-context-reviewed=true".to_owned(),
        ]
    }
    #[test]
    fn moderation_committee_canary_rejects_missing_or_forged_isolation_evidence() {
        let error = moderation_committee_canary(committee_canary_args_without_process_isolation())
            .expect_err("missing isolation enforcement must fail before I/O");
        assert!(error.contains("--process-isolation-enforcement"));
        let error = moderation_committee_canary(vec![
            "--process-isolation-enforcement=application_claim".to_owned(),
        ])
        .expect_err("unsupported isolation enforcement must fail");
        assert!(error.contains("must be one of"));
        let error = moderation_committee_canary(vec![
            "--process-isolation-attestation-digest=abababababababababababababababababababababababababababababababab".to_owned(),
        ])
        .expect_err("repeated-half placeholder attestation must fail");
        assert!(error.contains("placeholder"));
        let mut args = committee_canary_args_without_process_isolation();
        args.extend([
            "--process-isolation-enforcement=host_firewall".to_owned(),
            "--process-isolation-attestation-digest=202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f".to_owned(),
            "--process-isolation-verified-at=101".to_owned(),
            "--process-isolation-reviewed=true".to_owned(),
        ]);
        let error = moderation_committee_canary(args)
            .expect_err("future-dated isolation attestation must fail before I/O");
        assert!(error.contains("must not be after"));
    }
    #[test]
    fn moderation_committee_canary_rejects_manifest_mismatch() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let mut expected_manifest = manifest.clone();
        expected_manifest.body.runner_hash = [0xCC; 32];
        let payload = b"committee canary mismatch payload";
        let subject = "cid:bafy-committee-canary";
        let service = moderation_committee_fixture_service(manifest.clone(), 2);
        let status_response = moderation_committee_status_json(&service, "ready", None);
        let result_a =
            moderation_committee_result_fixture(&manifest, payload, subject, 5_900, 1_800_006_001);
        let result_b =
            moderation_committee_result_fixture(&manifest, payload, subject, 6_100, 1_800_006_002);
        let expected_aggregate = moderation_committee_expected_aggregate_from_values(
            &manifest,
            &[result_a.clone(), result_b.clone()],
            2,
            None,
        )
        .expect("expected aggregate");
        let aggregate_response = expected_aggregate.clone();
        let status_probe = moderation_canary_test_probe(
            "GET",
            "http://127.0.0.1:9196/v1/sorafs/moderation/committee/status",
            status_response,
        );
        let aggregate_probe = moderation_canary_test_probe(
            "POST",
            "http://127.0.0.1:9196/v1/sorafs/moderation/committee/aggregate",
            aggregate_response,
        );
        let err =
            moderation_committee_canary_evidence_json(ModerationCommitteeCanaryEvidenceInput {
                manifest: &expected_manifest,
                committee_url: "http://127.0.0.1:9196",
                status_url: "http://127.0.0.1:9196/v1/sorafs/moderation/committee/status",
                aggregate_url: "http://127.0.0.1:9196/v1/sorafs/moderation/committee/aggregate",
                quorum: 2,
                checked_at_unix: 1_800_006_999,
                deployment_context: moderation_canary_test_context(1_800_006_999),
                process_isolation: ModerationProcessIsolationEvidence {
                    enforcement: "systemd_ip_filter",
                    attestation_digest: core::array::from_fn(|index| {
                        u8::try_from(index + 32).expect("fixture digest index fits u8")
                    }),
                    verified_at_unix: 1_800_006_998,
                },
                notes: None,
                result_fingerprints: vec![
                    moderation_canary_test_result_fingerprint(1),
                    moderation_canary_test_result_fingerprint(2),
                ],
                expected_aggregate,
                status_probe,
                aggregate_probe,
            })
            .expect_err("runner hash mismatch rejected");
        assert!(err.contains("runner hash"), "unexpected error: {err}");
    }
    #[test]
    fn moderation_committee_run_aggregates_runner_results() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let root = canonical_temp_path(&temp);
        let manifest_path = root.join("repro.json");
        let out_path = root.join("committee.json");
        let result_a = root.join("a.json");
        let result_b = root.join("b.json");
        let result_c = root.join("c.json");
        let payload = b"committee payload bytes";
        let subject = "cid:bafy-committee";
        write_manifest_fixture!(manifest_path, manifest);
        write_moderation_json(
            &result_a,
            &moderation_committee_result_fixture(&manifest, payload, subject, 5_900, 1_800_003_001),
        );
        write_moderation_json(
            &result_b,
            &moderation_committee_result_fixture(&manifest, payload, subject, 6_100, 1_800_003_002),
        );
        write_moderation_json(
            &result_c,
            &moderation_committee_result_fixture(&manifest, payload, subject, 8_700, 1_800_003_003),
        );
        moderation_committee_run(vec![
            format!("--manifest={}", manifest_path.display()),
            "--quorum=2".to_string(),
            format!("--result={}", result_b.display()),
            format!("--result={}", result_a.display()),
            format!("--result={}", result_c.display()),
            "--notes=fixture committee aggregate".to_string(),
            format!("--json-out={}", out_path.display()),
        ])
        .expect("committee aggregate succeeds");
        let rendered = fs::read_to_string(&out_path).expect("read committee output");
        assert!(!rendered.contains("payload_b64"));
        let value: Value = norito::json::from_str(&rendered).expect("parse committee output");
        let object = value.as_object().expect("committee output object");
        assert_json_str!(object, "schema", "sorafs.moderation.committee.aggregate.v1");
        assert_json_str!(object, "status", "quorum_satisfied");
        assert_eq!(object.get("result_count").and_then(Value::as_u64), Some(3));
        assert_eq!(object.get("quorum").and_then(Value::as_u64), Some(2));
        assert_json_str!(object, "aggregation", "median_score_bps");
        assert_json_u64!(object, "aggregated_score_bps", 6_100);
        assert_json_str!(object, "verdict", "quarantine");
        assert_json_u64!(object, "screened_at_unix_min", 1_800_003_001);
        assert_json_u64!(object, "screened_at_unix_max", 1_800_003_003);
        let member_results = match object.get("member_results") {
            Some(Value::Array(values)) => values,
            other => panic!("expected member_results array, got {other:?}"),
        };
        assert_eq!(member_results.len(), 3);
    }
    #[test]
    fn moderation_committee_run_rejects_manifest_mismatch() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("repro.json");
        let result_path = temp.path().join("result.json");
        let out_path = temp.path().join("committee.json");
        write_manifest_fixture!(manifest_path, manifest);
        let mut result = moderation_committee_result_fixture(
            &manifest,
            b"committee mismatch payload",
            "cid:bafy-committee",
            6_100,
            1_800_003_010,
        );
        result.as_object_mut().expect("result object").insert(
            "manifest_id_hex".into(),
            Value::from(hex_encode([0xFF; 16])),
        );
        write_moderation_json(&result_path, &result);
        let err = moderation_committee_run(vec![
            format!("--manifest={}", manifest_path.display()),
            "--quorum=1".to_string(),
            format!("--result={}", result_path.display()),
            format!("--json-out={}", out_path.display()),
        ])
        .expect_err("manifest mismatch rejected");
        assert!(err.contains("manifest_id_hex"), "unexpected error: {err}");
    }
    #[test]
    fn moderation_committee_run_rejects_insufficient_quorum() {
        let manifest = signed_moderation_repro_manifest_fixture();
        let temp = TempDir::new().expect("tempdir");
        let manifest_path = temp.path().join("repro.json");
        let result_a = temp.path().join("a.json");
        let result_b = temp.path().join("b.json");
        write_manifest_fixture!(manifest_path, manifest);
        write_moderation_json(
            &result_a,
            &moderation_committee_result_fixture(
                &manifest,
                b"committee quorum payload",
                "cid:bafy-committee",
                5_900,
                1_800_003_020,
            ),
        );
        write_moderation_json(
            &result_b,
            &moderation_committee_result_fixture(
                &manifest,
                b"committee quorum payload",
                "cid:bafy-committee",
                6_100,
                1_800_003_021,
            ),
        );
        let err = moderation_committee_run(vec![
            format!("--manifest={}", manifest_path.display()),
            "--quorum=3".to_string(),
            format!("--result={}", result_a.display()),
            format!("--result={}", result_b.display()),
        ])
        .expect_err("insufficient quorum rejected");
        assert!(err.contains("quorum 3"), "unexpected error: {err}");
    }
    #[test]
    fn moderation_local_runner_rejects_inverted_thresholds() {
        let mut manifest = signed_moderation_repro_manifest_fixture();
        manifest.body.thresholds = ModerationThresholdsV1 {
            quarantine: 9_000,
            escalate: 8_000,
        };
        let err = validate_moderation_local_runner_manifest(&manifest)
            .expect_err("inverted thresholds rejected");
        assert!(
            err.contains("quarantine threshold 9000 exceeds escalate threshold 8000"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn taikai_cache_override_accepts_raw_object() {
        let value = sample_taikai_cache_value();
        let parsed = parse_taikai_cache_override(value).expect("parse succeeds");
        let config = parsed.expect("cache config present");
        assert_eq!(config.hot_capacity_bytes, 8_388_608);
        assert_eq!(config.qos.priority_rate_bps, 83_886_080);
    }
    #[test]
    fn taikai_cache_override_accepts_wrapped_object() {
        let raw = sample_taikai_cache_value();
        let mut map = Map::new();
        insert_json!(map["taikai_cache"] = raw);
        let parsed = parse_taikai_cache_override(Value::Object(map)).expect("wrapped cache parses");
        assert!(parsed.is_some());
    }
    #[test]
    fn taikai_cache_override_allows_null() {
        let parsed = parse_taikai_cache_override(Value::Null).expect("null parses");
        assert!(parsed.is_none());
    }
    #[test]
    fn taikai_cache_override_rejects_invalid_payload() {
        let invalid = norito::json::from_str(r#"{"hot_capacity_bytes": 1}"#).expect("parse");
        let err = parse_taikai_cache_override(invalid).expect_err("invalid config rejected");
        assert!(
            err.contains("failed to parse Taikai cache config"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn appeal_disburse_requires_juror_list() {
        let refund_account = account_string(10);
        let treasury_account = account_string(11);
        let escrow_account = account_string(12);
        let err = appeal_disburse(vec![
            "--deposit=100".into(),
            "--outcome=overturn".into(),
            format!("--refund-account={refund_account}"),
            format!("--treasury-account={treasury_account}"),
            format!("--escrow-account={escrow_account}"),
        ])
        .expect_err("juror roster required");
        assert!(
            err.contains("missing required `--juror`"),
            "unexpected error: {err}"
        );
    }
    fn sample_taikai_cache_value() -> Value {
        norito::json::from_str(
            r#"{
                "hot_capacity_bytes": 8388608,
                "hot_retention_secs": 45,
                "warm_capacity_bytes": 33554432,
                "warm_retention_secs": 180,
                "cold_capacity_bytes": 268435456,
                "cold_retention_secs": 3600,
                "qos": {
                    "priority_rate_bps": 83886080,
                    "standard_rate_bps": 41943040,
                    "bulk_rate_bps": 12582912,
                    "burst_multiplier": 4
                }
            }"#,
        )
        .expect("sample Taikai cache JSON parses")
    }
}
enum InputSummary {
    File { path: PathBuf, bytes: u64 },
    Directory { path: PathBuf, file_count: u64 },
}
#[derive(Clone, Debug)]
struct GatewayProviderSpec {
    name: String,
    provider_id_hex: String,
    gateway_public_key_hex: String,
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
}
struct PlanWithHandle {
    plan: CarBuildPlan,
    chunker_handle: String,
}
fn manifest_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut summary_source: Option<JsonSource> = None;
    let mut manifest_out: Option<PathBuf> = None;
    let mut manifest_json_out: Option<PathBuf> = None;
    let mut pin_min_replicas: Option<u16> = None;
    let mut pin_storage_class: Option<StorageClass> = None;
    let mut pin_retention_epoch: Option<u64> = None;
    let mut metadata_entries: Vec<(String, String)> = Vec::new();
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--summary" => summary_source = Some(JsonSource::from_arg(value)?),
            "--manifest-out" => manifest_out = Some(PathBuf::from(value)),
            "--manifest-json-out" => manifest_json_out = Some(PathBuf::from(value)),
            "--pin-min-replicas" => {
                let replicas: u16 = value
                    .parse()
                    .map_err(|err| format!("invalid --pin-min-replicas value: {err}"))?;
                pin_min_replicas = Some(replicas);
            }
            "--pin-storage-class" => pin_storage_class = Some(parse_storage_class(value)?),
            "--pin-retention-epoch" => {
                let epoch: u64 = value
                    .parse()
                    .map_err(|err| format!("invalid --pin-retention-epoch value: {err}"))?;
                pin_retention_epoch = Some(epoch);
            }
            "--metadata" => {
                let (k, v) = value
                    .split_once('=')
                    .ok_or_else(|| "--metadata expects key=value".to_string())?;
                metadata_entries.push((k.to_string(), v.to_string()));
            }
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli manifest build`"
                ));
            }
        }
    }
    let summary_source = summary_source.ok_or_else(|| {
        "missing required `--summary=PATH` for `sorafs_cli manifest build`".to_string()
    })?;
    let manifest_out = manifest_out.ok_or_else(|| {
        "missing required `--manifest-out=PATH` for `sorafs_cli manifest build`".to_string()
    })?;
    let summary_json = summary_source.read()?;
    let summary_obj = summary_json
        .as_object()
        .ok_or_else(|| "summary must be a JSON object".to_string())?;
    let chunker_handle = summary_obj
        .get("chunker_handle")
        .and_then(Value::as_str)
        .ok_or_else(|| "summary missing `chunker_handle`".to_string())?;
    let descriptor =
        manifest_chunker_registry::lookup_by_handle(chunker_handle).ok_or_else(|| {
            format!(
                "summary references unknown chunker handle `{chunker_handle}`; refresh the registry"
            )
        })?;
    let content_length = summary_obj
        .get("payload_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| "summary missing `payload_bytes`".to_string())?;
    let car_size = summary_obj
        .get("car_size")
        .and_then(Value::as_u64)
        .ok_or_else(|| "summary missing `car_size`".to_string())?;
    let car_digest_hex = summary_obj
        .get("car_digest_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| "summary missing `car_digest_hex`".to_string())?;
    let car_digest = parse_digest_hex(car_digest_hex)
        .map_err(|err| format!("invalid `car_digest_hex` in summary: {err}"))?;
    let chunk_digest_hex = summary_obj
        .get("chunk_digest_sha3_256_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| "summary missing `chunk_digest_sha3_256_hex`".to_string())?;
    let chunk_digest = parse_digest_hex(chunk_digest_hex)
        .map_err(|err| format!("invalid `chunk_digest_sha3_256_hex` in summary: {err}"))?;
    let por_root_hex = summary_obj
        .get("por_root_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| "summary missing `por_root_hex`".to_string())?;
    let por_root = parse_digest_hex(por_root_hex)
        .map_err(|err| format!("invalid `por_root_hex` in summary: {err}"))?;
    let root_cids = summary_obj
        .get("root_cids_hex")
        .and_then(Value::as_array)
        .ok_or_else(|| "summary missing `root_cids_hex` array".to_string())?;
    let first_root_hex = root_cids
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| "summary `root_cids_hex` array is empty".to_string())?;
    let root_cid = parse_hex_vec(first_root_hex)
        .map_err(|err| format!("invalid root CID hex `{first_root_hex}`: {err}"))?;
    let chunking_profile = ChunkingProfileV1::from_descriptor(descriptor);
    let pin_policy = PinPolicy {
        min_replicas: pin_min_replicas.unwrap_or(1),
        storage_class: pin_storage_class.unwrap_or_default(),
        retention_epoch: pin_retention_epoch.unwrap_or(86_400),
    };
    let mut builder = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
        .chunking_profile(chunking_profile)
        .chunk_digest_sha3_256(chunk_digest)
        .por_root(por_root)
        .content_length(content_length)
        .car_digest(car_digest)
        .car_size(car_size)
        .pin_policy(pin_policy);
    if !metadata_entries.is_empty() {
        builder = builder.extend_metadata(metadata_entries.clone());
    }
    let manifest = builder.build().map_err(format_manifest_error)?;
    let manifest_bytes = manifest
        .encode()
        .map_err(|err| format!("failed to encode manifest: {err}"))?;
    write_bytes(&manifest_out, &manifest_bytes)?;
    if let Some(json_path) = manifest_json_out.as_ref() {
        let rendered = to_string_pretty(
            &norito::json::to_value(&manifest)
                .map_err(|err| format!("failed to serialise manifest JSON: {err}"))?,
        )
        .map_err(|err| format!("failed to render manifest JSON: {err}"))?;
        write_text(json_path, rendered.as_bytes())?;
    }
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let mut summary = Map::new();
    insert_value!(summary["manifest_path"] = manifest_out.display().to_string());
    insert_value!(summary["manifest_digest_hex"] = hex_encode(manifest_digest.as_bytes()));
    insert_value!(summary["chunker_handle"] = chunker_handle);
    insert_value!(summary["chunker_profile_id"] = descriptor.id.0 as u64);
    insert_json!(summary["pin_policy"] = Value::Object(pin_policy_json(&pin_policy)));
    if let Some(json_path) = manifest_json_out {
        insert_value!(summary["manifest_json_path"] = json_path.display().to_string());
    }
    if !metadata_entries.is_empty() {
        insert_json!(
            summary["metadata_kv"] = Value::Array(
                metadata_entries
                    .into_iter()
                    .map(|(k, v)| {
                        let mut kv = Map::new();
                        insert_value!(kv["key"] = k);
                        insert_value!(kv["value"] = v);
                        Value::Object(kv)
                    })
                    .collect(),
            )
        );
    }
    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render manifest summary: {err}"))?;
    println!("{rendered}");
    Ok(())
}
fn norito_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut source_spec: Option<String> = None;
    let mut bytecode_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--source" => source_spec = Some(value.to_string()),
            "--bytecode-out" => bytecode_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli norito build`"
                ));
            }
        }
    }
    let source_spec = source_spec.ok_or_else(|| {
        "missing required `--source=PATH` for `sorafs_cli norito build`".to_string()
    })?;
    let bytecode_out = bytecode_out.ok_or_else(|| {
        "missing required `--bytecode-out=PATH` for `sorafs_cli norito build`".to_string()
    })?;
    let (source_text, source_path) = if source_spec == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|err| format!("failed to read Kotodama source from stdin: {err}"))?;
        (buf, None)
    } else {
        let path = PathBuf::from(&source_spec);
        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read Kotodama source `{}`: {err}", path.display()))?;
        (contents, Some(path))
    };
    let source_name = source_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_owned());
    let bytecode = CompilerSession::default()
        .build(CompileRequest {
            source: &source_text,
            source_name: Some(&source_name),
        })
        .map_err(|diagnostics| {
            format!(
                "failed to compile Kotodama source:\n{}",
                diagnostics.render_human()
            )
        })?
        .artifact;
    let abi_version = ivm::ProgramMetadata::parse(&bytecode)
        .map_err(|err| format!("compiler produced invalid Kotodama artifact: {err}"))?
        .metadata
        .abi_version;
    write_bytes(&bytecode_out, &bytecode)?;
    let mut summary = Map::new();
    insert_value!(summary["bytecode_path"] = bytecode_out.display().to_string());
    insert_value!(summary["bytecode_len"] = bytecode.len() as u64);
    insert_value!(summary["bytecode_blake3_hex"] = hex_encode(blake3_hash(&bytecode).as_bytes()));
    insert_value!(summary["abi_version"] = abi_version as u64);
    match &source_path {
        Some(path) => {
            insert_value!(summary["source_kind"] = "file");
            insert_value!(summary["source_path"] = path.display().to_string());
        }
        None => {
            insert_value!(summary["source_kind"] = "stdin");
        }
    }
    let summary_value = Value::Object(summary);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}
fn manifest_submit(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut chunk_digest_hex_arg: Option<String> = None;
    let mut torii_url: Option<String> = None;
    let mut authority_str: Option<String> = None;
    let mut authority_network_prefix: Option<u16> = None;
    let mut network_id: Option<NetworkId> = None;
    let mut private_key_inline: Option<String> = None;
    let mut private_key_path: Option<PathBuf> = None;
    let mut alias_namespace: Option<String> = None;
    let mut alias_name: Option<String> = None;
    let mut alias_proof_path: Option<PathBuf> = None;
    let mut successor_hex: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut response_out: Option<PathBuf> = None;
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--chunk-digest-sha3" => chunk_digest_hex_arg = Some(value.to_string()),
            "--torii-url" => torii_url = Some(value.to_string()),
            "--authority" => authority_str = Some(value.to_string()),
            "--network-prefix" => {
                authority_network_prefix = Some(parse_u16_arg(
                    "--network-prefix",
                    value,
                    "sorafs_cli manifest submit",
                )?);
            }
            "--network-id" => {
                network_id = Some(
                    value
                        .parse()
                        .map_err(|err| format!("invalid `--network-id` value: {err}"))?,
                );
            }
            "--private-key" => {
                if private_key_path.is_some() {
                    return Err(
                        "`--private-key` and `--private-key-file` are mutually exclusive"
                            .to_string(),
                    );
                }
                private_key_inline = Some(value.to_string());
            }
            "--private-key-file" => {
                if private_key_inline.is_some() {
                    return Err(
                        "`--private-key` and `--private-key-file` are mutually exclusive"
                            .to_string(),
                    );
                }
                private_key_path = Some(PathBuf::from(value));
            }
            "--alias-namespace" => alias_namespace = Some(value.to_string()),
            "--alias-name" => alias_name = Some(value.to_string()),
            "--alias-proof" => alias_proof_path = Some(PathBuf::from(value)),
            "--successor-of" => successor_hex = Some(value.to_string()),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--response-out" => response_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli manifest submit`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli manifest submit`".to_string()
    })?;
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli manifest submit`".to_string()
    })?;
    let authority_str = authority_str.ok_or_else(|| {
        "missing required `--authority=ACCOUNT_ID` for `sorafs_cli manifest submit`".to_string()
    })?;
    let network_id = network_id.ok_or_else(|| {
        "missing required `--network-id=NETWORK_ID` for `sorafs_cli manifest submit`".to_string()
    })?;
    authority_network_prefix =
        authority_network_prefix.or_else(|| infer_i105_network_prefix(&authority_str));
    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    let torii_base_url =
        Url::parse(&torii_url).map_err(|err| format!("invalid `--torii-url` value: {err}"))?;
    let plan_specs = if let Some(source) = chunk_plan_source {
        let value = source.read()?;
        Some(
            chunk_fetch_plan_from_json(&value)
                .map_err(|err| format!("failed to parse chunk plan JSON: {err}"))?
                .chunk_fetch_specs,
        )
    } else {
        None
    };
    let plan_chunk_count = plan_specs.as_ref().map(|specs| specs.len() as u64);
    let plan_digest = plan_specs
        .as_ref()
        .map(|specs| chunk_digest_sha3_from_specs(specs));
    let explicit_chunk_digest = chunk_digest_hex_arg
        .map(|hex| {
            parse_digest_hex(&hex).map_err(|err| format!("invalid `--chunk-digest-sha3`: {err}"))
        })
        .transpose()?;
    if let (Some(explicit), Some(from_plan)) = (explicit_chunk_digest, plan_digest)
        && explicit != from_plan
    {
        return Err(format!(
            "explicit chunk digest {} does not match chunk-plan digest {}",
            hex_encode(explicit),
            hex_encode(from_plan),
        ));
    }
    let supplied_chunk_digest = explicit_chunk_digest.or(plan_digest);
    if let Some(supplied) = supplied_chunk_digest
        && supplied != manifest.chunk_digest_sha3_256
    {
        let expected_hex = hex_encode(manifest.chunk_digest_sha3_256);
        let provided_hex = hex_encode(supplied);
        return Err(format!(
            "chunk digest `{provided_hex}` does not match manifest chunk-plan commitment `{expected_hex}`"
        ));
    }
    let chunk_digest = manifest.chunk_digest_sha3_256;
    let manifest_car_digest_hex = hex_encode(manifest.car_digest);
    let authority = parse_account_id_arg_with_prefix(
        "--authority",
        &authority_str,
        "sorafs_cli manifest submit",
        authority_network_prefix,
    )
    .map_err(|err| {
        err.strip_prefix(
            "failed to parse `--authority` for `sorafs_cli manifest submit` as account id: ",
        )
        .map_or_else(
            || format!("invalid authority: {err}"),
            |reason| format!("invalid authority: {reason}"),
        )
    })?;
    let authority_literal = authority_payload_literal(&authority, authority_network_prefix)?;
    let private_key = match (private_key_inline, private_key_path) {
        (Some(inline), None) => parse_private_key_inline(&inline)?,
        (None, Some(path)) => load_private_key_from_file(&path)?,
        (Some(_), Some(_)) => {
            return Err(
                "`--private-key` and `--private-key-file` are mutually exclusive".to_string(),
            );
        }
        (None, None) => {
            return Err(
                "missing private key: supply `--private-key` or `--private-key-file`".to_string(),
            );
        }
    };
    let alias_inputs = alias_inputs_from_flags(alias_namespace, alias_name, alias_proof_path)?;
    let successor_digest = match successor_hex.as_ref() {
        Some(hex) => Some(
            parse_digest_hex(hex)
                .map_err(|err| format!("invalid `--successor-of` value: {err}"))?,
        ),
        None => None,
    };
    let client = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| format!("failed to construct HTTP client: {err}"))?;
    let submission = submit_pin_register(
        &ManifestSubmitRequest {
            client: &client,
            torii_base_url: &torii_base_url,
            network_id: &network_id,
            authority: &authority,
            private_key: &private_key,
            alias_inputs: alias_inputs.as_ref(),
        },
        &manifest,
        successor_digest,
    )?;
    if let Some(path) = response_out {
        write_bytes(&path, &submission.response_bytes)?;
    }
    let mut summary = Map::new();
    insert_value!(summary["torii_url"] = torii_url);
    insert_value!(summary["torii_endpoint"] = submission.endpoint_used.clone());
    insert_value!(summary["torii_endpoint_requested"] = submission.endpoint_requested);
    insert_value!(summary["status"] = submission.status.as_u16() as u64);
    insert_value!(summary["authority"] = authority_literal);
    insert_value!(summary["submission_mode"] = submission.submission_mode);
    insert_value!(summary["manifest_path"] = manifest_path.display().to_string());
    insert_value!(summary["manifest_digest_hex"] = manifest_digest_hex.clone());
    insert_value!(summary["manifest_car_digest_hex"] = manifest_car_digest_hex.clone());
    insert_value!(summary["chunk_digest_sha3_hex"] = hex_encode(chunk_digest));
    insert_value!(
        summary["chunker_handle"] = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )
    );
    insert_json!(summary["pin_policy"] = Value::Object(pin_policy_json(&manifest.pin_policy)));
    if let Some(label) = chunk_plan_label {
        insert_value!(summary["chunk_plan"] = label);
    }
    if let Some(count) = plan_chunk_count {
        insert_value!(summary["chunk_plan_chunk_count"] = count);
    }
    if let Some(alias) = alias_inputs {
        insert_value!(summary["alias_namespace"] = alias.namespace);
        insert_value!(summary["alias_name"] = alias.name);
    }
    if let Some(hex) = successor_hex.as_ref() {
        insert_value!(summary["successor_of_hex"] = hex.clone());
    }
    insert_json!(summary["torii_response"] = submission.response_value);
    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct StorageFileEntryOwned {
    path: Vec<String>,
    size: u64,
}
type StoragePinPayload = (Vec<u8>, Option<Vec<StorageFileEntryOwned>>, &'static str);
fn manifest_root_cid_hex(manifest: &ManifestV1) -> Result<String, String> {
    let root_cid = ManifestRootCid::try_from_slice(&manifest.root_cid)
        .map_err(|err| format!("manifest root_cid is not canonical: {err}"))?;
    Ok(hex_encode(root_cid.as_bytes()))
}
fn chunk_profile_from_manifest(manifest: &ManifestV1) -> Result<ChunkProfile, String> {
    if manifest.chunking.profile_id.0 != 0 {
        let descriptor =
            chunker_registry::lookup(sorafs_car::ProfileId(manifest.chunking.profile_id.0))
                .ok_or_else(|| "manifest chunking profile is not registered".to_string())?;
        let profile = descriptor.profile;
        let geometry_matches = u32::try_from(profile.min_size).ok()
            == Some(manifest.chunking.min_size)
            && u32::try_from(profile.target_size).ok() == Some(manifest.chunking.target_size)
            && u32::try_from(profile.max_size).ok() == Some(manifest.chunking.max_size)
            && u32::try_from(profile.break_mask).ok() == Some(manifest.chunking.break_mask);
        let identity_matches = manifest.chunking.namespace == descriptor.namespace
            && manifest.chunking.name == descriptor.name
            && manifest.chunking.semver == descriptor.semver
            && manifest.chunking.multihash_code == descriptor.multihash_code;
        let aliases_match = manifest.chunking.aliases.len() == descriptor.aliases.len()
            && manifest
                .chunking
                .aliases
                .iter()
                .zip(descriptor.aliases.iter())
                .all(|(provided, expected)| provided == *expected);
        if !geometry_matches || !identity_matches || !aliases_match {
            return Err(
                "manifest chunking profile does not match its registered descriptor".to_string(),
            );
        }
        return Ok(profile);
    }
    if manifest.chunking.namespace != "inline"
        || manifest.chunking.name != "inline"
        || manifest.chunking.semver != "0.0.0"
        || manifest.chunking.aliases.as_slice() != ["inline.inline@0.0.0"]
    {
        return Err("manifest inline chunking profile identity is not canonical".to_string());
    }
    let profile = ChunkProfile {
        min_size: manifest.chunking.min_size as usize,
        target_size: manifest.chunking.target_size as usize,
        max_size: manifest.chunking.max_size as usize,
        break_mask: u64::from(manifest.chunking.break_mask),
    };
    profile
        .validate()
        .map_err(|err| format!("manifest inline chunking profile is invalid: {err}"))?;
    if profile.max_size > sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES as usize {
        return Err(format!(
            "manifest chunking.max_size exceeds the SoraFS limit ({})",
            sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES
        ));
    }
    Ok(profile)
}
fn storage_prepare(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut payload_path: Option<PathBuf> = None;
    let mut payload_out: Option<PathBuf> = None;
    let mut files_out: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--payload" => payload_path = Some(PathBuf::from(value)),
            "--payload-out" => payload_out = Some(PathBuf::from(value)),
            "--files-out" => files_out = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli storage prepare`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let payload_path = payload_path.ok_or_else(|| {
        "missing required `--payload=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let payload_out = payload_out.ok_or_else(|| {
        "missing required `--payload-out=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let files_out = files_out.ok_or_else(|| {
        "missing required `--files-out=PATH` for `sorafs_cli storage prepare`".to_string()
    })?;
    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    let manifest_id_hex = manifest_root_cid_hex(&manifest)?;
    let (payload_bytes, files, payload_kind) = load_storage_pin_payload(&payload_path, &manifest)?;
    let payload_bytes_len = u64::try_from(payload_bytes.len())
        .map_err(|_| "payload exceeds host limits".to_string())?;
    let payload_file_count = files.as_ref().map_or(0_u64, |entries| {
        u64::try_from(entries.len()).unwrap_or(u64::MAX)
    });
    write_bytes(&payload_out, &payload_bytes)?;
    let files_value = storage_files_to_json_value(files.as_deref());
    let files_rendered = to_string_pretty(&files_value)
        .map_err(|err| format!("failed to render storage files JSON: {err}"))?;
    write_text(&files_out, files_rendered.as_bytes())?;
    let mut summary = Map::new();
    insert_value!(summary["manifest_path"] = manifest_path.display().to_string());
    insert_value!(summary["payload_path"] = payload_path.display().to_string());
    insert_value!(summary["payload_out"] = payload_out.display().to_string());
    insert_value!(summary["files_out"] = files_out.display().to_string());
    insert_value!(summary["payload_kind"] = payload_kind);
    insert_value!(summary["payload_bytes"] = payload_bytes_len);
    insert_value!(summary["payload_file_count"] = payload_file_count);
    insert_value!(summary["manifest_digest_hex"] = manifest_digest_hex);
    insert_value!(summary["manifest_id_hex"] = manifest_id_hex);
    insert_value!(
        summary["chunker_handle"] = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )
    );
    let rendered = to_string_pretty(&Value::Object(summary))
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}
fn load_storage_pin_payload(
    input: &Path,
    manifest: &ManifestV1,
) -> Result<StoragePinPayload, String> {
    let metadata = fs::metadata(input)
        .map_err(|err| format!("failed to access payload `{}`: {err}", input.display()))?;
    if metadata.is_dir() {
        let profile = chunk_profile_from_manifest(manifest)?;
        let (plan, payload) = CarBuildPlan::from_directory_with_profile(input, profile)
            .map_err(|err| format!("failed to build directory payload plan: {err}"))?;
        let files = plan
            .files
            .iter()
            .map(|file| StorageFileEntryOwned {
                path: file.path.clone(),
                size: file.size,
            })
            .collect();
        return Ok((payload, Some(files), "directory"));
    }
    if metadata.is_file() {
        let payload = fs::read(input)
            .map_err(|err| format!("failed to read payload `{}`: {err}", input.display()))?;
        return Ok((payload, None, "file"));
    }
    Err("payload input must be a file or directory".to_string())
}
fn storage_files_to_json_value(files: Option<&[StorageFileEntryOwned]>) -> Value {
    match files {
        Some(entries) => Value::Array(
            entries
                .iter()
                .map(|entry| {
                    Value::Object(Map::from_iter([
                        (
                            "path".into(),
                            Value::Array(entry.path.iter().cloned().map(Value::from).collect()),
                        ),
                        ("size".into(), Value::from(entry.size)),
                    ]))
                })
                .collect(),
        ),
        None => Value::Null,
    }
}
fn decode_response_value_or_text(response: &[u8]) -> Value {
    match from_slice(response) {
        Ok(value) => value,
        Err(_) => Value::from(String::from_utf8_lossy(response).to_string()),
    }
}
fn manifest_proposal(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut chunk_digest_hex_arg: Option<String> = None;
    let mut successor_hex: Option<String> = None;
    let mut alias_hint: Option<String> = None;
    let mut proposal_out: Option<PathBuf> = None;
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--chunk-digest-sha3" => chunk_digest_hex_arg = Some(value.to_string()),
            "--successor-of" => successor_hex = Some(value.to_string()),
            "--alias-hint" => alias_hint = Some(value.to_string()),
            "--proposal-out" => proposal_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unknown option `{key}` for `sorafs_cli manifest proposal`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli manifest proposal`".to_string()
    })?;
    let proposal_out = proposal_out.ok_or_else(|| {
        "missing required `--proposal-out=PATH` for `sorafs_cli manifest proposal`".to_string()
    })?;
    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let plan_specs = if let Some(source) = chunk_plan_source {
        let value = source.read()?;
        Some(
            chunk_fetch_plan_from_json(&value)
                .map_err(|err| format!("failed to parse chunk plan JSON: {err}"))?
                .chunk_fetch_specs,
        )
    } else {
        None
    };
    let chunk_digest = match (chunk_digest_hex_arg, plan_specs.as_ref()) {
        (Some(hex), Some(specs)) => {
            let parsed =
                parse_digest_hex(&hex).map_err(|err| format!("invalid `--chunk-digest-sha3`: {err}"))?;
            let expected = chunk_digest_sha3_from_specs(specs);
            if parsed != expected {
                return Err(
                    "`--chunk-digest-sha3` does not match digest derived from `--chunk-plan`"
                        .to_string(),
                );
            }
            parsed
        }
        (Some(hex), None) => {
            parse_digest_hex(&hex).map_err(|err| format!("invalid `--chunk-digest-sha3`: {err}"))?
        }
        (None, Some(specs)) => chunk_digest_sha3_from_specs(specs),
        (None, None) => {
            return Err(
                "must provide either `--chunk-plan` or `--chunk-digest-sha3` for `sorafs_cli manifest proposal`"
                    .to_string(),
            )
        }
    };
    let successor_bytes = match successor_hex {
        Some(hex) => Some(
            parse_digest_hex(&hex)
                .map_err(|err| format!("invalid `--successor-of` value: {err}"))?,
        ),
        None => None,
    };
    let proposal_value = build_manifest_proposal_summary(ManifestProposalSummary {
        manifest_path: &manifest_path,
        manifest: &manifest,
        manifest_digest: &manifest_digest,
        chunk_digest_sha3: chunk_digest,
        chunk_plan_label: chunk_plan_label.as_deref(),
        alias_hint: alias_hint.as_deref(),
        successor_bytes,
    })?;
    let mut rendered = to_string_pretty(&proposal_value)
        .map_err(|err| format!("failed to render proposal JSON: {err}"))?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    write_text(&proposal_out, rendered.as_bytes())
        .map_err(|err| format!("failed to write `{}`: {err}", proposal_out.display()))?;
    println!("wrote {}", proposal_out.display());
    Ok(())
}
fn proof_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut car_path: Option<PathBuf> = None;
    let mut chunk_plan_source: Option<JsonSource> = None;
    let mut chunk_plan_label: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--car" => car_path = Some(PathBuf::from(value)),
            "--chunk-plan" => {
                chunk_plan_source = Some(JsonSource::from_arg(value)?);
                chunk_plan_label = Some(value.to_string());
            }
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli proof verify`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli proof verify`".to_string()
    })?;
    let car_path = car_path
        .ok_or_else(|| "missing required `--car=PATH` for `sorafs_cli proof verify`".to_string())?;
    let manifest_bytes = fs::read(&manifest_path).map_err(|err| {
        format!(
            "failed to read manifest `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to decode manifest: {err}"))?;
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let car_bytes = fs::read(&car_path)
        .map_err(|err| format!("failed to read CAR archive `{}`: {err}", car_path.display()))?;
    let resolved_plan = if let Some(source) = chunk_plan_source {
        let plan_json = source.read()?;
        let chunker_handle = chunker_handle_from_profile(&manifest.chunking).to_handle();
        Some(build_plan_from_specs(&plan_json, Some(&chunker_handle))?)
    } else {
        None
    };
    let report = if let Some(plan_with_handle) = resolved_plan.as_ref() {
        CarVerifier::verify_full_car_with_plan(&manifest, &plan_with_handle.plan, &car_bytes)
    } else {
        CarVerifier::verify_full_car(&manifest, &car_bytes)
    }
    .map_err(|err| format!("failed to verify CAR archive: {err}"))?;
    let payload_digest_hex = hex_encode(report.chunk_store.payload_digest().as_bytes());
    let chunk_digest_sha3 = chunk_digest_sha3_from_chunks(report.chunk_store.chunks());
    let chunk_digest_hex = hex_encode(chunk_digest_sha3);
    let car_payload_digest_hex = hex_encode(report.stats.car_payload_digest.as_bytes());
    let car_digest_hex = hex_encode(report.stats.car_archive_digest.as_bytes());
    let mut summary = Map::new();
    insert_value!(summary["manifest_path"] = manifest_path.display().to_string());
    insert_value!(summary["car_path"] = car_path.display().to_string());
    insert_value!(summary["chunk_count"] = report.chunk_store.chunks().len() as u64);
    if let Some(label) = chunk_plan_label {
        insert_value!(summary["chunk_plan_source"] = label);
    }
    if let Some(plan_with_handle) = resolved_plan.as_ref() {
        insert_value!(
            summary["chunk_plan_chunk_count"] = plan_with_handle.plan.chunks.len() as u64
        );
    }
    insert_value!(summary["payload_bytes"] = report.chunk_store.payload_len());
    insert_value!(summary["payload_digest_hex"] = payload_digest_hex);
    insert_value!(summary["chunk_digest_sha3_hex"] = chunk_digest_hex);
    insert_value!(summary["car_payload_digest_hex"] = car_payload_digest_hex);
    insert_value!(summary["car_digest_hex"] = car_digest_hex.clone());
    insert_value!(summary["manifest_car_digest_hex"] = car_digest_hex);
    insert_value!(summary["car_size"] = report.stats.car_size);
    insert_value!(
        summary["chunker_handle"] = format!(
            "{}.{}@{}",
            manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
        )
    );
    insert_value!(summary["manifest_digest_hex"] = hex_encode(manifest_digest.as_bytes()));
    insert_json!(summary["pin_policy"] = Value::Object(pin_policy_json(&manifest.pin_policy)));
    insert_json!(
        summary["root_cids_hex"] = Value::Array(
            report
                .stats
                .root_cids
                .iter()
                .map(|cid| Value::from(hex_encode(cid)))
                .collect(),
        )
    );
    insert_value!(summary["dag_codec"] = report.stats.dag_codec);
    insert_value!(summary["chunker_profile_id"] = u64::from(manifest.chunking.profile_id.0));
    insert_value!(summary["car_payload_bytes"] = report.stats.payload_bytes);
    let summary_value = Value::Object(summary);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}
struct ReputationRequestAuth {
    account_header_value: String,
    network_id: NetworkId,
    key_pair: KeyPair,
}
struct ReputationRequestHeaders<'a> {
    account_header_value: &'a str,
    signature_base64: String,
    timestamp_ms: u64,
    nonce: String,
}
fn parse_reputation_auth_option(
    key: &str,
    value: &str,
    context: &str,
    account_literal: &mut Option<String>,
    private_key_path: &mut Option<PathBuf>,
    network_id: &mut Option<NetworkId>,
) -> Result<bool, String> {
    match key {
        "--auth-account" => {
            if account_literal.replace(value.to_owned()).is_some() {
                return Err(format!("duplicate `--auth-account` for `{context}`"));
            }
            Ok(true)
        }
        "--auth-private-key-file" => {
            if private_key_path.replace(PathBuf::from(value)).is_some() {
                return Err(format!(
                    "duplicate `--auth-private-key-file` for `{context}`"
                ));
            }
            Ok(true)
        }
        "--network-id" => {
            *network_id = Some(value.parse().map_err(|_| {
                format!("`--network-id` must be a canonical checked NetworkId for `{context}`")
            })?);
            Ok(true)
        }
        _ => Ok(false),
    }
}
fn load_reputation_request_auth(
    account_literal: Option<String>,
    private_key_path: Option<PathBuf>,
    network_id: Option<NetworkId>,
    context: &str,
) -> Result<ReputationRequestAuth, String> {
    let account_literal = account_literal
        .ok_or_else(|| format!("missing required `--auth-account=I105` for `{context}`"))?;
    let private_key_path = private_key_path.ok_or_else(|| {
        format!("missing required `--auth-private-key-file=PATH` for `{context}`")
    })?;
    let network_id = network_id
        .ok_or_else(|| format!("missing required `--network-id=NETWORK_ID` for `{context}`"))?;
    if private_key_path.as_os_str().is_empty() {
        return Err(format!(
            "`--auth-private-key-file` must not be empty for `{context}`"
        ));
    }
    let account = parse_reputation_auth_account(&account_literal, context)?;
    let private_key = load_reputation_auth_private_key(&private_key_path, context)?;
    let key_pair = KeyPair::from_private_key(private_key).map_err(|_| {
        format!("failed to derive the reputation authentication key for `{context}`")
    })?;
    let signatory = account.try_signatory().ok_or_else(|| {
        format!("`--auth-account` must identify a single-key account for `{context}`")
    })?;
    if signatory != key_pair.public_key() {
        return Err(format!(
            "`--auth-private-key-file` does not control `--auth-account` for `{context}`"
        ));
    }
    let account_header_value = account
        .to_canonical_hex()
        .map_err(|_| format!("failed to encode `--auth-account` for `{context}`"))?;
    Ok(ReputationRequestAuth {
        account_header_value,
        network_id,
        key_pair,
    })
}
fn parse_reputation_auth_account(raw: &str, context: &str) -> Result<AccountId, String> {
    if raw.is_empty() || raw.trim() != raw {
        return Err(format!(
            "`--auth-account` must be an exact canonical I105 literal without padding for `{context}`"
        ));
    }
    let discriminant = AccountAddress::i105_discriminant(raw).map_err(|_| {
        format!("`--auth-account` must be an exact canonical I105 literal for `{context}`")
    })?;
    let address =
        AccountAddress::from_i105_for_discriminant(raw, Some(discriminant)).map_err(|_| {
            format!("`--auth-account` must be an exact canonical I105 literal for `{context}`")
        })?;
    let canonical = address
        .to_i105_for_discriminant(discriminant)
        .map_err(|_| format!("failed to canonicalise `--auth-account` for `{context}`"))?;
    if canonical != raw {
        return Err(format!(
            "`--auth-account` must be an exact canonical I105 literal for `{context}`"
        ));
    }
    let account = address
        .to_account_id()
        .map_err(|_| format!("failed to decode `--auth-account` for `{context}`"))?;
    if account.try_signatory().is_none() {
        return Err(format!(
            "`--auth-account` must identify a single-key account for `{context}`"
        ));
    }
    Ok(account)
}
#[cfg(unix)]
type ReputationAuthFileIdentity = (u64, u64);
#[cfg(windows)]
type ReputationAuthFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type ReputationAuthFileIdentity = ();
#[cfg(unix)]
fn reputation_auth_file_identity(metadata: &FsMetadata) -> ReputationAuthFileIdentity {
    (metadata.dev(), metadata.ino())
}
#[cfg(windows)]
fn reputation_auth_file_identity(metadata: &FsMetadata) -> ReputationAuthFileIdentity {
    use std::os::windows::fs::MetadataExt as _;
    (metadata.volume_serial_number(), metadata.file_index())
}
#[cfg(not(any(unix, windows)))]
fn reputation_auth_file_identity(_metadata: &FsMetadata) -> ReputationAuthFileIdentity {}
#[cfg(unix)]
const fn reputation_auth_file_identity_available(_identity: ReputationAuthFileIdentity) -> bool {
    true
}
#[cfg(windows)]
const fn reputation_auth_file_identity_available(identity: ReputationAuthFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}
#[cfg(not(any(unix, windows)))]
const fn reputation_auth_file_identity_available(_identity: ReputationAuthFileIdentity) -> bool {
    false
}
fn reputation_auth_file_is_single_link(metadata: &FsMetadata) -> bool {
    #[cfg(unix)]
    {
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}
#[cfg(windows)]
fn reputation_auth_file_is_reparse_point(metadata: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn reputation_auth_file_is_reparse_point(_metadata: &FsMetadata) -> bool {
    false
}
fn reputation_auth_file_is_indirect(metadata: &FsMetadata) -> bool {
    metadata.file_type().is_symlink() || reputation_auth_file_is_reparse_point(metadata)
}
fn validate_reputation_auth_file_metadata(metadata: &FsMetadata) -> Result<(), String> {
    if reputation_auth_file_is_indirect(metadata)
        || !metadata.file_type().is_file()
        || !reputation_auth_file_identity_available(reputation_auth_file_identity(metadata))
    {
        return Err(
            "reputation authentication private key must be a regular non-symlink file with a stable identity"
                .to_owned(),
        );
    }
    if !reputation_auth_file_is_single_link(metadata) {
        return Err(
            "reputation authentication private key must have exactly one hard link".to_owned(),
        );
    }
    #[cfg(unix)]
    if metadata.mode() & 0o077 != 0 {
        return Err(
            "reputation authentication private key must not grant group or world permissions"
                .to_owned(),
        );
    }
    Ok(())
}
#[cfg(unix)]
fn reputation_auth_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    reputation_auth_file_identity(left) == reputation_auth_file_identity(right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
}
#[cfg(windows)]
fn reputation_auth_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    reputation_auth_file_identity_available(reputation_auth_file_identity(left))
        && reputation_auth_file_identity(left) == reputation_auth_file_identity(right)
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}
#[cfg(not(any(unix, windows)))]
fn reputation_auth_metadata_unchanged(_left: &FsMetadata, _right: &FsMetadata) -> bool {
    false
}
#[cfg(unix)]
fn open_reputation_auth_private_key(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    options
        .open(path)
        .map_err(|_| "failed to securely open reputation authentication private key".to_owned())
}
#[cfg(windows)]
fn open_reputation_auth_private_key(path: &Path) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(0x0000_0001)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options
        .open(path)
        .map_err(|_| "failed to securely open reputation authentication private key".to_owned())
}
#[cfg(not(any(unix, windows)))]
fn open_reputation_auth_private_key(_path: &Path) -> Result<File, String> {
    Err(
        "this platform does not expose the stable file identity required for reputation authentication private keys"
            .to_owned(),
    )
}
fn read_reputation_auth_private_key(path: &Path) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path)
        .map_err(|_| "failed to inspect reputation authentication private key".to_owned())?;
    validate_reputation_auth_file_metadata(&before)?;
    if before.len() == 0 || before.len() > REPUTATION_AUTH_PRIVATE_KEY_MAX_BYTES {
        return Err(format!(
            "reputation authentication private key must contain between 1 and {REPUTATION_AUTH_PRIVATE_KEY_MAX_BYTES} bytes"
        ));
    }
    let expected_len = usize::try_from(before.len()).map_err(|_| {
        "reputation authentication private key length is not representable on this host".to_owned()
    })?;
    let mut file = open_reputation_auth_private_key(path)?;
    let opened = file
        .metadata()
        .map_err(|_| "failed to inspect opened reputation authentication private key".to_owned())?;
    validate_reputation_auth_file_metadata(&opened)?;
    if !reputation_auth_metadata_unchanged(&before, &opened) {
        return Err(
            "reputation authentication private key changed between inspection and open".to_owned(),
        );
    }
    let mut bytes = vec![0_u8; expected_len];
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            "reputation authentication private key changed length while being read".to_owned()
        } else {
            "failed to read reputation authentication private key".to_owned()
        }
    })?;
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(|_| "failed to finish reading reputation authentication private key".to_owned())?
        != 0
    {
        return Err(
            "reputation authentication private key changed length while being read".to_owned(),
        );
    }
    let after_file = file.metadata().map_err(|_| {
        "failed to re-inspect opened reputation authentication private key".to_owned()
    })?;
    let after_path = fs::symlink_metadata(path)
        .map_err(|_| "failed to re-inspect reputation authentication private key".to_owned())?;
    validate_reputation_auth_file_metadata(&after_file)?;
    validate_reputation_auth_file_metadata(&after_path)?;
    if !reputation_auth_metadata_unchanged(&opened, &after_file)
        || !reputation_auth_metadata_unchanged(&opened, &after_path)
        || after_file.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("reputation authentication private key changed while being read".to_owned());
    }
    Ok(bytes)
}
fn load_reputation_auth_private_key(path: &Path, context: &str) -> Result<PrivateKey, String> {
    let mut bytes = read_reputation_auth_private_key(path)?;
    let parsed = (|| {
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            format!("reputation authentication private key is not valid UTF-8 for `{context}`")
        })?;
        let token = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(text);
        if token.is_empty()
            || token.trim() != token
            || token
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(format!(
                "reputation authentication private key file is not canonical text for `{context}`"
            ));
        }
        PrivateKey::from_str(token).map_err(|_| {
            format!("reputation authentication private key file is malformed for `{context}`")
        })
    })();
    bytes.fill(0);
    parsed
}
include!("sorafs_cli/reputation_canonical_request.rs");
fn reputation_request_timestamp_ms_at(now: SystemTime) -> Result<u64, String> {
    let elapsed = now.duration_since(UNIX_EPOCH).map_err(|_| {
        "system clock is before the Unix epoch; cannot sign reputation request".to_owned()
    })?;
    elapsed
        .as_millis()
        .try_into()
        .map_err(|_| "system clock does not fit the reputation timestamp field".to_owned())
}
fn reputation_request_nonce_with_rng<R>(rng: &mut R) -> Result<String, String>
where
    R: rand::rand_core::TryCryptoRng + ?Sized,
{
    let mut nonce = [0_u8; 12];
    rand::rand_core::TryRngCore::try_fill_bytes(rng, &mut nonce)
        .map_err(|_| "OS RNG failed while signing reputation request".to_owned())?;
    Ok(BASE64_URL_SAFE_NO_PAD.encode(nonce))
}
fn reputation_request_headers_with_rng_at<'a, R>(
    auth: &'a ReputationRequestAuth,
    endpoint: &Url,
    now: SystemTime,
    rng: &mut R,
) -> Result<ReputationRequestHeaders<'a>, String>
where
    R: rand::rand_core::TryCryptoRng + ?Sized,
{
    let timestamp_ms = reputation_request_timestamp_ms_at(now)?;
    let nonce = reputation_request_nonce_with_rng(rng)?;
    let message =
        canonical_reputation_request_message(&auth.network_id, endpoint, timestamp_ms, &nonce)?;
    let signature = Signature::try_new(auth.key_pair.private_key(), &message)
        .map_err(|_| "failed to sign reputation request".to_owned())?;
    Ok(ReputationRequestHeaders {
        account_header_value: &auth.account_header_value,
        signature_base64: BASE64_STANDARD.encode(signature.payload()),
        timestamp_ms,
        nonce,
    })
}
fn reputation_request_headers<'a>(
    auth: &'a ReputationRequestAuth,
    endpoint: &Url,
) -> Result<ReputationRequestHeaders<'a>, String> {
    reputation_request_headers_with_rng_at(
        auth,
        endpoint,
        SystemTime::now(),
        &mut rand::rngs::OsRng,
    )
}
fn reputation_http_client() -> Result<HttpClient, String> {
    HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .redirect(RedirectPolicy::none())
        .referer(false)
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .no_proxy()
        .build()
        .map_err(|err| format!("failed to construct reputation HTTP client: {err}"))
}
fn send_reputation_request(
    client: &HttpClient,
    endpoint: &Url,
    auth: &ReputationRequestAuth,
) -> Result<reqwest::blocking::Response, String> {
    let headers = reputation_request_headers(auth, endpoint)?;
    client
        .get(endpoint.clone())
        .header("Accept", "application/json")
        .header(ACCEPT_ENCODING, "identity")
        .header(REPUTATION_HEADER_ACCOUNT, headers.account_header_value)
        .header(REPUTATION_HEADER_SIGNATURE, headers.signature_base64)
        .header(
            REPUTATION_HEADER_TIMESTAMP_MS,
            headers.timestamp_ms.to_string(),
        )
        .header(REPUTATION_HEADER_NONCE, headers.nonce)
        .send()
        .map_err(|_| "reputation request failed".to_owned())
}
fn reject_duplicate_reputation_option(
    seen: &mut BTreeSet<String>,
    key: &str,
    context: &str,
) -> Result<(), String> {
    if seen.insert(key.to_owned()) {
        Ok(())
    } else {
        Err(format!("duplicate `{key}` for `{context}`"))
    }
}
fn parse_reputation_provider_id(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    if bytes.is_empty()
        || bytes.len() > REPUTATION_PROVIDER_ID_MAX_BYTES
        || matches!(raw, "." | "..")
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(
            "`--provider-id` must be canonical 1..=256 byte ASCII [A-Za-z0-9_.:-] and must not be a dot-segment"
                .to_owned(),
        );
    }
    Ok(raw.to_owned())
}
fn reputation_snapshot(raw_args: Vec<String>) -> Result<(), String> {
    const CONTEXT: &str = "sorafs_cli reputation snapshot";
    let mut torii_url: Option<String> = None;
    let mut output: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut auth_account: Option<String> = None;
    let mut auth_private_key_path: Option<PathBuf> = None;
    let mut network_id: Option<NetworkId> = None;
    let mut seen_options = BTreeSet::new();
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument for `{CONTEXT}`"))?;
        reject_duplicate_reputation_option(&mut seen_options, key, CONTEXT)?;
        if parse_reputation_auth_option(
            key,
            value,
            CONTEXT,
            &mut auth_account,
            &mut auth_private_key_path,
            &mut network_id,
        )? {
            continue;
        }
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--output" => output = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation snapshot`"
                ));
            }
        }
    }
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation snapshot`".to_string()
    })?;
    let auth =
        load_reputation_request_auth(auth_account, auth_private_key_path, network_id, CONTEXT)?;
    let client = reputation_http_client()?;
    let endpoint = reputation_endpoint(&torii_url, "v1/sorafs/reputation/latest")?;
    let response = send_reputation_request(&client, &endpoint, &auth)
        .map_err(|_| "failed to fetch reputation snapshot".to_owned())?;
    let value = read_json_response(response, "reputation snapshot")?;
    let output_path = output.as_deref().or(summary_out.as_deref());
    emit_reputation_json(value, output_path)
}
fn reputation_fetch(raw_args: Vec<String>) -> Result<(), String> {
    const CONTEXT: &str = "sorafs_cli reputation fetch";
    let mut torii_url: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut format = ReputationFetchFormat::Table;
    let mut summary_out: Option<PathBuf> = None;
    let mut auth_account: Option<String> = None;
    let mut auth_private_key_path: Option<PathBuf> = None;
    let mut network_id: Option<NetworkId> = None;
    let mut seen_options = BTreeSet::new();
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument for `{CONTEXT}`"))?;
        reject_duplicate_reputation_option(&mut seen_options, key, CONTEXT)?;
        if parse_reputation_auth_option(
            key,
            value,
            CONTEXT,
            &mut auth_account,
            &mut auth_private_key_path,
            &mut network_id,
        )? {
            continue;
        }
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--provider-id" => provider_id = Some(value.to_string()),
            "--format" => format = ReputationFetchFormat::parse(value)?,
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation fetch`"
                ));
            }
        }
    }
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation fetch`".to_string()
    })?;
    let provider_id = provider_id
        .ok_or_else(|| {
            "missing required `--provider-id=ID` for `sorafs_cli reputation fetch`".to_string()
        })
        .and_then(|value| parse_reputation_provider_id(&value))?;
    let auth =
        load_reputation_request_auth(auth_account, auth_private_key_path, network_id, CONTEXT)?;
    let client = reputation_http_client()?;
    let route = format!("v1/sorafs/reputation/providers/{provider_id}");
    let endpoint = reputation_endpoint(&torii_url, &route)?;
    let response = send_reputation_request(&client, &endpoint, &auth)
        .map_err(|_| "failed to fetch reputation provider".to_owned())?;
    let value = read_json_response(response, "reputation fetch")?;
    if let Some(path) = summary_out.as_deref() {
        write_reputation_json(path, &value)?;
    }
    match format {
        ReputationFetchFormat::Json => {
            let rendered = to_string_pretty(&value)
                .map_err(|err| format!("failed to render reputation provider JSON: {err}"))?;
            println!("{rendered}");
        }
        ReputationFetchFormat::Table => {
            println!("{}", reputation_provider_table(&value)?);
        }
    }
    Ok(())
}
fn reputation_watch(raw_args: Vec<String>) -> Result<(), String> {
    const CONTEXT: &str = "sorafs_cli reputation watch";
    let mut torii_url: Option<String> = None;
    let mut since: Option<u64> = None;
    let mut limit: Option<u32> = None;
    let mut max_polls: Option<usize> = Some(1);
    let mut poll_interval_ms: u64 = 1_000;
    let mut summary_out: Option<PathBuf> = None;
    let mut auth_account: Option<String> = None;
    let mut auth_private_key_path: Option<PathBuf> = None;
    let mut network_id: Option<NetworkId> = None;
    let mut seen_options = BTreeSet::new();
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument for `{CONTEXT}`"))?;
        reject_duplicate_reputation_option(&mut seen_options, key, CONTEXT)?;
        if parse_reputation_auth_option(
            key,
            value,
            CONTEXT,
            &mut auth_account,
            &mut auth_private_key_path,
            &mut network_id,
        )? {
            continue;
        }
        match key {
            "--torii-url" => torii_url = Some(value.to_string()),
            "--since" => {
                since = Some(parse_u64_arg(
                    "since",
                    value,
                    "sorafs_cli reputation watch",
                )?)
            }
            "--limit" => {
                limit = Some(parse_u32_arg(
                    "limit",
                    value,
                    "sorafs_cli reputation watch",
                )?)
            }
            "--max-polls" => {
                let parsed = parse_usize(value, "--max-polls")?;
                max_polls = (parsed != 0).then_some(parsed);
            }
            "--poll-interval-ms" => {
                poll_interval_ms =
                    parse_u64_arg("poll-interval-ms", value, "sorafs_cli reputation watch")?;
            }
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation watch`"
                ));
            }
        }
    }
    let torii_url = torii_url.ok_or_else(|| {
        "missing required `--torii-url=URL` for `sorafs_cli reputation watch`".to_string()
    })?;
    let auth =
        load_reputation_request_auth(auth_account, auth_private_key_path, network_id, CONTEXT)?;
    let client = reputation_http_client()?;
    let final_value = run_reputation_watch(
        &torii_url,
        since,
        limit,
        max_polls,
        poll_interval_ms,
        |endpoint| {
            let response = send_reputation_request(&client, endpoint, &auth)
                .map_err(|_| "failed to watch reputation events".to_owned())?;
            read_json_response(response, "reputation watch")
        },
    )?;
    if let Some(path) = summary_out.as_deref() {
        write_reputation_json(path, &final_value)?;
    }
    Ok(())
}
fn run_reputation_watch<F>(
    torii_url: &str,
    since: Option<u64>,
    limit: Option<u32>,
    max_polls: Option<usize>,
    poll_interval_ms: u64,
    mut fetch: F,
) -> Result<Value, String>
where
    F: FnMut(&Url) -> Result<Value, String>,
{
    let mut next_since = since;
    let mut polls = 0_usize;
    loop {
        let endpoint = reputation_events_endpoint(torii_url, next_since, limit)?;
        let value = fetch(&endpoint)?;
        if let Some(cursor) = value.get("next_since").and_then(Value::as_u64) {
            next_since = Some(cursor);
        }
        let rendered = to_string_pretty(&value)
            .map_err(|err| format!("failed to render reputation watch JSON: {err}"))?;
        println!("{rendered}");
        polls = polls.saturating_add(1);
        if max_polls.is_some_and(|max| polls >= max) {
            return Ok(value);
        }
        thread::sleep(Duration::from_millis(poll_interval_ms));
    }
}
#[derive(Clone, Copy)]
enum ReputationFetchFormat {
    Table,
    Json,
}
impl ReputationFetchFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            _ => Err("unsupported reputation fetch format; expected `table` or `json`".to_owned()),
        }
    }
}
fn read_reputation_snapshot(path: &Path) -> Result<ReputationSnapshotV1, String> {
    let snapshot_bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read reputation snapshot `{}`: {err}",
            path.display()
        )
    })?;
    let snapshot: ReputationSnapshotV1 = decode_from_bytes(&snapshot_bytes)
        .map_err(|_| "failed to decode reputation snapshot".to_owned())?;
    snapshot
        .validate()
        .map_err(|_| "invalid reputation snapshot".to_owned())?;
    Ok(snapshot)
}
fn reputation_endpoint(torii_url: &str, route: &str) -> Result<Url, String> {
    if torii_url.is_empty() || torii_url.trim() != torii_url {
        return Err("`--torii-url` must be an exact canonical URL without padding".to_owned());
    }
    let parsed =
        Url::parse(torii_url).map_err(|_| "`--torii-url` must be a valid URL".to_owned())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "`--torii-url` must include a host".to_owned())?;
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && is_loopback) {
        return Err(
            "`--torii-url` must use HTTPS; HTTP is permitted only for loopback fixtures".to_owned(),
        );
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("`--torii-url` must not include userinfo".to_owned());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("`--torii-url` must not include a query or fragment".to_owned());
    }
    if parsed.port() == Some(0) {
        return Err("`--torii-url` must not use port zero".to_owned());
    }
    let canonical_origin = parsed.origin().ascii_serialization();
    let canonical_origin_with_slash = format!("{canonical_origin}/");
    if parsed.path() != "/"
        || (torii_url != canonical_origin && torii_url != canonical_origin_with_slash)
    {
        return Err(
            "`--torii-url` must be an exact canonical bare origin without a path prefix".to_owned(),
        );
    }
    parsed
        .join(route)
        .map_err(|_| "failed to build reputation endpoint URL".to_owned())
}
fn reputation_events_endpoint(
    torii_url: &str,
    since: Option<u64>,
    limit: Option<u32>,
) -> Result<Url, String> {
    let mut endpoint = reputation_endpoint(torii_url, "v1/sorafs/reputation/events")?;
    let mut serializer = Serializer::new(String::new());
    if let Some(since) = since {
        serializer.append_pair("since", &since.to_string());
    }
    if let Some(limit) = limit {
        serializer.append_pair("limit", &limit.to_string());
    }
    let query = serializer.finish();
    if !query.is_empty() {
        endpoint.set_query(Some(&query));
    }
    Ok(endpoint)
}
fn read_reputation_response_bounded(
    response: reqwest::blocking::Response,
    context: &str,
) -> Result<(StatusCode, Vec<u8>), String> {
    let status = response.status();
    if !status.is_success() {
        return Ok((status, Vec::new()));
    }
    let mut content_types = response.headers().get_all(CONTENT_TYPE).iter();
    let content_type_is_canonical = content_types
        .next()
        .is_some_and(|value| value.as_bytes() == b"application/json")
        && content_types.next().is_none();
    if !content_type_is_canonical {
        return Err(format!(
            "Torii {context} response must use canonical Content-Type application/json"
        ));
    }
    let content_encoding_headers = response.headers().get_all(CONTENT_ENCODING);
    let mut content_encodings = content_encoding_headers.iter();
    let identity_only = match content_encodings.next() {
        None => true,
        Some(encoding) => encoding.as_bytes() == b"identity" && content_encodings.next().is_none(),
    };
    if !identity_only {
        return Err(format!(
            "Torii {context} response must use identity content encoding"
        ));
    }
    let mut content_lengths = response.headers().get_all(CONTENT_LENGTH).iter();
    let content_length = content_lengths
        .next()
        .map(|value| {
            let raw = value.as_bytes();
            if raw.is_empty()
                || !raw.iter().all(u8::is_ascii_digit)
                || raw.len() > 1 && raw.starts_with(b"0")
            {
                return Err(format!(
                    "Torii {context} response Content-Length must be canonical unsigned decimal"
                ));
            }
            let raw = std::str::from_utf8(raw).map_err(|_| {
                format!("Torii {context} response Content-Length must be canonical ASCII")
            })?;
            raw.parse::<u64>()
                .map_err(|_| format!("Torii {context} response Content-Length does not fit u64"))
        })
        .transpose()?;
    if content_lengths.next().is_some() {
        return Err(format!(
            "Torii {context} response must not contain duplicate Content-Length headers"
        ));
    }
    if content_length.is_some_and(|length| length > REPUTATION_RESPONSE_MAX_BYTES) {
        return Err(format!(
            "Torii {context} response declared more than {REPUTATION_RESPONSE_MAX_BYTES} bytes"
        ));
    }
    let initial_capacity = content_length.unwrap_or(0);
    let initial_capacity = usize::try_from(initial_capacity)
        .map_err(|_| format!("Torii {context} response length does not fit usize"))?;
    let mut body = Vec::new();
    body.try_reserve_exact(initial_capacity)
        .map_err(|_| format!("failed to reserve bounded Torii {context} response"))?;
    response
        .take(REPUTATION_RESPONSE_MAX_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|_| format!("failed to read Torii {context} response body"))?;
    let body_len = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if body_len > REPUTATION_RESPONSE_MAX_BYTES {
        return Err(format!(
            "Torii {context} response exceeded {REPUTATION_RESPONSE_MAX_BYTES} bytes"
        ));
    }
    if content_length.is_some_and(|length| length != body_len) {
        return Err(format!(
            "Torii {context} response body length did not match Content-Length"
        ));
    }
    Ok((status, body))
}
fn read_json_response(
    response: reqwest::blocking::Response,
    context: &str,
) -> Result<Value, String> {
    let (status, body) = read_reputation_response_bounded(response, context)?;
    if !status.is_success() {
        return Err(format!("Torii {context} endpoint returned {status}"));
    }
    from_slice(&body).map_err(|_| format!("failed to decode Torii {context} JSON"))
}
fn emit_reputation_json(value: Value, output: Option<&Path>) -> Result<(), String> {
    let rendered = to_string_pretty(&value)
        .map_err(|err| format!("failed to render reputation JSON: {err}"))?;
    println!("{rendered}");
    if let Some(path) = output {
        write_reputation_json(path, &value)?;
    }
    Ok(())
}
fn write_reputation_json(path: &Path, value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|err| format!("failed to render reputation JSON: {err}"))?;
    write_text(path, rendered.as_bytes())
}
fn reputation_provider_table(value: &Value) -> Result<String, String> {
    let provider = value
        .get("provider")
        .and_then(Value::as_object)
        .ok_or_else(|| "reputation provider response is missing `provider` object".to_string())?;
    let proof = value
        .get("proof")
        .and_then(Value::as_object)
        .ok_or_else(|| "reputation provider response is missing `proof` object".to_string())?;
    let provider_id = provider
        .get("provider_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "reputation provider response is missing `provider.provider_id`".to_string()
        })?;
    let score_bps = provider
        .get("score_bps")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "reputation provider response is missing `provider.score_bps`".to_string()
        })?;
    let leaf_index = proof
        .get("leaf_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| "reputation provider response is missing `proof.leaf_index`".to_string())?;
    let leaf_count = proof
        .get("leaf_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "reputation provider response is missing `proof.leaf_count`".to_string())?;
    if leaf_count == 0 || leaf_index >= leaf_count {
        return Err("reputation provider response has invalid proof leaf geometry".to_string());
    }
    let sibling_count = proof
        .get("siblings_hex")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let merkle_root = value
        .get("merkle_root_hex")
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(format!(
        "provider_id\tscore_bps\tleaf_index\tleaf_count\tproof_siblings\tmerkle_root_hex\n{provider_id}\t{score_bps}\t{leaf_index}\t{leaf_count}\t{sibling_count}\t{merkle_root}"
    ))
}
fn reputation_verify(raw_args: Vec<String>) -> Result<(), String> {
    const CONTEXT: &str = "sorafs_cli reputation verify";
    let mut snapshot_path: Option<PathBuf> = None;
    let mut provider_id: Option<String> = None;
    let mut proof_path: Option<PathBuf> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut seen_options = BTreeSet::new();
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument for `{CONTEXT}`"))?;
        reject_duplicate_reputation_option(&mut seen_options, key, CONTEXT)?;
        match key {
            "--snapshot" => snapshot_path = Some(PathBuf::from(value)),
            "--provider-id" => provider_id = Some(value.to_string()),
            "--proof" => proof_path = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli reputation verify`"
                ));
            }
        }
    }
    let snapshot_path = snapshot_path.ok_or_else(|| {
        "missing required `--snapshot=PATH` for `sorafs_cli reputation verify`".to_string()
    })?;
    let provider_id = provider_id
        .map(|value| parse_reputation_provider_id(&value))
        .transpose()?;
    if provider_id.is_some() != proof_path.is_some() {
        return Err(
            "`--provider-id=ID` and `--proof=PATH` must be supplied together for reputation proof verification"
                .to_string(),
        );
    }
    let snapshot = read_reputation_snapshot(&snapshot_path)?;
    let mut summary = Map::new();
    insert_value!(summary["snapshot_path"] = snapshot_path.display().to_string());
    insert_value!(summary["snapshot_id_hex"] = hex_encode(snapshot.snapshot_id));
    insert_value!(summary["generated_at_unix"] = snapshot.generated_at_unix);
    if let Some(previous_snapshot_id) = snapshot.previous_snapshot_id {
        insert_value!(summary["previous_snapshot_id_hex"] = hex_encode(previous_snapshot_id));
    }
    insert_value!(summary["provider_count"] = snapshot.providers.len() as u64);
    insert_value!(summary["merkle_root_hex"] = hex_encode(snapshot.merkle_root));
    insert_value!(summary["alpha_bps"] = u64::from(snapshot.alpha_bps));
    insert_value!(
        summary["current_score_weight_bps"] = u64::from(snapshot.current_score_weight_bps)
    );
    insert_value!(summary["valid"] = true);
    if let (Some(provider_id), Some(proof_path)) = (provider_id, proof_path) {
        let provider = snapshot
            .providers
            .iter()
            .find(|entry| entry.provider_id == provider_id)
            .ok_or_else(|| {
                "requested provider was not found in the reputation snapshot".to_owned()
            })?;
        let proof_bytes = fs::read(&proof_path).map_err(|err| {
            format!(
                "failed to read reputation proof `{}`: {err}",
                proof_path.display()
            )
        })?;
        let proof: ReputationMerkleProofV1 = decode_from_bytes(&proof_bytes)
            .map_err(|_| "failed to decode reputation proof".to_owned())?;
        proof
            .verify(provider, snapshot.merkle_root)
            .map_err(|_| "invalid reputation proof".to_owned())?;
        insert_value!(summary["provider_id"] = provider.provider_id.clone());
        insert_value!(summary["provider_score_bps"] = u64::from(provider.score_bps));
        insert_value!(summary["proof_path"] = proof_path.display().to_string());
        insert_value!(summary["proof_leaf_index"] = u64::from(proof.leaf_index));
        insert_value!(summary["proof_sibling_count"] = proof.siblings.len() as u64);
        insert_value!(summary["proof_verified"] = true);
    }
    let summary_value = Value::Object(summary);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render reputation summary: {err}"))?;
    println!("{rendered}");
    if let Some(path) = summary_out {
        write_text(&path, rendered.as_bytes())?;
    }
    Ok(())
}
const PROOF_STREAM_ROUTE_V1: &str = "/v1/sorafs/proof/stream";
const PIN_MANIFEST_ROUTE_PREFIX_V1: &str = "/v1/sorafs/pin/";
const PIN_MANIFEST_RESPONSE_MAX_BYTES: u64 = 1024 * 1024;
const PROOF_STREAM_HTTP_TIMEOUT: Duration = Duration::from_secs(120);
const PROOF_STREAM_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PROOF_STREAM_BEARER_TOKEN_MAX_BYTES: usize = 8 * 1024;
#[derive(Clone, Copy, Debug)]
struct ValidatedPinManifestV1 {
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
    por_root: [u8; 32],
}
fn proof_stream_endpoint(
    torii_url: Option<&str>,
    gateway_url: Option<&str>,
) -> Result<Url, String> {
    let (flag, raw, exact_stream_path) = match (torii_url, gateway_url) {
        (Some(_), Some(_)) => {
            return Err("`--torii-url` cannot be combined with `--gateway-url`".to_string());
        }
        (Some(raw), None) => ("--torii-url", raw, false),
        (None, Some(raw)) => ("--gateway-url", raw, true),
        (None, None) => {
            return Err(
                "missing required `--torii-url=URL` (or `--gateway-url=URL`) for `sorafs_cli proof stream`"
                    .to_string(),
            );
        }
    };
    if raw.is_empty() {
        return Err(format!("`{flag}` must not be empty"));
    }
    if raw.trim() != raw {
        return Err(format!("`{flag}` must not contain surrounding whitespace"));
    }
    let mut parsed = Url::parse(raw).map_err(|_| format!("invalid `{flag}` URL"))?;
    if parsed.scheme() != "https" {
        return Err(format!("`{flag}` must use HTTPS"));
    }
    if parsed.host_str().is_none() {
        return Err(format!("`{flag}` must include a host"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(format!("`{flag}` must not include URL userinfo"));
    }
    if parsed.query().is_some() {
        return Err(format!("`{flag}` must not include a query"));
    }
    if parsed.fragment().is_some() {
        return Err(format!("`{flag}` must not include a fragment"));
    }
    if parsed.port() == Some(0) {
        return Err(format!("`{flag}` must not use port zero"));
    }
    let canonical_origin = parsed.origin().ascii_serialization();
    if exact_stream_path {
        let canonical_endpoint = format!("{canonical_origin}{PROOF_STREAM_ROUTE_V1}");
        if parsed.path() != PROOF_STREAM_ROUTE_V1 || raw != canonical_endpoint {
            return Err(format!(
                "`--gateway-url` must be the exact canonical HTTPS origin plus `{PROOF_STREAM_ROUTE_V1}`"
            ));
        }
    } else {
        let canonical_origin_with_slash = format!("{canonical_origin}/");
        if parsed.path() != "/" || (raw != canonical_origin && raw != canonical_origin_with_slash) {
            return Err(
                "`--torii-url` must be an exact canonical bare HTTPS origin without a path prefix"
                    .to_string(),
            );
        }
        parsed.set_path(PROOF_STREAM_ROUTE_V1);
    }
    Ok(parsed)
}
fn proof_stream_pin_manifest_endpoint(stream_endpoint: &Url, manifest_digest_hex: &str) -> Url {
    let mut endpoint = stream_endpoint.clone();
    endpoint.set_path(&format!(
        "{PIN_MANIFEST_ROUTE_PREFIX_V1}{manifest_digest_hex}"
    ));
    endpoint
}
fn redacted_endpoint(endpoint: &Url) -> String {
    format!(
        "{}{}",
        endpoint.origin().ascii_serialization(),
        endpoint.path()
    )
}
fn is_canonical_proof_stream_bearer_token(token: &str) -> bool {
    if token.is_empty()
        || token.len() > PROOF_STREAM_BEARER_TOKEN_MAX_BYTES
        || token.trim() != token
    {
        return false;
    }
    let mut saw_padding = false;
    let mut saw_token_byte = false;
    for byte in token.bytes() {
        if byte == b'=' {
            saw_padding = true;
            continue;
        }
        if saw_padding
            || !matches!(
                byte,
                b'A'..=b'Z'
                    | b'a'..=b'z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'.'
                    | b'_'
                    | b'~'
                    | b'+'
                    | b'/'
            )
        {
            return false;
        }
        saw_token_byte = true;
    }
    saw_token_byte
}
fn proof_stream_bearer_token_from_env(env_name: &str) -> Result<String, String> {
    let name_bytes = env_name.as_bytes();
    if name_bytes.is_empty()
        || !matches!(name_bytes[0], b'A'..=b'Z' | b'_')
        || !name_bytes
            .iter()
            .all(|byte| matches!(byte, b'A'..=b'Z' | b'0'..=b'9' | b'_'))
    {
        return Err(
            "`--bearer-token-env` must name an uppercase ASCII environment variable".to_string(),
        );
    }
    let token = env::var(env_name).map_err(|_| {
        format!("failed to read bearer token from environment variable `{env_name}`")
    })?;
    if !is_canonical_proof_stream_bearer_token(&token) {
        return Err(format!(
            "environment variable `{env_name}` is not a canonical bearer token"
        ));
    }
    Ok(token)
}
fn proof_stream_http_client() -> Result<HttpClient, String> {
    HttpClient::builder()
        .https_only(true)
        .connect_timeout(PROOF_STREAM_HTTP_CONNECT_TIMEOUT)
        .timeout(PROOF_STREAM_HTTP_TIMEOUT)
        .redirect(RedirectPolicy::none())
        .referer(false)
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .pool_max_idle_per_host(1)
        .build()
        .map_err(|error| format!("failed to construct hardened proof-stream HTTP client: {error}"))
}
fn fetch_finalized_pin_manifest(
    client: &HttpClient,
    endpoint: &Url,
    bearer_token: &str,
) -> Result<PinManifestFinalizedRecordV1, String> {
    let endpoint_label = redacted_endpoint(endpoint);
    let response = client
        .get(endpoint.clone())
        .bearer_auth(bearer_token)
        .header("Accept", "application/json")
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .map_err(|_| format!("failed to fetch finalized pin manifest from `{endpoint_label}`"))?;
    if response.status() != StatusCode::OK {
        return Err(format!(
            "finalized pin-manifest endpoint `{endpoint_label}` returned {}",
            response.status()
        ));
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    if content_type != Some("application/json") {
        return Err(
            "finalized pin-manifest endpoint returned a noncanonical Content-Type; expected `application/json`"
                .to_string(),
        );
    }
    let content_encoding = response
        .headers()
        .get(CONTENT_ENCODING)
        .map(|value| value.to_str())
        .transpose()
        .map_err(|_| {
            "finalized pin-manifest endpoint returned a non-ASCII Content-Encoding".to_string()
        })?;
    if !matches!(content_encoding, None | Some("identity")) {
        return Err(
            "finalized pin-manifest endpoint ignored `Accept-Encoding: identity`".to_string(),
        );
    }
    if response
        .content_length()
        .is_some_and(|length| length > PIN_MANIFEST_RESPONSE_MAX_BYTES)
    {
        return Err("finalized pin-manifest response exceeds the size limit".to_string());
    }
    let mut body = Vec::new();
    response
        .take(PIN_MANIFEST_RESPONSE_MAX_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|_| "failed to read finalized pin-manifest response".to_string())?;
    if body.len() as u64 > PIN_MANIFEST_RESPONSE_MAX_BYTES {
        return Err("finalized pin-manifest response exceeds the size limit".to_string());
    }
    from_slice(&body)
        .map_err(|_| "failed to decode finalized native pin-manifest record".to_string())
}
fn validate_finalized_pin_manifest(
    local_manifest: &ManifestV1,
    local_manifest_digest: &[u8; 32],
    finalized: &PinManifestFinalizedRecordV1,
) -> Result<ValidatedPinManifestV1, String> {
    let record = &finalized.manifest;
    if !matches!(record.status, PinStatus::Approved(_)) {
        return Err("pin manifest is not in the chain-authoritative Approved state".to_string());
    }
    if record.digest.as_bytes() != local_manifest_digest {
        return Err("pin-manifest digest does not match the local canonical manifest".to_string());
    }
    if record.root_cid.as_bytes().as_slice() != local_manifest.root_cid.as_slice() {
        return Err(
            "pin-manifest root CID does not match the local canonical manifest".to_string(),
        );
    }
    if record.chunker != chunker_handle_from_profile(&local_manifest.chunking) {
        return Err("pin-manifest chunker does not match the local canonical manifest".to_string());
    }
    if record.chunk_digest_sha3_256 != local_manifest.chunk_digest_sha3_256 {
        return Err(
            "pin-manifest chunk-plan digest does not match the local canonical manifest"
                .to_string(),
        );
    }
    if record.por_root != local_manifest.por_root {
        return Err(
            "pin-manifest PoR root does not match the local canonical manifest".to_string(),
        );
    }
    if record.content_length != local_manifest.content_length {
        return Err(
            "pin-manifest content length does not match the local canonical manifest".to_string(),
        );
    }
    if record.policy != convert_pin_policy(&local_manifest.pin_policy) {
        return Err("pin-manifest policy does not match the local canonical manifest".to_string());
    }
    if finalized.finalized_cursor.height == 0 {
        return Err("pin-manifest finalized cursor height must be non-zero".to_string());
    }
    if finalized
        .finalized_cursor
        .block_hash
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err("pin-manifest finalized cursor hash must be non-zero".to_string());
    }
    Ok(ValidatedPinManifestV1 {
        finalized_height: finalized.finalized_cursor.height,
        finalized_block_hash: finalized.finalized_cursor.block_hash,
        por_root: record.por_root,
    })
}
fn payload_free_proof_stream_event(item: &ProofStreamItem) -> Value {
    let mut map = Map::new();
    insert_value!(map["request_digest_hex"] = item.request_digest_hex());
    insert_value!(map["manifest_digest_hex"] = item.manifest_digest_hex());
    insert_value!(map["provider_id_hex"] = item.provider_id_hex());
    insert_value!(map["proof_kind"] = item.proof_kind().as_str());
    insert_value!(map["result"] = item.status().as_str());
    if let Some(value) = item.outcome_identity_hex() {
        insert_value!(map["outcome_identity_hex"] = value);
    }
    if let Some(value) = item.outcome_digest_hex() {
        insert_value!(map["outcome_digest_hex"] = value);
    }
    if let Some(value) = item.admission_envelope_digest_hex() {
        insert_value!(map["admission_envelope_digest_hex"] = value);
    }
    if let Some(value) = item.finalized_block_height() {
        insert_value!(map["finalized_block_height"] = value);
    }
    if let Some(value) = item.finalized_block_hash_hex() {
        insert_value!(map["finalized_block_hash_hex"] = value);
    }
    if let Some(value) = item.committed_at_ms() {
        insert_value!(map["committed_at_ms"] = value);
    }
    if let Some(value) = item.challenge_id_hex() {
        insert_value!(map["challenge_id_hex"] = value);
    }
    if let Some(value) = item.failure_reason() {
        insert_value!(map["failure_reason"] = value);
    }
    if let Some(value) = item.latency_ms() {
        insert_value!(map["latency_ms"] = u64::from(value));
    }
    if let Some(value) = item.deadline_ms() {
        insert_value!(map["deadline_ms"] = u64::from(value));
    }
    if let Some(value) = item.sample_index() {
        insert_value!(map["leaf_index_flat"] = value);
    }
    if let Some(value) = item.chunk_index() {
        insert_value!(map["chunk_index"] = u64::from(value));
    }
    if let Some(value) = item.segment_index() {
        insert_value!(map["segment_index"] = u64::from(value));
    }
    if let Some(value) = item.leaf_index() {
        insert_value!(map["leaf_index"] = u64::from(value));
    }
    if let Some(value) = item.tier() {
        insert_value!(map["tier"] = value.as_str());
    }
    if let Some(value) = item.recorded_at_ms() {
        insert_value!(map["recorded_at_ms"] = value);
    }
    Value::Object(map)
}
fn proof_stream(raw_args: Vec<String>) -> Result<(), String> {
    let mut manifest_path: Option<PathBuf> = None;
    let mut torii_url: Option<String> = None;
    let mut endpoint_url: Option<String> = None;
    let mut provider_id_hex: Option<String> = None;
    let mut proof_kind_arg: Option<String> = None;
    let mut challenge_id_hex: Option<String> = None;
    let mut samples: Option<u32> = None;
    let mut sample_seed: Option<u64> = None;
    let mut deadline_ms: Option<u32> = None;
    let mut tier_arg: Option<String> = None;
    let mut nonce_b64: Option<String> = None;
    let mut orchestrator_job_id_hex: Option<String> = None;
    let mut bearer_token_env: Option<String> = None;
    let mut summary_out: Option<PathBuf> = None;
    let mut evidence_dir: Option<PathBuf> = None;
    let mut emit_events = false;
    let mut seen_options = BTreeSet::new();
    for arg in raw_args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value argument, got `{arg}`"))?;
        if !seen_options.insert(key.to_string()) {
            return Err(format!(
                "duplicate option `{key}` for `sorafs_cli proof stream`"
            ));
        }
        match key {
            "--manifest" => manifest_path = Some(PathBuf::from(value)),
            "--torii-url" => torii_url = Some(value.to_string()),
            "--gateway-url" => endpoint_url = Some(value.to_string()),
            "--provider-id-hex" => provider_id_hex = Some(value.to_string()),
            "--proof-kind" => proof_kind_arg = Some(value.to_string()),
            "--challenge-id-hex" => challenge_id_hex = Some(value.to_string()),
            "--samples" => {
                samples = Some(parse_u32_arg(
                    "--samples",
                    value,
                    "sorafs_cli proof stream",
                )?);
            }
            "--sample-seed" => {
                sample_seed = Some(parse_u64_arg(
                    "--sample-seed",
                    value,
                    "sorafs_cli proof stream",
                )?);
            }
            "--deadline-ms" => {
                deadline_ms = Some(parse_u32_arg(
                    "--deadline-ms",
                    value,
                    "sorafs_cli proof stream",
                )?);
            }
            "--tier" => tier_arg = Some(value.to_string()),
            "--nonce-b64" => nonce_b64 = Some(value.to_string()),
            "--orchestrator-job-id-hex" => orchestrator_job_id_hex = Some(value.to_string()),
            "--bearer-token-env" => bearer_token_env = Some(value.to_string()),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--governance-evidence-dir" => evidence_dir = Some(PathBuf::from(value)),
            "--emit-events" => emit_events = parse_bool_flag(value, "--emit-events")?,
            _ => {
                return Err(format!(
                    "unrecognised option `{key}` for `sorafs_cli proof stream`"
                ));
            }
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| {
        "missing required `--manifest=PATH` for `sorafs_cli proof stream`".to_string()
    })?;
    let endpoint = proof_stream_endpoint(torii_url.as_deref(), endpoint_url.as_deref())?;
    let provider_id = if let Some(raw_hex) = provider_id_hex {
        let bytes = parse_digest_hex(&raw_hex)
            .map_err(|err| format!("invalid `--provider-id-hex` value: {err}"))?;
        let canonical = hex_encode(bytes);
        if raw_hex != canonical {
            return Err(
                "`--provider-id-hex` must be exact 64-character lowercase hexadecimal".to_string(),
            );
        }
        canonical
    } else {
        return Err(
            "missing required `--provider-id-hex=HEX32` for `sorafs_cli proof stream`".to_string(),
        );
    };
    let proof_kind = proof_kind_arg
        .as_deref()
        .map(|raw| {
            ProofKind::parse(raw)
                .map_err(|_| "unsupported proof kind; expected `por`, `pdp`, or `potr`".to_string())
        })
        .transpose()?
        .unwrap_or_default();
    let deadline_ms_arg = deadline_ms;
    let challenge_id_hex = challenge_id_hex
        .map(|value| {
            let bytes = parse_digest_hex(&value)
                .map_err(|err| format!("invalid `--challenge-id-hex` value: {err}"))?;
            let canonical = hex_encode(bytes);
            if value != canonical {
                return Err(
                    "`--challenge-id-hex` must be exact 64-character lowercase hexadecimal"
                        .to_string(),
                );
            }
            if bytes.iter().all(|byte| *byte == 0) {
                return Err("`--challenge-id-hex` must be non-zero".to_string());
            }
            Ok(canonical)
        })
        .transpose()?;
    let (challenge_id_hex, sample_count, deadline_ms) = match proof_kind {
        ProofKind::Por => {
            if challenge_id_hex.is_some() {
                return Err(
                    "`--challenge-id-hex` may only be used with `--proof-kind=pdp`".to_string(),
                );
            }
            if deadline_ms_arg.is_some() {
                return Err("`--deadline-ms` may only be used with `--proof-kind=potr`".to_string());
            }
            let count = samples.unwrap_or(32);
            if count == 0 {
                return Err("`--samples` must be greater than zero".to_string());
            }
            if count > MAX_PROOF_STREAM_SAMPLE_COUNT {
                return Err(format!(
                    "`--samples` must not exceed {MAX_PROOF_STREAM_SAMPLE_COUNT}"
                ));
            }
            (None, Some(count), None)
        }
        ProofKind::Pdp => {
            let challenge_id = challenge_id_hex.ok_or_else(|| {
                "`--challenge-id-hex=HEX32` is required when `--proof-kind=pdp`".to_string()
            })?;
            if deadline_ms_arg.is_some() {
                return Err("`--deadline-ms` may only be used with `--proof-kind=potr`".to_string());
            }
            if samples.is_some() {
                return Err(
                    "`--samples` is not supported for `--proof-kind=pdp`; sampling is fixed by the governed challenge"
                        .to_string(),
                );
            }
            if sample_seed.is_some() {
                return Err(
                    "`--sample-seed` is not supported for `--proof-kind=pdp`; sampling is fixed by the governed challenge"
                        .to_string(),
                );
            }
            (Some(challenge_id), None, None)
        }
        ProofKind::Potr => {
            if challenge_id_hex.is_some() {
                return Err(
                    "`--challenge-id-hex` may only be used with `--proof-kind=pdp`".to_string(),
                );
            }
            if samples.is_some() {
                return Err("`--samples` is not supported for `--proof-kind=potr`".to_string());
            }
            if sample_seed.is_some() {
                return Err("`--sample-seed` is only supported for `--proof-kind=por`".to_string());
            }
            let deadline = deadline_ms_arg.ok_or_else(|| {
                "`--deadline-ms` is required when `--proof-kind=potr`".to_string()
            })?;
            if deadline == 0 {
                return Err("`--deadline-ms` must be greater than zero".to_string());
            }
            (None, None, Some(deadline))
        }
    };
    let tier = tier_arg
        .as_deref()
        .map(|raw| {
            ProofTier::parse(raw).map_err(|_| {
                "unsupported proof tier; expected `hot`, `warm`, or `archive`".to_string()
            })
        })
        .transpose()?;
    let orchestrator_job_id_hex = orchestrator_job_id_hex
        .map(|value| {
            let bytes = parse_fixed_hex_bytes::<16>(&value, "--orchestrator-job-id-hex")?;
            if bytes.iter().all(|byte| *byte == 0) {
                return Err("`--orchestrator-job-id-hex` must be non-zero".to_string());
            }
            Ok(hex_encode(bytes))
        })
        .transpose()?;
    if matches!(proof_kind, ProofKind::Potr) && orchestrator_job_id_hex.is_none() {
        return Err(
            "`--orchestrator-job-id-hex=HEX16` is required when `--proof-kind=potr`".to_string(),
        );
    }
    let manifest_byte_limit = u64::try_from(MAX_MANIFEST_ENCODED_BYTES)
        .map_err(|_| "manifest byte limit does not fit u64".to_string())?;
    let manifest_bytes = read_file_bounded(&manifest_path, manifest_byte_limit, "manifest")?;
    let manifest = decode_manifest_v1_canonical(&manifest_bytes)
        .map_err(|err| format!("failed to decode exact canonical manifest: {err}"))?;
    if matches!(proof_kind, ProofKind::Por) && manifest.por_root == [0; 32] {
        return Err(
            "PoR proof streaming requires a non-zero `por_root` in the canonical manifest"
                .to_string(),
        );
    }
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let manifest_digest_hex = hex_encode(manifest_digest.as_bytes());
    let manifest_cid_hex = hex_encode(&manifest.root_cid);
    let bearer_token_env = bearer_token_env.ok_or_else(|| {
        "missing required `--bearer-token-env=VAR` for authenticated `sorafs_cli proof stream`"
            .to_string()
    })?;
    let bearer_token = proof_stream_bearer_token_from_env(&bearer_token_env)?;
    let client = proof_stream_http_client()?;
    let pin_manifest_endpoint = proof_stream_pin_manifest_endpoint(&endpoint, &manifest_digest_hex);
    let finalized_pin =
        fetch_finalized_pin_manifest(&client, &pin_manifest_endpoint, &bearer_token)?;
    let validated_pin =
        validate_finalized_pin_manifest(&manifest, manifest_digest.as_bytes(), &finalized_pin)?;
    let trusted_por_root = match proof_kind {
        ProofKind::Por => Some(validated_pin.por_root),
        ProofKind::Pdp | ProofKind::Potr => None,
    };
    let nonce = if let Some(encoded) = nonce_b64 {
        decode_nonce_b64(&encoded)?
    } else {
        generate_proof_stream_nonce(
            manifest_digest.as_bytes(),
            proof_kind,
            challenge_id_hex.as_deref(),
            sample_count,
            deadline_ms,
            Some(&provider_id),
        )
    };
    let request_model = ProofStreamRequestV1 {
        manifest_digest: *manifest_digest.as_bytes(),
        provider_id: parse_digest_hex(&provider_id)
            .map_err(|error| format!("invalid canonical provider id: {error}"))?,
        proof_kind,
        challenge_id: challenge_id_hex
            .as_deref()
            .map(parse_digest_hex)
            .transpose()
            .map_err(|error| format!("invalid canonical challenge id: {error}"))?,
        sample_count,
        deadline_ms,
        sample_seed,
        expected_finalized_height: Some(validated_pin.finalized_height),
        expected_finalized_block_hash: Some(validated_pin.finalized_block_hash),
        nonce,
        orchestrator_job_id: orchestrator_job_id_hex
            .as_deref()
            .map(|raw| parse_fixed_hex_bytes::<16>(raw, "orchestrator job id"))
            .transpose()?,
        tier,
    };
    let verification_context = ProofStreamVerificationContext::new(request_model, trusted_por_root)
        .map_err(|error| format!("invalid proof stream verification context: {error}"))?;
    let request = ProofStreamHttpRequestV1::new(request_model)
        .map_err(|error| format!("invalid proof stream request: {error}"))?;
    let request_body = to_vec(&request)
        .map_err(|error| format!("failed to encode canonical proof stream request: {error}"))?;
    let builder = client
        .post(endpoint.clone())
        .bearer_auth(&bearer_token)
        .header(CONTENT_TYPE, "application/json")
        .header("Accept", "application/x-ndjson")
        .header(ACCEPT_ENCODING, "identity");
    let response = builder
        .body(request_body)
        .send()
        .map_err(|err| format!("failed to initiate proof stream: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("gateway returned {status} when streaming proofs"));
    }
    let response_content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    if response_content_type != Some("application/x-ndjson") {
        return Err(
            "gateway returned a noncanonical proof stream Content-Type; expected `application/x-ndjson`"
                .to_string(),
        );
    }
    let response_content_encoding = response
        .headers()
        .get(CONTENT_ENCODING)
        .map(|value| value.to_str())
        .transpose()
        .map_err(|_| "gateway returned a non-ASCII proof stream Content-Encoding".to_string())?;
    if !matches!(response_content_encoding, None | Some("identity")) {
        return Err(
            "gateway ignored `Accept-Encoding: identity` for the proof stream response".to_string(),
        );
    }
    let reader = BufReader::new(response);
    let mut metrics = ProofStreamMetrics::default();
    let mut pending_events = Vec::new();
    for item in ProofStreamNdjsonReader::new(reader, &verification_context) {
        let item =
            item.map_err(|err| format!("gateway returned an invalid proof stream: {err}"))?;
        metrics.record(&item);
        if emit_events {
            let event = norito::json::to_string(&payload_free_proof_stream_event(&item))
                .map_err(|err| format!("failed to encode proof stream event: {err}"))?;
            pending_events.push(event);
        }
    }
    if metrics.item_total == 0 {
        return Err("gateway returned an empty proof stream".to_string());
    }
    if metrics.failure_total != 0 {
        return Err(format!(
            "proof stream reported {} gateway failures; V1 promotion evidence requires zero",
            metrics.failure_total
        ));
    }
    let mut summary_map = Map::new();
    let endpoint_label = redacted_endpoint(&endpoint);
    insert_value!(summary_map["endpoint"] = endpoint_label.clone());
    insert_value!(summary_map["manifest_path"] = manifest_path.display().to_string());
    insert_value!(summary_map["manifest_digest_hex"] = manifest_digest_hex.clone());
    insert_value!(summary_map["manifest_cid_hex"] = manifest_cid_hex.clone());
    insert_value!(summary_map["provider_id_hex"] = provider_id.clone());
    insert_value!(summary_map["proof_kind"] = proof_kind.as_str());
    insert_value!(
        summary_map["request_digest_hex"] = hex_encode(verification_context.request_digest())
    );
    insert_value!(summary_map["finalized_block_height"] = validated_pin.finalized_height);
    insert_value!(
        summary_map["finalized_block_hash_hex"] = hex_encode(validated_pin.finalized_block_hash)
    );
    if let Some(challenge_id) = challenge_id_hex {
        insert_value!(summary_map["requested_challenge_id_hex"] = challenge_id);
    }
    if let Some(count) = sample_count {
        insert_value!(summary_map["requested_sample_count"] = u64::from(count));
    }
    if let Some(seed) = sample_seed {
        insert_value!(summary_map["requested_sample_seed"] = seed);
    }
    if let Some(deadline) = deadline_ms {
        insert_value!(summary_map["requested_deadline_ms"] = u64::from(deadline));
    }
    if let Some(tier) = tier {
        insert_value!(summary_map["requested_tier"] = tier.as_str());
    }
    if let Some(job_id) = orchestrator_job_id_hex {
        insert_value!(summary_map["requested_orchestrator_job_id_hex"] = job_id);
    }
    insert_value!(summary_map["nonce_digest_hex"] = hex_encode(blake3_hash(&nonce).as_bytes()));
    insert_json!(summary_map["metrics"] = metrics.to_json());
    if let Some(root) = trusted_por_root.as_ref() {
        insert_value!(summary_map["verification_root_hex"] = hex_encode(root));
        insert_value!(summary_map["verification_total"] = metrics.item_total);
        insert_value!(summary_map["verification_successes"] = metrics.item_total);
        insert_value!(summary_map["verification_failures"] = 0_u64);
    }
    let summary_value = Value::Object(summary_map);
    let rendered = to_string_pretty(&summary_value)
        .map_err(|err| format!("failed to render proof stream summary: {err}"))?;
    for event in pending_events {
        println!("{event}");
    }
    println!("{rendered}");
    if let Some(path) = summary_out {
        write_text(&path, rendered.as_bytes())?;
    }
    if let Some(dir) = evidence_dir {
        write_proof_stream_evidence(
            &dir,
            &manifest_path,
            &manifest_bytes,
            &manifest_digest_hex,
            &rendered,
            &endpoint_label,
        )?;
    }
    Ok(())
}
fn write_proof_stream_evidence(
    dir: &Path,
    manifest_path: &Path,
    manifest_bytes: &[u8],
    manifest_digest_hex: &str,
    summary_json: &str,
    endpoint: &str,
) -> Result<(), String> {
    prepare_clean_dir(dir)?;
    let summary_file_name = "proof_stream_summary.json";
    let summary_path = dir.join(summary_file_name);
    write_text(&summary_path, summary_json.as_bytes())?;
    let manifest_copy_name = manifest_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "manifest.norito".to_string());
    let manifest_copy_path = dir.join(&manifest_copy_name);
    write_bytes(&manifest_copy_path, manifest_bytes)?;
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64;
    let mut metadata = Map::new();
    insert_value!(metadata["captured_at_unix_ms"] = captured_at_ms);
    insert_value!(metadata["sorafs_cli_version"] = SORAFS_CLI_VERSION);
    insert_value!(metadata["endpoint"] = endpoint.to_string());
    insert_value!(metadata["manifest_source"] = manifest_path.display().to_string());
    insert_value!(metadata["manifest_copy"] = manifest_copy_name);
    insert_value!(metadata["manifest_digest_hex"] = manifest_digest_hex.to_string());
    insert_value!(metadata["summary_file"] = summary_file_name);
    let metadata_json = to_string_pretty(&Value::Object(metadata))
        .map_err(|err| format!("failed to render governance evidence metadata: {err}"))?;
    write_text(&dir.join("metadata.json"), metadata_json.as_bytes())?;
    Ok(())
}
fn prepare_clean_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        if !dir.is_dir() {
            return Err(format!(
                "governance evidence path `{}` must be a directory",
                dir.display()
            ));
        }
        let mut entries = fs::read_dir(dir)
            .map_err(|err| format!("failed to inspect `{}`: {err}", dir.display()))?;
        if entries.next().is_some() {
            return Err(format!(
                "governance evidence directory `{}` must be empty",
                dir.display()
            ));
        }
    } else {
        fs::create_dir_all(dir)
            .map_err(|err| format!("failed to create `{}`: {err}", dir.display()))?;
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GovernanceDagOutputFormat {
    Table,
    Json,
}
impl GovernanceDagOutputFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unsupported governance DAG output format `{other}`; expected table|json"
            )),
        }
    }
}
#[derive(Debug, Clone)]
struct GovernanceDagArtifact {
    path: PathBuf,
    rel_path: String,
    encoded_len: u64,
    blake3_hex: String,
    sidecar_status: String,
    sidecar_value: Option<String>,
    sidecar_error: Option<String>,
    node: Option<GovernanceDagNodeSummary>,
    decode_error: Option<String>,
    outcome: Option<ValidationOutcomeV1>,
}
#[derive(Debug, Clone)]
struct GovernanceDagNodeSummary {
    node_cid: Vec<u8>,
    node_cid_label: String,
    node_cid_hex: String,
    prev_cid: Option<Vec<u8>>,
    prev_cid_label: Option<String>,
    prev_cid_hex: Option<String>,
    timestamp: u64,
    publisher_peer_id: String,
    submission_publisher_account_digest_hex: Option<String>,
    submission_origin: Option<&'static str>,
    payload_kind: &'static str,
}
fn governance_submission_summary(
    node: &GovernanceLogNodeV1,
) -> (Option<String>, Option<&'static str>) {
    node.submission_provenance
        .as_ref()
        .map_or((None, None), |provenance| {
            (
                Some(hex_encode(provenance.publisher_account_digest)),
                Some(provenance.origin.label()),
            )
        })
}
fn insert_governance_submission_summary(
    object: &mut Map,
    publisher_account_digest_hex: Option<&str>,
    origin: Option<&str>,
) {
    insert_json!(
        object["submission_publisher_account_digest_hex"] =
            publisher_account_digest_hex.map_or(Value::Null, Value::from)
    );
    insert_json!(object["submission_origin"] = origin.map_or(Value::Null, Value::from));
}
fn insert_governance_node_submission_summary(object: &mut Map, node: &GovernanceLogNodeV1) {
    let (publisher_account_digest_hex, origin) = governance_submission_summary(node);
    insert_governance_submission_summary(object, publisher_account_digest_hex.as_deref(), origin);
}
#[derive(Debug, Clone)]
struct GovernanceDagVerifyOptions {
    require_chain: bool,
    require_sidecars: bool,
    expected_head_cid: Option<Vec<u8>>,
}
fn governance_dag_list(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut format = GovernanceDagOutputFormat::Table;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--format" => format = GovernanceDagOutputFormat::parse(value)?,
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag list`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag list`".to_string()
    })?;
    let artifacts = load_governance_dag_inventory(&root)?;
    let summary = governance_dag_inventory_value(&root, &artifacts);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary)?;
    }
    match format {
        GovernanceDagOutputFormat::Json => print_governance_dag_json(&summary),
        GovernanceDagOutputFormat::Table => {
            print_governance_dag_inventory_table(&root, &artifacts);
            Ok(())
        }
    }
}
fn governance_dag_show(raw_args: Vec<String>) -> Result<(), String> {
    let mut node: Option<PathBuf> = None;
    let mut format = GovernanceDagOutputFormat::Table;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--node" => node = Some(PathBuf::from(value)),
            "--format" => format = GovernanceDagOutputFormat::parse(value)?,
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag show`"
                ));
            }
        }
    }
    let node = node.ok_or_else(|| {
        "missing required `--node=PATH` for `sorafs_cli governance dag show`".to_string()
    })?;
    let root = node.parent().unwrap_or_else(|| Path::new("."));
    let artifact = read_governance_dag_artifact(root, &node)?;
    let value = governance_dag_artifact_value(&artifact, true);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &value)?;
    }
    match format {
        GovernanceDagOutputFormat::Json => print_governance_dag_json(&value),
        GovernanceDagOutputFormat::Table => {
            print_governance_dag_artifact_table(&artifact);
            Ok(())
        }
    }
}
fn governance_dag_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut require_chain = false;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        match arg.as_str() {
            "--require-chain" => {
                require_chain = true;
                continue;
            }
            "--require-sidecars" => {
                require_sidecars = true;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag verify`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag verify`".to_string()
    })?;
    let artifacts = load_governance_dag_inventory(&root)?;
    let (ok, summary, _) = verify_governance_dag_inventory(
        &root,
        &artifacts,
        &GovernanceDagVerifyOptions {
            require_chain,
            require_sidecars,
            expected_head_cid: head_cid,
        },
    );
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary)?;
    }
    print_governance_dag_json(&summary)?;
    if ok {
        Ok(())
    } else {
        Err("governance DAG verification failed".to_string())
    }
}
fn governance_dag_export(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut require_chain = false;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        match arg.as_str() {
            "--require-chain" => {
                require_chain = true;
                continue;
            }
            "--require-sidecars" => {
                require_sidecars = true;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag export`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag export`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=DIR` for `sorafs_cli governance dag export`".to_string()
    })?;
    let artifacts = load_governance_dag_inventory(&root)?;
    let (ok, verify_summary, node_indices) = verify_governance_dag_inventory(
        &root,
        &artifacts,
        &GovernanceDagVerifyOptions {
            require_chain,
            require_sidecars,
            expected_head_cid: head_cid,
        },
    );
    if !ok {
        print_governance_dag_json(&verify_summary)?;
        return Err("governance DAG export refused invalid archive".to_string());
    }
    prepare_clean_dir(&out)?;
    let nodes_root = out.join("nodes");
    let mut exported_files = Vec::new();
    for index in node_indices {
        let artifact = &artifacts[index];
        let target = nodes_root.join(&artifact.rel_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create `{}`: {err}", parent.display()))?;
        }
        fs::copy(&artifact.path, &target).map_err(|err| {
            format!(
                "failed to copy governance node `{}` to `{}`: {err}",
                artifact.path.display(),
                target.display()
            )
        })?;
        let sidecar_path = target.with_extension("to.blake3");
        let mut sidecar = artifact.blake3_hex.clone();
        sidecar.push('\n');
        write_text(&sidecar_path, sidecar.as_bytes())?;
        let mut file = Map::new();
        insert_value!(file["path"] = format!("nodes/{}", artifact.rel_path));
        insert_value!(file["blake3"] = artifact.blake3_hex.clone());
        insert_value!(file["encoded_len"] = artifact.encoded_len);
        if let Some(node) = &artifact.node {
            insert_value!(file["node_cid"] = node.node_cid_label.clone());
            insert_value!(file["payload_kind"] = node.payload_kind);
            insert_governance_submission_summary(
                &mut file,
                node.submission_publisher_account_digest_hex.as_deref(),
                node.submission_origin,
            );
        }
        exported_files.push(Value::Object(file));
    }
    let mut manifest = Map::new();
    insert_value!(manifest["schema"] = "sorafs.governance_dag.export.v1");
    insert_value!(manifest["generated_at"] = governance_dag_now_secs());
    insert_value!(manifest["source_root"] = root.display().to_string());
    insert_json!(manifest["verification"] = verify_summary);
    insert_json!(manifest["files"] = Value::Array(exported_files));
    let manifest_value = Value::Object(manifest);
    write_governance_dag_json(&out.join("manifest.json"), &manifest_value)?;
    print_governance_dag_json(&manifest_value)
}
fn governance_dag_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut publisher_peer_id: Option<Vec<u8>> = None;
    let mut key_hex: Option<String> = None;
    let mut key_path: Option<PathBuf> = None;
    let mut generated_at: Option<u64> = None;
    let mut checkpoint_cid: Option<Vec<u8>> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut car_plan_out: Option<PathBuf> = None;
    let mut car_chunker_handle: Option<String> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--publisher-peer-id" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--publisher-peer-id` must not be empty".to_string());
                }
                publisher_peer_id = Some(trimmed.as_bytes().to_vec());
            }
            "--key-hex" => key_hex = Some(value.to_string()),
            "--key" => key_path = Some(PathBuf::from(value)),
            "--generated-at" => {
                generated_at = Some(parse_u64_arg(
                    "--generated-at",
                    value,
                    "sorafs_cli governance dag build",
                )?)
            }
            "--checkpoint-cid" => checkpoint_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            "--car-out" => car_out = Some(PathBuf::from(value)),
            "--car-plan-out" => car_plan_out = Some(PathBuf::from(value)),
            "--car-chunker-handle" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--car-chunker-handle` must not be empty".to_string());
                }
                car_chunker_handle = Some(trimmed.to_string());
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag build`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag build`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=DIR` for `sorafs_cli governance dag build`".to_string()
    })?;
    if car_out.is_none() && car_plan_out.is_some() {
        return Err(
            "`--car-plan-out=PATH` requires `--car-out=PATH` for `sorafs_cli governance dag build`"
                .to_string(),
        );
    }
    if car_out.is_none() && car_chunker_handle.is_some() {
        return Err(
            "`--car-chunker-handle=HANDLE` requires `--car-out=PATH` for `sorafs_cli governance dag build`"
                .to_string(),
        );
    }
    let publisher_peer_id = publisher_peer_id.ok_or_else(|| {
        "missing required `--publisher-peer-id=ID` for `sorafs_cli governance dag build`"
            .to_string()
    })?;
    let seed = load_governance_dag_build_seed(key_hex.as_deref(), key_path.as_deref())?;
    let signing_key = SigningKey::from_bytes(&seed);
    let generated_at = generated_at.unwrap_or_else(governance_dag_now_secs);
    let artifacts = load_governance_dag_inventory(&root)?;
    let (ok, verify_summary, node_indices) = verify_governance_dag_inventory(
        &root,
        &artifacts,
        &GovernanceDagVerifyOptions {
            require_chain: false,
            require_sidecars,
            expected_head_cid: None,
        },
    );
    if !ok {
        print_governance_dag_json(&verify_summary)?;
        return Err("governance DAG build refused invalid node archive".to_string());
    }
    let ordered_indices = governance_dag_build_order(&artifacts, &node_indices);
    prepare_clean_dir(&out)?;
    let blocks_dir = out.join("blocks");
    fs::create_dir_all(&blocks_dir)
        .map_err(|err| format!("failed to create `{}`: {err}", blocks_dir.display()))?;
    let mut blocks = Vec::<GovernanceDagBlockV1>::with_capacity(ordered_indices.len());
    let mut block_files = Vec::<Value>::new();
    let mut car_files = Vec::<FileEntry>::new();
    let mut prev_block_cid: Option<Vec<u8>> = None;
    for (sequence_usize, index) in ordered_indices.iter().enumerate() {
        let artifact = &artifacts[*index];
        let node = read_governance_log_node_file(&artifact.path)?;
        let sequence = u64::try_from(sequence_usize).unwrap_or(u64::MAX);
        let timestamp = node.timestamp;
        let block_cid = governance_dag_block_cid_v1(
            prev_block_cid.as_deref(),
            sequence,
            timestamp,
            &publisher_peer_id,
            &node,
        )
        .map_err(|err| format!("failed to derive governance DAG block CID: {err}"))?;
        let mut block = GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid,
            prev_block_cid: prev_block_cid.clone(),
            sequence,
            timestamp,
            publisher_peer_id: publisher_peer_id.clone(),
            node,
            block_signature: empty_governance_dag_ed25519_signature(),
        };
        sign_governance_dag_block_cli(&mut block, &signing_key)?;
        let block_bytes = to_bytes(&block)
            .map_err(|err| format!("failed to encode governance DAG block: {err}"))?;
        let block_file_name = format!("{sequence:020}-{}.to", hex_encode(&block.block_cid));
        let block_rel_path = format!("blocks/{block_file_name}");
        let block_path = blocks_dir.join(&block_file_name);
        write_text(&block_path, &block_bytes)?;
        let block_sidecar_bytes = write_governance_blake3_sidecar(&block_path, &block_bytes)?;
        car_files.push(governance_dag_car_file(
            &block_rel_path,
            block_bytes.clone(),
        )?);
        car_files.push(governance_dag_car_file(
            &format!("{block_rel_path}.blake3"),
            block_sidecar_bytes,
        )?);
        let mut block_value = Map::new();
        insert_value!(block_value["path"] = block_rel_path);
        insert_value!(block_value["sequence"] = sequence);
        insert_value!(block_value["block_cid_hex"] = hex_encode(&block.block_cid));
        insert_json!(
            block_value["prev_block_cid_hex"] = block
                .prev_block_cid
                .as_ref()
                .map(hex_encode)
                .map_or(Value::Null, Value::from)
        );
        insert_value!(block_value["source_node_path"] = artifact.rel_path.clone());
        if let Some(node) = &artifact.node {
            insert_value!(block_value["node_cid"] = node.node_cid_label.clone());
            insert_value!(block_value["node_cid_hex"] = node.node_cid_hex.clone());
            insert_value!(block_value["payload_kind"] = node.payload_kind);
            insert_governance_submission_summary(
                &mut block_value,
                node.submission_publisher_account_digest_hex.as_deref(),
                node.submission_origin,
            );
        }
        insert_value!(
            block_value["encoded_blake3_hex"] = hex_encode(blake3_hash(&block_bytes).as_bytes())
        );
        block_files.push(Value::Object(block_value));
        prev_block_cid = Some(block.block_cid.clone());
        blocks.push(block);
    }
    let head_block_cid = prev_block_cid.ok_or_else(|| {
        "governance DAG build found no validated governance nodes to build".to_string()
    })?;
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: head_block_cid.clone(),
        block_count: blocks.len() as u64,
        generated_at,
        publisher_peer_id: publisher_peer_id.clone(),
        checkpoint_cid,
        head_signature: empty_governance_dag_ed25519_signature(),
    };
    sign_governance_dag_head_cli(&mut head, &signing_key)?;
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .map_err(|err| format!("built governance DAG head failed validation: {err}"))?;
    let head_bytes =
        to_bytes(&head).map_err(|err| format!("failed to encode governance DAG head: {err}"))?;
    let head_path = out.join("head.to");
    write_text(&head_path, &head_bytes)?;
    let head_sidecar_bytes = write_governance_blake3_sidecar(&head_path, &head_bytes)?;
    car_files.push(governance_dag_car_file("head.to", head_bytes.clone())?);
    car_files.push(governance_dag_car_file(
        "head.to.blake3",
        head_sidecar_bytes,
    )?);
    let car_archive_summary = if let Some(car_path) = car_out.as_deref() {
        let handle = car_chunker_handle
            .as_deref()
            .unwrap_or(DEFAULT_CHUNKER_HANDLE);
        Some(write_governance_dag_car_archive(
            &out,
            car_path,
            car_plan_out.as_deref(),
            handle,
            car_files,
        )?)
    } else {
        None
    };
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.build.v1");
    insert_value!(summary["source_root"] = root.display().to_string());
    insert_value!(summary["output_root"] = out.display().to_string());
    insert_value!(summary["generated_at"] = generated_at);
    insert_value!(
        summary["publisher_peer_id"] = String::from_utf8_lossy(&publisher_peer_id).to_string()
    );
    insert_value!(
        summary["publisher_public_key_hex"] = hex_encode(signing_key.verifying_key().to_bytes())
    );
    insert_value!(summary["block_count"] = blocks.len() as u64);
    insert_value!(summary["head_block_cid_hex"] = hex_encode(&head_block_cid));
    insert_value!(summary["head_path"] = "head.to");
    insert_value!(summary["head_blake3_hex"] = hex_encode(blake3_hash(&head_bytes).as_bytes()));
    if let Some(checkpoint) = &head.checkpoint_cid {
        insert_value!(summary["checkpoint_cid_hex"] = hex_encode(checkpoint));
    }
    insert_json!(summary["blocks"] = Value::Array(block_files));
    if let Some(car_summary) = car_archive_summary {
        insert_json!(summary["car_archive"] = car_summary);
    }
    insert_json!(summary["input_verification"] = verify_summary);
    let summary_value = Value::Object(summary);
    write_governance_dag_json(&out.join("manifest.json"), &summary_value)?;
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)
}
fn governance_dag_verify_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag verify-build`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag verify-build`".to_string()
    })?;
    let (ok, summary) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, head_cid.as_deref());
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary)?;
    }
    print_governance_dag_json(&summary)?;
    if ok {
        Ok(())
    } else {
        Err("governance DAG build verification failed".to_string())
    }
}
fn governance_dag_rebuild_head(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut head_out: Option<PathBuf> = None;
    let mut publisher_peer_id: Option<Vec<u8>> = None;
    let mut key_hex: Option<String> = None;
    let mut key_path: Option<PathBuf> = None;
    let mut generated_at: Option<u64> = None;
    let mut checkpoint_cid: Option<Vec<u8>> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--head-out" => head_out = Some(PathBuf::from(value)),
            "--publisher-peer-id" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("`--publisher-peer-id` must not be empty".to_string());
                }
                publisher_peer_id = Some(trimmed.as_bytes().to_vec());
            }
            "--key-hex" => key_hex = Some(value.to_string()),
            "--key" => key_path = Some(PathBuf::from(value)),
            "--generated-at" => {
                generated_at = Some(parse_u64_arg(
                    "--generated-at",
                    value,
                    "sorafs_cli governance dag rebuild-head",
                )?)
            }
            "--checkpoint-cid" => checkpoint_cid = Some(parse_governance_cid_arg(value)?),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag rebuild-head`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag rebuild-head`".to_string()
    })?;
    let head_out = head_out.ok_or_else(|| {
        "missing required `--head-out=PATH` for `sorafs_cli governance dag rebuild-head`"
            .to_string()
    })?;
    let publisher_peer_id = publisher_peer_id.ok_or_else(|| {
        "missing required `--publisher-peer-id=ID` for `sorafs_cli governance dag rebuild-head`"
            .to_string()
    })?;
    let seed = load_governance_dag_build_seed(key_hex.as_deref(), key_path.as_deref())?;
    let signing_key = SigningKey::from_bytes(&seed);
    let generated_at = generated_at.unwrap_or_else(governance_dag_now_secs);
    let (blocks, block_records, warnings) =
        load_governance_dag_block_snapshot(&root, require_sidecars)?;
    let head_block_cid = governance_dag_head_cid_from_blocks(&blocks)?;
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: head_block_cid.clone(),
        block_count: blocks.len() as u64,
        generated_at,
        publisher_peer_id: publisher_peer_id.clone(),
        checkpoint_cid,
        head_signature: empty_governance_dag_ed25519_signature(),
    };
    sign_governance_dag_head_cli(&mut head, &signing_key)?;
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .map_err(|err| format!("rebuilt governance DAG head failed validation: {err}"))?;
    let head_bytes = to_bytes(&head)
        .map_err(|err| format!("failed to encode rebuilt governance DAG head: {err}"))?;
    write_text(&head_out, &head_bytes)?;
    write_governance_blake3_sidecar(&head_out, &head_bytes)?;
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.head.rebuild.v1");
    insert_value!(summary["source_root"] = root.display().to_string());
    insert_value!(summary["head_path"] = head_out.display().to_string());
    insert_value!(summary["generated_at"] = generated_at);
    insert_value!(
        summary["publisher_peer_id"] = String::from_utf8_lossy(&publisher_peer_id).to_string()
    );
    insert_value!(
        summary["publisher_public_key_hex"] = hex_encode(signing_key.verifying_key().to_bytes())
    );
    insert_value!(summary["block_count"] = blocks.len() as u64);
    insert_value!(summary["head_block_cid"] = cid_display(&head_block_cid));
    insert_value!(summary["head_block_cid_hex"] = hex_encode(&head_block_cid));
    insert_value!(summary["head_blake3_hex"] = hex_encode(blake3_hash(&head_bytes).as_bytes()));
    if let Some(checkpoint) = &head.checkpoint_cid {
        insert_value!(summary["checkpoint_cid_hex"] = hex_encode(checkpoint));
    }
    insert_json!(summary["blocks"] = Value::Array(block_records));
    insert_json!(summary["warnings"] = Value::Array(warnings));
    let summary_value = Value::Object(summary);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)
}
fn governance_dag_checkpoint(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut car_path: Option<PathBuf> = None;
    let mut mirror_index_path: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    let mut generated_at: Option<u64> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--car" => car_path = Some(PathBuf::from(value)),
            "--mirror-index" => mirror_index_path = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            "--generated-at" => {
                generated_at = Some(parse_u64_arg(
                    "--generated-at",
                    value,
                    "sorafs_cli governance dag checkpoint",
                )?)
            }
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag checkpoint`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag checkpoint`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=PATH` for `sorafs_cli governance dag checkpoint`".to_string()
    })?;
    let (ok, verification) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, head_cid.as_deref());
    if !ok {
        print_governance_dag_json(&verification)?;
        return Err("governance DAG checkpoint refused invalid build snapshot".to_string());
    }
    let head_path = root.join("head.to");
    let (head_bytes, head_len, head_blake3_hex) =
        governance_dag_read_digest_file(&head_path, "governance DAG checkpoint head")?;
    let head = decode_from_bytes::<GovernanceDagHeadV1>(&head_bytes).map_err(|err| {
        format!(
            "failed to decode governance DAG checkpoint head `{}`: {err}",
            head_path.display()
        )
    })?;
    let mut head_value = Map::new();
    insert_value!(head_value["path"] = "head.to");
    insert_value!(head_value["source_path"] = head_path.display().to_string());
    insert_value!(head_value["encoded_len"] = head_len);
    insert_value!(head_value["blake3"] = head_blake3_hex);
    insert_value!(head_value["head_block_cid"] = cid_display(&head.head_block_cid));
    insert_value!(head_value["head_block_cid_hex"] = hex_encode(&head.head_block_cid));
    insert_value!(head_value["block_count"] = head.block_count);
    insert_value!(head_value["generated_at"] = head.generated_at);
    insert_value!(
        head_value["publisher_peer_id"] =
            String::from_utf8_lossy(&head.publisher_peer_id).to_string()
    );
    insert_json!(
        head_value["checkpoint_cid_hex"] = head
            .checkpoint_cid
            .as_ref()
            .map(hex_encode)
            .map_or(Value::Null, Value::from)
    );
    let car_value = if let Some(path) = car_path.as_deref() {
        let (_, encoded_len, blake3_hex) =
            governance_dag_read_digest_file(path, "governance DAG checkpoint CAR")?;
        let mut value = Map::new();
        insert_value!(value["path"] = path.display().to_string());
        insert_value!(value["encoded_len"] = encoded_len);
        insert_value!(value["car_size"] = encoded_len);
        insert_value!(value["blake3"] = blake3_hex);
        Some(Value::Object(value))
    } else {
        None
    };
    let mirror_index_value = if let Some(path) = mirror_index_path.as_deref() {
        let (bytes, encoded_len, blake3_hex) =
            governance_dag_read_digest_file(path, "governance DAG checkpoint mirror index")?;
        let index: Value = from_slice(&bytes).map_err(|err| {
            format!(
                "failed to parse governance DAG checkpoint mirror index `{}` as JSON: {err}",
                path.display()
            )
        })?;
        if index.get("schema").and_then(Value::as_str) != Some("sorafs.governance_dag.mirror.v1") {
            return Err(format!(
                "governance DAG checkpoint mirror index `{}` has unsupported schema",
                path.display()
            ));
        }
        let index_head_cid = index
            .get("head")
            .and_then(|head| head.get("head_block_cid_hex"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "governance DAG checkpoint mirror index `{}` is missing `head.head_block_cid_hex`",
                    path.display()
                )
            })?;
        let snapshot_head_cid = hex_encode(&head.head_block_cid);
        if index_head_cid != snapshot_head_cid {
            return Err(format!(
                "governance DAG checkpoint mirror index `{}` advertises head `{index_head_cid}` but snapshot advertises `{snapshot_head_cid}`",
                path.display()
            ));
        }
        let index_block_count = index
            .get("block_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "governance DAG checkpoint mirror index `{}` is missing `block_count`",
                    path.display()
                )
            })?;
        if index_block_count != head.block_count {
            return Err(format!(
                "governance DAG checkpoint mirror index `{}` advertises block_count `{index_block_count}` but head advertises `{}`",
                path.display(),
                head.block_count
            ));
        }
        let mut value = Map::new();
        insert_value!(value["path"] = path.display().to_string());
        insert_value!(value["encoded_len"] = encoded_len);
        insert_value!(value["blake3"] = blake3_hex);
        insert_value!(value["schema"] = "sorafs.governance_dag.mirror.v1");
        insert_value!(value["block_count"] = index_block_count);
        insert_value!(value["head_block_cid_hex"] = index_head_cid.to_string());
        Some(Value::Object(value))
    } else {
        None
    };
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.checkpoint.v1");
    insert_value!(summary["source_root"] = root.display().to_string());
    insert_value!(summary["output_path"] = out.display().to_string());
    insert_value!(summary["generated_at"] = generated_at.unwrap_or_else(governance_dag_now_secs));
    insert_value!(summary["require_sidecars"] = require_sidecars);
    insert_json!(
        summary["expected_head_cid"] = head_cid
            .as_ref()
            .map(|cid| cid_display(cid))
            .map_or(Value::Null, Value::from)
    );
    insert_json!(
        summary["expected_head_cid_hex"] = head_cid
            .as_ref()
            .map(hex_encode)
            .map_or(Value::Null, Value::from)
    );
    insert_json!(summary["head"] = Value::Object(head_value));
    insert_value!(summary["block_count"] = head.block_count);
    if let Some(blocks) = verification.get("blocks").cloned() {
        insert_json!(summary["blocks"] = blocks);
    }
    if let Some(value) = car_value {
        insert_json!(summary["car_archive"] = value);
    }
    if let Some(value) = mirror_index_value {
        insert_json!(summary["mirror_index"] = value);
    }
    insert_json!(summary["verification"] = verification);
    let summary_value = Value::Object(summary);
    write_governance_dag_json(&out, &summary_value)?;
    print_governance_dag_json(&summary_value)
}
fn governance_dag_checkpoint_verify(raw_args: Vec<String>) -> Result<(), String> {
    let mut checkpoint_path: Option<PathBuf> = None;
    let mut root_override: Option<PathBuf> = None;
    let mut car_override: Option<PathBuf> = None;
    let mut mirror_index_override: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--checkpoint" => checkpoint_path = Some(PathBuf::from(value)),
            "--root" => root_override = Some(PathBuf::from(value)),
            "--car" => car_override = Some(PathBuf::from(value)),
            "--mirror-index" => mirror_index_override = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag checkpoint-verify`"
                ));
            }
        }
    }
    let checkpoint_path = checkpoint_path.ok_or_else(|| {
        "missing required `--checkpoint=PATH` for `sorafs_cli governance dag checkpoint-verify`"
            .to_string()
    })?;
    let (checkpoint_bytes, checkpoint_len, checkpoint_blake3_hex) =
        governance_dag_read_digest_file(&checkpoint_path, "governance DAG checkpoint manifest")?;
    let checkpoint: Value = from_slice(&checkpoint_bytes).map_err(|err| {
        format!(
            "failed to parse governance DAG checkpoint `{}` as JSON: {err}",
            checkpoint_path.display()
        )
    })?;
    let mut errors = Vec::<Value>::new();
    if checkpoint.get("schema").and_then(Value::as_str)
        != Some("sorafs.governance_dag.checkpoint.v1")
    {
        errors.push(governance_dag_problem(
            checkpoint_path.to_string_lossy().as_ref(),
            "schema",
            "checkpoint manifest has unsupported schema",
        ));
    }
    let mut checkpoint_file_value = Map::new();
    insert_value!(checkpoint_file_value["path"] = checkpoint_path.display().to_string());
    insert_value!(checkpoint_file_value["encoded_len"] = checkpoint_len);
    insert_value!(checkpoint_file_value["blake3"] = checkpoint_blake3_hex);
    let expected_head_cid_hex = checkpoint
        .get("head")
        .and_then(|head| head.get("head_block_cid_hex"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let expected_head_cid = match expected_head_cid_hex.as_deref() {
        Some(value) => match hex::decode(value) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                errors.push(governance_dag_problem(
                    checkpoint_path.to_string_lossy().as_ref(),
                    "head_cid",
                    format!("checkpoint head_block_cid_hex is not valid hex: {err}"),
                ));
                None
            }
        },
        None => {
            errors.push(governance_dag_problem(
                checkpoint_path.to_string_lossy().as_ref(),
                "head_cid",
                "checkpoint manifest is missing `head.head_block_cid_hex`",
            ));
            None
        }
    };
    let root_path = match root_override {
        Some(path) => Some(path),
        None => checkpoint
            .get("source_root")
            .and_then(Value::as_str)
            .map(PathBuf::from),
    };
    let mut root_value = Map::new();
    let mut head_check = Value::Null;
    let mut root_verification = Value::Null;
    if let Some(root) = root_path.as_deref() {
        insert_value!(root_value["path"] = root.display().to_string());
        let (root_ok, verification) = verify_governance_dag_build_snapshot(
            root,
            require_sidecars,
            expected_head_cid.as_deref(),
        );
        insert_value!(root_value["ok"] = root_ok);
        if !root_ok {
            errors.push(governance_dag_problem(
                root.to_string_lossy().as_ref(),
                "snapshot",
                "checkpoint root snapshot verification failed",
            ));
        }
        root_verification = verification;
        if let Some(expected_head) = checkpoint.get("head") {
            let head_path = root.join("head.to");
            head_check = governance_dag_checkpoint_file_check(
                "head",
                &head_path,
                expected_head,
                &["encoded_len"],
                &mut errors,
            );
        }
    } else {
        errors.push(governance_dag_problem(
            checkpoint_path.to_string_lossy().as_ref(),
            "source_root",
            "checkpoint verification requires `--root=DIR` or a checkpoint `source_root`",
        ));
    }
    insert_json!(root_value["verification"] = root_verification);
    let car_check = governance_dag_checkpoint_optional_artifact_check(
        &checkpoint,
        car_override.as_deref(),
        "car_archive",
        "governance DAG checkpoint CAR",
        &["encoded_len", "car_size"],
        &mut errors,
    )?;
    let mirror_check = governance_dag_checkpoint_optional_artifact_check(
        &checkpoint,
        mirror_index_override.as_deref(),
        "mirror_index",
        "governance DAG checkpoint mirror index",
        &["encoded_len"],
        &mut errors,
    )?;
    if let Some((path, value)) = mirror_check.as_ref() {
        governance_dag_checkpoint_validate_mirror_index(
            path,
            value,
            expected_head_cid_hex.as_deref(),
            checkpoint.get("block_count").and_then(Value::as_u64),
            &mut errors,
        );
    }
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.checkpoint.verify.v1");
    insert_value!(summary["ok"] = errors.is_empty());
    insert_value!(summary["require_sidecars"] = require_sidecars);
    insert_json!(summary["checkpoint"] = Value::Object(checkpoint_file_value));
    insert_json!(
        summary["expected_head_cid_hex"] = expected_head_cid_hex.map_or(Value::Null, Value::from)
    );
    insert_json!(summary["root"] = Value::Object(root_value));
    insert_json!(summary["head"] = head_check);
    if let Some((_, value)) = car_check {
        insert_json!(summary["car_archive"] = value);
    }
    if let Some((_, value)) = mirror_check {
        insert_json!(summary["mirror_index"] = value);
    }
    insert_json!(summary["errors"] = Value::Array(errors.clone()));
    let summary_value = Value::Object(summary);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err("governance DAG checkpoint verification failed".to_string())
    }
}
fn governance_dag_checkpoint_recover(raw_args: Vec<String>) -> Result<(), String> {
    let mut checkpoint_path: Option<PathBuf> = None;
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut car_path: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut summary_out: Option<PathBuf> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--checkpoint" => checkpoint_path = Some(PathBuf::from(value)),
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--car" => car_path = Some(PathBuf::from(value)),
            "--summary-out" => summary_out = Some(PathBuf::from(value)),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag checkpoint-recover`"
                ));
            }
        }
    }
    let checkpoint_path = checkpoint_path.ok_or_else(|| {
        "missing required `--checkpoint=PATH` for `sorafs_cli governance dag checkpoint-recover`"
            .to_string()
    })?;
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag checkpoint-recover`"
            .to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=PATH` for `sorafs_cli governance dag checkpoint-recover`"
            .to_string()
    })?;
    let (checkpoint_bytes, checkpoint_len, checkpoint_blake3_hex) =
        governance_dag_read_digest_file(&checkpoint_path, "governance DAG checkpoint manifest")?;
    let checkpoint: Value = from_slice(&checkpoint_bytes).map_err(|err| {
        format!(
            "failed to parse governance DAG checkpoint `{}` as JSON: {err}",
            checkpoint_path.display()
        )
    })?;
    let mut errors = Vec::<Value>::new();
    if checkpoint.get("schema").and_then(Value::as_str)
        != Some("sorafs.governance_dag.checkpoint.v1")
    {
        errors.push(governance_dag_problem(
            checkpoint_path.to_string_lossy().as_ref(),
            "schema",
            "checkpoint manifest has unsupported schema",
        ));
    }
    let mut checkpoint_file_value = Map::new();
    insert_value!(checkpoint_file_value["path"] = checkpoint_path.display().to_string());
    insert_value!(checkpoint_file_value["encoded_len"] = checkpoint_len);
    insert_value!(checkpoint_file_value["blake3"] = checkpoint_blake3_hex);
    let expected_head_cid_hex = checkpoint
        .get("head")
        .and_then(|head| head.get("head_block_cid_hex"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let expected_head_cid = match expected_head_cid_hex.as_deref() {
        Some(value) => match hex::decode(value) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                errors.push(governance_dag_problem(
                    checkpoint_path.to_string_lossy().as_ref(),
                    "head_cid",
                    format!("checkpoint head_block_cid_hex is not valid hex: {err}"),
                ));
                None
            }
        },
        None => {
            errors.push(governance_dag_problem(
                checkpoint_path.to_string_lossy().as_ref(),
                "head_cid",
                "checkpoint manifest is missing `head.head_block_cid_hex`",
            ));
            None
        }
    };
    let (root_ok, root_verification) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, expected_head_cid.as_deref());
    if !root_ok {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "snapshot",
            "checkpoint recovery root snapshot verification failed",
        ));
    }
    let head_check = if let Some(expected_head) = checkpoint.get("head") {
        governance_dag_checkpoint_file_check(
            "head",
            &root.join("head.to"),
            expected_head,
            &["encoded_len"],
            &mut errors,
        )
    } else {
        Value::Null
    };
    let car_check = governance_dag_checkpoint_optional_artifact_check(
        &checkpoint,
        car_path.as_deref(),
        "car_archive",
        "governance DAG checkpoint CAR",
        &["encoded_len", "car_size"],
        &mut errors,
    )?;
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.checkpoint.recover.v1");
    insert_value!(summary["require_sidecars"] = require_sidecars);
    insert_json!(summary["checkpoint"] = Value::Object(checkpoint_file_value));
    insert_json!(
        summary["expected_head_cid_hex"] = expected_head_cid_hex
            .clone()
            .map_or(Value::Null, Value::from)
    );
    let mut root_value = Map::new();
    insert_value!(root_value["path"] = root.display().to_string());
    insert_value!(root_value["ok"] = root_ok);
    insert_json!(root_value["verification"] = root_verification);
    insert_json!(summary["root"] = Value::Object(root_value));
    insert_json!(summary["head"] = head_check);
    if let Some((_, value)) = car_check {
        insert_json!(summary["car_archive"] = value);
    }
    if errors.is_empty() {
        let index = governance_dag_mirror_index_value(
            &root,
            require_sidecars,
            expected_head_cid.as_deref(),
        )?;
        write_governance_dag_json(&out, &index)?;
        let (index_bytes, encoded_len, blake3_hex) =
            governance_dag_read_digest_file(&out, "recovered governance DAG mirror index")?;
        let index_value: Value = from_slice(&index_bytes).map_err(|err| {
            format!(
                "failed to parse recovered governance DAG mirror index `{}` as JSON: {err}",
                out.display()
            )
        })?;
        let mut recovered = Map::new();
        insert_value!(recovered["path"] = out.display().to_string());
        insert_value!(recovered["encoded_len"] = encoded_len);
        insert_value!(recovered["blake3"] = blake3_hex);
        insert_value!(recovered["schema"] = "sorafs.governance_dag.mirror.v1");
        insert_json!(
            recovered["head_block_cid_hex"] = index_value
                .get("head")
                .and_then(|head| head.get("head_block_cid_hex"))
                .cloned()
                .unwrap_or(Value::Null)
        );
        insert_json!(
            recovered["block_count"] = index_value
                .get("block_count")
                .cloned()
                .unwrap_or(Value::Null)
        );
        insert_json!(summary["recovered_mirror_index"] = Value::Object(recovered));
    }
    insert_value!(summary["ok"] = errors.is_empty());
    insert_json!(summary["errors"] = Value::Array(errors.clone()));
    let summary_value = Value::Object(summary);
    if let Some(path) = summary_out.as_deref() {
        write_governance_dag_json(path, &summary_value)?;
    }
    print_governance_dag_json(&summary_value)?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err("governance DAG checkpoint recovery failed".to_string())
    }
}
fn governance_dag_mirror_build(raw_args: Vec<String>) -> Result<(), String> {
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut require_sidecars = false;
    let mut head_cid: Option<Vec<u8>> = None;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--require-sidecars" {
            require_sidecars = true;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--head-cid" => head_cid = Some(parse_governance_cid_arg(value)?),
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag mirror-build`"
                ));
            }
        }
    }
    let root = root.ok_or_else(|| {
        "missing required `--root=DIR` for `sorafs_cli governance dag mirror-build`".to_string()
    })?;
    let out = out.ok_or_else(|| {
        "missing required `--out=PATH` for `sorafs_cli governance dag mirror-build`".to_string()
    })?;
    let (ok, verification) =
        verify_governance_dag_build_snapshot(&root, require_sidecars, head_cid.as_deref());
    if !ok {
        print_governance_dag_json(&verification)?;
        return Err("governance DAG mirror index refused invalid build snapshot".to_string());
    }
    let index = governance_dag_mirror_index_value(&root, require_sidecars, head_cid.as_deref())?;
    write_governance_dag_json(&out, &index)?;
    print_governance_dag_json(&index)
}
enum GovernanceDagMirrorQuery {
    Head,
    BlockCid(Vec<u8>),
    NodeCid(Vec<u8>),
}
fn governance_dag_mirror_query(raw_args: Vec<String>) -> Result<(), String> {
    let mut index_path: Option<PathBuf> = None;
    let mut query: Option<GovernanceDagMirrorQuery> = None;
    let mut format = GovernanceDagOutputFormat::Table;
    for arg in raw_args {
        if arg == "--help" || arg == "-h" {
            return Err(governance_usage());
        }
        if arg == "--head" {
            set_governance_dag_mirror_query(&mut query, GovernanceDagMirrorQuery::Head)?;
            continue;
        }
        let Some((key, value)) = arg.split_once('=') else {
            return Err(governance_usage());
        };
        match key {
            "--index" => index_path = Some(PathBuf::from(value)),
            "--block-cid" => set_governance_dag_mirror_query(
                &mut query,
                GovernanceDagMirrorQuery::BlockCid(parse_governance_cid_arg(value)?),
            )?,
            "--node-cid" => set_governance_dag_mirror_query(
                &mut query,
                GovernanceDagMirrorQuery::NodeCid(parse_governance_cid_arg(value)?),
            )?,
            "--format" => format = GovernanceDagOutputFormat::parse(value)?,
            other => {
                return Err(format!(
                    "unrecognised option `{other}` for `sorafs_cli governance dag mirror-query`"
                ));
            }
        }
    }
    let index_path = index_path.ok_or_else(|| {
        "missing required `--index=PATH` for `sorafs_cli governance dag mirror-query`".to_string()
    })?;
    let query = query.ok_or_else(|| {
        "missing mirror query selector; provide `--head`, `--block-cid=...`, or `--node-cid=...`"
            .to_string()
    })?;
    let index = read_governance_dag_json_file(&index_path)?;
    if index.get("schema").and_then(Value::as_str) != Some("sorafs.governance_dag.mirror.v1") {
        return Err(format!(
            "governance DAG mirror index `{}` has unsupported schema",
            index_path.display()
        ));
    }
    let (found, query_value) = governance_dag_mirror_query_value(&index_path, &index, &query)?;
    match format {
        GovernanceDagOutputFormat::Json => print_governance_dag_json(&query_value)?,
        GovernanceDagOutputFormat::Table => print_governance_dag_mirror_query_table(&query_value),
    }
    if found {
        Ok(())
    } else {
        Err("governance DAG mirror query returned no match".to_string())
    }
}
fn set_governance_dag_mirror_query(
    slot: &mut Option<GovernanceDagMirrorQuery>,
    value: GovernanceDagMirrorQuery,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(
            "governance DAG mirror query accepts exactly one of `--head`, `--block-cid`, or `--node-cid`"
                .to_string(),
        );
    }
    *slot = Some(value);
    Ok(())
}
fn governance_dag_mirror_index_value(
    root: &Path,
    require_sidecars: bool,
    expected_head_cid: Option<&[u8]>,
) -> Result<Value, String> {
    let head_path = root.join("head.to");
    let head_bytes = fs::read(&head_path).map_err(|err| {
        format!(
            "failed to read governance DAG mirror head `{}`: {err}",
            head_path.display()
        )
    })?;
    let head_blake3_hex = hex_encode(blake3_hash(&head_bytes).as_bytes());
    let head = decode_from_bytes::<GovernanceDagHeadV1>(&head_bytes).map_err(|err| {
        format!(
            "failed to decode governance DAG mirror head `{}`: {err}",
            head_path.display()
        )
    })?;
    let blocks_dir = root.join("blocks");
    let mut block_paths = Vec::<PathBuf>::new();
    collect_governance_dag_to_files(&blocks_dir, &mut block_paths)?;
    block_paths.sort();
    let mut blocks = Vec::<(String, String, String, GovernanceDagBlockV1)>::new();
    for path in block_paths {
        let rel_path = governance_dag_relative_path(root, &path);
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "failed to read governance DAG mirror block `{}`: {err}",
                path.display()
            )
        })?;
        let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
        let (sidecar_status, _, sidecar_error) = governance_dag_sidecar_status(&path, &blake3_hex);
        match sidecar_status.as_str() {
            "mismatch" | "error" => {
                return Err(format!(
                    "governance DAG mirror block `{rel_path}` has invalid sidecar status `{sidecar_status}`{}",
                    sidecar_error
                        .as_deref()
                        .map(|err| format!(": {err}"))
                        .unwrap_or_default()
                ));
            }
            "missing" if require_sidecars => {
                return Err(format!(
                    "governance DAG mirror block `{rel_path}` is missing required sidecar"
                ));
            }
            _ => {}
        }
        let block = decode_from_bytes::<GovernanceDagBlockV1>(&bytes).map_err(|err| {
            format!(
                "failed to decode governance DAG mirror block `{}`: {err}",
                path.display()
            )
        })?;
        blocks.push((rel_path, blake3_hex, sidecar_status, block));
    }
    blocks.sort_by(|left, right| {
        left.3
            .sequence
            .cmp(&right.3.sequence)
            .then_with(|| left.0.cmp(&right.0))
    });
    let decoded_blocks = blocks
        .iter()
        .map(|(_, _, _, block)| block.clone())
        .collect::<Vec<_>>();
    validate_governance_dag_head_against_chain_v1(&head, &decoded_blocks)
        .map_err(|err| format!("governance DAG mirror snapshot failed validation: {err}"))?;
    if let Some(expected) = expected_head_cid
        && head.head_block_cid != expected
    {
        return Err(format!(
            "governance DAG mirror expected head CID `{}` but snapshot advertises `{}`",
            cid_display(expected),
            cid_display(&head.head_block_cid)
        ));
    }
    let mut by_block_cid_hex = Map::new();
    let mut by_node_cid_hex = Map::new();
    let mut block_values = Vec::<Value>::new();
    for (position, (path, blake3_hex, sidecar_status, block)) in blocks.iter().enumerate() {
        let block_cid_hex = hex_encode(&block.block_cid);
        let node_cid_hex = hex_encode(&block.node.node_cid);
        by_block_cid_hex.insert(block_cid_hex.clone(), Value::from(position as u64));
        by_node_cid_hex.insert(node_cid_hex.clone(), Value::from(position as u64));
        let mut block_value = Map::new();
        insert_value!(block_value["position"] = position as u64);
        insert_value!(block_value["path"] = path.clone());
        insert_value!(block_value["sequence"] = block.sequence);
        insert_value!(block_value["timestamp"] = block.timestamp);
        insert_value!(
            block_value["publisher_peer_id"] =
                String::from_utf8_lossy(&block.publisher_peer_id).to_string()
        );
        insert_value!(block_value["block_cid"] = cid_display(&block.block_cid));
        insert_value!(block_value["block_cid_hex"] = block_cid_hex);
        insert_json!(
            block_value["prev_block_cid_hex"] = block
                .prev_block_cid
                .as_ref()
                .map(hex_encode)
                .map_or(Value::Null, Value::from)
        );
        insert_value!(block_value["node_cid"] = cid_display(&block.node.node_cid));
        insert_value!(block_value["node_cid_hex"] = node_cid_hex);
        insert_value!(
            block_value["payload_kind"] = governance_payload_kind_cli(&block.node.payload)
        );
        insert_governance_node_submission_summary(&mut block_value, &block.node);
        insert_value!(block_value["blake3"] = blake3_hex.clone());
        insert_value!(block_value["sidecar_status"] = sidecar_status.clone());
        block_values.push(Value::Object(block_value));
    }
    let mut head_value = Map::new();
    insert_value!(head_value["path"] = "head.to");
    insert_value!(head_value["head_block_cid"] = cid_display(&head.head_block_cid));
    insert_value!(head_value["head_block_cid_hex"] = hex_encode(&head.head_block_cid));
    insert_value!(head_value["block_count"] = head.block_count);
    insert_value!(head_value["generated_at"] = head.generated_at);
    insert_value!(
        head_value["publisher_peer_id"] =
            String::from_utf8_lossy(&head.publisher_peer_id).to_string()
    );
    insert_value!(head_value["blake3"] = head_blake3_hex);
    insert_json!(
        head_value["checkpoint_cid_hex"] = head
            .checkpoint_cid
            .as_ref()
            .map(hex_encode)
            .map_or(Value::Null, Value::from)
    );
    let mut root_value = Map::new();
    insert_value!(root_value["schema"] = "sorafs.governance_dag.mirror.v1");
    insert_value!(root_value["source_root"] = root.display().to_string());
    insert_value!(root_value["generated_at"] = governance_dag_now_secs());
    insert_value!(root_value["require_sidecars"] = require_sidecars);
    insert_json!(root_value["head"] = Value::Object(head_value));
    insert_value!(root_value["block_count"] = block_values.len() as u64);
    insert_json!(root_value["blocks"] = Value::Array(block_values));
    insert_json!(root_value["by_block_cid_hex"] = Value::Object(by_block_cid_hex));
    insert_json!(root_value["by_node_cid_hex"] = Value::Object(by_node_cid_hex));
    Ok(Value::Object(root_value))
}
fn governance_dag_mirror_query_value(
    index_path: &Path,
    index: &Value,
    query: &GovernanceDagMirrorQuery,
) -> Result<(bool, Value), String> {
    let mut result = Map::new();
    insert_value!(result["schema"] = "sorafs.governance_dag.mirror.query.v1");
    insert_value!(result["index"] = index_path.display().to_string());
    match query {
        GovernanceDagMirrorQuery::Head => {
            insert_value!(result["query"] = "head");
            let head = index.get("head").cloned().unwrap_or(Value::Null);
            let found = !matches!(head, Value::Null);
            insert_value!(result["found"] = found);
            insert_json!(result["head"] = head);
            Ok((found, Value::Object(result)))
        }
        GovernanceDagMirrorQuery::BlockCid(cid) => {
            let cid_hex = hex_encode(cid);
            insert_value!(result["query"] = "block_cid");
            insert_value!(result["cid"] = cid_display(cid));
            insert_value!(result["cid_hex"] = cid_hex.clone());
            let block = governance_dag_mirror_lookup_block(index, "by_block_cid_hex", &cid_hex)?;
            let found = !matches!(block, Value::Null);
            insert_value!(result["found"] = found);
            insert_json!(result["block"] = block);
            Ok((found, Value::Object(result)))
        }
        GovernanceDagMirrorQuery::NodeCid(cid) => {
            let cid_hex = hex_encode(cid);
            insert_value!(result["query"] = "node_cid");
            insert_value!(result["cid"] = cid_display(cid));
            insert_value!(result["cid_hex"] = cid_hex.clone());
            let block = governance_dag_mirror_lookup_block(index, "by_node_cid_hex", &cid_hex)?;
            let found = !matches!(block, Value::Null);
            insert_value!(result["found"] = found);
            insert_json!(result["block"] = block);
            Ok((found, Value::Object(result)))
        }
    }
}
fn governance_dag_mirror_lookup_block(
    index: &Value,
    map_name: &str,
    cid_hex: &str,
) -> Result<Value, String> {
    let Some(position) = index
        .get(map_name)
        .and_then(Value::as_object)
        .and_then(|map| map.get(cid_hex))
        .and_then(Value::as_u64)
    else {
        return Ok(Value::Null);
    };
    let blocks = index
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance DAG mirror index is missing `blocks` array".to_string())?;
    let position = usize::try_from(position)
        .map_err(|_| "governance DAG mirror block position exceeds host limits".to_string())?;
    Ok(blocks.get(position).cloned().unwrap_or(Value::Null))
}
fn read_governance_dag_json_file(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;
    from_slice(&bytes).map_err(|err| format!("failed to parse `{}` as JSON: {err}", path.display()))
}
fn governance_dag_read_digest_file(
    path: &Path,
    label: &str,
) -> Result<(Vec<u8>, u64, String), String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read {label} `{}`: {err}", path.display()))?;
    let encoded_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
    Ok((bytes, encoded_len, blake3_hex))
}
fn governance_dag_checkpoint_file_check(
    artifact: &str,
    path: &Path,
    expected: &Value,
    size_fields: &[&str],
    errors: &mut Vec<Value>,
) -> Value {
    let mut value = Map::new();
    insert_value!(value["path"] = path.display().to_string());
    match governance_dag_read_digest_file(path, artifact) {
        Ok((_, encoded_len, blake3_hex)) => {
            insert_value!(value["encoded_len"] = encoded_len);
            insert_value!(value["blake3"] = blake3_hex.clone());
            let expected_blake3 = expected.get("blake3").and_then(Value::as_str);
            insert_json!(
                value["expected_blake3"] = expected_blake3.map_or(Value::Null, Value::from)
            );
            let digest_ok = expected_blake3 == Some(blake3_hex.as_str());
            insert_value!(value["digest_ok"] = digest_ok);
            if !digest_ok {
                errors.push(governance_dag_problem(
                    path.to_string_lossy().as_ref(),
                    format!("{artifact}_digest"),
                    "checkpoint artifact BLAKE3 digest does not match recorded value",
                ));
            }
            let expected_len = size_fields
                .iter()
                .find_map(|field| expected.get(*field).and_then(Value::as_u64));
            insert_json!(value["expected_len"] = expected_len.map_or(Value::Null, Value::from));
            let len_ok = expected_len == Some(encoded_len);
            insert_value!(value["encoded_len_ok"] = len_ok);
            if !len_ok {
                errors.push(governance_dag_problem(
                    path.to_string_lossy().as_ref(),
                    format!("{artifact}_encoded_len"),
                    "checkpoint artifact length does not match recorded value",
                ));
            }
            insert_value!(value["ok"] = digest_ok && len_ok);
        }
        Err(err) => {
            insert_value!(value["ok"] = false);
            insert_value!(value["read_error"] = err.clone());
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                format!("{artifact}_read"),
                err,
            ));
        }
    }
    Value::Object(value)
}
fn governance_dag_checkpoint_optional_artifact_check(
    checkpoint: &Value,
    override_path: Option<&Path>,
    artifact: &str,
    label: &str,
    size_fields: &[&str],
    errors: &mut Vec<Value>,
) -> Result<Option<(PathBuf, Value)>, String> {
    let expected = checkpoint.get(artifact);
    let Some(expected) = expected else {
        if let Some(path) = override_path {
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                artifact,
                format!(
                    "`--{}` was supplied but checkpoint manifest has no `{artifact}` record",
                    artifact.replace('_', "-")
                ),
            ));
        }
        return Ok(None);
    };
    if matches!(expected, Value::Null) {
        if let Some(path) = override_path {
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                artifact,
                format!(
                    "`--{}` was supplied but checkpoint manifest has a null `{artifact}` record",
                    artifact.replace('_', "-")
                ),
            ));
        }
        return Ok(None);
    }
    let path = if let Some(path) = override_path {
        path.to_path_buf()
    } else {
        let Some(recorded) = expected.get("path").and_then(Value::as_str) else {
            errors.push(governance_dag_problem(
                artifact,
                artifact,
                "checkpoint artifact record is missing `path`",
            ));
            return Ok(None);
        };
        PathBuf::from(recorded)
    };
    let mut value =
        governance_dag_checkpoint_file_check(artifact, &path, expected, size_fields, errors);
    if let Value::Object(ref mut obj) = value {
        insert_value!(obj["label"] = label.to_string());
        insert_value!(
            obj["path_source"] = if override_path.is_some() {
                "override"
            } else {
                "checkpoint"
            }
        );
    }
    Ok(Some((path, value)))
}
fn governance_dag_checkpoint_validate_mirror_index(
    path: &Path,
    check_value: &Value,
    expected_head_cid_hex: Option<&str>,
    expected_block_count: Option<u64>,
    errors: &mut Vec<Value>,
) {
    if check_value.get("ok").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let index = match read_governance_dag_json_file(path) {
        Ok(value) => value,
        Err(err) => {
            errors.push(governance_dag_problem(
                path.to_string_lossy().as_ref(),
                "mirror_index_json",
                err,
            ));
            return;
        }
    };
    if index.get("schema").and_then(Value::as_str) != Some("sorafs.governance_dag.mirror.v1") {
        errors.push(governance_dag_problem(
            path.to_string_lossy().as_ref(),
            "mirror_index_schema",
            "mirror index has unsupported schema",
        ));
    }
    let index_head_cid_hex = index
        .get("head")
        .and_then(|head| head.get("head_block_cid_hex"))
        .and_then(Value::as_str);
    if index_head_cid_hex != expected_head_cid_hex {
        errors.push(governance_dag_problem(
            path.to_string_lossy().as_ref(),
            "mirror_index_head",
            "mirror index head does not match checkpoint head",
        ));
    }
    let index_block_count = index.get("block_count").and_then(Value::as_u64);
    if index_block_count != expected_block_count {
        errors.push(governance_dag_problem(
            path.to_string_lossy().as_ref(),
            "mirror_index_block_count",
            "mirror index block count does not match checkpoint block count",
        ));
    }
}
fn print_governance_dag_mirror_query_table(value: &Value) {
    let found = value.get("found").and_then(Value::as_bool).unwrap_or(false);
    println!("found: {found}");
    match value.get("query").and_then(Value::as_str) {
        Some("head") => {
            if let Some(head) = value.get("head").and_then(Value::as_object) {
                println!(
                    "head_block_cid_hex: {}",
                    head.get("head_block_cid_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "block_count: {}",
                    head.get("block_count")
                        .and_then(Value::as_u64)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "<missing>".to_string())
                );
            }
        }
        Some("block_cid" | "node_cid") => {
            if let Some(block) = value.get("block").and_then(Value::as_object) {
                println!(
                    "sequence: {}",
                    block
                        .get("sequence")
                        .and_then(Value::as_u64)
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "<missing>".to_string())
                );
                println!(
                    "path: {}",
                    block
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "block_cid_hex: {}",
                    block
                        .get("block_cid_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "node_cid_hex: {}",
                    block
                        .get("node_cid_hex")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
                println!(
                    "payload_kind: {}",
                    block
                        .get("payload_kind")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing>")
                );
            }
        }
        _ => {}
    }
}
type GovernanceDagBlockSnapshot = (Vec<GovernanceDagBlockV1>, Vec<Value>, Vec<Value>);
fn load_governance_dag_block_snapshot(
    root: &Path,
    require_sidecars: bool,
) -> Result<GovernanceDagBlockSnapshot, String> {
    let blocks_dir = root.join("blocks");
    if !blocks_dir.is_dir() {
        return Err(format!(
            "governance DAG block snapshot `{}` must contain a `blocks` directory",
            root.display()
        ));
    }
    let mut block_paths = Vec::<PathBuf>::new();
    collect_governance_dag_to_files(&blocks_dir, &mut block_paths)?;
    block_paths.sort();
    if block_paths.is_empty() {
        return Err(format!(
            "governance DAG block snapshot `{}` contains no `.to` block payloads",
            blocks_dir.display()
        ));
    }
    let mut warnings = Vec::<Value>::new();
    let mut decoded = Vec::<(String, String, String, GovernanceDagBlockV1)>::new();
    for path in block_paths {
        let rel_path = governance_dag_relative_path(root, &path);
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "failed to read governance DAG block `{}`: {err}",
                path.display()
            )
        })?;
        let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
        let (sidecar_status, _, sidecar_error) = governance_dag_sidecar_status(&path, &blake3_hex);
        match sidecar_status.as_str() {
            "mismatch" | "error" => {
                return Err(format!(
                    "governance DAG block `{rel_path}` has invalid sidecar status `{sidecar_status}`{}",
                    sidecar_error
                        .as_deref()
                        .map(|err| format!(": {err}"))
                        .unwrap_or_default()
                ));
            }
            "missing" if require_sidecars => {
                return Err(format!(
                    "governance DAG block `{rel_path}` is missing required sidecar"
                ));
            }
            "missing" => warnings.push(governance_dag_problem(
                &rel_path,
                "sidecar",
                "missing optional `.to.blake3` sidecar",
            )),
            _ => {}
        }
        let block = decode_from_bytes::<GovernanceDagBlockV1>(&bytes).map_err(|err| {
            format!(
                "failed to decode governance DAG block `{}`: {err}",
                path.display()
            )
        })?;
        decoded.push((rel_path, blake3_hex, sidecar_status, block));
    }
    decoded.sort_by(|left, right| {
        left.3
            .sequence
            .cmp(&right.3.sequence)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut blocks = Vec::<GovernanceDagBlockV1>::with_capacity(decoded.len());
    let mut records = Vec::<Value>::with_capacity(decoded.len());
    for (position, (path, blake3_hex, sidecar_status, block)) in decoded.into_iter().enumerate() {
        let mut record = Map::new();
        insert_value!(record["position"] = position as u64);
        insert_value!(record["path"] = path);
        insert_value!(record["sequence"] = block.sequence);
        insert_value!(record["timestamp"] = block.timestamp);
        insert_value!(record["block_cid"] = cid_display(&block.block_cid));
        insert_value!(record["block_cid_hex"] = hex_encode(&block.block_cid));
        insert_json!(
            record["prev_block_cid_hex"] = block
                .prev_block_cid
                .as_ref()
                .map(hex_encode)
                .map_or(Value::Null, Value::from)
        );
        insert_value!(record["node_cid"] = cid_display(&block.node.node_cid));
        insert_value!(record["node_cid_hex"] = hex_encode(&block.node.node_cid));
        insert_value!(record["payload_kind"] = governance_payload_kind_cli(&block.node.payload));
        insert_governance_node_submission_summary(&mut record, &block.node);
        insert_value!(record["blake3"] = blake3_hex);
        insert_value!(record["sidecar_status"] = sidecar_status);
        blocks.push(block);
        records.push(Value::Object(record));
    }
    Ok((blocks, records, warnings))
}
fn governance_dag_head_cid_from_blocks(blocks: &[GovernanceDagBlockV1]) -> Result<Vec<u8>, String> {
    if blocks.is_empty() {
        return Err("governance DAG head rebuild requires at least one block".to_string());
    }
    let referenced = blocks
        .iter()
        .filter_map(|block| block.prev_block_cid.clone())
        .collect::<BTreeSet<_>>();
    let heads = blocks
        .iter()
        .filter(|block| !referenced.contains(&block.block_cid))
        .map(|block| block.block_cid.clone())
        .collect::<Vec<_>>();
    match heads.as_slice() {
        [head] => Ok(head.clone()),
        _ => Err(format!(
            "governance DAG head rebuild expected exactly one block-chain head, found {}",
            heads.len()
        )),
    }
}
fn governance_dag_car_file(rel_path: &str, data: Vec<u8>) -> Result<FileEntry, String> {
    let path = rel_path.split('/').map(str::to_string).collect::<Vec<_>>();
    if path.is_empty() || path.iter().any(|component| component.is_empty()) {
        return Err(format!(
            "invalid governance DAG CAR relative path `{rel_path}`"
        ));
    }
    Ok(FileEntry { path, data })
}
fn write_governance_dag_car_archive(
    snapshot_root: &Path,
    car_out: &Path,
    car_plan_out: Option<&Path>,
    chunker_handle: &str,
    files: Vec<FileEntry>,
) -> Result<Value, String> {
    let descriptor = chunker_registry::lookup_by_handle(chunker_handle).ok_or_else(|| {
        format!(
            "unknown governance DAG CAR chunker profile handle `{chunker_handle}`; see `sorafs_manifest_builder --list-chunker-profiles` for options"
        )
    })?;
    let (plan, payload) = CarBuildPlan::from_files_with_profile(files, descriptor.profile)
        .map_err(|err| format!("failed to build governance DAG CAR plan: {err}"))?;
    let car_file = open_output_file(car_out)?;
    let mut writer = BufWriter::new(car_file);
    let mut payload_reader = Cursor::new(payload);
    let stats = CarStreamingWriter::new(&plan)
        .write_from_reader(&mut payload_reader, &mut writer)
        .map_err(format_car_error)?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush `{}`: {err}", car_out.display()))?;
    if stats.chunk_profile != descriptor.profile {
        return Err("emitted governance DAG CAR used unexpected chunk profile".to_string());
    }
    if let Some(plan_path) = car_plan_out {
        let plan_json = chunk_fetch_plan_to_string(&plan)
            .map_err(|err| format!("failed to render governance DAG CAR chunk plan: {err}"))?;
        write_text(plan_path, plan_json.as_bytes())?;
    }
    let files = plan
        .files
        .iter()
        .map(|file| {
            let mut obj = Map::new();
            insert_value!(obj["path"] = file.path.join("/"));
            insert_value!(obj["size"] = file.size);
            insert_value!(obj["first_chunk"] = file.first_chunk as u64);
            insert_value!(obj["chunk_count"] = file.chunk_count as u64);
            Value::Object(obj)
        })
        .collect::<Vec<_>>();
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.car.v1");
    insert_value!(summary["snapshot_root"] = snapshot_root.display().to_string());
    insert_value!(summary["output_car"] = car_out.display().to_string());
    insert_value!(summary["chunker_handle"] = chunker_handle.to_string());
    insert_value!(summary["chunker_profile_id"] = descriptor.id.0 as u64);
    insert_value!(
        summary["chunker_profile_canonical"] = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )
    );
    insert_value!(summary["payload_bytes"] = plan.content_length);
    insert_value!(summary["chunk_count"] = plan.chunks.len() as u64);
    insert_value!(summary["file_count"] = plan.files.len() as u64);
    insert_json!(summary["files"] = Value::Array(files));
    insert_value!(summary["car_size"] = stats.car_size);
    insert_value!(
        summary["car_payload_digest_hex"] = hex_encode(stats.car_payload_digest.as_bytes())
    );
    insert_value!(summary["car_digest_hex"] = hex_encode(stats.car_archive_digest.as_bytes()));
    insert_value!(summary["car_cid_hex"] = hex_encode(&stats.car_cid));
    insert_json!(
        summary["root_cids_hex"] = Value::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| Value::from(hex_encode(cid)))
                .collect(),
        )
    );
    if let Some(plan_path) = car_plan_out {
        insert_value!(summary["chunk_plan_path"] = plan_path.display().to_string());
    }
    Ok(Value::Object(summary))
}
fn verify_governance_dag_build_snapshot(
    root: &Path,
    require_sidecars: bool,
    expected_head_cid: Option<&[u8]>,
) -> (bool, Value) {
    let mut errors = Vec::<Value>::new();
    let mut warnings = Vec::<Value>::new();
    let mut head_value = Map::new();
    let mut block_values = Vec::<Value>::new();
    let mut decoded_head: Option<GovernanceDagHeadV1> = None;
    let mut decoded_blocks = Vec::<(String, GovernanceDagBlockV1)>::new();
    if !root.is_dir() {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "root",
            "governance DAG build root must be a directory",
        ));
    }
    let head_path = root.join("head.to");
    let head_rel = governance_dag_relative_path(root, &head_path);
    insert_value!(head_value["path"] = head_rel.clone());
    insert_value!(head_value["source_path"] = head_path.display().to_string());
    match fs::read(&head_path) {
        Ok(bytes) => {
            let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
            let (sidecar_status, sidecar_value, sidecar_error) =
                governance_dag_sidecar_status(&head_path, &blake3_hex);
            insert_value!(
                head_value["encoded_len"] = u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            );
            insert_value!(head_value["blake3"] = blake3_hex);
            insert_value!(head_value["sidecar_status"] = sidecar_status.clone());
            if let Some(value) = sidecar_value {
                insert_value!(head_value["sidecar_blake3"] = value);
            }
            if let Some(error) = &sidecar_error {
                insert_value!(head_value["sidecar_error"] = error.clone());
            }
            push_governance_dag_sidecar_problem(
                &head_rel,
                &sidecar_status,
                sidecar_error.as_deref(),
                require_sidecars,
                &mut warnings,
                &mut errors,
            );
            match decode_from_bytes::<GovernanceDagHeadV1>(&bytes) {
                Ok(head) => {
                    insert_value!(head_value["version"] = head.version);
                    insert_value!(head_value["block_count"] = head.block_count);
                    insert_value!(head_value["generated_at"] = head.generated_at);
                    insert_value!(
                        head_value["publisher_peer_id"] =
                            String::from_utf8_lossy(&head.publisher_peer_id).to_string()
                    );
                    insert_value!(head_value["head_block_cid"] = cid_display(&head.head_block_cid));
                    insert_value!(
                        head_value["head_block_cid_hex"] = hex_encode(&head.head_block_cid)
                    );
                    insert_json!(
                        head_value["checkpoint_cid_hex"] = head
                            .checkpoint_cid
                            .as_ref()
                            .map(hex_encode)
                            .map_or(Value::Null, Value::from)
                    );
                    decoded_head = Some(head);
                }
                Err(err) => {
                    let message = format!("failed to decode GovernanceDagHeadV1: {err}");
                    insert_value!(head_value["decode_error"] = message.clone());
                    errors.push(governance_dag_problem(&head_rel, "decode_head", message));
                }
            }
        }
        Err(err) => {
            let message = format!("failed to read governance DAG head: {err}");
            insert_value!(head_value["read_error"] = message.clone());
            errors.push(governance_dag_problem(&head_rel, "head", message));
        }
    }
    let blocks_dir = root.join("blocks");
    let mut block_paths = Vec::<PathBuf>::new();
    if blocks_dir.is_dir() {
        if let Err(err) = collect_governance_dag_to_files(&blocks_dir, &mut block_paths) {
            errors.push(governance_dag_problem(
                &governance_dag_relative_path(root, &blocks_dir),
                "blocks",
                err,
            ));
        }
    } else {
        errors.push(governance_dag_problem(
            &governance_dag_relative_path(root, &blocks_dir),
            "blocks",
            "governance DAG build snapshot is missing the `blocks` directory",
        ));
    }
    block_paths.sort();
    if block_paths.is_empty() {
        errors.push(governance_dag_problem(
            &governance_dag_relative_path(root, &blocks_dir),
            "blocks",
            "no GovernanceDagBlockV1 `.to` payloads found",
        ));
    }
    for block_path in &block_paths {
        let rel_path = governance_dag_relative_path(root, block_path);
        let mut block_value = Map::new();
        insert_value!(block_value["path"] = rel_path.clone());
        insert_value!(block_value["source_path"] = block_path.display().to_string());
        match fs::read(block_path) {
            Ok(bytes) => {
                let blake3_hex = hex_encode(blake3_hash(&bytes).as_bytes());
                let (sidecar_status, sidecar_value, sidecar_error) =
                    governance_dag_sidecar_status(block_path, &blake3_hex);
                insert_value!(
                    block_value["encoded_len"] = u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                );
                insert_value!(block_value["blake3"] = blake3_hex);
                insert_value!(block_value["sidecar_status"] = sidecar_status.clone());
                if let Some(value) = sidecar_value {
                    insert_value!(block_value["sidecar_blake3"] = value);
                }
                if let Some(error) = &sidecar_error {
                    insert_value!(block_value["sidecar_error"] = error.clone());
                }
                push_governance_dag_sidecar_problem(
                    &rel_path,
                    &sidecar_status,
                    sidecar_error.as_deref(),
                    require_sidecars,
                    &mut warnings,
                    &mut errors,
                );
                match decode_from_bytes::<GovernanceDagBlockV1>(&bytes) {
                    Ok(block) => {
                        insert_value!(block_value["version"] = block.version);
                        insert_value!(block_value["sequence"] = block.sequence);
                        insert_value!(block_value["timestamp"] = block.timestamp);
                        insert_value!(
                            block_value["publisher_peer_id"] =
                                String::from_utf8_lossy(&block.publisher_peer_id).to_string()
                        );
                        insert_value!(block_value["block_cid"] = cid_display(&block.block_cid));
                        insert_value!(block_value["block_cid_hex"] = hex_encode(&block.block_cid));
                        insert_json!(
                            block_value["prev_block_cid_hex"] = block
                                .prev_block_cid
                                .as_ref()
                                .map(hex_encode)
                                .map_or(Value::Null, Value::from)
                        );
                        insert_value!(block_value["node_cid"] = cid_display(&block.node.node_cid));
                        insert_value!(
                            block_value["node_cid_hex"] = hex_encode(&block.node.node_cid)
                        );
                        insert_value!(
                            block_value["payload_kind"] =
                                governance_payload_kind_cli(&block.node.payload)
                        );
                        insert_governance_node_submission_summary(&mut block_value, &block.node);
                        decoded_blocks.push((rel_path, block));
                    }
                    Err(err) => {
                        let message = format!("failed to decode GovernanceDagBlockV1: {err}");
                        insert_value!(block_value["decode_error"] = message.clone());
                        errors.push(governance_dag_problem(&rel_path, "decode_block", message));
                    }
                }
            }
            Err(err) => {
                let message = format!("failed to read governance DAG block: {err}");
                insert_value!(block_value["read_error"] = message.clone());
                errors.push(governance_dag_problem(&rel_path, "block", message));
            }
        }
        block_values.push(Value::Object(block_value));
    }
    decoded_blocks.sort_by(|left, right| {
        left.1
            .sequence
            .cmp(&right.1.sequence)
            .then_with(|| left.0.cmp(&right.0))
    });
    let blocks = decoded_blocks
        .into_iter()
        .map(|(_, block)| block)
        .collect::<Vec<_>>();
    if let Some(head) = decoded_head.as_ref() {
        if let Some(expected) = expected_head_cid
            && head.head_block_cid != expected
        {
            errors.push(governance_dag_problem(
                &head_rel,
                "head_cid",
                format!(
                    "expected head CID `{}` but snapshot advertises `{}`",
                    cid_display(expected),
                    cid_display(&head.head_block_cid)
                ),
            ));
        }
        if let Err(err) = validate_governance_dag_head_against_chain_v1(head, &blocks) {
            errors.push(governance_dag_problem(
                &head_rel,
                "head_chain",
                format!("governance DAG block/head validation failed: {err}"),
            ));
        }
    }
    let mut summary = Map::new();
    insert_value!(summary["schema"] = "sorafs.governance_dag.build.verify.v1");
    insert_value!(summary["root"] = root.display().to_string());
    insert_value!(summary["ok"] = errors.is_empty());
    insert_value!(summary["require_sidecars"] = require_sidecars);
    insert_json!(
        summary["expected_head_cid"] = expected_head_cid
            .map(cid_display)
            .map_or(Value::Null, Value::from)
    );
    insert_json!(
        summary["expected_head_cid_hex"] = expected_head_cid
            .map(hex_encode)
            .map_or(Value::Null, Value::from)
    );
    insert_json!(summary["head"] = Value::Object(head_value));
    insert_value!(summary["block_file_count"] = block_paths.len() as u64);
    insert_value!(summary["block_count"] = blocks.len() as u64);
    insert_json!(summary["blocks"] = Value::Array(block_values));
    insert_json!(summary["warnings"] = Value::Array(warnings));
    insert_json!(summary["errors"] = Value::Array(errors.clone()));
    (errors.is_empty(), Value::Object(summary))
}
fn load_governance_dag_inventory(root: &Path) -> Result<Vec<GovernanceDagArtifact>, String> {
    if !root.is_dir() {
        return Err(format!(
            "governance DAG root `{}` must be a directory",
            root.display()
        ));
    }
    let mut paths = Vec::new();
    collect_governance_dag_to_files(root, &mut paths)?;
    paths.sort();
    paths
        .iter()
        .map(|path| read_governance_dag_artifact(root, path))
        .collect()
}
fn collect_governance_dag_to_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|err| {
        format!(
            "failed to read governance DAG directory `{}`: {err}",
            root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read governance DAG directory entry in `{}`: {err}",
                root.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "failed to inspect governance DAG path `{}`: {err}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            collect_governance_dag_to_files(&path, out)?;
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("to")
        {
            out.push(path);
        }
    }
    Ok(())
}
fn governance_dag_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn read_governance_dag_artifact(root: &Path, path: &Path) -> Result<GovernanceDagArtifact, String> {
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read governance DAG artifact `{}`: {err}",
            path.display()
        )
    })?;
    let digest = blake3_hash(&bytes);
    let blake3_hex = digest.to_hex().to_string();
    let rel_path = governance_dag_relative_path(root, path);
    let (sidecar_status, sidecar_value, sidecar_error) =
        governance_dag_sidecar_status(path, &blake3_hex);
    let (node, decode_error, outcome) = match decode_from_bytes::<GovernanceLogNodeV1>(&bytes) {
        Ok(node) => {
            let summary = GovernanceDagNodeSummary::from_node(&node);
            let outcome = validate_governance_log_node_bytes(
                &bytes,
                rel_path.clone(),
                Some(node.node_cid.as_slice()),
                governance_dag_now_secs(),
            );
            (Some(summary), None, Some(outcome))
        }
        Err(err) => (None, Some(err.to_string()), None),
    };
    Ok(GovernanceDagArtifact {
        path: path.to_path_buf(),
        rel_path,
        encoded_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        blake3_hex,
        sidecar_status,
        sidecar_value,
        sidecar_error,
        node,
        decode_error,
        outcome,
    })
}
fn read_governance_log_node_file(path: &Path) -> Result<GovernanceLogNodeV1, String> {
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read governance log node `{}`: {err}",
            path.display()
        )
    })?;
    decode_from_bytes::<GovernanceLogNodeV1>(&bytes).map_err(|err| {
        format!(
            "failed to decode governance log node `{}`: {err}",
            path.display()
        )
    })
}
fn governance_dag_build_order(
    artifacts: &[GovernanceDagArtifact],
    node_indices: &[usize],
) -> Vec<usize> {
    let mut ordered = node_indices.to_vec();
    ordered.sort_by(|left, right| {
        let left_artifact = &artifacts[*left];
        let right_artifact = &artifacts[*right];
        let left_node = left_artifact.node.as_ref();
        let right_node = right_artifact.node.as_ref();
        left_node
            .map(|node| node.timestamp)
            .cmp(&right_node.map(|node| node.timestamp))
            .then_with(|| {
                left_node
                    .map(|node| node.node_cid.as_slice())
                    .cmp(&right_node.map(|node| node.node_cid.as_slice()))
            })
            .then_with(|| left_artifact.rel_path.cmp(&right_artifact.rel_path))
    });
    ordered
}
fn load_governance_dag_build_seed(
    key_hex: Option<&str>,
    key_path: Option<&Path>,
) -> Result<[u8; ed25519_dalek::SECRET_KEY_LENGTH], String> {
    let raw = match (key_hex, key_path) {
        (Some(_), Some(_)) => {
            return Err(
                "governance DAG build key flags are mutually exclusive; provide `--key-hex` or `--key`"
                    .to_string(),
            );
        }
        (Some(value), None) => value.trim().to_string(),
        (None, Some(path)) => fs::read_to_string(path)
            .map_err(|err| {
                format!(
                    "failed to read governance DAG signing key `{}`: {err}",
                    path.display()
                )
            })?
            .trim()
            .to_string(),
        (None, None) => {
            return Err(
                "missing governance DAG signing key; provide `--key-hex=HEX` or `--key=PATH`"
                    .to_string(),
            );
        }
    };
    let trimmed = raw.strip_prefix("ed25519:").unwrap_or(raw.as_str()).trim();
    let bytes = parse_hex_vec(trimmed).map_err(|err| {
        format!("failed to parse governance DAG Ed25519 seed hex for `--key-hex`/`--key`: {err}")
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        format!(
            "governance DAG Ed25519 seed must be {} bytes, found {} bytes",
            ed25519_dalek::SECRET_KEY_LENGTH,
            bytes.len()
        )
    })
}
fn empty_governance_dag_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}
fn sign_governance_dag_block_cli(
    block: &mut GovernanceDagBlockV1,
    signing_key: &SigningKey,
) -> Result<(), String> {
    let payload = block
        .signature_payload_bytes()
        .map_err(|err| format!("failed to encode governance DAG block signing payload: {err}"))?;
    let signature = signing_key.sign(&payload);
    block.block_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    Ok(())
}
fn sign_governance_dag_head_cli(
    head: &mut GovernanceDagHeadV1,
    signing_key: &SigningKey,
) -> Result<(), String> {
    let payload = head
        .signature_payload_bytes()
        .map_err(|err| format!("failed to encode governance DAG head signing payload: {err}"))?;
    let signature = signing_key.sign(&payload);
    head.head_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
    Ok(())
}
fn write_governance_blake3_sidecar(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let sidecar_path = path.with_extension("to.blake3");
    let mut digest = hex_encode(blake3_hash(bytes).as_bytes());
    digest.push('\n');
    let sidecar_bytes = digest.into_bytes();
    write_text(&sidecar_path, &sidecar_bytes)?;
    Ok(sidecar_bytes)
}
impl GovernanceDagNodeSummary {
    fn from_node(node: &GovernanceLogNodeV1) -> Self {
        let (submission_publisher_account_digest_hex, submission_origin) =
            governance_submission_summary(node);
        Self {
            node_cid: node.node_cid.clone(),
            node_cid_label: cid_display(&node.node_cid),
            node_cid_hex: hex_encode(&node.node_cid),
            prev_cid: node.prev_cid.clone(),
            prev_cid_label: node.prev_cid.as_ref().map(|cid| cid_display(cid)),
            prev_cid_hex: node.prev_cid.as_ref().map(hex_encode),
            timestamp: node.timestamp,
            publisher_peer_id: String::from_utf8_lossy(&node.publisher_peer_id).to_string(),
            submission_publisher_account_digest_hex,
            submission_origin,
            payload_kind: governance_payload_kind_cli(&node.payload),
        }
    }
}
fn governance_dag_sidecar_status(
    path: &Path,
    blake3_hex: &str,
) -> (String, Option<String>, Option<String>) {
    let sidecar_path = path.with_extension("to.blake3");
    match fs::read_to_string(&sidecar_path) {
        Ok(raw) => {
            let sidecar_value = raw.trim().to_string();
            let status = if sidecar_value == blake3_hex {
                "match"
            } else {
                "mismatch"
            };
            (status.to_string(), Some(sidecar_value), None)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => ("missing".to_string(), None, None),
        Err(err) => ("error".to_string(), None, Some(err.to_string())),
    }
}
fn push_governance_dag_sidecar_problem(
    path: &str,
    status: &str,
    error: Option<&str>,
    require_sidecars: bool,
    warnings: &mut Vec<Value>,
    errors: &mut Vec<Value>,
) {
    match status {
        "mismatch" => errors.push(governance_dag_problem(
            path,
            "sidecar",
            "BLAKE3 sidecar does not match encoded bytes",
        )),
        "error" => errors.push(governance_dag_problem(
            path,
            "sidecar",
            format!(
                "failed to inspect BLAKE3 sidecar: {}",
                error.unwrap_or("unknown error")
            ),
        )),
        "missing" if require_sidecars => errors.push(governance_dag_problem(
            path,
            "sidecar",
            "missing required `.to.blake3` sidecar",
        )),
        "missing" => warnings.push(governance_dag_problem(
            path,
            "sidecar",
            "missing optional `.to.blake3` sidecar",
        )),
        _ => {}
    }
}
fn governance_payload_kind_cli(payload: &GovernanceLogPayloadV1) -> &'static str {
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(_) => "provider_advert",
        GovernanceLogPayloadV1::ReplicationOrder(_) => "replication_order",
        GovernanceLogPayloadV1::PorChallengePublication(_) => "por_challenge_publication",
        GovernanceLogPayloadV1::PorProof(_) => "por_proof",
        GovernanceLogPayloadV1::PdpArchive(_) => "pdp_archive",
        GovernanceLogPayloadV1::AuditVerdict(_) => "audit_verdict",
        GovernanceLogPayloadV1::DealSettlement(_) => "deal_settlement",
        GovernanceLogPayloadV1::ModerationBallotEvent(_) => "moderation_ballot_event",
        GovernanceLogPayloadV1::AppealFinanceReport(_) => "appeal_finance_report",
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(_) => "appeal_finance_weekly_rollup",
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(_) => {
            "appeal_finance_settlement_receipt"
        }
        GovernanceLogPayloadV1::OrderbookSettlementReceipt(_) => "orderbook_settlement_receipt",
        GovernanceLogPayloadV1::ExternalPayload(_) => "external_payload",
        GovernanceLogPayloadV1::SignedReputationSnapshot(_) => "reputation_snapshot",
        GovernanceLogPayloadV1::PorWeeklyReport(_) => "por_weekly_report",
    }
}
fn verify_governance_dag_inventory(
    root: &Path,
    artifacts: &[GovernanceDagArtifact],
    options: &GovernanceDagVerifyOptions,
) -> (bool, Value, Vec<usize>) {
    let mut errors = Vec::<Value>::new();
    let mut warnings = Vec::<Value>::new();
    let mut node_indices = Vec::<usize>::new();
    let mut node_by_cid = BTreeMap::<Vec<u8>, usize>::new();
    let mut referenced_prev = BTreeSet::<Vec<u8>>::new();
    for (index, artifact) in artifacts.iter().enumerate() {
        match artifact.sidecar_status.as_str() {
            "mismatch" | "error" => errors.push(governance_dag_problem(
                &artifact.rel_path,
                "sidecar",
                format!("BLAKE3 sidecar status is `{}`", artifact.sidecar_status),
            )),
            "missing" if options.require_sidecars => errors.push(governance_dag_problem(
                &artifact.rel_path,
                "sidecar",
                "missing required `.to.blake3` sidecar",
            )),
            "missing" => warnings.push(governance_dag_problem(
                &artifact.rel_path,
                "sidecar",
                "missing optional `.to.blake3` sidecar",
            )),
            _ => {}
        }
        if let Some(node) = &artifact.node {
            node_indices.push(index);
            if let Some(existing) = node_by_cid.insert(node.node_cid.clone(), index) {
                errors.push(governance_dag_problem(
                    &artifact.rel_path,
                    "duplicate_node_cid",
                    format!("node CID duplicates `{}`", artifacts[existing].rel_path),
                ));
            }
            if let Some(prev) = &node.prev_cid {
                referenced_prev.insert(prev.clone());
            }
            if !artifact
                .outcome
                .as_ref()
                .is_some_and(ValidationOutcomeV1::is_ok)
            {
                let code = artifact
                    .outcome
                    .as_ref()
                    .map(|outcome| outcome.code.as_str())
                    .unwrap_or("decode-error");
                errors.push(governance_dag_problem(
                    &artifact.rel_path,
                    "validation",
                    format!("governance node failed reference validation with `{code}`"),
                ));
            }
        }
    }
    if node_indices.is_empty() {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "inventory",
            "no GovernanceLogNodeV1 `.to` payloads found",
        ));
    }
    if options.require_chain {
        for index in &node_indices {
            let artifact = &artifacts[*index];
            let Some(node) = &artifact.node else {
                continue;
            };
            if let Some(prev) = &node.prev_cid
                && !node_by_cid.contains_key(prev)
            {
                errors.push(governance_dag_problem(
                    &artifact.rel_path,
                    "missing_parent",
                    format!(
                        "previous CID `{}` is not present in this archive",
                        cid_display(prev)
                    ),
                ));
            }
        }
    }
    let mut heads = Vec::<Vec<u8>>::new();
    for index in &node_indices {
        let Some(node) = &artifacts[*index].node else {
            continue;
        };
        if !referenced_prev.contains(&node.node_cid) {
            heads.push(node.node_cid.clone());
        }
    }
    heads.sort();
    if options.require_chain && heads.len() != 1 {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "head_count",
            format!(
                "expected exactly one governance DAG head, found {}",
                heads.len()
            ),
        ));
    }
    if let Some(expected_head) = &options.expected_head_cid
        && !heads.iter().any(|head| head == expected_head)
    {
        errors.push(governance_dag_problem(
            root.to_string_lossy().as_ref(),
            "head_cid",
            format!(
                "expected head CID `{}` is not an archive head",
                cid_display(expected_head)
            ),
        ));
    }
    let mut summary = governance_dag_inventory_value(root, artifacts);
    if let Value::Object(ref mut obj) = summary {
        insert_value!(obj["ok"] = errors.is_empty());
        insert_value!(obj["require_chain"] = options.require_chain);
        insert_value!(obj["require_sidecars"] = options.require_sidecars);
        insert_json!(
            obj["head_cids"] = Value::Array(
                heads
                    .iter()
                    .map(|cid| Value::from(cid_display(cid)))
                    .collect(),
            )
        );
        insert_json!(
            obj["head_cid_hex"] = Value::Array(
                heads
                    .iter()
                    .map(|cid| Value::from(hex_encode(cid)))
                    .collect(),
            )
        );
        if let Some(expected) = &options.expected_head_cid {
            insert_value!(obj["expected_head_cid"] = cid_display(expected));
            insert_value!(obj["expected_head_cid_hex"] = hex_encode(expected));
        }
        insert_json!(obj["warnings"] = Value::Array(warnings));
        insert_json!(obj["errors"] = Value::Array(errors.clone()));
    }
    (errors.is_empty(), summary, node_indices)
}
fn governance_dag_problem(
    path: &str,
    kind: impl Into<String>,
    message: impl Into<String>,
) -> Value {
    let mut obj = Map::new();
    insert_value!(obj["path"] = path.to_string());
    insert_value!(obj["kind"] = kind.into());
    insert_value!(obj["message"] = message.into());
    Value::Object(obj)
}
fn governance_dag_inventory_value(root: &Path, artifacts: &[GovernanceDagArtifact]) -> Value {
    let node_count = artifacts
        .iter()
        .filter(|artifact| artifact.node.is_some())
        .count();
    let valid_node_count = artifacts
        .iter()
        .filter(|artifact| {
            artifact
                .outcome
                .as_ref()
                .is_some_and(ValidationOutcomeV1::is_ok)
        })
        .count();
    let sidecar_mismatch_count = artifacts
        .iter()
        .filter(|artifact| matches!(artifact.sidecar_status.as_str(), "mismatch" | "error"))
        .count();
    let sidecar_missing_count = artifacts
        .iter()
        .filter(|artifact| artifact.sidecar_status == "missing")
        .count();
    let mut obj = Map::new();
    insert_value!(obj["schema"] = "sorafs.governance_dag.inventory.v1");
    insert_value!(obj["root"] = root.display().to_string());
    insert_value!(obj["artifact_count"] = artifacts.len() as u64);
    insert_value!(obj["node_count"] = node_count as u64);
    insert_value!(obj["valid_node_count"] = valid_node_count as u64);
    insert_value!(obj["sidecar_mismatch_count"] = sidecar_mismatch_count as u64);
    insert_value!(obj["sidecar_missing_count"] = sidecar_missing_count as u64);
    insert_json!(
        obj["artifacts"] = Value::Array(
            artifacts
                .iter()
                .map(|artifact| governance_dag_artifact_value(artifact, false))
                .collect(),
        )
    );
    Value::Object(obj)
}
fn governance_dag_artifact_value(artifact: &GovernanceDagArtifact, include_outcome: bool) -> Value {
    let mut obj = Map::new();
    insert_value!(obj["path"] = artifact.rel_path.clone());
    insert_value!(obj["source_path"] = artifact.path.display().to_string());
    insert_value!(obj["encoded_len"] = artifact.encoded_len);
    insert_value!(obj["blake3"] = artifact.blake3_hex.clone());
    insert_value!(obj["sidecar_status"] = artifact.sidecar_status.clone());
    if let Some(value) = &artifact.sidecar_value {
        insert_value!(obj["sidecar_blake3"] = value.clone());
    }
    if let Some(error) = &artifact.sidecar_error {
        insert_value!(obj["sidecar_error"] = error.clone());
    }
    if let Some(node) = &artifact.node {
        insert_json!(obj["node"] = governance_dag_node_value(node));
        if let Some(outcome) = &artifact.outcome {
            insert_value!(obj["validation_status"] = outcome.status.clone());
            insert_value!(obj["validation_code"] = outcome.code.clone());
            if include_outcome {
                insert_json!(obj["validation_outcome"] = to_value(outcome).unwrap_or(Value::Null));
            }
        }
    } else if let Some(error) = &artifact.decode_error {
        insert_value!(obj["validation_status"] = "not_governance_node");
        insert_value!(obj["decode_error"] = error.clone());
    }
    Value::Object(obj)
}
fn governance_dag_node_value(node: &GovernanceDagNodeSummary) -> Value {
    let mut obj = Map::new();
    insert_value!(obj["node_cid"] = node.node_cid_label.clone());
    insert_value!(obj["node_cid_hex"] = node.node_cid_hex.clone());
    insert_json!(obj["prev_cid"] = node.prev_cid_label.clone().map_or(Value::Null, Value::from));
    insert_json!(obj["prev_cid_hex"] = node.prev_cid_hex.clone().map_or(Value::Null, Value::from));
    insert_value!(obj["timestamp"] = node.timestamp);
    insert_value!(obj["publisher_peer_id"] = node.publisher_peer_id.clone());
    insert_governance_submission_summary(
        &mut obj,
        node.submission_publisher_account_digest_hex.as_deref(),
        node.submission_origin,
    );
    insert_value!(obj["payload_kind"] = node.payload_kind);
    Value::Object(obj)
}
fn print_governance_dag_inventory_table(root: &Path, artifacts: &[GovernanceDagArtifact]) {
    println!("root: {}", root.display());
    println!(
        "path\tkind\tsubmission_account_digest_hex\tsubmission_origin\tvalidation\tsidecar\tblake3"
    );
    for artifact in artifacts {
        let kind = artifact
            .node
            .as_ref()
            .map(|node| node.payload_kind)
            .unwrap_or("raw");
        let validation = artifact
            .outcome
            .as_ref()
            .map(|outcome| outcome.status.as_str())
            .unwrap_or("not_governance_node");
        let submission_account_digest_hex = artifact
            .node
            .as_ref()
            .and_then(|node| node.submission_publisher_account_digest_hex.as_deref())
            .unwrap_or("-");
        let submission_origin = artifact
            .node
            .as_ref()
            .and_then(|node| node.submission_origin)
            .unwrap_or("-");
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            artifact.rel_path,
            kind,
            submission_account_digest_hex,
            submission_origin,
            validation,
            artifact.sidecar_status,
            artifact.blake3_hex
        );
    }
}
fn print_governance_dag_artifact_table(artifact: &GovernanceDagArtifact) {
    println!("path: {}", artifact.rel_path);
    println!("encoded_len: {}", artifact.encoded_len);
    println!("blake3: {}", artifact.blake3_hex);
    println!("sidecar_status: {}", artifact.sidecar_status);
    if let Some(node) = &artifact.node {
        println!("node_cid: {}", node.node_cid_label);
        println!("node_cid_hex: {}", node.node_cid_hex);
        println!(
            "prev_cid: {}",
            node.prev_cid_label.as_deref().unwrap_or("<root>")
        );
        println!("timestamp: {}", node.timestamp);
        println!("publisher_peer_id: {}", node.publisher_peer_id);
        println!(
            "submission_publisher_account_digest_hex: {}",
            node.submission_publisher_account_digest_hex
                .as_deref()
                .unwrap_or("-")
        );
        println!(
            "submission_origin: {}",
            node.submission_origin.unwrap_or("-")
        );
        println!("payload_kind: {}", node.payload_kind);
    }
    if let Some(outcome) = &artifact.outcome {
        println!("validation_status: {}", outcome.status);
        println!("validation_code: {}", outcome.code);
        println!("validation_message: {}", outcome.message);
    } else if let Some(error) = &artifact.decode_error {
        println!("validation_status: not_governance_node");
        println!("decode_error: {error}");
    }
}
fn write_governance_dag_json(path: &Path, value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|err| format!("failed to render governance DAG JSON: {err}"))?;
    write_text(path, rendered.as_bytes())
}
fn print_governance_dag_json(value: &Value) -> Result<(), String> {
    let rendered = to_string_pretty(value)
        .map_err(|err| format!("failed to render governance DAG JSON: {err}"))?;
    println!("{rendered}");
    Ok(())
}
fn parse_governance_cid_arg(raw: &str) -> Result<Vec<u8>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("governance DAG CID argument must not be empty".to_string());
    }
    if let Some(hex) = trimmed.strip_prefix("hex:") {
        return hex::decode(hex)
            .map_err(|err| format!("failed to decode governance DAG hex CID `{trimmed}`: {err}"));
    }
    if trimmed.len().is_multiple_of(2) && trimmed.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return hex::decode(trimmed)
            .map_err(|err| format!("failed to decode governance DAG hex CID `{trimmed}`: {err}"));
    }
    Ok(trimmed.as_bytes().to_vec())
}
fn cid_display(cid: &[u8]) -> String {
    match std::str::from_utf8(cid) {
        Ok(value) if !value.is_empty() && value.chars().all(|ch| !ch.is_control()) => {
            value.to_string()
        }
        _ => format!("hex:{}", hex_encode(cid)),
    }
}
fn governance_dag_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}
fn generate_proof_stream_nonce(
    manifest_digest: &[u8],
    proof_kind: ProofKind,
    challenge_id_hex: Option<&str>,
    sample_count: Option<u32>,
    deadline_ms: Option<u32>,
    provider_id: Option<&str>,
) -> [u8; 16] {
    let mut buffer = Vec::with_capacity(manifest_digest.len() + 48);
    buffer.extend_from_slice(manifest_digest);
    buffer.extend_from_slice(proof_kind.as_str().as_bytes());
    if let Some(challenge_id) = challenge_id_hex {
        buffer.extend_from_slice(challenge_id.as_bytes());
    }
    let count_bytes = sample_count.unwrap_or(0).to_le_bytes();
    buffer.extend_from_slice(&count_bytes);
    if let Some(deadline) = deadline_ms {
        buffer.extend_from_slice(&deadline.to_le_bytes());
    }
    if let Some(provider) = provider_id {
        buffer.extend_from_slice(provider.as_bytes());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_nanos()
        .to_le_bytes();
    buffer.extend_from_slice(&timestamp);
    let digest = blake3_hash(&buffer);
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&digest.as_bytes()[..16]);
    nonce
}
fn decode_nonce_b64(input: &str) -> Result<[u8; 16], String> {
    if input.is_empty() {
        return Err("`--nonce-b64` may not be empty".to_string());
    }
    let decoded = BASE64_STANDARD
        .decode(input.as_bytes())
        .map_err(|err| format!("invalid `--nonce-b64` value: {err}"))?;
    if decoded.len() != 16 {
        return Err(format!(
            "`--nonce-b64` must decode to 16 bytes, found {} bytes",
            decoded.len()
        ));
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&decoded);
    if out.iter().all(|byte| *byte == 0) {
        return Err("`--nonce-b64` must decode to a non-zero nonce".to_string());
    }
    if BASE64_STANDARD.encode(out) != input {
        return Err("`--nonce-b64` must use canonical padded base64".to_string());
    }
    Ok(out)
}
#[derive(Debug, Clone)]
struct AliasInputs {
    namespace: String,
    name: String,
    proof: Vec<u8>,
}
fn parse_private_key_inline(value: &str) -> Result<PrivateKey, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("`--private-key` may not be empty".to_string());
    }
    PrivateKey::from_str(trimmed).map_err(|err| format!("failed to parse private key: {err}"))
}
fn load_private_key_from_file(path: &Path) -> Result<PrivateKey, String> {
    let contents = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read private key from `{}`: {err}",
            path.display()
        )
    })?;
    parse_private_key_inline(&contents)
}
fn alias_inputs_from_flags(
    namespace: Option<String>,
    name: Option<String>,
    proof_path: Option<PathBuf>,
) -> Result<Option<AliasInputs>, String> {
    match (namespace, name, proof_path) {
        (None, None, None) => Ok(None),
        (Some(ns), Some(name), Some(path)) => {
            let proof = fs::read(&path)
                .map_err(|err| format!("failed to read alias proof `{}`: {err}", path.display()))?;
            if proof.is_empty() {
                return Err(format!("alias proof file `{}` is empty", path.display()));
            }
            Ok(Some(AliasInputs {
                namespace: ns,
                name,
                proof,
            }))
        }
        _ => Err(
            "alias namespace, name, and proof must be provided together for `--alias-*` flags"
                .to_string(),
        ),
    }
}
fn build_pin_register_transaction(
    network_id: &NetworkId,
    authority: &AccountId,
    private_key: &PrivateKey,
    manifest: &ManifestV1,
    alias: Option<&AliasInputs>,
    successor_of: Option<[u8; 32]>,
) -> Result<iroha_data_model::transaction::SignedTransaction, String> {
    let manifest_payload = manifest
        .encode()
        .map_err(|err| format!("failed to encode canonical manifest payload: {err}"))?;
    let alias = alias.map(|alias| ManifestAliasBinding {
        namespace: alias.namespace.clone(),
        name: alias.name.clone(),
        proof: alias.proof.clone(),
    });
    let successor_of = successor_of
        .map(|successor| {
            if successor == [0; 32] {
                return Err("successor_of must not be the all-zero digest".to_owned());
            }
            Ok(ManifestDigest::new(successor))
        })
        .transpose()?;
    let instruction = RegisterPinManifest::new(manifest_payload, alias, successor_of);
    TransactionBuilder::new(
        *network_id,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .with_metadata(Metadata::default())
    .try_sign(private_key)
    .map_err(|err| format!("failed to sign pin-register transaction locally: {err}"))
}
struct ManifestProposalSummary<'a> {
    manifest_path: &'a Path,
    manifest: &'a ManifestV1,
    manifest_digest: &'a blake3::Hash,
    chunk_digest_sha3: [u8; 32],
    chunk_plan_label: Option<&'a str>,
    alias_hint: Option<&'a str>,
    successor_bytes: Option<[u8; 32]>,
}
fn build_manifest_proposal_summary(summary: ManifestProposalSummary<'_>) -> Result<Value, String> {
    let ManifestProposalSummary {
        manifest_path,
        manifest,
        manifest_digest,
        chunk_digest_sha3,
        chunk_plan_label,
        alias_hint,
        successor_bytes,
    } = summary;
    let chunker_handle = chunker_handle_from_profile(&manifest.chunking);
    let policy_dm = convert_pin_policy(&manifest.pin_policy);
    let register_value = build_register_instruction_value(
        manifest_digest,
        &chunker_handle,
        chunk_digest_sha3,
        &policy_dm,
        successor_bytes,
    );
    let mut map = Map::new();
    insert_value!(map["proposal_version"] = 1_u64);
    insert_value!(map["manifest_path"] = manifest_path.display().to_string());
    insert_value!(map["manifest_digest_hex"] = hex_encode(manifest_digest.as_bytes()));
    insert_value!(map["chunk_digest_sha3_hex"] = hex_encode(chunk_digest_sha3));
    insert_value!(map["chunker_handle"] = chunker_handle.to_handle());
    insert_json!(map["pin_policy"] = Value::Object(pin_policy_json(&manifest.pin_policy)));
    if let Some(label) = chunk_plan_label {
        insert_value!(map["chunk_plan_source"] = label);
    }
    if let Some(alias) = alias_hint {
        insert_value!(map["alias_hint"] = alias);
    }
    if let Some(bytes) = successor_bytes {
        insert_value!(map["successor_of_hex"] = hex_encode(bytes));
    }
    insert_json!(map["register_instruction"] = register_value);
    Ok(Value::Object(map))
}
fn chunker_handle_from_profile(profile: &ChunkingProfileV1) -> ChunkerProfileHandle {
    ChunkerProfileHandle {
        profile_id: profile.profile_id.0,
        namespace: profile.namespace.clone(),
        name: profile.name.clone(),
        semver: profile.semver.clone(),
        multihash_code: profile.multihash_code,
    }
}
fn convert_pin_policy(policy: &sorafs_manifest::PinPolicy) -> RegistryPinPolicy {
    RegistryPinPolicy {
        min_replicas: policy.min_replicas,
        storage_class: convert_storage_class(&policy.storage_class),
        retention_epoch: policy.retention_epoch,
    }
}
fn convert_storage_class(class: &sorafs_manifest::StorageClass) -> RegistryStorageClass {
    match class {
        sorafs_manifest::StorageClass::Hot => RegistryStorageClass::Hot,
        sorafs_manifest::StorageClass::Warm => RegistryStorageClass::Warm,
        sorafs_manifest::StorageClass::Cold => RegistryStorageClass::Cold,
    }
}
fn build_register_instruction_value(
    manifest_digest: &blake3::Hash,
    chunker_handle: &ChunkerProfileHandle,
    chunk_digest_sha3: [u8; 32],
    policy: &RegistryPinPolicy,
    successor_bytes: Option<[u8; 32]>,
) -> Value {
    let mut register_map = Map::new();
    insert_value!(register_map["digest_hex"] = hex_encode(manifest_digest.as_bytes()));
    insert_value!(register_map["chunker_handle"] = chunker_handle.to_handle());
    insert_value!(register_map["chunk_digest_sha3_256_hex"] = hex_encode(chunk_digest_sha3));
    insert_json!(register_map["policy"] = registry_pin_policy_to_value(policy));
    if let Some(bytes) = successor_bytes {
        insert_value!(register_map["successor_of_hex"] = hex_encode(bytes));
    }
    Value::Object(register_map)
}
fn registry_pin_policy_to_value(policy: &RegistryPinPolicy) -> Value {
    let mut map = Map::new();
    insert_value!(map["min_replicas"] = policy.min_replicas);
    insert_value!(
        map["storage_class"] = match policy.storage_class {
            RegistryStorageClass::Hot => "hot",
            RegistryStorageClass::Warm => "warm",
            RegistryStorageClass::Cold => "cold",
        }
    );
    insert_value!(map["retention_epoch"] = policy.retention_epoch);
    Value::Object(map)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
    use iroha_data_model::{
        metadata::Metadata,
        sorafs::pin_registry::{
            ManifestDigest, ManifestRootCid, PinManifestFinalizedCursorV1, PinManifestRecord,
        },
    };
    use norito::json::Map;
    use sorafs_car::{ChunkStore, por_json::sample_to_map};
    use sorafs_manifest::{
        GovernanceProofs, PinPolicy as ManifestPinPolicy, StorageClass as ManifestStorageClass,
    };
    use sorafs_orchestrator::{PolicyReport, PolicyStatus};
    use std::{fs, path::Path};
    use tempfile::tempdir;
    include!("sorafs_cli/appeal_verdict_parser_tests.rs");
    fn sample_manifest() -> ManifestV1 {
        let descriptor = sorafs_manifest::chunker_registry::default_descriptor();
        ManifestBuilder::new()
            .root_cid(sorafs_manifest::canonical_manifest_root_cid([0x01; 32]))
            .dag_codec(DagCodecId(MANIFEST_DAG_CODEC))
            .chunking_profile(ChunkingProfileV1::from_descriptor(descriptor))
            .chunk_digest_sha3_256([0xCD; 32])
            .por_root([0xCE; 32])
            .content_length(1_024)
            .car_digest([0xAB; 32])
            .car_size(2_048)
            .pin_policy(ManifestPinPolicy {
                min_replicas: 3,
                storage_class: ManifestStorageClass::Warm,
                retention_epoch: 64,
            })
            .governance(GovernanceProofs {
                council_signatures: Vec::new(),
            })
            .extend_metadata([("release".into(), "test".into())])
            .build()
            .expect("manifest build")
    }
    fn fixture_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive SoraFS CLI fixture key")
    }
    fn fixture_account(seed: u8) -> AccountId {
        AccountId::new(fixture_keypair(seed).public_key().clone())
    }
    #[test]
    fn sorafs_por_cursor_validation_is_strictly_canonical() {
        let cursor = PorStatusCursorV1 {
            version: sorafs_manifest::por::POR_STATUS_CURSOR_VERSION_V1,
            snapshot_generation: 7,
            selection_digest: [0x41; 32],
            last_epoch_id: 11,
            last_issued_at: 1_700_000_000,
            last_challenge_id: [0x42; 32],
        };
        let canonical = cursor.encode_opaque().expect("canonical cursor fixture");
        assert_eq!(
            validate_sorafs_por_cursor(&canonical, "cursor").expect("canonical bounded cursor"),
            cursor
        );
        for malformed in ["", "A", "AB", "AA=", "AA!", "AA\n"] {
            assert!(
                validate_sorafs_por_cursor(malformed, "cursor").is_err(),
                "cursor {malformed:?} must fail closed"
            );
        }
        assert!(
            validate_sorafs_por_cursor(
                &"A".repeat(POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1 + 1),
                "cursor",
            )
            .is_err()
        );
    }
    #[test]
    fn sorafs_por_response_decode_field_bound_is_exact() {
        assert!(por_status_response_bounds(0).is_none());
        assert!(
            por_status_response_bounds(POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1 + 1).is_none()
        );
        let response_bounds =
            por_status_response_bounds(POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1)
                .expect("protocol response bound");
        let response_max_bytes = response_bounds.response_max_bytes;
        let limits = response_bounds.decode_limits;
        assert_eq!(
            response_bounds.response_max_bytes_u64,
            u64::try_from(response_max_bytes).expect("response bound fits u64")
        );
        assert_eq!(
            response_bounds.response_read_limit,
            response_bounds.response_max_bytes_u64 + 1
        );
        assert_eq!(limits.max_field_bytes(), response_max_bytes);
        assert_eq!(
            limits.max_sequence_elements(),
            POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1
        );
        assert_eq!(
            limits.max_total_elements(),
            POR_STATUS_DECODE_MAX_TOTAL_ELEMENTS_V1
        );
        assert_eq!(
            limits.max_total_allocated_bytes(),
            response_max_bytes * POR_STATUS_DECODE_ALLOCATION_MULTIPLIER_V1
        );
        assert_eq!(
            limits.max_nesting_depth(),
            POR_STATUS_DECODE_MAX_NESTING_DEPTH_V1
        );
        let exact = u64::try_from(response_max_bytes)
            .expect("response bound fits u64")
            .to_le_bytes();
        norito::with_decode_limits(limits, || {
            norito::core::read_len_from_slice_with_flags(&exact, 0).map(|_| ())
        })
        .expect("the exact protocol field bound is accepted");
        let above = u64::try_from(response_max_bytes + 1)
            .expect("response bound plus one fits u64")
            .to_le_bytes();
        let error = norito::with_decode_limits(limits, || {
            norito::core::read_len_from_slice_with_flags(&above, 0).map(|_| ())
        })
        .expect_err("one byte above the protocol field bound is rejected");
        assert!(matches!(
            error,
            norito::Error::FieldLengthExceeded { length, limit }
                if length == u64::try_from(response_max_bytes + 1).expect("bound fits u64")
                    && limit == u64::try_from(response_max_bytes).expect("bound fits u64")
        ));
    }
    #[test]
    fn sorafs_por_status_filter_membership_is_exact() {
        let status = PorChallengeStatusV1 {
            version: 1,
            challenge_id: [0x11; 32],
            manifest_digest: [0x22; 32],
            provider_id: [0x33; 32],
            epoch_id: 42,
            drand_round: 100,
            status: PorChallengeOutcome::AwaitingProof,
            sample_count: 64,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: None,
            proof_digest: None,
            repair_task_id: None,
            failure_reason: None,
            verifier_latency_ms: None,
        };
        let exact = RequestedPorStatusFilter {
            manifest_digest: Some(status.manifest_digest),
            provider_id: Some(status.provider_id),
            epoch_id: Some(status.epoch_id),
            outcome: Some(status.status),
        };
        validate_por_status_filter_membership(std::slice::from_ref(&status), exact)
            .expect("exact status selection");
        for (filter, expected_field) in [
            (
                RequestedPorStatusFilter {
                    manifest_digest: Some([0x44; 32]),
                    ..exact
                },
                "manifest",
            ),
            (
                RequestedPorStatusFilter {
                    provider_id: Some([0x44; 32]),
                    ..exact
                },
                "provider",
            ),
            (
                RequestedPorStatusFilter {
                    epoch_id: Some(43),
                    ..exact
                },
                "epoch",
            ),
            (
                RequestedPorStatusFilter {
                    outcome: Some(PorChallengeOutcome::Verified),
                    ..exact
                },
                "outcome",
            ),
        ] {
            let error =
                validate_por_status_filter_membership(std::slice::from_ref(&status), filter)
                    .expect_err("substituted selection must fail closed");
            assert!(error.contains(expected_field), "unexpected error: {error}");
        }
    }
    fn fixture_reputation_auth(seed: u8, discriminant: u16) -> ReputationRequestAuth {
        let key_pair = fixture_keypair(seed);
        let account = AccountId::new(key_pair.public_key().clone());
        account
            .to_i105_for_discriminant(discriminant)
            .expect("encode reputation authentication account");
        ReputationRequestAuth {
            account_header_value: account
                .to_canonical_hex()
                .expect("encode reputation authentication header"),
            network_id: fixture_reputation_network_id(),
            key_pair,
        }
    }
    fn fixture_reputation_network_id() -> NetworkId {
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
            .parse()
            .expect("canonical reputation network identity")
    }
    fn write_reputation_private_key(path: &Path, key_pair: &KeyPair) {
        let exposed = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        fs::write(path, format!("{exposed}\n")).expect("write private key fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .expect("secure private key permissions");
        }
    }
    fn reputation_response_fixture(
        status: &str,
        content_type: Option<&str>,
        content_length: Option<u64>,
        content_encoding: Option<&str>,
        extra_headers: &str,
        body: Vec<u8>,
    ) -> (SocketAddr, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind reputation fixture server");
        let address = listener.local_addr().expect("reputation fixture address");
        let status = status.to_owned();
        let content_type = content_type.map(str::to_owned);
        let content_encoding = content_encoding.map(str::to_owned);
        let extra_headers = extra_headers.to_owned();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener
                .accept()
                .expect("accept reputation fixture request");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream
                    .read(&mut chunk)
                    .expect("read reputation fixture request");
                assert!(read > 0, "fixture request ended before its headers");
                request.extend_from_slice(&chunk[..read]);
                assert!(
                    request.len() <= 16 * 1024,
                    "fixture request headers exceeded their test bound"
                );
            }
            let length_header = content_length
                .map(|length| format!("Content-Length: {length}\r\n"))
                .unwrap_or_default();
            let encoding_header = content_encoding
                .map(|encoding| format!("Content-Encoding: {encoding}\r\n"))
                .unwrap_or_default();
            let type_header = content_type
                .map(|content_type| format!("Content-Type: {content_type}\r\n"))
                .unwrap_or_default();
            write!(
                stream,
                "HTTP/1.1 {status}\r\n{type_header}{length_header}{encoding_header}{extra_headers}Connection: close\r\n\r\n"
            )
            .expect("write reputation fixture headers");
            let _ = stream.write_all(&body);
            String::from_utf8_lossy(&request).into_owned()
        });
        (address, handle)
    }
    #[derive(Debug)]
    struct ReputationTestRngError;
    impl std::fmt::Display for ReputationTestRngError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("reputation test RNG failure")
        }
    }
    impl std::error::Error for ReputationTestRngError {}
    struct IncrementingReputationRng {
        next: u8,
    }
    impl rand::rand_core::TryRngCore for IncrementingReputationRng {
        type Error = ReputationTestRngError;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let value = self.next;
            self.next = self.next.wrapping_add(1);
            Ok(u32::from(value))
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let value = self.next;
            self.next = self.next.wrapping_add(1);
            Ok(u64::from(value))
        }
        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            destination.fill(self.next);
            self.next = self.next.wrapping_add(1);
            Ok(())
        }
    }
    impl rand::rand_core::TryCryptoRng for IncrementingReputationRng {}
    struct FailingReputationRng;
    impl rand::rand_core::TryRngCore for FailingReputationRng {
        type Error = ReputationTestRngError;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(ReputationTestRngError)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(ReputationTestRngError)
        }
        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
            Err(ReputationTestRngError)
        }
    }
    impl rand::rand_core::TryCryptoRng for FailingReputationRng {}
    include!("sorafs_cli/reputation_canonical_request_tests.rs");
    #[test]
    fn reputation_provider_id_parser_matches_the_exact_route_contract() {
        let valid = [
            "a".to_owned(),
            "provider-a".to_owned(),
            "provider_a".to_owned(),
            "provider.a".to_owned(),
            "provider:a".to_owned(),
            "provider..a".to_owned(),
            "A0".to_owned(),
            "p".repeat(REPUTATION_PROVIDER_ID_MAX_BYTES),
        ];
        for provider_id in valid {
            assert_eq!(
                parse_reputation_provider_id(&provider_id).expect("canonical provider id"),
                provider_id
            );
        }
        let secret_marker = "provider-private-key-marker";
        let invalid = [
            String::new(),
            ".".to_owned(),
            "..".to_owned(),
            " provider-a".to_owned(),
            "provider-a ".to_owned(),
            "provider/a".to_owned(),
            "provider%2Fa".to_owned(),
            "provider?query".to_owned(),
            "provider#fragment".to_owned(),
            "provider=alias".to_owned(),
            "provideré".to_owned(),
            "p".repeat(REPUTATION_PROVIDER_ID_MAX_BYTES + 1),
            secret_marker.to_owned() + "/",
        ];
        for provider_id in invalid {
            let error = parse_reputation_provider_id(&provider_id)
                .expect_err("noncanonical provider id must fail");
            assert_eq!(
                error,
                "`--provider-id` must be canonical 1..=256 byte ASCII [A-Za-z0-9_.:-] and must not be a dot-segment"
            );
            assert!(!error.contains(secret_marker));
        }
    }
    #[test]
    fn reputation_commands_reject_duplicate_scalar_options_without_values() {
        let secret_first = "provider-private-key-first";
        let secret_second = "provider-private-key-second";
        let errors = [
            reputation_snapshot(vec![
                "--torii-url=http://first.invalid".to_owned(),
                "--torii-url=http://second.invalid".to_owned(),
            ])
            .expect_err("snapshot duplicate must fail"),
            reputation_fetch(vec![
                format!("--provider-id={secret_first}"),
                format!("--provider-id={secret_second}"),
            ])
            .expect_err("fetch duplicate must fail"),
            reputation_watch(vec!["--limit=1".to_owned(), "--limit=2".to_owned()])
                .expect_err("watch duplicate must fail"),
            reputation_verify(vec![
                format!("--provider-id={secret_first}"),
                format!("--provider-id={secret_second}"),
            ])
            .expect_err("verify duplicate must fail"),
        ];
        for error in errors {
            assert!(error.contains("duplicate `--"));
            assert!(!error.contains(secret_first));
            assert!(!error.contains(secret_second));
        }
    }
    #[test]
    fn reputation_response_reader_enforces_identity_and_exact_size_cap() {
        let client = reputation_http_client().expect("hardened reputation client");
        let (address, handle) = reputation_response_fixture(
            "200 OK",
            Some("application/json"),
            Some(REPUTATION_RESPONSE_MAX_BYTES + 1),
            None,
            "",
            Vec::new(),
        );
        let response = client
            .get(format!("http://{address}/declared-oversize"))
            .send()
            .expect("declared-size fixture response");
        let error = read_reputation_response_bounded(response, "reputation test")
            .expect_err("oversized declared response must fail");
        handle.join().expect("declared-size fixture exits");
        assert!(error.contains("declared more than"));
        let streamed_size = usize::try_from(REPUTATION_RESPONSE_MAX_BYTES + 1)
            .expect("reputation response cap fits usize");
        let (address, handle) = reputation_response_fixture(
            "200 OK",
            Some("application/json"),
            None,
            None,
            "",
            vec![b' '; streamed_size],
        );
        let response = client
            .get(format!("http://{address}/streamed-oversize"))
            .send()
            .expect("streamed-size fixture response");
        let error = read_reputation_response_bounded(response, "reputation test")
            .expect_err("oversized streamed response must fail");
        handle.join().expect("streamed-size fixture exits");
        assert!(error.contains("response exceeded"));
        let exact_size = usize::try_from(REPUTATION_RESPONSE_MAX_BYTES)
            .expect("reputation response cap fits usize");
        let mut exact_success = vec![b' '; exact_size];
        exact_success[..2].copy_from_slice(b"{}");
        let (address, handle) = reputation_response_fixture(
            "200 OK",
            Some("application/json"),
            Some(REPUTATION_RESPONSE_MAX_BYTES),
            Some("identity"),
            "",
            exact_success,
        );
        let response = client
            .get(format!("http://{address}/exact-success"))
            .send()
            .expect("exact-size success response");
        let value =
            read_json_response(response, "reputation test").expect("exact cap must be accepted");
        handle.join().expect("exact success fixture exits");
        assert!(value.as_object().is_some());
        let secret_provider = "provider-private-key-error-body";
        let mut exact_error = vec![b'x'; exact_size];
        exact_error[..secret_provider.len()].copy_from_slice(secret_provider.as_bytes());
        let (address, handle) = reputation_response_fixture(
            "500 Internal Server Error",
            None,
            Some(REPUTATION_RESPONSE_MAX_BYTES),
            None,
            "",
            exact_error,
        );
        let response = client
            .get(format!("http://{address}/exact-error"))
            .send()
            .expect("exact-size error response");
        let error = read_json_response(response, "reputation test")
            .expect_err("error status must remain an error");
        handle.join().expect("exact error fixture exits");
        assert!(error.contains("500 Internal Server Error"));
        assert!(!error.contains(secret_provider));
        let (address, handle) = reputation_response_fixture(
            "200 OK",
            Some("application/json"),
            Some(2),
            Some("gzip"),
            "",
            b"{}".to_vec(),
        );
        let response = client
            .get(format!("http://{address}/encoded"))
            .send()
            .expect("encoded fixture response");
        let error = read_json_response(response, "reputation test")
            .expect_err("non-identity response must fail");
        handle.join().expect("encoded fixture exits");
        assert!(error.contains("identity content encoding"));
    }
    #[test]
    fn reputation_response_reader_requires_canonical_http_metadata() {
        let client = reputation_http_client().expect("hardened reputation client");
        let fixtures = [
            (None, Some(2), "", "canonical Content-Type application/json"),
            (
                Some("application/json; charset=utf-8"),
                Some(2),
                "",
                "canonical Content-Type application/json",
            ),
            (
                Some("application/json"),
                None,
                "Content-Length: 02\r\n",
                "canonical unsigned decimal",
            ),
            (
                Some("application/json"),
                Some(2),
                "Content-Length: 2\r\n",
                "duplicate Content-Length",
            ),
        ];
        for (content_type, content_length, extra_headers, expected_error) in fixtures {
            let (address, handle) = reputation_response_fixture(
                "200 OK",
                content_type,
                content_length,
                None,
                extra_headers,
                b"{}".to_vec(),
            );
            let response = client
                .get(format!("http://{address}/malformed-metadata"))
                .send()
                .expect("malformed-metadata fixture response");
            let error = read_json_response(response, "reputation test")
                .expect_err("noncanonical response metadata must fail");
            handle.join().expect("malformed-metadata fixture exits");
            assert!(
                error.contains(expected_error),
                "unexpected error for malformed metadata: {error}"
            );
        }
    }
    #[test]
    fn reputation_endpoint_requires_a_canonical_secure_origin() {
        let secure = reputation_endpoint("https://torii.example/", "v1/sorafs/reputation/latest")
            .expect("canonical HTTPS origin");
        assert_eq!(
            secure.as_str(),
            "https://torii.example/v1/sorafs/reputation/latest"
        );
        let loopback = reputation_endpoint("http://127.0.0.1:8080", "v1/sorafs/reputation/latest")
            .expect("loopback HTTP fixture origin");
        assert_eq!(
            loopback.as_str(),
            "http://127.0.0.1:8080/v1/sorafs/reputation/latest"
        );
        let secret_marker = "runtime-private-marker";
        for invalid in [
            "http://torii.example",
            "https://torii.example/path",
            "https://torii.example/?query=1",
            "https://torii.example/#fragment",
            "https://runtime-private-marker@torii.example",
            " https://torii.example",
            "https://torii.example:0",
            "HTTPS://torii.example",
        ] {
            let error = reputation_endpoint(invalid, "v1/sorafs/reputation/latest")
                .expect_err("noncanonical reputation origin must fail");
            assert!(!error.contains(invalid));
            assert!(!error.contains(secret_marker));
        }
    }
    #[test]
    fn reputation_requests_advertise_identity_encoding_only() {
        let (address, handle) = reputation_response_fixture(
            "200 OK",
            Some("application/json"),
            Some(2),
            None,
            "",
            b"{}".to_vec(),
        );
        let endpoint = Url::parse(&format!("http://{address}/v1/sorafs/reputation/latest"))
            .expect("fixture endpoint");
        let client = reputation_http_client().expect("hardened reputation client");
        let auth = fixture_reputation_auth(0x36, 369);
        let response =
            send_reputation_request(&client, &endpoint, &auth).expect("signed fixture request");
        read_json_response(response, "reputation test").expect("fixture JSON");
        let request = handle.join().expect("identity fixture exits");
        assert!(
            request
                .to_ascii_lowercase()
                .contains("\r\naccept-encoding: identity\r\n")
        );
    }
    #[test]
    fn reputation_account_header_uses_ascii_canonical_address_hex() {
        let account = fixture_account(0x34);
        let literal = account
            .to_i105_for_discriminant(753)
            .expect("Kana-bearing canonical I105");
        assert!(
            !literal.is_ascii(),
            "fixture must exercise I105 Kana bytes: {literal}"
        );
        let parsed = parse_reputation_auth_account(&literal, "test reputation request")
            .expect("parse canonical I105 account");
        let header_value = parsed
            .to_canonical_hex()
            .expect("encode canonical account header");
        assert!(header_value.is_ascii());
        assert!(header_value.starts_with("0x"));
        assert_eq!(parsed, account);
        let client = reputation_http_client().expect("hardened reputation client");
        let request = client
            .get("http://127.0.0.1/v1/sorafs/reputation/latest")
            .header(REPUTATION_HEADER_ACCOUNT, header_value.clone())
            .build()
            .expect("reqwest accepts the canonical ASCII account header");
        let header = request
            .headers()
            .get(REPUTATION_HEADER_ACCOUNT)
            .expect("account header");
        assert_eq!(header.as_bytes(), header_value.as_bytes());
    }
    #[test]
    fn reputation_auth_uses_fresh_nonce_and_signature_for_each_poll() {
        let auth = fixture_reputation_auth(0x32, 369);
        let first_endpoint =
            Url::parse("http://127.0.0.1/v1/sorafs/reputation/events?since=1&limit=10")
                .expect("first endpoint");
        let second_endpoint =
            Url::parse("http://127.0.0.1/v1/sorafs/reputation/events?since=2&limit=10")
                .expect("second endpoint");
        let now = UNIX_EPOCH + Duration::from_millis(1_725_000_000_123);
        let mut rng = IncrementingReputationRng { next: 0x21 };
        let first = reputation_request_headers_with_rng_at(&auth, &first_endpoint, now, &mut rng)
            .expect("first signed poll");
        let second = reputation_request_headers_with_rng_at(&auth, &second_endpoint, now, &mut rng)
            .expect("second signed poll");
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.signature_base64, second.signature_base64);
        assert_eq!(first.account_header_value, auth.account_header_value);
        assert_eq!(second.account_header_value, auth.account_header_value);
    }
    #[test]
    fn reputation_auth_fails_closed_on_rng_and_clock_failures() {
        let auth = fixture_reputation_auth(0x33, 369);
        let endpoint =
            Url::parse("http://127.0.0.1/v1/sorafs/reputation/latest").expect("endpoint");
        let rng_error = reputation_request_headers_with_rng_at(
            &auth,
            &endpoint,
            UNIX_EPOCH,
            &mut FailingReputationRng,
        )
        .err()
        .expect("RNG failure must abort signing");
        assert!(rng_error.contains("OS RNG failed"));
        let clock_error = reputation_request_timestamp_ms_at(
            UNIX_EPOCH
                .checked_sub(Duration::from_millis(1))
                .expect("pre-epoch fixture"),
        )
        .expect_err("pre-epoch time must fail");
        assert!(clock_error.contains("before the Unix epoch"));
    }
    #[test]
    fn reputation_watch_issues_exactly_one_fetch_per_poll() {
        let mut endpoints = Vec::new();
        let mut next_cursor = 8_u64;
        let final_value = run_reputation_watch(
            "http://127.0.0.1:9/",
            Some(7),
            Some(12),
            Some(3),
            0,
            |endpoint| {
                endpoints.push(endpoint.clone());
                let mut response = Map::new();
                response.insert("events".to_owned(), Value::Array(Vec::new()));
                response.insert("next_since".to_owned(), Value::from(next_cursor));
                next_cursor += 1;
                Ok(Value::Object(response))
            },
        )
        .expect("bounded watch");
        assert_eq!(endpoints.len(), 3);
        assert_eq!(endpoints[0].query(), Some("since=7&limit=12"));
        assert_eq!(endpoints[1].query(), Some("since=8&limit=12"));
        assert_eq!(endpoints[2].query(), Some("since=9&limit=12"));
        assert_eq!(
            final_value.get("next_since").and_then(Value::as_u64),
            Some(10)
        );
    }
    #[test]
    fn reputation_http_client_never_follows_redirects() {
        let redirect_listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect listener");
        let redirect_address = redirect_listener.local_addr().expect("redirect address");
        let target_listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
        let target_address = target_listener.local_addr().expect("target address");
        let server = thread::spawn(move || {
            let (mut stream, _) = redirect_listener.accept().expect("accept one request");
            write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .expect("write redirect");
        });
        let client = reputation_http_client().expect("hardened reputation client");
        let response = client
            .get(format!("http://{redirect_address}/initial"))
            .send()
            .expect("receive redirect without following it");
        assert_eq!(response.status(), StatusCode::FOUND);
        server.join().expect("redirect server");
        target_listener
            .set_nonblocking(true)
            .expect("nonblocking target listener");
        let target_error = target_listener
            .accept()
            .expect_err("redirect target must not receive a request");
        assert_eq!(target_error.kind(), io::ErrorKind::WouldBlock);
    }
    #[test]
    fn reputation_live_reads_require_only_the_hard_cut_auth_flags() {
        for error in [
            reputation_snapshot(vec!["--torii-url=http://127.0.0.1:9/".to_owned()])
                .expect_err("snapshot auth is mandatory"),
            reputation_fetch(vec![
                "--torii-url=http://127.0.0.1:9/".to_owned(),
                "--provider-id=provider-a".to_owned(),
            ])
            .expect_err("fetch auth is mandatory"),
            reputation_watch(vec!["--torii-url=http://127.0.0.1:9/".to_owned()])
                .expect_err("watch auth is mandatory"),
        ] {
            assert!(error.contains("missing required `--auth-account=I105`"));
        }
        let account_literal = fixture_account(0x35)
            .to_i105_for_discriminant(369)
            .expect("canonical auth account");
        let error = reputation_snapshot(vec![
            "--torii-url=http://127.0.0.1:9/".to_owned(),
            format!("--auth-account={account_literal}"),
        ])
        .expect_err("private key file is mandatory");
        assert!(error.contains("missing required `--auth-private-key-file=PATH`"));
        let inline_secret = "secret-inline-value";
        let error = reputation_snapshot(vec![
            "--torii-url=http://127.0.0.1:9/".to_owned(),
            format!("--auth-private-key={inline_secret}"),
        ])
        .expect_err("inline authentication secrets are retired");
        assert!(error.contains("unrecognised option `--auth-private-key`"));
        assert!(!error.contains(inline_secret));
        let witness_secret = "secret-witness-value";
        let error = reputation_snapshot(vec![
            "--torii-url=http://127.0.0.1:9/".to_owned(),
            format!("--auth-witness={witness_secret}"),
        ])
        .expect_err("witness compatibility is retired");
        assert!(error.contains("unrecognised option `--auth-witness`"));
        assert!(!error.contains(witness_secret));
        let error = reputation_snapshot(vec![
            "--torii-url=http://127.0.0.1:9/".to_owned(),
            "--auth-account=merchant@paynet".to_owned(),
            "--auth-private-key-file=/does/not/matter".to_owned(),
        ])
        .expect_err("account aliases are retired");
        assert!(error.contains("exact canonical I105 literal"));
    }
    #[test]
    fn reputation_auth_private_key_must_match_the_account() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("reputation.key");
        let account_key = fixture_keypair(0x41);
        let wrong_key = fixture_keypair(0x42);
        write_reputation_private_key(&path, &wrong_key);
        let account_literal = AccountId::new(account_key.public_key().clone())
            .to_i105_for_discriminant(369)
            .expect("account literal");
        let error = load_reputation_request_auth(
            Some(account_literal),
            Some(path),
            Some(fixture_reputation_network_id()),
            "sorafs_cli reputation snapshot",
        )
        .err()
        .expect("mismatched private key must fail");
        assert!(error.contains("does not control"));
    }
    #[test]
    fn reputation_auth_private_key_rejects_malformed_oversize_and_leaking_errors() {
        let directory = tempdir().expect("tempdir");
        let malformed_path = directory.path().join("malformed.key");
        let secret = "secret-material-that-must-not-appear";
        fs::write(&malformed_path, secret).expect("write malformed key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&malformed_path, fs::Permissions::from_mode(0o600))
                .expect("secure malformed fixture");
        }
        let error =
            load_reputation_auth_private_key(&malformed_path, "test").expect_err("malformed key");
        assert!(error.contains("malformed"));
        assert!(!error.contains(secret));
        let utf8_path = directory.path().join("invalid-utf8.key");
        fs::write(&utf8_path, [0xff_u8]).expect("write invalid UTF-8 key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&utf8_path, fs::Permissions::from_mode(0o600))
                .expect("secure UTF-8 fixture");
        }
        let error =
            load_reputation_auth_private_key(&utf8_path, "test").expect_err("invalid UTF-8 key");
        assert!(error.contains("not valid UTF-8"));
        let oversize_path = directory.path().join("oversize.key");
        fs::write(
            &oversize_path,
            vec![b'a'; REPUTATION_AUTH_PRIVATE_KEY_MAX_BYTES as usize + 1],
        )
        .expect("write oversize key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&oversize_path, fs::Permissions::from_mode(0o600))
                .expect("secure oversize fixture");
        }
        let error =
            load_reputation_auth_private_key(&oversize_path, "test").expect_err("oversize key");
        assert!(error.contains("between 1 and"));
    }
    #[cfg(unix)]
    #[test]
    fn reputation_auth_private_key_rejects_symlinks_hardlinks_and_open_permissions() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let directory = tempdir().expect("tempdir");
        let key_pair = fixture_keypair(0x43);
        let target = directory.path().join("target.key");
        write_reputation_private_key(&target, &key_pair);
        let symlink_path = directory.path().join("symlink.key");
        symlink(&target, &symlink_path).expect("create symlink fixture");
        let error =
            load_reputation_auth_private_key(&symlink_path, "test").expect_err("symlink key");
        assert!(error.contains("non-symlink"));
        let hardlink = directory.path().join("hardlink.key");
        fs::hard_link(&target, &hardlink).expect("create hardlink fixture");
        let error = load_reputation_auth_private_key(&target, "test").expect_err("hardlinked key");
        assert!(error.contains("exactly one hard link"));
        fs::remove_file(hardlink).expect("remove hardlink fixture");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640))
            .expect("make key group-readable");
        let error =
            load_reputation_auth_private_key(&target, "test").expect_err("insecure key mode");
        assert!(error.contains("group or world permissions"));
    }
    #[test]
    fn gateway_cli_usage_exposes_only_canonical_denial_audit_inputs() {
        let fetch = fetch_usage();
        assert!(fetch.contains("--expected-cache-version=VERSION"));
        assert!(!fetch.contains("--moderation-key-b64"));
        let moderation = moderation_usage();
        assert!(moderation.contains("--expected-catalog-digest=HEX"));
        for retired in [
            "--expected-cache-version",
            "--moderation-key-b64",
            "--require-proof",
        ] {
            assert!(
                !moderation.contains(retired),
                "retired honey-audit flag remains advertised: {retired}"
            );
        }
    }
    #[test]
    fn honey_audit_rejects_retired_local_denial_proof_flags() {
        for retired in [
            "--expected-cache-version=cache-v1",
            "--moderation-key-b64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "--require-proof",
        ] {
            let error = moderation_honey_audit(vec![retired.to_owned()]).expect_err("retired flag");
            assert_eq!(error, moderation_usage());
        }
    }
    #[test]
    fn honey_audit_expected_catalog_digest_is_exact_lowercase_hex() {
        let valid = "ab".repeat(32);
        let accepted = moderation_honey_audit(vec![format!("--expected-catalog-digest={valid}")])
            .expect_err("manifest is still required");
        assert!(accepted.contains("missing required `--manifest-id`"));
        for invalid in [
            String::new(),
            "01".repeat(31),
            "01".repeat(33),
            valid.to_ascii_uppercase(),
            format!("{}g", &valid[..63]),
            format!(" {valid}"),
        ] {
            let error =
                moderation_honey_audit(vec![format!("--expected-catalog-digest={invalid}")])
                    .expect_err("non-canonical digest");
            assert_eq!(
                error,
                "`--expected-catalog-digest` must be lowercase 32-byte hex"
            );
        }
    }
    fn finalized_pin_for_manifest(manifest: &ManifestV1) -> PinManifestFinalizedRecordV1 {
        let digest = manifest.digest().expect("manifest digest");
        let mut record = PinManifestRecord::new(
            ManifestDigest::new(*digest.as_bytes()),
            ManifestRootCid::try_from_slice(manifest.root_cid.as_slice())
                .expect("canonical manifest CID"),
            chunker_handle_from_profile(&manifest.chunking),
            manifest.chunk_digest_sha3_256,
            manifest.por_root,
            manifest.content_length,
            convert_pin_policy(&manifest.pin_policy),
            fixture_account(0x71),
            4,
            None,
            None,
            Metadata::default(),
        );
        record.status = PinStatus::Approved(5);
        PinManifestFinalizedRecordV1 {
            finalized_cursor: PinManifestFinalizedCursorV1 {
                height: 17,
                block_hash: [0x66; 32],
            },
            manifest: record,
        }
    }
    #[test]
    fn proof_stream_endpoint_policy_requires_one_exact_https_origin() {
        let endpoint = proof_stream_endpoint(Some("https://torii.sora.example"), None)
            .expect("bare HTTPS Torii origin");
        assert_eq!(
            endpoint.as_str(),
            "https://torii.sora.example/v1/sorafs/proof/stream"
        );
        assert_eq!(
            redacted_endpoint(&endpoint),
            "https://torii.sora.example/v1/sorafs/proof/stream"
        );
        let pin = proof_stream_pin_manifest_endpoint(&endpoint, &"ab".repeat(32));
        assert_eq!(
            pin.as_str(),
            format!(
                "https://torii.sora.example/v1/sorafs/pin/{}",
                "ab".repeat(32)
            )
        );
        let direct = proof_stream_endpoint(
            None,
            Some("https://regional.sora.example/v1/sorafs/proof/stream"),
        )
        .expect("exact HTTPS gateway route");
        assert_eq!(
            direct,
            endpoint_with_host(&endpoint, "regional.sora.example")
        );
        for (torii, gateway) in [
            (Some("http://torii.sora.example"), None),
            (Some("https://user@torii.sora.example"), None),
            (Some("https://torii.sora.example?token=secret"), None),
            (Some("https://torii.sora.example#fragment"), None),
            (Some("https://torii.sora.example/prefix"), None),
            (Some("https://torii.sora.example/."), None),
            (None, Some("https://regional.sora.example/v1/proof/stream")),
            (
                None,
                Some("https://regional.sora.example/v1/sorafs/tmp/../proof/stream"),
            ),
        ] {
            assert!(
                proof_stream_endpoint(torii, gateway).is_err(),
                "unsafe endpoint must fail: torii={torii:?} gateway={gateway:?}"
            );
        }
        assert!(
            proof_stream_endpoint(
                Some("https://torii.sora.example"),
                Some("https://regional.sora.example/v1/sorafs/proof/stream")
            )
            .is_err()
        );
    }
    fn endpoint_with_host(endpoint: &Url, host: &str) -> Url {
        let mut expected = endpoint.clone();
        expected.set_host(Some(host)).expect("replace fixture host");
        expected
    }
    #[test]
    fn proof_stream_bearer_token_syntax_is_bounded_and_header_safe() {
        for valid in [
            "opaque-token_1",
            "eyJhbGciOiJFZERTQSJ9.payload.signature",
            "YWJjZA==",
        ] {
            assert!(is_canonical_proof_stream_bearer_token(valid));
        }
        for invalid in [
            "",
            "=",
            " token",
            "token ",
            "to ken",
            "token\nsecret",
            "YW=Jj",
            "token:secret",
        ] {
            assert!(!is_canonical_proof_stream_bearer_token(invalid));
        }
        assert!(!is_canonical_proof_stream_bearer_token(
            &"a".repeat(PROOF_STREAM_BEARER_TOKEN_MAX_BYTES + 1)
        ));
    }
    #[test]
    fn finalized_pin_validation_binds_every_manifest_commitment_and_cursor() {
        let manifest = sample_manifest();
        let digest = manifest.digest().expect("manifest digest");
        let finalized = finalized_pin_for_manifest(&manifest);
        let validated = validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &finalized)
            .expect("matching approved finalized pin");
        assert_eq!(validated.finalized_height, 17);
        assert_eq!(validated.finalized_block_hash, [0x66; 32]);
        assert_eq!(validated.por_root, manifest.por_root);
        let mut pending = finalized.clone();
        pending.manifest.status = PinStatus::Pending;
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &pending)
                .expect_err("pending record must fail")
                .contains("Approved")
        );
        let mut wrong_digest = finalized.clone();
        wrong_digest.manifest.digest = ManifestDigest::new([0xA1; 32]);
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_digest).is_err()
        );
        let mut wrong_cid = finalized.clone();
        wrong_cid.manifest.root_cid =
            ManifestRootCid::from_blake3_digest([0xA2; 32]).expect("alternate canonical CID");
        assert!(validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_cid).is_err());
        let mut wrong_chunker = finalized.clone();
        wrong_chunker.manifest.chunker.semver = "9.9.9".to_string();
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_chunker).is_err()
        );
        let mut wrong_chunk_plan = finalized.clone();
        wrong_chunk_plan.manifest.chunk_digest_sha3_256 = [0xA4; 32];
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_chunk_plan)
                .is_err()
        );
        let mut wrong_root = finalized.clone();
        wrong_root.manifest.por_root = [0xA3; 32];
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_root).is_err()
        );
        let mut wrong_length = finalized.clone();
        wrong_length.manifest.content_length += 1;
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_length).is_err()
        );
        let mut wrong_policy = finalized.clone();
        wrong_policy.manifest.policy.min_replicas += 1;
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &wrong_policy).is_err()
        );
        let mut zero_height = finalized.clone();
        zero_height.finalized_cursor.height = 0;
        assert!(
            validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &zero_height).is_err()
        );
        let mut zero_hash = finalized;
        zero_hash.finalized_cursor.block_hash = [0; 32];
        assert!(validate_finalized_pin_manifest(&manifest, digest.as_bytes(), &zero_hash).is_err());
    }
    #[test]
    fn proof_stream_events_are_request_bound_and_payload_free_after_eof() {
        let payload = (0_u16..512)
            .map(|value| u8::try_from(value % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&payload).expect("ingest PoR fixture");
        let root = *store.por_tree().root();
        let request = ProofStreamRequestV1 {
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            proof_kind: ProofKind::Por,
            challenge_id: None,
            sample_count: Some(1),
            deadline_ms: None,
            sample_seed: Some(7),
            expected_finalized_height: Some(17),
            expected_finalized_block_hash: Some([0x66; 32]),
            nonce: [0x33; 16],
            orchestrator_job_id: None,
            tier: None,
        };
        let context =
            ProofStreamVerificationContext::new(request, Some(root)).expect("verification context");
        let (flat_index, proof) = store
            .sample_leaves(
                1,
                context
                    .por_sample_seed()
                    .expect("request-bound PoR sample seed"),
                &payload,
            )
            .expect("sample PoR fixture")
            .into_iter()
            .next()
            .expect("one sample");
        let mut item = sample_to_map(flat_index, &proof);
        insert_value!(item["request_digest_hex"] = hex_encode(context.request_digest()));
        insert_value!(item["manifest_digest_hex"] = hex_encode(request.manifest_digest));
        insert_value!(item["provider_id_hex"] = hex_encode(request.provider_id));
        insert_value!(
            item["finalized_block_height"] =
                request.expected_finalized_height.expect("finalized height")
        );
        insert_value!(
            item["finalized_block_hash_hex"] = hex_encode(
                request
                    .expected_finalized_block_hash
                    .expect("finalized hash"),
            )
        );
        insert_value!(item["proof_kind"] = "por");
        insert_value!(item["result"] = "success");
        insert_value!(item["latency_ms"] = 40_u64);
        let ndjson = format!(
            "{}\n",
            norito::json::to_string(&Value::Object(item)).expect("encode PoR item")
        );
        let items = ProofStreamNdjsonReader::new(Cursor::new(ndjson.as_bytes()), &context)
            .collect::<Result<Vec<_>, _>>()
            .expect("request-bound stream verifies through EOF");
        assert_eq!(items.len(), 1);
        assert!(items[0].to_json().get("proof").is_some());
        let projection = payload_free_proof_stream_event(&items[0]);
        let object = projection.as_object().expect("event projection object");
        let expected_request_digest = hex_encode(context.request_digest());
        assert_eq!(
            object.get("request_digest_hex").and_then(Value::as_str),
            Some(expected_request_digest.as_str())
        );
        for forbidden in [
            "proof",
            "leaf_bytes_hex",
            "segment_leaves_hex",
            "chunk_segments_hex",
            "chunk_merkle_path_hex",
            "receipt_b64",
            "trace_id",
            "nonce_b64",
            "authorization",
            "credential",
        ] {
            assert!(
                !object.contains_key(forbidden),
                "payload-free event leaked `{forbidden}`"
            );
        }
    }
    include!("sorafs_cli/canonical_argument_tests.rs");
}
fn chunk_digest_sha3_from_specs(specs: &[ChunkFetchSpec]) -> [u8; 32] {
    let mut ordered = specs.to_vec();
    ordered.sort_by_key(|spec| spec.chunk_index);
    let mut hasher = Sha3_256::new();
    for spec in ordered {
        hasher.update(spec.offset.to_le_bytes());
        hasher.update(u64::from(spec.length).to_le_bytes());
        hasher.update(spec.digest);
    }
    hasher.finalize().into()
}
fn chunk_digest_sha3_from_chunks(chunks: &[StoredChunk]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    for chunk in chunks {
        hasher.update(chunk.offset.to_le_bytes());
        hasher.update(u64::from(chunk.length).to_le_bytes());
        hasher.update(chunk.blake3);
    }
    hasher.finalize().into()
}
fn parse_bool_flag(value: &str, flag: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!("{flag} expects a boolean value (true|false)")),
    }
}
fn build_plan_from_specs(
    plan_json: &Value,
    chunker_handle_hint: Option<&str>,
) -> Result<PlanWithHandle, String> {
    let parsed_plan = chunk_fetch_plan_from_json(plan_json)
        .map_err(|err| format!("failed to parse canonical chunk fetch plan: {err}"))?;
    let payload_digest = blake3::Hash::from_bytes(parsed_plan.payload_digest);
    let mut chunk_specs = parsed_plan.chunk_fetch_specs;
    if chunk_specs.is_empty() {
        return Err("chunk fetch plan contained no entries".into());
    }
    chunk_specs.sort_by_key(|spec| spec.chunk_index);
    for (idx, spec) in chunk_specs.iter().enumerate() {
        if spec.chunk_index != idx {
            return Err(format!(
                "chunk fetch specs missing chunk index {} (found {})",
                idx, spec.chunk_index
            ));
        }
    }
    let content_length = chunk_specs
        .iter()
        .map(|spec| spec.offset + u64::from(spec.length))
        .max()
        .ok_or_else(|| "failed to derive content length from chunk fetch specs".to_string())?;
    let (chunk_profile, resolved_handle) = if let Some(handle) = chunker_handle_hint {
        let trimmed = handle.trim();
        let descriptor = chunker_registry::lookup_by_handle(trimmed).ok_or_else(|| {
            format!(
                "unknown chunker handle `{trimmed}`; register the profile or provide a valid handle"
            )
        })?;
        (descriptor.profile, trimmed.to_string())
    } else if let Some(descriptor) = chunker_registry::lookup_by_profile(
        ChunkProfile::DEFAULT,
        chunker_registry::DEFAULT_MULTIHASH_CODE,
    ) {
        (
            descriptor.profile,
            format!(
                "{}.{}@{}",
                descriptor.namespace, descriptor.name, descriptor.semver
            ),
        )
    } else {
        (ChunkProfile::DEFAULT, DEFAULT_CHUNKER_HANDLE.to_string())
    };
    let plan = CarBuildPlan {
        chunk_profile,
        payload_digest,
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
    Ok(PlanWithHandle {
        plan,
        chunker_handle: resolved_handle,
    })
}
struct FetchSummaryOptions<'a> {
    client_id: Option<&'a str>,
    rollout_phase: RolloutPhase,
    write_mode: WriteModeHint,
    cache_profile: Option<FetchCacheProfile>,
}
fn build_fetch_summary(
    manifest_id_hex: &str,
    chunker_handle: &str,
    plan: &CarBuildPlan,
    session: &FetchSession,
    options: FetchSummaryOptions<'_>,
) -> Value {
    let outcome = &session.outcome;
    let policy_report = &session.policy_report;
    let mut root = Map::new();
    insert_value!(root["manifest_id_hex"] = manifest_id_hex);
    insert_value!(root["chunker_handle"] = chunker_handle);
    insert_value!(root["rollout_phase"] = options.rollout_phase.label());
    insert_value!(root["write_mode"] = options.write_mode.label());
    insert_value!(root["write_mode_enforces_pq"] = options.write_mode.enforces_pq_only());
    if let Some(client) = options.client_id {
        insert_value!(root["client_id"] = client);
    }
    if let Some(profile) = options.cache_profile {
        let label = profile.label();
        insert_value!(root["cache_profile"] = label);
        insert_value!(root["cache_state"] = label);
    }
    insert_value!(root["chunk_count"] = plan.chunks.len() as u64);
    insert_value!(root["content_length"] = plan.content_length);
    let assembled_bytes: u64 = outcome.chunks.iter().map(|chunk| chunk.len() as u64).sum();
    insert_value!(root["assembled_bytes"] = assembled_bytes);
    let provider_reports = outcome
        .provider_reports
        .iter()
        .map(|report| {
            let mut obj = Map::new();
            insert_value!(obj["provider"] = report.provider.id().as_str());
            insert_value!(obj["successes"] = report.successes as u64);
            insert_value!(obj["failures"] = report.failures as u64);
            insert_value!(obj["disabled"] = report.disabled);
            Value::Object(obj)
        })
        .collect();
    insert_json!(root["provider_reports"] = Value::Array(provider_reports));
    let receipts = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| {
            let mut obj = Map::new();
            insert_value!(obj["chunk_index"] = receipt.chunk_index as u64);
            insert_value!(obj["provider"] = receipt.provider.as_str());
            insert_value!(obj["attempts"] = receipt.attempts as u64);
            Value::Object(obj)
        })
        .collect();
    insert_json!(root["chunk_receipts"] = Value::Array(receipts));
    if let Some(manifest) = &session.local_proxy_manifest {
        let manifest_json =
            to_value(manifest).expect("local proxy manifest should serialise to JSON");
        insert_json!(root["local_proxy_manifest"] = manifest_json);
    }
    if let Some(stats) = session.taikai_cache_stats {
        insert_json!(root["taikai_cache_summary"] = taikai_cache_stats_to_value(stats));
    }
    if let Some(queue_stats) = session.taikai_cache_queue {
        insert_json!(root["taikai_cache_queue"] = taikai_cache_queue_to_value(queue_stats));
    }
    if let Some(verification) = &session.car_verification {
        insert_value!(
            root["manifest_digest_hex"] = hex_encode(verification.manifest_digest.as_bytes())
        );
        insert_value!(
            root["manifest_payload_digest_hex"] =
                hex_encode(verification.manifest_payload_digest.as_bytes())
        );
        insert_value!(
            root["manifest_car_digest_hex"] = hex_encode(verification.manifest_car_digest)
        );
        insert_value!(root["manifest_content_length"] = verification.manifest_content_length);
        insert_value!(root["manifest_chunk_count"] = verification.manifest_chunk_count);
        insert_value!(
            root["manifest_chunk_profile_handle"] = verification.chunk_profile_handle.clone()
        );
        let governance_signatures: Vec<Value> = verification
            .manifest_governance
            .council_signatures
            .iter()
            .map(|signature| {
                let mut obj = Map::new();
                insert_value!(obj["signer_hex"] = hex_encode(signature.signer));
                insert_value!(obj["signature_hex"] = hex_encode(&signature.signature));
                Value::Object(obj)
            })
            .collect();
        let mut governance_obj = Map::new();
        insert_json!(governance_obj["council_signatures"] = Value::Array(governance_signatures));
        insert_json!(root["manifest_governance"] = Value::Object(governance_obj));
        let mut car_obj = Map::new();
        insert_value!(car_obj["size"] = verification.car_stats.car_size);
        insert_value!(
            car_obj["payload_digest_hex"] =
                hex_encode(verification.car_stats.car_payload_digest.as_bytes(),)
        );
        insert_value!(
            car_obj["archive_digest_hex"] =
                hex_encode(verification.car_stats.car_archive_digest.as_bytes(),)
        );
        insert_value!(car_obj["cid_hex"] = hex_encode(&verification.car_stats.car_cid));
        insert_json!(
            car_obj["root_cids_hex"] = Value::Array(
                verification
                    .car_stats
                    .root_cids
                    .iter()
                    .map(|cid| Value::from(hex_encode(cid)))
                    .collect(),
            )
        );
        insert_value!(car_obj["verified"] = true);
        insert_value!(car_obj["por_leaf_count"] = verification.por_leaf_count as u64);
        insert_json!(root["car_archive"] = Value::Object(car_obj));
    }
    insert_value!(
        root["anonymity_policy"] = anonymity_policy_label(policy_report.policy).to_string()
    );
    insert_value!(root["anonymity_status"] = policy_report.status_label());
    insert_value!(root["anonymity_reason"] = policy_report.reason_label());
    insert_value!(root["anonymity_soranet_selected"] = policy_report.selected_soranet_total as u64);
    insert_value!(root["anonymity_pq_selected"] = policy_report.selected_pq as u64);
    insert_value!(root["anonymity_classical_selected"] = policy_report.selected_classical() as u64);
    insert_value!(root["anonymity_classical_ratio"] = policy_report.classical_ratio());
    insert_value!(root["anonymity_pq_ratio"] = policy_report.pq_ratio());
    insert_value!(root["anonymity_candidate_ratio"] = policy_report.candidate_ratio());
    insert_value!(root["anonymity_deficit_ratio"] = policy_report.deficit_ratio());
    insert_value!(root["anonymity_supply_delta"] = policy_report.supply_delta_ratio());
    insert_value!(root["anonymity_brownout"] = policy_report.is_brownout());
    insert_value!(root["anonymity_brownout_effective"] = policy_report.should_flag_brownout());
    insert_value!(root["anonymity_uses_classical"] = policy_report.uses_classical());
    Value::Object(root)
}
fn taikai_cache_stats_to_value(stats: TaikaiCacheStatsSnapshot) -> Value {
    let mut map = Map::new();
    insert_json!(map["hits"] = tier_counts_value(stats.hits.hot, stats.hits.warm, stats.hits.cold));
    insert_value!(map["misses"] = stats.misses);
    insert_json!(
        map["inserts"] =
            tier_counts_value(stats.inserts.hot, stats.inserts.warm, stats.inserts.cold)
    );
    let mut evictions = Map::new();
    insert_json!(
        evictions["hot"] =
            reason_counts_value(stats.evictions.hot.expired, stats.evictions.hot.capacity)
    );
    insert_json!(
        evictions["warm"] =
            reason_counts_value(stats.evictions.warm.expired, stats.evictions.warm.capacity)
    );
    insert_json!(
        evictions["cold"] =
            reason_counts_value(stats.evictions.cold.expired, stats.evictions.cold.capacity)
    );
    insert_json!(map["evictions"] = Value::Object(evictions));
    insert_json!(
        map["promotions"] = promotion_counts_value(
            stats.promotions.warm_to_hot,
            stats.promotions.cold_to_warm,
            stats.promotions.cold_to_hot,
        )
    );
    insert_json!(
        map["qos_denials"] = qos_counts_value(
            stats.qos_denials.priority,
            stats.qos_denials.standard,
            stats.qos_denials.bulk,
        )
    );
    Value::Object(map)
}
fn taikai_cache_queue_to_value(stats: TaikaiPullQueueStats) -> Value {
    let mut map = Map::new();
    insert_value!(map["pending_segments"] = stats.pending_segments);
    insert_value!(map["pending_bytes"] = stats.pending_bytes);
    insert_value!(map["pending_batches"] = stats.pending_batches);
    insert_value!(map["in_flight_batches"] = stats.in_flight_batches);
    insert_value!(map["hedged_batches"] = stats.hedged_batches);
    insert_json!(
        map["shaper_denials"] = qos_counts_value(
            stats.shaper_denials.priority,
            stats.shaper_denials.standard,
            stats.shaper_denials.bulk,
        )
    );
    insert_value!(map["dropped_segments"] = stats.dropped_segments);
    insert_value!(map["failovers"] = stats.failovers);
    insert_value!(map["open_circuits"] = stats.open_circuits);
    Value::Object(map)
}
fn tier_counts_value(hot: u64, warm: u64, cold: u64) -> Value {
    let mut map = Map::new();
    insert_value!(map["hot"] = hot);
    insert_value!(map["warm"] = warm);
    insert_value!(map["cold"] = cold);
    Value::Object(map)
}
fn reason_counts_value(expired: u64, capacity: u64) -> Value {
    let mut map = Map::new();
    insert_value!(map["expired"] = expired);
    insert_value!(map["capacity"] = capacity);
    Value::Object(map)
}
fn promotion_counts_value(warm_to_hot: u64, cold_to_warm: u64, cold_to_hot: u64) -> Value {
    let mut map = Map::new();
    insert_value!(map["warm_to_hot"] = warm_to_hot);
    insert_value!(map["cold_to_warm"] = cold_to_warm);
    insert_value!(map["cold_to_hot"] = cold_to_hot);
    Value::Object(map)
}
fn qos_counts_value(priority: u64, standard: u64, bulk: u64) -> Value {
    let mut map = Map::new();
    insert_value!(map["priority"] = priority);
    insert_value!(map["standard"] = standard);
    insert_value!(map["bulk"] = bulk);
    Value::Object(map)
}
fn parse_gateway_provider_spec(value: &str) -> Result<GatewayProviderSpec, String> {
    let mut name: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut gateway_public_key: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut stream_token: Option<String> = None;
    let mut privacy_events_url: Option<String> = None;
    for pair in value.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair
            .split_once('=')
            .ok_or_else(|| "--provider expects comma-separated key=value pairs".to_string())?;
        let val = val.trim();
        match key {
            "name" => {
                if val.is_empty() {
                    return Err("--provider name must not be empty".into());
                }
                name = Some(val.to_string());
            }
            "provider-id" | "provider_id" => {
                if val.len() != 64 || !val.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("--provider provider-id must be 32-byte hex".into());
                }
                provider_id = Some(val.to_ascii_lowercase());
            }
            "gateway-key" | "gateway_key" | "gateway-public-key" | "gateway_public_key" => {
                if val.len() != 64 || !val.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("--provider gateway-key must be 32-byte hex".into());
                }
                gateway_public_key = Some(val.to_ascii_lowercase());
            }
            "base-url" | "base_url" => {
                if val.is_empty() {
                    return Err("--provider base-url must not be empty".into());
                }
                base_url = Some(val.to_string());
            }
            "stream-token" | "stream_token" => {
                if val.is_empty() {
                    return Err("--provider stream-token must not be empty".into());
                }
                stream_token = Some(val.to_string());
            }
            "privacy-url" | "privacy_url" => {
                if val.is_empty() {
                    return Err("--provider privacy-url must not be empty".into());
                }
                privacy_events_url = Some(val.to_string());
            }
            other => return Err(format!("unknown key `{other}` in --provider argument")),
        }
    }
    let name = name.ok_or_else(|| "--provider requires a `name=` entry".to_string())?;
    let provider_id_hex =
        provider_id.ok_or_else(|| "--provider requires a `provider-id=` entry".to_string())?;
    let gateway_public_key_hex = gateway_public_key
        .ok_or_else(|| "--provider requires a `gateway-key=` entry".to_string())?;
    let base_url = base_url.ok_or_else(|| "--provider requires a `base-url=` entry".to_string())?;
    let stream_token_b64 =
        stream_token.ok_or_else(|| "--provider requires a `stream-token=` entry".to_string())?;
    Ok(GatewayProviderSpec {
        name,
        provider_id_hex,
        gateway_public_key_hex,
        base_url,
        stream_token_b64,
        privacy_events_url,
    })
}
fn parse_usize(raw: &str, flag: &str) -> Result<usize, String> {
    require_canonical_unsigned_decimal(flag, raw, "sorafs_cli")?;
    raw.parse::<usize>()
        .map_err(|err| format!("invalid {flag} value `{raw}`: {err}"))
}
fn parse_taikai_cache_override(value: Value) -> Result<Option<TaikaiCacheConfig>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let inner = match value {
        Value::Object(mut map) => {
            if let Some(embedded) = map.remove("taikai_cache") {
                embedded
            } else {
                Value::Object(map)
            }
        }
        other => other,
    };
    let mut wrapper = Map::new();
    insert_json!(wrapper["taikai_cache"] = inner);
    let parsed = orchestrator_config_from_json(&Value::Object(wrapper))
        .map_err(|err| format!("failed to parse Taikai cache config: {err}"))?;
    Ok(parsed.taikai_cache)
}
fn parse_storage_class(value: &str) -> Result<StorageClass, String> {
    match value.to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        _ => Err(format!(
            "invalid storage class `{value}`; expected hot|warm|cold"
        )),
    }
}
fn parse_iso_week_arg(raw: &str) -> Result<PorReportIsoWeek, String> {
    let trimmed = raw.trim();
    let (year_part, week_part) = if let Some((year, week)) = trimmed.split_once("-W") {
        (year, week)
    } else if let Some((year, week)) = trimmed.split_once('W') {
        (year, week)
    } else if let Some((year, week)) = trimmed.split_once('-') {
        (year, week)
    } else {
        return Err(format!(
            "invalid ISO week `{trimmed}`; expected format YYYY-Www"
        ));
    };
    let year = year_part.trim().parse::<u16>().map_err(|err| {
        format!("invalid ISO week year `{year_part}` supplied to `--week`: {err}")
    })?;
    let week = week_part.trim().parse::<u8>().map_err(|err| {
        format!("invalid ISO week number `{week_part}` supplied to `--week`: {err}")
    })?;
    let week_id = PorReportIsoWeek { year, week };
    week_id
        .validate()
        .map_err(|err| format!("invalid ISO week `{trimmed}`: {err}"))?;
    Ok(week_id)
}
fn render_status_table(entries: &[PorChallengeStatusV1]) -> String {
    if entries.is_empty() {
        return "No PoR challenges found.".to_string();
    }
    let mut out = String::new();
    let _ = writeln!(
        &mut out,
        "{:<12} {:<12} {:<15} {:>8} {:>6} {:>12} {:>12} FAILURE",
        "CHALLENGE", "PROVIDER", "STATUS", "SAMPLES", "FORCED", "ISSUED", "RESPONDED"
    );
    for entry in entries {
        let challenge = hex_prefix(&entry.challenge_id, 12);
        let provider = hex_prefix(&entry.provider_id, 12);
        let status = entry.status.as_str();
        let samples = entry.sample_count;
        let forced = bool_label(entry.forced);
        let issued = entry.issued_at.to_string();
        let responded = entry
            .responded_at
            .map(|ts| ts.to_string())
            .unwrap_or_else(|| "-".to_string());
        let failure = entry
            .failure_reason
            .as_deref()
            .map(|reason| truncate_with_ellipsis(reason, 40))
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            &mut out,
            "{challenge:<12} {provider:<12} {status:<15} {samples:>8} {forced:>6} \
             {issued:>12} {responded:>12} {failure}"
        );
    }
    out
}
fn render_report_markdown(report: &PorWeeklyReportV1) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "# PoR Weekly Health — {}", report.cycle);
    let _ = writeln!(&mut out, "\nGenerated (unix): {}", report.generated_at);
    let _ = writeln!(&mut out, "\n## Aggregate Metrics");
    let _ = writeln!(&mut out, "- Total challenges: {}", report.challenges_total);
    let _ = writeln!(&mut out, "- Verified: {}", report.challenges_verified);
    let _ = writeln!(&mut out, "- Failed: {}", report.challenges_failed);
    let _ = writeln!(
        &mut out,
        "- Forced challenges: {}",
        report.forced_challenges
    );
    let _ = writeln!(&mut out, "- Repairs enqueued: {}", report.repairs_enqueued);
    let _ = writeln!(
        &mut out,
        "- Repairs completed: {}",
        report.repairs_completed
    );
    if let Some(mean) = report.mean_latency_ms {
        let _ = writeln!(&mut out, "- Mean latency: {mean} ms");
    }
    if let Some(p95) = report.p95_latency_ms {
        let _ = writeln!(&mut out, "- P95 latency: {p95} ms");
    }
    if !report.top_offenders.is_empty() {
        let _ = writeln!(&mut out, "\n## Provider Summaries");
        let _ = writeln!(
            &mut out,
            "| Provider | Challenges | Successes | Failures | Forced | Success Rate | Pending Repairs | Repair Dispatched | Ticket | First Failure (unix) | p95 latency ms |"
        );
        let _ = writeln!(
            &mut out,
            "|----------|------------|-----------|---------|--------|--------------|-----------------|-------------------|--------|----------------------|----------------|"
        );
        for provider in &report.top_offenders {
            let provider_id = hex_prefix(&provider.provider_id, 12);
            let success_rate_whole = provider.success_rate_bps / 100;
            let success_rate_fractional = provider.success_rate_bps % 100;
            let ticket = provider.ticket_id.as_deref().unwrap_or("-");
            let first_failure = provider
                .first_failure_at
                .map(|ts| ts.to_string())
                .unwrap_or_else(|| "-".to_string());
            let latency = provider
                .last_success_latency_ms_p95
                .map(|ms| ms.to_string())
                .unwrap_or_else(|| "-".to_string());
            let _ = writeln!(
                &mut out,
                "| {} | {} | {} | {} | {} | {}.{:02}% | {} | {} | {} | {} | {} |",
                provider_id,
                provider.challenges,
                provider.successes,
                provider.failures,
                provider.forced,
                success_rate_whole,
                success_rate_fractional,
                provider.pending_repairs,
                bool_label(provider.repair_dispatched),
                ticket,
                first_failure,
                latency
            );
        }
    }
    if !report.providers_missing_vrf.is_empty() {
        let _ = writeln!(&mut out, "\n## Providers Missing VRF");
        for provider in &report.providers_missing_vrf {
            let _ = writeln!(&mut out, "- {}", hex_prefix(provider, 12));
        }
    }
    if !report.slashing_events.is_empty() {
        let _ = writeln!(&mut out, "\n## Slashing Events");
        for event in &report.slashing_events {
            let provider = hex_prefix(&event.provider_id, 12);
            let manifest = hex_prefix(&event.manifest_digest, 12);
            let _ = writeln!(
                &mut out,
                "- Provider {} manifest {} penalty {} XOR (verdict `{}`, decided unix {})",
                provider, manifest, event.penalty_xor, event.verdict_cid, event.decided_at
            );
        }
    }
    if let Some(notes) = report.notes.as_deref() {
        let _ = writeln!(&mut out, "\n## Notes\n{}", notes.trim());
    }
    out
}
fn truncate_with_ellipsis(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &value[..max_len - 3])
    }
}
fn bool_label(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}
fn hex_prefix(bytes: &[u8], len: usize) -> String {
    let full = hex_encode(bytes);
    let end = len.min(full.len());
    full[..end].to_string()
}
fn body_snippet(body: &[u8]) -> String {
    if body.is_empty() {
        return "empty response body".to_string();
    }
    if let Ok(text) = std::str::from_utf8(body) {
        let trimmed = text.trim();
        truncate_with_ellipsis(trimmed, 120)
    } else {
        format!("{} bytes (binary payload)", body.len())
    }
}
fn parse_digest_hex(input: &str) -> Result<[u8; 32], String> {
    parse_fixed_hex_bytes::<32>(input, "digest")
}
fn parse_fixed_hex_bytes<const N: usize>(input: &str, field: &str) -> Result<[u8; N], String> {
    let bytes = parse_hex_vec(input)?;
    if bytes.len() != N {
        return Err(format!(
            "{field} must encode exactly {N} bytes, found {} bytes",
            bytes.len()
        ));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    if input != hex_encode(out) {
        return Err(format!(
            "{field} must use exact lowercase hexadecimal without surrounding whitespace"
        ));
    }
    Ok(out)
}
fn parse_hex_vec(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) {
        return Err("hex string must contain an even number of characters".into());
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let mut iter = input.as_bytes().chunks_exact(2);
    for pair in &mut iter {
        let hi = hex_value(pair[0])?;
        let lo = hex_value(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}
fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit `{}`", byte as char)),
    }
}
fn format_manifest_error(err: ManifestBuildError) -> String {
    match err {
        ManifestBuildError::MissingField(field) => {
            format!("manifest missing required field `{field}`")
        }
    }
}
fn pin_policy_json(policy: &PinPolicy) -> Map {
    let mut obj = Map::new();
    let label = match policy.storage_class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    };
    insert_value!(obj["min_replicas"] = policy.min_replicas as u64);
    insert_value!(obj["storage_class"] = label);
    insert_value!(obj["retention_epoch"] = policy.retention_epoch);
    obj
}
enum JsonSource {
    Stdin,
    File(PathBuf),
}
impl JsonSource {
    fn from_arg(arg: &str) -> Result<Self, String> {
        if arg == "-" {
            Ok(Self::Stdin)
        } else {
            Ok(Self::File(PathBuf::from(arg)))
        }
    }
    fn read(self) -> Result<Value, String> {
        match self {
            JsonSource::Stdin => {
                let mut buf = String::new();
                io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|err| format!("failed to read summary from stdin: {err}"))?;
                norito::json::from_str(&buf)
                    .map_err(|err| format!("failed to parse summary JSON from stdin: {err}"))
            }
            JsonSource::File(path) => {
                let file = File::open(&path)
                    .map_err(|err| format!("failed to open `{}`: {err}", path.display()))?;
                let mut reader = BufReader::new(file);
                let mut buf = String::new();
                reader
                    .read_to_string(&mut buf)
                    .map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;
                norito::json::from_str(&buf)
                    .map_err(|err| format!("failed to parse JSON from `{}`: {err}", path.display()))
            }
        }
    }
}

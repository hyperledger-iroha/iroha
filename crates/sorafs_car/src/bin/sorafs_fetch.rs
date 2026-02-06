//! Prototype CLI for exercising the multi-source chunk fetch orchestrator.
#![allow(unexpected_cfgs)]
//!
//! The tool accepts a chunk fetch plan (as emitted by `sorafs-manifest-stub
//! --chunk-fetch-plan-out`) and a list of local provider payloads. Each provider
//! exposes the original payload as a single file; the orchestrator reads the
//! required byte ranges for every chunk, verifies BLAKE3 digests and lengths,
//! and assembles the payload once all chunks have been downloaded. When
//! `--gateway-provider` entries are supplied the fetcher instead issues HTTP
//! chunk requests against Torii gateways using authenticated stream tokens.
//! When `--output` is supplied the CLI streams verified chunks directly to disk
//! while downloads are still in flight.
//!
//! This lets developers validate the scheduling and verification logic of the
//! orchestrator without standing up real storage nodes.
//!
//! JSON outputs (`--json-out`, `--provider-metrics-out`) now surface the SF-2d
//! advert metadata captured during discovery. Each provider record includes
//! `metadata.range_capability`, `metadata.stream_budget`, and
//! `metadata.transport_hints` when the source advert supplied them so dashboards
//! and fixtures can reason about the same scheduling inputs as the orchestrator.

use blake3::Hasher;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Verifier, VerifyingKey,
};
use hex::encode as hex_encode;
use iroha_crypto::HybridSuite;
use norito::{
    decode_from_bytes,
    json::{Map, Value, from_slice, to_string_pretty},
};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarStreamingWriter, CarVerificationReport, CarVerifier, CarWriteStats,
    ChunkFetchSpec, FilePlan, chunker_registry,
    fetch_plan::{
        chunk_fetch_specs_from_json, expected_payload_digest_from_json,
        expected_payload_len_from_json, parse_digest_hex,
    },
    gateway::{GatewayFetchConfig, GatewayFetchContext, GatewayFetchError, GatewayProviderInput},
    multi_fetch,
    multi_fetch::{
        ChunkDelivery, ChunkObserver, ChunkResponse, FetchOutcome, FetchRequest, MultiSourceError,
        ObserverError, ProviderMetadata, ProviderReport, ProviderScoreContext,
        ProviderScoreDecision, RangeCapability, ScorePolicy, StreamBudget, TransportHint,
    },
    policy::{
        AnonymityPolicy, PolicyLabelSummary, TransportPolicy, anonymity_policy_labels,
        transport_policy_labels,
    },
    scoreboard::{self, ProviderTelemetry, TelemetrySnapshot},
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    AvailabilityTier, CapabilityType, ManifestV1, ProviderAdvertV1, ProviderCapabilityRangeV1,
    SignatureAlgorithm, TransportHintV1, TransportProtocol,
    hybrid_envelope::{HYBRID_PAYLOAD_ENVELOPE_VERSION_V1, HybridPayloadEnvelopeV1},
    provider_admission::{
        AdmissionRecord, ProviderAdmissionEnvelopeV1, verify_advert_against_record,
    },
    provider_advert::ProviderCapabilitySoranetPqV1,
};

const KNOWN_CAPABILITIES: &[CapabilityType; 4] = &[
    CapabilityType::ToriiGateway,
    CapabilityType::QuicNoise,
    CapabilityType::ChunkRangeFetch,
    CapabilityType::SoraNetHybridPq,
];
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    process,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

use base64::Engine as _;

#[derive(Clone)]
enum JsonSource {
    File(PathBuf),
    Stdin,
}

#[derive(Clone)]
enum BinarySource {
    File(PathBuf),
    Stdin,
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Err(usage().to_string());
    }

    let mut plan_source: Option<JsonSource> = None;
    let mut manifest_source: Option<JsonSource> = None;
    let mut manifest_bytes_source: Option<BinarySource> = None;
    let mut telemetry_source: Option<JsonSource> = None;
    let mut telemetry_source_label: Option<String> = None;
    let mut telemetry_region: Option<String> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut car_out: Option<PathBuf> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut provider_metrics_out: Option<PathBuf> = None;
    let mut chunk_receipts_out: Option<PathBuf> = None;
    let mut scoreboard_out: Option<PathBuf> = None;
    let mut provider_specs: Vec<ProviderSpec> = Vec::new();
    let mut gateway_specs: Vec<GatewayProviderSpec> = Vec::new();
    let mut provider_advert_paths: HashMap<String, PathBuf> = HashMap::new();
    let mut admission_dir: Option<PathBuf> = None;
    let mut assume_now: Option<u64> = None;
    let mut max_parallel: Option<usize> = None;
    let mut max_peers: Option<usize> = None;
    let mut retry_budget: Option<usize> = None;
    let mut failure_threshold: Option<usize> = None;
    let mut deny_providers: HashSet<String> = HashSet::new();
    let mut boost_providers: HashMap<String, i64> = HashMap::new();
    let mut use_scoreboard = true;
    let mut expect_payload_digest: Option<[u8; 32]> = None;
    let mut expect_payload_len: Option<u64> = None;
    let mut skip_verify_digest = false;
    let mut skip_verify_length = false;
    let mut allow_insecure = false;
    let mut allow_implicit_metadata = false;
    let mut gateway_manifest_id: Option<String> = None;
    let mut gateway_manifest_envelope: Option<String> = None;
    let mut gateway_client_id: Option<String> = None;
    let mut gateway_chunker_handle: Option<String> = None;
    let mut gateway_manifest_cid: Option<String> = None;
    let mut scoreboard_gateway_manifest_id: Option<String> = None;
    let mut scoreboard_gateway_manifest_cid: Option<String> = None;
    let mut transport_policy: Option<TransportPolicy> = None;
    let mut transport_policy_override: Option<TransportPolicy> = None;
    let mut anonymity_policy: Option<AnonymityPolicy> = None;
    let mut anonymity_policy_override: Option<AnonymityPolicy> = None;

    for arg in &args {
        if arg == "--help" {
            return Err(usage().to_string());
        }
        if let Some(rest) = arg.strip_prefix("--plan=") {
            if rest == "-" {
                if matches!(manifest_source, Some(JsonSource::Stdin))
                    || matches!(telemetry_source, Some(JsonSource::Stdin))
                    || matches!(manifest_bytes_source, Some(BinarySource::Stdin))
                {
                    return Err(
                        "stdin already reserved for another input; cannot use --plan=-".into(),
                    );
                }
                plan_source = Some(JsonSource::Stdin);
            } else {
                plan_source = Some(JsonSource::File(PathBuf::from(rest)));
            }
        } else if let Some(rest) = arg.strip_prefix("--provider=") {
            provider_specs.push(parse_provider_spec(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--gateway-provider=") {
            gateway_specs.push(parse_gateway_provider_spec(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--provider-advert=") {
            let (name, path) = rest
                .split_once('=')
                .ok_or_else(|| "--provider-advert expects name=path".to_string())?;
            if provider_advert_paths
                .insert(name.to_string(), PathBuf::from(path))
                .is_some()
            {
                return Err(format!(
                    "duplicate --provider-advert provided for provider '{name}'"
                ));
            }
        } else if let Some(rest) = arg.strip_prefix("--output=") {
            output_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--car-out=") {
            car_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--provider-metrics-out=") {
            provider_metrics_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--chunk-receipts-out=") {
            chunk_receipts_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--telemetry-json=") {
            if telemetry_source.is_some() {
                return Err("cannot specify --telemetry-json multiple times".into());
            }
            let source = if rest == "-" {
                if matches!(plan_source, Some(JsonSource::Stdin))
                    || matches!(manifest_source, Some(JsonSource::Stdin))
                    || matches!(manifest_bytes_source, Some(BinarySource::Stdin))
                {
                    return Err(
                        "stdin already reserved for another input; cannot use --telemetry-json=-"
                            .into(),
                    );
                }
                JsonSource::Stdin
            } else {
                JsonSource::File(PathBuf::from(rest))
            };
            telemetry_source = Some(source);
            use_scoreboard = true;
        } else if let Some(rest) = arg.strip_prefix("--telemetry-source-label=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--telemetry-source-label` must not be empty".into());
            }
            telemetry_source_label = Some(trimmed.to_string());
        } else if let Some(rest) = arg.strip_prefix("--telemetry-region=") {
            let trimmed = rest.trim();
            if trimmed.is_empty() {
                return Err("`--telemetry-region` must not be empty".into());
            }
            telemetry_region = Some(trimmed.to_string());
            use_scoreboard = true;
        } else if arg == "--use-scoreboard" {
            use_scoreboard = true;
        } else if let Some(rest) = arg.strip_prefix("--scoreboard-out=") {
            scoreboard_out = Some(PathBuf::from(rest));
            use_scoreboard = true;
        } else if let Some(rest) = arg.strip_prefix("--deny-provider=") {
            if !deny_providers.insert(rest.to_string()) {
                return Err(format!("duplicate --deny-provider for provider '{rest}'"));
            }
        } else if let Some(rest) = arg.strip_prefix("--boost-provider=") {
            let (name, delta_str) = rest
                .split_once(':')
                .ok_or_else(|| "--boost-provider expects name:delta".to_string())?;
            let delta: i64 = delta_str
                .parse()
                .map_err(|err| format!("invalid boost delta '{delta_str}': {err}"))?;
            boost_providers.insert(name.to_string(), delta);
        } else if let Some(rest) = arg.strip_prefix("--max-parallel=") {
            max_parallel = Some(parse_usize(rest, "--max-parallel")?);
        } else if let Some(rest) = arg.strip_prefix("--max-peers=") {
            max_peers = Some(parse_usize(rest, "--max-peers")?);
        } else if let Some(rest) = arg.strip_prefix("--retry-limit=") {
            let value = parse_usize(rest, "--retry-limit")?;
            match retry_budget {
                Some(existing) if existing != value => {
                    return Err(
                        "cannot provide both --retry-budget and --retry-limit with different values"
                            .to_string(),
                    );
                }
                Some(_) => {}
                None => retry_budget = Some(value),
            }
        } else if let Some(rest) = arg.strip_prefix("--retry-budget=") {
            let value = parse_usize(rest, "--retry-budget")?;
            match retry_budget {
                Some(existing) if existing != value => {
                    return Err(
                        "cannot provide multiple retry budget values in the same invocation"
                            .to_string(),
                    );
                }
                _ => retry_budget = Some(value),
            }
        } else if let Some(rest) = arg.strip_prefix("--provider-failure-threshold=") {
            failure_threshold = Some(parse_usize(rest, "--provider-failure-threshold")?);
        } else if let Some(rest) = arg.strip_prefix("--admission-dir=") {
            admission_dir = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--manifest-report=") {
            if rest == "-" {
                if matches!(plan_source, Some(JsonSource::Stdin))
                    || matches!(telemetry_source, Some(JsonSource::Stdin))
                    || matches!(manifest_bytes_source, Some(BinarySource::Stdin))
                {
                    return Err(
                        "stdin already reserved for another input; cannot use --manifest-report=-"
                            .into(),
                    );
                }
                manifest_source = Some(JsonSource::Stdin);
            } else {
                manifest_source = Some(JsonSource::File(PathBuf::from(rest)));
            }
        } else if let Some(rest) = arg.strip_prefix("--manifest=") {
            if manifest_bytes_source.is_some() {
                return Err("duplicate --manifest option supplied".into());
            }
            if rest == "-" {
                if matches!(plan_source, Some(JsonSource::Stdin))
                    || matches!(manifest_source, Some(JsonSource::Stdin))
                    || matches!(telemetry_source, Some(JsonSource::Stdin))
                {
                    return Err(
                        "cannot use --manifest=- alongside --plan=- or --manifest-report=-".into(),
                    );
                }
                manifest_bytes_source = Some(BinarySource::Stdin);
            } else {
                manifest_bytes_source = Some(BinarySource::File(PathBuf::from(rest)));
            }
        } else if arg == "--no-verify-digest" {
            skip_verify_digest = true;
        } else if arg == "--no-verify-length" {
            skip_verify_length = true;
        } else if arg == "--allow-insecure" {
            allow_insecure = true;
        } else if arg == "--allow-implicit-provider-metadata" {
            allow_implicit_metadata = true;
        } else if let Some(rest) = arg.strip_prefix("--expect-payload-digest=") {
            expect_payload_digest = Some(parse_digest_hex(rest).map_err(|err| err.to_string())?);
        } else if let Some(rest) = arg.strip_prefix("--expect-payload-len=") {
            expect_payload_len = Some(
                parse_u64_value(rest)
                    .map_err(|err| format!("invalid --expect-payload-len value: {err}"))?,
            );
        } else if let Some(rest) = arg.strip_prefix("--assume-now=") {
            assume_now = Some(
                parse_u64_value(rest)
                    .map_err(|err| format!("invalid --assume-now value: {err}"))?,
            );
        } else if let Some(rest) = arg.strip_prefix("--gateway-manifest-id=") {
            gateway_manifest_id = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--gateway-manifest-envelope=") {
            gateway_manifest_envelope = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--gateway-client-id=") {
            gateway_client_id = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--gateway-chunker-handle=") {
            gateway_chunker_handle = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--gateway-manifest-cid=") {
            gateway_manifest_cid = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--transport-policy=") {
            let value = rest.trim();
            if value.is_empty() {
                return Err("`--transport-policy` must not be empty".into());
            }
            let parsed = TransportPolicy::parse(value).ok_or_else(|| {
                "`--transport-policy` must be one of soranet-first|soranet-strict|direct-only"
                    .to_string()
            })?;
            transport_policy = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--transport-policy-override=") {
            let value = rest.trim();
            if value.is_empty() {
                return Err("`--transport-policy-override` must not be empty".into());
            }
            let parsed = TransportPolicy::parse(value).ok_or_else(|| {
                "`--transport-policy-override` must be one of soranet-first|soranet-strict|direct-only"
                    .to_string()
            })?;
            transport_policy_override = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--anonymity-policy=") {
            let value = rest.trim();
            if value.is_empty() {
                return Err("`--anonymity-policy` must not be empty".into());
            }
            let parsed = AnonymityPolicy::parse(value).ok_or_else(|| {
                "`--anonymity-policy` must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq"
                    .to_string()
            })?;
            anonymity_policy = Some(parsed);
        } else if let Some(rest) = arg.strip_prefix("--anonymity-policy-override=") {
            let value = rest.trim();
            if value.is_empty() {
                return Err("`--anonymity-policy-override` must not be empty".into());
            }
            let parsed = AnonymityPolicy::parse(value).ok_or_else(|| {
                "`--anonymity-policy-override` must be one of anon-guard-pq|anon-majority-pq|anon-strict-pq"
                    .to_string()
            })?;
            anonymity_policy_override = Some(parsed);
        } else if arg.starts_with("--") {
            return Err(format!("unknown option: {arg}"));
        }
    }

    if (skip_verify_digest || skip_verify_length) && !allow_insecure {
        return Err(
            "refusing to disable integrity verification without --allow-insecure".to_string(),
        );
    }

    if provider_specs.is_empty() && gateway_specs.is_empty() {
        return Err(
            "specify at least one --provider=name=/path/to/payload[#concurrency] or --gateway-provider=name=...,provider-id=...,base-url=...,stream-token=..."
                .into(),
        );
    }

    let manifest_report: Option<Value> = if let Some(source) = &manifest_source {
        Some(load_json(source)?)
    } else {
        None
    };

    let plan_json = if let Some(source) = plan_source {
        load_json(&source)?
    } else if let Some(report) = manifest_report.clone() {
        report
    } else {
        return Err(
            "provide either --plan=<chunk_fetch_specs.json> or --manifest-report=<report.json>"
                .to_string(),
        );
    };

    if expect_payload_digest.is_none() {
        expect_payload_digest = expected_payload_digest_from_json(&plan_json).or_else(|| {
            manifest_report
                .as_ref()
                .and_then(expected_payload_digest_from_json)
        });
    }

    if expect_payload_len.is_none() {
        expect_payload_len = expected_payload_len_from_json(&plan_json).or_else(|| {
            manifest_report
                .as_ref()
                .and_then(expected_payload_len_from_json)
        });
    }

    let mut chunk_specs = chunk_fetch_specs_from_json(&plan_json)
        .map_err(|err| format!("failed to parse chunk fetch specs: {err}"))?;
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

    let file_entry = FilePlan {
        path: Vec::new(),
        first_chunk: 0,
        chunk_count: chunk_specs.len(),
        size: content_length,
    };

    let manifest = if let Some(source) = &manifest_bytes_source {
        Some(load_manifest_from_source(source)?)
    } else if let Some(report) = manifest_report.as_ref() {
        manifest_from_report(report)?
    } else {
        None
    };

    if let Some(manifest_ref) = manifest.as_ref() {
        if manifest_ref.content_length != content_length {
            return Err(format!(
                "manifest content length {} does not match chunk fetch plan length {}",
                manifest_ref.content_length, content_length
            ));
        }
        if expect_payload_len.is_none() {
            expect_payload_len = Some(manifest_ref.content_length);
        }
    }

    let chunk_profile = manifest
        .as_ref()
        .and_then(|manifest| {
            chunker_registry::lookup(manifest.chunking.profile_id.into())
                .map(|descriptor| descriptor.profile)
        })
        .unwrap_or(ChunkProfile::DEFAULT);

    let plan = CarBuildPlan {
        chunk_profile,
        payload_digest: blake3::hash(&[]),
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
        files: vec![file_entry],
    };

    let plan_profile_handle = chunker_registry::lookup_by_profile(
        plan.chunk_profile,
        chunker_registry::DEFAULT_MULTIHASH_CODE,
    )
    .map(|descriptor| {
        format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )
    });

    let mut gateway_fetcher_opt = None;
    let mut gateway_fetch_providers = Vec::new();
    if !gateway_specs.is_empty() {
        let manifest_ref = manifest.as_ref().ok_or_else(|| {
            "--gateway-provider requires --manifest or --manifest-report with manifest bytes"
                .to_string()
        })?;
        let manifest_digest = manifest_ref
            .digest()
            .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
        let computed_manifest_cid_hex = hex::encode(manifest_digest.as_bytes());
        let manifest_id_hex = gateway_manifest_id.clone().ok_or_else(|| {
            "--gateway-manifest-id is required when specifying --gateway-provider".to_string()
        })?;
        let chunker_handle = gateway_chunker_handle
            .clone()
            .or_else(|| plan_profile_handle.clone())
            .ok_or_else(|| {
                "could not determine chunker handle; provide --gateway-chunker-handle".to_string()
            })?;
        if let Some(cid) = gateway_manifest_cid.as_ref()
            && (cid.len() != 64 || !cid.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("--gateway-manifest-cid must be 32-byte hex".into());
        }
        let expected_manifest_cid_hex = gateway_manifest_cid
            .clone()
            .unwrap_or(computed_manifest_cid_hex);
        scoreboard_gateway_manifest_id = Some(manifest_id_hex.clone());
        scoreboard_gateway_manifest_cid = Some(expected_manifest_cid_hex.clone());
        let config = GatewayFetchConfig {
            manifest_id_hex,
            chunker_handle,
            manifest_envelope_b64: gateway_manifest_envelope.clone(),
            client_id: gateway_client_id.clone(),
            expected_manifest_cid_hex: Some(expected_manifest_cid_hex),
            blinded_cid_b64: None,
            salt_epoch: None,
            expected_cache_version: None,
            moderation_token_key_b64: None,
        };
        let inputs: Vec<GatewayProviderInput> = gateway_specs
            .iter()
            .map(|spec| GatewayProviderInput {
                name: spec.name.clone(),
                provider_id_hex: spec.provider_id_hex.clone(),
                base_url: spec.base_url.clone(),
                stream_token_b64: spec.stream_token_b64.clone(),
                privacy_events_url: spec.privacy_events_url.clone(),
            })
            .collect();
        let context = GatewayFetchContext::new(config, inputs).map_err(|err| format!("{err}"))?;
        gateway_fetch_providers = context.providers();
        gateway_fetcher_opt = Some(context.fetcher());
    }

    let admission_registry = if let Some(dir) = admission_dir.as_ref() {
        Some(load_admission_registry(dir)?)
    } else {
        None
    };

    let mut advert_data: HashMap<String, AdvertMetadata> = HashMap::new();
    for (name, path) in provider_advert_paths {
        if !provider_specs.iter().any(|spec| spec.name == name) {
            return Err(format!(
                "--provider-advert specified for unknown provider '{name}'"
            ));
        }
        let metadata = load_provider_advert(
            &path,
            plan_profile_handle.as_deref(),
            admission_registry.as_ref(),
            assume_now,
        )?;
        advert_data.insert(name, metadata);
    }

    for spec in &mut provider_specs {
        if let Some(metadata) = advert_data.remove(&spec.name) {
            if !metadata.supports_chunk_range {
                return Err(format!(
                    "provider advert '{}' does not advertise the chunk_range_fetch capability required for multi-source fetch",
                    spec.name
                ));
            }
            if !spec.concurrency_explicit
                && let Some(concurrency) = metadata.concurrency
            {
                spec.max_concurrent = concurrency;
            }
            if !spec.weight_explicit
                && let Some(weight) = metadata.weight
            {
                spec.weight = Some(weight);
            }
            spec.metadata = Some(metadata.provider_metadata);
        }
        if spec.weight.is_none() {
            spec.weight = Some(NonZeroU32::new(1).expect("constant non-zero"));
        }
    }

    for spec in &provider_specs {
        if let Some(metadata) = spec.metadata.as_ref() {
            ensure_range_capability_satisfies_plan(&plan, &spec.name, metadata)?;
        }
    }

    let scoreboard_mode = use_scoreboard || scoreboard_out.is_some() || telemetry_source.is_some();
    let telemetry_snapshot = load_telemetry(telemetry_source.clone())
        .map_err(|err| format!("failed to load telemetry: {err}"))?;

    let mut scoreboard_metadata = Vec::new();
    let mut scoreboard_aliases = Vec::new();

    for spec in &provider_specs {
        let metadata =
            metadata_for_provider_spec(spec, &plan, scoreboard_mode && !allow_implicit_metadata)?;
        scoreboard_aliases.push(spec.name.clone());
        scoreboard_metadata.push(metadata);
    }

    for provider in &gateway_fetch_providers {
        let alias = provider.id().as_str().to_string();
        let mut metadata = provider
            .metadata()
            .cloned()
            .unwrap_or_else(ProviderMetadata::new);
        if metadata.range_capability.is_none() {
            metadata.range_capability = Some(default_range_capability(&plan));
        }
        if metadata.stream_budget.is_none() {
            let concurrency = NonZeroUsize::new(provider.max_concurrent_chunks())
                .expect("provider concurrency must be non-zero");
            metadata.stream_budget = Some(stream_budget_from_plan(&plan, concurrency));
        }
        metadata.provider_id = Some(alias.clone());
        if !metadata.profile_aliases.iter().any(|entry| entry == &alias) {
            metadata.profile_aliases.push(alias.clone());
        }
        scoreboard_aliases.push(alias);
        scoreboard_metadata.push(metadata);
    }

    let gateway_manifest_provided = gateway_manifest_present(
        &gateway_manifest_envelope,
        !gateway_fetch_providers.is_empty(),
    );
    let mut scoreboard_config = scoreboard::ScoreboardConfig::default();
    if let Some(now) = assume_now {
        scoreboard_config.now_unix_secs = now;
    }
    let telemetry_label = telemetry_source_label
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| match telemetry_source.as_ref() {
            Some(JsonSource::File(path)) => Some(format!("file:{}", path.display())),
            Some(JsonSource::Stdin) => Some("stdin".to_string()),
            None => None,
        });
    let mut fetch_options = multi_fetch::FetchOptions::default();
    let mut max_providers_limit: Option<usize> = None;
    if skip_verify_digest {
        fetch_options.verify_digests = false;
    }
    if skip_verify_length {
        fetch_options.verify_lengths = false;
    }
    if let Some(limit) = retry_budget {
        fetch_options.per_chunk_retry_limit = Some(limit.max(1));
    }
    if let Some(threshold) = failure_threshold {
        fetch_options.provider_failure_threshold = threshold;
    }
    if let Some(limit) = max_parallel {
        fetch_options.global_parallel_limit = Some(limit.max(1));
    }
    if let Some(limit) = max_peers {
        let limit = limit.max(1);
        max_providers_limit = Some(limit);
        fetch_options.global_parallel_limit = Some(
            fetch_options
                .global_parallel_limit
                .map_or(limit, |existing| existing.min(limit)),
        );
    }

    let provider_sources = build_provider_sources(&provider_specs)?;
    let mut provider_source_map: HashMap<String, ProviderSource> = provider_sources
        .into_iter()
        .map(|source| (source.name.clone(), source))
        .collect();

    if !deny_providers.is_empty() || !boost_providers.is_empty() {
        let policy = CliScorePolicy::new(deny_providers, boost_providers);
        fetch_options.score_policy = Some(Arc::new(policy));
    }

    let scoreboard = scoreboard::build_scoreboard(
        &plan,
        &scoreboard_metadata,
        &telemetry_snapshot,
        &scoreboard_config,
    )
    .map_err(|err| format!("failed to build provider scoreboard: {err}"))?;

    let (mut eligible_aliases, provider_lookup, ineligible_entries) =
        classify_scoreboard_aliases(scoreboard.entries(), &scoreboard_aliases);
    for (alias, reason) in ineligible_entries {
        eprintln!("[scoreboard] excluding provider '{alias}': {reason}");
    }

    let mut eligible_alias_order = Vec::new();
    for (entry, alias) in scoreboard.entries().iter().zip(scoreboard_aliases.iter()) {
        if matches!(entry.eligibility, scoreboard::Eligibility::Eligible)
            && eligible_aliases.contains(alias)
        {
            eligible_alias_order.push(alias.clone());
        }
    }
    if let Some(limit) = max_providers_limit {
        let allowed: HashSet<String> = eligible_alias_order.into_iter().take(limit).collect();
        eligible_aliases.retain(|alias| allowed.contains(alias));
    }

    if eligible_aliases.is_empty() {
        return Err("scoreboard produced no eligible providers".into());
    }

    let mut runtime_registry: HashMap<String, ProviderRuntime> = HashMap::new();
    for alias in eligible_aliases.iter() {
        if let Some(source) = provider_source_map.remove(alias) {
            runtime_registry.insert(alias.clone(), ProviderRuntime::Local(source));
        }
    }
    for provider in &gateway_fetch_providers {
        let alias = provider.id().as_str().to_string();
        if eligible_aliases.contains(&alias) {
            runtime_registry.insert(alias.clone(), ProviderRuntime::Gateway);
        }
    }

    if runtime_registry.len() != eligible_aliases.len() {
        return Err(
            "scoreboard selected providers that are not available locally or via gateway".into(),
        );
    }

    let (provider_count_value, gateway_provider_count_value) =
        runtime_provider_counts(&runtime_registry);
    let provider_mix_label = provider_mix_label(provider_count_value, gateway_provider_count_value);
    let scoreboard_metadata_value = build_scoreboard_metadata(ScoreboardMetadataOptions {
        scoreboard_mode,
        allow_implicit_metadata,
        provider_count: provider_count_value,
        gateway_provider_count: gateway_provider_count_value,
        provider_mix: provider_mix_label,
        max_parallel,
        max_peers,
        retry_budget,
        failure_threshold,
        assume_now,
        telemetry_label: telemetry_label.as_deref(),
        telemetry_region: telemetry_region.as_deref(),
        gateway_manifest_id: scoreboard_gateway_manifest_id.as_deref(),
        gateway_manifest_cid: scoreboard_gateway_manifest_cid.as_deref(),
        gateway_manifest_envelope_present: gateway_manifest_provided,
        transport_policy,
        transport_policy_override,
        anonymity_policy,
        anonymity_policy_override,
    });
    if let Some(path) = scoreboard_out.as_ref() {
        scoreboard
            .persist_to_path(path, Some(scoreboard_metadata_value.clone()))
            .map_err(|err| format!("failed to persist scoreboard: {err}"))?;
    }

    let mut fetch_providers = Vec::new();
    for (entry, alias) in scoreboard.entries().iter().zip(scoreboard_aliases.iter()) {
        if !eligible_aliases.contains(alias) {
            continue;
        }
        if let scoreboard::Eligibility::Eligible = entry.eligibility {
            fetch_providers.push(entry.provider.clone());
            if let Some(limit) = max_providers_limit
                && fetch_providers.len() >= limit
            {
                break;
            }
        }
    }

    let provider_registry = Arc::new(runtime_registry);
    let provider_lookup = Arc::new(provider_lookup);
    let gateway_fetcher = gateway_fetcher_opt.clone();
    let fetcher = move |request: FetchRequest| {
        let registry = Arc::clone(&provider_registry);
        let lookup = Arc::clone(&provider_lookup);
        let gateway_fetcher = gateway_fetcher.clone();
        async move {
            let provider_id = request.provider.id().as_str().to_string();
            let provider_alias = lookup.get(&provider_id).cloned().unwrap_or(provider_id);
            let runtime = registry
                .get(&provider_alias)
                .cloned()
                .ok_or_else(|| FetchIoError::new("unknown provider in request"))?;
            match runtime {
                ProviderRuntime::Local(source) => source.fetch_chunk(&request.spec),
                ProviderRuntime::Gateway => {
                    let fetcher = gateway_fetcher
                        .clone()
                        .ok_or_else(|| FetchIoError::new("gateway fetcher unavailable"))?;
                    fetcher.fetch(request).await.map_err(FetchIoError::from)
                }
            }
        }
    };

    let streaming_writer = if let Some(path) = &output_path {
        Some(Arc::new(Mutex::new(StreamingWriter::create(path)?)))
    } else {
        None
    };

    let outcome = if let Some(writer) = streaming_writer.as_ref() {
        let observer = StreamingObserver::new(Arc::clone(writer));
        match futures::executor::block_on(multi_fetch::fetch_plan_parallel_with_observer(
            &plan,
            fetch_providers.clone(),
            fetcher,
            fetch_options.clone(),
            observer,
        )) {
            Ok(outcome) => outcome,
            Err(err) => return Err(format_multi_source_error(err)?),
        }
    } else {
        match futures::executor::block_on(multi_fetch::fetch_plan_parallel(
            &plan,
            fetch_providers,
            fetcher,
            fetch_options.clone(),
        )) {
            Ok(outcome) => outcome,
            Err(err) => return Err(format_multi_source_error(err)?),
        }
    };

    let streamed_stats = if let Some(writer) = streaming_writer.as_ref() {
        let mut guard = writer
            .lock()
            .map_err(|_| "streaming writer lock poisoned".to_string())?;
        guard
            .flush()
            .map_err(|err| format!("failed to flush output: {err}"))?;
        let bytes = guard.total_written();
        let digest = guard.current_digest();
        Some((bytes, digest))
    } else {
        None
    };

    let mut payload_vec = None;
    let (payload_len, payload_digest_bytes) = if let Some((written, digest)) = streamed_stats {
        if written != plan.content_length {
            return Err(format!(
                "streamed payload length {} does not match assembled length {}",
                written, plan.content_length
            ));
        }
        (written, digest)
    } else {
        let payload = outcome.assemble_payload();
        let digest = blake3::hash(&payload);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(digest.as_bytes());
        payload_vec = Some(payload);
        (payload_vec.as_ref().unwrap().len() as u64, bytes)
    };

    if let Some((_, digest)) = streamed_stats {
        if digest != payload_digest_bytes {
            return Err("streamed payload digest mismatch".into());
        }
    } else if let Some(path) = &output_path
        && let Some(payload) = payload_vec.as_ref()
    {
        write_binary(path, payload)?;
    }

    let mut car_stats = if let Some(path) = &car_out {
        Some(
            write_car_archive(&plan, &outcome.chunks, path)
                .map_err(|err| format!("failed to write CAR: {err}"))?,
        )
    } else {
        None
    };

    let mut car_verification: Option<CarVerificationReport> = None;
    if let Some(manifest_ref) = manifest.as_ref() {
        let car_bytes = if let Some(path) = &car_out {
            fs::read(path).map_err(|err| format!("failed to read CAR archive {path:?}: {err}"))?
        } else {
            build_car_bytes(&plan, &outcome.chunks)?
        };

        let verification = CarVerifier::verify_full_car_with_plan(manifest_ref, &plan, &car_bytes)
            .map_err(|err| format!("CAR verification failed: {err}"))?;
        eprintln!(
            "info: CAR verification succeeded (chunks={}, payload_bytes={})",
            verification.stats.chunk_count, verification.stats.payload_bytes
        );
        if car_stats.is_none() {
            car_stats = Some(verification.stats.clone());
        }
        car_verification = Some(verification);
    }

    if let Some(expected_len) = expect_payload_len
        && payload_len != expected_len
    {
        return Err(format!(
            "assembled payload length {payload_len} does not match expected {expected_len}"
        ));
    }

    if let Some(expected_digest) = expect_payload_digest
        && payload_digest_bytes != expected_digest
    {
        return Err(format!(
            "assembled payload digest {} does not match expected {}",
            to_hex(&payload_digest_bytes),
            to_hex(&expected_digest)
        ));
    }

    let car_stats_ref: Option<&CarWriteStats> = car_stats
        .as_ref()
        .or_else(|| car_verification.as_ref().map(|report| &report.stats));

    let transport_labels = transport_policy_labels(transport_policy, transport_policy_override);
    let report = build_report(ReportContext {
        outcome: &outcome,
        payload_len,
        digest: &payload_digest_bytes,
        car_stats: car_stats_ref,
        car_verification: car_verification.as_ref(),
        provider_count: provider_count_value,
        gateway_provider_count: gateway_provider_count_value,
        provider_mix: provider_mix_label,
        manifest_id: scoreboard_gateway_manifest_id.as_deref(),
        manifest_cid: scoreboard_gateway_manifest_cid.as_deref(),
        gateway_manifest_provided,
        telemetry_label: telemetry_label.as_deref(),
        telemetry_region: telemetry_region.as_deref(),
        transport_labels,
    });
    let mut report_str = to_string_pretty(&report)
        .map_err(|err| format!("failed to serialise orchestrator report: {err}"))?;
    report_str.push('\n');

    let mut report_written_to_stdout = false;
    if let Some(path) = &json_out {
        if path == Path::new("-") {
            report_written_to_stdout = true;
        }
        write_text(path, &report_str)?;
    }

    if let Some(path) = &provider_metrics_out {
        let providers = report
            .get("provider_reports")
            .ok_or_else(|| "internal error: provider_reports missing from report".to_string())?;
        let mut providers_str = to_string_pretty(providers)
            .map_err(|err| format!("failed to serialise provider metrics: {err}"))?;
        providers_str.push('\n');
        write_text(path, &providers_str)?;
    }

    if let Some(path) = &chunk_receipts_out {
        let receipts = report
            .get("chunk_receipts")
            .ok_or_else(|| "internal error: chunk_receipts missing from report".to_string())?;
        let mut receipts_str = to_string_pretty(receipts)
            .map_err(|err| format!("failed to serialise chunk receipts: {err}"))?;
        receipts_str.push('\n');
        write_text(path, &receipts_str)?;
    }

    if !report_written_to_stdout {
        print!("{report_str}");
    }
    Ok(())
}

const USAGE: &str = concat!(
    "usage: sorafs-fetch \
     --plan=chunk_fetch_specs.json|- \
     --provider=name=/path/to/payload[#concurrency] \
     [--provider=name=/path/to/payload[#concurrency][@weight] ...] \
     [--gateway-provider=name=alias,provider-id=hex,base-url=https://...,stream-token=base64 ...] \
     [--gateway-manifest-id=hex] [--gateway-manifest-envelope=base64] \
     [--gateway-client-id=string] [--gateway-chunker-handle=profile] \
     [--gateway-manifest-cid=hex] \
     [--provider-advert=name=/path/to/advert.norito ...] \
     [--admission-dir=governance/envelopes/] \
     [--manifest-report=report.json|-] \
     [--manifest=manifest.to|-] \
     [--output=assembled.bin] \
     [--car-out=payload.car] \
     [--json-out=report.json] \
     [--provider-metrics-out=providers.json] \
     [--chunk-receipts-out=receipts.json] \
     [--stats-out=stats.json] \
     [--scoreboard-out=scoreboard.json] \
     [--telemetry-json=telemetry.json|-] \
     [--telemetry-source-label=LABEL] \
     [--telemetry-region=REGION] \
     [--deny-provider=name ...] \
     [--boost-provider=name:delta ...] \
     [--max-parallel=n] \
     [--max-peers=n] \
     [--retry-budget=n] \
     [--retry-limit=n] \
	     [--provider-failure-threshold=n] \
	     [--allow-implicit-provider-metadata] \
	     [--allow-insecure] [--no-verify-digest] [--no-verify-length] \
	     [--expect-payload-digest=hex] \
	     [--expect-payload-len=bytes] \
	     [--assume-now=unix_secs]\n\
\n\
JSON outputs (`--json-out`, `--provider-metrics-out`) include provider metadata \
fields (`metadata.range_capability`, `metadata.stream_budget`, \
`metadata.transport_hints`) when adverts supply them. Scoreboard metadata \
checks are enabled by default; provide `--provider-advert=name=PATH` for every \
`--provider` entry or pass `--allow-implicit-provider-metadata` when replaying \
fixtures that rely on baked-in capability hints.\n\n",
    include_str!("../../../../docs/source/sorafs/snippets/multi_source_flag_notes.txt")
);

fn usage() -> &'static str {
    USAGE
}

#[derive(Clone)]
enum ProviderRuntime {
    Local(ProviderSource),
    Gateway,
}

fn runtime_provider_counts(runtime_registry: &HashMap<String, ProviderRuntime>) -> (u64, u64) {
    let mut provider_count: u64 = 0;
    let mut gateway_count: u64 = 0;
    for runtime in runtime_registry.values() {
        match runtime {
            ProviderRuntime::Local(_) => provider_count = provider_count.saturating_add(1),
            ProviderRuntime::Gateway => gateway_count = gateway_count.saturating_add(1),
        }
    }
    (provider_count, gateway_count)
}

fn provider_mix_label(provider_count: u64, gateway_count: u64) -> &'static str {
    match (provider_count > 0, gateway_count > 0) {
        (true, true) => "mixed",
        (true, false) => "direct-only",
        (false, true) => "gateway-only",
        (false, false) => "none",
    }
}

#[derive(Clone, Copy)]
struct ScoreboardMetadataOptions<'a> {
    scoreboard_mode: bool,
    allow_implicit_metadata: bool,
    provider_count: u64,
    gateway_provider_count: u64,
    provider_mix: &'a str,
    max_parallel: Option<usize>,
    max_peers: Option<usize>,
    retry_budget: Option<usize>,
    failure_threshold: Option<usize>,
    assume_now: Option<u64>,
    telemetry_label: Option<&'a str>,
    telemetry_region: Option<&'a str>,
    gateway_manifest_id: Option<&'a str>,
    gateway_manifest_cid: Option<&'a str>,
    gateway_manifest_envelope_present: bool,
    transport_policy: Option<TransportPolicy>,
    transport_policy_override: Option<TransportPolicy>,
    anonymity_policy: Option<AnonymityPolicy>,
    anonymity_policy_override: Option<AnonymityPolicy>,
}

fn build_scoreboard_metadata(options: ScoreboardMetadataOptions<'_>) -> Value {
    let mut metadata = Map::new();
    metadata.insert(
        "version".to_string(),
        Value::from(env!("CARGO_PKG_VERSION")),
    );
    metadata.insert(
        "use_scoreboard".to_string(),
        Value::from(options.scoreboard_mode),
    );
    metadata.insert(
        "allow_implicit_metadata".to_string(),
        Value::from(options.allow_implicit_metadata),
    );
    metadata.insert(
        "provider_count".to_string(),
        Value::from(options.provider_count),
    );
    metadata.insert(
        "gateway_provider_count".to_string(),
        Value::from(options.gateway_provider_count),
    );
    metadata.insert(
        "provider_mix".to_string(),
        Value::from(options.provider_mix),
    );
    let option_usize_to_value = |value: Option<usize>| {
        value
            .and_then(|val| u64::try_from(val).ok())
            .map(Value::from)
            .unwrap_or(Value::Null)
    };
    metadata.insert(
        "max_parallel".to_string(),
        option_usize_to_value(options.max_parallel),
    );
    metadata.insert(
        "max_peers".to_string(),
        option_usize_to_value(options.max_peers),
    );
    metadata.insert(
        "retry_budget".to_string(),
        option_usize_to_value(options.retry_budget),
    );
    metadata.insert(
        "provider_failure_threshold".to_string(),
        option_usize_to_value(options.failure_threshold),
    );
    metadata.insert(
        "assume_now".to_string(),
        options.assume_now.map(Value::from).unwrap_or(Value::Null),
    );
    metadata.insert(
        "telemetry_source".to_string(),
        options
            .telemetry_label
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    metadata.insert(
        "telemetry_region".to_string(),
        options
            .telemetry_region
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    metadata.insert(
        "gateway_manifest_id".to_string(),
        options
            .gateway_manifest_id
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    metadata.insert(
        "gateway_manifest_cid".to_string(),
        options
            .gateway_manifest_cid
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    metadata.insert(
        "gateway_manifest_provided".to_string(),
        Value::from(options.gateway_manifest_envelope_present),
    );
    let transport_labels =
        transport_policy_labels(options.transport_policy, options.transport_policy_override);
    metadata.insert(
        "transport_policy".to_string(),
        Value::from(transport_labels.effective_label),
    );
    metadata.insert(
        "transport_policy_override".to_string(),
        Value::from(transport_labels.override_flag),
    );
    metadata.insert(
        "transport_policy_override_label".to_string(),
        transport_labels
            .override_label
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    let anonymity_labels =
        anonymity_policy_labels(options.anonymity_policy, options.anonymity_policy_override);
    metadata.insert(
        "anonymity_policy".to_string(),
        Value::from(anonymity_labels.effective_label),
    );
    metadata.insert(
        "anonymity_policy_override".to_string(),
        Value::from(anonymity_labels.override_flag),
    );
    metadata.insert(
        "anonymity_policy_override_label".to_string(),
        anonymity_labels
            .override_label
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    Value::Object(metadata)
}

fn gateway_manifest_present(
    manifest_envelope: &Option<String>,
    has_gateway_providers: bool,
) -> bool {
    if !has_gateway_providers {
        return false;
    }
    let Some(raw_envelope) = manifest_envelope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let decoded = match base64::engine::general_purpose::STANDARD.decode(raw_envelope) {
        Ok(bytes) if !bytes.is_empty() => bytes,
        _ => return false,
    };
    match norito::decode_from_bytes::<HybridPayloadEnvelopeV1>(&decoded) {
        Ok(envelope)
            if envelope.version == HYBRID_PAYLOAD_ENVELOPE_VERSION_V1
                && !envelope.kem.ephemeral_public.is_empty()
                && !envelope.kem.kyber_ciphertext.is_empty()
                && !envelope.ciphertext.is_empty() =>
        {
            matches!(
                HybridSuite::from_str(&envelope.suite),
                Ok(HybridSuite::X25519MlKem768ChaCha20Poly1305)
            )
        }
        _ => false,
    }
}

fn parse_provider_spec(value: &str) -> Result<ProviderSpec, String> {
    let (name, rest) = value
        .split_once('=')
        .ok_or_else(|| "provider spec must be name=/path[#concurrency]".to_string())?;
    if name.is_empty() {
        return Err("provider name cannot be empty".to_string());
    }

    let mut path_segment = rest;
    let mut weight: Option<NonZeroU32> = None;
    if let Some(at_idx) = path_segment.rfind('@') {
        let (prefix, suffix) = path_segment.split_at(at_idx);
        if suffix.len() <= 1 {
            return Err("provider weight must follow '@'".to_string());
        }
        let parsed_weight = suffix[1..]
            .parse::<u32>()
            .map_err(|err| format!("invalid provider weight '{suffix}': {err}"))?;
        let nz_weight = NonZeroU32::new(parsed_weight)
            .ok_or_else(|| "provider weight must be greater than zero".to_string())?;
        weight = Some(nz_weight);
        path_segment = prefix;
    }

    let (path_str, concurrency) = if let Some(hash_idx) = path_segment.rfind('#') {
        let (path_part, conc_part) = path_segment.split_at(hash_idx);
        if conc_part.len() <= 1 {
            return Err("provider concurrency must follow '#'".to_string());
        }
        let conc_value = conc_part[1..]
            .parse::<usize>()
            .map_err(|err| format!("invalid provider concurrency '{conc_part}': {err}"))?;
        (path_part, Some(conc_value))
    } else {
        (path_segment, None)
    };

    if path_str.is_empty() {
        return Err("provider path cannot be empty".to_string());
    }

    let concurrency_explicit = concurrency.is_some();
    let max_concurrent = concurrency
        .and_then(NonZeroUsize::new)
        .unwrap_or_else(|| NonZeroUsize::new(2).expect("constant non-zero"));

    let weight_explicit = weight.is_some();
    let weight = weight.unwrap_or_else(|| NonZeroU32::new(1).expect("constant non-zero"));

    Ok(ProviderSpec {
        name: name.to_string(),
        path: PathBuf::from(path_str),
        max_concurrent,
        weight: Some(weight),
        concurrency_explicit,
        weight_explicit,
        metadata: None,
    })
}
fn parse_gateway_provider_spec(value: &str) -> Result<GatewayProviderSpec, String> {
    let mut name: Option<String> = None;
    let mut provider_id: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut stream_token: Option<String> = None;
    let mut privacy_events_url: Option<String> = None;

    for pair in value.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair.split_once('=').ok_or_else(|| {
            "--gateway-provider expects comma-separated key=value pairs".to_string()
        })?;
        let val = val.trim();
        match key {
            "name" => {
                if val.is_empty() {
                    return Err("--gateway-provider name must not be empty".into());
                }
                name = Some(val.to_string());
            }
            "provider-id" | "provider_id" => {
                if val.len() != 64 || !val.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("--gateway-provider provider-id must be 32-byte hex".into());
                }
                provider_id = Some(val.to_ascii_lowercase());
            }
            "base-url" | "base_url" => {
                if val.is_empty() {
                    return Err("--gateway-provider base-url must not be empty".into());
                }
                base_url = Some(val.to_string());
            }
            "privacy-url" | "privacy_url" => {
                if val.is_empty() {
                    return Err("--gateway-provider privacy-url must not be empty".into());
                }
                privacy_events_url = Some(val.to_string());
            }
            "stream-token" | "stream_token" => {
                if val.is_empty() {
                    return Err("--gateway-provider stream-token must not be empty".into());
                }
                stream_token = Some(val.to_string());
            }
            other => {
                return Err(format!(
                    "unknown --gateway-provider key '{other}'. expected name, provider-id, base-url, stream-token, privacy-url"
                ));
            }
        }
    }

    let name = name.ok_or_else(|| "--gateway-provider requires name=<alias>".to_string())?;
    let provider_id_hex =
        provider_id.ok_or_else(|| "--gateway-provider requires provider-id=<hex>".to_string())?;
    let base_url =
        base_url.ok_or_else(|| "--gateway-provider requires base-url=<https://...>".to_string())?;
    let stream_token_b64 = stream_token
        .ok_or_else(|| "--gateway-provider requires stream-token=<base64>".to_string())?;

    Ok(GatewayProviderSpec {
        name,
        provider_id_hex,
        base_url,
        stream_token_b64,
        privacy_events_url,
    })
}

fn parse_usize(input: &str, flag: &str) -> Result<usize, String> {
    input
        .parse::<usize>()
        .map_err(|err| format!("invalid value for {flag}: {err}"))
        .and_then(|value| {
            if value == 0 {
                Err(format!("{flag} must be at least 1"))
            } else {
                Ok(value)
            }
        })
}

fn parse_u64_value(input: &str) -> Result<u64, std::num::ParseIntError> {
    input.parse::<u64>()
}

fn build_provider_sources(specs: &[ProviderSpec]) -> Result<Vec<ProviderSource>, String> {
    specs
        .iter()
        .map(|spec| ProviderSource::new(&spec.name, &spec.path))
        .collect()
}

fn ensure_range_capability_satisfies_plan(
    plan: &CarBuildPlan,
    provider_name: &str,
    metadata: &ProviderMetadata,
) -> Result<(), String> {
    let Some(range) = metadata.range_capability.as_ref() else {
        return Ok(());
    };
    let max_chunk_length = plan
        .chunks
        .iter()
        .map(|chunk| chunk.length)
        .max()
        .unwrap_or(0);
    if max_chunk_length > range.max_chunk_span {
        return Err(format!(
            "provider '{provider_name}' advertises max_chunk_span={} bytes but plan chunks require up to {} bytes",
            range.max_chunk_span, max_chunk_length
        ));
    }
    if range.requires_alignment {
        let alignment = u64::from(range.min_granularity);
        if let Some((idx, chunk)) = plan.chunks.iter().enumerate().find(|(_, chunk)| {
            chunk.offset % alignment != 0 || u64::from(chunk.length) % alignment != 0
        }) {
            return Err(format!(
                "provider '{provider_name}' requires {}-byte alignment but chunk {} (offset={}, length={}) violates it",
                range.min_granularity, idx, chunk.offset, chunk.length
            ));
        }
    }
    Ok(())
}

fn load_json(source: &JsonSource) -> Result<Value, String> {
    match source {
        JsonSource::File(path) => load_json_file(path),
        JsonSource::Stdin => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|err| format!("failed to read JSON from stdin: {err}"))?;
            from_slice(&buf).map_err(|err| format!("failed to parse JSON from stdin: {err}"))
        }
    }
}

fn load_json_file(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|err| format!("failed to read {path:?}: {err}"))?;
    from_slice(&bytes).map_err(|err| format!("failed to parse JSON from {path:?}: {err}"))
}

fn write_binary(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(bytes)
            .map_err(|err| format!("failed to write binary payload to stdout: {err}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    fs::write(path, bytes).map_err(|err| format!("failed to write {path:?}: {err}"))
}

struct StreamingWriter {
    writer: BufWriter<File>,
    hasher: Hasher,
    total: u64,
}

impl StreamingWriter {
    fn create(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {parent:?}: {err}"))?;
        }
        let file = File::create(path)
            .map_err(|err| format!("failed to create output file {path:?}: {err}"))?;
        Ok(Self {
            writer: BufWriter::new(file),
            hasher: Hasher::new(),
            total: 0,
        })
    }

    fn write_chunk(&mut self, chunk_index: usize, bytes: &[u8]) -> Result<(), ObserverError> {
        self.writer.write_all(bytes).map_err(|err| {
            ObserverError::new(format!("failed to write chunk {chunk_index}: {err}"))
        })?;
        self.hasher.update(bytes);
        self.total += bytes.len() as u64;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.writer.flush()
    }

    fn total_written(&self) -> u64 {
        self.total
    }

    fn current_digest(&self) -> [u8; 32] {
        let clone = self.hasher.clone();
        let hash = clone.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(hash.as_bytes());
        out
    }
}

struct StreamingObserver {
    writer: Arc<Mutex<StreamingWriter>>,
}

impl StreamingObserver {
    fn new(writer: Arc<Mutex<StreamingWriter>>) -> Self {
        Self { writer }
    }
}

impl ChunkObserver for StreamingObserver {
    fn on_chunk(&mut self, delivery: ChunkDelivery<'_>) -> Result<(), ObserverError> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| ObserverError::new("streaming writer lock poisoned"))?;
        guard.write_chunk(delivery.chunk_index, delivery.bytes)
    }
}

const fn availability_label(tier: AvailabilityTier) -> &'static str {
    match tier {
        AvailabilityTier::Hot => "hot",
        AvailabilityTier::Warm => "warm",
        AvailabilityTier::Cold => "cold",
    }
}

const fn availability_weight_hint(tier: AvailabilityTier) -> NonZeroU32 {
    match tier {
        AvailabilityTier::Hot => NonZeroU32::new(3).expect("non-zero"),
        AvailabilityTier::Warm => NonZeroU32::new(2).expect("non-zero"),
        AvailabilityTier::Cold => NonZeroU32::new(1).expect("non-zero"),
    }
}

fn capability_name(capability: CapabilityType) -> &'static str {
    match capability {
        CapabilityType::ToriiGateway => "torii_gateway",
        CapabilityType::QuicNoise => "quic_noise",
        CapabilityType::ChunkRangeFetch => "chunk_range_fetch",
        CapabilityType::SoraNetHybridPq => "soranet_pq",
        CapabilityType::VendorReserved => "vendor_reserved",
    }
}

fn ensure_capability_label(labels: &mut Vec<String>, label: &str) {
    if !labels.iter().any(|existing| existing == label) {
        labels.push(label.to_string());
    }
}

fn transport_protocol_label(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::ToriiHttpRange => "torii_http_range",
        TransportProtocol::QuicStream => "quic_stream",
        TransportProtocol::SoraNetRelay => "soranet_relay",
        TransportProtocol::VendorReserved => "vendor_reserved",
    }
}

fn provider_advert_to_metadata(advert: ProviderAdvertV1) -> Result<AdvertMetadata, String> {
    let qos_concurrency = NonZeroUsize::new(advert.body.qos.max_concurrent_streams as usize)
        .ok_or_else(|| "provider advert advertised max_concurrent_streams=0".to_string())?;
    let mut concurrency = qos_concurrency;
    let mut supports_chunk_range = false;
    let provider_hex = to_hex(&advert.body.provider_id);
    let mut provider_metadata = ProviderMetadata::new();
    provider_metadata.provider_id = Some(to_hex(&advert.body.provider_id));
    provider_metadata.profile_id = Some(advert.body.profile_id.clone());
    let mut aliases = advert.body.profile_aliases.clone().unwrap_or_default();
    if !aliases.iter().any(|alias| alias == &advert.body.profile_id) {
        aliases.insert(0, advert.body.profile_id.clone());
    }
    aliases.retain(|alias| !alias.is_empty());
    aliases.dedup();
    provider_metadata.profile_aliases = aliases;
    provider_metadata.availability =
        Some(availability_label(advert.body.qos.availability).to_string());
    provider_metadata.stake_amount = Some(advert.body.stake.stake_amount.to_string());
    provider_metadata.max_streams = Some(advert.body.qos.max_concurrent_streams);
    provider_metadata.refresh_deadline = Some(advert.refresh_deadline());
    provider_metadata.expires_at = Some(advert.expires_at);
    provider_metadata.ttl_secs = Some(advert.ttl());
    provider_metadata.allow_unknown_capabilities = advert.allow_unknown_capabilities;
    provider_metadata.capability_names = advert
        .body
        .capabilities
        .iter()
        .filter_map(|cap| {
            if KNOWN_CAPABILITIES.contains(&cap.cap_type) {
                Some(capability_name(cap.cap_type).to_string())
            } else {
                None
            }
        })
        .collect();
    provider_metadata.notes = advert.body.notes.clone();
    provider_metadata.rendezvous_topics = advert
        .body
        .rendezvous_topics
        .iter()
        .map(|topic| format!("{}:{}", topic.topic, topic.region))
        .collect();

    for capability in &advert.body.capabilities {
        if capability.cap_type == CapabilityType::ChunkRangeFetch {
            supports_chunk_range = true;
            match ProviderCapabilityRangeV1::from_bytes(&capability.payload) {
                Ok(range) => {
                    provider_metadata.range_capability = Some(RangeCapability {
                        max_chunk_span: range.max_chunk_span,
                        min_granularity: range.min_granularity,
                        supports_sparse_offsets: range.supports_sparse_offsets,
                        requires_alignment: range.requires_alignment,
                        supports_merkle_proof: range.supports_merkle_proof,
                    });
                }
                Err(err) => {
                    return Err(format!(
                        "provider advert advertised chunk_range_fetch capability with invalid payload: {err}"
                    ));
                }
            }
            continue;
        }
        if capability.cap_type == CapabilityType::SoraNetHybridPq {
            let pq = ProviderCapabilitySoranetPqV1::from_bytes(&capability.payload).map_err(
                |err| {
                    format!(
                        "provider advert advertised soranet_pq capability with invalid payload: {err}"
                    )
                },
            )?;
            ensure_capability_label(&mut provider_metadata.capability_names, "soranet_pq");
            ensure_capability_label(&mut provider_metadata.capability_names, "soranet_pq_guard");
            if pq.supports_majority {
                ensure_capability_label(
                    &mut provider_metadata.capability_names,
                    "soranet_pq_majority",
                );
            }
            if pq.supports_strict {
                ensure_capability_label(
                    &mut provider_metadata.capability_names,
                    "soranet_pq_strict",
                );
            }
        }
    }

    let transport_hints = advert
        .body
        .transport_hints
        .as_ref()
        .map_or(&[] as &[TransportHintV1], |hints| hints.as_slice());
    if supports_chunk_range && transport_hints.is_empty() {
        return Err(format!(
            "provider advert {provider_hex} is missing transport_hints required for chunk_range_fetch"
        ));
    }
    provider_metadata.transport_hints = transport_hints
        .iter()
        .map(|hint| TransportHint {
            protocol: transport_protocol_label(hint.protocol).to_string(),
            protocol_id: hint.protocol as u8,
            priority: hint.priority,
        })
        .collect();

    if supports_chunk_range {
        let budget = advert.body.stream_budget.as_ref().ok_or_else(|| {
            format!(
                "provider advert {provider_hex} is missing a stream_budget required for chunk_range_fetch"
            )
        })?;
        budget.validate().map_err(|err| {
            format!("provider advert {provider_hex} has an invalid stream_budget: {err}")
        })?;
        provider_metadata.stream_budget = Some(StreamBudget {
            max_in_flight: budget.max_in_flight,
            max_bytes_per_sec: budget.max_bytes_per_sec,
            burst_bytes: budget.burst_bytes,
        });
        let budget_limit = usize::from(budget.max_in_flight);
        let clamped = concurrency.get().min(budget_limit.max(1));
        concurrency = NonZeroUsize::new(clamped)
            .ok_or_else(|| "stream budget advertised max_in_flight=0".to_string())?;
    }

    Ok(AdvertMetadata {
        concurrency: Some(concurrency),
        weight: Some(availability_weight_hint(advert.body.qos.availability)),
        supports_chunk_range,
        provider_metadata,
    })
}

fn load_admission_registry(dir: &Path) -> Result<HashMap<[u8; 32], AdmissionRecord>, String> {
    if !dir.is_dir() {
        return Err(format!(
            "admission directory {dir:?} does not exist or is not a directory"
        ));
    }
    let mut registry = HashMap::new();
    let read_dir = fs::read_dir(dir)
        .map_err(|err| format!("failed to read admission directory {dir:?}: {err}"))?;
    for entry in read_dir {
        let entry =
            entry.map_err(|err| format!("failed to read admission entry in {dir:?}: {err}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|err| format!("failed to read admission envelope {path:?}: {err}"))?;
        let envelope = decode_admission_envelope(&bytes, &path)?;
        let record = AdmissionRecord::new(envelope)
            .map_err(|err| format!("invalid admission envelope {path:?}: {err}"))?;
        let provider_id = *record.provider_id();
        if registry.insert(provider_id, record).is_some() {
            return Err(format!(
                "duplicate admission envelope for provider {} in {dir:?}",
                hex_encode(provider_id)
            ));
        }
    }
    Ok(registry)
}

fn decode_admission_envelope(
    bytes: &[u8],
    path: &Path,
) -> Result<ProviderAdmissionEnvelopeV1, String> {
    decode_from_bytes(bytes)
        .map_err(|err| format!("failed to decode admission envelope {path:?} as Norito: {err}"))
}

fn verify_provider_advert_signature(advert: &ProviderAdvertV1) -> Result<(), String> {
    if advert.signature.algorithm != SignatureAlgorithm::Ed25519 {
        return Err(format!(
            "unsupported signature algorithm: {:?}",
            advert.signature.algorithm
        ));
    }
    if advert.signature.public_key.len() != PUBLIC_KEY_LENGTH {
        return Err(format!(
            "provider advert public key must be {PUBLIC_KEY_LENGTH} bytes (found {})",
            advert.signature.public_key.len()
        ));
    }
    if advert.signature.signature.len() != SIGNATURE_LENGTH {
        return Err(format!(
            "provider advert signature must be {SIGNATURE_LENGTH} bytes (found {})",
            advert.signature.signature.len()
        ));
    }

    let mut pk = [0u8; PUBLIC_KEY_LENGTH];
    pk.copy_from_slice(&advert.signature.public_key);
    let verifying_key =
        VerifyingKey::from_bytes(&pk).map_err(|err| format!("invalid advert public key: {err}"))?;

    let mut sig_bytes = [0u8; SIGNATURE_LENGTH];
    sig_bytes.copy_from_slice(&advert.signature.signature);
    let signature = DalekSignature::from_bytes(&sig_bytes);

    let body_bytes = norito::to_bytes(&advert.body)
        .map_err(|err| format!("failed to encode advert body: {err}"))?;

    verifying_key
        .verify(&body_bytes, &signature)
        .map_err(|err| format!("advert signature verification failed: {err}"))
}

fn write_car_archive(
    plan: &CarBuildPlan,
    chunks: &[Vec<u8>],
    path: &Path,
) -> Result<CarWriteStats, String> {
    if chunks.len() != plan.chunks.len() {
        return Err(format!(
            "chunk count mismatch plan={} outcome={}",
            plan.chunks.len(),
            chunks.len()
        ));
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    let file =
        File::create(path).map_err(|err| format!("failed to create CAR file {path:?}: {err}"))?;
    let mut writer = BufWriter::new(file);
    let mut cursor = ChunkCursor::new(chunks);
    let stats = CarStreamingWriter::new(plan)
        .write_from_reader(&mut cursor, &mut writer)
        .map_err(|err| err.to_string())?;
    writer.flush().map_err(|err| err.to_string())?;
    Ok(stats)
}

fn build_car_bytes(plan: &CarBuildPlan, chunks: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if chunks.len() != plan.chunks.len() {
        return Err(format!(
            "chunk count mismatch plan={} outcome={}",
            plan.chunks.len(),
            chunks.len()
        ));
    }
    let mut chunk_cursor = ChunkCursor::new(chunks);
    let mut cursor = io::Cursor::new(Vec::new());
    CarStreamingWriter::new(plan)
        .write_from_reader(&mut chunk_cursor, &mut cursor)
        .map_err(|err| err.to_string())?;
    Ok(cursor.into_inner())
}

fn manifest_from_report(report: &Value) -> Result<Option<ManifestV1>, String> {
    if let Some(manifest_obj) = report.get("manifest")
        && let Some(hex) = manifest_obj.get("manifest_hex").and_then(Value::as_str)
    {
        return decode_manifest_hex(hex).map(Some);
    }
    if let Some(hex) = report.get("manifest_hex").and_then(Value::as_str) {
        return decode_manifest_hex(hex).map(Some);
    }
    Ok(None)
}

fn decode_manifest_hex(hex_str: &str) -> Result<ManifestV1, String> {
    let bytes =
        hex::decode(hex_str).map_err(|err| format!("failed to decode manifest_hex: {err}"))?;
    decode_manifest_bytes(&bytes)
}

fn decode_manifest_bytes(bytes: &[u8]) -> Result<ManifestV1, String> {
    decode_from_bytes(bytes)
        .map_err(|err| format!("failed to decode manifest payload as Norito: {err}"))
}

fn load_manifest_from_source(source: &BinarySource) -> Result<ManifestV1, String> {
    let bytes = match source {
        BinarySource::File(path) => {
            fs::read(path).map_err(|err| format!("failed to read manifest {path:?}: {err}"))?
        }
        BinarySource::Stdin => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|err| format!("failed to read manifest from stdin: {err}"))?;
            buf
        }
    };
    decode_manifest_bytes(&bytes)
}

struct ChunkCursor<'a> {
    chunks: &'a [Vec<u8>],
    chunk_index: usize,
    offset: usize,
}

impl<'a> ChunkCursor<'a> {
    fn new(chunks: &'a [Vec<u8>]) -> Self {
        Self {
            chunks,
            chunk_index: 0,
            offset: 0,
        }
    }
}

impl Read for ChunkCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut written = 0;
        while written < buf.len() {
            if self.chunk_index >= self.chunks.len() {
                break;
            }
            let chunk = &self.chunks[self.chunk_index];
            if self.offset >= chunk.len() {
                self.chunk_index += 1;
                self.offset = 0;
                continue;
            }
            let available = chunk.len() - self.offset;
            let to_copy = available.min(buf.len() - written);
            buf[written..written + to_copy]
                .copy_from_slice(&chunk[self.offset..self.offset + to_copy]);
            self.offset += to_copy;
            written += to_copy;
        }
        if written == 0 { Ok(0) } else { Ok(written) }
    }
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(text.as_bytes())
            .map_err(|err| format!("failed to write to stdout: {err}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    fs::write(path, text.as_bytes()).map_err(|err| format!("failed to write {path:?}: {err}"))
}

fn load_provider_advert(
    path: &Path,
    expected_profile: Option<&str>,
    admissions: Option<&HashMap<[u8; 32], AdmissionRecord>>,
    now_override: Option<u64>,
) -> Result<AdvertMetadata, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("failed to read provider advert {path:?}: {err}"))?;
    let advert: ProviderAdvertV1 = decode_from_bytes(&bytes).map_err(|err| err.to_string())?;
    let now = now_override
        .or_else(unix_time_now)
        .unwrap_or(advert.expires_at);
    advert
        .validate_with_body(now)
        .map_err(|err| err.to_string())?;
    verify_provider_advert_signature(&advert)?;
    let refresh_deadline = advert.refresh_deadline();
    if now >= refresh_deadline {
        return Err(format!(
            "provider advert {path:?} is stale (now={now}, refresh_deadline={refresh_deadline})"
        ));
    }
    let unknown_capabilities: Vec<CapabilityType> = advert
        .body
        .capabilities
        .iter()
        .map(|cap| cap.cap_type)
        .filter(|cap| !KNOWN_CAPABILITIES.contains(cap))
        .collect();
    if !unknown_capabilities.is_empty() && !advert.allow_unknown_capabilities {
        let names: Vec<&'static str> = unknown_capabilities
            .iter()
            .map(|cap| capability_name(*cap))
            .collect();
        return Err(format!(
            "provider advert {path:?} advertised unsupported capabilities {} without allow_unknown_capabilities=true",
            names.join(", ")
        ));
    }
    if !unknown_capabilities.is_empty() {
        eprintln!(
            "warning: provider advert {path:?} advertised unknown capabilities {}; ignoring them",
            unknown_capabilities
                .iter()
                .map(|cap| capability_name(*cap))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(expected) = expected_profile
        && advert.body.profile_id.trim() != expected
    {
        eprintln!(
            "warning: provider advert profile '{}' does not match plan profile '{}'",
            advert.body.profile_id, expected
        );
    }
    if let Some(records) = admissions {
        let provider_id = advert.body.provider_id;
        let record = records.get(&provider_id).ok_or_else(|| {
            format!(
                "provider advert {path:?} (provider {}) is not authorised by the supplied admission directory",
                hex_encode(provider_id)
            )
        })?;
        verify_advert_against_record(&advert, record).map_err(|err| {
            format!("provider advert {path:?} failed admission verification: {err}")
        })?;
    }
    provider_advert_to_metadata(advert)
}

fn metadata_for_provider_spec(
    spec: &ProviderSpec,
    plan: &CarBuildPlan,
    enforce_advert: bool,
) -> Result<ProviderMetadata, String> {
    if let Some(existing) = spec.metadata.clone() {
        let mut meta = existing;
        meta.provider_id = Some(spec.name.clone());
        if !meta.profile_aliases.iter().any(|alias| alias == &spec.name) {
            meta.profile_aliases.push(spec.name.clone());
        }
        if meta.range_capability.is_none() {
            meta.range_capability = Some(default_range_capability(plan));
        }
        if meta.stream_budget.is_none() {
            meta.stream_budget = Some(stream_budget_from_plan(plan, spec.max_concurrent));
        }
        if meta.max_streams.is_none() {
            meta.max_streams = Some(spec.max_concurrent.get().min(u16::MAX as usize) as u16);
        }
        return Ok(meta);
    }

    if enforce_advert {
        return Err(format!(
            "multi-source fetch requires a --provider-advert for provider '{}' (or pass --allow-implicit-provider-metadata to reuse baked-in hints)",
            spec.name
        ));
    }

    let mut meta = ProviderMetadata::new();
    meta.provider_id = Some(spec.name.clone());
    meta.profile_aliases.push(spec.name.clone());
    meta.range_capability = Some(default_range_capability(plan));
    meta.stream_budget = Some(stream_budget_from_plan(plan, spec.max_concurrent));
    meta.max_streams = Some(spec.max_concurrent.get().min(u16::MAX as usize) as u16);
    Ok(meta)
}

fn default_range_capability(plan: &CarBuildPlan) -> RangeCapability {
    let max_span = plan.chunk_profile.max_size.min(u32::MAX as usize) as u32;
    let min_granularity = plan.chunk_profile.min_size.min(u32::MAX as usize) as u32;
    RangeCapability {
        max_chunk_span: max_span,
        min_granularity,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    }
}

fn stream_budget_from_plan(plan: &CarBuildPlan, concurrency: NonZeroUsize) -> StreamBudget {
    let max_chunk = plan
        .chunks
        .iter()
        .map(|chunk| u64::from(chunk.length))
        .max()
        .unwrap_or(plan.chunk_profile.max_size as u64)
        .max(1);
    let concurrency_u64 = concurrency.get() as u64;
    let max_bytes_per_sec = max_chunk.saturating_mul(concurrency_u64);
    let burst_bytes = Some(max_bytes_per_sec.saturating_mul(2));
    StreamBudget {
        max_in_flight: concurrency.get().min(u16::MAX as usize) as u16,
        max_bytes_per_sec,
        burst_bytes,
    }
}

type ScoreboardClassification = (
    HashSet<String>,
    HashMap<String, String>,
    Vec<(String, scoreboard::IneligibilityReason)>,
);

fn classify_scoreboard_aliases(
    entries: &[scoreboard::ScoreboardEntry],
    aliases: &[String],
) -> ScoreboardClassification {
    let mut eligible = HashSet::new();
    let mut lookup = HashMap::new();
    let mut ineligible = Vec::new();

    for (entry, alias) in entries.iter().zip(aliases.iter()) {
        match entry.eligibility {
            scoreboard::Eligibility::Eligible => {
                let alias_str = alias.clone();
                let provider_id = entry.provider.id().as_str().to_string();
                if !lookup.contains_key(&alias_str) {
                    lookup.insert(alias_str.clone(), alias_str.clone());
                }
                lookup
                    .entry(provider_id)
                    .or_insert_with(|| alias_str.clone());
                eligible.insert(alias_str);
            }
            scoreboard::Eligibility::Ineligible(ref reason) => {
                ineligible.push((alias.clone(), reason.clone()));
            }
        }
    }

    (eligible, lookup, ineligible)
}

fn unix_time_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

struct ReportContext<'a> {
    outcome: &'a FetchOutcome,
    payload_len: u64,
    digest: &'a [u8],
    car_stats: Option<&'a CarWriteStats>,
    car_verification: Option<&'a CarVerificationReport>,
    provider_count: u64,
    gateway_provider_count: u64,
    provider_mix: &'static str,
    manifest_id: Option<&'a str>,
    manifest_cid: Option<&'a str>,
    gateway_manifest_provided: bool,
    telemetry_label: Option<&'a str>,
    telemetry_region: Option<&'a str>,
    transport_labels: PolicyLabelSummary,
}

fn build_report(context: ReportContext<'_>) -> Value {
    let ReportContext {
        outcome,
        payload_len,
        digest,
        car_stats,
        car_verification,
        provider_count,
        gateway_provider_count,
        provider_mix,
        manifest_id,
        manifest_cid,
        gateway_manifest_provided,
        telemetry_label,
        telemetry_region,
        transport_labels,
    } = context;
    let mut root = Map::new();
    let chunk_count = outcome.chunks.len() as u64;
    let total_attempts: u64 = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| receipt.attempts as u64)
        .sum();
    let retry_count: u64 = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| receipt.attempts.saturating_sub(1) as u64)
        .sum();
    let provider_failure_total: u64 = outcome
        .provider_reports
        .iter()
        .map(|report| report.failures as u64)
        .sum();
    let provider_disabled_total: u64 = outcome
        .provider_reports
        .iter()
        .filter(|report| report.disabled)
        .count() as u64;
    let provider_success_total: u64 = outcome
        .provider_reports
        .iter()
        .map(|report| report.successes as u64)
        .sum();

    root.insert("chunk_count".into(), Value::from(chunk_count));
    root.insert("provider_count".into(), Value::from(provider_count));
    root.insert(
        "gateway_provider_count".into(),
        Value::from(gateway_provider_count),
    );
    root.insert("provider_mix".into(), Value::from(provider_mix));
    if let Some(value) = manifest_id {
        root.insert("manifest_id".into(), Value::from(value));
    }
    if let Some(value) = manifest_cid {
        root.insert("manifest_cid".into(), Value::from(value));
    }
    if let Some(label) = telemetry_label {
        root.insert("telemetry_source".into(), Value::from(label));
    }
    if let Some(region) = telemetry_region {
        root.insert("telemetry_region".into(), Value::from(region));
    }
    root.insert(
        "gateway_manifest_provided".into(),
        Value::from(gateway_manifest_provided),
    );
    root.insert(
        "transport_policy".into(),
        Value::from(transport_labels.effective_label),
    );
    root.insert(
        "transport_policy_override".into(),
        Value::from(transport_labels.override_flag),
    );
    root.insert(
        "transport_policy_override_label".into(),
        transport_labels
            .override_label
            .map(Value::from)
            .unwrap_or(Value::Null),
    );
    root.insert("payload_len".into(), Value::from(payload_len));
    root.insert("payload_digest_hex".into(), Value::from(to_hex(digest)));
    root.insert("chunk_attempt_total".into(), Value::from(total_attempts));
    root.insert("chunk_retry_total".into(), Value::from(retry_count));
    let chunk_retry_rate = if chunk_count > 0 {
        retry_count as f64 / chunk_count as f64
    } else {
        0.0
    };
    let chunk_attempt_avg = if chunk_count > 0 {
        total_attempts as f64 / chunk_count as f64
    } else {
        0.0
    };
    root.insert("chunk_retry_rate".into(), Value::from(chunk_retry_rate));
    root.insert(
        "chunk_attempt_average".into(),
        Value::from(chunk_attempt_avg),
    );
    root.insert(
        "provider_success_total".into(),
        Value::from(provider_success_total),
    );
    root.insert(
        "provider_failure_total".into(),
        Value::from(provider_failure_total),
    );
    let provider_total_ops = provider_success_total + provider_failure_total;
    let provider_failure_rate = if provider_total_ops > 0 {
        provider_failure_total as f64 / provider_total_ops as f64
    } else {
        0.0
    };
    root.insert(
        "provider_failure_rate".into(),
        Value::from(provider_failure_rate),
    );
    root.insert(
        "provider_disabled_total".into(),
        Value::from(provider_disabled_total),
    );

    let provider_reports: Vec<Value> = outcome
        .provider_reports
        .iter()
        .map(provider_report_to_value)
        .collect();
    root.insert("provider_reports".into(), Value::Array(provider_reports));

    let chunk_receipts: Vec<Value> = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| {
            let mut obj = Map::new();
            obj.insert(
                "chunk_index".into(),
                Value::from(receipt.chunk_index as u64),
            );
            obj.insert("provider".into(), Value::from(receipt.provider.as_str()));
            obj.insert("attempts".into(), Value::from(receipt.attempts as u64));
            obj.insert("latency_ms".into(), Value::from(receipt.latency_ms));
            obj.insert("bytes".into(), Value::from(receipt.bytes as u64));
            Value::Object(obj)
        })
        .collect();
    root.insert("chunk_receipts".into(), Value::Array(chunk_receipts));

    let (car_stats_to_use, verification_opt) = match (car_stats, car_verification) {
        (Some(stats), verification) => (Some(stats), verification),
        (None, Some(verification)) => (Some(&verification.stats), Some(verification)),
        (None, None) => (None, None),
    };

    if let Some(stats) = car_stats_to_use {
        let mut car_obj = Map::new();
        car_obj.insert("size".into(), Value::from(stats.car_size));
        car_obj.insert(
            "payload_digest_hex".into(),
            Value::from(to_hex(stats.car_payload_digest.as_bytes())),
        );
        car_obj.insert(
            "archive_digest_hex".into(),
            Value::from(to_hex(stats.car_archive_digest.as_bytes())),
        );
        car_obj.insert("cid_hex".into(), Value::from(to_hex(&stats.car_cid)));
        car_obj.insert(
            "root_cids_hex".into(),
            Value::Array(
                stats
                    .root_cids
                    .iter()
                    .map(|cid| Value::from(to_hex(cid)))
                    .collect(),
            ),
        );
        if let Some(verification) = verification_opt {
            car_obj.insert("verified".into(), Value::from(true));
            car_obj.insert(
                "por_leaf_count".into(),
                Value::from(verification.chunk_store.por_leaf_count() as u64),
            );
        }
        root.insert("car_archive".into(), Value::Object(car_obj));
    }

    Value::Object(root)
}

fn provider_report_to_value(report: &ProviderReport) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "provider".into(),
        Value::from(report.provider.id().as_str()),
    );
    obj.insert("successes".into(), Value::from(report.successes as u64));
    obj.insert("failures".into(), Value::from(report.failures as u64));
    obj.insert("disabled".into(), Value::from(report.disabled));
    if let Some(metadata) = report.provider.metadata() {
        let mut meta = Map::new();
        if let Some(id) = &metadata.provider_id {
            meta.insert("provider_id".into(), Value::from(id.clone()));
        }
        if let Some(profile) = &metadata.profile_id {
            meta.insert("profile_id".into(), Value::from(profile.clone()));
        }
        if !metadata.profile_aliases.is_empty() {
            meta.insert(
                "profile_aliases".into(),
                Value::Array(
                    metadata
                        .profile_aliases
                        .iter()
                        .cloned()
                        .map(Value::from)
                        .collect(),
                ),
            );
        }
        if let Some(availability) = &metadata.availability {
            meta.insert("availability".into(), Value::from(availability.clone()));
        }
        if let Some(stake) = &metadata.stake_amount {
            meta.insert("stake_amount".into(), Value::from(stake.clone()));
        }
        if let Some(max_streams) = metadata.max_streams {
            meta.insert("max_streams".into(), Value::from(max_streams as u64));
        }
        if let Some(refresh_deadline) = metadata.refresh_deadline {
            meta.insert("refresh_deadline".into(), Value::from(refresh_deadline));
        }
        if let Some(expires_at) = metadata.expires_at {
            meta.insert("expires_at".into(), Value::from(expires_at));
        }
        if let Some(ttl_secs) = metadata.ttl_secs {
            meta.insert("ttl_secs".into(), Value::from(ttl_secs));
        }
        meta.insert(
            "allow_unknown_capabilities".into(),
            Value::from(metadata.allow_unknown_capabilities),
        );
        if !metadata.capability_names.is_empty() {
            let caps = metadata
                .capability_names
                .iter()
                .cloned()
                .map(Value::from)
                .collect();
            meta.insert("capabilities".into(), Value::Array(caps));
        }
        if let Some(notes) = &metadata.notes {
            meta.insert("notes".into(), Value::from(notes.clone()));
        }
        if !metadata.rendezvous_topics.is_empty() {
            let topics = metadata
                .rendezvous_topics
                .iter()
                .cloned()
                .map(Value::from)
                .collect();
            meta.insert("rendezvous_topics".into(), Value::Array(topics));
        }
        if let Some(range) = &metadata.range_capability {
            meta.insert(
                "range_capability".into(),
                range_capability_metadata_to_value(range),
            );
        }
        if let Some(budget) = &metadata.stream_budget {
            meta.insert(
                "stream_budget".into(),
                stream_budget_metadata_to_value(budget),
            );
        }
        if !metadata.transport_hints.is_empty() {
            let hints = metadata
                .transport_hints
                .iter()
                .map(transport_hint_metadata_to_value)
                .collect();
            meta.insert("transport_hints".into(), Value::Array(hints));
        }
        obj.insert("metadata".into(), Value::Object(meta));
    }
    Value::Object(obj)
}

fn range_capability_metadata_to_value(range: &RangeCapability) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "max_chunk_span".into(),
        Value::from(range.max_chunk_span as u64),
    );
    obj.insert(
        "min_granularity".into(),
        Value::from(range.min_granularity as u64),
    );
    obj.insert(
        "supports_sparse_offsets".into(),
        Value::from(range.supports_sparse_offsets),
    );
    obj.insert(
        "requires_alignment".into(),
        Value::from(range.requires_alignment),
    );
    obj.insert(
        "supports_merkle_proof".into(),
        Value::from(range.supports_merkle_proof),
    );
    Value::Object(obj)
}

fn stream_budget_metadata_to_value(budget: &StreamBudget) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "max_in_flight".into(),
        Value::from(budget.max_in_flight as u64),
    );
    obj.insert(
        "max_bytes_per_sec".into(),
        Value::from(budget.max_bytes_per_sec),
    );
    obj.insert(
        "burst_bytes".into(),
        budget.burst_bytes.map(Value::from).unwrap_or(Value::Null),
    );
    Value::Object(obj)
}

fn transport_hint_metadata_to_value(hint: &TransportHint) -> Value {
    let mut obj = Map::new();
    obj.insert("protocol".into(), Value::from(hint.protocol.clone()));
    obj.insert("protocol_id".into(), Value::from(hint.protocol_id as u64));
    obj.insert("priority".into(), Value::from(hint.priority as u64));
    Value::Object(obj)
}

fn to_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

fn format_multi_source_error(error: MultiSourceError) -> Result<String, String> {
    use MultiSourceError::*;
    match error {
        NoProviders => Ok("no providers were supplied".to_string()),
        NoHealthyProviders {
            chunk_index,
            attempts,
            last_error,
        } => Ok(format!(
            "no healthy providers remaining after {attempts} attempt(s) for chunk {chunk_index}: {last_error:?}"
        )),
        NoCompatibleProviders {
            chunk_index,
            providers,
        } => {
            let details = providers
                .iter()
                .map(|(provider, reason)| format!("{provider}: {reason}"))
                .collect::<Vec<_>>()
                .join("; ");
            Ok(format!(
                "no compatible providers available for chunk {chunk_index}: {details}"
            ))
        }
        ExhaustedRetries {
            chunk_index,
            attempts,
            last_error,
        } => Ok(format!(
            "retry budget exhausted after {attempts} attempt(s) for chunk {chunk_index}: {last_error:?}"
        )),
        ObserverFailed {
            chunk_index,
            source,
        } => Ok(format!(
            "streaming observer failed for chunk {chunk_index}: {source}"
        )),
        InternalInvariant(reason) => Err(format!(
            "orchestrator invariant violated: {reason}. this is a bug in the CLI"
        )),
    }
}
#[derive(Debug, Clone)]
struct GatewayProviderSpec {
    name: String,
    provider_id_hex: String,
    base_url: String,
    stream_token_b64: String,
    privacy_events_url: Option<String>,
}

#[derive(Debug)]
struct ProviderSpec {
    name: String,
    path: PathBuf,
    max_concurrent: NonZeroUsize,
    weight: Option<NonZeroU32>,
    concurrency_explicit: bool,
    weight_explicit: bool,
    metadata: Option<ProviderMetadata>,
}

struct AdvertMetadata {
    concurrency: Option<NonZeroUsize>,
    weight: Option<NonZeroU32>,
    supports_chunk_range: bool,
    provider_metadata: ProviderMetadata,
}

#[derive(Clone)]
struct ProviderSource {
    name: String,
    path: PathBuf,
}

impl ProviderSource {
    fn new(name: &str, path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!("provider payload path {path:?} does not exist"));
        }
        if !path.is_file() {
            return Err(format!(
                "provider payload path {path:?} is not a regular file (expected a payload snapshot)"
            ));
        }
        Ok(Self {
            name: name.to_string(),
            path: path.to_path_buf(),
        })
    }

    fn fetch_chunk(&self, spec: &ChunkFetchSpec) -> Result<ChunkResponse, FetchIoError> {
        let mut file = fs::File::open(&self.path).map_err(FetchIoError::from)?;
        use std::io::{Read, Seek, SeekFrom};
        file.seek(SeekFrom::Start(spec.offset))
            .map_err(FetchIoError::from)?;
        let mut buf = vec![0u8; spec.length as usize];
        file.read_exact(&mut buf).map_err(FetchIoError::from)?;
        Ok(ChunkResponse::new(buf))
    }
}

#[derive(Debug)]
struct FetchIoError(String);

impl FetchIoError {
    fn new(msg: &str) -> Self {
        Self(msg.to_string())
    }
}

impl From<std::io::Error> for FetchIoError {
    fn from(err: std::io::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<GatewayFetchError> for FetchIoError {
    fn from(err: GatewayFetchError) -> Self {
        Self(err.to_string())
    }
}

impl std::fmt::Display for FetchIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FetchIoError {}

#[derive(Debug)]
struct CliScorePolicy {
    deny: HashSet<String>,
    boosts: HashMap<String, i64>,
}

impl CliScorePolicy {
    fn new(deny: HashSet<String>, boosts: HashMap<String, i64>) -> Self {
        Self { deny, boosts }
    }
}

impl ScorePolicy for CliScorePolicy {
    fn score(&self, ctx: ProviderScoreContext<'_>) -> ProviderScoreDecision {
        let provider_id = ctx.provider.id().as_str();
        if self.deny.contains(provider_id) {
            return ProviderScoreDecision {
                priority_delta: 0,
                allow: false,
            };
        }
        let delta = self.boosts.get(provider_id).copied().unwrap_or(0);
        ProviderScoreDecision {
            priority_delta: delta,
            allow: true,
        }
    }
}

fn load_telemetry(source: Option<JsonSource>) -> Result<TelemetrySnapshot, String> {
    let Some(source) = source else {
        return Ok(TelemetrySnapshot::default());
    };
    let bytes = match source {
        JsonSource::File(path) => fs::read(&path)
            .map_err(|err| format!("failed to read telemetry file {path:?}: {err}"))?,
        JsonSource::Stdin => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|err| format!("failed to read telemetry from stdin: {err}"))?;
            buf
        }
    };
    if bytes.is_empty() {
        return Ok(TelemetrySnapshot::default());
    }
    let value: Value =
        from_slice(&bytes).map_err(|err| format!("failed to parse telemetry JSON: {err}"))?;
    telemetry_from_value(value)
}

fn telemetry_from_value(value: Value) -> Result<TelemetrySnapshot, String> {
    match value {
        Value::Null => Ok(TelemetrySnapshot::default()),
        Value::Array(entries) => telemetry_from_entries(entries),
        Value::Object(mut map) => {
            if let Some(providers) = map.remove("providers") {
                match providers {
                    Value::Array(entries) => telemetry_from_entries(entries),
                    other => Err(format!(
                        "telemetry 'providers' field must be an array, got {other:?}"
                    )),
                }
            } else {
                Err("telemetry JSON object must contain a 'providers' array".into())
            }
        }
        other => Err(format!(
            "telemetry JSON must be an array or object, got {other:?}"
        )),
    }
}

fn telemetry_from_entries(entries: Vec<Value>) -> Result<TelemetrySnapshot, String> {
    let mut records = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.into_iter().enumerate() {
        let map = match entry {
            Value::Object(map) => map,
            other => {
                return Err(format!(
                    "telemetry entry {idx} must be an object, got {other:?}"
                ));
            }
        };
        let provider_id = match map.get("provider_id") {
            Some(Value::String(id)) if !id.is_empty() => id.clone(),
            _ => {
                return Err(format!(
                    "telemetry entry {idx} missing string 'provider_id' field"
                ));
            }
        };
        let mut record = ProviderTelemetry::new(provider_id);
        if let Some(value) = map.get("qos_score") {
            record.qos_score = Some(parse_f64(value, "qos_score", idx)?);
        }
        if let Some(value) = map.get("latency_p95_ms") {
            record.latency_p95_ms = Some(parse_f64(value, "latency_p95_ms", idx)?);
        }
        if let Some(value) = map.get("failure_rate_ewma") {
            record.failure_rate_ewma = Some(parse_f64(value, "failure_rate_ewma", idx)?);
        }
        if let Some(value) = map.get("token_health") {
            record.token_health = Some(parse_f64(value, "token_health", idx)?);
        }
        if let Some(value) = map.get("staking_weight") {
            record.staking_weight = Some(parse_f64(value, "staking_weight", idx)?);
        }
        if let Some(value) = map.get("penalty") {
            record.penalty = parse_bool(value, "penalty", idx)?;
        }
        if let Some(value) = map.get("last_updated_unix") {
            record.last_updated_unix = Some(parse_u64(value, "last_updated_unix", idx)?);
        }
        records.push(record);
    }
    Ok(TelemetrySnapshot::from_records(records))
}

fn parse_f64(value: &Value, field: &str, idx: usize) -> Result<f64, String> {
    match value {
        Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| format!("telemetry entry {idx} field '{field}' is not a finite number")),
        Value::String(text) => text.parse::<f64>().map_err(|err| {
            format!("telemetry entry {idx} field '{field}' could not be parsed as number: {err}")
        }),
        other => Err(format!(
            "telemetry entry {idx} field '{field}' must be a number, got {other:?}"
        )),
    }
}

fn parse_bool(value: &Value, field: &str, idx: usize) -> Result<bool, String> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        other => Err(format!(
            "telemetry entry {idx} field '{field}' must be a boolean, got {other:?}"
        )),
    }
}

fn parse_u64(value: &Value, field: &str, idx: usize) -> Result<u64, String> {
    match value {
        Value::Number(num) => num.as_u64().ok_or_else(|| {
            format!(
                "telemetry entry {idx} field '{field}' must be an unsigned integer, got {num:?}"
            )
        }),
        Value::String(text) => text.parse::<u64>().map_err(|err| {
            format!(
                "telemetry entry {idx} field '{field}' could not be parsed as unsigned integer: {err}"
            )
        }),
        other => Err(format!(
            "telemetry entry {idx} field '{field}' must be an unsigned integer, got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        env,
        path::{Path, PathBuf},
        sync::Arc,
    };

    use assert_cmd::Command as AssertCommand;
    use ed25519_dalek::{Signer, SigningKey};
    use norito::to_bytes;
    use sorafs_car::{
        CarWriter,
        multi_fetch::{ChunkReceipt, FetchOutcome, FetchProvider, ProviderId, ProviderReport},
    };
    use sorafs_manifest::{
        AdvertEndpoint, AdvertSignature, CapabilityTlv, DagCodecId, EndpointKind, EndpointMetadata,
        EndpointMetadataKey, GovernanceProofs, ManifestBuilder, PROVIDER_ADVERT_VERSION_V1,
        PathDiversityPolicy, PinPolicy, ProviderAdvertBodyV1, ProviderCapabilityRangeV1, QosHints,
        RendezvousTopic, SignatureAlgorithm, StakePointer, StorageClass, StreamBudgetV1,
        TransportHintV1, TransportProtocol, hybrid_envelope::HybridKemBundleV1,
        provider_advert::ProviderCapabilitySoranetPqV1,
    };
    use tempfile::{NamedTempFile, tempdir};

    use super::*;

    fn cargo_bin_path(bin_name: &str) -> PathBuf {
        let env_var = format!("CARGO_BIN_EXE_{bin_name}");
        if let Some(path) = env::var_os(env_var) {
            return PathBuf::from(path);
        }
        let mut path = env::current_exe().expect("current exe path should be available");
        path.pop();
        if path.ends_with("deps") {
            path.pop();
        }
        path.join(format!("{bin_name}{}", env::consts::EXE_SUFFIX))
    }

    fn sorafs_fetch_cmd() -> AssertCommand {
        AssertCommand::new(cargo_bin_path("sorafs_fetch"))
    }

    fn range_capability_payload() -> Vec<u8> {
        let profile = ChunkProfile::DEFAULT;
        ProviderCapabilityRangeV1 {
            max_chunk_span: profile.max_size as u32,
            min_granularity: profile.min_size as u32,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        }
        .to_bytes()
        .expect("encode range capability")
    }

    fn sample_stream_budget() -> StreamBudgetV1 {
        StreamBudgetV1 {
            max_in_flight: 4,
            max_bytes_per_sec: 5_000_000,
            burst_bytes: Some(2_500_000),
        }
    }

    fn default_profile_aliases() -> Vec<String> {
        chunker_registry::default_descriptor()
            .aliases
            .iter()
            .map(|alias| alias.to_string())
            .collect()
    }

    fn sample_endpoint() -> AdvertEndpoint {
        AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "torii.example.org".into(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Region,
                value: b"global".to_vec(),
            }],
        }
    }

    fn sample_rendezvous_topics(label: &str) -> Vec<RendezvousTopic> {
        vec![RendezvousTopic {
            topic: format!("sorafs.{label}.primary"),
            region: "global".into(),
        }]
    }

    fn sample_transport_hints() -> Vec<TransportHintV1> {
        vec![TransportHintV1 {
            protocol: TransportProtocol::ToriiHttpRange,
            priority: 0,
        }]
    }

    fn sample_gateway_manifest_envelope() -> HybridPayloadEnvelopeV1 {
        HybridPayloadEnvelopeV1 {
            version: HYBRID_PAYLOAD_ENVELOPE_VERSION_V1,
            suite: HybridSuite::X25519MlKem768ChaCha20Poly1305.to_string(),
            kem: HybridKemBundleV1 {
                ephemeral_public: vec![7, 8, 9],
                kyber_ciphertext: vec![10, 11, 12],
            },
            nonce: [1_u8; 12],
            ciphertext: vec![42; 16],
        }
    }

    fn encode_gateway_manifest_envelope(envelope: &HybridPayloadEnvelopeV1) -> String {
        let bytes = to_bytes(envelope).expect("encode envelope");
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn sample_gateway_manifest_envelope_b64() -> String {
        encode_gateway_manifest_envelope(&sample_gateway_manifest_envelope())
    }

    #[test]
    fn runtime_provider_counts_reports_direct_and_gateway_lengths() {
        let mut registry = HashMap::new();
        let provider_file = NamedTempFile::new().expect("temp provider payload");
        registry.insert(
            "alpha".to_string(),
            ProviderRuntime::Local(
                ProviderSource::new("alpha", provider_file.path())
                    .expect("provider source should be constructed"),
            ),
        );
        registry.insert("gw-beta".to_string(), ProviderRuntime::Gateway);

        let (provider_count, gateway_count) = runtime_provider_counts(&registry);

        assert_eq!(provider_count, 1);
        assert_eq!(gateway_count, 1);
    }

    #[test]
    fn runtime_provider_counts_handles_gateway_only_runs() {
        let mut registry = HashMap::new();
        registry.insert("gw-alpha".to_string(), ProviderRuntime::Gateway);

        let (provider_count, gateway_count) = runtime_provider_counts(&registry);

        assert_eq!(provider_count, 0);
        assert_eq!(gateway_count, 1);
    }

    #[test]
    fn provider_mix_labels_gateway_only_runs() {
        assert_eq!(provider_mix_label(0, 2), "gateway-only");
    }

    #[test]
    fn provider_mix_labels_mixed_runs() {
        assert_eq!(provider_mix_label(2, 2), "mixed");
    }

    #[test]
    fn report_records_manifest_identifiers_when_present() {
        let provider = Arc::new(FetchProvider::new("did:sora:test"));
        let outcome = FetchOutcome {
            chunks: vec![vec![1, 2, 3, 4]],
            chunk_receipts: vec![ChunkReceipt {
                chunk_index: 0,
                provider: ProviderId::new("did:sora:test"),
                attempts: 1,
                latency_ms: 9.5,
                bytes: 4,
            }],
            provider_reports: vec![ProviderReport {
                provider: Arc::clone(&provider),
                successes: 1,
                failures: 0,
                disabled: false,
            }],
        };
        let digest = [0_u8; 32];
        let transport_labels = transport_policy_labels(
            Some(TransportPolicy::SoranetPreferred),
            Some(TransportPolicy::DirectOnly),
        );
        let report = build_report(ReportContext {
            outcome: &outcome,
            payload_len: 4,
            digest: &digest,
            car_stats: None,
            car_verification: None,
            provider_count: 0,
            gateway_provider_count: 1,
            provider_mix: "gateway-only",
            manifest_id: Some("feedface"),
            manifest_cid: Some("c0ffee"),
            gateway_manifest_provided: true,
            telemetry_label: None,
            telemetry_region: None,
            transport_labels,
        });
        let map = report.as_object().expect("report object");
        assert_eq!(
            map.get("manifest_id").and_then(Value::as_str),
            Some("feedface")
        );
        assert_eq!(
            map.get("manifest_cid").and_then(Value::as_str),
            Some("c0ffee")
        );
        assert_eq!(
            map.get("gateway_manifest_provided")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("transport_policy").and_then(Value::as_str),
            Some("direct-only")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("transport_policy_override_label")
                .and_then(Value::as_str),
            Some("direct-only")
        );
    }

    #[test]
    fn report_omits_manifest_identifiers_when_absent() {
        let provider = Arc::new(FetchProvider::new("did:sora:test"));
        let outcome = FetchOutcome {
            chunks: vec![vec![1, 2, 3, 4]],
            chunk_receipts: vec![ChunkReceipt {
                chunk_index: 0,
                provider: ProviderId::new("did:sora:test"),
                attempts: 1,
                latency_ms: 9.5,
                bytes: 4,
            }],
            provider_reports: vec![ProviderReport {
                provider: Arc::clone(&provider),
                successes: 1,
                failures: 0,
                disabled: false,
            }],
        };
        let digest = [0_u8; 32];
        let transport_labels =
            transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
        let report = build_report(ReportContext {
            outcome: &outcome,
            payload_len: 4,
            digest: &digest,
            car_stats: None,
            car_verification: None,
            provider_count: 1,
            gateway_provider_count: 0,
            provider_mix: "direct-only",
            manifest_id: None,
            manifest_cid: None,
            gateway_manifest_provided: false,
            telemetry_label: None,
            telemetry_region: None,
            transport_labels,
        });
        let map = report.as_object().expect("report object");
        assert!(map.get("manifest_id").is_none());
        assert!(map.get("manifest_cid").is_none());
        assert_eq!(
            map.get("gateway_manifest_provided")
                .and_then(Value::as_bool),
            Some(false)
        );
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
                .is_none_or(Value::is_null)
        );
    }

    #[test]
    fn report_records_telemetry_label_when_present() {
        let provider = Arc::new(FetchProvider::new("did:sora:test"));
        let outcome = FetchOutcome {
            chunks: vec![vec![1, 2, 3, 4]],
            chunk_receipts: vec![ChunkReceipt {
                chunk_index: 0,
                provider: ProviderId::new("did:sora:test"),
                attempts: 1,
                latency_ms: 2.0,
                bytes: 4,
            }],
            provider_reports: vec![ProviderReport {
                provider: Arc::clone(&provider),
                successes: 1,
                failures: 0,
                disabled: false,
            }],
        };
        let digest = [0_u8; 32];
        let transport_labels =
            transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
        let report = build_report(ReportContext {
            outcome: &outcome,
            payload_len: 4,
            digest: &digest,
            car_stats: None,
            car_verification: None,
            provider_count: 1,
            gateway_provider_count: 0,
            provider_mix: "direct-only",
            manifest_id: None,
            manifest_cid: None,
            gateway_manifest_provided: false,
            telemetry_label: Some("otel::ci"),
            telemetry_region: None,
            transport_labels,
        });
        let map = report.as_object().expect("report object");
        assert_eq!(
            map.get("telemetry_source").and_then(Value::as_str),
            Some("otel::ci")
        );
    }

    #[test]
    fn report_records_telemetry_region_when_present() {
        let provider = Arc::new(FetchProvider::new("did:sora:test"));
        let outcome = FetchOutcome {
            chunks: vec![vec![1, 2, 3, 4]],
            chunk_receipts: vec![ChunkReceipt {
                chunk_index: 0,
                provider: ProviderId::new("did:sora:test"),
                attempts: 1,
                latency_ms: 2.0,
                bytes: 4,
            }],
            provider_reports: vec![ProviderReport {
                provider: Arc::clone(&provider),
                successes: 1,
                failures: 0,
                disabled: false,
            }],
        };
        let digest = [0_u8; 32];
        let transport_labels =
            transport_policy_labels(Some(TransportPolicy::SoranetPreferred), None);
        let report = build_report(ReportContext {
            outcome: &outcome,
            payload_len: 4,
            digest: &digest,
            car_stats: None,
            car_verification: None,
            provider_count: 1,
            gateway_provider_count: 0,
            provider_mix: "direct-only",
            manifest_id: None,
            manifest_cid: None,
            gateway_manifest_provided: false,
            telemetry_label: None,
            telemetry_region: Some("regulated-eu"),
            transport_labels,
        });
        let map = report.as_object().expect("report object");
        assert_eq!(
            map.get("telemetry_region").and_then(Value::as_str),
            Some("regulated-eu")
        );
    }

    #[test]
    fn scoreboard_metadata_records_manifest_envelope_presence() {
        let provider_count = 0;
        let gateway_count = 1;
        let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
            scoreboard_mode: true,
            allow_implicit_metadata: false,
            provider_count,
            gateway_provider_count: gateway_count,
            provider_mix: provider_mix_label(provider_count, gateway_count),
            max_parallel: None,
            max_peers: None,
            retry_budget: None,
            failure_threshold: None,
            assume_now: Some(1_700_000_000),
            telemetry_label: Some("test-source"),
            telemetry_region: Some("iad-prod"),
            gateway_manifest_id: Some("feedface"),
            gateway_manifest_cid: Some("c0ffee"),
            gateway_manifest_envelope_present: true,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: None,
            anonymity_policy_override: None,
        });
        let map = metadata.as_object().expect("metadata object");
        assert_eq!(
            map.get("gateway_manifest_provided")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("telemetry_source").and_then(Value::as_str),
            Some("test-source")
        );
        assert_eq!(
            map.get("telemetry_region").and_then(Value::as_str),
            Some("iad-prod")
        );
        assert_eq!(
            map.get("gateway_manifest_id").and_then(Value::as_str),
            Some("feedface")
        );
        assert_eq!(
            map.get("gateway_manifest_cid").and_then(Value::as_str),
            Some("c0ffee")
        );
    }

    #[test]
    fn scoreboard_metadata_marks_missing_manifest_envelope() {
        let provider_count = 0;
        let gateway_count = 0;
        let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
            scoreboard_mode: true,
            allow_implicit_metadata: false,
            provider_count,
            gateway_provider_count: gateway_count,
            provider_mix: provider_mix_label(provider_count, gateway_count),
            max_parallel: None,
            max_peers: None,
            retry_budget: None,
            failure_threshold: None,
            assume_now: None,
            telemetry_label: None,
            telemetry_region: None,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            gateway_manifest_envelope_present: false,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: None,
            anonymity_policy_override: None,
        });
        let map = metadata.as_object().expect("metadata object");
        assert_eq!(
            map.get("gateway_manifest_provided")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(map.get("gateway_manifest_id").is_none_or(Value::is_null));
        assert!(map.get("gateway_manifest_cid").is_none_or(Value::is_null));
    }

    #[test]
    fn scoreboard_metadata_records_transport_policy_labels() {
        let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
            scoreboard_mode: true,
            allow_implicit_metadata: false,
            provider_count: 1,
            gateway_provider_count: 1,
            provider_mix: provider_mix_label(1, 1),
            max_parallel: None,
            max_peers: None,
            retry_budget: None,
            failure_threshold: None,
            assume_now: None,
            telemetry_label: None,
            telemetry_region: None,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            gateway_manifest_envelope_present: false,
            transport_policy: Some(TransportPolicy::SoranetStrict),
            transport_policy_override: None,
            anonymity_policy: None,
            anonymity_policy_override: None,
        });
        let map = metadata.as_object().expect("metadata object");
        assert_eq!(
            map.get("transport_policy").and_then(Value::as_str),
            Some("soranet-strict")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            map.get("transport_policy_override_label")
                .is_none_or(Value::is_null)
        );

        let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
            scoreboard_mode: true,
            allow_implicit_metadata: false,
            provider_count: 1,
            gateway_provider_count: 1,
            provider_mix: provider_mix_label(1, 1),
            max_parallel: None,
            max_peers: None,
            retry_budget: None,
            failure_threshold: None,
            assume_now: None,
            telemetry_label: None,
            telemetry_region: None,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            gateway_manifest_envelope_present: false,
            transport_policy: Some(TransportPolicy::SoranetPreferred),
            transport_policy_override: Some(TransportPolicy::DirectOnly),
            anonymity_policy: None,
            anonymity_policy_override: None,
        });
        let map = metadata.as_object().expect("metadata object");
        assert_eq!(
            map.get("transport_policy").and_then(Value::as_str),
            Some("direct-only")
        );
        assert_eq!(
            map.get("transport_policy_override")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            map.get("transport_policy_override_label")
                .and_then(Value::as_str),
            Some("direct-only")
        );
    }

    #[test]
    fn scoreboard_metadata_records_anonymity_policy_labels() {
        let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
            scoreboard_mode: true,
            allow_implicit_metadata: false,
            provider_count: 1,
            gateway_provider_count: 1,
            provider_mix: provider_mix_label(1, 1),
            max_parallel: None,
            max_peers: None,
            retry_budget: None,
            failure_threshold: None,
            assume_now: None,
            telemetry_label: None,
            telemetry_region: None,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            gateway_manifest_envelope_present: false,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: Some(AnonymityPolicy::GuardPq),
            anonymity_policy_override: None,
        });
        let map = metadata.as_object().expect("metadata object");
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
                .is_none_or(Value::is_null)
        );

        let metadata = build_scoreboard_metadata(ScoreboardMetadataOptions {
            scoreboard_mode: true,
            allow_implicit_metadata: false,
            provider_count: 1,
            gateway_provider_count: 1,
            provider_mix: provider_mix_label(1, 1),
            max_parallel: None,
            max_peers: None,
            retry_budget: None,
            failure_threshold: None,
            assume_now: None,
            telemetry_label: None,
            telemetry_region: None,
            gateway_manifest_id: None,
            gateway_manifest_cid: None,
            gateway_manifest_envelope_present: false,
            transport_policy: None,
            transport_policy_override: None,
            anonymity_policy: Some(AnonymityPolicy::MajorityPq),
            anonymity_policy_override: Some(AnonymityPolicy::StrictPq),
        });
        let map = metadata.as_object().expect("metadata object");
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
    }

    #[test]
    fn gateway_manifest_flag_requires_gateway_providers() {
        let envelope = sample_gateway_manifest_envelope_b64();
        assert!(
            !gateway_manifest_present(&Some(envelope.clone()), false),
            "presence flag should be false when no gateway providers are active"
        );
        assert!(gateway_manifest_present(&Some(envelope), true));
    }

    #[test]
    fn gateway_manifest_flag_rejects_unknown_suite() {
        let mut envelope = sample_gateway_manifest_envelope();
        envelope.suite = "unknown-suite".to_string();
        let encoded = encode_gateway_manifest_envelope(&envelope);
        assert!(!gateway_manifest_present(&Some(encoded), true));
    }

    #[test]
    fn gateway_manifest_flag_ignores_blank_values() {
        assert!(!gateway_manifest_present(&Some("   ".to_string()), true));
        assert!(!gateway_manifest_present(&None, true));
    }

    #[test]
    fn gateway_manifest_flag_rejects_non_norito_payloads() {
        let bogus = base64::engine::general_purpose::STANDARD.encode(b"not-an-envelope");
        assert!(!gateway_manifest_present(&Some(bogus), true));
    }

    #[test]
    fn gateway_manifest_flag_rejects_invalid_base64() {
        assert!(!gateway_manifest_present(
            &Some("not-base64!?".to_string()),
            true
        ));
    }

    #[test]
    fn classify_scoreboard_aliases_maps_provider_ids() {
        let eligible_entry = scoreboard::ScoreboardEntry {
            normalised_weight: 0.0,
            raw_score: 0.0,
            provider: FetchProvider::new("did:sora:alpha"),
            eligibility: scoreboard::Eligibility::Eligible,
        };
        let ineligible_entry = scoreboard::ScoreboardEntry {
            normalised_weight: 0.0,
            raw_score: 0.0,
            provider: FetchProvider::new("did:sora:beta"),
            eligibility: scoreboard::Eligibility::Ineligible(
                scoreboard::IneligibilityReason::TelemetryPenalty,
            ),
        };
        let aliases = vec!["alpha".to_string(), "beta".to_string()];

        let (eligible, lookup, ineligible) =
            classify_scoreboard_aliases(&[eligible_entry, ineligible_entry], &aliases);

        assert!(eligible.contains("alpha"));
        assert!(!eligible.contains("beta"));
        assert_eq!(lookup.get("alpha").map(String::as_str), Some("alpha"));
        assert_eq!(
            lookup.get("did:sora:alpha").map(String::as_str),
            Some("alpha")
        );
        assert!(!lookup.contains_key("beta"));
        assert_eq!(ineligible.len(), 1);
        assert_eq!(ineligible[0].0, "beta");
        assert_eq!(
            ineligible[0].1,
            scoreboard::IneligibilityReason::TelemetryPenalty
        );
    }

    fn write_payload(path: &Path, size: usize) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        for (idx, byte) in buf.iter_mut().enumerate() {
            *byte = (idx as u8).wrapping_mul(31).wrapping_add(7);
        }
        fs::write(path, &buf).expect("write payload");
        buf
    }

    #[test]
    fn parse_provider_with_concurrency_and_weight() {
        let spec = parse_provider_spec("alpha=/tmp/payload#4@3").expect("parse");
        assert_eq!(spec.name, "alpha");
        assert_eq!(spec.path, PathBuf::from("/tmp/payload"));
        assert_eq!(spec.max_concurrent.get(), 4);
        assert_eq!(spec.weight.unwrap().get(), 3);
        assert!(spec.concurrency_explicit);
        assert!(spec.weight_explicit);
    }

    #[test]
    fn parse_provider_with_weight_only() {
        let spec = parse_provider_spec("beta=/data/payload@5").expect("parse");
        assert_eq!(spec.max_concurrent.get(), 2);
        assert_eq!(spec.weight.unwrap().get(), 5);
        assert!(!spec.concurrency_explicit);
        assert!(spec.weight_explicit);
    }

    #[test]
    fn parse_provider_with_concurrency_only() {
        let spec = parse_provider_spec("gamma=/srv/payload#6").expect("parse");
        assert_eq!(spec.max_concurrent.get(), 6);
        assert_eq!(spec.weight.unwrap().get(), 1);
        assert!(spec.concurrency_explicit);
        assert!(!spec.weight_explicit);
    }

    #[test]
    fn parse_provider_defaults_when_flags_omitted() {
        let spec = parse_provider_spec("delta=/srv/payload").expect("parse");
        assert_eq!(spec.max_concurrent.get(), 2);
        assert_eq!(spec.weight.unwrap().get(), 1);
        assert!(!spec.concurrency_explicit);
        assert!(!spec.weight_explicit);
    }

    #[test]
    fn fetch_cli_applies_provider_advert() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 8 * 1024);

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let advert_path = tempdir.path().join("provider.advert");
        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let now = unix_time_now().unwrap_or(1_700_000_000);
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0x11; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
            stake: StakePointer {
                pool_id: [0x22; 32],
                stake_amount: 1_000_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 500,
                max_concurrent_streams: 5,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "localhost".into(),
                metadata: vec![EndpointMetadata {
                    key: EndpointMetadataKey::Alpn,
                    value: b"h2".to_vec(),
                }],
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: Some("test provider".into()),
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let signing_key = SigningKey::from_bytes(&[0xAB; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + 3_600,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let output_path = tempdir.path().join("assembled.bin");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .arg(format!("--output={}", output_path.display()))
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
        let report: Value = norito::json::from_str(&stdout).expect("parse report");
        let provider_reports = report
            .get("provider_reports")
            .and_then(Value::as_array)
            .expect("provider reports array");
        let provider = provider_reports.first().expect("provider entry");
        let metadata = provider
            .get("metadata")
            .and_then(Value::as_object)
            .expect("metadata present");
        assert_eq!(
            metadata
                .get("availability")
                .and_then(Value::as_str)
                .expect("availability"),
            "hot"
        );
        assert_eq!(
            metadata
                .get("max_streams")
                .and_then(Value::as_u64)
                .expect("max_streams") as u16,
            5
        );
        assert_eq!(
            metadata
                .get("stake_amount")
                .and_then(Value::as_str)
                .expect("stake_amount"),
            "1000000"
        );
        assert!(
            metadata
                .get("rendezvous_topics")
                .and_then(Value::as_array)
                .is_some_and(|topics| !topics.is_empty())
        );
        assert_eq!(
            metadata
                .get("allow_unknown_capabilities")
                .and_then(Value::as_bool),
            Some(false)
        );
        let capabilities = metadata
            .get("capabilities")
            .and_then(Value::as_array)
            .expect("capabilities present");
        let capability_names: Vec<&str> = capabilities.iter().filter_map(Value::as_str).collect();
        assert!(capability_names.contains(&"chunk_range_fetch"));
        let aliases = metadata
            .get("profile_aliases")
            .and_then(Value::as_array)
            .expect("profile_aliases present");
        let alias_strings: Vec<&str> = aliases.iter().filter_map(Value::as_str).collect();
        assert!(alias_strings.contains(&profile_handle.as_str()));
        assert_eq!(
            metadata.get("refresh_deadline").and_then(Value::as_u64),
            Some(now + 1_800)
        );
        let range = metadata
            .get("range_capability")
            .and_then(Value::as_object)
            .expect("range capability present");
        let profile = ChunkProfile::DEFAULT;
        assert_eq!(
            range.get("max_chunk_span").and_then(Value::as_u64),
            Some(profile.max_size as u64)
        );
        assert_eq!(
            range.get("min_granularity").and_then(Value::as_u64),
            Some(profile.min_size as u64)
        );
        let stream_budget = metadata
            .get("stream_budget")
            .and_then(Value::as_object)
            .expect("stream budget present");
        assert_eq!(
            stream_budget.get("max_in_flight").and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            stream_budget
                .get("max_bytes_per_sec")
                .and_then(Value::as_u64),
            Some(5_000_000)
        );
        let transport_hints = metadata
            .get("transport_hints")
            .and_then(Value::as_array)
            .expect("transport hints present");
        assert_eq!(transport_hints.len(), 1);
        let hint = transport_hints[0]
            .as_object()
            .expect("transport hint object");
        assert_eq!(
            hint.get("protocol").and_then(Value::as_str),
            Some("torii_http_range")
        );
        assert_eq!(hint.get("priority").and_then(Value::as_u64), Some(0));

        let assembled = fs::read(&output_path).expect("read payload");
        assert_eq!(assembled, payload);
    }

    #[test]
    fn fetch_cli_persists_scoreboard() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 8 * 1024);

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let advert_path = tempdir.path().join("provider.advert");
        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let profile_aliases = default_profile_aliases();
        let now = unix_time_now().unwrap_or(1_700_000_000);
        let provider_id = [0x44; 32];
        let advert_body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id: profile_handle.clone(),
            profile_aliases: Some(profile_aliases.clone()),
            stake: StakePointer {
                pool_id: [0x55; 32],
                stake_amount: 750_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 400,
                max_concurrent_streams: 4,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![sample_endpoint()],
            rendezvous_topics: sample_rendezvous_topics("alpha"),
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: Some("scoreboard integration".into()),
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let signing_key = SigningKey::from_bytes(&[0x9F; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + 3_600,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let telemetry_path = tempdir.path().join("telemetry.json");
        let mut telemetry_entry = Map::new();
        telemetry_entry.insert("provider_id".into(), Value::String(to_hex(&provider_id)));
        telemetry_entry.insert("qos_score".into(), Value::from(92.0));
        telemetry_entry.insert("latency_p95_ms".into(), Value::from(180.0));
        telemetry_entry.insert("failure_rate_ewma".into(), Value::from(0.03));
        telemetry_entry.insert("token_health".into(), Value::from(0.96));
        telemetry_entry.insert("staking_weight".into(), Value::from(1.05));
        telemetry_entry.insert("last_updated_unix".into(), Value::from(now));
        let telemetry = Value::Array(vec![Value::Object(telemetry_entry)]);
        fs::write(
            &telemetry_path,
            (norito::json::to_string_pretty(&telemetry).expect("telemetry json") + "\n").as_bytes(),
        )
        .expect("write telemetry");

        let scoreboard_path = tempdir.path().join("scoreboard.json");
        let output_path = tempdir.path().join("assembled.bin");

        sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .arg(format!("--output={}", output_path.display()))
            .arg(format!("--telemetry-json={}", telemetry_path.display()))
            .arg(format!("--scoreboard-out={}", scoreboard_path.display()))
            .arg("--use-scoreboard")
            .assert()
            .success();

        let scoreboard_contents = fs::read_to_string(&scoreboard_path).expect("read scoreboard");
        let scoreboard_value: Value =
            norito::json::from_str(&scoreboard_contents).expect("parse scoreboard");
        let entries = scoreboard_value
            .get("entries")
            .and_then(Value::as_array)
            .expect("entries array");
        assert_eq!(entries.len(), 1);
        let entry = entries[0].as_object().expect("scoreboard entry object");
        assert_eq!(
            entry
                .get("provider_id")
                .and_then(Value::as_str)
                .expect("provider id"),
            "alpha"
        );

        let assembled = fs::read(&output_path).expect("read assembled payload");
        assert_eq!(assembled, payload);
    }

    #[test]
    fn fetch_cli_score_policy_filters_providers() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path_alpha = tempdir.path().join("alpha.bin");
        let payload = write_payload(&payload_path_alpha, 8 * 1024);
        let payload_path_beta = tempdir.path().join("beta.bin");
        fs::write(&payload_path_beta, &payload).expect("write beta payload");

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let profile_aliases = default_profile_aliases();
        let now = unix_time_now().unwrap_or(1_700_000_000);

        let advert_alpha = ProviderAdvertBodyV1 {
            provider_id: [0x66; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(profile_aliases.clone()),
            stake: StakePointer {
                pool_id: [0x01; 32],
                stake_amount: 600_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 400,
                max_concurrent_streams: 3,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![sample_endpoint()],
            rendezvous_topics: sample_rendezvous_topics("alpha"),
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: Some("alpha provider".into()),
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let advert_beta = ProviderAdvertBodyV1 {
            provider_id: [0x77; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(profile_aliases.clone()),
            stake: StakePointer {
                pool_id: [0x02; 32],
                stake_amount: 900_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 350,
                max_concurrent_streams: 4,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![sample_endpoint()],
            rendezvous_topics: sample_rendezvous_topics("beta"),
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: Some("beta provider".into()),
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };

        let advert_path_alpha = tempdir.path().join("alpha.advert");
        let advert_path_beta = tempdir.path().join("beta.advert");
        for (body, path, key_byte) in [
            (advert_alpha, &advert_path_alpha, 0xA1u8),
            (advert_beta, &advert_path_beta, 0xB2u8),
        ] {
            let signing_key = SigningKey::from_bytes(&[key_byte; 32]);
            let body_bytes = to_bytes(&body).expect("serialize body");
            let signature = signing_key.sign(&body_bytes);
            let advert = ProviderAdvertV1 {
                version: PROVIDER_ADVERT_VERSION_V1,
                issued_at: now,
                expires_at: now + 3_600,
                body,
                signature: AdvertSignature {
                    algorithm: SignatureAlgorithm::Ed25519,
                    public_key: signing_key.verifying_key().to_bytes().to_vec(),
                    signature: signature.to_bytes().to_vec(),
                },
                signature_strict: true,
                allow_unknown_capabilities: false,
            };
            let advert_bytes = to_bytes(&advert).expect("serialize advert");
            fs::write(path, advert_bytes).expect("write advert");
        }

        let metrics_path = tempdir.path().join("providers.json");
        let output_path = tempdir.path().join("assembled.bin");

        sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path_alpha.display()))
            .arg(format!("--provider=beta={}", payload_path_beta.display()))
            .arg(format!(
                "--provider-advert=alpha={}",
                advert_path_alpha.display()
            ))
            .arg(format!(
                "--provider-advert=beta={}",
                advert_path_beta.display()
            ))
            .arg(format!("--provider-metrics-out={}", metrics_path.display()))
            .arg(format!("--output={}", output_path.display()))
            .arg("--deny-provider=alpha")
            .assert()
            .success();

        let metrics = fs::read_to_string(&metrics_path).expect("read provider metrics");
        let metrics_value: Value = norito::json::from_str(&metrics).expect("parse metrics");
        let entries = metrics_value.as_array().expect("provider metrics array");
        let alpha_entry = entries
            .iter()
            .find(|entry| {
                entry
                    .get("provider")
                    .and_then(Value::as_str)
                    .map(|id| id == "alpha")
                    .unwrap_or(false)
            })
            .expect("alpha entry present");
        assert_eq!(
            alpha_entry.get("successes").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(alpha_entry.get("failures").and_then(Value::as_u64), Some(0));
        assert_eq!(
            alpha_entry.get("disabled").and_then(Value::as_bool),
            Some(false)
        );

        let beta_entry = entries
            .iter()
            .find(|entry| {
                entry
                    .get("provider")
                    .and_then(Value::as_str)
                    .map(|id| id == "beta")
                    .unwrap_or(false)
            })
            .expect("beta entry present");
        let beta_successes = beta_entry
            .get("successes")
            .and_then(Value::as_u64)
            .expect("beta successes");
        assert!(beta_successes > 0);

        let assembled = fs::read(&output_path).expect("read assembled payload");
        assert_eq!(assembled, payload);
    }

    #[test]
    fn fetch_cli_rejects_provider_without_range_capability() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 4 * 1024);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let now = unix_time_now().unwrap_or(1_700_000_000);
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0xAA; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
            stake: StakePointer {
                pool_id: [0xBB; 32],
                stake_amount: 1_500_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 400,
                max_concurrent_streams: 4,
            },
            capabilities: vec![CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            }],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "localhost".into(),
                metadata: Vec::new(),
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: None,
        };
        let signing_key = SigningKey::from_bytes(&[0x01; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + 3_600,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        let advert_path = tempdir.path().join("provider.advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .assert()
            .failure();

        let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
        assert!(stderr.contains("chunk_range_fetch capability"));
    }

    #[test]
    fn fetch_cli_verifies_car_when_manifest_available() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 8 * 1024);

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car_bytes = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car_bytes)
            .expect("write car");

        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();

        let mut car_digest = [0u8; 32];
        car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
        let manifest = ManifestBuilder::new()
            .root_cid(stats.root_cids[0].clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
            .content_length(plan.content_length)
            .car_digest(car_digest)
            .car_size(stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 0,
            })
            .governance(GovernanceProofs::default())
            .build()
            .expect("manifest");

        let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
        let manifest_hex = to_hex(&manifest_bytes);

        let mut manifest_obj = Map::new();
        manifest_obj.insert("manifest_hex".into(), Value::from(manifest_hex));
        manifest_obj.insert(
            "car_digest_hex".into(),
            Value::from(to_hex(stats.car_archive_digest.as_bytes())),
        );
        manifest_obj.insert("car_size".into(), Value::from(stats.car_size));

        let mut report_obj = Map::new();
        report_obj.insert("chunk_fetch_specs".into(), Value::Array(fetch_array));
        report_obj.insert(
            "payload_digest_hex".into(),
            Value::from(blake3::hash(&payload).to_hex().to_string()),
        );
        report_obj.insert("payload_len".into(), Value::from(payload.len() as u64));
        report_obj.insert("manifest".into(), Value::Object(manifest_obj));

        let manifest_path = tempdir.path().join("report.json");
        fs::write(
            &manifest_path,
            (to_string_pretty(&Value::Object(report_obj)).expect("json") + "\n").as_bytes(),
        )
        .expect("write manifest report");

        let car_path = tempdir.path().join("payload.car");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--manifest-report={}", manifest_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg("--allow-implicit-provider-metadata")
            .arg(format!("--car-out={}", car_path.display()))
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
        let report: Value = norito::json::from_str(&stdout).expect("parse report");
        let car_archive = report
            .get("car_archive")
            .and_then(Value::as_object)
            .expect("car_archive present");
        assert_eq!(
            car_archive.get("verified").and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            car_archive
                .get("por_leaf_count")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
        );
    }

    #[test]
    fn fetch_cli_rejects_corrupted_payload_when_manifest_provided() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 4 * 1024);

        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let mut car_bytes = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("writer")
            .write_to(&mut car_bytes)
            .expect("write car");

        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();

        let mut car_digest = [0u8; 32];
        car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
        let manifest = ManifestBuilder::new()
            .root_cid(stats.root_cids[0].clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
            .content_length(plan.content_length)
            .car_digest(car_digest)
            .car_size(stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 0,
            })
            .governance(GovernanceProofs::default())
            .build()
            .expect("manifest");

        let manifest_bytes = to_bytes(&manifest).expect("manifest bytes");
        let mut manifest_obj = Map::new();
        manifest_obj.insert("manifest_hex".into(), Value::from(to_hex(&manifest_bytes)));
        manifest_obj.insert(
            "car_digest_hex".into(),
            Value::from(to_hex(stats.car_archive_digest.as_bytes())),
        );
        manifest_obj.insert("car_size".into(), Value::from(stats.car_size));

        let mut report_obj = Map::new();
        report_obj.insert("chunk_fetch_specs".into(), Value::Array(fetch_array));
        report_obj.insert(
            "payload_digest_hex".into(),
            Value::from(blake3::hash(&payload).to_hex().to_string()),
        );
        report_obj.insert("payload_len".into(), Value::from(payload.len() as u64));
        report_obj.insert("manifest".into(), Value::Object(manifest_obj));

        let manifest_path = tempdir.path().join("report.json");
        fs::write(
            &manifest_path,
            (to_string_pretty(&Value::Object(report_obj)).expect("json") + "\n").as_bytes(),
        )
        .expect("write manifest report");

        // Corrupt the provider payload after the plan/manifest have been generated.
        let mut corrupted = fs::read(&payload_path).expect("read payload");
        corrupted[0] ^= 0xFF;
        fs::write(&payload_path, &corrupted).expect("rewrite payload");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--manifest-report={}", manifest_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg("--allow-implicit-provider-metadata")
            .assert()
            .failure();

        let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
        let verification_failed = stderr.contains("CAR verification failed")
            || stderr.contains("chunk digest mismatch")
            || stderr.contains("payload length does not match")
            || stderr.contains("retry budget exhausted");
        assert!(
            verification_failed,
            "stderr did not include expected verification failure, got: {stderr}"
        );
    }

    #[test]
    fn provider_advert_concurrency_respects_stream_budget() {
        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0x42; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
            stake: StakePointer {
                pool_id: [0x24; 32],
                stake_amount: 2_000_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Warm,
                max_retrieval_latency_ms: 800,
                max_concurrent_streams: 6,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "storage".into(),
                metadata: vec![],
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 5,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: 0,
            expires_at: 3_600,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0u8; PUBLIC_KEY_LENGTH],
                signature: vec![0u8; SIGNATURE_LENGTH],
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };

        let metadata = provider_advert_to_metadata(advert).expect("metadata");
        assert!(metadata.supports_chunk_range);
        let concurrency = metadata.concurrency.expect("concurrency").get();
        assert_eq!(concurrency, sample_stream_budget().max_in_flight as usize);
        let budget = metadata
            .provider_metadata
            .stream_budget
            .expect("stream budget metadata");
        assert_eq!(budget.max_in_flight, sample_stream_budget().max_in_flight);
        assert_eq!(metadata.provider_metadata.transport_hints.len(), 1);
    }

    #[test]
    fn fetch_cli_rejects_unknown_capabilities_without_allow_flag() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 4 * 1024);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let now = unix_time_now().unwrap_or(1_700_000_000);
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0x21; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
            stake: StakePointer {
                pool_id: [0x31; 32],
                stake_amount: 2_000_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 350,
                max_concurrent_streams: 6,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::VendorReserved,
                    payload: vec![0xFF],
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "localhost".into(),
                metadata: Vec::new(),
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let signing_key = SigningKey::from_bytes(&[0x55; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + 6_000,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        let advert_path = tempdir.path().join("provider.advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .assert()
            .failure();

        let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
        assert!(stderr.contains("unsupported capabilities"));
    }

    #[test]
    fn fetch_cli_ignores_unknown_capabilities_when_allowed() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 4 * 1024);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let now = unix_time_now().unwrap_or(1_700_000_000);
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0x41; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
            stake: StakePointer {
                pool_id: [0x51; 32],
                stake_amount: 2_500_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Warm,
                max_retrieval_latency_ms: 800,
                max_concurrent_streams: 3,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::VendorReserved,
                    payload: vec![0xAA, 0xBB],
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "localhost".into(),
                metadata: Vec::new(),
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 8,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let signing_key = SigningKey::from_bytes(&[0x77; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + 7_200,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: true,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        let advert_path = tempdir.path().join("provider.advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .assert()
            .success();

        let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
        assert!(stderr.contains("advertised unknown capabilities"));

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
        let report: Value = norito::json::from_str(&stdout).expect("parse report");
        let provider_reports = report
            .get("provider_reports")
            .and_then(Value::as_array)
            .expect("provider reports array");
        let provider = provider_reports.first().expect("provider entry");
        let metadata = provider
            .get("metadata")
            .and_then(Value::as_object)
            .expect("metadata present");
        let capabilities = metadata
            .get("capabilities")
            .and_then(Value::as_array)
            .expect("capabilities present");
        let capability_names: Vec<&str> = capabilities.iter().filter_map(Value::as_str).collect();
        assert!(capability_names.contains(&"chunk_range_fetch"));
        assert!(capability_names.contains(&"torii_gateway"));
        assert!(!capability_names.contains(&"vendor_reserved"));
    }

    #[test]
    fn fetch_cli_exposes_soranet_pq_labels() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 4 * 1024);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let profile_aliases = default_profile_aliases();
        let now = unix_time_now().unwrap_or(1_700_000_000);
        let pq_payload = ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: true,
            supports_strict: false,
        }
        .to_bytes()
        .expect("encode soranet_pq");
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0x31; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(profile_aliases.clone()),
            stake: StakePointer {
                pool_id: [0x41; 32],
                stake_amount: 1_500_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 600,
                max_concurrent_streams: 5,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::SoraNetHybridPq,
                    payload: pq_payload,
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "relay.example.com".into(),
                metadata: Vec::new(),
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let signing_key = SigningKey::from_bytes(&[0x66; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + 7_200,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        let advert_path = tempdir.path().join("provider.advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
        let report: Value = norito::json::from_str(&stdout).expect("parse report");
        let provider_reports = report
            .get("provider_reports")
            .and_then(Value::as_array)
            .expect("provider reports array");
        let provider = provider_reports.first().expect("provider entry");
        let metadata = provider
            .get("metadata")
            .and_then(Value::as_object)
            .expect("metadata present");
        let capabilities = metadata
            .get("capabilities")
            .and_then(Value::as_array)
            .expect("capabilities present");
        let mut labels: Vec<&str> = capabilities.iter().filter_map(Value::as_str).collect();
        labels.sort();
        assert!(
            labels.contains(&"soranet_pq"),
            "expected base soranet_pq label, got {labels:?}"
        );
        assert!(
            labels.contains(&"soranet_pq_guard"),
            "expected guard label, got {labels:?}"
        );
        assert!(
            labels.contains(&"soranet_pq_majority"),
            "expected majority label, got {labels:?}"
        );
        assert!(
            !labels.contains(&"soranet_pq_strict"),
            "strict label should be absent, got {labels:?}"
        );
    }

    #[test]
    fn fetch_cli_rejects_stale_provider_advert() {
        let tempdir = tempdir().expect("tempdir");
        let payload_path = tempdir.path().join("payload.bin");
        let payload = write_payload(&payload_path, 4 * 1024);
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let fetch_specs = plan.chunk_fetch_specs();
        let fetch_array: Vec<Value> = fetch_specs
            .iter()
            .map(|spec| {
                let mut obj = norito::json::Map::new();
                obj.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
                obj.insert("offset".into(), Value::from(spec.offset));
                obj.insert("length".into(), Value::from(spec.length as u64));
                obj.insert("digest_blake3".into(), Value::from(to_hex(&spec.digest)));
                Value::Object(obj)
            })
            .collect();
        let plan_path = tempdir.path().join("plan.json");
        fs::write(
            &plan_path,
            (to_string_pretty(&Value::Array(fetch_array)).expect("json") + "\n").as_bytes(),
        )
        .expect("write plan");

        let descriptor = chunker_registry::default_descriptor();
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let base_now = unix_time_now().unwrap_or(1_700_000_000);
        let issued_at = base_now.saturating_sub(13 * 3_600);
        let advert_body = ProviderAdvertBodyV1 {
            provider_id: [0x61; 32],
            profile_id: profile_handle.clone(),
            profile_aliases: Some(vec![profile_handle.clone(), "sorafs-sf1".into()]),
            stake: StakePointer {
                pool_id: [0x71; 32],
                stake_amount: 1_000_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Warm,
                max_retrieval_latency_ms: 900,
                max_concurrent_streams: 3,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: range_capability_payload(),
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "localhost".into(),
                metadata: Vec::new(),
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".into(),
                region: "global".into(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: Some(sample_stream_budget()),
            transport_hints: Some(sample_transport_hints()),
        };
        let signing_key = SigningKey::from_bytes(&[0x91; 32]);
        let body_bytes = to_bytes(&advert_body).expect("serialize body");
        let signature = signing_key.sign(&body_bytes);
        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at: issued_at + 24 * 3_600,
            body: advert_body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        let advert_bytes = to_bytes(&advert).expect("serialize advert");
        let advert_path = tempdir.path().join("provider.advert");
        fs::write(&advert_path, advert_bytes).expect("write advert");

        let assert = sorafs_fetch_cmd()
            .arg(format!("--plan={}", plan_path.display()))
            .arg(format!("--provider=alpha={}", payload_path.display()))
            .arg(format!("--provider-advert=alpha={}", advert_path.display()))
            .assert()
            .failure();

        let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");
        assert!(stderr.contains("is stale"));
    }

    #[test]
    fn ensure_range_capability_detects_max_span_violation() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(16 * 1024).collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let chunk_len = plan
            .chunks
            .first()
            .map(|chunk| chunk.length)
            .expect("chunk length present");
        assert!(chunk_len > 1);

        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len - 1,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: false,
        });

        let err = ensure_range_capability_satisfies_plan(&plan, "alpha", &metadata)
            .expect_err("should fail");
        assert!(
            err.contains("max_chunk_span"),
            "error should mention max_chunk_span, got {err}"
        );
    }

    #[test]
    fn ensure_range_capability_detects_alignment_violation() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(32 * 1024).collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let chunk_len = plan
            .chunks
            .first()
            .map(|chunk| chunk.length)
            .expect("chunk length present");

        let mut metadata = ProviderMetadata::new();
        metadata.range_capability = Some(RangeCapability {
            max_chunk_span: chunk_len.saturating_mul(4),
            min_granularity: chunk_len.saturating_mul(2),
            supports_sparse_offsets: false,
            requires_alignment: true,
            supports_merkle_proof: false,
        });

        let err = ensure_range_capability_satisfies_plan(&plan, "beta", &metadata)
            .expect_err("should fail");
        assert!(
            err.contains("alignment"),
            "error should mention alignment, got {err}"
        );
    }
}

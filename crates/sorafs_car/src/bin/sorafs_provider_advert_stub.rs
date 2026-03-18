//! CLI helper for constructing SoraFS provider advertisements.
use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::{Signature as DalekSignature, Signer, SigningKey, VerifyingKey};
use norito::json::{Map, Value, to_string_pretty};
use sorafs_car::{ProfileId, chunker_registry};
use sorafs_manifest::{
    AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, EndpointKind,
    EndpointMetadata, EndpointMetadataKey, MAX_ADVERT_TTL_SECS, ProviderAdvertBuildError,
    ProviderAdvertV1, ProviderCapabilityRangeV1, REFRESH_RECOMMENDATION_SECS, RendezvousTopic,
    SignatureAlgorithm, StreamBudgetV1, TransportHintV1, TransportProtocol,
    provider_advert::ProviderCapabilitySoranetPqV1,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = match args.next() {
        Some(flag) if flag == "--emit" => {
            let mut opts = EmitOptions::default();
            for arg in args {
                let (key, value) = arg
                    .split_once('=')
                    .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
                match key {
                    "--profile-id" => {
                        if opts.profile_handle.is_some() {
                            return Err(
                                "use either --profile-id or --chunker-profile (not both)".into()
                            );
                        }
                        opts.profile_id = Some(value.to_string());
                    }
                    "--chunker-profile" => {
                        if opts.profile_id.is_some() {
                            return Err(
                                "use either --profile-id or --chunker-profile (not both)".into()
                            );
                        }
                        opts.profile_handle = Some(value.trim().to_string());
                    }
                    "--provider-id" => opts.provider_id = Some(parse_hex_fixed::<32>(value)?),
                    "--stake-pool-id" => opts.stake_pool_id = Some(parse_hex_fixed::<32>(value)?),
                    "--stake-amount" => opts.stake_amount = Some(parse_u128(value)?),
                    "--availability" => opts.availability = Some(parse_availability(value)?),
                    "--max-latency-ms" => opts.max_latency_ms = Some(parse_u32(value)?),
                    "--max-streams" => opts.max_streams = Some(parse_u16(value)?),
                    "--range-capability" => {
                        if opts.range_capability.is_some() {
                            return Err("only one --range-capability entry may be specified".into());
                        }
                        opts.range_capability = Some(parse_range_capability(value)?);
                    }
                    "--soranet-pq" => {
                        if opts.soranet_pq.is_some() {
                            return Err("only one --soranet-pq entry may be specified".into());
                        }
                        opts.soranet_pq = Some(parse_soranet_pq(value)?);
                    }
                    "--stream-budget" => {
                        if opts.stream_budget.is_some() {
                            return Err("only one --stream-budget entry may be specified".into());
                        }
                        opts.stream_budget = Some(parse_stream_budget(value)?);
                    }
                    "--transport-hint" => {
                        let hint = parse_transport_hint(value)?;
                        opts.transport_hints.push(hint);
                    }
                    "--capability" => opts.capabilities.push(parse_capability(value)?),
                    "--endpoint" => opts.endpoints.push(parse_endpoint(value)?),
                    "--endpoint-meta" => parse_endpoint_metadata(value, &mut opts)?,
                    "--topic" => opts.topics.push(parse_topic(value)?),
                    "--min-guard-weight" => opts.min_guard_weight = Some(parse_u16(value)?),
                    "--max-same-asn" => opts.max_same_asn = Some(parse_u8(value)?),
                    "--max-same-pool" => opts.max_same_pool = Some(parse_u8(value)?),
                    "--notes" => opts.notes = Some(value.to_string()),
                    "--issued-at" => opts.issued_at = Some(parse_u64(value)?),
                    "--allow-unknown-capabilities" => {
                        opts.allow_unknown_capabilities = parse_bool(value)?;
                    }
                    "--ttl-secs" => opts.ttl_secs = Some(parse_u64(value)?),
                    "--signature-alg" => opts.signature_alg = Some(parse_signature_alg(value)?),
                    "--public-key" => opts.public_key = Some(parse_hex_vec(value)?),
                    "--public-key-file" => opts.public_key = Some(read_file_bytes(value)?),
                    "--signature" => opts.signature = Some(parse_hex_vec(value)?),
                    "--signature-file" => opts.signature = Some(read_file_bytes(value)?),
                    "--signing-key-file" => opts.signing_key_file = Some(PathBuf::from(value)),
                    "--signing-key" => opts.signing_key_hex = Some(parse_hex_vec(value)?),
                    "--public-key-out" => opts.public_key_out = Some(PathBuf::from(value)),
                    "--signature-out" => opts.signature_out = Some(PathBuf::from(value)),
                    "--advert-out" => opts.advert_out = Some(PathBuf::from(value)),
                    "--json-out" => opts.json_out = Some(PathBuf::from(value)),
                    _ => return Err(format!("unknown option: {key}")),
                }
            }
            Command::Emit(Box::new(opts))
        }
        Some(flag) if flag == "--verify" => {
            let mut opts = VerifyOptions::default();
            for arg in args {
                let (key, value) = arg
                    .split_once('=')
                    .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
                match key {
                    "--advert" => opts.advert_path = Some(PathBuf::from(value)),
                    "--json-out" => opts.json_out = Some(PathBuf::from(value)),
                    "--now" => opts.now = Some(parse_u64(value)?),
                    _ => return Err(format!("unknown option: {key}")),
                }
            }
            Command::Verify(opts)
        }
        _ => return Err(usage().to_string()),
    };

    match command {
        Command::Emit(opts) => handle_emit(*opts),
        Command::Verify(opts) => handle_verify(opts),
    }
}

fn handle_emit(opts: EmitOptions) -> Result<(), String> {
    if opts.signing_key_file.is_some() && opts.signing_key_hex.is_some() {
        return Err("use either --signing-key or --signing-key-file (not both)".into());
    }
    if (opts.signing_key_file.is_some() || opts.signing_key_hex.is_some())
        && (opts.public_key.is_some() || opts.signature.is_some() || opts.signature_alg.is_some())
    {
        return Err(
            "use --signing-key/--signing-key-file without --public-key/--signature/--signature-alg"
                .into(),
        );
    }
    let mut advert = build_advert(&opts)?;
    let mut signature_verified = true;
    if opts.signing_key_file.is_some() {
        advert.signature_strict = true;
        verify_advert_signature(&advert)
            .map_err(|err| format!("signature validation failed: {err}"))?;
    } else {
        match verify_advert_signature(&advert) {
            Ok(()) => {
                advert.signature_strict = true;
            }
            Err(err) => {
                advert.signature_strict = false;
                signature_verified = false;
                eprintln!("warning: signature validation failed: {err}");
            }
        }
    }

    let bytes = norito::to_bytes(&advert).map_err(|err| err.to_string())?;

    if let Some(path) = &opts.advert_out {
        write_bytes(path, &bytes)?;
    }

    if let Some(path) = &opts.public_key_out {
        write_bytes(path, &advert.signature.public_key)?;
    }
    if let Some(path) = &opts.signature_out {
        write_bytes(path, &advert.signature.signature)?;
    }

    let report = build_report(&advert, &bytes, signature_verified);
    let mut report_string =
        to_string_pretty(&report).map_err(|err| format!("failed to serialise JSON: {err}"))?;
    report_string.push('\n');

    let mut wrote_stdout = false;
    if let Some(path) = &opts.json_out
        && write_text(path, &report_string)?
    {
        wrote_stdout = true;
    }

    if !wrote_stdout {
        print!("{report_string}");
    }
    Ok(())
}

fn handle_verify(opts: VerifyOptions) -> Result<(), String> {
    let advert_path = opts
        .advert_path
        .ok_or_else(|| "missing required option: --advert=<path>".to_string())?;
    let bytes = fs::read(&advert_path)
        .map_err(|err| format!("failed to read advert {advert_path:?}: {err}"))?;
    let advert =
        norito::decode_from_bytes::<ProviderAdvertV1>(&bytes).map_err(|err| err.to_string())?;
    let now = opts.now.unwrap_or(advert.issued_at);
    advert
        .validate_with_body(now)
        .map_err(|err| err.to_string())?;
    let mut signature_verified = true;
    match verify_advert_signature(&advert) {
        Ok(()) => {}
        Err(err) => {
            if advert.signature_strict {
                return Err(format!("signature validation failed: {err}"));
            }
            signature_verified = false;
            eprintln!("warning: signature validation failed: {err}");
        }
    }

    let report = build_report(&advert, &bytes, signature_verified);
    let mut report_string =
        to_string_pretty(&report).map_err(|err| format!("failed to serialise JSON: {err}"))?;
    report_string.push('\n');

    let mut wrote_stdout = false;
    if let Some(path) = &opts.json_out
        && write_text(path, &report_string)?
    {
        wrote_stdout = true;
    }

    if !wrote_stdout {
        print!("{report_string}");
    }
    Ok(())
}

fn usage() -> &'static str {
    "usage: sorafs-provider-advert-stub <--emit|--verify> \
     --emit \
     [--chunker-profile=namespace.name@semver | --profile-id=id] \
     --provider-id=hex32 \
     --stake-pool-id=hex32 \
     --stake-amount=number \
     --availability=hot|warm|cold \
     --max-latency-ms=value \
     --max-streams=value \
     --capability=kind \
     [--soranet-pq=guard|majority|strict] \
     [--range-capability=max_span=...,min_granularity=...[,sparse=bool,...]] \
     [--stream-budget=max_in_flight=...,max_bytes_per_sec=...[,burst=...]] \
     [--transport-hint=protocol:priority] \
     --endpoint=kind:host \
     --topic=name:region \
     --public-key=hex \
     --public-key-file=path \
     --signature=hex \
     --signature-file=path \
     [--signing-key-file=path] \
     [--signing-key=hex] \
     [--signature-alg=ed25519] \
      [--notes=text] \
      [--min-guard-weight=number] \
      [--max-same-asn=number] \
      [--max-same-pool=number] \
      [--allow-unknown-capabilities=true|false] \
      [--ttl-secs=seconds] \
     [--issued-at=unix] \
     [--endpoint-meta=key:value] \
     [--advert-out=path] \
     [--json-out=path] \
     [--public-key-out=path] \
     [--signature-out=path] \
     --verify \
     --advert=path \
     [--now=unix] \
     [--json-out=path]"
}

fn build_advert(opts: &EmitOptions) -> Result<ProviderAdvertV1, String> {
    chunker_registry::ensure_charter_compliance()
        .map_err(|err| format!("registry charter violation: {err}"))?;
    let profile_handle = if let Some(handle) = &opts.profile_handle {
        resolve_profile_handle(handle)?
    } else if let Some(id) = &opts.profile_id {
        resolve_profile_handle(id)?
    } else {
        return Err("missing required option: --chunker-profile or --profile-id".into());
    };
    let profile_aliases = chunker_registry::lookup_by_handle(&profile_handle)
        .map(|descriptor| {
            let mut aliases: Vec<String> = descriptor
                .aliases
                .iter()
                .map(|alias| alias.to_string())
                .collect();
            if !aliases.iter().any(|alias| alias == &profile_handle) {
                aliases.insert(0, profile_handle.clone());
            } else if aliases.first().map(String::as_str) != Some(profile_handle.as_str()) {
                // Ensure canonical handle is first for deterministic negotiation.
                aliases.retain(|alias| alias != &profile_handle);
                aliases.insert(0, profile_handle.clone());
            }
            aliases
        })
        .unwrap_or_else(|| vec![profile_handle.clone()]);

    let issued_at = opts
        .issued_at
        .unwrap_or_else(|| current_unix_time().unwrap_or(0));
    let ttl = opts
        .ttl_secs
        .unwrap_or(REFRESH_RECOMMENDATION_SECS * 2)
        .min(MAX_ADVERT_TTL_SECS);
    if ttl == 0 {
        return Err("ttl-secs must be greater than zero".into());
    }
    let provider_id = opts
        .provider_id
        .ok_or_else(|| "missing required option: --provider-id".to_string())?;
    let stake_pool_id = opts
        .stake_pool_id
        .ok_or_else(|| "missing required option: --stake-pool-id".to_string())?;
    let stake_amount = opts
        .stake_amount
        .ok_or_else(|| "missing required option: --stake-amount".to_string())?;
    let availability = opts
        .availability
        .ok_or_else(|| "missing required option: --availability".to_string())?;
    let max_latency_ms = opts
        .max_latency_ms
        .ok_or_else(|| "missing required option: --max-latency-ms".to_string())?;
    let max_streams = opts
        .max_streams
        .ok_or_else(|| "missing required option: --max-streams".to_string())?;
    let capability_count = opts.capabilities.len()
        + usize::from(opts.range_capability.is_some())
        + usize::from(opts.soranet_pq.is_some());
    if capability_count == 0 {
        return Err(
            "at least one capability is required (--capability or --range-capability)".into(),
        );
    }
    if opts.endpoints.is_empty() {
        return Err("at least one --endpoint entry is required".into());
    }
    if opts.topics.is_empty() {
        return Err("at least one --topic entry is required".into());
    }
    if opts.stream_budget.is_some() && opts.range_capability.is_none() {
        return Err("--stream-budget requires --range-capability".into());
    }
    if !opts.transport_hints.is_empty() && opts.range_capability.is_none() {
        return Err("--transport-hint requires --range-capability".into());
    }
    if opts
        .transport_hints
        .iter()
        .any(|hint| hint.protocol == TransportProtocol::SoraNetRelay)
        && !(opts
            .capabilities
            .iter()
            .any(|cap| cap.cap_type == CapabilityType::SoraNetHybridPq)
            || opts.soranet_pq.is_some())
    {
        return Err(
            "--transport-hint=soranet_relay requires --capability=soranet_pq or --soranet-pq"
                .into(),
        );
    }

    let mut signing_key = if let Some(path) = &opts.signing_key_file {
        let bytes = read_file_bytes(path)?;
        Some(parse_signing_key(&bytes)?)
    } else if let Some(hex) = opts.signing_key_hex.as_deref() {
        Some(parse_signing_key(hex)?)
    } else {
        None
    };

    let (signature_alg, public_key_bytes, signature_bytes) =
        if let Some(signing_key) = signing_key.as_ref() {
            (
                SignatureAlgorithm::Ed25519,
                signing_key.verifying_key().to_bytes().to_vec(),
                vec![0u8; 64],
            )
        } else {
            let signature_alg = opts.signature_alg.unwrap_or(SignatureAlgorithm::Ed25519);
            let public_key = opts
                .public_key
                .clone()
                .ok_or_else(|| "missing required option: --public-key".to_string())?;
            let signature = opts
                .signature
                .clone()
                .ok_or_else(|| "missing required option: --signature".to_string())?;
            (signature_alg, public_key, signature)
        };

    let mut builder = ProviderAdvertV1::builder();
    let _ = builder
        .profile_id(profile_handle.clone())
        .profile_aliases(profile_aliases)
        .provider_id(provider_id)
        .stake_pool_id(stake_pool_id)
        .stake_amount(stake_amount)
        .availability(availability)
        .max_retrieval_latency_ms(max_latency_ms)
        .max_concurrent_streams(max_streams)
        .issued_at(issued_at)
        .ttl_secs(ttl)
        .allow_unknown_capabilities(opts.allow_unknown_capabilities)
        .path_policy_min_guard_weight(opts.min_guard_weight.unwrap_or(10))
        .path_policy_max_same_asn_per_path(opts.max_same_asn.unwrap_or(1))
        .path_policy_max_same_pool_per_path(opts.max_same_pool.unwrap_or(1));
    if let Some(notes) = &opts.notes {
        let _ = builder.notes(notes.clone());
    }
    for capability in &opts.capabilities {
        let _ = builder.add_capability(capability.clone());
    }
    if let Some(range_capability) = &opts.range_capability {
        builder
            .add_range_capability(*range_capability)
            .map_err(|err| format!("invalid range capability: {err}"))?;
    }
    if let Some(pq_capability) = &opts.soranet_pq {
        let payload = pq_capability
            .to_bytes()
            .map_err(|err| format!("invalid soranet-pq capability: {err}"))?;
        let _ = builder.add_capability(CapabilityTlv {
            cap_type: CapabilityType::SoraNetHybridPq,
            payload,
        });
    }
    if let Some(budget) = &opts.stream_budget {
        let _ = builder.stream_budget(*budget);
    }
    if !opts.transport_hints.is_empty() {
        let _ = builder.transport_hints(opts.transport_hints.clone());
    }
    for endpoint in &opts.endpoints {
        let _ = builder.add_endpoint(endpoint.clone());
    }
    for topic in &opts.topics {
        let _ = builder.add_topic(topic.clone());
    }
    let _ = builder.signature(signature_alg, public_key_bytes, signature_bytes);

    let mut advert = builder.build().map_err(|err| match err {
        ProviderAdvertBuildError::MissingField(field) => {
            let option = match field {
                "profile_id" => "--chunker-profile/--profile-id",
                "provider_id" => "--provider-id",
                "stake_pool_id" => "--stake-pool-id",
                "stake_amount" => "--stake-amount",
                "availability" => "--availability",
                "max_retrieval_latency_ms" => "--max-latency-ms",
                "max_concurrent_streams" => "--max-streams",
                "capabilities" => "--capability/--range-capability",
                "endpoints" => "--endpoint",
                "rendezvous_topics" => "--topic",
                "public_key" => "--public-key",
                "signature" => "--signature",
                other => other,
            };
            format!("missing required option: {option}")
        }
        ProviderAdvertBuildError::Validation(validation) => validation.to_string(),
    })?;

    if let Some(signing_key) = signing_key.take() {
        let body_bytes =
            norito::to_bytes(&advert.body).map_err(|err| format!("encode advert body: {err}"))?;
        let sig = signing_key.sign(&body_bytes);
        advert.signature.algorithm = SignatureAlgorithm::Ed25519;
        advert.signature.public_key = signing_key.verifying_key().to_bytes().to_vec();
        advert.signature.signature = sig.to_bytes().to_vec();
    }

    Ok(advert)
}

fn build_report(advert: &ProviderAdvertV1, bytes: &[u8], signature_verified: bool) -> Value {
    let mut advert_obj = Map::new();
    advert_obj.insert("version".into(), Value::from(advert.version));
    advert_obj.insert("issued_at".into(), Value::from(advert.issued_at));
    advert_obj.insert("expires_at".into(), Value::from(advert.expires_at));
    advert_obj.insert("ttl_secs".into(), Value::from(advert.ttl()));
    advert_obj.insert(
        "refresh_recommended_at".into(),
        Value::from(advert.refresh_deadline()),
    );
    advert_obj.insert(
        "allow_unknown_capabilities".into(),
        Value::from(advert.allow_unknown_capabilities),
    );

    let mut body_obj = Map::new();
    body_obj.insert(
        "provider_id_hex".into(),
        Value::from(hex(&advert.body.provider_id)),
    );
    body_obj.insert(
        "profile_id".into(),
        Value::from(advert.body.profile_id.clone()),
    );
    if let Some(aliases) = &advert.body.profile_aliases {
        body_obj.insert(
            "profile_aliases".into(),
            Value::Array(aliases.iter().cloned().map(Value::from).collect()),
        );
    }
    let descriptor_opt =
        chunker_registry::lookup_by_handle(&advert.body.profile_id).or_else(|| {
            resolve_profile_handle(&advert.body.profile_id)
                .ok()
                .and_then(|handle| chunker_registry::lookup_by_handle(&handle))
        });
    if let Some(descriptor) = descriptor_opt {
        body_obj.insert(
            "profile_handle".into(),
            Value::from(format!(
                "{}.{}@{}",
                descriptor.namespace, descriptor.name, descriptor.semver
            )),
        );
        body_obj.insert(
            "profile_namespace".into(),
            Value::from(descriptor.namespace),
        );
        body_obj.insert("profile_name".into(), Value::from(descriptor.name));
        body_obj.insert("profile_semver".into(), Value::from(descriptor.semver));
    }
    body_obj.insert(
        "stake_pool_id_hex".into(),
        Value::from(hex(&advert.body.stake.pool_id)),
    );
    body_obj.insert(
        "stake_amount".into(),
        Value::from(advert.body.stake.stake_amount.to_string()),
    );
    body_obj.insert(
        "availability".into(),
        Value::from(format!("{:?}", advert.body.qos.availability)),
    );
    body_obj.insert(
        "max_retrieval_latency_ms".into(),
        Value::from(advert.body.qos.max_retrieval_latency_ms),
    );
    body_obj.insert(
        "max_concurrent_streams".into(),
        Value::from(advert.body.qos.max_concurrent_streams),
    );

    let capabilities: Vec<Value> = advert
        .body
        .capabilities
        .iter()
        .map(|cap| {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::from(capability_name(cap.cap_type)));
            obj.insert("payload_hex".into(), Value::from(hex(&cap.payload)));
            if cap.cap_type == CapabilityType::ChunkRangeFetch {
                match ProviderCapabilityRangeV1::from_bytes(&cap.payload) {
                    Ok(range) => {
                        let mut range_obj = Map::new();
                        range_obj
                            .insert("max_chunk_span".into(), Value::from(range.max_chunk_span));
                        range_obj
                            .insert("min_granularity".into(), Value::from(range.min_granularity));
                        range_obj.insert(
                            "supports_sparse_offsets".into(),
                            Value::from(range.supports_sparse_offsets),
                        );
                        range_obj.insert(
                            "requires_alignment".into(),
                            Value::from(range.requires_alignment),
                        );
                        range_obj.insert(
                            "supports_merkle_proof".into(),
                            Value::from(range.supports_merkle_proof),
                        );
                        obj.insert("range".into(), Value::Object(range_obj));
                    }
                    Err(err) => {
                        obj.insert("range_decode_error".into(), Value::from(format!("{err}")));
                    }
                }
            } else if cap.cap_type == CapabilityType::SoraNetHybridPq {
                match ProviderCapabilitySoranetPqV1::from_bytes(&cap.payload) {
                    Ok(pq) => {
                        let mut pq_obj = Map::new();
                        pq_obj.insert("supports_guard".into(), Value::from(pq.supports_guard));
                        pq_obj.insert(
                            "supports_majority".into(),
                            Value::from(pq.supports_majority),
                        );
                        pq_obj.insert("supports_strict".into(), Value::from(pq.supports_strict));
                        obj.insert("soranet_pq".into(), Value::Object(pq_obj));
                    }
                    Err(err) => {
                        obj.insert(
                            "soranet_pq_decode_error".into(),
                            Value::from(format!("{err}")),
                        );
                    }
                }
            }
            Value::Object(obj)
        })
        .collect();
    body_obj.insert("capabilities".into(), Value::Array(capabilities));

    if let Some(budget) = &advert.body.stream_budget {
        let mut budget_obj = Map::new();
        budget_obj.insert(
            "max_in_flight".into(),
            Value::from(budget.max_in_flight as u64),
        );
        budget_obj.insert(
            "max_bytes_per_sec".into(),
            Value::from(budget.max_bytes_per_sec),
        );
        if let Some(burst) = budget.burst_bytes {
            budget_obj.insert("burst_bytes".into(), Value::from(burst));
        }
        body_obj.insert("stream_budget".into(), Value::Object(budget_obj));
    }
    if let Some(hints) = &advert.body.transport_hints {
        let hint_values: Vec<Value> = hints
            .iter()
            .map(|hint| {
                let mut hint_obj = Map::new();
                hint_obj.insert(
                    "protocol".into(),
                    Value::from(transport_protocol_name(hint.protocol)),
                );
                hint_obj.insert("priority".into(), Value::from(hint.priority as u64));
                Value::Object(hint_obj)
            })
            .collect();
        body_obj.insert("transport_hints".into(), Value::Array(hint_values));
    }

    let endpoints: Vec<Value> = advert
        .body
        .endpoints
        .iter()
        .map(|endpoint| {
            let mut obj = Map::new();
            obj.insert(
                "kind".into(),
                Value::from(endpoint_kind_name(endpoint.kind)),
            );
            obj.insert(
                "host_pattern".into(),
                Value::from(endpoint.host_pattern.clone()),
            );
            let meta: Vec<Value> = endpoint
                .metadata
                .iter()
                .map(|entry| {
                    let mut mobj = Map::new();
                    mobj.insert("key".into(), Value::from(endpoint_metadata_name(entry.key)));
                    mobj.insert("value_hex".into(), Value::from(hex(&entry.value)));
                    Value::Object(mobj)
                })
                .collect();
            obj.insert("metadata".into(), Value::Array(meta));
            Value::Object(obj)
        })
        .collect();
    body_obj.insert("endpoints".into(), Value::Array(endpoints));

    let topics: Vec<Value> = advert
        .body
        .rendezvous_topics
        .iter()
        .map(|topic| {
            let mut obj = Map::new();
            obj.insert("topic".into(), Value::from(topic.topic.clone()));
            obj.insert("region".into(), Value::from(topic.region.clone()));
            Value::Object(obj)
        })
        .collect();
    body_obj.insert("rendezvous_topics".into(), Value::Array(topics));

    let mut path_obj = Map::new();
    path_obj.insert(
        "min_guard_weight".into(),
        Value::from(advert.body.path_policy.min_guard_weight as u64),
    );
    path_obj.insert(
        "max_same_asn_per_path".into(),
        Value::from(advert.body.path_policy.max_same_asn_per_path as u64),
    );
    path_obj.insert(
        "max_same_pool_per_path".into(),
        Value::from(advert.body.path_policy.max_same_pool_per_path as u64),
    );
    body_obj.insert("path_policy".into(), Value::Object(path_obj));

    if let Some(notes) = &advert.body.notes {
        body_obj.insert("notes".into(), Value::from(notes.clone()));
    }

    advert_obj.insert("body".into(), Value::Object(body_obj));
    let mut sig_obj = Map::new();
    sig_obj.insert(
        "algorithm".into(),
        Value::from(signature_alg_name(advert.signature.algorithm)),
    );
    sig_obj.insert(
        "public_key_hex".into(),
        Value::from(hex(&advert.signature.public_key)),
    );
    sig_obj.insert(
        "signature_hex".into(),
        Value::from(hex(&advert.signature.signature)),
    );
    advert_obj.insert("signature".into(), Value::Object(sig_obj));
    advert_obj.insert("signature_verified".into(), Value::from(signature_verified));
    advert_obj.insert("norito_len".into(), Value::from(bytes.len() as u64));
    advert_obj.insert("norito_hex".into(), Value::from(hex(bytes)));

    Value::Object(advert_obj)
}

enum Command {
    Emit(Box<EmitOptions>),
    Verify(VerifyOptions),
}

#[derive(Default)]
struct EmitOptions {
    profile_id: Option<String>,
    profile_handle: Option<String>,
    provider_id: Option<[u8; 32]>,
    stake_pool_id: Option<[u8; 32]>,
    stake_amount: Option<u128>,
    availability: Option<AvailabilityTier>,
    max_latency_ms: Option<u32>,
    max_streams: Option<u16>,
    capabilities: Vec<CapabilityTlv>,
    endpoints: Vec<AdvertEndpoint>,
    topics: Vec<RendezvousTopic>,
    min_guard_weight: Option<u16>,
    max_same_asn: Option<u8>,
    max_same_pool: Option<u8>,
    notes: Option<String>,
    issued_at: Option<u64>,
    ttl_secs: Option<u64>,
    signature_alg: Option<SignatureAlgorithm>,
    public_key: Option<Vec<u8>>,
    signature: Option<Vec<u8>>,
    signing_key_file: Option<PathBuf>,
    signing_key_hex: Option<Vec<u8>>,
    public_key_out: Option<PathBuf>,
    signature_out: Option<PathBuf>,
    advert_out: Option<PathBuf>,
    json_out: Option<PathBuf>,
    allow_unknown_capabilities: bool,
    range_capability: Option<ProviderCapabilityRangeV1>,
    soranet_pq: Option<ProviderCapabilitySoranetPqV1>,
    stream_budget: Option<StreamBudgetV1>,
    transport_hints: Vec<TransportHintV1>,
}

#[derive(Default)]
struct VerifyOptions {
    advert_path: Option<PathBuf>,
    json_out: Option<PathBuf>,
    now: Option<u64>,
}

fn parse_availability(value: &str) -> Result<AvailabilityTier, String> {
    match value.to_ascii_lowercase().as_str() {
        "hot" => Ok(AvailabilityTier::Hot),
        "warm" => Ok(AvailabilityTier::Warm),
        "cold" => Ok(AvailabilityTier::Cold),
        other => Err(format!("unknown availability tier: {other}")),
    }
}

fn parse_capability(value: &str) -> Result<CapabilityTlv, String> {
    let (head, payload_str) = value
        .split_once(':')
        .map_or((value, None), |(h, rest)| (h, Some(rest)));
    let head_lower = head.trim().to_ascii_lowercase();
    let cap_type = match head_lower.as_str() {
        "torii" | "torii-gateway" => CapabilityType::ToriiGateway,
        "quic" | "quic-noise" => CapabilityType::QuicNoise,
        "soranet" | "soranet-pq" | "soranet_pq" | "soranet-hybrid-pq" => {
            CapabilityType::SoraNetHybridPq
        }
        "range" | "chunk-range" | "chunk_range" => {
            return Err(
                "use --range-capability=<key=value,...> to describe chunk-range support".into(),
            );
        }
        _ if head_lower == "vendor" => CapabilityType::VendorReserved,
        other => {
            return Err(format!(
                "unknown capability type: {other} (expected torii|quic|soranet|soranet-pq|vendor)"
            ));
        }
    };
    let payload = match (cap_type, payload_str) {
        (CapabilityType::VendorReserved, Some(rest)) => parse_hex_vec(rest.trim())?,
        (CapabilityType::SoraNetHybridPq, Some(rest)) => parse_soranet_pq(rest.trim())?
            .to_bytes()
            .map_err(|err| format!("invalid soranet-pq capability: {err}"))?,
        (CapabilityType::SoraNetHybridPq, None) => ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: false,
            supports_strict: false,
        }
        .to_bytes()
        .map_err(|err| format!("invalid soranet-pq capability: {err}"))?,
        (_, Some(rest)) => parse_hex_vec(rest.trim())?,
        _ => Vec::new(),
    };
    Ok(CapabilityTlv { cap_type, payload })
}

fn parse_soranet_pq(value: &str) -> Result<ProviderCapabilitySoranetPqV1, String> {
    let mut supports_guard = false;
    let mut supports_majority = false;
    let mut supports_strict = false;
    let mut seen_level = false;
    for raw in value.split(|c| [',', '+', '|'].contains(&c)) {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        seen_level = true;
        match token.to_ascii_lowercase().as_str() {
            "guard" | "stage-a" | "stagea" => {
                supports_guard = true;
            }
            "majority" | "stage-b" | "stageb" => {
                supports_guard = true;
                supports_majority = true;
            }
            "strict" | "stage-c" | "stagec" => {
                supports_guard = true;
                supports_majority = true;
                supports_strict = true;
            }
            other => {
                return Err(format!(
                    "unknown soranet-pq level `{other}` (expected guard|majority|strict)"
                ));
            }
        }
    }
    if !seen_level {
        supports_guard = true;
    }
    let capability = ProviderCapabilitySoranetPqV1 {
        supports_guard,
        supports_majority,
        supports_strict,
    };
    capability
        .validate()
        .map_err(|err| format!("invalid soranet-pq capability: {err}"))?;
    Ok(capability)
}

fn parse_range_capability(value: &str) -> Result<ProviderCapabilityRangeV1, String> {
    let mut capability = ProviderCapabilityRangeV1::default();
    let mut max_span = None;
    let mut min_granularity = None;
    for part in value.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, raw) = trimmed.split_once('=').ok_or_else(|| {
            format!("range-capability requires key=value entries, got: {trimmed}")
        })?;
        let key_lower = key.trim().to_ascii_lowercase();
        let raw_trimmed = raw.trim();
        match key_lower.as_str() {
            "max_span" | "max-chunk-span" | "max_chunk_span" => {
                let span = parse_u32(raw_trimmed)?;
                max_span = Some(span);
            }
            "min_granularity" | "min-granularity" => {
                let granularity = parse_u32(raw_trimmed)?;
                min_granularity = Some(granularity);
            }
            "sparse" | "supports_sparse_offsets" | "supports-sparse-offsets" => {
                capability.supports_sparse_offsets = parse_bool(raw_trimmed)?;
            }
            "alignment" | "requires_alignment" | "requires-alignment" => {
                capability.requires_alignment = parse_bool(raw_trimmed)?;
            }
            "merkle" | "supports_merkle_proof" | "supports-merkle-proof" => {
                capability.supports_merkle_proof = parse_bool(raw_trimmed)?;
            }
            other => {
                return Err(format!(
                    "unknown range-capability field: {other} (expected max_span|min_granularity|sparse|alignment|merkle)"
                ));
            }
        }
    }
    capability.max_chunk_span =
        max_span.ok_or_else(|| "range-capability requires max_span=<u32>".to_string())?;
    capability.min_granularity = min_granularity
        .ok_or_else(|| "range-capability requires min_granularity=<u32>".to_string())?;
    capability
        .validate()
        .map_err(|err| format!("invalid range capability: {err}"))?;
    Ok(capability)
}

fn parse_stream_budget(value: &str) -> Result<StreamBudgetV1, String> {
    let mut max_in_flight = None;
    let mut max_bytes_per_sec = None;
    let mut burst_bytes = None;
    for part in value.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, raw) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("stream-budget requires key=value entries, got: {trimmed}"))?;
        let key_lower = key.trim().to_ascii_lowercase();
        let raw_trimmed = raw.trim();
        match key_lower.as_str() {
            "max_in_flight" | "max-in-flight" | "inflight" => {
                let inflight = parse_u16(raw_trimmed)?;
                max_in_flight = Some(inflight);
            }
            "max_bytes_per_sec" | "max-bytes-per-sec" | "max_rate" | "max-rate" => {
                let rate = parse_u64(raw_trimmed)?;
                max_bytes_per_sec = Some(rate);
            }
            "burst" | "burst_bytes" | "burst-bytes" => {
                let burst = parse_u64(raw_trimmed)?;
                burst_bytes = Some(burst);
            }
            other => {
                return Err(format!(
                    "unknown stream-budget field: {other} (expected max_in_flight|max_bytes_per_sec|burst)"
                ));
            }
        }
    }
    let budget = StreamBudgetV1 {
        max_in_flight: max_in_flight
            .ok_or_else(|| "stream-budget requires max_in_flight=<u16>".to_string())?,
        max_bytes_per_sec: max_bytes_per_sec
            .ok_or_else(|| "stream-budget requires max_bytes_per_sec=<u64>".to_string())?,
        burst_bytes,
    };
    budget
        .validate()
        .map_err(|err| format!("invalid stream budget: {err}"))?;
    Ok(budget)
}

fn parse_transport_hint(value: &str) -> Result<TransportHintV1, String> {
    let (protocol_str, priority_str) = value
        .split_once(':')
        .ok_or_else(|| "transport-hint requires protocol:priority".to_string())?;
    let protocol = parse_transport_protocol(protocol_str.trim())?;
    let priority = parse_u8(priority_str.trim())?;
    let hint = TransportHintV1 { protocol, priority };
    hint.validate()
        .map_err(|err| format!("invalid transport hint: {err}"))?;
    Ok(hint)
}

fn parse_transport_protocol(value: &str) -> Result<TransportProtocol, String> {
    match value.to_ascii_lowercase().as_str() {
        "torii" | "torii-http" | "torii_http" | "torii-range" | "torii_range" => {
            Ok(TransportProtocol::ToriiHttpRange)
        }
        "quic" | "quic-stream" | "quic_stream" => Ok(TransportProtocol::QuicStream),
        "soranet" | "relay" | "soranet-relay" | "soranet_relay" => {
            Ok(TransportProtocol::SoraNetRelay)
        }
        "vendor" | "vendor-reserved" | "vendor_reserved" => Ok(TransportProtocol::VendorReserved),
        other => Err(format!(
            "unknown transport protocol: {other} (expected torii|quic|soranet|vendor)"
        )),
    }
}

fn parse_endpoint(value: &str) -> Result<AdvertEndpoint, String> {
    let (kind_str, host) = value
        .split_once(':')
        .ok_or_else(|| "endpoint requires kind:host".to_string())?;
    let kind = match kind_str.to_ascii_lowercase().as_str() {
        "torii" => EndpointKind::Torii,
        "quic" => EndpointKind::Quic,
        "noritorpc" | "norito-rpc" => EndpointKind::NoritoRpc,
        other => return Err(format!("unknown endpoint kind: {other}")),
    };
    Ok(AdvertEndpoint {
        kind,
        host_pattern: host.to_string(),
        metadata: Vec::new(),
    })
}

fn parse_endpoint_metadata(value: &str, opts: &mut EmitOptions) -> Result<(), String> {
    let (key_str, data) = value
        .split_once(':')
        .ok_or_else(|| "endpoint-meta requires key:value".to_string())?;
    let endpoint = opts
        .endpoints
        .last_mut()
        .ok_or_else(|| "endpoint-meta requires at least one --endpoint before it".to_string())?;
    let key = match key_str.to_ascii_lowercase().as_str() {
        "tls" | "tls-fingerprint" => EndpointMetadataKey::TlsFingerprint,
        "alpn" => EndpointMetadataKey::Alpn,
        "region" => EndpointMetadataKey::Region,
        other => return Err(format!("unknown endpoint metadata key: {other}")),
    };
    let value_bytes = match key {
        EndpointMetadataKey::Region => data.as_bytes().to_vec(),
        _ => parse_hex_vec(data)?,
    };
    endpoint.metadata.push(EndpointMetadata {
        key,
        value: value_bytes,
    });
    Ok(())
}

fn parse_topic(value: &str) -> Result<RendezvousTopic, String> {
    let (topic, region) = value
        .split_once(':')
        .ok_or_else(|| "topic requires name:region".to_string())?;
    Ok(RendezvousTopic {
        topic: topic.to_string(),
        region: region.to_string(),
    })
}

fn parse_signature_alg(value: &str) -> Result<SignatureAlgorithm, String> {
    match value.to_ascii_lowercase().as_str() {
        "ed25519" => Ok(SignatureAlgorithm::Ed25519),
        "multisig" => Ok(SignatureAlgorithm::MultiSig),
        other => Err(format!("unknown signature algorithm: {other}")),
    }
}

fn resolve_profile_handle(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("chunker profile cannot be empty".into());
    }
    if let Some(descriptor) = chunker_registry::lookup_by_handle(trimmed) {
        return Ok(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        ));
    }
    if let Ok(id) = trimmed.parse::<u32>() {
        if let Some(descriptor) = chunker_registry::lookup(ProfileId(id)) {
            return Ok(format!(
                "{}.{}@{}",
                descriptor.namespace, descriptor.name, descriptor.semver
            ));
        }
        return Err(format!(
            "unknown chunker profile id: {id}. Use --list-chunker-profiles to inspect the registry"
        ));
    }
    if let Some(descriptor) = chunker_registry::registry().iter().find(|entry| {
        entry
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(trimmed))
    }) {
        return Ok(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        ));
    }
    Err(format!(
        "unknown chunker profile handle '{trimmed}'. expected namespace.name@semver"
    ))
}

fn parse_hex_vec(value: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(value.len().div_ceil(2));
    let bytes = value.as_bytes();
    let mut idx = 0;
    if !value.len().is_multiple_of(2) {
        let low = decode_hex(bytes[0])?;
        out.push(low);
        idx = 1;
    }
    while idx < bytes.len() {
        let high = decode_hex(bytes[idx])?;
        let low = decode_hex(bytes[idx + 1])?;
        out.push((high << 4) | low);
        idx += 2;
    }
    Ok(out)
}

fn parse_hex_fixed<const N: usize>(value: &str) -> Result<[u8; N], String> {
    let vec = parse_hex_vec(value)?;
    if vec.len() > N {
        return Err(format!("expected at most {N} hex bytes, got {}", vec.len()));
    }
    let mut arr = [0u8; N];
    let start = N - vec.len();
    arr[start..].copy_from_slice(&vec);
    Ok(arr)
}

fn parse_u64(value: &str) -> Result<u64, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u64::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u64>().map_err(|err| err.to_string())
    }
}

fn parse_u128(value: &str) -> Result<u128, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u128::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u128>().map_err(|err| err.to_string())
    }
}

fn parse_u32(value: &str) -> Result<u32, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u32::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u32>().map_err(|err| err.to_string())
    }
}

fn parse_u16(value: &str) -> Result<u16, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u16::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u16>().map_err(|err| err.to_string())
    }
}

fn parse_u8(value: &str) -> Result<u8, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u8::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u8>().map_err(|err| err.to_string())
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        other => Err(format!(
            "expected boolean true/false/1/0/yes/no, got {other}"
        )),
    }
}

fn decode_hex(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit: {}", byte as char)),
    }
}

fn write_bytes(path: &PathBuf, bytes: &[u8]) -> Result<(), String> {
    if path.as_path() == Path::new("-") {
        return Err("binary outputs (advert/public key/signature) do not support '-'".into());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    let mut file = File::create(path).map_err(|err| format!("failed to create {path:?}: {err}"))?;
    file.write_all(bytes)
        .map_err(|err| format!("failed to write {path:?}: {err}"))
}

fn write_text(path: &PathBuf, text: &str) -> Result<bool, String> {
    if path.as_path() == Path::new("-") {
        io::stdout()
            .write_all(text.as_bytes())
            .map_err(|err| format!("failed to write to stdout: {err}"))?;
        return Ok(true);
    }
    write_bytes(path, text.as_bytes()).map(|_| false)
}

fn capability_name(cap: CapabilityType) -> &'static str {
    match cap {
        CapabilityType::ToriiGateway => "torii",
        CapabilityType::QuicNoise => "quic",
        CapabilityType::ChunkRangeFetch => "range",
        CapabilityType::SoraNetHybridPq => "soranet_pq",
        CapabilityType::VendorReserved => "vendor",
    }
}

fn read_file_bytes(path: impl AsRef<Path>) -> Result<Vec<u8>, String> {
    let path_ref = path.as_ref();
    fs::read(path_ref).map_err(|err| format!("failed to read {}: {err}", path_ref.display()))
}

fn parse_signing_key(bytes: &[u8]) -> Result<SigningKey, String> {
    match bytes.len() {
        32 => {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(bytes);
            Ok(SigningKey::from_bytes(&seed))
        }
        64 => {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes[..32]);
            Ok(SigningKey::from_bytes(&seed))
        }
        other => Err(format!(
            "signing key must be 32-byte seed or 64-byte expanded key, got {other} bytes"
        )),
    }
}

fn verify_advert_signature(advert: &ProviderAdvertV1) -> Result<(), String> {
    match advert.signature.algorithm {
        SignatureAlgorithm::Ed25519 => {
            if advert.signature.public_key.len() != 32 {
                return Err("ed25519 public key must be 32 bytes".into());
            }
            if advert.signature.signature.len() != 64 {
                return Err("ed25519 signature must be 64 bytes".into());
            }
            let mut pk = [0u8; 32];
            pk.copy_from_slice(&advert.signature.public_key);
            let verifying_key = VerifyingKey::from_bytes(&pk)
                .map_err(|err| format!("invalid ed25519 public key: {err}"))?;

            let mut sig_bytes = [0u8; 64];
            sig_bytes.copy_from_slice(&advert.signature.signature);
            let signature = DalekSignature::from_bytes(&sig_bytes);

            let body_bytes = norito::to_bytes(&advert.body)
                .map_err(|err| format!("encode advert body: {err}"))?;
            verifying_key
                .verify_strict(&body_bytes, &signature)
                .map_err(|_| "ed25519 signature validation failed".to_string())
        }
        other => Err(format!("unsupported signature algorithm: {other:?}")),
    }
}

fn endpoint_kind_name(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Torii => "torii",
        EndpointKind::Quic => "quic",
        EndpointKind::NoritoRpc => "norito-rpc",
    }
}

fn endpoint_metadata_name(key: EndpointMetadataKey) -> &'static str {
    match key {
        EndpointMetadataKey::TlsFingerprint => "tls_fingerprint",
        EndpointMetadataKey::Alpn => "alpn",
        EndpointMetadataKey::Region => "region",
    }
}

fn transport_protocol_name(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::ToriiHttpRange => "torii",
        TransportProtocol::QuicStream => "quic",
        TransportProtocol::SoraNetRelay => "soranet",
        TransportProtocol::VendorReserved => "vendor",
    }
}

fn signature_alg_name(alg: SignatureAlgorithm) -> &'static str {
    match alg {
        SignatureAlgorithm::Ed25519 => "ed25519",
        SignatureAlgorithm::MultiSig => "multisig",
    }
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

fn current_unix_time() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_capability_success() {
        let capability = parse_range_capability(
            "max_span=16,min_granularity=4,sparse=true,alignment=false,merkle=true",
        )
        .expect("range capability parses");
        assert_eq!(capability.max_chunk_span, 16);
        assert_eq!(capability.min_granularity, 4);
        assert!(capability.supports_sparse_offsets);
        assert!(!capability.requires_alignment);
        assert!(capability.supports_merkle_proof);
    }

    #[test]
    fn parse_range_capability_missing_field() {
        let err = parse_range_capability("min_granularity=4,sparse=false")
            .expect_err("missing max_span rejected");
        assert!(err.contains("max_span"), "unexpected error message: {err}");
    }

    #[test]
    fn parse_stream_budget_success() {
        let budget =
            parse_stream_budget("max_in_flight=5,max_bytes_per_sec=1000,burst=200").unwrap();
        assert_eq!(budget.max_in_flight, 5);
        assert_eq!(budget.max_bytes_per_sec, 1000);
        assert_eq!(budget.burst_bytes, Some(200));
    }

    #[test]
    fn parse_transport_hint_enforces_priority() {
        let hint = parse_transport_hint("torii:0").expect("valid hint parses");
        assert_eq!(hint.protocol, TransportProtocol::ToriiHttpRange);
        assert_eq!(hint.priority, 0);

        let err = parse_transport_hint("quic:32").expect_err("priority above 15 rejected");
        assert!(
            err.contains("invalid transport hint"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolves_canonical_handle() {
        let handle = resolve_profile_handle("sorafs.sf1@1.0.0").expect("handle resolves");
        assert_eq!(handle, "sorafs.sf1@1.0.0");
    }

    #[test]
    fn resolves_numeric_id() {
        let handle = resolve_profile_handle("1").expect("numeric resolves");
        assert_eq!(handle, "sorafs.sf1@1.0.0");
    }

    #[test]
    fn resolves_alias_with_dash() {
        let handle = resolve_profile_handle("sorafs-sf1").expect("alias resolves");
        assert_eq!(handle, "sorafs.sf1@1.0.0");
    }

    #[test]
    fn rejects_unknown_id() {
        let err = resolve_profile_handle("99").expect_err("unknown id must fail");
        assert!(
            err.contains("unknown chunker profile id"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn rejects_empty_input() {
        let err = resolve_profile_handle("   ").expect_err("empty input");
        assert!(err.contains("cannot be empty"));
    }
}

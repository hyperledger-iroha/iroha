use super::{
    chunker_registry, parse_hex_array, parse_hex_vec, parse_profile_handle, parse_u16, parse_u32,
    parse_u64, read_file_bytes, write_binary, write_json,
};
use ed25519_dalek::{Signer, SigningKey};
use norito::{
    decode_from_bytes,
    json::{Map, Value, to_string_pretty},
    to_bytes,
};
use sorafs_manifest::{
    AdmissionRecord, AdvertEndpoint, CapabilityTlv, CapabilityType, CouncilSignature,
    ENDPOINT_ATTESTATION_VERSION_V1, EndpointAdmissionV1, EndpointAttestationKind,
    EndpointAttestationV1, EndpointKind, PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
    PROVIDER_ADMISSION_PROPOSAL_VERSION_V1, PROVIDER_ADMISSION_RENEWAL_VERSION_V1,
    PROVIDER_ADMISSION_REVOCATION_VERSION_V1, ProviderAdmissionCouncilPolicy,
    ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdmissionRenewalV1,
    ProviderAdmissionRevocationV1, ProviderCapabilityRangeV1, ProviderVrfPublicKeyV1, StakePointer,
    StreamBudgetV1, TransportHintV1, TransportProtocol, compute_advert_body_digest,
    compute_envelope_authorization_digest, compute_envelope_digest, compute_proposal_digest,
    deal::XorQuantity,
    decode_provider_advert_v1,
    provider_advert::{PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1, ProviderCapabilitySoranetPqV1},
    verify_advert_against_record, verify_revocation_signatures_untrusted_signers,
};
use std::{
    fs,
    io::Read as _,
    iter::Iterator,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const PROPOSAL_VERSION: u8 = PROVIDER_ADMISSION_PROPOSAL_VERSION_V1;
const ENVELOPE_VERSION: u8 = PROVIDER_ADMISSION_ENVELOPE_VERSION_V1;
const RENEWAL_VERSION: u8 = PROVIDER_ADMISSION_RENEWAL_VERSION_V1;
const REVOCATION_VERSION: u8 = PROVIDER_ADMISSION_REVOCATION_VERSION_V1;
pub(super) fn run<I>(mut args: I) -> Result<(), String>
where
    I: Iterator<Item = String>,
{
    let subcommand = args.next().ok_or_else(|| usage().to_string())?;
    match subcommand.as_str() {
        "proposal" => run_proposal(args.collect()),
        "sign" => run_sign(args.collect()),
        "verify" => run_verify(args.collect()),
        "renewal" => run_renewal(args.collect()),
        "revoke" => run_revoke(args.collect()),
        "--help" | "-h" => Err(usage().to_string()),
        other => Err(format!(
            "{usage}\nunknown provider-admission subcommand: {other}",
            usage = usage()
        )),
    }
}
fn usage() -> &'static str {
    "usage: sorafs_manifest_builder provider-admission <proposal|sign|verify|renewal|revoke> [options]"
}
fn run_proposal(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(proposal_usage().to_string());
    }
    let mut opts = ProposalOptions::default();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--provider-id" => opts.provider_id = Some(parse_hex_array(value)?),
            "--chunker-profile" => {
                opts.profile_handle = Some(parse_profile_handle(value, "--chunker-profile")?)
            }
            "--stake-pool-id" => opts.stake_pool_id = Some(parse_hex_array(value)?),
            "--stake-amount" => opts.stake_amount = Some(parse_xor_quantity(value)?),
            "--advert-key" => opts.advert_key = Some(parse_hex_array(value)?),
            "--por-vrf-key" => opts.por_vrf_key = Some(parse_provider_vrf_key(value)?),
            "--jurisdiction-code" => opts.jurisdiction = Some(parse_jurisdiction_code(value)?),
            "--contact-uri" => opts.contact_uri = Some(value.to_string()),
            "--capability" => opts.capabilities.push(parse_capability(value)?),
            "--range-capability" => {
                if opts.range_capability.is_some() {
                    return Err("range capability specified multiple times".to_string());
                }
                opts.range_capability = Some(parse_range_capability(value)?);
            }
            "--soranet-pq" => {
                if opts.soranet_pq.is_some() {
                    return Err("SoraNet PQ capability specified multiple times".to_string());
                }
                opts.soranet_pq = Some(parse_soranet_pq(value)?);
            }
            "--endpoint" => {
                let endpoint = parse_endpoint(value)?;
                opts.endpoints.push(EndpointBuilder::new(endpoint));
            }
            "--endpoint-attestation-kind" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.attestation_kind = Some(parse_attestation_kind(value)?);
            }
            "--endpoint-attestation-attested-at" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.attested_at = Some(parse_u64(value)?);
            }
            "--endpoint-attestation-expires-at" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.expires_at = Some(parse_u64(value)?);
            }
            "--endpoint-attestation-leaf" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.leaf = Some(read_file_bytes(value)?);
            }
            "--endpoint-attestation-leaf-hex" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.leaf = Some(parse_hex_vec(value)?);
            }
            "--endpoint-attestation-alpn" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.alpn.push(value.to_string());
            }
            "--endpoint-attestation-report" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.report = Some(read_file_bytes(value)?);
            }
            "--endpoint-attestation-report-hex" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.report = Some(parse_hex_vec(value)?);
            }
            "--endpoint-attestation-intermediate" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.intermediates.push(read_file_bytes(value)?);
            }
            "--endpoint-attestation-intermediate-hex" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.intermediates.push(parse_hex_vec(value)?);
            }
            "--stream-budget" => {
                if opts.stream_budget.is_some() {
                    return Err("stream budget specified multiple times".to_string());
                }
                opts.stream_budget = Some(parse_stream_budget(value)?);
            }
            "--transport-hint" => {
                let hint = parse_transport_hint(value)?;
                opts.transport_hints.push(hint);
            }
            "--proposal-out" => opts.proposal_out = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option: {other}")),
        }
    }
    let descriptor = lookup_profile(&opts)?;
    let provider_id = opts
        .provider_id
        .ok_or_else(|| "missing option --provider-id".to_string())?;
    let stake_pool = opts
        .stake_pool_id
        .ok_or_else(|| "missing option --stake-pool-id".to_string())?;
    let stake_amount = opts
        .stake_amount
        .ok_or_else(|| "missing option --stake-amount".to_string())?;
    let advert_key = opts
        .advert_key
        .ok_or_else(|| "missing option --advert-key".to_string())?;
    let por_vrf_key = opts
        .por_vrf_key
        .ok_or_else(|| "missing option --por-vrf-key".to_string())?;
    let jurisdiction = opts
        .jurisdiction
        .ok_or_else(|| "missing option --jurisdiction-code".to_string())?;
    if opts.capabilities.is_empty() && opts.range_capability.is_none() && opts.soranet_pq.is_none()
    {
        return Err(
            "at least one --capability, --range-capability, or --soranet-pq is required".into(),
        );
    }
    if opts.endpoints.is_empty() {
        return Err("at least one --endpoint is required".into());
    }
    let canonical_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let mut profile_aliases: Vec<String> = descriptor
        .aliases
        .iter()
        .map(|alias| alias.to_string())
        .collect();
    profile_aliases.retain(|alias| alias != &canonical_handle);
    profile_aliases.insert(0, canonical_handle.clone());
    let mut seen = std::collections::HashSet::new();
    profile_aliases.retain(|alias| seen.insert(alias.clone()));
    let mut capabilities = opts.capabilities;
    if let Some(range) = opts.range_capability {
        capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::ChunkRangeFetch,
            payload: range
                .to_bytes()
                .map_err(|err| format!("failed to encode range capability: {err}"))?,
        });
    }
    if let Some(pq) = opts.soranet_pq {
        capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::SoraNetHybridPq,
            payload: pq
                .to_bytes()
                .map_err(|err| format!("failed to encode SoraNet PQ capability: {err}"))?,
        });
    }
    let endpoints: Vec<_> = opts
        .endpoints
        .into_iter()
        .map(|builder| builder.into_admission())
        .collect::<Result<_, _>>()?;
    let transport_hints = if opts.transport_hints.is_empty() {
        None
    } else {
        Some(opts.transport_hints)
    };
    let proposal = ProviderAdmissionProposalV1 {
        version: PROPOSAL_VERSION,
        provider_id,
        profile_id: canonical_handle,
        profile_aliases: Some(profile_aliases),
        stake: StakePointer {
            pool_id: stake_pool,
            stake_amount,
        },
        capabilities,
        endpoints,
        advert_key,
        por_vrf_key,
        jurisdiction_code: jurisdiction,
        contact_uri: opts.contact_uri,
        stream_budget: opts.stream_budget,
        transport_hints,
    };
    proposal
        .validate()
        .map_err(|err| format!("proposal validation failed: {err}"))?;
    let proposal_bytes =
        to_bytes(&proposal).map_err(|err| format!("failed to encode proposal: {err}"))?;
    if let Some(path) = opts.proposal_out.as_ref() {
        write_binary(path, &proposal_bytes)?;
    }
    let digest = compute_proposal_digest(&proposal)
        .map_err(|err| format!("failed to compute proposal digest: {err}"))?;
    let mut report = Map::new();
    report.insert("version".into(), Value::from(PROPOSAL_VERSION));
    report.insert(
        "provider_id_hex".into(),
        Value::from(encode_hex(&proposal.provider_id)),
    );
    report.insert(
        "profile_id".into(),
        Value::from(proposal.profile_id.clone()),
    );
    report.insert(
        "stake_amount".into(),
        Value::from(proposal.stake.stake_amount.to_string()),
    );
    report.insert(
        "capability_count".into(),
        Value::from(proposal.capabilities.len() as u64),
    );
    report.insert(
        "endpoint_count".into(),
        Value::from(proposal.endpoints.len() as u64),
    );
    report.insert(
        "proposal_digest_hex".into(),
        Value::from(encode_hex(&digest)),
    );
    report.insert(
        "proposal_len".into(),
        Value::from(proposal_bytes.len() as u64),
    );
    let mut json = to_string_pretty(&Value::Object(report))
        .map_err(|err| format!("failed to serialise JSON: {err}"))?;
    json.push('\n');
    if let Some(path) = opts.json_out.as_ref() {
        write_json(path, &json)?;
    } else {
        print!("{json}");
    }
    Ok(())
}
fn run_sign(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(sign_usage().to_string());
    }
    let mut opts = SignOptions::default();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--proposal" => opts.proposal_path = Some(PathBuf::from(value)),
            "--advert" => opts.advert_path = Some(PathBuf::from(value)),
            "--advert-body" => opts.advert_body_path = Some(PathBuf::from(value)),
            "--issued-at" => opts.issued_at = Some(parse_u64(value)?),
            "--retention-epoch" => opts.retention_epoch = Some(parse_u64(value)?),
            "--council-signature" => opts.signatures.push(parse_signature(value)?),
            "--council-signature-file" => {
                opts.signatures.push(parse_signature_file_entry(
                    value,
                    opts.signature_public.as_ref(),
                )?);
            }
            "--council-signature-public-key" => {
                let key_bytes = parse_hex_vec(value)?;
                if key_bytes.len() != 32 {
                    return Err("--council-signature-public-key must be 32 bytes".into());
                }
                opts.signature_public = Some(key_bytes);
            }
            "--council-signature-public-key-file" => {
                let key_bytes = read_file_bytes(value)?;
                if key_bytes.len() != 32 {
                    return Err("--council-signature-public-key-file must contain 32 bytes".into());
                }
                opts.signature_public = Some(key_bytes);
            }
            "--council-secret-key" => opts.secret_keys.push(parse_hex_vec(value)?),
            "--council-secret-key-file" => {
                opts.secret_keys.push(read_file_bytes(value)?);
            }
            "--notes" => opts.notes = Some(value.to_string()),
            "--envelope-out" => opts.envelope_out = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option: {other}")),
        }
    }
    let SignOptions {
        proposal_path,
        advert_path,
        advert_body_path,
        issued_at,
        retention_epoch,
        notes,
        mut signatures,
        signature_public: _,
        secret_keys,
        envelope_out,
        json_out,
    } = opts;
    let proposal_path = proposal_path.ok_or_else(|| "missing option --proposal".to_string())?;
    let proposal_bytes = read_file_bytes_path(&proposal_path)?;
    let proposal: ProviderAdmissionProposalV1 = decode_from_bytes(&proposal_bytes)
        .map_err(|err| format!("failed to decode proposal: {err}"))?;
    proposal
        .validate()
        .map_err(|err| format!("proposal validation failed: {err}"))?;
    let advert_path = advert_path.ok_or_else(|| "missing option --advert".to_string())?;
    let advert_bytes =
        read_file_bytes_path_bounded(&advert_path, PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1)?;
    let advert = decode_provider_advert_v1(&advert_bytes)
        .map_err(|err| format!("failed to decode provider advert: {err}"))?;
    advert
        .validate_with_body(advert.issued_at)
        .map_err(|err| format!("advert validation failed: {err}"))?;
    if let Some(body_path) = advert_body_path.as_ref() {
        let body_bytes =
            read_file_bytes_path_bounded(body_path, PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1)?;
        let expected_body =
            to_bytes(&advert.body).map_err(|err| format!("failed to encode advert body: {err}"))?;
        if body_bytes != expected_body {
            return Err("provided advert body does not match --advert".into());
        }
    }
    let proposal_digest = compute_proposal_digest(&proposal)
        .map_err(|err| format!("failed to compute proposal digest: {err}"))?;
    let advert_body = advert.body.clone();
    let advert_digest = compute_advert_body_digest(&advert_body)
        .map_err(|err| format!("failed to compute advert digest: {err}"))?;
    let issued_at = issued_at.unwrap_or_else(now_secs);
    let retention_epoch =
        retention_epoch.ok_or_else(|| "missing option --retention-epoch".to_string())?;
    let mut envelope = ProviderAdmissionEnvelopeV1 {
        version: ENVELOPE_VERSION,
        proposal,
        proposal_digest,
        advert_body: advert_body.clone(),
        advert_body_digest: advert_digest,
        issued_at,
        retention_epoch,
        council_signatures: Vec::new(),
        notes,
    };
    let authorization_digest = compute_envelope_authorization_digest(&envelope)
        .map_err(|err| format!("failed to compute envelope authorization digest: {err}"))?;
    for key_bytes in secret_keys {
        let signing_key = signing_key_from_bytes(&key_bytes)
            .map_err(|err| format!("invalid council secret key: {err}"))?;
        let signer = signing_key.verifying_key().to_bytes();
        let signature = signing_key.sign(&authorization_digest).to_bytes();
        signatures.push(CouncilSignature {
            signer,
            signature: signature.to_vec(),
        });
    }
    if signatures.is_empty() {
        return Err("at least one --council-signature is required".into());
    }
    signatures.sort_unstable_by_key(|signature| signature.signer);
    envelope.council_signatures = signatures.clone();
    let record = AdmissionRecord::new_untrusted_signers(envelope.clone())
        .map_err(|err| format!("envelope validation failed: {err}"))?;
    verify_advert_against_record(&advert, &record)
        .map_err(|err| format!("advert validation failed: {err}"))?;
    let envelope_bytes =
        to_bytes(&envelope).map_err(|err| format!("failed to encode envelope: {err}"))?;
    if let Some(path) = envelope_out.as_ref() {
        write_binary(path, &envelope_bytes)?;
    }
    let mut map = Map::new();
    map.insert("version".into(), Value::from(ENVELOPE_VERSION));
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(encode_hex(&envelope.proposal_digest)),
    );
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(encode_hex(&envelope.advert_body_digest)),
    );
    map.insert("issued_at".into(), Value::from(issued_at));
    map.insert("retention_epoch".into(), Value::from(retention_epoch));
    map.insert(
        "council_signature_count".into(),
        Value::from(signatures.len() as u64),
    );
    map.insert("signatures_integrity_verified".into(), Value::from(true));
    map.insert(
        "proposal_input".into(),
        Value::from(proposal_path.display().to_string()),
    );
    map.insert(
        "advert_input".into(),
        Value::from(advert_path.display().to_string()),
    );
    if let Some(body_path) = advert_body_path.as_ref() {
        map.insert(
            "advert_body_input".into(),
            Value::from(body_path.display().to_string()),
        );
    }
    let mut json = to_string_pretty(&Value::Object(map))
        .map_err(|err| format!("failed to serialise JSON: {err}"))?;
    json.push('\n');
    if let Some(path) = json_out.as_ref() {
        write_json(path, &json)?;
    } else {
        print!("{json}");
    }
    Ok(())
}
fn run_verify(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(verify_usage().to_string());
    }
    let mut opts = VerifyOptions::default();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--envelope" => opts.envelope_path = Some(PathBuf::from(value)),
            "--proposal" => opts.proposal_path = Some(PathBuf::from(value)),
            "--advert" => opts.advert_path = Some(PathBuf::from(value)),
            "--advert-body" => opts.advert_body_path = Some(PathBuf::from(value)),
            "--trusted-council-key" => {
                opts.trusted_council_keys
                    .push(parse_trusted_council_key(value)?);
            }
            "--signature-threshold" => {
                opts.signature_threshold = Some(
                    value
                        .parse::<usize>()
                        .map_err(|err| format!("invalid --signature-threshold: {err}"))?,
                );
            }
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option: {other}")),
        }
    }
    let signature_threshold = opts
        .signature_threshold
        .ok_or_else(|| "missing option --signature-threshold".to_string())?;
    let policy = ProviderAdmissionCouncilPolicy::new(
        opts.trusted_council_keys.iter().copied(),
        signature_threshold,
    )
    .map_err(|err| format!("invalid provider admission council policy: {err}"))?;
    let envelope_path = opts
        .envelope_path
        .ok_or_else(|| "missing option --envelope".to_string())?;
    let envelope_bytes = read_file_bytes_path(&envelope_path)?;
    let envelope: ProviderAdmissionEnvelopeV1 = decode_from_bytes(&envelope_bytes)
        .map_err(|err| format!("failed to decode envelope: {err}"))?;
    let record = AdmissionRecord::new(envelope.clone(), &policy)
        .map_err(|err| format!("envelope validation failed: {err}"))?;
    let mut proposal_match = None;
    if let Some(path) = opts.proposal_path.as_deref() {
        let bytes = read_file_bytes_path(path)?;
        let proposal: ProviderAdmissionProposalV1 =
            decode_from_bytes(&bytes).map_err(|err| format!("failed to decode proposal: {err}"))?;
        if proposal != envelope.proposal {
            return Err("provided proposal does not match envelope".into());
        }
        proposal_match = Some(true);
    }
    let mut advert_match = None;
    if let Some(path) = opts.advert_path.as_deref() {
        let bytes = read_file_bytes_path_bounded(path, PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1)?;
        let advert = decode_provider_advert_v1(&bytes)
            .map_err(|err| format!("failed to decode provider advert: {err}"))?;
        verify_advert_against_record(&advert, &record)
            .map_err(|err| format!("provided advert does not match envelope: {err}"))?;
        advert_match = Some(true);
    }
    let mut advert_body_match = None;
    if let Some(path) = opts.advert_body_path.as_deref() {
        let bytes = read_file_bytes_path_bounded(path, PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1)?;
        let expected_body = to_bytes(&envelope.advert_body)
            .map_err(|err| format!("failed to encode advert body: {err}"))?;
        if bytes != expected_body {
            return Err("provided advert body does not match envelope".into());
        }
        advert_body_match = Some(true);
    }
    let trusted_signatures_verified = true;
    let mut map = Map::new();
    map.insert("version".into(), Value::from(ENVELOPE_VERSION));
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(encode_hex(&envelope.proposal_digest)),
    );
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(encode_hex(&envelope.advert_body_digest)),
    );
    map.insert(
        "council_signature_count".into(),
        Value::from(envelope.council_signatures.len() as u64),
    );
    map.insert(
        "trusted_signatures_verified".into(),
        Value::from(trusted_signatures_verified),
    );
    if let Some(matched) = proposal_match {
        map.insert("proposal_match".into(), Value::from(matched));
    }
    if let Some(matched) = advert_match {
        map.insert("advert_match".into(), Value::from(matched));
    }
    if let Some(matched) = advert_body_match {
        map.insert("advert_body_match".into(), Value::from(matched));
    }
    let mut json = to_string_pretty(&Value::Object(map))
        .map_err(|err| format!("failed to serialise JSON: {err}"))?;
    json.push('\n');
    if let Some(path) = opts.json_out.as_ref() {
        write_json(path, &json)?;
    } else {
        print!("{json}");
    }
    Ok(())
}
fn run_renewal(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(renewal_usage().to_string());
    }
    let mut opts = RenewalOptions::default();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--previous-envelope" => opts.previous_envelope = Some(PathBuf::from(value)),
            "--envelope" => opts.envelope = Some(PathBuf::from(value)),
            "--notes" => opts.notes = Some(value.to_string()),
            "--renewal-out" => opts.renewal_out = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option: {other}")),
        }
    }
    let RenewalOptions {
        previous_envelope,
        envelope,
        renewal_out,
        json_out,
        notes,
    } = opts;
    let previous_path =
        previous_envelope.ok_or_else(|| "missing option --previous-envelope".to_string())?;
    let envelope_path = envelope.ok_or_else(|| "missing option --envelope".to_string())?;
    let previous_bytes = read_file_bytes_path(&previous_path)?;
    let previous_envelope: ProviderAdmissionEnvelopeV1 = decode_from_bytes(&previous_bytes)
        .map_err(|err| format!("failed to decode previous envelope: {err}"))?;
    let previous_record = AdmissionRecord::new_untrusted_signers(previous_envelope)
        .map_err(|err| format!("previous envelope validation failed: {err}"))?;
    let envelope_bytes = read_file_bytes_path(&envelope_path)?;
    let envelope: ProviderAdmissionEnvelopeV1 = decode_from_bytes(&envelope_bytes)
        .map_err(|err| format!("failed to decode renewal envelope: {err}"))?;
    let envelope_digest = compute_envelope_digest(&envelope)
        .map_err(|err| format!("failed to compute envelope digest: {err}"))?;
    let renewal = ProviderAdmissionRenewalV1 {
        version: RENEWAL_VERSION,
        provider_id: envelope.proposal.provider_id,
        previous_envelope_digest: *previous_record.envelope_digest(),
        envelope_digest,
        envelope,
        notes,
    };
    previous_record
        .apply_renewal_untrusted_signers(&renewal)
        .map_err(|err| format!("renewal validation failed: {err}"))?;
    let renewal_bytes =
        to_bytes(&renewal).map_err(|err| format!("failed to encode renewal: {err}"))?;
    if let Some(path) = renewal_out.as_ref() {
        write_binary(path, &renewal_bytes)?;
    }
    let mut map = Map::new();
    map.insert("version".into(), Value::from(RENEWAL_VERSION));
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode_hex(&renewal.provider_id)),
    );
    map.insert(
        "previous_envelope_digest_hex".into(),
        Value::from(encode_hex(&renewal.previous_envelope_digest)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(encode_hex(&renewal.envelope_digest)),
    );
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(encode_hex(&renewal.envelope.proposal_digest)),
    );
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(encode_hex(&renewal.envelope.advert_body_digest)),
    );
    map.insert("issued_at".into(), Value::from(renewal.envelope.issued_at));
    map.insert(
        "retention_epoch".into(),
        Value::from(renewal.envelope.retention_epoch),
    );
    map.insert(
        "stake_amount".into(),
        Value::from(renewal.envelope.proposal.stake.stake_amount.to_string()),
    );
    map.insert(
        "endpoint_count".into(),
        Value::from(renewal.envelope.proposal.endpoints.len() as u64),
    );
    map.insert(
        "council_signature_count".into(),
        Value::from(renewal.envelope.council_signatures.len() as u64),
    );
    if let Some(notes) = &renewal.notes {
        map.insert("notes".into(), Value::from(notes.clone()));
    }
    let mut json = to_string_pretty(&Value::Object(map))
        .map_err(|err| format!("failed to serialise JSON: {err}"))?;
    json.push('\n');
    if let Some(path) = json_out.as_ref() {
        write_json(path, &json)?;
    } else {
        print!("{json}");
    }
    Ok(())
}
fn run_revoke(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(revoke_usage().to_string());
    }
    let mut opts = RevocationOptions::default();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--envelope" => opts.envelope_path = Some(PathBuf::from(value)),
            "--reason" => opts.reason = Some(value.to_string()),
            "--revoked-at" => opts.revoked_at = Some(parse_u64(value)?),
            "--notes" => opts.notes = Some(value.to_string()),
            "--council-signature" => opts.signatures.push(parse_signature(value)?),
            "--council-signature-file" => opts.signatures.push(parse_signature_file_entry(
                value,
                opts.signature_public.as_ref(),
            )?),
            "--council-signature-public-key" => {
                opts.signature_public = Some(parse_hex_vec(value)?);
            }
            "--council-signature-public-key-file" => {
                opts.signature_public = Some(read_file_bytes(value)?);
            }
            "--council-secret-key" => opts.secret_keys.push(parse_hex_vec(value)?),
            "--council-secret-key-file" => {
                opts.secret_keys.push(read_file_bytes(value)?);
            }
            "--revocation-out" => opts.revocation_out = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option: {other}")),
        }
    }
    let RevocationOptions {
        envelope_path,
        reason,
        revoked_at,
        notes,
        mut signatures,
        signature_public: _,
        secret_keys,
        revocation_out,
        json_out,
    } = opts;
    let envelope_path = envelope_path.ok_or_else(|| "missing option --envelope".to_string())?;
    let reason = reason.ok_or_else(|| "missing option --reason".to_string())?;
    let envelope_bytes = read_file_bytes_path(&envelope_path)?;
    let envelope: ProviderAdmissionEnvelopeV1 = decode_from_bytes(&envelope_bytes)
        .map_err(|err| format!("failed to decode envelope: {err}"))?;
    let record = AdmissionRecord::new_untrusted_signers(envelope)
        .map_err(|err| format!("envelope validation failed: {err}"))?;
    let revoked_at = revoked_at.unwrap_or_else(now_secs);
    let mut revocation = ProviderAdmissionRevocationV1 {
        version: REVOCATION_VERSION,
        provider_id: *record.provider_id(),
        envelope_digest: *record.envelope_digest(),
        revoked_at,
        reason: reason.clone(),
        council_signatures: Vec::new(),
        notes: notes.clone(),
    };
    let digest = revocation
        .digest()
        .map_err(|err| format!("failed to compute revocation digest: {err}"))?;
    for key_bytes in secret_keys {
        let signing_key = signing_key_from_bytes(&key_bytes)
            .map_err(|err| format!("invalid council secret key: {err}"))?;
        let signer = signing_key.verifying_key().to_bytes();
        let signature = signing_key.sign(&digest).to_bytes();
        signatures.push(CouncilSignature {
            signer,
            signature: signature.to_vec(),
        });
    }
    if signatures.is_empty() {
        return Err("at least one --council-signature is required".into());
    }
    revocation.council_signatures = signatures.clone();
    verify_revocation_signatures_untrusted_signers(&revocation)
        .map_err(|err| format!("revocation validation failed: {err}"))?;
    record
        .verify_revocation_untrusted_signers(&revocation)
        .map_err(|err| format!("revocation does not match envelope: {err}"))?;
    let revocation_bytes =
        to_bytes(&revocation).map_err(|err| format!("failed to encode revocation: {err}"))?;
    if let Some(path) = revocation_out.as_ref() {
        write_binary(path, &revocation_bytes)?;
    }
    let mut map = Map::new();
    map.insert("version".into(), Value::from(REVOCATION_VERSION));
    map.insert(
        "provider_id_hex".into(),
        Value::from(encode_hex(&revocation.provider_id)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(encode_hex(&revocation.envelope_digest)),
    );
    map.insert("revoked_at".into(), Value::from(revocation.revoked_at));
    map.insert("reason".into(), Value::from(revocation.reason.clone()));
    map.insert(
        "council_signature_count".into(),
        Value::from(revocation.council_signatures.len() as u64),
    );
    map.insert(
        "revocation_digest_hex".into(),
        Value::from(encode_hex(&digest)),
    );
    if let Some(notes) = &revocation.notes {
        map.insert("notes".into(), Value::from(notes.clone()));
    }
    let mut json = to_string_pretty(&Value::Object(map))
        .map_err(|err| format!("failed to serialise JSON: {err}"))?;
    json.push('\n');
    if let Some(path) = json_out.as_ref() {
        write_json(path, &json)?;
    } else {
        print!("{json}");
    }
    Ok(())
}
fn proposal_usage() -> &'static str {
    "usage: sorafs_manifest_builder provider-admission proposal --provider-id=<hex32> \
        --chunker-profile=<handle> --stake-pool-id=<hex32> --stake-amount=<canonical_xor_quantity> \
        --advert-key=<hex32> --por-vrf-key=<normal:hex48|small:hex96> \
        --jurisdiction-code=<ISO3166-1> --endpoint=<kind:host> \
        [--endpoint-attestation-kind=<kind>] \
        --endpoint-attestation-attested-at=<secs> --endpoint-attestation-expires-at=<secs> \
        (--endpoint-attestation-leaf=<path>|--endpoint-attestation-leaf-hex=<lowercase_hex>) \
        [--endpoint-attestation-intermediate=<path>]... \
        [--endpoint-attestation-intermediate-hex=<lowercase_hex>]... \
        [--endpoint-attestation-alpn=<id>]... [--endpoint-attestation-report=<path>] \
        [--endpoint-attestation-report-hex=<lowercase_hex>] [--capability=<spec>] \
        [--range-capability=max_span=...,min_granularity=...[,sparse=bool,...]] \
        [--soranet-pq=guard|majority|strict] \
        [--stream-budget=max_in_flight=...,max_bytes_per_sec=...[,burst=...]] \
        [--transport-hint=protocol:priority] [--proposal-out=<path>] \
        [--json-out=<path>]"
}
fn sign_usage() -> &'static str {
    "usage: sorafs_manifest_builder provider-admission sign --proposal=<path> --advert=<path> \
        --retention-epoch=<epoch> [--issued-at=<secs>] --council-signature=<signer_hex:signature_hex> \
        [--advert-body=<path>] [--council-signature-file=<path>] \
        [--council-signature-public-key=<hex32>|--council-signature-public-key-file=<path>] \
        [--notes=<text>] [--envelope-out=<path>] [--json-out=<path>]"
}
fn verify_usage() -> &'static str {
    "usage: sorafs_manifest_builder provider-admission verify --envelope=<path> \
        --trusted-council-key=<hex32>... --signature-threshold=<count> [--proposal=<path>] \
        [--advert=<path>] [--advert-body=<path>] [--json-out=<path>]"
}
fn renewal_usage() -> &'static str {
    "usage: sorafs_manifest_builder provider-admission renewal --previous-envelope=<path> \
        --envelope=<path> [--notes=<text>] [--renewal-out=<path>] [--json-out=<path>]"
}
fn revoke_usage() -> &'static str {
    "usage: sorafs_manifest_builder provider-admission revoke --envelope=<path> --reason=<text> \
        [--revoked-at=<secs>] --council-signature=<signer_hex:signature_hex> \
        [--council-signature-file=<path>] \
        [--council-signature-public-key=<hex32>|--council-signature-public-key-file=<path>] \
        [--notes=<text>] [--revocation-out=<path>] [--json-out=<path>]"
}
#[derive(Default)]
struct ProposalOptions {
    provider_id: Option<[u8; 32]>,
    profile_handle: Option<String>,
    stake_pool_id: Option<[u8; 32]>,
    stake_amount: Option<XorQuantity>,
    advert_key: Option<[u8; 32]>,
    por_vrf_key: Option<ProviderVrfPublicKeyV1>,
    jurisdiction: Option<String>,
    contact_uri: Option<String>,
    capabilities: Vec<CapabilityTlv>,
    range_capability: Option<ProviderCapabilityRangeV1>,
    soranet_pq: Option<ProviderCapabilitySoranetPqV1>,
    endpoints: Vec<EndpointBuilder>,
    stream_budget: Option<StreamBudgetV1>,
    transport_hints: Vec<TransportHintV1>,
    proposal_out: Option<PathBuf>,
    json_out: Option<PathBuf>,
}
#[derive(Default)]
struct SignOptions {
    proposal_path: Option<PathBuf>,
    advert_path: Option<PathBuf>,
    advert_body_path: Option<PathBuf>,
    issued_at: Option<u64>,
    retention_epoch: Option<u64>,
    notes: Option<String>,
    signatures: Vec<CouncilSignature>,
    signature_public: Option<Vec<u8>>,
    secret_keys: Vec<Vec<u8>>,
    envelope_out: Option<PathBuf>,
    json_out: Option<PathBuf>,
}
#[derive(Default)]
struct VerifyOptions {
    envelope_path: Option<PathBuf>,
    proposal_path: Option<PathBuf>,
    advert_path: Option<PathBuf>,
    advert_body_path: Option<PathBuf>,
    trusted_council_keys: Vec<[u8; 32]>,
    signature_threshold: Option<usize>,
    json_out: Option<PathBuf>,
}
#[derive(Default)]
struct RenewalOptions {
    previous_envelope: Option<PathBuf>,
    envelope: Option<PathBuf>,
    renewal_out: Option<PathBuf>,
    json_out: Option<PathBuf>,
    notes: Option<String>,
}
#[derive(Default)]
struct RevocationOptions {
    envelope_path: Option<PathBuf>,
    reason: Option<String>,
    revoked_at: Option<u64>,
    notes: Option<String>,
    signatures: Vec<CouncilSignature>,
    signature_public: Option<Vec<u8>>,
    secret_keys: Vec<Vec<u8>>,
    revocation_out: Option<PathBuf>,
    json_out: Option<PathBuf>,
}
struct EndpointBuilder {
    endpoint: AdvertEndpoint,
    attestation_kind: Option<EndpointAttestationKind>,
    attested_at: Option<u64>,
    expires_at: Option<u64>,
    leaf: Option<Vec<u8>>,
    intermediates: Vec<Vec<u8>>,
    alpn: Vec<String>,
    report: Option<Vec<u8>>,
}
impl EndpointBuilder {
    fn new(endpoint: AdvertEndpoint) -> Self {
        let kind = match endpoint.kind {
            EndpointKind::Torii | EndpointKind::NoritoRpc => EndpointAttestationKind::Mtls,
            EndpointKind::Quic => EndpointAttestationKind::Quic,
        };
        Self {
            endpoint,
            attestation_kind: Some(kind),
            attested_at: None,
            expires_at: None,
            leaf: None,
            intermediates: Vec::new(),
            alpn: Vec::new(),
            report: None,
        }
    }
    fn into_admission(self) -> Result<EndpointAdmissionV1, String> {
        let kind = self
            .attestation_kind
            .ok_or_else(|| "missing --endpoint-attestation-kind for endpoint".to_string())?;
        let attested_at = self
            .attested_at
            .ok_or_else(|| "missing --endpoint-attestation-attested-at for endpoint".to_string())?;
        let expires_at = self
            .expires_at
            .ok_or_else(|| "missing --endpoint-attestation-expires-at for endpoint".to_string())?;
        let leaf = self
            .leaf
            .ok_or_else(|| "missing --endpoint-attestation-leaf for endpoint".to_string())?;
        Ok(EndpointAdmissionV1 {
            endpoint: self.endpoint,
            attestation: EndpointAttestationV1 {
                version: ENDPOINT_ATTESTATION_VERSION_V1,
                kind,
                attested_at,
                expires_at,
                leaf_certificate: leaf,
                intermediate_certificates: self.intermediates,
                alpn_ids: self.alpn,
                report: self.report.unwrap_or_default(),
            },
        })
    }
}
fn lookup_profile(
    opts: &ProposalOptions,
) -> Result<&'static chunker_registry::ChunkerProfileDescriptor, String> {
    let handle = opts
        .profile_handle
        .as_ref()
        .ok_or_else(|| "missing option --chunker-profile".to_string())?;
    let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
        format!("unknown chunker profile handle `{handle}`; use --list-chunker-profiles")
    })?;
    let canonical = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    if handle != &canonical {
        return Err(format!(
            "chunker profile handle `{handle}` is not canonical; expected `{canonical}`"
        ));
    }
    Ok(descriptor)
}
fn current_endpoint<'a>(
    endpoints: &'a mut [EndpointBuilder],
    flag: &str,
) -> Result<&'a mut EndpointBuilder, String> {
    endpoints
        .last_mut()
        .ok_or_else(|| format!("{flag} requires at least one preceding --endpoint"))
}
fn parse_provider_vrf_key(value: &str) -> Result<ProviderVrfPublicKeyV1, String> {
    require_no_ascii_whitespace(value, "provider VRF key")?;
    let (variant, encoded) = value.split_once(':').ok_or_else(|| {
        "provider VRF key must be `normal:<hex48>` or `small:<hex96>`".to_string()
    })?;
    let bytes = parse_hex_vec(encoded)?;
    let key = match variant {
        "normal" => {
            ProviderVrfPublicKeyV1::BlsNormal(bytes.try_into().map_err(|bytes: Vec<u8>| {
                format!("normal VRF key must be 48 bytes, found {}", bytes.len())
            })?)
        }
        "small" => {
            ProviderVrfPublicKeyV1::BlsSmall(bytes.try_into().map_err(|bytes: Vec<u8>| {
                format!("small VRF key must be 96 bytes, found {}", bytes.len())
            })?)
        }
        _ => return Err("provider VRF key variant must be `normal` or `small`".to_string()),
    };
    key.validate()
        .map_err(|err| format!("invalid provider VRF key: {err}"))?;
    Ok(key)
}
fn parse_capability(value: &str) -> Result<CapabilityTlv, String> {
    let (head, payload) = value
        .split_once(':')
        .map_or((value, None), |(h, rest)| (h, Some(rest)));
    if head.is_empty() {
        return Err("capability type must not be empty".to_string());
    }
    require_no_ascii_whitespace(head, "capability type")?;
    let cap_type = match head {
        "torii" => CapabilityType::ToriiGateway,
        "quic" => CapabilityType::QuicNoise,
        "potr-mldsa" => CapabilityType::PotrMlDsa,
        "range" => {
            return Err(
                "use --range-capability=<key=value,...> to describe chunk-range support".into(),
            );
        }
        "soranet" | "soranet-pq" => {
            return Err("use --soranet-pq=<guard|majority|strict>".into());
        }
        "vendor" => CapabilityType::VendorReserved,
        other => {
            return Err(format!(
                "unknown capability `{other}` (expected torii|quic|potr-mldsa|vendor; use --range-capability or --soranet-pq for structured capabilities)"
            ));
        }
    };
    let payload_bytes = match (cap_type, payload) {
        (CapabilityType::PotrMlDsa, Some(rest)) => {
            require_capability_payload(rest)?;
            parse_hex_vec(rest)?
        }
        (CapabilityType::PotrMlDsa, None) => {
            return Err("potr-mldsa capability requires a hex ML-DSA-65 public key".to_owned());
        }
        (_, Some(rest)) => {
            require_capability_payload(rest)?;
            parse_hex_vec(rest)?
        }
        _ => Vec::new(),
    };
    Ok(CapabilityTlv {
        cap_type,
        payload: payload_bytes,
    })
}
fn parse_soranet_pq(value: &str) -> Result<ProviderCapabilitySoranetPqV1, String> {
    let capability = match value {
        "guard" => ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: false,
            supports_strict: false,
        },
        "majority" => ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: true,
            supports_strict: false,
        },
        "strict" => ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: true,
            supports_strict: true,
        },
        other => {
            return Err(format!(
                "unknown soranet-pq level `{other}` (expected exactly guard|majority|strict)"
            ));
        }
    };
    capability
        .validate()
        .map_err(|err| format!("invalid soranet-pq capability: {err}"))?;
    Ok(capability)
}
fn parse_range_capability(value: &str) -> Result<ProviderCapabilityRangeV1, String> {
    let mut max_span = None;
    let mut min_granularity = None;
    let mut sparse = None;
    let mut alignment = None;
    let mut merkle = None;
    for part in value.split(',') {
        if part.is_empty() {
            return Err("range-capability must not contain empty entries".to_string());
        }
        require_no_ascii_whitespace(part, "range-capability entry")?;
        let (key, raw) = part
            .split_once('=')
            .ok_or_else(|| format!("range-capability requires key=value entries, got: {part}"))?;
        match key {
            "max_span" => {
                if max_span.is_some() {
                    return Err("range-capability field max_span specified multiple times".into());
                }
                max_span = Some(parse_u32(raw)?);
            }
            "min_granularity" => {
                if min_granularity.is_some() {
                    return Err(
                        "range-capability field min_granularity specified multiple times".into(),
                    );
                }
                min_granularity = Some(parse_u32(raw)?);
            }
            "sparse" => {
                if sparse.is_some() {
                    return Err("range-capability field sparse specified multiple times".into());
                }
                sparse = Some(parse_bool(raw)?);
            }
            "alignment" => {
                if alignment.is_some() {
                    return Err("range-capability field alignment specified multiple times".into());
                }
                alignment = Some(parse_bool(raw)?);
            }
            "merkle" => {
                if merkle.is_some() {
                    return Err("range-capability field merkle specified multiple times".into());
                }
                merkle = Some(parse_bool(raw)?);
            }
            other => {
                return Err(format!(
                    "unknown range-capability field: {other} (expected max_span|min_granularity|sparse|alignment|merkle)"
                ));
            }
        }
    }
    let capability = ProviderCapabilityRangeV1 {
        max_chunk_span: max_span
            .ok_or_else(|| "range-capability requires max_span=<u32>".to_string())?,
        min_granularity: min_granularity
            .ok_or_else(|| "range-capability requires min_granularity=<u32>".to_string())?,
        supports_sparse_offsets: sparse.unwrap_or(false),
        requires_alignment: alignment.unwrap_or(false),
        supports_merkle_proof: merkle.unwrap_or(false),
    };
    capability
        .validate()
        .map_err(|err| format!("invalid range capability: {err}"))?;
    Ok(capability)
}
fn parse_endpoint(value: &str) -> Result<AdvertEndpoint, String> {
    let (kind_str, host) = value
        .split_once(':')
        .ok_or_else(|| "endpoint requires kind:host".to_string())?;
    let kind = match kind_str {
        "torii" => EndpointKind::Torii,
        "quic" => EndpointKind::Quic,
        "norito-rpc" => EndpointKind::NoritoRpc,
        other => return Err(format!("unknown endpoint kind: {other}")),
    };
    Ok(AdvertEndpoint {
        kind,
        host_pattern: host.to_string(),
        metadata: Vec::new(),
    })
}
fn parse_stream_budget(value: &str) -> Result<StreamBudgetV1, String> {
    let mut max_in_flight = None;
    let mut max_bytes_per_sec = None;
    let mut burst_bytes = None;
    for part in value.split(',') {
        if part.is_empty() {
            return Err("stream-budget must not contain empty entries".to_string());
        }
        require_no_ascii_whitespace(part, "stream-budget entry")?;
        let (key, raw) = part
            .split_once('=')
            .ok_or_else(|| format!("stream-budget requires key=value entries, got: {part}"))?;
        match key {
            "max_in_flight" => {
                if max_in_flight.is_some() {
                    return Err("stream-budget field max_in_flight specified multiple times".into());
                }
                max_in_flight = Some(parse_u16(raw)?);
            }
            "max_bytes_per_sec" => {
                if max_bytes_per_sec.is_some() {
                    return Err(
                        "stream-budget field max_bytes_per_sec specified multiple times".into(),
                    );
                }
                max_bytes_per_sec = Some(parse_u64(raw)?);
            }
            "burst" => {
                if burst_bytes.is_some() {
                    return Err("stream-budget field burst specified multiple times".into());
                }
                burst_bytes = Some(parse_u64(raw)?);
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
    require_no_ascii_whitespace(protocol_str, "transport-hint protocol")?;
    require_no_ascii_whitespace(priority_str, "transport-hint priority")?;
    let protocol = parse_transport_protocol(protocol_str)?;
    let priority = parse_u8(priority_str)?;
    let hint = TransportHintV1 { protocol, priority };
    hint.validate()
        .map_err(|err| format!("invalid transport hint: {err}"))?;
    Ok(hint)
}
fn parse_transport_protocol(value: &str) -> Result<TransportProtocol, String> {
    match value {
        "torii" => Ok(TransportProtocol::ToriiHttpRange),
        "quic" => Ok(TransportProtocol::QuicStream),
        "soranet" => Ok(TransportProtocol::SoraNetRelay),
        "vendor" => Ok(TransportProtocol::VendorReserved),
        other => Err(format!(
            "unknown transport protocol: {other} (expected torii|quic|soranet|vendor)"
        )),
    }
}
fn parse_u8(value: &str) -> Result<u8, String> {
    let parsed = parse_u16(value)?;
    u8::try_from(parsed).map_err(|_| format!("u8 value out of range: {parsed}"))
}
fn parse_jurisdiction_code(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err("jurisdiction code must not be empty".to_string());
    }
    require_no_ascii_whitespace(value, "jurisdiction code")?;
    if value.len() != 2 || !value.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(
            "jurisdiction code must be a canonical ISO-3166 alpha-2 uppercase token".to_string(),
        );
    }
    Ok(value.to_string())
}
fn require_capability_payload(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("capability payload must not be empty".to_string());
    }
    require_no_ascii_whitespace(value, "capability payload")
}
fn require_no_ascii_whitespace(value: &str, label: &str) -> Result<(), String> {
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        Err(format!("{label} must not contain ASCII whitespace"))
    } else {
        Ok(())
    }
}
fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected boolean true|false, got {other}")),
    }
}
fn parse_attestation_kind(value: &str) -> Result<EndpointAttestationKind, String> {
    match value {
        "mtls" => Ok(EndpointAttestationKind::Mtls),
        "quic" => Ok(EndpointAttestationKind::Quic),
        other => Err(format!("unknown attestation kind: {other}")),
    }
}
fn parse_signature(value: &str) -> Result<CouncilSignature, String> {
    super::parse_signature_hex(value)
}
fn parse_trusted_council_key(value: &str) -> Result<[u8; 32], String> {
    let bytes = parse_hex_vec(value)?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        format!("trusted council key must be 32 bytes, got {}", bytes.len())
    })
}
fn parse_signature_file_entry(
    value: &str,
    default_signer: Option<&Vec<u8>>,
) -> Result<CouncilSignature, String> {
    if value.contains(':') {
        return super::parse_signature_file(value);
    }
    let signer_bytes = default_signer.ok_or_else(|| {
        "provide --council-signature-public-key before --council-signature-file without signer"
            .to_string()
    })?;
    let signature = read_file_bytes(value)?;
    super::build_council_signature(signer_bytes.clone(), signature)
}
fn signing_key_from_bytes(bytes: &[u8]) -> Result<SigningKey, String> {
    if bytes.len() != 32 {
        return Err("council secret key must be 32 bytes".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(bytes);
    if seed.iter().all(|byte| *byte == 0) {
        return Err("council secret key material must not be all zero".into());
    }
    Ok(SigningKey::from_bytes(&seed))
}
fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn read_file_bytes_path(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("failed to read {path:?}: {err}"))
}
fn read_file_bytes_path_bounded(path: &Path, maximum_bytes: usize) -> Result<Vec<u8>, String> {
    let maximum_u64 =
        u64::try_from(maximum_bytes).map_err(|_| "input byte ceiling exceeds u64".to_owned())?;
    let file = fs::File::open(path).map_err(|err| format!("failed to open {path:?}: {err}"))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to inspect {path:?}: {err}"))?;
    if !metadata.is_file() || metadata.len() > maximum_u64 {
        return Err(format!(
            "{path:?} is not a regular file within the {maximum_bytes}-byte input ceiling"
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| format!("{path:?} is too large"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(maximum_u64.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read {path:?}: {err}"))?;
    if bytes.len() > maximum_bytes {
        return Err(format!(
            "{path:?} exceeds the {maximum_bytes}-byte input ceiling"
        ));
    }
    Ok(bytes)
}
fn parse_xor_quantity(value: &str) -> Result<XorQuantity, String> {
    let quantity = value
        .parse::<XorQuantity>()
        .map_err(|err| format!("invalid XOR quantity `{value}`: {err}"))?;
    if quantity.to_string() != value {
        return Err(format!(
            "XOR quantity `{value}` must use the canonical decimal spelling"
        ));
    }
    Ok(quantity)
}
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signing_key_from_bytes_rejects_all_zero_seed_material() {
        match signing_key_from_bytes(&[0u8; 32]) {
            Err(err) => assert!(err.contains("all zero"), "unexpected error: {err}"),
            Ok(_) => panic!("all-zero council signing seed must fail"),
        }
    }
    #[test]
    fn bounded_provider_advert_reader_accepts_boundary_and_rejects_one_over() {
        let directory = tempfile::tempdir().expect("temporary advert directory");
        let path = directory.path().join("advert.to");
        fs::write(&path, vec![0xA5; PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1])
            .expect("write exact-boundary advert");
        assert_eq!(
            read_file_bytes_path_bounded(&path, PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1)
                .expect("read exact-boundary advert")
                .len(),
            PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1
        );
        fs::write(
            &path,
            vec![0xA5; PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1 + 1],
        )
        .expect("write one-over advert");
        assert!(
            read_file_bytes_path_bounded(&path, PROVIDER_ADVERT_MAX_CANONICAL_BYTES_V1).is_err()
        );
    }
    #[test]
    fn stake_amount_parser_is_exact_canonical_and_not_u128_bounded() {
        assert_eq!(
            parse_xor_quantity("0.000000001")
                .expect("sub-micro quantity")
                .to_string(),
            "0.000000001"
        );
        assert_eq!(
            parse_xor_quantity("340282366920938463463374607431768211456")
                .expect("quantity wider than u128")
                .to_string(),
            "340282366920938463463374607431768211456"
        );
        for value in ["", "01", "1.0", "+1", "-1", " 1", "1 "] {
            parse_xor_quantity(value).expect_err("noncanonical XOR quantity must fail");
        }
    }
    #[test]
    fn proposal_structured_tokens_reject_whitespace_and_noncanonical_forms() {
        assert_eq!(parse_jurisdiction_code("US").expect("jurisdiction"), "US");
        assert_eq!(parse_u8("0xff").expect("hex priority"), 255);
        assert_eq!(parse_u8("7").expect("decimal priority"), 7);
        for value in ["", "us", " USA", "U S", "USA"] {
            let err = parse_jurisdiction_code(value).expect_err("invalid jurisdiction must fail");
            assert!(
                err.contains("empty")
                    || err.contains("ASCII whitespace")
                    || err.contains("uppercase"),
                "unexpected jurisdiction error for {value:?}: {err}"
            );
        }
        for value in ["01", "0X1", "0x01", "256"] {
            let err = parse_u8(value).expect_err("invalid priority must fail");
            assert!(
                err.contains("canonical unsigned") || err.contains("out of range"),
                "unexpected priority error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn parse_capability_rejects_noncanonical_payloads_and_aliases() {
        assert_eq!(
            parse_capability("torii")
                .expect("canonical Torii capability")
                .cap_type,
            CapabilityType::ToriiGateway
        );
        for value in [
            " range:64",
            "range: 64",
            "range:064",
            "range:",
            "chunk-range",
            "torii-gateway",
            "quic-noise",
            "potr_mldsa:11",
            "TORII",
            "soranet:",
            "soranet:guard, strict",
            "soranet:guard,,strict",
            "soranet_pq:guard",
            "soranet-hybrid-pq:guard",
            "torii: 0a",
        ] {
            let err = parse_capability(value).expect_err("invalid capability must fail");
            assert!(
                err.contains("empty")
                    || err.contains("ASCII whitespace")
                    || err.contains("use --range-capability")
                    || err.contains("use --soranet-pq")
                    || err.contains("unknown capability"),
                "unexpected capability error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn structured_range_capability_validates_in_a_real_proposal() {
        let range = parse_range_capability(
            "max_span=32,min_granularity=8,sparse=true,alignment=false,merkle=true",
        )
        .expect("canonical range capability");
        let range_tlv = CapabilityTlv {
            cap_type: CapabilityType::ChunkRangeFetch,
            payload: range.to_bytes().expect("encode range capability"),
        };
        let descriptor =
            chunker_registry::lookup_by_handle("sorafs.sf1@1.0.0").expect("registered profile");
        let aliases = descriptor
            .aliases
            .iter()
            .map(|alias| (*alias).to_owned())
            .collect();
        let signing_key = SigningKey::from_bytes(&[0x33; 32]);
        let (vrf_public, vrf_private) =
            iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![0x34; 32]))
                .expect("fixture BLS keypair");
        let vrf_pair: iroha_crypto::KeyPair = (vrf_public, vrf_private).into();
        let proposal = ProviderAdmissionProposalV1 {
            version: PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id: [0x11; 32],
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(aliases),
            stake: StakePointer {
                pool_id: [0x22; 32],
                stake_amount: parse_xor_quantity("1").expect("positive stake"),
            },
            capabilities: vec![
                parse_capability("torii").expect("canonical Torii capability"),
                range_tlv,
            ],
            endpoints: vec![EndpointAdmissionV1 {
                endpoint: parse_endpoint("torii:storage.example.com").expect("canonical endpoint"),
                attestation: EndpointAttestationV1 {
                    version: ENDPOINT_ATTESTATION_VERSION_V1,
                    kind: EndpointAttestationKind::Mtls,
                    attested_at: 1,
                    expires_at: 2,
                    leaf_certificate: vec![1],
                    intermediate_certificates: Vec::new(),
                    alpn_ids: vec!["h2".to_owned()],
                    report: Vec::new(),
                },
            }],
            advert_key: signing_key.verifying_key().to_bytes(),
            por_vrf_key: ProviderVrfPublicKeyV1::BlsNormal(
                vrf_pair
                    .public_key()
                    .to_bytes()
                    .1
                    .try_into()
                    .expect("normal BLS public key is 48 bytes"),
            ),
            jurisdiction_code: "US".to_owned(),
            contact_uri: None,
            stream_budget: Some(
                parse_stream_budget("max_in_flight=2,max_bytes_per_sec=1024,burst=512")
                    .expect("canonical stream budget"),
            ),
            transport_hints: Some(vec![
                parse_transport_hint("torii:0").expect("canonical transport hint"),
            ]),
        };
        proposal
            .validate()
            .expect("structured range capability must pass proposal validation");
    }
    #[test]
    fn structured_selector_parsers_reject_compatibility_aliases() {
        for value in ["guard", "majority", "strict"] {
            parse_soranet_pq(value).expect("canonical SoraNet PQ level");
        }
        for value in ["torii", "quic", "soranet", "vendor"] {
            parse_transport_protocol(value).expect("canonical transport");
        }
        for value in [
            "torii:storage.example",
            "quic:storage.example",
            "norito-rpc:storage.example",
        ] {
            parse_endpoint(value).expect("canonical endpoint");
        }
        for value in ["mtls", "quic"] {
            parse_attestation_kind(value).expect("canonical attestation kind");
        }
        assert!(parse_bool("true").expect("canonical true"));
        assert!(!parse_bool("false").expect("canonical false"));
        for value in [
            "",
            "Guard",
            " guard",
            "guard ",
            "stage-a",
            "stagea",
            "guard+strict",
            "guard,strict",
            "guard|strict",
        ] {
            parse_soranet_pq(value).expect_err("SoraNet PQ alias must fail");
        }
        for value in [
            "max-chunk-span=32,min_granularity=8",
            "max_chunk_span=32,min_granularity=8",
            "max_span=32,min-granularity=8",
            "max_span=32,min_granularity=8,supports_sparse_offsets=true",
            "MAX_SPAN=32,min_granularity=8",
        ] {
            parse_range_capability(value).expect_err("range field alias must fail");
        }
        for value in [
            "max-in-flight=2,max_bytes_per_sec=1024",
            "inflight=2,max_bytes_per_sec=1024",
            "max_in_flight=2,max-bytes-per-sec=1024",
            "max_in_flight=2,max_rate=1024",
            "max_in_flight=2,max_bytes_per_sec=1024,burst_bytes=512",
        ] {
            parse_stream_budget(value).expect_err("stream field alias must fail");
        }
        for value in [
            "torii-http",
            "torii_http",
            "quic-stream",
            "quic_stream",
            "relay",
            "soranet-relay",
            "soranet_relay",
            "vendor-reserved",
            "vendor_reserved",
            "TORII",
        ] {
            parse_transport_protocol(value).expect_err("transport alias must fail");
        }
        for value in [
            "noritorpc:storage.example",
            "TORII:storage.example",
            " torii:storage.example",
        ] {
            parse_endpoint(value).expect_err("endpoint alias must fail");
        }
        for value in ["tls", "MTLS", "Quic"] {
            parse_attestation_kind(value).expect_err("attestation alias must fail");
        }
        for value in ["1", "0", "yes", "no", "TRUE", "False"] {
            parse_bool(value).expect_err("boolean alias must fail");
        }
    }
    #[test]
    fn profile_lookup_requires_the_exact_canonical_handle() {
        let canonical = ProposalOptions {
            profile_handle: Some("sorafs.sf1@1.0.0".to_owned()),
            ..ProposalOptions::default()
        };
        lookup_profile(&canonical).expect("canonical profile handle");
        for handle in ["sorafs/sf1@1.0.0", "sorafs-sf1", "1", "SORAFS.SF1@1.0.0"] {
            let opts = ProposalOptions {
                profile_handle: Some(handle.to_owned()),
                ..ProposalOptions::default()
            };
            lookup_profile(&opts).expect_err("profile selector alias must fail");
        }
    }
    #[test]
    fn structured_selectors_reject_duplicate_fields() {
        for value in [
            "max_span=32,max_span=32,min_granularity=8",
            "max_span=32,min_granularity=8,sparse=true,sparse=false",
            "max_span=32,min_granularity=8,alignment=true,alignment=false",
            "max_span=32,min_granularity=8,merkle=true,merkle=false",
        ] {
            let err = parse_range_capability(value).expect_err("duplicate range field must fail");
            assert!(err.contains("multiple times"), "unexpected error: {err}");
        }
        for value in [
            "max_in_flight=2,max_in_flight=3,max_bytes_per_sec=1024",
            "max_in_flight=2,max_bytes_per_sec=1024,max_bytes_per_sec=2048",
            "max_in_flight=2,max_bytes_per_sec=1024,burst=512,burst=256",
        ] {
            let err = parse_stream_budget(value).expect_err("duplicate budget field must fail");
            assert!(err.contains("multiple times"), "unexpected error: {err}");
        }
    }
    #[test]
    fn parse_stream_budget_rejects_ambiguous_entries() {
        let budget = parse_stream_budget("max_in_flight=2,max_bytes_per_sec=1024,burst=512")
            .expect("stream budget");
        assert_eq!(budget.max_in_flight, 2);
        assert_eq!(budget.max_bytes_per_sec, 1024);
        assert_eq!(budget.burst_bytes, Some(512));
        for value in [
            "max_in_flight=2,,max_bytes_per_sec=1024",
            "max_in_flight =2,max_bytes_per_sec=1024",
            "max_in_flight= 2,max_bytes_per_sec=1024",
            "max_in_flight=02,max_bytes_per_sec=1024",
            "max_in_flight=2,max_bytes_per_sec=0x010",
        ] {
            let err = parse_stream_budget(value).expect_err("invalid stream budget must fail");
            assert!(
                err.contains("empty")
                    || err.contains("ASCII whitespace")
                    || err.contains("canonical unsigned"),
                "unexpected stream budget error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn parse_transport_hint_rejects_noncanonical_priority() {
        let hint = parse_transport_hint("torii:1").expect("transport hint");
        assert_eq!(hint.protocol, TransportProtocol::ToriiHttpRange);
        assert_eq!(hint.priority, 1);
        for value in [
            " torii:1",
            "torii: 1",
            "torii:01",
            "torii:0X1",
            "torii:0x01",
        ] {
            let err = parse_transport_hint(value).expect_err("invalid transport hint must fail");
            assert!(
                err.contains("ASCII whitespace") || err.contains("canonical unsigned"),
                "unexpected transport hint error for {value:?}: {err}"
            );
        }
    }
}

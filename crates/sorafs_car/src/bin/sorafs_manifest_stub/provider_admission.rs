use std::{
    fs,
    iter::Iterator,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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
    PROVIDER_ADMISSION_REVOCATION_VERSION_V1, ProviderAdmissionEnvelopeV1,
    ProviderAdmissionProposalV1, ProviderAdmissionRenewalV1, ProviderAdmissionRevocationV1,
    ProviderAdvertBodyV1, ProviderAdvertV1, StakePointer, StreamBudgetV1, TransportHintV1,
    TransportProtocol, compute_advert_body_digest, compute_envelope_digest,
    compute_proposal_digest, provider_advert::ProviderCapabilitySoranetPqV1,
    verify_advert_against_record, verify_revocation_signatures,
};

use super::{
    chunker_registry, parse_hex_array, parse_hex_vec, parse_u16, parse_u64, read_file_bytes,
    write_binary, write_json,
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
    "usage: sorafs_manifest_stub provider-admission <proposal|sign|verify|renewal|revoke> [options]"
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
            "--chunker-profile" => opts.profile_handle = Some(value.trim().to_string()),
            "--stake-pool-id" => opts.stake_pool_id = Some(parse_hex_array(value)?),
            "--stake-amount" => opts.stake_amount = Some(parse_u128(value)?),
            "--advert-key" => opts.advert_key = Some(parse_hex_array(value)?),
            "--jurisdiction" | "--jurisdiction-code" => {
                opts.jurisdiction = Some(value.trim().to_ascii_uppercase())
            }
            "--contact-uri" => opts.contact_uri = Some(value.to_string()),
            "--capability" => opts.capabilities.push(parse_capability(value)?),
            "--endpoint" => {
                let endpoint = parse_endpoint(value)?;
                opts.endpoints.push(EndpointBuilder::new(endpoint));
            }
            "--endpoint-kind" | "--endpoint-attestation-kind" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.attestation_kind = Some(parse_attestation_kind(value)?);
            }
            "--endpoint-attested-at" | "--endpoint-attestation-attested-at" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.attested_at = Some(parse_u64(value)?);
            }
            "--endpoint-expires-at" | "--endpoint-attestation-expires-at" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.expires_at = Some(parse_u64(value)?);
            }
            "--endpoint-leaf" | "--endpoint-attestation-leaf" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.leaf = Some(read_file_bytes(value)?);
            }
            "--endpoint-leaf-hex" | "--endpoint-attestation-leaf-hex" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.leaf = Some(parse_hex_vec(value)?);
            }
            "--endpoint-alpn" | "--endpoint-attestation-alpn" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.alpn.push(value.to_string());
            }
            "--endpoint-report" | "--endpoint-attestation-report" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.report = Some(read_file_bytes(value)?);
            }
            "--endpoint-report-hex" | "--endpoint-attestation-report-hex" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.report = Some(parse_hex_vec(value)?);
            }
            "--endpoint-intermediate" | "--endpoint-attestation-intermediate" => {
                let builder = current_endpoint(opts.endpoints.as_mut_slice(), key)?;
                builder.intermediates.push(read_file_bytes(value)?);
            }
            "--endpoint-intermediate-hex" | "--endpoint-attestation-intermediate-hex" => {
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
    let jurisdiction = opts
        .jurisdiction
        .ok_or_else(|| "missing option --jurisdiction".to_string())?;
    if opts.capabilities.is_empty() {
        return Err("at least one --capability is required".into());
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
        capabilities: opts.capabilities,
        endpoints,
        advert_key,
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
    report.insert("stake_amount".into(), Value::from(stake_amount.to_string()));
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
    let advert_bytes = read_file_bytes_path(&advert_path)?;
    let advert: ProviderAdvertV1 = decode_from_bytes(&advert_bytes)
        .map_err(|err| format!("failed to decode provider advert: {err}"))?;
    advert
        .validate_with_body(advert.issued_at)
        .map_err(|err| format!("advert validation failed: {err}"))?;
    if let Some(body_path) = advert_body_path.as_ref() {
        let body_bytes = read_file_bytes_path(body_path)?;
        let body: ProviderAdvertBodyV1 = decode_from_bytes(&body_bytes)
            .map_err(|err| format!("failed to decode advert body: {err}"))?;
        if body != advert.body {
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

    for key_bytes in secret_keys {
        let signing_key = signing_key_from_bytes(&key_bytes)
            .map_err(|err| format!("invalid council secret key: {err}"))?;
        let signer = signing_key.verifying_key().to_bytes();
        let signature = signing_key.sign(&proposal_digest).to_bytes();
        signatures.push(CouncilSignature {
            signer,
            signature: signature.to_vec(),
        });
    }

    if signatures.is_empty() {
        return Err("at least one --council-signature is required".into());
    }

    let envelope = ProviderAdmissionEnvelopeV1 {
        version: ENVELOPE_VERSION,
        proposal,
        proposal_digest,
        advert_body: advert_body.clone(),
        advert_body_digest: advert_digest,
        issued_at,
        retention_epoch,
        council_signatures: signatures.clone(),
        notes,
    };
    let record = AdmissionRecord::new(envelope.clone())
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
    map.insert("signatures_verified".into(), Value::from(true));
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
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown option: {other}")),
        }
    }

    let envelope_path = opts
        .envelope_path
        .ok_or_else(|| "missing option --envelope".to_string())?;
    let envelope_bytes = read_file_bytes_path(&envelope_path)?;
    let envelope: ProviderAdmissionEnvelopeV1 = decode_from_bytes(&envelope_bytes)
        .map_err(|err| format!("failed to decode envelope: {err}"))?;
    let record = AdmissionRecord::new(envelope.clone())
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
        let bytes = read_file_bytes_path(path)?;
        let advert: ProviderAdvertV1 = decode_from_bytes(&bytes)
            .map_err(|err| format!("failed to decode provider advert: {err}"))?;
        verify_advert_against_record(&advert, &record)
            .map_err(|err| format!("provided advert does not match envelope: {err}"))?;
        advert_match = Some(true);
    }
    let mut advert_body_match = None;
    if let Some(path) = opts.advert_body_path.as_deref() {
        let bytes = read_file_bytes_path(path)?;
        let body: ProviderAdvertBodyV1 = decode_from_bytes(&bytes)
            .map_err(|err| format!("failed to decode advert body: {err}"))?;
        if body != envelope.advert_body {
            return Err("provided advert body does not match envelope".into());
        }
        advert_body_match = Some(true);
    }

    let signatures_verified = true;

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
        "signatures_verified".into(),
        Value::from(signatures_verified),
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
    let previous_record = AdmissionRecord::new(previous_envelope)
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
        .apply_renewal(&renewal)
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
    let record = AdmissionRecord::new(envelope)
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

    verify_revocation_signatures(&revocation)
        .map_err(|err| format!("revocation validation failed: {err}"))?;
    record
        .verify_revocation(&revocation)
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
    "usage: sorafs_manifest_stub provider-admission proposal --provider-id=<hex32> \
        --chunker-profile=<handle> --stake-pool-id=<hex32> --stake-amount=<amount> \
        --advert-key=<hex32> --jurisdiction-code=<ISO3166-1> --endpoint=<kind:host> \
        [--endpoint-attestation-kind=<kind>] \
        --endpoint-attestation-attested-at=<secs> --endpoint-attestation-expires-at=<secs> \
        --endpoint-attestation-leaf=<path> [--endpoint-attestation-intermediate=<path>]... \
        [--endpoint-attestation-alpn=<id>]... [--capability=<spec>] [--proposal-out=<path>] \
        [--json-out=<path>]"
}

fn sign_usage() -> &'static str {
    "usage: sorafs_manifest_stub provider-admission sign --proposal=<path> --advert=<path> \
        --retention-epoch=<epoch> [--issued-at=<secs>] --council-signature=<signer_hex:signature_hex> \
        [--advert-body=<path>] [--council-signature-file=<path>] \
        [--council-signature-public-key=<hex32>|--council-signature-public-key-file=<path>] \
        [--notes=<text>] [--envelope-out=<path>] [--json-out=<path>]"
}

fn verify_usage() -> &'static str {
    "usage: sorafs_manifest_stub provider-admission verify --envelope=<path> [--proposal=<path>] \
        [--advert=<path>] [--advert-body=<path>] [--json-out=<path>]"
}

fn renewal_usage() -> &'static str {
    "usage: sorafs_manifest_stub provider-admission renewal --previous-envelope=<path> \
        --envelope=<path> [--notes=<text>] [--renewal-out=<path>] [--json-out=<path>]"
}

fn revoke_usage() -> &'static str {
    "usage: sorafs_manifest_stub provider-admission revoke --envelope=<path> --reason=<text> \
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
    stake_amount: Option<u128>,
    advert_key: Option<[u8; 32]>,
    jurisdiction: Option<String>,
    contact_uri: Option<String>,
    capabilities: Vec<CapabilityTlv>,
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
            .ok_or_else(|| "missing --endpoint-kind for endpoint".to_string())?;
        let attested_at = self
            .attested_at
            .ok_or_else(|| "missing --endpoint-attested-at for endpoint".to_string())?;
        let expires_at = self
            .expires_at
            .ok_or_else(|| "missing --endpoint-expires-at for endpoint".to_string())?;
        let leaf = self
            .leaf
            .ok_or_else(|| "missing --endpoint-leaf for endpoint".to_string())?;
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
    chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
        format!("unknown chunker profile handle `{handle}`; use --list-chunker-profiles")
    })
}

fn current_endpoint<'a>(
    endpoints: &'a mut [EndpointBuilder],
    flag: &str,
) -> Result<&'a mut EndpointBuilder, String> {
    endpoints
        .last_mut()
        .ok_or_else(|| format!("{flag} requires at least one preceding --endpoint"))
}

fn parse_capability(value: &str) -> Result<CapabilityTlv, String> {
    let (head, payload) = value
        .split_once(':')
        .map_or((value, None), |(h, rest)| (h, Some(rest)));
    let cap_type = match head.trim().to_ascii_lowercase().as_str() {
        "torii" | "torii-gateway" => CapabilityType::ToriiGateway,
        "quic" | "quic-noise" => CapabilityType::QuicNoise,
        "soranet" | "soranet-pq" | "soranet_pq" | "soranet-hybrid-pq" => {
            CapabilityType::SoraNetHybridPq
        }
        "range" | "chunk-range" => CapabilityType::ChunkRangeFetch,
        "vendor" | "vendor-reserved" => CapabilityType::VendorReserved,
        other => {
            return Err(format!(
                "unknown capability `{other}` (expected torii|quic|soranet|soranet-pq|range|vendor)"
            ));
        }
    };
    let payload_bytes = match (cap_type, payload) {
        (CapabilityType::ChunkRangeFetch, Some(rest)) => {
            let value_raw = parse_u64(rest.trim())?;
            let value: u16 = value_raw
                .try_into()
                .map_err(|_| "range payload must fit in u16")?;
            value.to_le_bytes().to_vec()
        }
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
    Ok(CapabilityTlv {
        cap_type,
        payload: payload_bytes,
    })
}

fn parse_soranet_pq(value: &str) -> Result<ProviderCapabilitySoranetPqV1, String> {
    let mut supports_guard = false;
    let mut supports_majority = false;
    let mut supports_strict = false;
    let mut specified = false;
    for raw in value.split(|c| [',', '+', '|'].contains(&c)) {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        specified = true;
        match token.to_ascii_lowercase().as_str() {
            "guard" | "stage-a" | "stagea" => supports_guard = true,
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
    if !specified {
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

fn parse_endpoint(value: &str) -> Result<AdvertEndpoint, String> {
    let (kind_str, host) = value
        .split_once(':')
        .ok_or_else(|| "endpoint requires kind:host".to_string())?;
    let kind = match kind_str.to_ascii_lowercase().as_str() {
        "torii" => EndpointKind::Torii,
        "quic" => EndpointKind::Quic,
        "norito-rpc" | "noritorpc" => EndpointKind::NoritoRpc,
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
                max_in_flight = Some(parse_u16(raw_trimmed)?);
            }
            "max_bytes_per_sec" | "max-bytes-per-sec" | "max_rate" | "max-rate" => {
                max_bytes_per_sec = Some(parse_u64(raw_trimmed)?);
            }
            "burst" | "burst_bytes" | "burst-bytes" => {
                burst_bytes = Some(parse_u64(raw_trimmed)?);
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

fn parse_u8(value: &str) -> Result<u8, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u8::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u8>().map_err(|err| err.to_string())
    }
}

fn parse_attestation_kind(value: &str) -> Result<EndpointAttestationKind, String> {
    match value.to_ascii_lowercase().as_str() {
        "mtls" | "tls" => Ok(EndpointAttestationKind::Mtls),
        "quic" => Ok(EndpointAttestationKind::Quic),
        other => Err(format!("unknown attestation kind: {other}")),
    }
}

fn parse_signature(value: &str) -> Result<CouncilSignature, String> {
    super::parse_signature_hex(value)
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
    Ok(SigningKey::from_bytes(&seed))
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_file_bytes_path(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("failed to read {path:?}: {err}"))
}

fn parse_u128(value: &str) -> Result<u128, String> {
    super::parse_u128(value)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

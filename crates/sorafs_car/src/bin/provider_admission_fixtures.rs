//! Generates deterministic provider admission fixtures for SoraFS tests.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signer, SigningKey};
use hex::FromHex;
use norito::json::{Map, Value, to_string_pretty};
use sorafs_car::CarBuildPlan;
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    AdmissionRecord, AdvertEndpoint, AdvertSignature, AvailabilityTier, CapabilityTlv,
    CapabilityType, CouncilSignature, ENDPOINT_ATTESTATION_VERSION_V1, EndpointAdmissionV1,
    EndpointAttestationKind, EndpointAttestationV1, EndpointKind,
    PROVIDER_ADMISSION_RENEWAL_VERSION_V1, PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
    PathDiversityPolicy, ProviderAdmissionEnvelopeError, ProviderAdmissionEnvelopeV1,
    ProviderAdmissionProposalV1, ProviderAdmissionRenewalV1, ProviderAdmissionRevocationV1,
    ProviderAdvertBodyV1, ProviderAdvertV1, ProviderCapabilityRangeV1, QosHints, RendezvousTopic,
    SignatureAlgorithm, StakePointer, StreamBudgetV1, TransportHintV1, TransportProtocol,
    chunker_registry, compute_advert_body_digest, compute_envelope_digest, compute_proposal_digest,
    verify_revocation_signatures,
};
const DEFAULT_OUTPUT_DIR: &str = "fixtures/sorafs_manifest/provider_admission";

const PROVIDER_ID_HEX: &str = "0a0b0c0d0e0f0011223344556677889900aa0bb0ccddeeff1122334455667788";
const STAKE_POOL_ID_HEX: &str = "99887766554433221100ffeeddccbbaa99887766554433221100ffeeddccbbaa";
const ADVERT_KEY_HEX: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const PROVIDER_ENDPOINT_TORII: &str = "torii:cluster.primary.svc.local";
const PROVIDER_ENDPOINT_QUIC: &str = "quic:cluster.primary.svc.local";
const RENDEZVOUS_TOPIC: &str = "sorafs.sf1.primary";
const RENDEZVOUS_REGION: &str = "global";

const LEAF_CERT: &[u8] = &[0xAA, 0xBB, 0xCC, 0xDD];
const INTERMEDIATE_CERT: &[u8] = &[0x11, 0x22, 0x33, 0x44];
const QUIC_REPORT: &[u8] = &[0x10, 0x20, 0x30];

const COUNCIL_KEY_BYTES: [u8; 32] = [0x45; 32];
const PROVIDER_SIGNING_KEY_BYTES: [u8; 32] = [0x21; 32];

#[derive(Debug)]
struct Options {
    out_dir: PathBuf,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FixtureSummary {
    proposal_v1_digest: [u8; 32],
    envelope_v1_digest: [u8; 32],
    renewal_envelope_digest: [u8; 32],
    revocation_digest: [u8; 32],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_args(env::args().skip(1))?;
    let summary = generate_fixtures(&options.out_dir)?;

    println!(
        "provider admission fixtures refreshed under {} (proposal digest {}, envelope digest {})",
        options.out_dir.display(),
        hex_lower(summary.proposal_v1_digest),
        hex_lower(summary.envelope_v1_digest)
    );

    Ok(())
}

fn parse_args<I>(args: I) -> Result<Options, Box<dyn std::error::Error>>
where
    I: Iterator<Item = String>,
{
    let mut out_dir = PathBuf::from(DEFAULT_OUTPUT_DIR);
    for arg in args {
        if arg == "-h" || arg == "--help" {
            println!("usage: provider_admission_fixtures [--out-dir=<path>]");
            println!("       Regenerates deterministic SoraFS provider admission fixtures.");
            std::process::exit(0);
        } else if let Some(value) = arg.strip_prefix("--out-dir=") {
            out_dir = PathBuf::from(value);
        } else {
            return Err(format!("unknown argument: {arg}").into());
        }
    }
    Ok(Options { out_dir })
}

fn generate_fixtures(out_dir: &Path) -> Result<FixtureSummary, Box<dyn std::error::Error>> {
    fs::create_dir_all(out_dir)?;

    let descriptor =
        chunker_registry::lookup_by_handle("sorafs.sf1@1.0.0").expect("registry handle available");

    let provider_id = decode_hex_array(PROVIDER_ID_HEX)?;
    let stake_pool_id = decode_hex_array(STAKE_POOL_ID_HEX)?;
    let advert_key = decode_hex_array(ADVERT_KEY_HEX)?;

    let provider_signing_key = SigningKey::from_bytes(&PROVIDER_SIGNING_KEY_BYTES);
    let council_key = SigningKey::from_bytes(&COUNCIL_KEY_BYTES);

    let proposal_v1 = build_proposal(ProposalParams {
        namespace: descriptor.namespace,
        name: descriptor.name,
        semver: descriptor.semver,
        aliases: descriptor.aliases,
        provider_id,
        stake_pool_id,
        advert_key,
        stake_amount: 5_000,
        attested_at: 1_700_000_000,
        expires_at: 1_700_003_600,
    });
    let advert_v1 = build_advert(&proposal_v1, &provider_signing_key, 120, 600, 1_500, 32)?;
    let envelope_v1 = build_envelope(
        proposal_v1.clone(),
        advert_v1.body.clone(),
        120,
        600,
        &council_key,
    )?;
    let record_v1 = AdmissionRecord::new(envelope_v1.clone())?;

    write_binary(out_dir, "proposal_v1.to", &norito::to_bytes(&proposal_v1)?)?;
    write_json(
        out_dir,
        "proposal_v1.json",
        Value::Object(build_proposal_summary(&proposal_v1)),
    )?;

    write_binary(out_dir, "advert_v1.to", &norito::to_bytes(&advert_v1)?)?;
    write_json(
        out_dir,
        "advert_v1.json",
        Value::Object(build_advert_summary(&advert_v1)),
    )?;

    write_binary(out_dir, "envelope_v1.to", &norito::to_bytes(&envelope_v1)?)?;
    write_json(
        out_dir,
        "envelope_v1.json",
        Value::Object(build_envelope_summary(&envelope_v1, &record_v1)),
    )?;

    let proposal_v2 = build_proposal(ProposalParams {
        namespace: descriptor.namespace,
        name: descriptor.name,
        semver: descriptor.semver,
        aliases: descriptor.aliases,
        provider_id,
        stake_pool_id,
        advert_key,
        stake_amount: 7_000,
        attested_at: 1_700_000_000,
        expires_at: 1_700_007_200,
    });
    let advert_v2 = build_advert(&proposal_v2, &provider_signing_key, 220, 900, 1_400, 32)?;
    let envelope_v2 = build_envelope(
        proposal_v2.clone(),
        advert_v2.body.clone(),
        220,
        900,
        &council_key,
    )?;
    let envelope_v2_digest = compute_envelope_digest(&envelope_v2)?;

    let renewal = ProviderAdmissionRenewalV1 {
        version: PROVIDER_ADMISSION_RENEWAL_VERSION_V1,
        provider_id,
        previous_envelope_digest: *record_v1.envelope_digest(),
        envelope_digest: envelope_v2_digest,
        envelope: envelope_v2.clone(),
        notes: Some("stake top-up 2025-03".into()),
    };
    // Ensure renewal respects invariants.
    let _ = record_v1.apply_renewal(&renewal)?;

    write_binary(out_dir, "proposal_v2.to", &norito::to_bytes(&proposal_v2)?)?;
    write_binary(out_dir, "advert_v2.to", &norito::to_bytes(&advert_v2)?)?;
    write_binary(out_dir, "envelope_v2.to", &norito::to_bytes(&envelope_v2)?)?;
    write_binary(out_dir, "renewal_v1.to", &norito::to_bytes(&renewal)?)?;
    write_json(
        out_dir,
        "renewal_v1.json",
        Value::Object(build_renewal_summary(&renewal)),
    )?;

    let mut revocation = ProviderAdmissionRevocationV1 {
        version: PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
        provider_id,
        envelope_digest: *record_v1.envelope_digest(),
        revoked_at: 970,
        reason: "endpoint compromise".into(),
        council_signatures: Vec::new(),
        notes: Some("incident-456".into()),
    };
    let revocation_digest = revocation.digest()?;
    let revocation_signature = council_key.sign(&revocation_digest);
    revocation.council_signatures.push(CouncilSignature {
        signer: *council_key.verifying_key().as_bytes(),
        signature: revocation_signature.to_bytes().to_vec(),
    });
    verify_revocation_signatures(&revocation)?;
    record_v1.verify_revocation(&revocation)?;

    write_binary(out_dir, "revocation_v1.to", &norito::to_bytes(&revocation)?)?;
    write_json(
        out_dir,
        "revocation_v1.json",
        Value::Object(build_revocation_summary(&revocation, &revocation_digest)),
    )?;

    write_json(
        out_dir,
        "metadata.json",
        Value::Object(build_metadata_summary(
            &proposal_v1,
            &renewal,
            &revocation_digest,
            &record_v1,
        )),
    )?;

    let plan_payload: Vec<u8> = (0..(64 * 1024)).map(|idx| (idx % 251) as u8).collect();
    let plan = CarBuildPlan::single_file_with_profile(&plan_payload, ChunkProfile::DEFAULT)?;
    let plan_specs = plan.chunk_fetch_specs();
    let plan_entries: Vec<Value> = plan_specs
        .iter()
        .map(|spec| {
            let mut map = Map::new();
            map.insert("chunk_index".into(), Value::from(spec.chunk_index as u64));
            map.insert("offset".into(), Value::from(spec.offset));
            map.insert("length".into(), Value::from(spec.length as u64));
            map.insert("digest_blake3".into(), Value::from(hex_lower(spec.digest)));
            Value::Object(map)
        })
        .collect();
    write_json(out_dir, "multi_fetch_plan.json", Value::Array(plan_entries))?;
    write_readme(out_dir)?;

    Ok(FixtureSummary {
        proposal_v1_digest: compute_proposal_digest(&proposal_v1)?,
        envelope_v1_digest: *record_v1.envelope_digest(),
        renewal_envelope_digest: envelope_v2_digest,
        revocation_digest,
    })
}

struct ProposalParams<'a> {
    namespace: &'a str,
    name: &'a str,
    semver: &'a str,
    aliases: &'a [&'a str],
    provider_id: [u8; 32],
    stake_pool_id: [u8; 32],
    advert_key: [u8; 32],
    stake_amount: u128,
    attested_at: u64,
    expires_at: u64,
}

fn build_proposal(params: ProposalParams<'_>) -> ProviderAdmissionProposalV1 {
    let canonical_handle = format!("{}.{}@{}", params.namespace, params.name, params.semver);
    let mut alias_list: Vec<String> = params
        .aliases
        .iter()
        .map(|alias| alias.to_string())
        .collect();
    alias_list.retain(|alias| alias != &canonical_handle);
    alias_list.insert(0, canonical_handle.clone());

    let range_payload = ProviderCapabilityRangeV1 {
        max_chunk_span: 32,
        min_granularity: 8,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    }
    .to_bytes()
    .expect("encode range capability");

    ProviderAdmissionProposalV1 {
        version: 1,
        provider_id: params.provider_id,
        profile_id: canonical_handle,
        profile_aliases: Some(alias_list),
        stake: StakePointer {
            pool_id: params.stake_pool_id,
            stake_amount: params.stake_amount,
        },
        capabilities: vec![
            CapabilityTlv {
                cap_type: CapabilityType::ToriiGateway,
                payload: Vec::new(),
            },
            CapabilityTlv {
                cap_type: CapabilityType::ChunkRangeFetch,
                payload: range_payload,
            },
        ],
        endpoints: vec![
            EndpointAdmissionV1 {
                endpoint: AdvertEndpoint {
                    kind: EndpointKind::Torii,
                    host_pattern: PROVIDER_ENDPOINT_TORII.into(),
                    metadata: Vec::new(),
                },
                attestation: EndpointAttestationV1 {
                    version: ENDPOINT_ATTESTATION_VERSION_V1,
                    kind: EndpointAttestationKind::Mtls,
                    attested_at: params.attested_at,
                    expires_at: params.expires_at,
                    leaf_certificate: LEAF_CERT.to_vec(),
                    intermediate_certificates: vec![INTERMEDIATE_CERT.to_vec()],
                    alpn_ids: vec!["h2".into()],
                    report: QUIC_REPORT.to_vec(),
                },
            },
            EndpointAdmissionV1 {
                endpoint: AdvertEndpoint {
                    kind: EndpointKind::Quic,
                    host_pattern: PROVIDER_ENDPOINT_QUIC.into(),
                    metadata: Vec::new(),
                },
                attestation: EndpointAttestationV1 {
                    version: ENDPOINT_ATTESTATION_VERSION_V1,
                    kind: EndpointAttestationKind::Quic,
                    attested_at: params.attested_at,
                    expires_at: params.expires_at,
                    leaf_certificate: LEAF_CERT.to_vec(),
                    intermediate_certificates: Vec::new(),
                    alpn_ids: vec!["h3".into()],
                    report: QUIC_REPORT.to_vec(),
                },
            },
        ],
        advert_key: params.advert_key,
        jurisdiction_code: "US".into(),
        contact_uri: Some("mailto:ops@example.com".into()),
        stream_budget: Some(StreamBudgetV1 {
            max_in_flight: 8,
            max_bytes_per_sec: 9_000_000,
            burst_bytes: Some(4_500_000),
        }),
        transport_hints: Some(vec![
            TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 0,
            },
            TransportHintV1 {
                protocol: TransportProtocol::QuicStream,
                priority: 1,
            },
        ]),
    }
}

fn build_advert(
    proposal: &ProviderAdmissionProposalV1,
    provider_key: &SigningKey,
    issued_at: u64,
    retention_epoch: u64,
    max_latency_ms: u32,
    max_streams: u16,
) -> Result<ProviderAdvertV1, Box<dyn std::error::Error>> {
    let body = ProviderAdvertBodyV1 {
        provider_id: proposal.provider_id,
        profile_id: proposal.profile_id.clone(),
        profile_aliases: proposal.profile_aliases.clone(),
        stake: proposal.stake,
        qos: QosHints {
            availability: AvailabilityTier::Hot,
            max_retrieval_latency_ms: max_latency_ms,
            max_concurrent_streams: max_streams,
        },
        capabilities: proposal.capabilities.clone(),
        endpoints: proposal
            .endpoints
            .iter()
            .map(|entry| entry.endpoint.clone())
            .collect(),
        rendezvous_topics: vec![RendezvousTopic {
            topic: RENDEZVOUS_TOPIC.into(),
            region: RENDEZVOUS_REGION.into(),
        }],
        path_policy: PathDiversityPolicy {
            min_guard_weight: 10,
            max_same_asn_per_path: 1,
            max_same_pool_per_path: 1,
        },
        notes: None,
        stream_budget: proposal.stream_budget,
        transport_hints: proposal.transport_hints.clone(),
    };
    let body_bytes = norito::to_bytes(&body)?;
    let signature = provider_key.sign(&body_bytes);
    Ok(ProviderAdvertV1 {
        version: 1,
        issued_at,
        expires_at: retention_epoch,
        body,
        signature: AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: provider_key.verifying_key().as_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        },
        signature_strict: true,
        allow_unknown_capabilities: false,
    })
}

fn build_envelope(
    proposal: ProviderAdmissionProposalV1,
    advert_body: ProviderAdvertBodyV1,
    issued_at: u64,
    retention_epoch: u64,
    council_key: &SigningKey,
) -> Result<ProviderAdmissionEnvelopeV1, ProviderAdmissionEnvelopeError> {
    let proposal_digest = compute_proposal_digest(&proposal).map_err(|source| {
        ProviderAdmissionEnvelopeError::Serialization {
            context: "proposal",
            source,
        }
    })?;
    let advert_digest = compute_advert_body_digest(&advert_body).map_err(|source| {
        ProviderAdmissionEnvelopeError::Serialization {
            context: "advert_body",
            source,
        }
    })?;

    let signature = council_key.sign(&proposal_digest);
    let council_signature = CouncilSignature {
        signer: *council_key.verifying_key().as_bytes(),
        signature: signature.to_bytes().to_vec(),
    };

    let envelope = ProviderAdmissionEnvelopeV1 {
        version: 1,
        proposal,
        proposal_digest,
        advert_body,
        advert_body_digest: advert_digest,
        issued_at,
        retention_epoch,
        council_signatures: vec![council_signature],
        notes: None,
    };
    AdmissionRecord::new(envelope.clone()).map(|_| envelope)
}

fn build_proposal_summary(proposal: &ProviderAdmissionProposalV1) -> Map {
    let mut map = Map::new();
    map.insert(
        "provider_id_hex".into(),
        Value::from(hex_lower(proposal.provider_id)),
    );
    map.insert(
        "profile_id".into(),
        Value::from(proposal.profile_id.clone()),
    );
    map.insert(
        "profile_aliases".into(),
        Value::Array(
            proposal
                .profile_aliases
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(Value::from)
                .collect(),
        ),
    );
    map.insert(
        "stake_amount".into(),
        Value::from(proposal.stake.stake_amount.to_string()),
    );
    map.insert(
        "capabilities".into(),
        Value::from(proposal.capabilities.len() as u64),
    );
    map.insert(
        "endpoints".into(),
        Value::from(proposal.endpoints.len() as u64),
    );
    map.insert(
        "stream_budget".into(),
        match proposal.stream_budget.as_ref() {
            Some(budget) => stream_budget_summary(budget),
            None => Value::Null,
        },
    );
    map.insert(
        "transport_hints".into(),
        match proposal.transport_hints.as_ref() {
            Some(hints) => transport_hints_summary(hints),
            None => Value::Null,
        },
    );
    map
}

fn build_advert_summary(advert: &ProviderAdvertV1) -> Map {
    let mut map = Map::new();
    map.insert("issued_at".into(), Value::from(advert.issued_at));
    map.insert("expires_at".into(), Value::from(advert.expires_at));
    map.insert(
        "signature_hex".into(),
        Value::from(hex_lower(&advert.signature.signature)),
    );
    map.insert(
        "public_key_hex".into(),
        Value::from(hex_lower(&advert.signature.public_key)),
    );
    map.insert(
        "capabilities".into(),
        Value::from(advert.body.capabilities.len() as u64),
    );
    map.insert(
        "endpoint_count".into(),
        Value::from(advert.body.endpoints.len() as u64),
    );
    map.insert(
        "stream_budget".into(),
        match advert.body.stream_budget.as_ref() {
            Some(budget) => stream_budget_summary(budget),
            None => Value::Null,
        },
    );
    map.insert(
        "transport_hints".into(),
        match advert.body.transport_hints.as_ref() {
            Some(hints) => transport_hints_summary(hints),
            None => Value::Null,
        },
    );
    map
}

fn build_envelope_summary(envelope: &ProviderAdmissionEnvelopeV1, record: &AdmissionRecord) -> Map {
    let mut map = Map::new();
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(hex_lower(envelope.proposal_digest)),
    );
    map.insert(
        "advert_body_digest_hex".into(),
        Value::from(hex_lower(envelope.advert_body_digest)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(hex_lower(record.envelope_digest())),
    );
    map.insert(
        "council_signature_count".into(),
        Value::from(envelope.council_signatures.len() as u64),
    );
    map
}

fn build_renewal_summary(renewal: &ProviderAdmissionRenewalV1) -> Map {
    let mut map = Map::new();
    map.insert(
        "previous_envelope_digest_hex".into(),
        Value::from(hex_lower(renewal.previous_envelope_digest)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(hex_lower(renewal.envelope_digest)),
    );
    map.insert(
        "retention_epoch".into(),
        Value::from(renewal.envelope.retention_epoch),
    );
    map
}

fn build_revocation_summary(
    revocation: &ProviderAdmissionRevocationV1,
    revocation_digest: &[u8; 32],
) -> Map {
    let mut map = Map::new();
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(hex_lower(revocation.envelope_digest)),
    );
    map.insert(
        "revocation_digest_hex".into(),
        Value::from(hex_lower(revocation_digest)),
    );
    map.insert("revoked_at".into(), Value::from(revocation.revoked_at));
    map.insert("reason".into(), Value::from(revocation.reason.clone()));
    map.insert(
        "council_signature_count".into(),
        Value::from(revocation.council_signatures.len() as u64),
    );
    map
}

fn build_metadata_summary(
    proposal: &ProviderAdmissionProposalV1,
    renewal: &ProviderAdmissionRenewalV1,
    revocation_digest: &[u8; 32],
    record: &AdmissionRecord,
) -> Map {
    let mut map = Map::new();
    let proposal_digest =
        compute_proposal_digest(proposal).expect("proposal digest should serialize");
    map.insert(
        "proposal_digest_hex".into(),
        Value::from(hex_lower(proposal_digest)),
    );
    map.insert(
        "envelope_digest_hex".into(),
        Value::from(hex_lower(record.envelope_digest())),
    );
    map.insert(
        "renewal_envelope_digest_hex".into(),
        Value::from(hex_lower(renewal.envelope_digest)),
    );
    map.insert(
        "revocation_digest_hex".into(),
        Value::from(hex_lower(revocation_digest)),
    );
    map.insert(
        "notes".into(),
        Value::from("Deterministic fixtures for tests"),
    );
    map
}

fn write_binary(
    out_dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let path = out_dir.join(name);
    fs::write(path, bytes)?;
    Ok(())
}

fn write_json(out_dir: &Path, name: &str, value: Value) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_string = to_string_pretty(&value)?;
    json_string.push('\n');
    let path = out_dir.join(name);
    fs::write(path, json_string)?;
    Ok(())
}

fn write_readme(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = out_dir.join("README.md");
    let mut content = String::from(
        "# Provider Admission Fixtures\n\n\
These files are generated via `cargo run -p sorafs_car --features cli --bin provider_admission_fixtures`.\n\
They provide deterministic governance proposals, adverts, envelopes, renewals, and revocations for\n\
integration tests across Rust, Torii, and CLI tooling.\n\n\
Additional artifacts include a sample multi-source fetch plan so SDKs can exercise chunk scheduling\n\
end-to-end.\n\n\
Do not edit manually; rerun the generator if data changes.\n",
    );
    content.push('\n');
    fs::write(path, content)?;
    Ok(())
}

fn decode_hex_array(input: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    let mut out = [0u8; 32];
    let bytes = Vec::from_hex(input)?;
    if bytes.len() != 32 {
        return Err(format!("expected 32-byte hex string, got {}", bytes.len()).into());
    }
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_lower<T: AsRef<[u8]>>(bytes: T) -> String {
    hex::encode(bytes)
}

fn stream_budget_summary(budget: &StreamBudgetV1) -> Value {
    let mut map = Map::new();
    map.insert(
        "max_in_flight".into(),
        Value::from(budget.max_in_flight as u64),
    );
    map.insert(
        "max_bytes_per_sec".into(),
        Value::from(budget.max_bytes_per_sec),
    );
    map.insert(
        "burst_bytes".into(),
        match budget.burst_bytes {
            Some(burst) => Value::from(burst),
            None => Value::Null,
        },
    );
    Value::Object(map)
}

fn transport_hints_summary(hints: &[TransportHintV1]) -> Value {
    Value::Array(
        hints
            .iter()
            .map(|hint| {
                let mut map = Map::new();
                map.insert(
                    "protocol".into(),
                    Value::from(transport_protocol_label(hint.protocol)),
                );
                map.insert("priority".into(), Value::from(hint.priority as u64));
                Value::Object(map)
            })
            .collect(),
    )
}

fn transport_protocol_label(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::ToriiHttpRange => "torii_http_range",
        TransportProtocol::QuicStream => "quic_stream",
        TransportProtocol::SoraNetRelay => "soranet_relay",
        TransportProtocol::VendorReserved => "vendor_reserved",
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn generate_fixtures_produces_expected_artifacts() {
        let dir = TempDir::new().expect("tmp dir");
        let summary = generate_fixtures(dir.path()).expect("fixtures");

        assert_eq!(
            hex_lower(summary.proposal_v1_digest),
            "ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936"
        );
        assert_eq!(
            hex_lower(summary.envelope_v1_digest),
            "25741a0e70302a8af6bfe59f13544ce8ee6dc8f29eddf07781c308e19f3f05c5"
        );
        assert_eq!(
            hex_lower(summary.renewal_envelope_digest),
            "6f539600bbf619c93d424936def6eecded8249cd56c4546aac17029a33a0c459"
        );
        assert_eq!(
            hex_lower(summary.revocation_digest),
            "d706ec5b0da1627a696ee68ea0400e446b658cd82c669e4e6e5a094cbb57be02"
        );

        let proposal_path = dir.path().join("proposal_v1.to");
        assert!(proposal_path.exists(), "proposal fixture missing");
    }
}

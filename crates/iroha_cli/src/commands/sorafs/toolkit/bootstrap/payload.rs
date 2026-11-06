//! Build initial Torii provider material from explicit reviewed public inputs.
//!
//! Input TLS certificate bytes and stake pointers are operator evidence, not facts discovered
//! or authenticated by this offline constructor. Native admission/council validation is reused.

use std::path::PathBuf;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, ensure, eyre};
use iroha_crypto::{Algorithm, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    sorafs::{
        bootstrap::{
            SORAFS_BOOTSTRAP_MAX_PROVIDERS_V1, SorafsBootstrapGrantV1, SorafsBootstrapPayloadV1,
            SorafsBootstrapProviderV1,
        },
        provider_admission::ProviderAdmissionCouncilPolicyV1,
    },
};
use sorafs_manifest::{
    CouncilSignature,
    provider_admission::{
        EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1,
        ProviderAdmissionGenesisMaterialV1, ProviderAdmissionProposalV1, ProviderVrfPublicKeyV1,
        compute_envelope_authorization_digest,
    },
    provider_advert::{
        AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, EndpointKind,
        PathDiversityPolicy, ProviderAdvertBodyV1, ProviderCapabilityRangeV1, QosHints,
        RendezvousTopic, StakePointer, account_read::RegisteredAccountReadV1,
    },
};

use super::{parse_digest, read_pinned, read_private_key, validate_payload, write_new};

/// Public request and explicitly selected runtime council credentials.
#[derive(Debug, clap::Args)]
pub struct PreparePayloadArgs {
    /// Independently selected genesis-derived network identity.
    #[arg(long)]
    network_id: NetworkId,
    /// Bounded public bootstrap request JSON; it contains no private keys or key paths.
    #[arg(long, value_name = "PATH")]
    request_json: PathBuf,
    /// BLAKE3 of the reviewed public request, lowercase hex.
    #[arg(long, value_parser = parse_digest)]
    request_blake3_hex: [u8; 32],
    /// Explicit owner-private native Ed25519 council key; repeat for the exact threshold.
    #[arg(long, value_name = "PATH", required = true)]
    council_key_file: Vec<PathBuf>,
    /// New JSON SorafsBootstrapPayloadV1 for the prepare operation.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PayloadRequest {
    schema: String,
    council: ProviderAdmissionCouncilPolicyV1,
    providers: Vec<ProviderInput>,
    grants: Vec<SorafsBootstrapGrantV1>,
    compliance_operators: Vec<AccountId>,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ProviderInput {
    owner: AccountId,
    provider_id: [u8; 32],
    profile_id: String,
    stake_pool_id: [u8; 32],
    stake_xor: String,
    advert_public_key: PublicKey,
    por_vrf_public_key: PublicKey,
    potr_mldsa_public_key: Option<PublicKey>,
    jurisdiction_code: String,
    contact_uri: Option<String>,
    issued_at: u64,
    retention_epoch: u64,
    endpoints: Vec<EndpointInput>,
    qos: QosInput,
    range: Option<RangeInput>,
    registered_account_read: Option<RegisteredAccountReadV1>,
    rendezvous: Vec<RendezvousInput>,
    path_policy: PathPolicyInput,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct EndpointInput {
    https_origin: String,
    attested_at: u64,
    expires_at: u64,
    leaf_certificate_der_base64: String,
    intermediate_certificates_der_base64: Vec<String>,
    alpn_ids: Vec<String>,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct QosInput {
    availability: String,
    max_retrieval_latency_ms: u32,
    max_concurrent_streams: u16,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RangeInput {
    max_chunk_span: u32,
    min_granularity: u32,
    supports_sparse_offsets: bool,
    requires_alignment: bool,
    supports_merkle_proof: bool,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RendezvousInput {
    topic: String,
    region: String,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PathPolicyInput {
    min_guard_weight: u16,
    max_same_asn_per_path: u8,
    max_same_pool_per_path: u8,
}

pub(super) fn run(args: PreparePayloadArgs, now: u64) -> Result<()> {
    let bytes = read_pinned(
        &args.request_json,
        8 * super::SORAFS_BOOTSTRAP_MAX_PAYLOAD_BYTES_V1,
        args.request_blake3_hex,
    )?;
    let request: PayloadRequest = norito::json::from_slice(&bytes)?;
    // Validate all public material and bounds before opening any credential.
    let materials = validate_request(&request, &args.network_id, now)?;
    ensure!(
        args.council_key_file.len() == usize::from(request.council.signature_threshold),
        "supply exactly the configured council threshold of credential files"
    );
    let keys = args
        .council_key_file
        .iter()
        .map(|path| read_private_key(path))
        .collect::<Result<Vec<_>>>()?;
    let payload = sign_payload(request, &materials, &keys, &args.network_id, now)?;
    write_new(&args.output, &norito::json::to_vec(&payload)?)?;
    Ok(())
}

fn validate_request(
    request: &PayloadRequest,
    network: &NetworkId,
    now: u64,
) -> Result<Vec<ProviderAdmissionGenesisMaterialV1>> {
    ensure!(
        request.schema == "iroha.sorafs.bootstrap.request.v1",
        "unexpected bootstrap request schema"
    );
    request.council.validate()?;
    ensure!(
        request.council.network_id == *network.as_bytes()
            && request.council.revision == 1
            && !request.council.paused,
        "council must be active revision one on the selected network"
    );
    ensure!(
        !request.providers.is_empty()
            && request.providers.len() <= SORAFS_BOOTSTRAP_MAX_PROVIDERS_V1,
        "provider count is outside bootstrap bounds"
    );
    ensure!(
        request
            .providers
            .windows(2)
            .all(|pair| pair[0].provider_id < pair[1].provider_id),
        "providers must be strictly ordered by id"
    );
    let materials = request
        .providers
        .iter()
        .map(|provider| material(provider, now))
        .collect::<Result<Vec<_>>>()?;
    // Check grants/compliance ordering and closed permission names without signing admissions.
    SorafsBootstrapPayloadV1 {
        council: norito::encode_canonical(&request.council)?,
        providers: request
            .providers
            .iter()
            .zip(&materials)
            .map(|(provider, material)| {
                Ok(SorafsBootstrapProviderV1 {
                    owner: provider.owner.clone(),
                    envelope: norito::encode_canonical(&material.project(
                        request.council.network_id,
                        request.council.policy_id,
                        request.council.canonical_digest()?,
                    )?)?,
                })
            })
            .collect::<Result<_>>()?,
        grants: request.grants.clone(),
        compliance_operators: request.compliance_operators.clone(),
    }
    .validate()?;
    Ok(materials)
}

fn sign_payload(
    request: PayloadRequest,
    materials: &[ProviderAdmissionGenesisMaterialV1],
    keys: &[PrivateKey],
    network: &NetworkId,
    now: u64,
) -> Result<SorafsBootstrapPayloadV1> {
    ensure!(
        keys.len() == usize::from(request.council.signature_threshold),
        "council key count must equal threshold"
    );
    let mut signers = keys
        .iter()
        .map(|key| {
            let public = PublicKey::from_private_key(key)
                .map_err(|_| eyre!("invalid council credential"))?;
            let signer = ed25519_bytes(&public)?;
            ensure!(
                request
                    .council
                    .trusted_signers
                    .binary_search(&signer)
                    .is_ok(),
                "council credential is not in the reviewed trust set"
            );
            Ok((signer, key))
        })
        .collect::<Result<Vec<_>>>()?;
    signers.sort_by_key(|(public, _)| *public);
    ensure!(
        signers.windows(2).all(|pair| pair[0].0 != pair[1].0),
        "duplicate council credential"
    );
    ensure!(
        materials.len() == request.providers.len(),
        "provider material count mismatch"
    );
    let policy_digest = request.council.canonical_digest()?;
    let providers = request
        .providers
        .iter()
        .zip(materials)
        .map(|(provider, material)| {
            let mut envelope = material.project(
                request.council.network_id,
                request.council.policy_id,
                policy_digest,
            )?;
            let digest = compute_envelope_authorization_digest(&envelope)?;
            envelope.council_signatures = signers
                .iter()
                .map(|(public, key)| {
                    Ok(CouncilSignature {
                        signer: *public,
                        signature: Signature::try_new(key, &digest)
                            .map_err(|_| eyre!("council signing failed"))?
                            .payload()
                            .to_vec(),
                    })
                })
                .collect::<Result<_>>()?;
            request
                .council
                .verify_envelope_policy_claim(&envelope, now)?;
            Ok(SorafsBootstrapProviderV1 {
                owner: provider.owner.clone(),
                envelope: norito::encode_canonical(&envelope)?,
            })
        })
        .collect::<Result<_>>()?;
    let payload = SorafsBootstrapPayloadV1 {
        council: norito::encode_canonical(&request.council)?,
        providers,
        grants: request.grants,
        compliance_operators: request.compliance_operators,
    };
    validate_payload(&payload, network, now)?;
    Ok(payload)
}

fn material(input: &ProviderInput, now: u64) -> Result<ProviderAdmissionGenesisMaterialV1> {
    ensure!(
        input.issued_at <= now && now < input.retention_epoch,
        "provider admission validity does not contain current time"
    );
    let descriptor = sorafs_manifest::chunker_registry::lookup_by_handle(&input.profile_id)
        .ok_or_else(|| eyre!("unknown chunker profile"))?;
    let profile_aliases = Some(
        descriptor
            .aliases
            .iter()
            .map(|alias| (*alias).to_owned())
            .collect(),
    );
    let stake = StakePointer {
        pool_id: input.stake_pool_id,
        stake_amount: super::super::super::parse_xor_quantity_labeled(
            &input.stake_xor,
            "provider stake",
        )?,
    };
    ensure!(
        input.stake_pool_id != [0; 32],
        "stake pool id must be nonzero"
    );
    let mut capabilities = vec![CapabilityTlv {
        cap_type: CapabilityType::ToriiGateway,
        payload: Vec::new(),
    }];
    if let Some(range) = &input.range {
        let range = ProviderCapabilityRangeV1 {
            max_chunk_span: range.max_chunk_span,
            min_granularity: range.min_granularity,
            supports_sparse_offsets: range.supports_sparse_offsets,
            requires_alignment: range.requires_alignment,
            supports_merkle_proof: range.supports_merkle_proof,
        };
        range.validate()?;
        capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::ChunkRangeFetch,
            payload: range.to_bytes()?,
        });
    }
    if let Some(policy) = &input.registered_account_read {
        ensure!(
            input.endpoints.iter().any(|endpoint| policy
                .https_origin()
                .is_ok_and(|origin| origin == endpoint.https_origin)),
            "account-read origin must be an explicitly admitted endpoint"
        );
        capabilities.push(policy.to_capability()?);
    }
    if let Some(public_key) = &input.potr_mldsa_public_key {
        let (algorithm, bytes) = public_key.try_to_bytes()?;
        ensure!(
            algorithm == Algorithm::MlDsa,
            "PoTR capability requires a native ML-DSA-65 public key"
        );
        sorafs_manifest::provider_advert::validate_potr_mldsa_capability(&bytes)?;
        capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::PotrMlDsa,
            payload: bytes.to_vec(),
        });
    }
    ensure!(
        !input.endpoints.is_empty() && input.endpoints.len() <= 16,
        "expected 1 to 16 explicit Torii endpoints"
    );
    let endpoints = input
        .endpoints
        .iter()
        .map(|endpoint| endpoint_material(endpoint, input.issued_at, input.retention_epoch))
        .collect::<Result<Vec<_>>>()?;
    let (vrf_algorithm, vrf_bytes) = input.por_vrf_public_key.try_to_bytes()?;
    let por_vrf_key = match vrf_algorithm {
        Algorithm::BlsNormal => ProviderVrfPublicKeyV1::BlsNormal(
            vrf_bytes
                .try_into()
                .map_err(|_| eyre!("invalid BLS normal key length"))?,
        ),
        Algorithm::BlsSmall => ProviderVrfPublicKeyV1::BlsSmall(
            vrf_bytes
                .try_into()
                .map_err(|_| eyre!("invalid BLS small key length"))?,
        ),
        _ => return Err(eyre!("PoR VRF key must use a native BLS algorithm")),
    };
    let proposal = ProviderAdmissionProposalV1 {
        version: 1,
        provider_id: input.provider_id,
        profile_id: input.profile_id.clone(),
        profile_aliases,
        stake,
        capabilities,
        endpoints,
        advert_key: ed25519_bytes(&input.advert_public_key)?,
        por_vrf_key,
        jurisdiction_code: input.jurisdiction_code.clone(),
        contact_uri: input.contact_uri.clone(),
        stream_budget: None,
        transport_hints: None,
    };
    let qos = QosHints {
        availability: match input.qos.availability.as_str() {
            "hot" => AvailabilityTier::Hot,
            "warm" => AvailabilityTier::Warm,
            "cold" => AvailabilityTier::Cold,
            _ => return Err(eyre!("availability must be hot, warm or cold")),
        },
        max_retrieval_latency_ms: input.qos.max_retrieval_latency_ms,
        max_concurrent_streams: input.qos.max_concurrent_streams,
    };
    let advert_body = ProviderAdvertBodyV1 {
        provider_id: proposal.provider_id,
        profile_id: proposal.profile_id.clone(),
        profile_aliases: proposal.profile_aliases.clone(),
        stake: proposal.stake.clone(),
        qos,
        capabilities: proposal.capabilities.clone(),
        endpoints: proposal
            .endpoints
            .iter()
            .map(|endpoint| endpoint.endpoint.clone())
            .collect(),
        rendezvous_topics: input
            .rendezvous
            .iter()
            .map(|entry| RendezvousTopic {
                topic: entry.topic.clone(),
                region: entry.region.clone(),
            })
            .collect(),
        path_policy: PathDiversityPolicy {
            min_guard_weight: input.path_policy.min_guard_weight,
            max_same_asn_per_path: input.path_policy.max_same_asn_per_path,
            max_same_pool_per_path: input.path_policy.max_same_pool_per_path,
        },
        notes: None,
        stream_budget: None,
        transport_hints: None,
    };
    advert_body.validate()?;
    let material = ProviderAdmissionGenesisMaterialV1 {
        proposal,
        advert_body,
        issued_at: input.issued_at,
        retention_epoch: input.retention_epoch,
    };
    material.validate()?;
    Ok(material)
}

fn ed25519_bytes(public: &PublicKey) -> Result<[u8; 32]> {
    let (algorithm, bytes) = public.try_to_bytes()?;
    ensure!(
        algorithm == Algorithm::Ed25519,
        "expected an Ed25519 public key"
    );
    bytes
        .try_into()
        .map_err(|_| eyre!("invalid Ed25519 key length"))
}

fn endpoint_material(
    input: &EndpointInput,
    issued_at: u64,
    retention_epoch: u64,
) -> Result<EndpointAdmissionV1> {
    let url = url::Url::parse(&input.https_origin)?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && url.port().is_none(),
        "provider endpoint must be an HTTPS origin on port 443 without credentials or path"
    );
    let host = match url.host() {
        Some(url::Host::Domain(host)) => host,
        _ => return Err(eyre!("provider endpoint must be a DNS hostname")),
    };
    ensure!(
        input.https_origin == format!("https://{host}"),
        "endpoint must use its exact lowercase canonical HTTPS origin without a trailing slash"
    );
    ensure!(
        input.attested_at <= issued_at && input.expires_at >= retention_epoch,
        "TLS attestation window must cover the complete admission interval"
    );
    ensure!(
        input.intermediate_certificates_der_base64.len() <= 8 && input.alpn_ids.len() <= 8,
        "TLS certificate chain or ALPN count exceeds bound"
    );
    let decode = |text: &str| -> Result<Vec<u8>> {
        ensure!(
            text.len() <= 128 * 1024,
            "TLS certificate exceeds byte bound"
        );
        let bytes = STANDARD.decode(text)?;
        ensure!(
            !bytes.is_empty() && STANDARD.encode(&bytes) == text,
            "TLS certificate must be nonempty canonical base64"
        );
        Ok(bytes)
    };
    Ok(EndpointAdmissionV1 {
        endpoint: AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: host.to_owned(),
            metadata: Vec::new(),
        },
        attestation: EndpointAttestationV1 {
            version: 1,
            kind: EndpointAttestationKind::Tls,
            attested_at: input.attested_at,
            expires_at: input.expires_at,
            leaf_certificate: decode(&input.leaf_certificate_der_base64)?,
            intermediate_certificates: input
                .intermediate_certificates_der_base64
                .iter()
                .map(|text| decode(text))
                .collect::<Result<_>>()?,
            alpn_ids: input.alpn_ids.clone(),
            report: Vec::new(),
        },
    })
}

#[cfg(test)]
mod tests;

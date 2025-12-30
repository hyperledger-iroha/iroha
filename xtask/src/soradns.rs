use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use blake3::hash as blake3_hash;
use data_encoding::BASE32_NOPAD;
use hex::{decode as hex_decode, encode as hex_encode};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use iroha_data_model::{
    ipfs::IpfsPath,
    soradns::{RAD_VERSION_V1, ResolverAttestationDocumentV1, ResolverDirectoryRecordV1},
};
use iroha_primitives::soradns::{
    GatewayHostBindings, GatewayHostError, canonical_gateway_suffix, derive_gateway_hosts,
};
use norito::{
    decode_from_bytes,
    json::{self, Value as NoritoJsonValue, native::Number as NoritoNumber},
    to_bytes,
};
use serde::{Deserialize, Serialize};
use serde_json::{self, Value as SerdeJsonValue, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::normalize_path;

const RAD_HASH_DOMAIN: &[u8] = b"rad-v1";
const DEFAULT_CSP_TEMPLATE: &str = "default-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'";
const DEFAULT_HSTS_TEMPLATE: &str = "max-age=63072000; includeSubDomains; preload";
const DEFAULT_PERMISSIONS_POLICY: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), clipboard-read=(self), clipboard-write=(self), encrypted-media=(), fullscreen=(self), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), speaker-selection=(), usb=(), xr-spatial-tracking=()";
pub const DEFAULT_ACME_DIRECTORY_URL: &str = "https://acme-v02.api.letsencrypt.org/directory";

/// Summary of gateway host derivation for a SoraDNS entry.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HostSummary {
    /// Original FQDN provided by the caller.
    pub name: String,
    /// Normalised ASCII FQDN used for hashing.
    pub normalized_name: String,
    /// Base32-encoded Blake3 label for the canonical host.
    pub canonical_label: String,
    /// Canonical host (`<hash>.gw.sora.id`).
    pub canonical_host: String,
    /// Pretty host (`<fqdn>.gw.sora.name`).
    pub pretty_host: String,
    /// Wildcard entry that must be authorised for canonical hosts.
    pub canonical_wildcard: &'static str,
}

/// Expectations supplied when verifying gateway binding payloads.
#[derive(Debug, Clone, Default)]
pub struct GatewayBindingExpectations {
    /// Optional canonical alias expected in the binding.
    pub alias: Option<String>,
    /// Optional content CID override.
    pub content_cid: Option<String>,
    /// Optional hostname override.
    pub hostname: Option<String>,
    /// Optional proof-status override.
    pub proof_status: Option<String>,
    /// Optional manifest JSON path used to derive the expected content CID.
    pub manifest_path: Option<PathBuf>,
}

/// Summary returned after verifying a gateway binding payload.
#[derive(Debug, Clone)]
pub struct GatewayBindingSummary {
    /// Alias detected in the payload/headers.
    pub alias: Option<String>,
    /// Content CID authorised by the binding.
    pub content_cid: String,
    /// Hostname recorded in the payload (if any).
    #[allow(dead_code)]
    pub hostname: Option<String>,
    /// Proof-status string carried in headers.
    pub proof_status: Option<String>,
    /// Canonical label derived from the alias.
    pub canonical_label: String,
    /// Canonical host derived from the alias.
    pub canonical_host: String,
}

/// Computed manifest metadata used for GAR templates and probes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestCidDigest {
    /// Base32 content identifier derived from the manifest root bytes.
    pub manifest_cid: String,
    /// BLAKE3-256 digest of the manifest payload (hex).
    pub manifest_digest_hex: String,
}

/// Errors raised while deriving manifest CID/digest pairs.
#[derive(Debug, Error)]
pub enum ManifestCidDigestError {
    /// Underlying IO error while reading the manifest.
    #[error("failed to read manifest `{path}`: {source}")]
    Io {
        /// Source path.
        path: PathBuf,
        /// IO failure.
        #[source]
        source: std::io::Error,
    },
    /// Manifest JSON could not be parsed.
    #[error("failed to parse manifest `{path}`: {source}")]
    Parse {
        /// Source path.
        path: PathBuf,
        /// Parse failure.
        #[source]
        source: serde_json::Error,
    },
    /// Root CID extraction failed.
    #[error(transparent)]
    Root(#[from] GatewayBindingVerifyError),
}

/// Read a manifest file and return the canonical CID plus BLAKE3 digest.
pub fn manifest_cid_and_digest_from_path(
    path: &Path,
) -> Result<ManifestCidDigest, ManifestCidDigestError> {
    let data = fs::read(path).map_err(|source| ManifestCidDigestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest: SerdeJsonValue =
        serde_json::from_slice(&data).map_err(|source| ManifestCidDigestError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    let root_bytes = manifest_root_bytes(&manifest, path)?;
    let encoded = BASE32_NOPAD.encode(&root_bytes).to_ascii_lowercase();
    let manifest_cid = format!("b{encoded}");
    let manifest_digest_hex = blake3_hash(&data).to_hex().to_string();
    Ok(ManifestCidDigest {
        manifest_cid,
        manifest_digest_hex,
    })
}

/// User-facing options for generating an ACME certificate plan.
#[derive(Debug, Clone)]
pub struct AcmePlanOptions {
    /// SoraDNS names that require gateway certificates.
    pub names: Vec<String>,
    /// ACME directory URL used by downstream automation.
    pub directory_url: String,
    /// Whether to include canonical wildcard certificates.
    pub include_canonical_wildcard: bool,
    /// Whether to include pretty host certificates.
    pub include_pretty_hosts: bool,
    /// Timestamp recorded in the resulting plan.
    pub generated_at: OffsetDateTime,
}

/// JSON blob describing the SAN/challenge plan for gateway certificates.
#[derive(Debug, Clone, Serialize)]
pub struct AcmePlan {
    /// ACME directory URL to target.
    pub directory_url: String,
    /// RFC3339 timestamp when the plan was rendered.
    pub generated_at: String,
    /// Certificate plan per alias.
    pub hosts: Vec<AcmePlanHost>,
}

/// Certificate plan for a single alias.
#[derive(Debug, Clone, Serialize)]
pub struct AcmePlanHost {
    /// Alias literal.
    pub name: String,
    /// Normalised alias used for hashing.
    pub normalized_name: String,
    /// Base32 label derived from the alias.
    pub canonical_label: String,
    /// Canonical host (`<label>.gw.sora.id`).
    pub canonical_host: String,
    /// Wildcard covering the canonical namespace.
    pub canonical_wildcard: String,
    /// Pretty host (`<alias>.gw.sora.name`).
    pub pretty_host: String,
    /// Planned certificate permutations.
    pub certificates: Vec<AcmeCertificatePlan>,
}

/// Individual certificate plan (SAN set + challenge guidance).
#[derive(Debug, Clone, Serialize)]
pub struct AcmeCertificatePlan {
    /// Certificate kind (wildcard vs pretty host).
    pub kind: AcmeCertificateKind,
    /// Subject Alternative Names included in the CSR/order.
    pub san: Vec<String>,
    /// Recommended ACME challenge types.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recommended_challenges: Vec<String>,
    /// DNS-01 labels that automation must publish.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dns_challenge_labels: Vec<String>,
    /// Free-form operator notes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Enumeration describing the certificate plan type.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcmeCertificateKind {
    /// Wildcard certificate used for canonical hosts.
    CanonicalWildcard,
    /// Certificate covering the pretty host.
    PrettyHost,
}

/// Options supplied when producing a cache invalidation plan.
#[derive(Debug, Clone)]
pub struct CacheInvalidationPlanOptions {
    /// Aliases that require cache purge coverage.
    pub names: Vec<String>,
    /// Whether to add pretty hosts to the purge target list.
    pub include_pretty_hosts: bool,
    /// HTTP paths that must be purged.
    pub paths: Vec<String>,
    /// HTTP method to use when issuing purge requests.
    pub http_method: String,
    /// Optional name of the authentication header used by automation.
    pub auth_header: Option<String>,
    /// Optional environment variable holding the purge credential/token.
    pub auth_env: Option<String>,
    /// Timestamp recorded in the resulting plan.
    pub generated_at: OffsetDateTime,
}

/// High-level cache invalidation plan for a batch of aliases.
#[derive(Debug, Clone, Serialize)]
pub struct CacheInvalidationPlan {
    /// RFC3339 timestamp when the plan was generated.
    pub generated_at: String,
    /// HTTP method to use for every purge request.
    pub http_method: String,
    /// Optional name of the authentication header supplied with the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<String>,
    /// Optional environment variable that carries the purge credential/token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_env: Option<String>,
    /// Alias-specific cache purge targets.
    pub entries: Vec<CacheInvalidationEntry>,
}

/// Cache purge targets for a single alias.
#[derive(Debug, Clone, Serialize)]
pub struct CacheInvalidationEntry {
    /// Alias literal.
    pub name: String,
    /// Normalised alias used for hashing.
    pub normalized_name: String,
    /// Base32-encoded canonical label.
    pub canonical_label: String,
    /// Canonical host (`<label>.gw.sora.id`).
    pub canonical_host: String,
    /// Wildcard covering the canonical namespace.
    pub canonical_wildcard: String,
    /// Pretty host (`<alias>.gw.sora.name`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretty_host: Option<String>,
    /// Concrete purge targets derived from the alias.
    pub purge_targets: Vec<CachePurgeTarget>,
}

/// Concrete purge request guidance.
#[derive(Debug, Clone, Serialize)]
pub struct CachePurgeTarget {
    /// Hostname that should be purged.
    pub host: String,
    /// HTTP paths that should be purged on the host.
    pub paths: Vec<String>,
}

/// Options supplied when producing a route promotion/rollback plan.
#[derive(Debug, Clone)]
pub struct RoutePlanOptions {
    /// Aliases that require promotion coverage.
    pub names: Vec<String>,
    /// Whether to include pretty hosts in the plan.
    pub include_pretty_hosts: bool,
    /// Timestamp recorded in the resulting plan.
    pub generated_at: OffsetDateTime,
}

/// Deterministic promotion + rollback plan for gateway bindings.
#[derive(Debug, Clone, Serialize)]
pub struct RoutePlan {
    /// RFC3339 timestamp when the plan was generated.
    pub generated_at: String,
    /// Entries per alias.
    pub entries: Vec<RoutePlanEntry>,
}

/// Alias-specific promotion metadata.
#[derive(Debug, Clone, Serialize)]
pub struct RoutePlanEntry {
    /// Alias literal.
    pub name: String,
    /// Normalised alias used for hashing.
    pub normalized_name: String,
    /// Canonical host.
    pub canonical_host: String,
    /// Pretty host (when requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretty_host: Option<String>,
    /// Promotion steps executed in order.
    pub promote_steps: Vec<RoutePlanStep>,
    /// Rollback steps executed in order.
    pub rollback_steps: Vec<RoutePlanStep>,
}

/// Structured action used for promotion or rollback.
#[derive(Debug, Clone, Serialize)]
pub struct RoutePlanStep {
    /// Action identifier (machine friendly).
    pub action: String,
    /// Human-readable description.
    pub description: String,
    /// Target host (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Optional notes/free-form guidance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// JSON shape emitted by docs portal tooling (camelCase + snake_case).
#[derive(Debug, Deserialize)]
struct GatewayBindingRecord {
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default, rename = "contentCid", alias = "content_cid")]
    content_cid: Option<String>,
    #[serde(default, rename = "proofStatus", alias = "proof_status")]
    proof_status: Option<String>,
    #[serde(default, rename = "generatedAt", alias = "generated_at")]
    generated_at: Option<String>,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    #[serde(default, rename = "headersTemplate", alias = "headers_template")]
    #[allow(dead_code)]
    headers_template: Option<String>,
    #[allow(dead_code)]
    #[serde(default, rename = "jsonPath", alias = "json_path")]
    json_path: Option<String>,
    #[allow(dead_code)]
    #[serde(default, rename = "headersPath", alias = "headers_path")]
    headers_path: Option<String>,
}

/// Errors encountered while deriving gateway hosts.
#[derive(Debug, Error)]
pub enum HostSummaryError {
    /// The provided FQDN violated the normalisation rules.
    #[error("failed to derive gateway hosts for `{name}`: {source}")]
    Derive {
        /// Offending FQDN.
        name: String,
        /// Underlying validation error.
        #[source]
        source: GatewayHostError,
    },
    /// Canonical label or host failed the Blake3/base32 derivation check.
    #[error(
        "canonical host for `{name}` does not match blake3/base32 derivation (expected label `{expected_label}` host `{expected_host}`, found label `{found_label}` host `{found_host}`)"
    )]
    CanonicalDerivationMismatch {
        /// Original FQDN.
        name: String,
        /// Expected base32 label.
        expected_label: String,
        /// Expected canonical host.
        expected_host: String,
        /// Observed label.
        found_label: String,
        /// Observed canonical host.
        found_host: String,
    },
}

/// Errors emitted while verifying gateway binding headers.
#[derive(Debug, Error)]
pub enum GatewayBindingVerifyError {
    /// Unable to read the supplied JSON file.
    #[error("failed to read gateway binding `{path}`: {source}")]
    Io {
        /// Binding path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Unable to parse the JSON payload.
    #[error("failed to parse gateway binding `{path}`: {source}")]
    Parse {
        /// Binding path.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },
    /// Binding payload omitted the headers block.
    #[error("gateway binding `{path}` is missing the headers block")]
    MissingHeaders {
        /// Binding path.
        path: PathBuf,
    },
    /// Required header is absent.
    #[error("gateway binding `{path}` is missing header `{header}`")]
    MissingHeader {
        /// Binding path.
        path: PathBuf,
        /// Missing header name.
        header: &'static str,
    },
    /// Alias could not be derived from payload or headers.
    #[error("gateway binding `{path}` does not declare an alias")]
    MissingAlias {
        /// Binding path.
        path: PathBuf,
    },
    /// Content CID missing altogether.
    #[error("gateway binding `{path}` does not declare a content CID")]
    MissingContentCid {
        /// Binding path.
        path: PathBuf,
    },
    /// Unable to read manifest JSON when deriving expectations.
    #[error("failed to read manifest `{path}`: {source}")]
    ManifestIo {
        /// Manifest path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Manifest JSON parse failure.
    #[error("failed to parse manifest `{path}`: {source}")]
    ManifestParse {
        /// Manifest path.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },
    /// Manifest lacked root CID metadata.
    #[error("manifest `{path}` is missing root CID fields")]
    ManifestMissingRoot {
        /// Manifest path.
        path: PathBuf,
    },
    /// Manifest root CID data was invalid.
    #[error("manifest `{path}` has invalid root CID data: {details}")]
    ManifestRootInvalid {
        /// Manifest path.
        path: PathBuf,
        /// Description of the issue.
        details: String,
    },
    /// Alias did not match the expected value.
    #[error("gateway binding `{path}` alias `{found}` does not match expected `{expected}`")]
    AliasMismatch {
        /// Binding path.
        path: PathBuf,
        /// Expected alias.
        expected: String,
        /// Alias found in payload.
        found: String,
    },
    /// Content CID mismatch vs. expectations.
    #[error("gateway binding `{path}` content CID `{found}` does not match expected `{expected}`")]
    ContentCidMismatch {
        /// Binding path.
        path: PathBuf,
        /// Expected CID.
        expected: String,
        /// CID found in payload.
        found: String,
    },
    /// Proof-status mismatch vs. expectations.
    #[error("gateway binding `{path}` proof status `{found}` does not match expected `{expected}`")]
    ProofStatusMismatch {
        /// Binding path.
        path: PathBuf,
        /// Expected status.
        expected: String,
        /// Status found in payload.
        found: String,
    },
    /// Sora-Proof base64 decode failed.
    #[error("failed to decode Sora-Proof header in `{path}`: {source}")]
    InvalidProofBase64 {
        /// Binding path.
        path: PathBuf,
        /// Underlying decode error.
        #[source]
        source: base64::DecodeError,
    },
    /// Sora-Proof payload could not be parsed.
    #[error("Sora-Proof payload in `{path}` is not valid JSON: {source}")]
    InvalidProofJson {
        /// Binding path.
        path: PathBuf,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
    /// Required field missing from decoded proof.
    #[error("Sora-Proof payload in `{path}` is missing `{field}`")]
    ProofMissingField {
        /// Binding path.
        path: PathBuf,
        /// Missing field name.
        field: &'static str,
    },
    /// Alias contained in decoded proof mismatched the binding alias.
    #[error("Sora-Proof alias `{alias}` differs from binding alias `{expected}` in `{path}`")]
    ProofAliasMismatch {
        /// Binding path.
        path: PathBuf,
        /// Alias found in proof payload.
        alias: String,
        /// Expected alias.
        expected: String,
    },
    /// Proof manifest mismatched the declared content CID.
    #[error("Sora-Proof manifest `{manifest}` does not match content CID `{expected}` in `{path}`")]
    ProofManifestMismatch {
        /// Binding path.
        path: PathBuf,
        /// Manifest string.
        manifest: String,
        /// Expected CID.
        expected: String,
    },
    /// `Sora-Route-Binding` header missing a required component.
    #[error("Sora-Route-Binding header in `{path}` is missing `{key}`")]
    MissingRouteComponent {
        /// Binding path.
        path: PathBuf,
        /// Missing component.
        key: &'static str,
    },
    /// Hostname recorded in the route binding header mismatched expectations.
    #[error("Sora-Route-Binding host `{host}` does not match expected `{expected}` in `{path}`")]
    RouteHostMismatch {
        /// Binding path.
        path: PathBuf,
        /// Expected host.
        expected: String,
        /// Host discovered in header.
        host: String,
    },
    /// CID recorded in the route binding failed validation.
    #[error("Sora-Route-Binding CID `{cid}` does not match `{expected}` in `{path}`")]
    RouteCidMismatch {
        /// Binding path.
        path: PathBuf,
        /// Expected CID.
        expected: String,
        /// CID discovered in header.
        cid: String,
    },
    /// Timestamp recorded in the route binding differs from payload metadata.
    #[error(
        "Sora-Route-Binding generated_at `{timestamp}` does not match `{expected}` in `{path}`"
    )]
    RouteTimestampMismatch {
        /// Binding path.
        path: PathBuf,
        /// Expected timestamp.
        expected: String,
        /// Timestamp discovered in header.
        timestamp: String,
    },
    /// Alias host derivation failed.
    #[error("failed to derive canonical host for alias `{alias}` in `{path}`: {source}")]
    AliasDerive {
        /// Binding path.
        path: PathBuf,
        /// Alias literal.
        alias: String,
        /// Underlying error.
        #[source]
        source: GatewayHostError,
    },
}

/// Derive canonical/pretty gateway hosts for every supplied SoraDNS name.
pub fn derive_host_summaries(names: &[String]) -> Result<Vec<HostSummary>, HostSummaryError> {
    let mut summaries = Vec::with_capacity(names.len());
    for name in names {
        let bindings = derive_gateway_hosts(name).map_err(|source| HostSummaryError::Derive {
            name: name.clone(),
            source,
        })?;
        let summary = HostSummary::from_bindings(name, &bindings);
        validate_canonical_derivation(&summary)?;
        summaries.push(summary);
    }
    Ok(summaries)
}

/// Build a deterministic ACME certificate plan for the supplied aliases.
pub fn build_acme_plan(options: &AcmePlanOptions) -> Result<AcmePlan, HostSummaryError> {
    let summaries = derive_host_summaries(&options.names)?;
    let generated_at = options
        .generated_at
        .format(&Rfc3339)
        .expect("RFC3339 formatting is infallible");
    let mut hosts = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let mut certificates = Vec::new();
        if options.include_canonical_wildcard {
            let dns_label = format!("_acme-challenge.{}", summary.canonical_host);
            certificates.push(AcmeCertificatePlan {
                kind: AcmeCertificateKind::CanonicalWildcard,
                san: vec![
                    format!("*.{}", summary.canonical_host),
                    summary.canonical_host.clone(),
                ],
                recommended_challenges: vec!["dns-01".to_string()],
                dns_challenge_labels: vec![dns_label],
                notes: vec![
                    "Wildcard orders require DNS-01; the automation controller updates this label via torii.sorafs_gateway.acme.dns_provider.".to_string(),
                    "Include the apex host alongside the wildcard so the canonical endpoint can terminate TLS without SNI rewrites.".to_string(),
                ],
            });
        }
        if options.include_pretty_hosts {
            certificates.push(AcmeCertificatePlan {
                kind: AcmeCertificateKind::PrettyHost,
                san: vec![summary.pretty_host.clone()],
                recommended_challenges: vec!["tls-alpn-01".to_string(), "http-01".to_string()],
                dns_challenge_labels: Vec::new(),
                notes: vec![
                    "Prefer TLS-ALPN-01 so automation can request per-host certificates without DNS delegation changes.".to_string(),
                    "HTTP-01 remains available for manual recovery; ensure /.well-known/acme-challenge is routed to the gateway when used.".to_string(),
                ],
            });
        }
        hosts.push(AcmePlanHost {
            name: summary.name.clone(),
            normalized_name: summary.normalized_name.clone(),
            canonical_label: summary.canonical_label.clone(),
            canonical_host: summary.canonical_host.clone(),
            canonical_wildcard: format!("*.{}", summary.canonical_host),
            pretty_host: summary.pretty_host.clone(),
            certificates,
        });
    }
    Ok(AcmePlan {
        directory_url: options.directory_url.clone(),
        generated_at,
        hosts,
    })
}

/// Build a deterministic cache invalidation plan for the supplied aliases.
pub fn build_cache_invalidation_plan(
    options: &CacheInvalidationPlanOptions,
) -> Result<CacheInvalidationPlan, HostSummaryError> {
    let mut summaries = derive_host_summaries(&options.names)?;
    summaries.sort_by(|lhs, rhs| lhs.normalized_name.cmp(&rhs.normalized_name));
    let generated_at = options
        .generated_at
        .format(&Rfc3339)
        .expect("RFC3339 formatting is infallible");
    let mut deduped_paths: Vec<String> = BTreeSet::from_iter(
        options
            .paths
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    )
    .into_iter()
    .collect();
    if deduped_paths.is_empty() {
        deduped_paths.push("/".to_string());
    }
    let mut entries = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let mut targets = Vec::new();
        targets.push(CachePurgeTarget {
            host: summary.canonical_host.clone(),
            paths: deduped_paths.clone(),
        });
        if options.include_pretty_hosts {
            targets.push(CachePurgeTarget {
                host: summary.pretty_host.clone(),
                paths: deduped_paths.clone(),
            });
        }
        entries.push(CacheInvalidationEntry {
            name: summary.name.clone(),
            normalized_name: summary.normalized_name.clone(),
            canonical_label: summary.canonical_label.clone(),
            canonical_host: summary.canonical_host.clone(),
            canonical_wildcard: format!("*.{}", summary.canonical_host),
            pretty_host: options
                .include_pretty_hosts
                .then(|| summary.pretty_host.clone()),
            purge_targets: targets,
        });
    }
    Ok(CacheInvalidationPlan {
        generated_at,
        http_method: options.http_method.clone(),
        auth_header: options.auth_header.clone(),
        auth_env: options.auth_env.clone(),
        entries,
    })
}

/// Build a promotion/rollback plan for the supplied aliases.
pub fn build_route_plan(options: &RoutePlanOptions) -> Result<RoutePlan, HostSummaryError> {
    let mut summaries = derive_host_summaries(&options.names)?;
    summaries.sort_by(|lhs, rhs| lhs.normalized_name.cmp(&rhs.normalized_name));
    let generated_at = options
        .generated_at
        .format(&Rfc3339)
        .expect("RFC3339 formatting is infallible");
    let mut entries = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let canonical_promote = RoutePlanStep {
            action: "preflight_canonical".to_string(),
            description: "Run health checks against the canonical host before promotion"
                .to_string(),
            host: Some(summary.canonical_host.clone()),
            notes: Some("HEAD/GET https://<canonical>/healthz + verify Sora-* headers".to_string()),
        };
        let canonical_cutover = RoutePlanStep {
            action: "promote_canonical_binding".to_string(),
            description: "Attach the new manifest to the canonical host and publish GAR evidence"
                .to_string(),
            host: Some(summary.canonical_host.clone()),
            notes: Some("Update portal.gateway.binding.json + upload signed bundle".to_string()),
        };
        let canonical_observe = RoutePlanStep {
            action: "observe_canonical_metrics".to_string(),
            description:
                "Monitor gateway metrics and alerting for the canonical host for 5 minutes".into(),
            host: Some(summary.canonical_host.clone()),
            notes: Some("Grafana: gateway latency/cache/cert dashboards".into()),
        };
        let mut promote_steps = vec![canonical_promote, canonical_cutover, canonical_observe];
        let mut rollback_steps = vec![
            RoutePlanStep {
                action: "rollback_canonical_binding".to_string(),
                description: "Re-attach the previous manifest bundle to the canonical host".into(),
                host: Some(summary.canonical_host.clone()),
                notes: Some("Use last known-good binding snapshot".into()),
            },
            RoutePlanStep {
                action: "verify_canonical_revert".to_string(),
                description: "Hit canonical host and confirm old content + headers reappear".into(),
                host: Some(summary.canonical_host.clone()),
                notes: Some("HEAD/GET sanity check".into()),
            },
        ];
        if options.include_pretty_hosts {
            promote_steps.push(RoutePlanStep {
                action: "promote_pretty_binding".to_string(),
                description: "Flip the pretty host alias to the new manifest bundle".into(),
                host: Some(summary.pretty_host.clone()),
                notes: Some("Ensure DNS + bindings reference the canonical ID".into()),
            });
            promote_steps.push(RoutePlanStep {
                action: "purge_pretty_cache".to_string(),
                description: "Purge CDN/cache for the pretty host after promotion".into(),
                host: Some(summary.pretty_host.clone()),
                notes: Some("Reuse cache plan entries for this host".into()),
            });
            rollback_steps.insert(
                0,
                RoutePlanStep {
                    action: "rollback_pretty_binding".to_string(),
                    description: "Restore the previous pretty host binding".into(),
                    host: Some(summary.pretty_host.clone()),
                    notes: Some("Reapply last known-good Sora-* headers".into()),
                },
            );
        }
        entries.push(RoutePlanEntry {
            name: summary.name.clone(),
            normalized_name: summary.normalized_name.clone(),
            canonical_host: summary.canonical_host.clone(),
            pretty_host: options
                .include_pretty_hosts
                .then(|| summary.pretty_host.clone()),
            promote_steps,
            rollback_steps,
        });
    }
    Ok(RoutePlan {
        generated_at,
        entries,
    })
}

fn validate_canonical_derivation(summary: &HostSummary) -> Result<(), HostSummaryError> {
    let digest = blake3_hash(summary.normalized_name.as_bytes());
    let expected_label = encode_base32_lower(digest.as_bytes());
    let expected_host = format!("{expected_label}.{}", canonical_gateway_suffix());
    if summary.canonical_label != expected_label || summary.canonical_host != expected_host {
        return Err(HostSummaryError::CanonicalDerivationMismatch {
            name: summary.name.clone(),
            expected_label,
            expected_host,
            found_label: summary.canonical_label.clone(),
            found_host: summary.canonical_host.clone(),
        });
    }
    Ok(())
}

/// Verify `portal.gateway.binding.json` (or embedded `gateway_binding`) payloads.
pub fn verify_gateway_binding(
    path: &Path,
    expectations: &GatewayBindingExpectations,
) -> Result<GatewayBindingSummary, GatewayBindingVerifyError> {
    let binding_path = path.to_path_buf();
    let bytes = fs::read(path).map_err(|source| GatewayBindingVerifyError::Io {
        path: binding_path.clone(),
        source,
    })?;
    let record: GatewayBindingRecord =
        serde_json::from_slice(&bytes).map_err(|source| GatewayBindingVerifyError::Parse {
            path: binding_path.clone(),
            source,
        })?;
    let headers =
        record
            .headers
            .as_ref()
            .ok_or_else(|| GatewayBindingVerifyError::MissingHeaders {
                path: binding_path.clone(),
            })?;

    let alias_header = require_header(headers, "Sora-Name", &binding_path)?.trim();
    if alias_header.is_empty() {
        return Err(GatewayBindingVerifyError::MissingAlias { path: binding_path });
    }
    let alias_value = pick_string(
        expectations.alias.as_ref(),
        record.alias.as_ref(),
        alias_header,
    );
    if alias_value != alias_header {
        return Err(GatewayBindingVerifyError::AliasMismatch {
            path: binding_path.clone(),
            expected: alias_value,
            found: alias_header.to_string(),
        });
    }
    let alias = alias_header.to_string();
    let alias_bindings =
        derive_gateway_hosts(&alias).map_err(|source| GatewayBindingVerifyError::AliasDerive {
            path: binding_path.clone(),
            alias: alias.clone(),
            source,
        })?;

    let header_cid = require_header(headers, "Sora-Content-CID", path)?.trim();
    if header_cid.is_empty() {
        return Err(GatewayBindingVerifyError::MissingContentCid {
            path: binding_path.clone(),
        });
    }
    if let Some(json_cid) = normalize_owned(record.content_cid.as_ref())
        && json_cid != header_cid
    {
        return Err(GatewayBindingVerifyError::ContentCidMismatch {
            path: binding_path.clone(),
            expected: json_cid,
            found: header_cid.to_string(),
        });
    }
    let mut expected_content_cid = normalize_owned(expectations.content_cid.as_ref());
    if expected_content_cid.is_none()
        && let Some(manifest_path) = expectations.manifest_path.as_ref()
    {
        expected_content_cid = Some(manifest_content_cid(manifest_path)?);
    }
    let content_cid = header_cid.to_string();
    if let Some(expected_cid) = expected_content_cid
        && expected_cid != content_cid
    {
        return Err(GatewayBindingVerifyError::ContentCidMismatch {
            path: binding_path.clone(),
            expected: expected_cid,
            found: content_cid.clone(),
        });
    }

    let proof_status_header = require_header(headers, "Sora-Proof-Status", path)?.trim();
    if let Some(json_status) = normalize_owned(record.proof_status.as_ref())
        && json_status != proof_status_header
    {
        return Err(GatewayBindingVerifyError::ProofStatusMismatch {
            path: binding_path.clone(),
            expected: json_status,
            found: proof_status_header.to_string(),
        });
    }
    if let Some(expected_status) = normalize_owned(expectations.proof_status.as_ref())
        && expected_status != proof_status_header
    {
        return Err(GatewayBindingVerifyError::ProofStatusMismatch {
            path: binding_path.clone(),
            expected: expected_status,
            found: proof_status_header.to_string(),
        });
    }
    let proof_status = proof_status_header.to_string();

    let proof_header = require_header(headers, "Sora-Proof", path)?;
    let proof_bytes = BASE64_STANDARD
        .decode(proof_header.as_bytes())
        .map_err(|source| GatewayBindingVerifyError::InvalidProofBase64 {
            path: binding_path.clone(),
            source,
        })?;
    let proof_value: SerdeJsonValue = serde_json::from_slice(&proof_bytes).map_err(|source| {
        GatewayBindingVerifyError::InvalidProofJson {
            path: binding_path.clone(),
            source,
        }
    })?;
    let proof_alias = proof_value
        .get("alias")
        .and_then(SerdeJsonValue::as_str)
        .ok_or_else(|| GatewayBindingVerifyError::ProofMissingField {
            path: binding_path.clone(),
            field: "alias",
        })?
        .trim()
        .to_string();
    if proof_alias != alias {
        return Err(GatewayBindingVerifyError::ProofAliasMismatch {
            path: binding_path.clone(),
            alias: proof_alias,
            expected: alias.clone(),
        });
    }
    let proof_manifest = proof_value
        .get("manifest")
        .and_then(SerdeJsonValue::as_str)
        .ok_or_else(|| GatewayBindingVerifyError::ProofMissingField {
            path: binding_path.clone(),
            field: "manifest",
        })?
        .trim()
        .to_string();
    if proof_manifest != content_cid {
        return Err(GatewayBindingVerifyError::ProofManifestMismatch {
            path: binding_path.clone(),
            manifest: proof_manifest,
            expected: content_cid.clone(),
        });
    }

    let route_binding = require_header(headers, "Sora-Route-Binding", path)?;
    let route_parts = parse_route_binding(route_binding);
    let route_cid =
        route_parts
            .get("cid")
            .ok_or_else(|| GatewayBindingVerifyError::MissingRouteComponent {
                path: binding_path.clone(),
                key: "cid",
            })?;
    if route_cid != &content_cid {
        return Err(GatewayBindingVerifyError::RouteCidMismatch {
            path: binding_path.clone(),
            expected: content_cid.clone(),
            cid: route_cid.clone(),
        });
    }
    let route_host = route_parts.get("host").cloned().ok_or_else(|| {
        GatewayBindingVerifyError::MissingRouteComponent {
            path: binding_path.clone(),
            key: "host",
        }
    })?;
    let hostname = pick_optional_string(
        expectations.hostname.as_ref(),
        record.hostname.as_ref(),
        Some(route_host.as_str()),
    );
    if let Some(expected_host) = hostname.as_ref()
        && route_host != *expected_host
    {
        return Err(GatewayBindingVerifyError::RouteHostMismatch {
            path: binding_path.clone(),
            expected: expected_host.clone(),
            host: route_host,
        });
    }
    let route_generated = route_parts.get("generated_at").cloned().ok_or_else(|| {
        GatewayBindingVerifyError::MissingRouteComponent {
            path: binding_path.clone(),
            key: "generated_at",
        }
    })?;
    if let Some(record_timestamp) = pick_optional_string(
        record.generated_at.as_ref(),
        None,
        Some(route_generated.as_str()),
    ) && route_generated != record_timestamp
    {
        return Err(GatewayBindingVerifyError::RouteTimestampMismatch {
            path: binding_path.clone(),
            expected: record_timestamp,
            timestamp: route_generated,
        });
    }

    // Ensure TLS/security headers are present.
    require_header(headers, "Content-Security-Policy", path)?;
    require_header(headers, "Permissions-Policy", path)?;
    require_header(headers, "Strict-Transport-Security", path)?;

    Ok(GatewayBindingSummary {
        alias: Some(alias),
        content_cid,
        hostname,
        proof_status: Some(proof_status),
        canonical_label: alias_bindings.canonical_label().to_string(),
        canonical_host: alias_bindings.canonical_host().to_string(),
    })
}

fn pick_string(
    override_value: Option<&String>,
    record_value: Option<&String>,
    fallback: &str,
) -> String {
    normalize_owned(override_value)
        .or_else(|| normalize_owned(record_value))
        .unwrap_or_else(|| fallback.to_string())
}

fn pick_optional_string(
    override_value: Option<&String>,
    record_value: Option<&String>,
    fallback: Option<&str>,
) -> Option<String> {
    normalize_owned(override_value)
        .or_else(|| normalize_owned(record_value))
        .or_else(|| normalize_str(fallback))
}

fn normalize_owned(value: Option<&String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn normalize_str(value: Option<&str>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn require_header<'a>(
    headers: &'a HashMap<String, String>,
    name: &'static str,
    path: &Path,
) -> Result<&'a str, GatewayBindingVerifyError> {
    headers
        .get(name)
        .map(|value| value.as_str())
        .ok_or_else(|| GatewayBindingVerifyError::MissingHeader {
            path: path.to_path_buf(),
            header: name,
        })
}

fn parse_route_binding(value: &str) -> BTreeMap<String, String> {
    let mut components = BTreeMap::new();
    for segment in value.split(';') {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=') {
            components.insert(key.trim().to_string(), val.trim().to_string());
        }
    }
    components
}

fn encode_base32_lower(bytes: &[u8]) -> String {
    BASE32_NOPAD.encode(bytes).to_lowercase()
}

fn manifest_content_cid(path: &Path) -> Result<String, GatewayBindingVerifyError> {
    let data = fs::read(path).map_err(|source| GatewayBindingVerifyError::ManifestIo {
        path: path.to_path_buf(),
        source,
    })?;
    let value: SerdeJsonValue = serde_json::from_slice(&data).map_err(|source| {
        GatewayBindingVerifyError::ManifestParse {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let root_bytes = manifest_root_bytes(&value, path)?;
    let encoded = BASE32_NOPAD.encode(&root_bytes).to_lowercase();
    Ok(format!("b{encoded}"))
}

fn manifest_root_bytes(
    manifest: &SerdeJsonValue,
    path: &Path,
) -> Result<Vec<u8>, GatewayBindingVerifyError> {
    if let Some(array) = manifest.get("root_cid").and_then(SerdeJsonValue::as_array) {
        let mut bytes = Vec::with_capacity(array.len());
        for entry in array {
            let Some(number) = entry.as_i64() else {
                return Err(GatewayBindingVerifyError::ManifestRootInvalid {
                    path: path.to_path_buf(),
                    details: "root_cid entries must be integers".to_string(),
                });
            };
            if !(0..=255).contains(&number) {
                return Err(GatewayBindingVerifyError::ManifestRootInvalid {
                    path: path.to_path_buf(),
                    details: format!("root_cid entry {number} is outside [0,255]"),
                });
            }
            bytes.push(number as u8);
        }
        if bytes.is_empty() {
            return Err(GatewayBindingVerifyError::ManifestRootInvalid {
                path: path.to_path_buf(),
                details: "root_cid is empty".to_string(),
            });
        }
        return Ok(bytes);
    }
    if let Some(array) = manifest
        .get("root_cids_hex")
        .and_then(SerdeJsonValue::as_array)
    {
        for entry in array {
            if let Some(hex_value) = entry.as_str() {
                return decode_manifest_hex(hex_value, path);
            }
        }
    }
    if let Some(hex_value) = manifest
        .get("root_cid_hex")
        .and_then(SerdeJsonValue::as_str)
    {
        return decode_manifest_hex(hex_value, path);
    }
    Err(GatewayBindingVerifyError::ManifestMissingRoot {
        path: path.to_path_buf(),
    })
}

fn decode_manifest_hex(value: &str, path: &Path) -> Result<Vec<u8>, GatewayBindingVerifyError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayBindingVerifyError::ManifestRootInvalid {
            path: path.to_path_buf(),
            details: "root CID hex value is empty".to_string(),
        });
    }
    let bytes =
        hex_decode(trimmed).map_err(|err| GatewayBindingVerifyError::ManifestRootInvalid {
            path: path.to_path_buf(),
            details: format!("failed to decode root CID hex: {err}"),
        })?;
    if bytes.is_empty() {
        return Err(GatewayBindingVerifyError::ManifestRootInvalid {
            path: path.to_path_buf(),
            details: "decoded root CID is empty".to_string(),
        });
    }
    Ok(bytes)
}

/// Errors encountered when verifying GAR host pattern inputs.
#[derive(Debug, Error)]
pub enum HostPatternInputError {
    /// Unable to read a verification file.
    #[error("failed to read `--verify-host-patterns` input `{path}`: {source}")]
    Io {
        /// Source path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Unable to parse verification JSON.
    #[error("failed to parse `--verify-host-patterns` input `{path}`: {source}")]
    Json {
        /// Source path.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },
    /// Verification entry missing a `name`.
    #[error("verification entry missing `name` field ({context})")]
    MissingName {
        /// Context describing the entry (usually a filename).
        context: String,
    },
    /// Verification entry provided a non-string `name`.
    #[error("verification entry name must be a string ({context})")]
    InvalidNameType {
        /// Description of entry location.
        context: String,
    },
    /// Verification entry missing the `host_patterns` array.
    #[error("verification entry `{name}` is missing `host_patterns`")]
    MissingPatterns {
        /// Entry name.
        name: String,
    },
    /// Verification entry provided a non-array `host_patterns`.
    #[error("verification entry `{name}` must list host_patterns as an array of strings")]
    InvalidPatternList {
        /// Entry name.
        name: String,
    },
    /// Verification entry contained a non-string host pattern.
    #[error("verification entry `{name}` contains a non-string host pattern")]
    InvalidPatternValue {
        /// Entry name.
        name: String,
    },
    /// Verification entry contained zero valid patterns.
    #[error("verification entry `{name}` did not provide any host patterns")]
    EmptyPatternList {
        /// Entry name.
        name: String,
    },
    /// Multiple entries map to the same name.
    #[error("verification entry `{name}` appears multiple times")]
    DuplicateEntry {
        /// Entry name.
        name: String,
    },
    /// Verification input did not include an entry for the derived name.
    #[error("verification input missing host_patterns for `{name}`")]
    EntryMissingForName {
        /// Derived name reference.
        name: String,
    },
    /// Verification input listed names that were not requested.
    #[error("verification input included host_patterns for unknown names: {names:?}")]
    UnknownNames {
        /// Remaining names after verification.
        names: Vec<String>,
    },
    /// Entry did not authorise every required host pattern.
    #[error("verification entry `{name}` missing host_patterns: {missing:?}")]
    MissingRequiredPatterns {
        /// Entry name.
        name: String,
        /// Missing host patterns.
        missing: Vec<String>,
    },
    /// JSON payload used an unsupported top-level shape.
    #[error("verification JSON structure is invalid: {details}")]
    InvalidShape {
        /// Description of the issue.
        details: String,
    },
}

/// Verify that GAR host pattern JSON contains the canonical/pretty hosts.
pub fn verify_host_patterns(
    summaries: &[HostSummary],
    inputs: &[PathBuf],
) -> Result<(), HostPatternInputError> {
    if inputs.is_empty() {
        return Ok(());
    }
    let mut pattern_map = HashMap::new();
    for path in inputs {
        let context = format!("file {}", path.display());
        let data = fs::read(path).map_err(|source| HostPatternInputError::Io {
            path: path.clone(),
            source,
        })?;
        let value: SerdeJsonValue =
            serde_json::from_slice(&data).map_err(|source| HostPatternInputError::Json {
                path: path.clone(),
                source,
            })?;
        merge_host_pattern_entries(&mut pattern_map, &value, &context)?;
    }
    verify_host_map(summaries, pattern_map)
}

/// Options for rendering a Sora gateway binding payload (`portal.gateway.binding.json`).
#[derive(Debug, Clone)]
pub struct BindingTemplateOptions {
    /// Manifest JSON describing the bundle being promoted.
    pub manifest_path: PathBuf,
    /// Alias literal copied into `Sora-Name`.
    pub alias: String,
    /// Hostname referenced by `Sora-Route-Binding`.
    pub hostname: String,
    /// Optional logical label included in the route binding metadata.
    pub route_label: Option<String>,
    /// Optional proof-status text for `Sora-Proof-Status`.
    pub proof_status: Option<String>,
    /// Optional override for the CSP header template.
    pub csp_template: Option<String>,
    /// Optional override for the Permissions-Policy header template.
    pub permissions_template: Option<String>,
    /// Optional override for the HSTS header template.
    pub hsts_template: Option<String>,
    /// Whether to emit the default CSP header.
    pub include_csp: bool,
    /// Whether to emit the default Permissions-Policy header.
    pub include_permissions: bool,
    /// Whether to emit the default HSTS header.
    pub include_hsts: bool,
    /// Timestamp embedded in the route binding metadata.
    pub generated_at: OffsetDateTime,
}

/// Rendered binding payload and the formatted header block.
#[derive(Debug, Clone)]
pub struct BindingTemplateRender {
    /// JSON payload mirroring `portal.gateway.binding.json`.
    pub payload: SerdeJsonValue,
    /// Ready-to-paste HTTP header block.
    pub headers_template: String,
}

/// Errors surfaced while rendering gateway bindings.
#[derive(Debug, Error)]
pub enum BindingTemplateError {
    /// Manifest file could not be read.
    #[error("failed to read manifest `{path}`: {source}")]
    ManifestIo {
        /// Manifest path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Manifest JSON parse failure.
    #[error("failed to parse manifest `{path}`: {source}")]
    ManifestParse {
        /// Manifest path.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: norito::json::Error,
    },
    /// Manifest lacked a usable root CID payload.
    #[error("manifest `{path}` is missing root CID data: {details}")]
    ManifestRoot {
        /// Manifest path.
        path: PathBuf,
        /// Details about the missing/invalid payload.
        details: String,
    },
    /// Alias literal was missing or empty.
    #[error("alias must not be empty")]
    MissingAlias,
    /// Hostname literal was missing or empty.
    #[error("hostname must not be empty")]
    MissingHostname,
    /// Header override supplied while the header was disabled.
    #[error("header `{header}` override requires enabling the header (remove `--no-*`)")]
    HeaderOverrideDisabled {
        /// Header name.
        header: &'static str,
    },
    /// Header template override was empty.
    #[error("header `{header}` template must not be empty")]
    HeaderTemplateEmpty {
        /// Header name.
        header: &'static str,
    },
}

/// Render a deterministic `portal.gateway.binding.json` payload.
pub fn build_binding_template(
    options: &BindingTemplateOptions,
) -> Result<BindingTemplateRender, BindingTemplateError> {
    let alias = options.alias.trim();
    if alias.is_empty() {
        return Err(BindingTemplateError::MissingAlias);
    }
    let hostname = options.hostname.trim();
    if hostname.is_empty() {
        return Err(BindingTemplateError::MissingHostname);
    }

    let manifest_bytes =
        fs::read(&options.manifest_path).map_err(|source| BindingTemplateError::ManifestIo {
            path: options.manifest_path.clone(),
            source,
        })?;
    let manifest: NoritoJsonValue =
        norito::json::from_slice(&manifest_bytes).map_err(|source| {
            BindingTemplateError::ManifestParse {
                path: options.manifest_path.clone(),
                source,
            }
        })?;
    let root_bytes = manifest_root_bytes_binding(&manifest, &options.manifest_path)?;
    let content_cid = format!("b{}", encode_base32_lower(&root_bytes));
    let generated_at = options.generated_at.format(&Rfc3339).map_err(|err| {
        BindingTemplateError::ManifestRoot {
            path: options.manifest_path.clone(),
            details: format!("failed to format timestamp: {err}"),
        }
    })?;

    let mut headers = BTreeMap::new();
    headers.insert("Sora-Content-CID".into(), content_cid.clone());
    headers.insert("Sora-Name".into(), alias.to_string());
    let proof_payload = serde_json::json!({
        "alias": alias,
        "manifest": content_cid,
    });
    let proof_bytes =
        serde_json::to_vec(&proof_payload).map_err(|err| BindingTemplateError::ManifestRoot {
            path: options.manifest_path.clone(),
            details: format!("failed to encode proof payload: {err}"),
        })?;
    headers.insert("Sora-Proof".into(), BASE64_STANDARD.encode(proof_bytes));
    let proof_status = options
        .proof_status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("ok");
    headers.insert("Sora-Proof-Status".into(), proof_status.to_string());

    let mut route_parts = vec![
        format!("host={hostname}"),
        format!("cid={content_cid}"),
        format!("generated_at={generated_at}"),
    ];
    if let Some(label) = options
        .route_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        route_parts.push(format!("label={label}"));
    }
    headers.insert("Sora-Route-Binding".into(), route_parts.join(";"));

    resolve_header_template(
        &mut headers,
        "Content-Security-Policy",
        options.include_csp,
        options.csp_template.as_deref(),
        DEFAULT_CSP_TEMPLATE,
    )?;
    resolve_header_template(
        &mut headers,
        "Strict-Transport-Security",
        options.include_hsts,
        options.hsts_template.as_deref(),
        DEFAULT_HSTS_TEMPLATE,
    )?;
    resolve_header_template(
        &mut headers,
        "Permissions-Policy",
        options.include_permissions,
        options.permissions_template.as_deref(),
        DEFAULT_PERMISSIONS_POLICY,
    )?;

    let headers_template = format_headers_template(&headers);
    let mut payload = serde_json::Map::new();
    payload.insert("alias".into(), SerdeJsonValue::String(alias.to_string()));
    payload.insert(
        "hostname".into(),
        SerdeJsonValue::String(hostname.to_string()),
    );
    payload.insert(
        "contentCid".into(),
        SerdeJsonValue::String(content_cid.clone()),
    );
    payload.insert(
        "proofStatus".into(),
        SerdeJsonValue::String(proof_status.to_string()),
    );
    payload.insert(
        "generatedAt".into(),
        SerdeJsonValue::String(generated_at.clone()),
    );
    payload.insert(
        "headersTemplate".into(),
        SerdeJsonValue::String(headers_template.clone()),
    );
    payload.insert(
        "headers".into(),
        SerdeJsonValue::Object(headers_to_value(&headers)),
    );
    if let Some(label) = options
        .route_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        payload.insert(
            "routeLabel".into(),
            SerdeJsonValue::String(label.to_string()),
        );
    }

    Ok(BindingTemplateRender {
        payload: SerdeJsonValue::Object(payload),
        headers_template,
    })
}

fn manifest_root_bytes_binding(
    manifest: &NoritoJsonValue,
    manifest_path: &Path,
) -> Result<Vec<u8>, BindingTemplateError> {
    if let Some(array) = manifest.get("root_cid").and_then(NoritoJsonValue::as_array) {
        let mut bytes = Vec::with_capacity(array.len());
        for entry in array {
            let Some(number) = entry.as_i64() else {
                return Err(BindingTemplateError::ManifestRoot {
                    path: manifest_path.to_path_buf(),
                    details: "root_cid entries must be integers".to_string(),
                });
            };
            if !(0..=255).contains(&number) {
                return Err(BindingTemplateError::ManifestRoot {
                    path: manifest_path.to_path_buf(),
                    details: format!("root_cid entry {number} is outside [0,255]"),
                });
            }
            bytes.push(number as u8);
        }
        if bytes.is_empty() {
            return Err(BindingTemplateError::ManifestRoot {
                path: manifest_path.to_path_buf(),
                details: "root_cid is empty".to_string(),
            });
        }
        return Ok(bytes);
    }
    if let Some(array) = manifest
        .get("root_cids_hex")
        .and_then(NoritoJsonValue::as_array)
    {
        for entry in array {
            if let Some(hex_value) = entry.as_str() {
                let trimmed = hex_value.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let decoded =
                    hex_decode(trimmed).map_err(|err| BindingTemplateError::ManifestRoot {
                        path: manifest_path.to_path_buf(),
                        details: format!("failed to decode root_cids_hex entry: {err}"),
                    })?;
                if decoded.is_empty() {
                    continue;
                }
                return Ok(decoded);
            }
        }
    }
    if let Some(hex_value) = manifest
        .get("root_cid_hex")
        .and_then(NoritoJsonValue::as_str)
    {
        let trimmed = hex_value.trim();
        if trimmed.is_empty() {
            return Err(BindingTemplateError::ManifestRoot {
                path: manifest_path.to_path_buf(),
                details: "root_cid_hex was empty".to_string(),
            });
        }
        return hex_decode(trimmed).map_err(|err| BindingTemplateError::ManifestRoot {
            path: manifest_path.to_path_buf(),
            details: format!("failed to decode root_cid_hex: {err}"),
        });
    }
    Err(BindingTemplateError::ManifestRoot {
        path: manifest_path.to_path_buf(),
        details: "manifest is missing root CID fields".to_string(),
    })
}

fn resolve_header_template(
    headers: &mut BTreeMap<String, String>,
    name: &'static str,
    include: bool,
    override_value: Option<&str>,
    default_template: &'static str,
) -> Result<(), BindingTemplateError> {
    if override_value.is_some() && !include {
        return Err(BindingTemplateError::HeaderOverrideDisabled { header: name });
    }
    if !include {
        return Ok(());
    }
    let template = match override_value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(BindingTemplateError::HeaderTemplateEmpty { header: name });
            }
            trimmed.to_string()
        }
        None => default_template.to_string(),
    };
    headers.insert(name.to_string(), template);
    Ok(())
}

/// Options for constructing a GAR template payload.
#[derive(Debug, Clone)]
pub struct GarTemplateOptions {
    /// Registered SoraDNS name (may include uppercase letters; helper normalises it).
    pub name: String,
    /// Manifest CID authorised by the GAR.
    pub manifest_cid: String,
    /// Optional BLAKE3-256 digest (hex) of the manifest.
    pub manifest_digest: Option<String>,
    /// Optional override for the CSP template.
    pub csp_template: Option<String>,
    /// Optional override for the HSTS template.
    pub hsts_template: Option<String>,
    /// Optional override for the Permissions-Policy header template.
    pub permissions_template: Option<String>,
    /// Start of the GAR validity window (Unix epoch seconds).
    pub valid_from_epoch: u64,
    /// End of the validity window (Unix epoch seconds).
    pub valid_until_epoch: Option<u64>,
    /// Telemetry labels embedded in the policy payload.
    pub telemetry_labels: Vec<String>,
}

/// Errors encountered while rendering GAR templates.
#[derive(Debug, Error)]
pub enum GarTemplateError {
    /// Gateway host derivation failed.
    #[error("failed to derive deterministic hosts: {0}")]
    Host(#[from] GatewayHostError),
    /// Manifest CID is empty.
    #[error("manifest_cid must not be empty")]
    EmptyManifestCid,
    /// Manifest digest failed validation.
    #[error("manifest digest must be a 64-character hex string")]
    InvalidManifestDigest,
}

/// Build a GAR payload template carrying canonical host patterns and header defaults.
pub fn build_gar_template(
    options: &GarTemplateOptions,
) -> Result<SerdeJsonValue, GarTemplateError> {
    let bindings = derive_gateway_hosts(&options.name)?;
    let manifest_cid = options.manifest_cid.trim();
    if manifest_cid.is_empty() {
        return Err(GarTemplateError::EmptyManifestCid);
    }
    let manifest_digest = options
        .manifest_digest
        .as_ref()
        .map(|digest| normalize_manifest_digest(digest))
        .transpose()?;
    let host_patterns = bindings
        .host_patterns()
        .iter()
        .map(|pattern| SerdeJsonValue::from(*pattern))
        .collect::<Vec<_>>();
    let telemetry_labels = options
        .telemetry_labels
        .iter()
        .map(|label| label.trim())
        .filter(|label| !label.is_empty())
        .map(|label| label.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(SerdeJsonValue::from)
        .collect::<Vec<_>>();
    let csp_template = options
        .csp_template
        .as_deref()
        .unwrap_or(DEFAULT_CSP_TEMPLATE)
        .to_string();
    let hsts_template = options
        .hsts_template
        .as_deref()
        .unwrap_or(DEFAULT_HSTS_TEMPLATE)
        .to_string();
    let permissions_template = options
        .permissions_template
        .as_deref()
        .unwrap_or(DEFAULT_PERMISSIONS_POLICY)
        .to_string();

    Ok(json!({
        "version": 2,
        "name": bindings.normalized_name(),
        "manifest_cid": manifest_cid,
        "manifest_digest": manifest_digest,
        "host_patterns": host_patterns,
        "csp_template": csp_template,
        "hsts_template": hsts_template,
        "permissions_template": permissions_template,
        "valid_from_epoch": options.valid_from_epoch,
        "valid_until_epoch": options.valid_until_epoch,
        "license_sets": [],
        "moderation_directives": [],
        "metrics_policy": SerdeJsonValue::Null,
        "telemetry_labels": telemetry_labels,
        "rpt_digest": SerdeJsonValue::Null,
    }))
}

pub fn normalize_manifest_digest(value: &str) -> Result<String, GarTemplateError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GarTemplateError::InvalidManifestDigest);
    }
    if trimmed.len() != 64 || trimmed.chars().any(|ch| !ch.is_ascii_hexdigit()) {
        return Err(GarTemplateError::InvalidManifestDigest);
    }
    Ok(trimmed.to_ascii_lowercase())
}

/// Verification options supplied when validating GAR payloads.
#[derive(Debug, Clone, Default)]
pub struct GarVerifyOptions {
    /// Registered SoraDNS name used to derive canonical hosts.
    pub name: String,
    /// Optional manifest CID expectation.
    pub expected_manifest_cid: Option<String>,
    /// Optional manifest digest expectation.
    pub expected_manifest_digest: Option<String>,
    /// Telemetry labels that must appear in the GAR payload.
    pub required_telemetry_labels: Vec<String>,
}

/// Summary returned after validating a GAR payload.
#[derive(Debug, Clone)]
pub struct GarVerifySummary {
    /// GAR schema version.
    pub version: u64,
    /// Normalised SoraDNS name recorded in the payload.
    pub normalized_name: String,
    /// Canonical label derived from the normalized name.
    pub canonical_label: String,
    /// Canonical host derived from the normalized name.
    pub canonical_host: String,
    /// Manifest CID authorised by the GAR.
    pub manifest_cid: String,
    /// Optional manifest digest.
    pub manifest_digest: Option<String>,
    /// Host patterns authorised by the GAR.
    pub host_patterns: Vec<String>,
    /// Telemetry labels embedded in the payload.
    pub telemetry_labels: Vec<String>,
    /// Start of the validity window.
    pub valid_from_epoch: u64,
    /// Optional end of the validity window.
    pub valid_until_epoch: Option<u64>,
}

impl GarVerifySummary {
    /// Render the GAR verification summary as JSON for automation.
    pub fn to_json_value(&self) -> SerdeJsonValue {
        let host_patterns = self
            .host_patterns
            .iter()
            .map(|pattern| SerdeJsonValue::String(pattern.clone()))
            .collect::<Vec<_>>();
        let telemetry_labels = self
            .telemetry_labels
            .iter()
            .map(|label| SerdeJsonValue::String(label.clone()))
            .collect::<Vec<_>>();
        json!({
            "version": self.version,
            "name": self.normalized_name,
            "canonical_label": self.canonical_label,
            "canonical_host": self.canonical_host,
            "manifest_cid": self.manifest_cid,
            "manifest_digest": self.manifest_digest,
            "host_patterns": host_patterns,
            "telemetry_labels": telemetry_labels,
            "valid_from_epoch": self.valid_from_epoch,
            "valid_until_epoch": self.valid_until_epoch,
        })
    }
}

/// Errors raised while validating GAR payloads.
#[derive(Debug, Error)]
pub enum GarVerifyError {
    /// Unable to read the GAR payload.
    #[error("failed to read GAR `{path}`: {source}")]
    Io {
        /// Source path.
        path: PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    /// Unable to parse the GAR payload.
    #[error("failed to parse GAR `{path}`: {source}")]
    Parse {
        /// Source path.
        path: PathBuf,
        /// Underlying parse error.
        #[source]
        source: serde_json::Error,
    },
    /// Required field missing in the payload.
    #[error("GAR `{path}` is missing `{field}`")]
    MissingField {
        /// Source path.
        path: PathBuf,
        /// Missing field.
        field: &'static str,
    },
    /// Field present but invalid.
    #[error("GAR `{path}` has invalid `{field}`: {details}")]
    InvalidField {
        /// Source path.
        path: PathBuf,
        /// Field name.
        field: &'static str,
        /// Description of the issue.
        details: String,
    },
    /// The payload recorded an unexpected name.
    #[error("GAR `{path}` name `{found}` does not match expected `{expected}`")]
    NameMismatch {
        /// Source path.
        path: PathBuf,
        /// Expected normalised name.
        expected: String,
        /// Name found in the payload.
        found: String,
    },
    /// Required host patterns were missing.
    #[error("GAR `{path}` host_patterns missing entries: {missing:?}")]
    MissingHostPatterns {
        /// Source path.
        path: PathBuf,
        /// Missing host patterns.
        missing: Vec<String>,
    },
    /// Manifest CID mismatch.
    #[error("GAR `{path}` manifest_cid `{found}` does not match expected `{expected}`")]
    ManifestCidMismatch {
        /// Source path.
        path: PathBuf,
        /// Expected manifest CID.
        expected: String,
        /// Manifest CID present in the payload.
        found: String,
    },
    /// Manifest digest mismatch.
    #[error("GAR `{path}` manifest_digest `{found}` does not match expected `{expected}`")]
    ManifestDigestMismatch {
        /// Source path.
        path: PathBuf,
        /// Expected digest.
        expected: String,
        /// Digest found in the payload.
        found: String,
    },
    /// Required telemetry labels missing.
    #[error("GAR `{path}` telemetry labels missing: {missing:?}")]
    MissingTelemetryLabels {
        /// Source path.
        path: PathBuf,
        /// Missing labels.
        missing: Vec<String>,
    },
    /// Failed to derive deterministic hosts for the provided name.
    #[error("failed to derive deterministic hosts for `{name}`: {source}")]
    HostDerive {
        /// Provided name.
        name: String,
        /// Derivation error.
        #[source]
        source: GatewayHostError,
    },
}

/// Validate a GAR payload against the deterministic host policy.
pub fn verify_gar_payload(
    gar_path: &Path,
    options: &GarVerifyOptions,
) -> Result<GarVerifySummary, GarVerifyError> {
    let path = gar_path.to_path_buf();
    let raw = fs::read(gar_path).map_err(|source| GarVerifyError::Io {
        path: path.clone(),
        source,
    })?;
    let value: SerdeJsonValue =
        serde_json::from_slice(&raw).map_err(|source| GarVerifyError::Parse {
            path: path.clone(),
            source,
        })?;
    let object = value
        .as_object()
        .ok_or_else(|| GarVerifyError::InvalidField {
            path: path.clone(),
            field: "<root>",
            details: "expected JSON object".to_string(),
        })?;
    let version = object
        .get("version")
        .and_then(SerdeJsonValue::as_u64)
        .ok_or_else(|| GarVerifyError::MissingField {
            path: path.clone(),
            field: "version",
        })?;
    let name_value = object
        .get("name")
        .and_then(SerdeJsonValue::as_str)
        .ok_or_else(|| GarVerifyError::MissingField {
            path: path.clone(),
            field: "name",
        })?;
    let gar_name = name_value.trim();
    if gar_name.is_empty() {
        return Err(GarVerifyError::InvalidField {
            path: path.clone(),
            field: "name",
            details: "name cannot be empty".to_string(),
        });
    }
    let manifest_cid_value = object
        .get("manifest_cid")
        .and_then(SerdeJsonValue::as_str)
        .ok_or_else(|| GarVerifyError::MissingField {
            path: path.clone(),
            field: "manifest_cid",
        })?;
    let manifest_cid = manifest_cid_value.trim();
    if manifest_cid.is_empty() {
        return Err(GarVerifyError::InvalidField {
            path: path.clone(),
            field: "manifest_cid",
            details: "manifest_cid cannot be empty".to_string(),
        });
    }
    let manifest_digest = match object.get("manifest_digest") {
        None | Some(SerdeJsonValue::Null) => Ok(None),
        Some(SerdeJsonValue::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                normalize_manifest_digest(trimmed).map(Some).map_err(|err| {
                    GarVerifyError::InvalidField {
                        path: path.clone(),
                        field: "manifest_digest",
                        details: format!("{err}"),
                    }
                })
            }
        }
        Some(other) => Err(GarVerifyError::InvalidField {
            path: path.clone(),
            field: "manifest_digest",
            details: format!("expected string or null, got {other:?}"),
        }),
    }?;
    let host_patterns_value =
        object
            .get("host_patterns")
            .ok_or_else(|| GarVerifyError::MissingField {
                path: path.clone(),
                field: "host_patterns",
            })?;
    let host_patterns_array =
        host_patterns_value
            .as_array()
            .ok_or_else(|| GarVerifyError::InvalidField {
                path: path.clone(),
                field: "host_patterns",
                details: "expected an array of strings".to_string(),
            })?;
    if host_patterns_array.is_empty() {
        return Err(GarVerifyError::InvalidField {
            path: path.clone(),
            field: "host_patterns",
            details: "host_patterns must contain at least one entry".to_string(),
        });
    }
    let mut host_patterns = Vec::new();
    for pattern in host_patterns_array {
        let Some(raw) = pattern.as_str() else {
            return Err(GarVerifyError::InvalidField {
                path: path.clone(),
                field: "host_patterns",
                details: "host pattern entries must be strings".to_string(),
            });
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(GarVerifyError::InvalidField {
                path: path.clone(),
                field: "host_patterns",
                details: "host pattern entries cannot be empty".to_string(),
            });
        }
        host_patterns.push(trimmed.to_string());
    }
    let valid_from_epoch = object
        .get("valid_from_epoch")
        .and_then(SerdeJsonValue::as_u64)
        .ok_or_else(|| GarVerifyError::MissingField {
            path: path.clone(),
            field: "valid_from_epoch",
        })?;
    let valid_until_epoch = match object.get("valid_until_epoch") {
        None | Some(SerdeJsonValue::Null) => None,
        Some(value) => Some(value.as_u64().ok_or_else(|| GarVerifyError::InvalidField {
            path: path.clone(),
            field: "valid_until_epoch",
            details: "expected unsigned integer".to_string(),
        })?),
    };
    let telemetry_labels = match object.get("telemetry_labels") {
        None | Some(SerdeJsonValue::Null) => Vec::new(),
        Some(value) => {
            let array = value
                .as_array()
                .ok_or_else(|| GarVerifyError::InvalidField {
                    path: path.clone(),
                    field: "telemetry_labels",
                    details: "expected an array of strings".to_string(),
                })?;
            let mut labels = Vec::new();
            for entry in array {
                let Some(raw) = entry.as_str() else {
                    return Err(GarVerifyError::InvalidField {
                        path: path.clone(),
                        field: "telemetry_labels",
                        details: "labels must be strings".to_string(),
                    });
                };
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    continue;
                }
                labels.push(trimmed.to_string());
            }
            labels
        }
    };
    let bindings =
        derive_gateway_hosts(&options.name).map_err(|source| GarVerifyError::HostDerive {
            name: options.name.clone(),
            source,
        })?;
    let summary = HostSummary::from_bindings(&options.name, &bindings);
    let summary_name = summary.normalized_name.clone();
    let normalized_payload_name = gar_name.to_ascii_lowercase();
    if normalized_payload_name != summary_name {
        return Err(GarVerifyError::NameMismatch {
            path: path.clone(),
            expected: summary_name.clone(),
            found: gar_name.to_string(),
        });
    }
    let missing = missing_required_patterns(&summary, &host_patterns);
    if !missing.is_empty() {
        return Err(GarVerifyError::MissingHostPatterns {
            path: path.clone(),
            missing,
        });
    }
    if let Some(expected_cid) = options.expected_manifest_cid.as_ref() {
        let trimmed = expected_cid.trim();
        if trimmed.is_empty() {
            return Err(GarVerifyError::ManifestCidMismatch {
                path: path.clone(),
                expected: trimmed.to_string(),
                found: manifest_cid.to_string(),
            });
        }
        if !manifest_cid.eq_ignore_ascii_case(trimmed) {
            return Err(GarVerifyError::ManifestCidMismatch {
                path: path.clone(),
                expected: trimmed.to_string(),
                found: manifest_cid.to_string(),
            });
        }
    }
    if let Some(expected_digest) = options.expected_manifest_digest.as_ref() {
        let expected = normalize_manifest_digest(expected_digest).map_err(|err| {
            GarVerifyError::InvalidField {
                path: path.clone(),
                field: "expected_manifest_digest",
                details: format!("{err}"),
            }
        })?;
        match &manifest_digest {
            Some(found) if found == &expected => {}
            Some(found) => {
                return Err(GarVerifyError::ManifestDigestMismatch {
                    path: path.clone(),
                    expected,
                    found: found.clone(),
                });
            }
            None => {
                return Err(GarVerifyError::ManifestDigestMismatch {
                    path: path.clone(),
                    expected,
                    found: "<absent>".to_string(),
                });
            }
        }
    }
    if !options.required_telemetry_labels.is_empty() {
        let actual = telemetry_labels
            .iter()
            .map(|label| label.trim().to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let mut missing = Vec::new();
        for label in &options.required_telemetry_labels {
            let trimmed = label.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !actual.contains(&trimmed.to_ascii_lowercase()) {
                missing.push(trimmed.to_string());
            }
        }
        if !missing.is_empty() {
            return Err(GarVerifyError::MissingTelemetryLabels { path, missing });
        }
    }
    Ok(GarVerifySummary {
        version,
        normalized_name: summary_name,
        canonical_label: summary.canonical_label.to_string(),
        canonical_host: summary.canonical_host.to_string(),
        manifest_cid: manifest_cid.to_string(),
        manifest_digest,
        host_patterns,
        telemetry_labels,
        valid_from_epoch,
        valid_until_epoch,
    })
}

fn merge_host_pattern_entries(
    target: &mut HashMap<String, Vec<String>>,
    value: &SerdeJsonValue,
    context: &str,
) -> Result<(), HostPatternInputError> {
    match value {
        SerdeJsonValue::Array(items) => {
            for item in items {
                merge_host_pattern_entries(target, item, context)?;
            }
            Ok(())
        }
        SerdeJsonValue::Object(map) => {
            if map.contains_key("host_patterns") {
                let name_value =
                    map.get("name")
                        .ok_or_else(|| HostPatternInputError::MissingName {
                            context: context.to_string(),
                        })?;
                let Some(name_str) = name_value.as_str() else {
                    return Err(HostPatternInputError::InvalidNameType {
                        context: context.to_string(),
                    });
                };
                let canonical_name = normalise_name_key(name_str)?;
                let patterns = parse_host_patterns(map.get("host_patterns"), name_str)?;
                insert_host_patterns(target, canonical_name, patterns)
            } else {
                for (name, entry) in map {
                    let canonical_name = normalise_name_key(name)?;
                    let patterns = parse_host_patterns(Some(entry), name)?;
                    insert_host_patterns(target, canonical_name, patterns)?;
                }
                Ok(())
            }
        }
        other => Err(HostPatternInputError::InvalidShape {
            details: format!("unexpected JSON token: {other}"),
        }),
    }
}

fn normalise_name_key(name: &str) -> Result<String, HostPatternInputError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(HostPatternInputError::MissingName {
            context: "blank name".to_string(),
        });
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn parse_host_patterns(
    value: Option<&SerdeJsonValue>,
    display_name: &str,
) -> Result<Vec<String>, HostPatternInputError> {
    let Some(raw) = value else {
        return Err(HostPatternInputError::MissingPatterns {
            name: display_name.to_string(),
        });
    };
    let arr = raw
        .as_array()
        .ok_or_else(|| HostPatternInputError::InvalidPatternList {
            name: display_name.to_string(),
        })?;
    let mut patterns = Vec::with_capacity(arr.len());
    for pattern in arr {
        let Some(text) = pattern.as_str() else {
            return Err(HostPatternInputError::InvalidPatternValue {
                name: display_name.to_string(),
            });
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        patterns.push(trimmed.to_ascii_lowercase());
    }
    if patterns.is_empty() {
        return Err(HostPatternInputError::EmptyPatternList {
            name: display_name.to_string(),
        });
    }
    Ok(patterns)
}

fn insert_host_patterns(
    target: &mut HashMap<String, Vec<String>>,
    name: String,
    patterns: Vec<String>,
) -> Result<(), HostPatternInputError> {
    if target.insert(name.clone(), patterns).is_some() {
        return Err(HostPatternInputError::DuplicateEntry { name });
    }
    Ok(())
}

const ROUTE_HEADER_ORDER: &[&str] = &[
    "Sora-Name",
    "Sora-Content-CID",
    "Sora-Proof",
    "Sora-Proof-Status",
    "Sora-Route-Binding",
    "Content-Security-Policy",
    "Strict-Transport-Security",
    "Permissions-Policy",
];

fn format_headers_template(headers: &BTreeMap<String, String>) -> String {
    let mut lines = Vec::new();
    for key in ROUTE_HEADER_ORDER {
        if let Some(value) = headers.get(*key) {
            lines.push(format!("{key}: {value}"));
        }
    }
    for (key, value) in headers {
        if ROUTE_HEADER_ORDER.contains(&key.as_str()) {
            continue;
        }
        lines.push(format!("{key}: {value}"));
    }
    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

fn headers_to_value(headers: &BTreeMap<String, String>) -> serde_json::Map<String, SerdeJsonValue> {
    let mut map = serde_json::Map::new();
    for (key, value) in headers {
        map.insert(key.clone(), SerdeJsonValue::String(value.clone()));
    }
    map
}

fn verify_host_map(
    summaries: &[HostSummary],
    mut entries: HashMap<String, Vec<String>>,
) -> Result<(), HostPatternInputError> {
    for summary in summaries {
        let Some(patterns) = entries.remove(summary.normalized_name.as_str()) else {
            return Err(HostPatternInputError::EntryMissingForName {
                name: summary.name.clone(),
            });
        };
        let missing = missing_required_patterns(summary, &patterns);
        if !missing.is_empty() {
            return Err(HostPatternInputError::MissingRequiredPatterns {
                name: summary.name.clone(),
                missing,
            });
        }
        println!(
            "{}: verified GAR host_patterns ({}) canonical_label={} canonical_host={}",
            summary.name,
            patterns.len(),
            summary.canonical_label,
            summary.canonical_host
        );
    }
    if !entries.is_empty() {
        let mut leftover = entries.keys().cloned().collect::<Vec<_>>();
        leftover.sort();
        return Err(HostPatternInputError::UnknownNames { names: leftover });
    }
    Ok(())
}

fn missing_required_patterns(summary: &HostSummary, patterns: &[String]) -> Vec<String> {
    let mut normalised_patterns = HashSet::new();
    for pattern in patterns {
        normalised_patterns.insert(pattern.trim().to_ascii_lowercase());
    }
    let required = [
        summary.canonical_host.to_ascii_lowercase(),
        summary.canonical_wildcard.to_string(),
        summary.pretty_host.to_ascii_lowercase(),
    ];
    required
        .into_iter()
        .filter(|pattern| !normalised_patterns.contains(pattern))
        .collect()
}

impl HostSummary {
    fn from_bindings(name: &str, bindings: &GatewayHostBindings) -> Self {
        Self {
            name: name.to_string(),
            normalized_name: bindings.normalized_name().to_string(),
            canonical_label: bindings.canonical_label().to_string(),
            canonical_host: bindings.canonical_host().to_string(),
            pretty_host: bindings.pretty_host().to_string(),
            canonical_wildcard: GatewayHostBindings::canonical_wildcard(),
        }
    }
}

/// Command-line options for `cargo xtask soradns-directory-release`.
#[derive(Debug, Clone)]
pub struct DirectoryReleaseOptions {
    pub rad_dir: PathBuf,
    pub output_root: PathBuf,
    pub release_key_path: PathBuf,
    pub car_cid: String,
    pub prev_id_hex: Option<String>,
    pub created_at: Option<OffsetDateTime>,
    pub note: Option<String>,
}

/// Run the directory release generator.
pub fn release_directory(options: DirectoryReleaseOptions) -> Result<(), Box<dyn Error>> {
    let rad_dir = normalize_path(&options.rad_dir)?;
    if !rad_dir.is_dir() {
        return Err(format!(
            "RAD directory `{}` does not exist or is not a directory",
            rad_dir.display()
        )
        .into());
    }

    let created_at = options.created_at.unwrap_or_else(OffsetDateTime::now_utc);
    let created_at_ms = (created_at.unix_timestamp_nanos() / 1_000_000) as u64;
    let created_at_rfc3339 = created_at.format(&Rfc3339)?;

    let mut rad_entries =
        load_rad_entries(&rad_dir).map_err(|err| -> Box<dyn Error> { Box::new(err) })?;
    if rad_entries.is_empty() {
        return Err("no `.norito` files were found in the supplied --rad-dir".into());
    }
    rad_entries.sort_by(|a, b| a.resolver_id_hex.cmp(&b.resolver_id_hex));

    let rad_count = rad_entries.len();
    let leaves: Vec<[u8; 32]> = rad_entries.iter().map(|entry| entry.leaf_hash).collect();
    let root_hash = compute_merkle_root(&leaves)?;
    let root_hex = hex_encode(root_hash);

    let directory_json = DirectoryJson {
        version: 1,
        created_at_ms,
        rad_count,
        merkle_root: root_hex.clone(),
        previous_root: options
            .prev_id_hex
            .as_deref()
            .map(|value| value.to_ascii_lowercase()),
        rad: rad_entries
            .iter()
            .map(|entry| DirectoryRadJsonEntry {
                resolver_id: entry.resolver_id_hex.clone(),
                rad_sha256: entry.rad_digest_hex.clone(),
                leaf_hash: entry.leaf_hash_hex.clone(),
                file: format!("rad/{}.norito", entry.resolver_id_hex),
            })
            .collect(),
    };
    let directory_value =
        serde_json::to_value(&directory_json).map_err(|err| -> Box<dyn Error> { Box::new(err) })?;
    let directory_json_bytes = canonical_json_bytes(&directory_value)
        .map_err(|err| -> Box<dyn Error> { Box::new(err) })?;
    let directory_json_sha256 = sha256_bytes(&directory_json_bytes);
    let directory_json_sha256_hex = hex_encode(directory_json_sha256);

    let output_root = normalize_path(&options.output_root)?;
    fs::create_dir_all(&output_root)?;
    let dir_name = release_dir_name(created_at);
    let release_dir = output_root.join(dir_name);
    fs::create_dir_all(&release_dir)?;

    let directory_json_path = release_dir.join("directory.json");
    fs::write(&directory_json_path, &directory_json_bytes)?;

    let rad_dest_dir = release_dir.join("rad");
    fs::create_dir_all(&rad_dest_dir)?;
    for entry in &rad_entries {
        let dest = rad_dest_dir.join(format!("{}.norito", entry.resolver_id_hex));
        fs::copy(&entry.source_path, dest)?;
    }

    let proof_manifest_cid = build_ipfs_path(&options.car_cid)?;
    let prev_root = match options.prev_id_hex.as_deref() {
        Some(hex) => Some(parse_hex_hash(hex)?),
        None => None,
    };
    let release_key_path = normalize_path(&options.release_key_path)?;
    let builder_keypair = load_release_key(&release_key_path)?;
    let builder_public_key = builder_keypair.public_key().clone();

    let signing_payload = SigningPayload::new(
        1,
        created_at_ms,
        rad_count as u32,
        &root_hash,
        &directory_json_sha256,
        prev_root.as_ref(),
        proof_manifest_cid.as_ref(),
        &builder_public_key,
    );
    let signing_bytes = serde_json::to_vec(&signing_payload)?;
    let builder_signature = Signature::new(builder_keypair.private_key(), &signing_bytes);

    let record = ResolverDirectoryRecordV1 {
        root_hash,
        record_version: 1,
        created_at_ms,
        rad_count: rad_count as u32,
        directory_json_sha256,
        previous_root: prev_root,
        published_at_block: 0,
        published_at_unix: created_at.unix_timestamp() as u64,
        proof_manifest_cid,
        builder_public_key,
        builder_signature,
    };

    let record_json_path = release_dir.join("record.json");
    let mut record_json = norito::json::to_vec(&record)?;
    record_json.push(b'\n');
    fs::write(&record_json_path, &record_json)?;

    let record_norito_path = release_dir.join("record.to");
    fs::write(&record_norito_path, to_bytes(&record)?)?;

    let metadata = ReleaseMetadata {
        directory_id_hex: root_hex.clone(),
        rad_count,
        directory_json_sha256_hex: directory_json_sha256_hex.clone(),
        created_at: created_at_rfc3339,
        car_cid: options.car_cid.clone(),
        output_dir: release_dir.display().to_string(),
        files: DirectoryFilesMetadata {
            directory_json: relative_path(&directory_json_path, &release_dir),
            record_json: relative_path(&record_json_path, &release_dir),
            record_norito: relative_path(&record_norito_path, &release_dir),
        },
        rad_entries: rad_entries
            .iter()
            .map(|entry| RadEntryMetadata {
                resolver_id: entry.resolver_id_hex.clone(),
                rad_sha256_hex: entry.rad_digest_hex.clone(),
                leaf_hash_hex: entry.leaf_hash_hex.clone(),
                file: format!("rad/{}.norito", entry.resolver_id_hex),
            })
            .collect(),
        note: options.note.clone(),
    };
    let metadata_path = release_dir.join("metadata.json");
    fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;

    println!("SoraDNS directory release bundle created");
    println!("  Output directory : {}", release_dir.display());
    println!("  Directory ID     : {root_hex}");
    println!("  RAD entries      : {rad_count}");
    println!("  directory.json   : {}", directory_json_path.display());
    println!("  Record (JSON)    : {}", record_json_path.display());
    println!("  Record (Norito)  : {}", record_norito_path.display());
    println!("  metadata.json    : {}", metadata_path.display());

    Ok(())
}

#[derive(Debug)]
struct RadEntry {
    resolver_id_hex: String,
    rad_digest_hex: String,
    leaf_hash_hex: String,
    leaf_hash: [u8; 32],
    source_path: PathBuf,
}

fn load_rad_entries(rad_dir: &Path) -> Result<Vec<RadEntry>, DirectoryReleaseError> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in fs::read_dir(rad_dir).map_err(|err| DirectoryReleaseError::Io {
        path: rad_dir.to_path_buf(),
        source: err,
    })? {
        let entry = entry.map_err(|err| DirectoryReleaseError::Io {
            path: rad_dir.to_path_buf(),
            source: err,
        })?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            if !matches!(ext, "norito" | "to") {
                continue;
            }
        } else {
            continue;
        }
        let bytes = fs::read(&path).map_err(|err| DirectoryReleaseError::Io {
            path: path.clone(),
            source: err,
        })?;
        let rad: ResolverAttestationDocumentV1 =
            decode_from_bytes(&bytes).map_err(|err| DirectoryReleaseError::Decode {
                path: path.clone(),
                source: err,
            })?;
        if rad.version != RAD_VERSION_V1 {
            return Err(DirectoryReleaseError::UnsupportedVersion {
                path,
                version: rad.version,
            });
        }
        let resolver_id_hex = hex_encode(rad.resolver_id);
        if !seen.insert(resolver_id_hex.clone()) {
            return Err(DirectoryReleaseError::DuplicateResolver {
                resolver_id: resolver_id_hex,
            });
        }
        let rad_value = json::to_value(&rad)?;
        let serde_value = norito_value_to_serde(&rad_value);
        let canonical_json = canonical_json_bytes(&serde_value)?;
        let rad_digest = hash_rad(&canonical_json);
        let leaf_hash = hash_leaf(&rad_digest);

        entries.push(RadEntry {
            resolver_id_hex,
            rad_digest_hex: hex_encode(rad_digest),
            leaf_hash_hex: hex_encode(leaf_hash),
            leaf_hash,
            source_path: path,
        });
    }
    Ok(entries)
}

fn hash_rad(canonical_json: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RAD_HASH_DOMAIN);
    hasher.update(canonical_json);
    hasher.finalize().into()
}

fn hash_leaf(rad_digest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(rad_digest);
    hasher.finalize().into()
}

fn compute_merkle_root(leaves: &[[u8; 32]]) -> Result<[u8; 32], DirectoryReleaseError> {
    if leaves.is_empty() {
        return Err(DirectoryReleaseError::EmptyRadSet);
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for chunk in level.chunks(2) {
            let combined = match chunk {
                [left, right] => hash_branch(left, right),
                [single] => hash_branch(single, single),
                _ => unreachable!(),
            };
            next.push(combined);
        }
        level = next;
    }
    Ok(level[0])
}

fn hash_branch(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn canonical_json_bytes(value: &SerdeJsonValue) -> Result<Vec<u8>, DirectoryReleaseError> {
    let serde_value = canonicalize_value(value);
    let mut bytes = Vec::new();
    serde_json::to_writer(&mut bytes, &serde_value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonicalize_value(value: &SerdeJsonValue) -> SerdeJsonValue {
    match value {
        SerdeJsonValue::Null => SerdeJsonValue::Null,
        SerdeJsonValue::Bool(b) => SerdeJsonValue::Bool(*b),
        SerdeJsonValue::Number(num) => SerdeJsonValue::Number(num.clone()),
        SerdeJsonValue::String(s) => SerdeJsonValue::String(s.clone()),
        SerdeJsonValue::Array(items) => SerdeJsonValue::Array(
            items
                .iter()
                .map(canonicalize_value)
                .collect::<Vec<SerdeJsonValue>>(),
        ),
        SerdeJsonValue::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut obj = serde_json::Map::new();
            for (key, value) in entries {
                obj.insert(key.clone(), canonicalize_value(value));
            }
            SerdeJsonValue::Object(obj)
        }
    }
}

fn norito_value_to_serde(value: &NoritoJsonValue) -> SerdeJsonValue {
    match value {
        NoritoJsonValue::Null => SerdeJsonValue::Null,
        NoritoJsonValue::Bool(b) => SerdeJsonValue::Bool(*b),
        NoritoJsonValue::Number(num) => {
            let parsed = match num {
                NoritoNumber::I64(v) => serde_json::Number::from(*v),
                NoritoNumber::U64(v) => serde_json::Number::from(*v),
                NoritoNumber::F64(v) => serde_json::Number::from_f64(*v)
                    .expect("Norito emitted NaN/inf, which JSON forbids"),
            };
            SerdeJsonValue::Number(parsed)
        }
        NoritoJsonValue::String(s) => SerdeJsonValue::String(s.clone()),
        NoritoJsonValue::Array(items) => SerdeJsonValue::Array(
            items
                .iter()
                .map(norito_value_to_serde)
                .collect::<Vec<SerdeJsonValue>>(),
        ),
        NoritoJsonValue::Object(map) => {
            let mut obj = serde_json::Map::new();
            for (key, value) in map.iter() {
                obj.insert(key.clone(), norito_value_to_serde(value));
            }
            SerdeJsonValue::Object(obj)
        }
    }
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn build_ipfs_path(cid: &str) -> Result<IpfsPath, Box<dyn Error>> {
    if cid.trim().is_empty() {
        return Err("car CID must not be empty".into());
    }
    let normalized = if cid.starts_with('/') {
        cid.to_string()
    } else {
        format!("/ipfs/{cid}")
    };
    normalized
        .parse::<IpfsPath>()
        .map_err(|err| format!("invalid --car-cid `{cid}`: {err}").into())
}

fn load_release_key(path: &Path) -> Result<KeyPair, Box<dyn Error>> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read release key from `{}`: {err}",
            path.display()
        )
    })?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(format!("release key file `{}` is empty", path.display()).into());
    }
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, trimmed)
        .map_err(|err| format!("failed to parse release key `{}`: {err}", path.display()))?;
    KeyPair::from_private_key(private_key)
        .map_err(|err| format!("invalid release key `{}`: {err}", path.display()).into())
}

fn parse_hex_hash(hex: &str) -> Result<[u8; 32], Box<dyn Error>> {
    let stripped = hex.trim_start_matches("0x");
    let bytes = hex::decode(stripped).map_err(|err| format!("invalid hex value `{hex}`: {err}"))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32-byte hex value, found {} bytes", bytes.len()).into());
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn release_dir_name(timestamp: OffsetDateTime) -> String {
    let raw = timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".into());
    let mut sanitized = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch);
        }
    }
    if sanitized.is_empty() {
        sanitized.push_str("soradnsdirectory");
    }
    format!("{}_soradns_directory", sanitized)
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[derive(Debug, Serialize)]
struct DirectoryJson {
    version: u32,
    created_at_ms: u64,
    rad_count: usize,
    merkle_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_root: Option<String>,
    rad: Vec<DirectoryRadJsonEntry>,
}

#[derive(Debug, Serialize)]
struct DirectoryRadJsonEntry {
    resolver_id: String,
    rad_sha256: String,
    leaf_hash: String,
    file: String,
}

#[derive(Debug, Serialize)]
struct ReleaseMetadata {
    directory_id_hex: String,
    rad_count: usize,
    directory_json_sha256_hex: String,
    created_at: String,
    car_cid: String,
    output_dir: String,
    files: DirectoryFilesMetadata,
    rad_entries: Vec<RadEntryMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct DirectoryFilesMetadata {
    directory_json: String,
    record_json: String,
    record_norito: String,
}

#[derive(Debug, Serialize)]
struct RadEntryMetadata {
    resolver_id: String,
    rad_sha256_hex: String,
    leaf_hash_hex: String,
    file: String,
}

#[derive(Debug, Serialize)]
struct SigningPayload {
    record_version: u16,
    created_at_ms: u64,
    rad_count: u32,
    root_hash_hex: String,
    directory_json_sha256_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_root_hex: Option<String>,
    proof_manifest_cid: String,
    builder_public_key_hex: String,
}

impl SigningPayload {
    #[allow(clippy::too_many_arguments)]
    fn new(
        record_version: u16,
        created_at_ms: u64,
        rad_count: u32,
        root_hash: &[u8; 32],
        directory_json_sha256: &[u8; 32],
        previous_root: Option<&[u8; 32]>,
        proof_manifest_cid: &str,
        builder_public_key: &iroha_crypto::PublicKey,
    ) -> Self {
        let (_, pk_bytes) = builder_public_key.to_bytes();
        SigningPayload {
            record_version,
            created_at_ms,
            rad_count,
            root_hash_hex: hex_encode(root_hash),
            directory_json_sha256_hex: hex_encode(directory_json_sha256),
            previous_root_hex: previous_root.map(hex_encode),
            proof_manifest_cid: proof_manifest_cid.to_string(),
            builder_public_key_hex: hex_encode(pk_bytes),
        }
    }
}

#[derive(Debug, Error)]
enum DirectoryReleaseError {
    #[error("failed to read `{path}`: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to decode `{path}`: {source}")]
    Decode {
        path: PathBuf,
        source: norito::Error,
    },
    #[error("RAD version {version} in `{path}` is not supported")]
    UnsupportedVersion { path: PathBuf, version: u8 },
    #[error("resolver `{resolver_id}` appears multiple times in the RAD set")]
    DuplicateResolver { resolver_id: String },
    #[error("at least one RAD document is required to build a directory")]
    EmptyRadSet,
    #[error("failed to encode JSON: {source}")]
    JsonEncode { source: serde_json::Error },
    #[error("failed to serialize Norito JSON: {source}")]
    NoritoJson { source: norito::json::Error },
}

impl From<serde_json::Error> for DirectoryReleaseError {
    fn from(source: serde_json::Error) -> Self {
        Self::JsonEncode { source }
    }
}

impl From<norito::json::Error> for DirectoryReleaseError {
    fn from(source: norito::json::Error) -> Self {
        Self::NoritoJson { source }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use blake3::hash as blake3_hash;
    use iroha_primitives::soradns::{
        canonical_gateway_suffix, canonical_gateway_wildcard_pattern, pretty_gateway_suffix,
    };
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn derives_host_summaries_with_normalisation() {
        let names = vec!["example.sora".to_string(), "Example.Dao.Sora".to_string()];
        let summaries = derive_host_summaries(&names).expect("derivation succeeds");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].name, "example.sora");
        assert_eq!(summaries[0].normalized_name, "example.sora");
        assert!(summaries[0].canonical_host.ends_with(".gw.sora.id"));
        assert_eq!(
            summaries[0].canonical_wildcard,
            GatewayHostBindings::canonical_wildcard()
        );

        assert_eq!(summaries[1].name, "Example.Dao.Sora");
        assert_eq!(summaries[1].normalized_name, "example.dao.sora");
        assert!(summaries[1].pretty_host.ends_with(".gw.sora.name"));
    }

    #[test]
    fn host_summary_matches_blake3_derivation() {
        let summaries =
            derive_host_summaries(&["Portal.Sora".to_string()]).expect("derivation succeeds");
        let summary = summaries.first().expect("summary present");
        let digest = blake3_hash(summary.normalized_name.as_bytes());
        let expected_label = encode_base32_lower(digest.as_bytes());
        let expected_host = format!("{expected_label}.{}", canonical_gateway_suffix());
        assert_eq!(summary.canonical_label, expected_label);
        assert_eq!(summary.canonical_host, expected_host);
        assert_eq!(
            summary.pretty_host,
            format!("{}.{}", summary.normalized_name, pretty_gateway_suffix())
        );
        assert_eq!(
            summary.canonical_wildcard,
            canonical_gateway_wildcard_pattern()
        );
    }

    #[test]
    fn host_summary_error_surfaces_context() {
        let names = vec!["invalid..name".to_string()];
        let error = derive_host_summaries(&names).expect_err("derivation should fail");
        match error {
            HostSummaryError::Derive { name, source } => {
                assert_eq!(name, "invalid..name");
                assert!(matches!(source, GatewayHostError::ConsecutiveDots));
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn verify_host_patterns_accepts_mapped_records() {
        let names = vec!["docs.sora".to_string()];
        let summaries = derive_host_summaries(&names).expect("derivation succeeds");
        let summary = &summaries[0];

        let temp = tempdir().expect("tempdir");
        let file = temp.path().join("gar.json");
        let payload = serde_json::json!({
            "docs.sora": [
                summary.canonical_host,
                summary.canonical_wildcard,
                summary.pretty_host
            ]
        });
        fs::write(&file, serde_json::to_vec(&payload).unwrap()).expect("write file");

        verify_host_patterns(&summaries, &[file]).expect("verification succeeds");
    }

    #[test]
    fn verify_host_patterns_rejects_missing_entries() {
        let names = vec!["docs.sora".to_string()];
        let summaries = derive_host_summaries(&names).expect("derivation succeeds");
        let summary = &summaries[0];
        let temp = tempdir().expect("tempdir");
        let file = temp.path().join("gar.json");
        let payload = serde_json::json!([
            {
                "name": "docs.sora",
                "host_patterns": [
                    summary.canonical_host,
                    summary.canonical_wildcard
                ]
            }
        ]);
        fs::write(&file, serde_json::to_vec(&payload).unwrap()).expect("write file");

        let error =
            verify_host_patterns(&summaries, &[file]).expect_err("verification should fail");
        assert!(matches!(
            error,
            HostPatternInputError::MissingRequiredPatterns { .. }
        ));
    }

    #[test]
    fn detect_canonical_derivation_mismatch() {
        let mut summaries = derive_host_summaries(&["docs.sora".to_string()])
            .expect("derivation succeeds with canonical bindings");
        let mut summary = summaries.swap_remove(0);
        let expected_label = summary.canonical_label.clone();
        summary.canonical_label = "invalidlabel".to_string();
        let err = super::validate_canonical_derivation(&summary).expect_err("should reject label");
        match err {
            HostSummaryError::CanonicalDerivationMismatch {
                expected_label: got_expected,
                found_label,
                ..
            } => {
                assert_eq!(got_expected, expected_label);
                assert_eq!(found_label, "invalidlabel");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn binding_template_emits_payload_and_headers() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        let manifest = serde_json::json!({
            "root_cid": [1, 2, 3, 4],
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("write manifest");

        let options = BindingTemplateOptions {
            manifest_path: manifest_path.clone(),
            alias: "docs.sora".to_string(),
            hostname: "docs.sora.link".to_string(),
            route_label: Some("production".to_string()),
            proof_status: Some("ok".to_string()),
            csp_template: None,
            permissions_template: None,
            hsts_template: None,
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let render = build_binding_template(&options).expect("template");
        let payload = render
            .payload
            .as_object()
            .expect("payload should be JSON object");
        assert_eq!(
            payload.get("alias").and_then(SerdeJsonValue::as_str),
            Some("docs.sora")
        );
        assert_eq!(
            payload.get("hostname").and_then(SerdeJsonValue::as_str),
            Some("docs.sora.link")
        );
        assert!(render.headers_template.contains("Sora-Name: docs.sora"));
        assert!(
            render
                .headers_template
                .contains("Sora-Route-Binding: host=docs.sora.link;")
        );
    }

    #[test]
    fn binding_template_populates_default_headers() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::json!({ "root_cid": [0_u8, 1, 2, 3] }).to_string(),
        )
        .expect("write manifest");

        let generated_at = OffsetDateTime::UNIX_EPOCH;
        let hostname = "portal.gw.sora.name";
        let options = BindingTemplateOptions {
            manifest_path: manifest_path.clone(),
            alias: "portal.sora".to_string(),
            hostname: hostname.to_string(),
            route_label: Some("preview".to_string()),
            proof_status: None,
            csp_template: None,
            permissions_template: None,
            hsts_template: None,
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            generated_at,
        };

        let render = build_binding_template(&options).expect("template render");
        let payload = render.payload.as_object().expect("json payload");
        let headers = payload["headers"].as_object().expect("headers map").clone();
        let cid_suffix = encode_base32_lower(&[0, 1, 2, 3]);
        let expected_cid = format!("b{cid_suffix}");
        let timestamp = generated_at.format(&Rfc3339).expect("timestamp formatting");
        assert_eq!(headers["Sora-Name"], "portal.sora");
        assert_eq!(headers["Sora-Content-CID"], expected_cid);
        assert_eq!(headers["Sora-Proof-Status"], "ok");
        assert_eq!(
            headers["Sora-Route-Binding"],
            format!("host={hostname};cid={expected_cid};generated_at={timestamp};label=preview")
        );
        assert_eq!(headers["Content-Security-Policy"], DEFAULT_CSP_TEMPLATE);
        assert_eq!(headers["Strict-Transport-Security"], DEFAULT_HSTS_TEMPLATE);
        assert_eq!(headers["Permissions-Policy"], DEFAULT_PERMISSIONS_POLICY);
        assert!(
            render
                .headers_template
                .starts_with("Sora-Name: portal.sora")
        );
        assert!(
            render
                .headers_template
                .contains("Sora-Route-Binding: host=portal.gw.sora.name;cid=")
        );
    }

    #[test]
    fn gar_template_normalizes_labels_and_digest() {
        let manifest_digest = "ABCDEF12".repeat(8);
        let options = GarTemplateOptions {
            name: "Docs.Sora".to_string(),
            manifest_cid: "bafybeigdyrzt2vx7demoexamplecid".to_string(),
            manifest_digest: Some(manifest_digest.clone()),
            csp_template: None,
            hsts_template: None,
            permissions_template: None,
            valid_from_epoch: 1_735_771_200,
            valid_until_epoch: Some(1_735_771_800),
            telemetry_labels: vec![
                "  ops  ".to_string(),
                "".to_string(),
                "DG-3".to_string(),
                "ops".to_string(),
            ],
        };
        let payload = build_gar_template(&options).expect("gar payload");
        let labels = payload
            .get("telemetry_labels")
            .and_then(SerdeJsonValue::as_array)
            .expect("labels array");
        let label_values: Vec<&str> = labels
            .iter()
            .map(|value| value.as_str().expect("string label"))
            .collect();
        let digest_lower = manifest_digest.to_ascii_lowercase();
        assert_eq!(label_values, vec!["DG-3", "ops"], "labels trimmed/deduped");
        assert_eq!(
            payload
                .get("manifest_digest")
                .and_then(SerdeJsonValue::as_str),
            Some(digest_lower.as_str()),
            "manifest digest should be normalised to lowercase"
        );
    }

    #[test]
    fn binding_template_rejects_empty_alias() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        let manifest = serde_json::json!({
            "root_cid": [1, 2, 3],
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("write manifest");

        let options = BindingTemplateOptions {
            manifest_path,
            alias: " ".to_string(),
            hostname: "docs.sora.link".to_string(),
            route_label: None,
            proof_status: None,
            csp_template: None,
            permissions_template: None,
            hsts_template: None,
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let err = build_binding_template(&options).expect_err("alias missing");
        match err {
            BindingTemplateError::MissingAlias => {}
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn binding_template_custom_headers_replace_defaults() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        let manifest = serde_json::json!({
            "root_cid": [5, 6, 7, 8],
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("write manifest");

        let options = BindingTemplateOptions {
            manifest_path,
            alias: "docs.sora".to_string(),
            hostname: "docs.sora.link".to_string(),
            route_label: None,
            proof_status: None,
            csp_template: Some("default-src 'self' sorafs://;".to_string()),
            permissions_template: Some("geolocation=(self)".to_string()),
            hsts_template: Some("max-age=123; preload".to_string()),
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let render = build_binding_template(&options).expect("template");
        let headers = render.payload["headers"].as_object().expect("headers map");
        assert_eq!(
            headers
                .get("Content-Security-Policy")
                .and_then(SerdeJsonValue::as_str),
            Some("default-src 'self' sorafs://;")
        );
        assert_eq!(
            headers
                .get("Permissions-Policy")
                .and_then(SerdeJsonValue::as_str),
            Some("geolocation=(self)")
        );
        assert_eq!(
            headers
                .get("Strict-Transport-Security")
                .and_then(SerdeJsonValue::as_str),
            Some("max-age=123; preload")
        );
    }

    #[test]
    fn verify_gateway_binding_reports_canonical_label() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        let manifest = serde_json::json!({
            "root_cid": [9, 10, 11, 12],
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("write manifest");
        let options = BindingTemplateOptions {
            manifest_path,
            alias: "docs.sora".to_string(),
            hostname: "docs.sora.link".to_string(),
            route_label: None,
            proof_status: None,
            csp_template: None,
            permissions_template: None,
            hsts_template: None,
            include_csp: true,
            include_permissions: true,
            include_hsts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let render = build_binding_template(&options).expect("template");
        let binding_path = temp.path().join("binding.json");
        fs::write(&binding_path, serde_json::to_vec(&render.payload).unwrap())
            .expect("write binding");
        let summary = verify_gateway_binding(&binding_path, &GatewayBindingExpectations::default())
            .expect("binding verification succeeds");
        let bindings = derive_gateway_hosts("docs.sora").expect("bindings derived");
        assert_eq!(summary.canonical_label, bindings.canonical_label());
        assert_eq!(summary.canonical_host, bindings.canonical_host());
    }

    #[test]
    fn binding_template_rejects_disabled_header_override() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        let manifest = serde_json::json!({
            "root_cid": [9, 10, 11],
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("write manifest");

        let options = BindingTemplateOptions {
            manifest_path,
            alias: "docs.sora".to_string(),
            hostname: "docs.sora.link".to_string(),
            route_label: None,
            proof_status: None,
            csp_template: Some("default-src 'none'".to_string()),
            permissions_template: None,
            hsts_template: None,
            include_csp: false,
            include_permissions: true,
            include_hsts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let err = build_binding_template(&options).expect_err("override should fail");
        match err {
            BindingTemplateError::HeaderOverrideDisabled { header } => {
                assert_eq!(header, "Content-Security-Policy");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn gar_template_includes_defaults() {
        let options = GarTemplateOptions {
            name: "Docs.Sora".to_string(),
            manifest_cid: "bafybeigdyrzt".to_string(),
            manifest_digest: Some(
                "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789".to_string(),
            ),
            csp_template: None,
            hsts_template: None,
            permissions_template: None,
            valid_from_epoch: 1_700_000_000,
            valid_until_epoch: Some(1_800_000_000),
            telemetry_labels: vec!["dg-3".to_string(), " rollout ".to_string()],
        };
        let template = build_gar_template(&options).expect("template");
        let obj = template
            .as_object()
            .expect("gar template payload should be an object");
        assert_eq!(
            obj.get("name").and_then(SerdeJsonValue::as_str),
            Some("docs.sora")
        );
        assert_eq!(
            obj.get("manifest_cid").and_then(SerdeJsonValue::as_str),
            Some("bafybeigdyrzt")
        );
        assert_eq!(
            obj.get("manifest_digest").and_then(SerdeJsonValue::as_str),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
        let hosts = obj
            .get("host_patterns")
            .and_then(SerdeJsonValue::as_array)
            .expect("host patterns");
        assert_eq!(hosts.len(), 3);
        assert!(hosts.iter().any(|value| value == "docs.sora.gw.sora.name"));
        assert!(hosts.iter().any(|value| value == "*.gw.sora.id"));
        assert_eq!(
            obj.get("csp_template").and_then(SerdeJsonValue::as_str),
            Some(DEFAULT_CSP_TEMPLATE)
        );
        assert_eq!(
            obj.get("hsts_template").and_then(SerdeJsonValue::as_str),
            Some(DEFAULT_HSTS_TEMPLATE)
        );
        assert_eq!(
            obj.get("permissions_template")
                .and_then(SerdeJsonValue::as_str),
            Some(DEFAULT_PERMISSIONS_POLICY)
        );
        let labels = obj
            .get("telemetry_labels")
            .and_then(SerdeJsonValue::as_array)
            .expect("labels");
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().any(|label| label == "dg-3"));
    }

    #[test]
    fn gar_template_respects_overrides_and_patterns() {
        let options = GarTemplateOptions {
            name: "Portal.Sora".to_string(),
            manifest_cid: "bafyportalmanifest".to_string(),
            manifest_digest: Some("FF".repeat(32)),
            csp_template: Some("default-src 'self' sorafs://".to_string()),
            hsts_template: Some("max-age=42; preload".to_string()),
            permissions_template: Some("microphone=()".to_string()),
            valid_from_epoch: 1_700_000_000,
            valid_until_epoch: Some(1_700_003_600),
            telemetry_labels: vec!["alpha".to_string(), "beta".to_string(), "alpha".to_string()],
        };
        let template = build_gar_template(&options).expect("template");
        let obj = template
            .as_object()
            .expect("gar template payload should be an object");
        let bindings = derive_gateway_hosts("portal.sora").expect("bindings derived");
        let actual_patterns = obj["host_patterns"]
            .as_array()
            .expect("host pattern array")
            .iter()
            .map(|value| value.as_str().expect("host pattern").to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected_patterns = bindings
            .host_patterns()
            .iter()
            .map(|pattern| pattern.to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual_patterns, expected_patterns, "host patterns mismatch");
        assert_eq!(
            obj.get("csp_template").and_then(SerdeJsonValue::as_str),
            Some("default-src 'self' sorafs://")
        );
        assert_eq!(
            obj.get("hsts_template").and_then(SerdeJsonValue::as_str),
            Some("max-age=42; preload")
        );
        assert_eq!(
            obj.get("permissions_template")
                .and_then(SerdeJsonValue::as_str),
            Some("microphone=()")
        );
        let labels = obj
            .get("telemetry_labels")
            .and_then(SerdeJsonValue::as_array)
            .expect("labels");
        let label_values: Vec<&str> = labels
            .iter()
            .map(|label| label.as_str().expect("label string"))
            .collect();
        assert_eq!(label_values, vec!["alpha", "beta"]);
        let digest_lower = "ff".repeat(32);
        assert_eq!(
            obj.get("manifest_digest").and_then(SerdeJsonValue::as_str),
            Some(digest_lower.as_str())
        );
    }

    #[test]
    fn gar_template_rejects_invalid_digest() {
        let options = GarTemplateOptions {
            name: "docs.sora".to_string(),
            manifest_cid: "bafybeigdyrzt".to_string(),
            manifest_digest: Some("bad-digest".to_string()),
            csp_template: None,
            hsts_template: None,
            permissions_template: None,
            valid_from_epoch: 0,
            valid_until_epoch: None,
            telemetry_labels: Vec::new(),
        };
        let err = build_gar_template(&options).expect_err("should fail");
        matches!(err, GarTemplateError::InvalidManifestDigest);
    }

    #[test]
    fn verify_gar_payload_accepts_valid_manifest() {
        let manifest_cid = "bafybeigdyrzt2vx7demoexamplecid";
        let manifest_digest = "8A2A332D5E52EDC13ED088B79B6B2940AF0B31C7F7FBB9324A88ACDF1A0AF07D";
        let bindings = derive_gateway_hosts("docs.sora").expect("bindings derived for docs.sora");
        let host_patterns = bindings
            .host_patterns()
            .iter()
            .map(|pattern| serde_json::Value::String((*pattern).to_string()))
            .collect::<Vec<_>>();
        let payload = serde_json::json!({
            "version": 2,
            "name": bindings.normalized_name(),
            "manifest_cid": manifest_cid,
            "manifest_digest": manifest_digest,
            "host_patterns": host_patterns,
            "csp_template": DEFAULT_CSP_TEMPLATE,
            "hsts_template": DEFAULT_HSTS_TEMPLATE,
            "valid_from_epoch": 1_735_771_200u64,
            "valid_until_epoch": 1_767_307_200u64,
            "license_sets": [],
            "moderation_directives": [],
            "metrics_policy": serde_json::Value::Null,
            "telemetry_labels": ["dg-3", "docs-portal"],
            "rpt_digest": serde_json::Value::Null,
        });
        let temp = tempdir().expect("tempdir");
        let gar_path = temp.path().join("gar.json");
        fs::write(&gar_path, serde_json::to_vec_pretty(&payload).unwrap()).expect("write gar");

        let summary = verify_gar_payload(
            &gar_path,
            &GarVerifyOptions {
                name: "docs.sora".to_string(),
                expected_manifest_cid: Some(manifest_cid.to_string()),
                expected_manifest_digest: Some(manifest_digest.to_string()),
                required_telemetry_labels: vec!["dg-3".to_string()],
            },
        )
        .expect("gar verification succeeds");
        assert_eq!(summary.version, 2);
        assert_eq!(summary.normalized_name, bindings.normalized_name());
        assert_eq!(summary.canonical_label, bindings.canonical_label());
        assert_eq!(summary.canonical_host, bindings.canonical_host());
        assert_eq!(summary.manifest_cid, manifest_cid);
        let digest_lower = manifest_digest.to_ascii_lowercase();
        assert_eq!(
            summary.manifest_digest.as_deref(),
            Some(digest_lower.as_str())
        );
        assert!(
            summary
                .host_patterns
                .iter()
                .any(|pattern| pattern == bindings.pretty_host())
        );
        assert_eq!(summary.telemetry_labels.len(), 2);
    }

    #[test]
    fn verify_gar_payload_rejects_missing_host_pattern() {
        let bindings = derive_gateway_hosts("docs.sora").expect("bindings derived for docs.sora");
        let host_patterns = serde_json::json!([
            bindings.canonical_host(),
            GatewayHostBindings::canonical_wildcard()
        ]);
        let payload = serde_json::json!({
            "version": 2,
            "name": bindings.normalized_name(),
            "manifest_cid": "bafybeigdyrzt2vx7demoexamplecid",
            "host_patterns": host_patterns,
            "csp_template": DEFAULT_CSP_TEMPLATE,
            "hsts_template": DEFAULT_HSTS_TEMPLATE,
            "valid_from_epoch": 1_735_771_200u64,
            "telemetry_labels": [],
        });
        let temp = tempdir().expect("tempdir");
        let gar_path = temp.path().join("gar.json");
        fs::write(&gar_path, serde_json::to_vec_pretty(&payload).unwrap()).expect("write gar");

        let error = verify_gar_payload(
            &gar_path,
            &GarVerifyOptions {
                name: "docs.sora".to_string(),
                ..GarVerifyOptions::default()
            },
        )
        .expect_err("verification should fail");
        assert!(matches!(error, GarVerifyError::MissingHostPatterns { .. }));
    }

    #[test]
    fn verify_gar_payload_detects_manifest_mismatch() {
        let bindings = derive_gateway_hosts("docs.sora").expect("bindings derived for docs.sora");
        let payload = serde_json::json!({
            "version": 2,
            "name": bindings.normalized_name(),
            "manifest_cid": "bafybeigdyrzt2vx7demoexamplecid",
            "host_patterns": bindings.host_patterns(),
            "csp_template": DEFAULT_CSP_TEMPLATE,
            "hsts_template": DEFAULT_HSTS_TEMPLATE,
            "valid_from_epoch": 1_735_771_200u64,
            "telemetry_labels": ["dg-3"]
        });
        let temp = tempdir().expect("tempdir");
        let gar_path = temp.path().join("gar.json");
        fs::write(&gar_path, serde_json::to_vec_pretty(&payload).unwrap()).expect("write gar");

        let error = verify_gar_payload(
            &gar_path,
            &GarVerifyOptions {
                name: "docs.sora".to_string(),
                expected_manifest_cid: Some("differentcid".to_string()),
                ..GarVerifyOptions::default()
            },
        )
        .expect_err("verification should fail");
        assert!(matches!(error, GarVerifyError::ManifestCidMismatch { .. }));
    }

    #[test]
    fn gateway_binding_verification_accepts_valid_payload() {
        let temp = tempdir().expect("tempdir");
        let binding_path = temp.path().join("binding.json");
        let manifest_path = temp.path().join("manifest.json");
        let manifest_root = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let expected_cid = format!("b{}", BASE32_NOPAD.encode(&manifest_root).to_lowercase());
        fs::write(
            &manifest_path,
            serde_json::json!({ "root_cid": manifest_root }).to_string(),
        )
        .expect("write manifest");
        let proof_payload = serde_json::json!({
            "alias": "docs.sora",
            "manifest": expected_cid
        });
        let proof_header = BASE64_STANDARD.encode(serde_json::to_vec(&proof_payload).unwrap());
        let payload = serde_json::json!({
            "alias": "docs.sora",
            "hostname": "docs.sora.link",
            "contentCid": expected_cid,
            "proofStatus": "ok",
            "generatedAt": "2026-01-02T03:04:05Z",
            "headers": {
                "Sora-Name": "docs.sora",
            "Sora-Content-CID": expected_cid,
            "Sora-Proof": proof_header,
            "Sora-Proof-Status": "ok",
            "Sora-Route-Binding": format!("host=docs.sora.link;cid={expected_cid};generated_at=2026-01-02T03:04:05Z"),
            "Content-Security-Policy": DEFAULT_CSP_TEMPLATE,
            "Permissions-Policy": DEFAULT_PERMISSIONS_POLICY,
            "Strict-Transport-Security": DEFAULT_HSTS_TEMPLATE,
        }
        });
        fs::write(&binding_path, serde_json::to_vec_pretty(&payload).unwrap())
            .expect("write binding");

        let summary = verify_gateway_binding(
            &binding_path,
            &GatewayBindingExpectations {
                alias: Some("docs.sora".to_string()),
                hostname: Some("docs.sora.link".to_string()),
                proof_status: Some("ok".to_string()),
                manifest_path: Some(manifest_path),
                content_cid: None,
            },
        )
        .expect("verification succeeds");
        assert_eq!(summary.alias.as_deref(), Some("docs.sora"));
        assert_eq!(summary.content_cid, expected_cid);
        assert_eq!(summary.hostname.as_deref(), Some("docs.sora.link"));
        assert_eq!(summary.proof_status.as_deref(), Some("ok"));
    }

    #[test]
    fn gateway_binding_verification_rejects_mismatched_proof() {
        let temp = tempdir().expect("tempdir");
        let binding_path = temp.path().join("binding.json");
        let proof_payload = serde_json::json!({
            "alias": "docs.sora",
            "manifest": "bafwrong"
        });
        let proof_header = BASE64_STANDARD.encode(serde_json::to_vec(&proof_payload).unwrap());
        let payload = serde_json::json!({
            "alias": "docs.sora",
            "hostname": "docs.sora.link",
            "contentCid": "bafgatewaycid",
            "proofStatus": "ok",
            "generatedAt": "2026-01-02T03:04:05Z",
            "headers": {
                "Sora-Name": "docs.sora",
            "Sora-Content-CID": "bafgatewaycid",
            "Sora-Proof": proof_header,
            "Sora-Proof-Status": "ok",
            "Sora-Route-Binding": "host=docs.sora.link;cid=bafgatewaycid;generated_at=2026-01-02T03:04:05Z",
            "Content-Security-Policy": DEFAULT_CSP_TEMPLATE,
            "Permissions-Policy": DEFAULT_PERMISSIONS_POLICY,
            "Strict-Transport-Security": DEFAULT_HSTS_TEMPLATE,
        }
        });
        fs::write(&binding_path, serde_json::to_vec_pretty(&payload).unwrap())
            .expect("write binding");

        let err = verify_gateway_binding(&binding_path, &GatewayBindingExpectations::default())
            .expect_err("verification should fail");
        matches!(err, GatewayBindingVerifyError::ProofManifestMismatch { .. });
    }

    #[test]
    fn gateway_binding_verification_rejects_manifest_mismatch() {
        let temp = tempdir().expect("tempdir");
        let binding_path = temp.path().join("binding.json");
        let manifest_path = temp.path().join("manifest.json");
        let manifest_root = vec![0x10, 0x20, 0x30];
        fs::write(
            &manifest_path,
            serde_json::json!({ "root_cid": manifest_root }).to_string(),
        )
        .expect("write manifest");
        let binding_root = vec![0xAA, 0xBB, 0xCC];
        let binding_cid = format!("b{}", BASE32_NOPAD.encode(&binding_root).to_lowercase());
        let proof_payload = serde_json::json!({
            "alias": "docs.sora",
            "manifest": binding_cid
        });
        let proof_header = BASE64_STANDARD.encode(serde_json::to_vec(&proof_payload).unwrap());
        let payload = serde_json::json!({
            "alias": "docs.sora",
            "hostname": "docs.sora.link",
            "contentCid": binding_cid,
            "proofStatus": "ok",
            "generatedAt": "2026-01-02T03:04:05Z",
            "headers": {
                "Sora-Name": "docs.sora",
            "Sora-Content-CID": binding_cid,
            "Sora-Proof": proof_header,
            "Sora-Proof-Status": "ok",
            "Sora-Route-Binding": format!("host=docs.sora.link;cid={binding_cid};generated_at=2026-01-02T03:04:05Z"),
            "Content-Security-Policy": DEFAULT_CSP_TEMPLATE,
            "Permissions-Policy": DEFAULT_PERMISSIONS_POLICY,
            "Strict-Transport-Security": DEFAULT_HSTS_TEMPLATE,
        }
        });
        fs::write(&binding_path, serde_json::to_vec_pretty(&payload).unwrap())
            .expect("write binding");

        let err = verify_gateway_binding(
            &binding_path,
            &GatewayBindingExpectations {
                alias: Some("docs.sora".to_string()),
                manifest_path: Some(manifest_path),
                hostname: Some("docs.sora.link".to_string()),
                proof_status: Some("ok".to_string()),
                content_cid: None,
            },
        )
        .expect_err("verification should fail");
        matches!(err, GatewayBindingVerifyError::ContentCidMismatch { .. });
    }

    #[test]
    fn gateway_binding_verification_requires_permissions_policy() {
        let temp = tempdir().expect("tempdir");
        let binding_path = temp.path().join("binding.json");
        let payload = serde_json::json!({
            "alias": "docs.sora",
            "hostname": "docs.sora.link",
            "contentCid": "bafgatewaycid",
            "proofStatus": "ok",
            "generatedAt": "2026-01-02T03:04:05Z",
            "headers": {
                "Sora-Name": "docs.sora",
                "Sora-Content-CID": "bafgatewaycid",
                "Sora-Proof": BASE64_STANDARD.encode(br#"{"alias":"docs.sora","manifest":"bafgatewaycid"}"#),
                "Sora-Proof-Status": "ok",
                "Sora-Route-Binding": "host=docs.sora.link;cid=bafgatewaycid;generated_at=2026-01-02T03:04:05Z",
                "Content-Security-Policy": DEFAULT_CSP_TEMPLATE,
                "Strict-Transport-Security": DEFAULT_HSTS_TEMPLATE
            }
        });
        fs::write(
            &binding_path,
            serde_json::to_vec_pretty(&payload).expect("serialize binding"),
        )
        .expect("write binding");

        let err = verify_gateway_binding(&binding_path, &GatewayBindingExpectations::default())
            .expect_err("verification should fail without Permissions-Policy");
        match err {
            GatewayBindingVerifyError::MissingHeader { header, .. } => {
                assert_eq!(header, "Permissions-Policy");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn acme_plan_lists_expected_certificates() {
        let options = AcmePlanOptions {
            names: vec!["docs.sora".to_string()],
            directory_url: "https://acme.invalid/directory".to_string(),
            include_canonical_wildcard: true,
            include_pretty_hosts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let plan = build_acme_plan(&options).expect("plan renders");
        assert_eq!(plan.directory_url, "https://acme.invalid/directory");
        assert_eq!(plan.generated_at, "1970-01-01T00:00:00Z");
        assert_eq!(plan.hosts.len(), 1);
        let host = &plan.hosts[0];
        assert!(host.canonical_host.ends_with(".gw.sora.id"));
        assert_eq!(
            host.canonical_wildcard,
            format!("*.{}", host.canonical_host)
        );
        let wildcard = host
            .certificates
            .iter()
            .find(|plan| matches!(plan.kind, AcmeCertificateKind::CanonicalWildcard))
            .expect("wildcard plan present");
        assert_eq!(
            wildcard.san,
            vec![host.canonical_wildcard.clone(), host.canonical_host.clone()]
        );
        assert_eq!(wildcard.recommended_challenges, vec!["dns-01"]);
        assert_eq!(
            wildcard.dns_challenge_labels,
            vec![format!("_acme-challenge.{}", host.canonical_host)]
        );
        let pretty = host
            .certificates
            .iter()
            .find(|plan| matches!(plan.kind, AcmeCertificateKind::PrettyHost))
            .expect("pretty host plan present");
        assert_eq!(pretty.san, vec![host.pretty_host.clone()]);
        assert_eq!(
            pretty.recommended_challenges,
            vec!["tls-alpn-01", "http-01"]
        );
    }

    #[test]
    fn route_plan_emits_promotion_and_rollback_entries() {
        let options = RoutePlanOptions {
            names: vec!["docs.sora".to_string()],
            include_pretty_hosts: true,
            generated_at: OffsetDateTime::UNIX_EPOCH,
        };
        let plan = build_route_plan(&options).expect("plan renders");
        assert_eq!(plan.entries.len(), 1);
        let entry = &plan.entries[0];
        assert!(
            entry
                .promote_steps
                .iter()
                .any(|step| step.action == "promote_canonical_binding")
        );
        assert!(
            entry
                .rollback_steps
                .iter()
                .any(|step| step.action == "rollback_canonical_binding")
        );
        assert!(
            entry
                .promote_steps
                .iter()
                .any(|step| step.action == "promote_pretty_binding")
        );
        assert!(
            entry
                .rollback_steps
                .iter()
                .any(|step| step.action == "rollback_pretty_binding")
        );
    }

    #[test]
    fn manifest_cid_and_digest_from_path_yields_expected_values() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(&manifest_path, r#"{ "root_cid_hex": "0123456789abcdef" }"#)
            .expect("write manifest");

        let info =
            super::manifest_cid_and_digest_from_path(&manifest_path).expect("derive metadata");
        let root = hex::decode("0123456789abcdef").expect("root hex");
        let expected_cid = format!("b{}", BASE32_NOPAD.encode(&root).to_ascii_lowercase());
        assert_eq!(info.manifest_cid, expected_cid);
        let expected_digest = blake3_hash(&fs::read(&manifest_path).expect("read manifest"))
            .to_hex()
            .to_string();
        assert_eq!(info.manifest_digest_hex, expected_digest);
    }
}

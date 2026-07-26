//! SoraNet point-of-presence automation helpers used by the `soranet-popctl` CLI.
//!
//! The module offers data structures for PoP configuration templates,
//! validation routines, and health evaluation helpers so the CLI can stay thin.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
};

use norito::json::{JsonDeserialize, JsonSerialize};

/// Configuration describing a SoraNet PoP rollout.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct PopConfig {
    /// Top-level metadata about the PoP.
    pub metadata: PopMetadata,
    /// Networking and rack layout information.
    pub network: NetworkConfig,
    /// Services that should be provisioned inside the PoP.
    #[norito(default)]
    pub services: Vec<ServiceConfig>,
    /// Secret material required by the PoP.
    #[norito(default)]
    pub secrets: Vec<SecretSpec>,
    /// Promotion policy controlling when a PoP transitions between lifecycle stages.
    pub promotion: PromotionPolicy,
    /// Sigstore policy gating golden image attestations.
    pub sigstore: SigstorePolicy,
    /// Health policy describing required probes.
    pub health: HealthPolicy,
}

/// Static metadata attached to a PoP configuration.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct PopMetadata {
    /// Canonical identifier for the PoP (usually site code or city index).
    pub name: String,
    /// Geographic region or metro area.
    pub region: String,
    /// Deployment environment (e.g., `lab`, `alpha`, `ga`).
    pub environment: String,
    /// Additional labels exposed to automation tooling.
    #[norito(default)]
    pub labels: BTreeMap<String, String>,
}

/// Network topology and rack information for the PoP.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct NetworkConfig {
    /// Autonomous system number announced by the PoP.
    pub asn: u32,
    /// Anycast IPv4 prefixes advertised by the PoP.
    #[norito(default)]
    pub anycast_ipv4: Vec<String>,
    /// Anycast IPv6 prefixes advertised by the PoP.
    #[norito(default)]
    pub anycast_ipv6: Vec<String>,
    /// Rack layout definition.
    #[norito(default)]
    pub racks: Vec<RackSpec>,
}

/// Rack specification describing the hardware footprint.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct RackSpec {
    /// Rack identifier, usually aligning with datacentre naming.
    pub name: String,
    /// High-level role for the rack (edge, core, cache).
    pub role: String,
    /// Nodes provisioned in the rack.
    #[norito(default)]
    pub nodes: Vec<NodeSpec>,
}

/// Node specification inside a PoP rack.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct NodeSpec {
    /// Hostname assigned to the node.
    pub hostname: String,
    /// Functional roles performed by the node (gateway, cache, resolver, etc).
    #[norito(default)]
    pub roles: Vec<String>,
    /// Base image digest used for provisioning.
    pub image: String,
}

/// Service configuration describing workloads deployed within the PoP.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct ServiceConfig {
    /// Human readable service name.
    pub name: String,
    /// Service role identifier (e.g., `edge-gateway`, `resolver`).
    pub role: String,
    /// Container or VM image reference.
    pub image: String,
    /// Version or release channel of the service.
    pub version: String,
    /// Ports exposed by the service.
    #[norito(default)]
    pub ports: Vec<u16>,
    /// Environment variables injected at runtime.
    #[norito(default)]
    pub env: BTreeMap<String, String>,
}

/// Secrets required for PoP bootstrap.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SecretSpec {
    /// Logical name of the secret.
    pub name: String,
    /// Storage path or vault reference.
    pub path: String,
    /// Rotation policy in days.
    pub rotation_days: u32,
    /// Optional operator notes.
    #[norito(default)]
    pub description: Option<String>,
}

/// Promotion policy covering lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct PromotionPolicy {
    /// Ordered list of stages that a PoP must pass through.
    pub stages: Vec<PromotionStage>,
}

/// Individual promotion stage with explicit gates.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct PromotionStage {
    /// Stage identifier.
    pub name: String,
    /// Operator facing summary of the stage.
    pub description: String,
    /// Gates that must succeed before advancement.
    #[norito(default)]
    pub gates: Vec<PromotionGate>,
}

/// Promotion gate type.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct PromotionGate {
    /// Gate type identifier (e.g., `sigstore-attestation`, `health-check`, `manual-approval`).
    pub kind: String,
    /// Optional gate specific payload.
    #[norito(default)]
    pub description: Option<String>,
    /// Service the gate targets (if applicable).
    #[norito(default)]
    pub service: Option<String>,
}

/// Sigstore policy configuration.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct SigstorePolicy {
    /// Fulcio root or mirror endpoint.
    pub fulcio_url: String,
    /// Rekor transparency log endpoint.
    pub rekor_url: String,
    /// Trusted OIDC issuers whose identities are accepted.
    #[norito(default)]
    pub trusted_oidc_issuers: Vec<String>,
    /// Expected annotations embedded in attestations.
    #[norito(default)]
    pub required_annotations: BTreeMap<String, String>,
}

/// Health policy capturing required probes.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct HealthPolicy {
    /// Expected probes that should report healthy before promotion.
    pub checks: Vec<HealthCheckSpec>,
}

/// Health check definition for the PoP.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct HealthCheckSpec {
    /// Unique identifier of the check.
    pub name: String,
    /// Service targeted by the check.
    pub service: String,
    /// Endpoint polled by the check.
    pub endpoint: String,
    /// Polling interval in seconds.
    pub interval_seconds: u32,
    /// Expected status labels accepted as healthy.
    #[norito(default)]
    pub expect: Vec<String>,
    /// Optional latency threshold in milliseconds.
    #[norito(default)]
    pub latency_threshold_ms: Option<u64>,
}

/// Template options used to seed a PoP configuration.
#[derive(Debug, Clone)]
pub struct TemplateOptions {
    /// PoP identifier.
    pub name: String,
    /// Deployment region.
    pub region: String,
    /// Deployment environment.
    pub environment: String,
    /// Autonomous system number.
    pub asn: u32,
    /// IPv4 prefixes.
    pub anycast_ipv4: Vec<String>,
    /// IPv6 prefixes.
    pub anycast_ipv6: Vec<String>,
    /// Control plane image reference.
    pub control_plane_image: String,
    /// Data plane (gateway) image reference.
    pub edge_image: String,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            name: "pop-001".to_string(),
            region: "us-west-1".to_string(),
            environment: "alpha".to_string(),
            asn: 65_512,
            anycast_ipv4: vec!["203.0.113.0/24".to_string()],
            anycast_ipv6: vec!["2001:db8::/48".to_string()],
            control_plane_image: "registry.sora.network/infra/control-plane:latest".to_string(),
            edge_image: "registry.sora.network/infra/edge-gateway:latest".to_string(),
        }
    }
}

/// Build a default PoP configuration using the supplied template options.
pub fn build_template(options: &TemplateOptions) -> PopConfig {
    let metadata = PopMetadata {
        name: options.name.clone(),
        region: options.region.clone(),
        environment: options.environment.clone(),
        labels: BTreeMap::new(),
    };

    let network = NetworkConfig {
        asn: options.asn,
        anycast_ipv4: options.anycast_ipv4.clone(),
        anycast_ipv6: options.anycast_ipv6.clone(),
        racks: vec![
            RackSpec {
                name: format!("{}-edge-a", options.name),
                role: "edge".to_string(),
                nodes: vec![
                    NodeSpec {
                        hostname: format!("{}-edge-01", options.name),
                        roles: vec!["edge-gateway".to_string()],
                        image: options.edge_image.clone(),
                    },
                    NodeSpec {
                        hostname: format!("{}-edge-02", options.name),
                        roles: vec!["edge-gateway".to_string()],
                        image: options.edge_image.clone(),
                    },
                ],
            },
            RackSpec {
                name: format!("{}-core-a", options.name),
                role: "core".to_string(),
                nodes: vec![NodeSpec {
                    hostname: format!("{}-control-01", options.name),
                    roles: vec!["control-plane".to_string(), "telemetry".to_string()],
                    image: options.control_plane_image.clone(),
                }],
            },
        ],
    };

    let services = vec![
        ServiceConfig {
            name: "soranet-edge-gateway".to_string(),
            role: "edge-gateway".to_string(),
            image: options.edge_image.clone(),
            version: "preview".to_string(),
            ports: vec![80, 443, 4433],
            env: BTreeMap::new(),
        },
        ServiceConfig {
            name: "soranet-resolver".to_string(),
            role: "resolver".to_string(),
            image: "registry.sora.network/infra/resolver:latest".to_string(),
            version: "preview".to_string(),
            ports: vec![853, 8853],
            env: BTreeMap::new(),
        },
        ServiceConfig {
            name: "soranet-telemetry".to_string(),
            role: "telemetry".to_string(),
            image: options.control_plane_image.clone(),
            version: "preview".to_string(),
            ports: vec![9090],
            env: BTreeMap::new(),
        },
    ];

    let secrets = vec![
        SecretSpec {
            name: "acme-account-key".to_string(),
            path: "vault://edge/acme/account-key".to_string(),
            rotation_days: 30,
            description: Some("ACME account key used for wildcard issuance".to_string()),
        },
        SecretSpec {
            name: "soranet-gateway-token".to_string(),
            path: "vault://edge/runtime/gateway-token".to_string(),
            rotation_days: 7,
            description: Some("Gateway bootstrap token exchanged with orchestrator".to_string()),
        },
    ];

    let promotion = PromotionPolicy {
        stages: vec![
            PromotionStage {
                name: "lab".to_string(),
                description: "Provision in lab, verify golden image and sigstore attestations"
                    .to_string(),
                gates: vec![
                    PromotionGate {
                        kind: "sigstore-attestation".to_string(),
                        description: Some(
                            "Golden image attested by trusted Fulcio issuer".to_string(),
                        ),
                        service: Some("soranet-edge-gateway".to_string()),
                    },
                    PromotionGate {
                        kind: "health-check".to_string(),
                        description: Some("All health checks passing for 30 minutes".to_string()),
                        service: None,
                    },
                ],
            },
            PromotionStage {
                name: "alpha".to_string(),
                description: "Enable limited tenant traffic and monitor stability".to_string(),
                gates: vec![PromotionGate {
                    kind: "manual-approval".to_string(),
                    description: Some("Edge Infra duty officer approval".to_string()),
                    service: None,
                }],
            },
            PromotionStage {
                name: "ga".to_string(),
                description: "Full production traffic".to_string(),
                gates: vec![PromotionGate {
                    kind: "slo-regression".to_string(),
                    description: Some("Latency and error budgets within guardrails".to_string()),
                    service: None,
                }],
            },
        ],
    };

    let sigstore = SigstorePolicy {
        fulcio_url: "https://fulcio.sigstore.dev".to_string(),
        rekor_url: "https://rekor.sigstore.dev".to_string(),
        trusted_oidc_issuers: vec![
            "https://accounts.google.com".to_string(),
            "https://issuer.sora.network/dex".to_string(),
        ],
        required_annotations: {
            let mut annotations = BTreeMap::new();
            annotations.insert("soranet.component".to_string(), "edge-gateway".to_string());
            annotations.insert(
                "soranet.environment".to_string(),
                options.environment.clone(),
            );
            annotations
        },
    };

    let health = HealthPolicy {
        checks: vec![
            HealthCheckSpec {
                name: "edge-http".to_string(),
                service: "soranet-edge-gateway".to_string(),
                endpoint: "https://edge.example.net/healthz".to_string(),
                interval_seconds: 30,
                expect: vec!["healthy".to_string()],
                latency_threshold_ms: Some(200),
            },
            HealthCheckSpec {
                name: "resolver-dnssec".to_string(),
                service: "soranet-resolver".to_string(),
                endpoint: "https://resolver.example.net/healthz".to_string(),
                interval_seconds: 30,
                expect: vec!["healthy".to_string(), "degraded".to_string()],
                latency_threshold_ms: Some(100),
            },
            HealthCheckSpec {
                name: "telemetry-scrape".to_string(),
                service: "soranet-telemetry".to_string(),
                endpoint: "https://telemetry.example.net/-/ready".to_string(),
                interval_seconds: 60,
                expect: vec!["healthy".to_string()],
                latency_threshold_ms: Some(500),
            },
        ],
    };

    PopConfig {
        metadata,
        network,
        services,
        secrets,
        promotion,
        sigstore,
        health,
    }
}

/// Validation errors produced while checking a PoP configuration.
#[derive(Debug, thiserror::Error)]
pub enum PopValidationError {
    /// Report multiple validation failures at once.
    #[error("configuration validation failed: {0}")]
    Invalid(String),
}

/// Validate a PoP configuration and surface all discovered issues.
pub fn validate_config(config: &PopConfig) -> Result<(), PopValidationError> {
    let mut errors = Vec::new();

    if config.metadata.name.trim().is_empty() {
        errors.push("metadata.name must not be empty".to_string());
    }

    if config.metadata.region.trim().is_empty() {
        errors.push("metadata.region must not be empty".to_string());
    }

    if config.network.anycast_ipv4.is_empty() && config.network.anycast_ipv6.is_empty() {
        errors.push("at least one anycast prefix must be configured (IPv4/IPv6)".to_string());
    }

    if config.services.is_empty() {
        errors.push("at least one service must be defined".to_string());
    } else {
        for service in &config.services {
            if service.name.trim().is_empty() {
                errors.push("service.name must not be empty".to_string());
            }
            if service.image.trim().is_empty() {
                errors.push(format!(
                    "service `{}` must declare a non-empty image reference",
                    service.name
                ));
            }
            if service.role.trim().is_empty() {
                errors.push(format!(
                    "service `{}` must declare a role identifier",
                    service.name
                ));
            }
        }
    }

    if !config
        .services
        .iter()
        .any(|service| service.role == "edge-gateway")
    {
        errors.push("an `edge-gateway` service role is required for PoP operation".to_string());
    }

    for secret in &config.secrets {
        if secret.name.trim().is_empty() {
            errors.push("secret.name must not be empty".to_string());
        }
        if secret.path.trim().is_empty() {
            errors.push(format!(
                "secret `{}` must include a vault or file path",
                secret.name
            ));
        }
        if secret.rotation_days == 0 {
            errors.push(format!(
                "secret `{}` must declare a rotation policy greater than zero days",
                secret.name
            ));
        }
    }

    if config.promotion.stages.is_empty() {
        errors.push("promotion policy must contain at least one stage".to_string());
    }

    if config.sigstore.fulcio_url.trim().is_empty() {
        errors.push("sigstore.fulcio_url must not be empty".to_string());
    }

    if config.sigstore.rekor_url.trim().is_empty() {
        errors.push("sigstore.rekor_url must not be empty".to_string());
    }

    if config.health.checks.is_empty() {
        errors.push("health policy must include at least one check".to_string());
    } else {
        let mut seen = BTreeSet::new();
        for check in &config.health.checks {
            if !seen.insert((&check.service, &check.name)) {
                errors.push(format!(
                    "duplicate health check `{}` for service `{}`",
                    check.name, check.service
                ));
            }
            if check.expect.is_empty() {
                errors.push(format!(
                    "health check `{}` must include at least one expected status label",
                    check.name
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(PopValidationError::Invalid(errors.join("; ")))
    }
}

/// Parsed health report emitted by PoP monitoring.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct HealthReport {
    /// RFC3339 timestamp when the report was generated.
    pub generated_at: String,
    /// Individual service health entries.
    pub services: Vec<ServiceHealth>,
}

/// Health entry for a specific service.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct ServiceHealth {
    /// Service identifier.
    pub service: String,
    /// Declared service role.
    pub role: String,
    /// Top-level health status label.
    pub status: String,
    /// Detailed check results.
    #[norito(default)]
    pub checks: Vec<CheckResult>,
}

/// Detailed result for a health check.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct CheckResult {
    /// Check identifier.
    pub name: String,
    /// Status label.
    pub status: String,
    /// Optional latency measurement in milliseconds.
    #[norito(default)]
    pub latency_ms: Option<u64>,
    /// Optional free-form message.
    #[norito(default)]
    pub message: Option<String>,
}

/// High-level health state interpreted from the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthState {
    /// Service and all checks are healthy.
    Healthy,
    /// Service is operating with reduced redundancy or increased latency.
    Degraded,
    /// Service is unavailable.
    Unhealthy,
}

impl HealthState {
    fn from_label(label: &str) -> Option<Self> {
        match label {
            "healthy" => Some(Self::Healthy),
            "degraded" => Some(Self::Degraded),
            "unhealthy" => Some(Self::Unhealthy),
            _ => None,
        }
    }

    /// Convert the health state back into the canonical label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        }
    }
}

impl fmt::Display for HealthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Summary produced after evaluating PoP health.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSummary {
    /// Worst-case status observed across services.
    pub overall_status: HealthState,
    /// Checks that were expected but missing in the report.
    pub missing_checks: Vec<MissingCheck>,
    /// Checks that failed or reported degraded status.
    pub failed_checks: Vec<FailedCheck>,
}

/// Missing health check information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingCheck {
    /// Service identifier.
    pub service: String,
    /// Check identifier.
    pub check: String,
}

/// Failed health check information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedCheck {
    /// Service identifier.
    pub service: String,
    /// Check identifier.
    pub check: String,
    /// Resulting status for the check.
    pub status: HealthState,
    /// Optional failure message.
    pub message: Option<String>,
}

/// Errors surfaced while processing health reports.
#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    /// Report omitted a service that appears in the configuration.
    #[error("missing health entry for service `{service}`")]
    MissingService { service: String },
    /// Health report contained an unknown status label.
    #[error("unknown health status `{status}`")]
    UnknownStatus { status: String },
}

/// Evaluate PoP health by comparing the report against the configuration.
pub fn evaluate_health(
    config: &PopConfig,
    report: &HealthReport,
) -> Result<HealthSummary, HealthError> {
    let mut overall = HealthState::Healthy;
    let mut failed_checks = Vec::new();
    let mut missing_checks = Vec::new();

    let expected_checks: BTreeMap<(&str, &str), &HealthCheckSpec> = config
        .health
        .checks
        .iter()
        .map(|check| ((check.service.as_str(), check.name.as_str()), check))
        .collect();

    for service in &config.services {
        let entry = report
            .services
            .iter()
            .find(|item| item.service == service.name)
            .ok_or_else(|| HealthError::MissingService {
                service: service.name.clone(),
            })?;

        let service_state =
            HealthState::from_label(&entry.status).ok_or_else(|| HealthError::UnknownStatus {
                status: entry.status.clone(),
            })?;
        if service_state > overall {
            overall = service_state;
        }

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for check in &entry.checks {
            let check_state = HealthState::from_label(&check.status).ok_or_else(|| {
                HealthError::UnknownStatus {
                    status: check.status.clone(),
                }
            })?;
            if seen.insert(check.name.as_str()) && check_state > HealthState::Healthy {
                if check_state > overall {
                    overall = check_state;
                }
                failed_checks.push(FailedCheck {
                    service: service.name.clone(),
                    check: check.name.clone(),
                    status: check_state,
                    message: check.message.clone(),
                });
            }
        }

        for ((svc, check_name), expected) in expected_checks.iter() {
            if *svc == service.name {
                if !seen.contains(check_name) {
                    missing_checks.push(MissingCheck {
                        service: service.name.clone(),
                        check: (*check_name).to_string(),
                    });
                    if overall < HealthState::Degraded {
                        overall = HealthState::Degraded;
                    }
                } else if let Some(threshold) = expected.latency_threshold_ms
                    && let Some(entry_check) =
                        entry.checks.iter().find(|check| check.name == *check_name)
                    && let Some(latency) = entry_check.latency_ms
                    && latency > threshold
                {
                    failed_checks.push(FailedCheck {
                        service: service.name.clone(),
                        check: (*check_name).to_string(),
                        status: HealthState::Degraded,
                        message: Some(format!(
                            "latency {latency}ms exceeded threshold {threshold}ms"
                        )),
                    });
                    if overall < HealthState::Degraded {
                        overall = HealthState::Degraded;
                    }
                }
            }
        }
    }

    Ok(HealthSummary {
        overall_status: overall,
        missing_checks,
        failed_checks,
    })
}

/// Sigstore bundle with annotations extracted for validation.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct SigstoreBundle {
    /// OIDC issuer URL associated with the signature.
    pub issuer: String,
    /// Subject identifier (usually workload identity).
    pub subject: String,
    /// Container or VM image digest (e.g., `sha256:...`).
    pub image_digest: String,
    /// Recorded annotations bundled with the attestation.
    #[norito(default)]
    pub annotations: BTreeMap<String, String>,
    /// Optional timestamp of the attestation.
    #[norito(default)]
    pub issued_at: Option<String>,
}

/// PXE execution log entry emitted during provisioning.
#[derive(Debug, Clone, JsonDeserialize)]
pub struct PxeEvent {
    /// Hostname that executed the PXE stage.
    pub hostname: String,
    /// Provisioning stage identifier.
    pub stage: String,
    /// Resulting status (e.g., `success`, `failure`).
    pub status: String,
    /// Optional digest of the execution log artefact.
    #[norito(default)]
    pub log_digest: Option<String>,
    /// Optional timestamp for auditing.
    #[norito(default)]
    pub timestamp: Option<String>,
}

/// Attestation verification failures.
#[derive(Debug, thiserror::Error)]
pub enum AttestationError {
    /// Issuer was not present in the trusted list.
    #[error("issuer `{issuer}` is not trusted")]
    UntrustedIssuer { issuer: String },
    /// Required annotation missing or mismatched.
    #[error("required annotation `{key}` expected `{expected}` but found `{found}`")]
    AnnotationMismatch {
        key: String,
        expected: String,
        found: String,
    },
    /// Annotation missing entirely.
    #[error("required annotation `{key}` missing from bundle")]
    MissingAnnotation { key: String },
    /// Attestation did not include an image digest.
    #[error("attestation bundle missing image digest")]
    MissingImageDigest,
}

/// PXE log validation failures.
#[derive(Debug, thiserror::Error)]
pub enum PxeLogError {
    /// PXE log validation collected one or more issues.
    #[error("pxe log validation failed: {0}")]
    Invalid(String),
}

/// Verify the attestation bundle against the configured sigstore policy.
pub fn verify_attestation(
    policy: &SigstorePolicy,
    bundle: &SigstoreBundle,
) -> Result<(), AttestationError> {
    if bundle.image_digest.trim().is_empty() {
        return Err(AttestationError::MissingImageDigest);
    }

    if !policy
        .trusted_oidc_issuers
        .iter()
        .any(|issuer| issuer == &bundle.issuer)
    {
        return Err(AttestationError::UntrustedIssuer {
            issuer: bundle.issuer.clone(),
        });
    }

    for (key, expected) in &policy.required_annotations {
        match bundle.annotations.get(key) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(AttestationError::AnnotationMismatch {
                    key: key.clone(),
                    expected: expected.clone(),
                    found: found.clone(),
                });
            }
            None => {
                return Err(AttestationError::MissingAnnotation { key: key.clone() });
            }
        }
    }

    Ok(())
}

/// Ensure PXE logs cover every node and report success.
pub fn verify_pxe_log(config: &PopConfig, events: &[PxeEvent]) -> Result<(), PxeLogError> {
    let mut errors = Vec::new();
    let mut successes: HashSet<&str> = HashSet::new();

    let expected_hosts: BTreeSet<&str> = config
        .network
        .racks
        .iter()
        .flat_map(|rack| rack.nodes.iter().map(|node| node.hostname.as_str()))
        .collect();

    for event in events {
        if !expected_hosts.contains(event.hostname.as_str()) {
            errors.push(format!(
                "log entry references unknown host `{}`",
                event.hostname
            ));
            continue;
        }

        match event.stage.as_str() {
            "pxe" | "PXE" => match event.status.as_str() {
                "success" | "completed" | "success-with-warnings" => {
                    successes.insert(event.hostname.as_str());
                }
                "failure" | "failed" => {
                    errors.push(format!("host `{}` reported PXE failure", event.hostname))
                }
                other => errors.push(format!(
                    "host `{}` reported unsupported PXE status `{other}`",
                    event.hostname
                )),
            },
            other => errors.push(format!(
                "host `{}` reported unexpected stage `{other}` (expected `pxe`)",
                event.hostname
            )),
        }
    }

    for host in expected_hosts {
        if !successes.contains(host) {
            errors.push(format!("missing PXE success event for host `{host}`"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(PxeLogError::Invalid(errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_includes_edge_gateway() {
        let config = build_template(&TemplateOptions::default());
        assert_eq!(config.metadata.environment, "alpha");
        assert!(
            config
                .services
                .iter()
                .any(|service| service.role == "edge-gateway"),
            "template must include an edge-gateway service"
        );
        assert!(
            config
                .health
                .checks
                .iter()
                .any(|check| check.service == "soranet-edge-gateway"),
            "template must include an edge health check"
        );
    }

    #[test]
    fn validation_detects_missing_edge_role() {
        let mut config = build_template(&TemplateOptions::default());
        config.services.retain(|svc| svc.role != "edge-gateway");
        let error = validate_config(&config).expect_err("validation should fail");
        let message = error.to_string();
        assert!(
            message.contains("edge-gateway"),
            "error should mention missing edge-gateway role, got {message}"
        );
    }

    #[test]
    fn health_reports_success() {
        let config = build_template(&TemplateOptions::default());
        let report = HealthReport {
            generated_at: "2026-01-02T03:04:05Z".to_string(),
            services: vec![
                ServiceHealth {
                    service: "soranet-edge-gateway".to_string(),
                    role: "edge-gateway".to_string(),
                    status: "healthy".to_string(),
                    checks: vec![CheckResult {
                        name: "edge-http".to_string(),
                        status: "healthy".to_string(),
                        latency_ms: Some(120),
                        message: None,
                    }],
                },
                ServiceHealth {
                    service: "soranet-resolver".to_string(),
                    role: "resolver".to_string(),
                    status: "healthy".to_string(),
                    checks: vec![CheckResult {
                        name: "resolver-dnssec".to_string(),
                        status: "healthy".to_string(),
                        latency_ms: Some(80),
                        message: None,
                    }],
                },
                ServiceHealth {
                    service: "soranet-telemetry".to_string(),
                    role: "telemetry".to_string(),
                    status: "healthy".to_string(),
                    checks: vec![CheckResult {
                        name: "telemetry-scrape".to_string(),
                        status: "healthy".to_string(),
                        latency_ms: Some(300),
                        message: None,
                    }],
                },
            ],
        };
        let summary = evaluate_health(&config, &report).expect("health evaluation succeeds");
        assert_eq!(summary.overall_status, HealthState::Healthy);
        assert!(summary.failed_checks.is_empty());
        assert!(summary.missing_checks.is_empty());
    }

    #[test]
    fn health_detects_missing_check() {
        let config = build_template(&TemplateOptions::default());
        let report = HealthReport {
            generated_at: "2026-01-02T03:04:05Z".to_string(),
            services: vec![
                ServiceHealth {
                    service: "soranet-edge-gateway".to_string(),
                    role: "edge-gateway".to_string(),
                    status: "healthy".to_string(),
                    checks: vec![],
                },
                ServiceHealth {
                    service: "soranet-resolver".to_string(),
                    role: "resolver".to_string(),
                    status: "degraded".to_string(),
                    checks: vec![CheckResult {
                        name: "resolver-dnssec".to_string(),
                        status: "degraded".to_string(),
                        latency_ms: Some(250),
                        message: Some("elevated latency".to_string()),
                    }],
                },
                ServiceHealth {
                    service: "soranet-telemetry".to_string(),
                    role: "telemetry".to_string(),
                    status: "healthy".to_string(),
                    checks: vec![CheckResult {
                        name: "telemetry-scrape".to_string(),
                        status: "healthy".to_string(),
                        latency_ms: Some(250),
                        message: None,
                    }],
                },
            ],
        };
        let summary = evaluate_health(&config, &report).expect("health evaluation succeeds");
        assert_eq!(summary.overall_status, HealthState::Degraded);
        assert!(
            summary
                .missing_checks
                .iter()
                .any(|missing| missing.check == "edge-http"),
            "missing_checks should flag the absent edge-http probe"
        );
        assert!(
            summary
                .failed_checks
                .iter()
                .any(|failed| failed.check == "resolver-dnssec"),
            "failed_checks should include degraded resolver-dnssec"
        );
    }

    #[test]
    fn health_unknown_status_errors() {
        let config = build_template(&TemplateOptions::default());
        let report = HealthReport {
            generated_at: "2026-01-02T03:04:05Z".to_string(),
            services: vec![ServiceHealth {
                service: "soranet-edge-gateway".to_string(),
                role: "edge-gateway".to_string(),
                status: "mystery".to_string(),
                checks: vec![],
            }],
        };
        let error = evaluate_health(&config, &report).expect_err("unexpected status should fail");
        match error {
            HealthError::UnknownStatus { status } => {
                assert_eq!(status, "mystery");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn attestation_verification_succeeds() {
        let mut config = build_template(&TemplateOptions::default());
        config.sigstore.trusted_oidc_issuers = vec!["https://issuer.example".to_string()];
        let annotations: BTreeMap<_, _> = config
            .sigstore
            .required_annotations
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let bundle = SigstoreBundle {
            issuer: "https://issuer.example".to_string(),
            subject: "workload@example".to_string(),
            image_digest: "sha256:1234".to_string(),
            annotations,
            issued_at: Some("2026-01-02T03:04:05Z".to_string()),
        };
        verify_attestation(&config.sigstore, &bundle).expect("attestation should pass");
    }

    #[test]
    fn attestation_missing_annotation_fails() {
        let mut config = build_template(&TemplateOptions::default());
        config.sigstore.trusted_oidc_issuers = vec!["https://issuer.example".to_string()];
        config.sigstore.required_annotations =
            BTreeMap::from([("soranet.component".to_string(), "edge-gateway".to_string())]);
        let bundle = SigstoreBundle {
            issuer: "https://issuer.example".to_string(),
            subject: "workload@example".to_string(),
            image_digest: "sha256:1234".to_string(),
            annotations: BTreeMap::new(),
            issued_at: None,
        };
        let error =
            verify_attestation(&config.sigstore, &bundle).expect_err("validation should fail");
        match error {
            AttestationError::MissingAnnotation { key } => {
                assert_eq!(key, "soranet.component");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn pxe_log_verification_detects_failure() {
        let config = build_template(&TemplateOptions::default());
        let events = vec![
            PxeEvent {
                hostname: "pop-001-edge-01".to_string(),
                stage: "pxe".to_string(),
                status: "success".to_string(),
                log_digest: None,
                timestamp: None,
            },
            PxeEvent {
                hostname: "pop-001-edge-02".to_string(),
                stage: "pxe".to_string(),
                status: "failure".to_string(),
                log_digest: None,
                timestamp: None,
            },
        ];
        let error = verify_pxe_log(&config, &events).expect_err("pxe log must fail");
        assert!(
            error.to_string().contains("missing PXE success event"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn pxe_log_verification_succeeds() {
        let config = build_template(&TemplateOptions::default());
        let events = vec![
            PxeEvent {
                hostname: "pop-001-edge-01".to_string(),
                stage: "pxe".to_string(),
                status: "success".to_string(),
                log_digest: Some("sha256:edge01".to_string()),
                timestamp: Some("2026-01-02T03:04:05Z".to_string()),
            },
            PxeEvent {
                hostname: "pop-001-edge-02".to_string(),
                stage: "pxe".to_string(),
                status: "completed".to_string(),
                log_digest: Some("sha256:edge02".to_string()),
                timestamp: Some("2026-01-02T03:04:06Z".to_string()),
            },
            PxeEvent {
                hostname: "pop-001-control-01".to_string(),
                stage: "pxe".to_string(),
                status: "success".to_string(),
                log_digest: Some("sha256:control1".to_string()),
                timestamp: Some("2026-01-02T03:04:07Z".to_string()),
            },
        ];
        verify_pxe_log(&config, &events).expect("pxe log should succeed");
    }
}

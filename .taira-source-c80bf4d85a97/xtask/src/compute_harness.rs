//! Shared helpers for compute fixtures, SLO reports, and the lightweight gateway.
//! The helpers mirror the minimal entrypoints shipped in the `compute_gateway`
//! binary so tests and CLI fixtures stay in sync.

use std::{
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
};

use iroha_config::parameters::defaults::compute as compute_defaults;
use iroha_crypto::Hash;
use iroha_data_model::{
    compute::{
        ComputeAuthPolicy, ComputeCall, ComputeCodec, ComputeManifest, ComputeMetering,
        ComputeOutcome, ComputeOutcomeKind, ComputePriceAmplifiers, ComputePriceWeights,
        ComputeRoute, ComputeRouteId, ComputeSandboxRules, ComputeValidationError,
    },
    name::Name,
};

/// Minimal error surface used by the compute harness.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ComputeHarnessError {
    /// Entrypoint not recognised by the harness.
    #[error("unknown entrypoint {0}")]
    UnknownEntrypoint(String),
}

/// Execute a built-in entrypoint and return the outcome and optional response bytes.
pub fn execute_entrypoint(
    entrypoint: &str,
    payload: &[u8],
    call: &ComputeCall,
) -> Result<(ComputeOutcome, Option<Vec<u8>>), ComputeHarnessError> {
    let mut response = match entrypoint {
        "echo" | "quote_entry" => Some(payload.to_vec()),
        "uppercase" => Some(payload.iter().map(u8::to_ascii_uppercase).collect()),
        "sha3" => Some(Hash::new(payload).as_ref().to_vec()),
        "amplify" => {
            let mut buf = payload.to_vec();
            buf.extend_from_slice(payload);
            Some(buf)
        }
        other => return Err(ComputeHarnessError::UnknownEntrypoint(other.to_string())),
    };

    let cycles = (payload.len() as u64)
        .saturating_mul(10_000)
        .saturating_add(5_000);
    if cycles > call.gas_limit.get() {
        response = None;
    }

    if let Some(resp) = &response
        && resp.len() as u64 > call.max_response_bytes.get()
    {
        response = None;
    }

    let outcome = if let Some(resp) = &response {
        ComputeOutcome {
            kind: ComputeOutcomeKind::Success,
            response_hash: Some(Hash::new(resp)),
            response_bytes: Some(resp.len() as u64),
            response_codec: Some(call.codec),
        }
    } else {
        ComputeOutcome {
            kind: ComputeOutcomeKind::BudgetExhausted,
            response_hash: None,
            response_bytes: None,
            response_codec: None,
        }
    };
    Ok((outcome, response))
}

/// Compute metering for a request/response pair.
pub fn meter(
    route: &ComputeRoute,
    call: &ComputeCall,
    payload: &[u8],
    response_bytes: &Option<Vec<u8>>,
) -> ComputeMetering {
    let ingress_headers: usize = call
        .request
        .headers
        .iter()
        .map(|(k, v)| k.len() + v.len())
        .sum();
    let egress_bytes = response_bytes.as_ref().map_or(0, Vec::len);
    let ingress_bytes = payload.len().saturating_add(ingress_headers);
    let cycles = (payload.len() as u64)
        .saturating_mul(10_000)
        .saturating_add(10_000);
    let duration_ms = 1 + (payload.len() as u64 / 256);

    let mut metering = ComputeMetering {
        cycles,
        ingress_bytes: ingress_bytes as u64,
        egress_bytes: egress_bytes as u64,
        duration_ms,
        price_family: call.price_family.clone(),
        charged_units: 0,
    };

    if metering.cycles > route.gas_budget.get() {
        metering.cycles = route.gas_budget.get();
    }
    if metering.egress_bytes > route.max_response_bytes.get() {
        metering.egress_bytes = route.max_response_bytes.get();
    }
    metering
}

/// Compute charged units for metering based on known price families.
pub fn charge_units(
    price_families: &BTreeMap<Name, ComputePriceWeights>,
    default_price_family: &Name,
    amplifiers: &ComputePriceAmplifiers,
    max_cu_per_call: std::num::NonZeroU64,
    max_amplification_ratio: std::num::NonZeroU32,
    call: &ComputeCall,
    metering: &mut ComputeMetering,
) {
    if metering.ingress_bytes > 0 {
        let max_egress = metering
            .ingress_bytes
            .saturating_mul(max_amplification_ratio.get() as u64);
        metering.egress_bytes = metering.egress_bytes.min(max_egress);
    }

    let price_family = price_families
        .get(&metering.price_family)
        .or_else(|| price_families.get(default_price_family))
        .expect("compute price families must be non-empty");
    let base_units = price_family.charge_units(metering);
    let amplified = amplifiers.apply(base_units, call.execution_class, call.determinism);
    metering.charged_units = amplified.min(max_cu_per_call.get());
}

/// Provide a deterministic default manifest for tests and CLI helpers.
pub fn default_manifest() -> ComputeManifest {
    ComputeManifest {
        namespace: compute_defaults::default_namespaces()
            .into_iter()
            .next()
            .expect("default compute namespace"),
        abi_version: ComputeManifest::ABI_VERSION,
        sandbox: ComputeSandboxRules {
            mode: compute_defaults::sandbox_rules().mode,
            randomness: compute_defaults::sandbox_rules().randomness,
            storage: compute_defaults::sandbox_rules().storage,
            deny_nondeterministic_syscalls: true,
            allow_gpu_hints: false,
            allow_tee_hints: false,
        },
        routes: vec![ComputeRoute {
            id: ComputeRouteId::new(
                "payments".parse().expect("route"),
                "quote".parse().expect("route"),
            ),
            entrypoint: "echo".to_string(),
            codecs: vec![ComputeCodec::NoritoJson, ComputeCodec::OctetStream],
            ttl_slots: compute_defaults::default_ttl_slots(),
            gas_budget: compute_defaults::max_gas_per_call(),
            max_request_bytes: std::num::NonZeroU64::new(compute_defaults::MAX_REQUEST_BYTES.0)
                .expect("request cap"),
            max_response_bytes: std::num::NonZeroU64::new(compute_defaults::MAX_RESPONSE_BYTES.0)
                .expect("response cap"),
            determinism: iroha_data_model::compute::ComputeDeterminism::Strict,
            execution_class: iroha_data_model::compute::ComputeExecutionClass::Cpu,
            input_limits: Some(iroha_data_model::compute::ComputeInputLimits {
                max_inline_bytes: std::num::NonZeroU64::new(
                    compute_defaults::MAX_REQUEST_BYTES.0 / 2,
                )
                .expect("inline cap"),
                max_chunks: std::num::NonZeroU32::new(32).expect("chunk count cap"),
                chunk_size_bytes: std::num::NonZeroU64::new(65_536).expect("chunk size cap"),
            }),
            model: Some(iroha_data_model::compute::ComputeModelRef {
                bundle_hash: Hash::new(b"model:echo"),
                path: "models/echo.bin".to_string(),
                expected_bytes: std::num::NonZeroU64::new(1_048_576).expect("model size"),
                chunk_size_bytes: std::num::NonZeroU64::new(131_072).expect("model chunk"),
            }),
            price_family: compute_defaults::default_price_family(),
            resource_profile: compute_defaults::default_resource_profile(),
            auth: ComputeAuthPolicy::Either,
        }],
    }
}

/// Helper used by fixtures/tests to build a call from a manifest route.
pub fn build_call_for_route(
    manifest: &ComputeManifest,
    route: &ComputeRoute,
    payload: &[u8],
    codec: iroha_data_model::compute::ComputeCodec,
    auth: iroha_data_model::compute::ComputeAuthz,
) -> Result<iroha_data_model::compute::ComputeCall, ComputeValidationError> {
    let call = iroha_data_model::compute::ComputeCall {
        namespace: manifest.namespace.clone(),
        route: route.id.clone(),
        codec,
        ttl_slots: route.ttl_slots,
        gas_limit: route.gas_budget,
        max_response_bytes: route.max_response_bytes,
        determinism: route.determinism,
        execution_class: route.execution_class,
        declared_input_bytes: std::num::NonZeroU64::new(payload.len() as u64),
        declared_input_chunks: std::num::NonZeroU32::new(1),
        sponsor_budget_cu: Some(compute_defaults::sponsor_policy().max_cu_per_call),
        price_family: route.price_family.clone(),
        resource_profile: route.resource_profile.clone(),
        auth,
        request: iroha_data_model::compute::ComputeRequest {
            headers: BTreeMap::new(),
            payload_hash: Hash::new(payload),
        },
    };
    route.validate_call(&call)?;
    manifest.validate_call(&call)?;
    Ok(call)
}

/// Generate a deterministic payload (a..z cycling) of the requested length.
#[must_use]
pub fn payload_with_len(len: usize) -> Vec<u8> {
    (0..len).map(|i| b'a' + (i as u8 % 26)).collect()
}

/// Convenience wrapper for common SLO defaults.
#[must_use]
pub fn slo_targets() -> SloTargets {
    SloTargets {
        max_inflight_per_route: compute_defaults::max_inflight_per_route(),
        queue_depth_per_route: compute_defaults::queue_depth_per_route(),
        max_requests_per_second: compute_defaults::max_requests_per_second(),
        target_p50_latency_ms: compute_defaults::target_p50_latency_ms(),
        target_p95_latency_ms: compute_defaults::target_p95_latency_ms(),
        target_p99_latency_ms: compute_defaults::target_p99_latency_ms(),
    }
}

/// Minimal struct used by the SLO harness to evaluate budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SloTargets {
    /// Maximum in-flight requests per route.
    pub max_inflight_per_route: NonZeroUsize,
    /// Maximum queued requests per route.
    pub queue_depth_per_route: NonZeroUsize,
    /// Allowed sustained requests per second.
    pub max_requests_per_second: NonZeroU32,
    /// p50 latency budget.
    pub target_p50_latency_ms: NonZeroU64,
    /// p95 latency budget.
    pub target_p95_latency_ms: NonZeroU64,
    /// p99 latency budget.
    pub target_p99_latency_ms: NonZeroU64,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::compute::{ComputeAuthz, ComputeCodec};

    use super::*;

    #[test]
    fn entrypoint_respects_gas_and_size_caps() {
        let manifest = default_manifest();
        let route = &manifest.routes[0];
        let payload = payload_with_len(16);
        let call = build_call_for_route(
            &manifest,
            route,
            &payload,
            ComputeCodec::NoritoJson,
            ComputeAuthz::Public,
        )
        .expect("call");
        let (outcome, response) =
            execute_entrypoint(&route.entrypoint, &payload, &call).expect("entrypoint");
        assert!(matches!(outcome.kind, ComputeOutcomeKind::Success));
        assert_eq!(response.as_ref().map(Vec::len), Some(payload.len()));

        let tiny_gas_call = iroha_data_model::compute::ComputeCall {
            gas_limit: NonZeroU64::new(1).unwrap(),
            ..call.clone()
        };
        let (outcome_tiny, response_tiny) =
            execute_entrypoint(&route.entrypoint, &payload, &tiny_gas_call).expect("entrypoint");
        assert!(matches!(
            outcome_tiny.kind,
            ComputeOutcomeKind::BudgetExhausted
        ));
        assert!(response_tiny.is_none());
    }

    #[test]
    fn metering_caps_egress_and_cycles() {
        let manifest = default_manifest();
        let route = &manifest.routes[0];
        let payload = payload_with_len(2048);
        let call = build_call_for_route(
            &manifest,
            route,
            &payload,
            ComputeCodec::NoritoJson,
            ComputeAuthz::Public,
        )
        .expect("call");
        let (_outcome, response) =
            execute_entrypoint(&route.entrypoint, &payload, &call).expect("entrypoint");
        let metering = meter(route, &call, &payload, &response);
        assert!(metering.cycles <= route.gas_budget.get());
        assert!(metering.egress_bytes <= route.max_response_bytes.get());
        assert_eq!(metering.price_family, route.price_family);
    }

    #[test]
    fn default_manifest_contains_expected_route() {
        let manifest = default_manifest();
        assert_eq!(manifest.routes.len(), 1);
        let route = &manifest.routes[0];
        assert_eq!(route.entrypoint, "echo");
        assert!(route.codecs.contains(&ComputeCodec::NoritoJson));
        assert!(route.codecs.contains(&ComputeCodec::OctetStream));
    }
}

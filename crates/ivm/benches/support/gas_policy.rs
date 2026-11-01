//! Benchmark-only selection of the existing IVM acceleration policy.
//!
//! `IROHA_GAS_BENCH_IVM_POLICY=scalar` disables IVM SIMD, Metal and CUDA.
//! The default `runtime` preserves the configured policy. This owner does not
//! control Ed25519, FHE or other libraries' independent dispatch.

/// Policy requested by a gas benchmark, never a node environment setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Preserve the existing IVM policy and its runtime selection.
    Runtime,
    /// Use the existing scalar IVM policy.
    Scalar,
}

/// Parse the benchmark setting, refusing misspelled or empty selections.
///
/// # Errors
/// Returns an error for any value other than `runtime` or `scalar`.
pub fn parse(value: Option<&str>) -> Result<Policy, &'static str> {
    match value {
        None | Some("runtime") => Ok(Policy::Runtime),
        Some("scalar") => Ok(Policy::Scalar),
        Some(_) => Err("IROHA_GAS_BENCH_IVM_POLICY must be runtime or scalar"),
    }
}

/// Apply only IVM's policy and report the selected SIMD backend.
///
/// # Panics
/// Panics if the existing scalar policy fails to select the scalar backend.
pub fn apply(policy: Policy) -> ivm::SimdChoice {
    if policy == Policy::Scalar {
        ivm::set_acceleration_config(ivm::AccelerationConfig {
            enable_simd: false,
            enable_metal: false,
            enable_cuda: false,
            ..ivm::acceleration_config()
        });
    }
    let selected = ivm::simd_choice();
    if policy == Policy::Scalar {
        assert_eq!(
            selected,
            ivm::SimdChoice::Scalar,
            "scalar IVM benchmark policy"
        );
    }
    selected
}

/// Configure a benchmark and emit scoped policy/selection evidence.
///
/// # Panics
/// Panics for an invalid setting or if the scalar policy is not applied.
pub fn configure_from_environment() {
    let value = match std::env::var("IROHA_GAS_BENCH_IVM_POLICY") {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => panic!("non-Unicode IVM benchmark policy"),
    };
    let policy = parse(value.as_deref()).expect("valid IVM benchmark policy");
    let selected = apply(policy);
    let configured = ivm::acceleration_config();
    println!(
        "GAS_CALIBRATION_IVM_POLICY scope=ivm-only requested={policy:?} selected={selected:?} enable_simd={} enable_metal={} enable_cuda={}",
        configured.enable_simd, configured.enable_metal, configured.enable_cuda,
    );
}

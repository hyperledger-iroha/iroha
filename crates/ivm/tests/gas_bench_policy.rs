//! Checks the gas benches' scoped policy selection without changing node APIs.

#[path = "../benches/support/gas_policy.rs"]
mod gas_policy;

#[test]
fn policy_parser_requires_an_explicit_known_selection() {
    use gas_policy::{Policy, parse};
    assert_eq!(parse(None), Ok(Policy::Runtime));
    assert_eq!(parse(Some("runtime")), Ok(Policy::Runtime));
    assert_eq!(parse(Some("scalar")), Ok(Policy::Scalar));
    for invalid in ["", "Scalar", "auto", "avx2", "scalar "] {
        assert!(parse(Some(invalid)).is_err(), "{invalid:?}");
    }
}

#[test]
fn scalar_policy_is_observed_and_runtime_does_not_override_it() {
    let _guard = ivm::forced_simd_test_lock();
    struct Restore(ivm::AccelerationConfig);
    impl Drop for Restore {
        fn drop(&mut self) {
            ivm::set_acceleration_config(self.0);
        }
    }
    let _restore = Restore(ivm::acceleration_config());
    assert_eq!(
        gas_policy::apply(gas_policy::Policy::Scalar),
        ivm::SimdChoice::Scalar
    );
    let configured = ivm::acceleration_config();
    assert!(!configured.enable_simd);
    assert!(!configured.enable_metal);
    assert!(!configured.enable_cuda);
    assert_eq!(
        gas_policy::apply(gas_policy::Policy::Runtime),
        ivm::SimdChoice::Scalar
    );
    assert!(!ivm::acceleration_config().enable_simd);
}

#[test]
fn environment_entry_refuses_invalid_policy_in_an_isolated_process() {
    const CHILD: &str = "IROHA_GAS_POLICY_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        gas_policy::configure_from_environment();
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let exact =
        format!("{module}::environment_entry_refuses_invalid_policy_in_an_isolated_process");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture"])
        .env(CHILD, "1")
        .env("IROHA_GAS_BENCH_IVM_POLICY", "scalra")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be runtime or scalar"));
}

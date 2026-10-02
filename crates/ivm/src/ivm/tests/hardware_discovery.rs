//! Lazy accelerator diagnostics preserve configuration and VM construction semantics.

use super::{
    AccelerationPolicy, AllocationBudget, HARDWARE_CAPABILITY_LOOKUPS_FOR_TEST,
    HardwareCapabilities, IVM, IvmBuilder, IvmConfig, IvmConfigBuilder,
};

fn lookups() -> u64 {
    HARDWARE_CAPABILITY_LOOKUPS_FOR_TEST.with(std::cell::Cell::get)
}

#[test]
fn automatic_config_roundtrips_do_not_discover_hardware() {
    let before = lookups();
    for config in [
        IvmConfig::new(100),
        IvmConfig::adaptive(100),
        IvmConfig::deterministic(100),
        IvmConfigBuilder::new(100).build(),
        IvmConfigBuilder::adaptive(100).build(),
        IvmConfigBuilder::deterministic(100).build(),
    ] {
        assert_eq!(config.capabilities, None);
        let rebuilt = config
            .to_builder()
            .with_gas_limit(101)
            .build()
            .map(|builder| builder.with_gas_limit(100));
        assert_eq!(rebuilt, config);
        let builder = IvmBuilder::from_config(config);
        assert_eq!(builder.config(), config);
        assert_eq!(builder.build_config(), config);
    }
    assert_eq!(lookups(), before);
}

#[test]
fn vm_construction_and_suppressed_builders_do_not_discover_hardware() {
    let before = lookups();
    assert_eq!(IVM::new(100).hardware_capabilities, None);
    assert_eq!(IVM::try_new(100).unwrap().hardware_capabilities, None);
    assert_eq!(
        IVM::new_with_config(IvmConfig::deterministic(100)).hardware_capabilities,
        None
    );
    let budget = AllocationBudget::new(128 * 1024 * 1024);
    let funded = IVM::try_new_with_memory_budget(100, &budget).unwrap();
    assert_eq!(funded.hardware_capabilities, None);
    drop(funded);
    for config in [IvmConfig::adaptive(100), IvmConfig::deterministic(100)] {
        let (returned, vm) = IVM::with_config(config)
            .suppress_startup_banner()
            .build_with_config();
        assert_eq!(returned, config);
        assert_eq!(returned.capabilities, None);
        assert_eq!(vm.hardware_capabilities, None);
        assert_eq!(vm.acceleration_policy(), config.acceleration());
    }
    assert_eq!(lookups(), before);
}

#[test]
fn explicit_capabilities_and_policy_changes_do_not_discover_hardware() {
    let before = lookups();
    let cuda = HardwareCapabilities::new(true, false);
    let metal = HardwareCapabilities::new(false, true);
    let config = IvmConfigBuilder::new(100)
        .with_capabilities(cuda)
        .with_acceleration(AccelerationPolicy::new(true, true))
        .build();
    assert_eq!(config.capabilities(), cuda);
    let mapped = config.map_capabilities(|capabilities| {
        assert_eq!(capabilities, cuda);
        metal
    });
    assert_eq!(mapped.capabilities(), metal);
    assert_eq!(config.builder().build(), config);
    let mut vm = IVM::with_config(mapped)
        .with_capabilities(cuda)
        .suppress_startup_banner()
        .build();
    assert_eq!(vm.hardware_capabilities(), cuda);
    assert!(vm.uses_cuda());
    assert!(!vm.uses_metal());
    vm.set_hardware_capabilities(metal);
    assert!(!vm.uses_cuda());
    assert!(vm.uses_metal());
    vm.set_acceleration_policy(AccelerationPolicy::deterministic());
    assert_eq!(vm.hardware_capabilities(), metal);
    assert!(!vm.uses_cuda());
    assert!(!vm.uses_metal());
    assert_eq!(lookups(), before);
}

#[test]
fn disabled_usage_queries_do_not_discover_automatic_capabilities() {
    let before = lookups();
    let mut vm = IVM::new_with_config(IvmConfig::deterministic(100));
    assert!(!vm.uses_cuda());
    assert!(!vm.uses_metal());
    vm.set_acceleration_policy(AccelerationPolicy::new(true, false));
    assert!(!vm.uses_metal());
    vm.set_acceleration_policy(AccelerationPolicy::new(false, true));
    assert!(!vm.uses_cuda());
    assert_eq!(vm.hardware_capabilities, None);
    assert_eq!(lookups(), before);
}

#[test]
fn explicit_automatic_queries_resolve_the_detected_snapshot() {
    let before = lookups();
    let config = IvmConfig::adaptive(100);
    let detected = config.capabilities();
    assert_eq!(lookups(), before + 1);
    // Querying diagnostics does not silently replace automatic selection in a reusable config.
    assert_eq!(config.capabilities, None);
    let vm = IVM::new_with_config(config);
    assert_eq!(vm.hardware_capabilities(), detected);
    assert_eq!(lookups(), before + 2);
    let mapped = config.map_capabilities(|capabilities| {
        assert_eq!(capabilities, detected);
        HardwareCapabilities::none()
    });
    assert_eq!(lookups(), before + 3);
    assert_eq!(mapped.capabilities, Some(HardwareCapabilities::none()));
    assert_eq!(mapped.capabilities(), HardwareCapabilities::none());
    assert_eq!(lookups(), before + 3);
}

#[test]
fn snapshots_preserve_automatic_and_explicit_capabilities_without_discovery() {
    let before = lookups();
    let mut vm = IVM::new(100);
    let automatic = vm.try_clone_snapshot().unwrap();
    assert_eq!(automatic.hardware_capabilities, None);
    assert_eq!(automatic.acceleration_policy(), vm.acceleration_policy());
    let explicit = HardwareCapabilities::new(true, false);
    vm.set_hardware_capabilities(explicit);
    vm.set_acceleration_policy(AccelerationPolicy::new(true, true));
    let snapshot = vm.try_clone_snapshot().unwrap();
    assert_eq!(snapshot.hardware_capabilities(), explicit);
    assert_eq!(snapshot.acceleration_policy(), vm.acceleration_policy());
    assert!(snapshot.uses_cuda());
    assert!(!snapshot.uses_metal());
    assert_eq!(automatic.hardware_capabilities, None);
    assert_eq!(lookups(), before);
}

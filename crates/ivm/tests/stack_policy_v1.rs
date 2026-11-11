//! Regression coverage for the immutable IVM ABI V1 guest-stack policy.
use crate::common;
use common::assemble;
use ivm::{IVM, IvmStackPolicy, Memory, VMError, encoding, instruction, runtime::IvmConfig};
const GAS_LIMIT: u64 = 100_000;
fn assemble_words(words: &[u32]) -> Vec<u8> {
    let mut code = Vec::with_capacity(words.len() * 4);
    for word in words {
        code.extend_from_slice(&word.to_le_bytes());
    }
    assemble(&code)
}
fn observe_stack_top(mut vm: IVM, program: &[u8]) -> u64 {
    vm.load_program(program).expect("stack probe must load");
    vm.run().expect("stack probe must run");
    vm.register(1)
}
#[test]
fn policy_vectors_enforce_v1_floor_alignment_and_ceiling() {
    let policy = IvmStackPolicy::V1;
    assert_eq!(policy.minimum_stack_bytes(), 64 * 1024);
    assert_eq!(policy.maximum_stack_bytes(), 4 * 1024 * 1024);
    assert_eq!(policy.bytes_per_gas(), 4);
    assert_eq!(policy.stack_alignment_bytes(), 16);
    assert_eq!(policy.stack_limit_for_gas(0), policy.minimum_stack_bytes());
    assert_eq!(policy.stack_limit_for_gas(GAS_LIMIT), 400_000);
    assert_eq!(policy.stack_limit_for_gas(GAS_LIMIT + 1), 400_000);
    assert_eq!(
        policy.stack_limit_for_gas(u64::MAX),
        policy.maximum_stack_bytes()
    );
}
#[test]
fn every_public_vm_constructor_exposes_the_same_v1_stack_top() {
    let program = assemble_words(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 31, 0),
        encoding::wide::encode_halt(),
    ]);
    let expected = Memory::STACK_START + IvmStackPolicy::V1.stack_limit_for_gas(GAS_LIMIT);
    let observed = [
        observe_stack_top(IVM::new(GAS_LIMIT), &program),
        observe_stack_top(
            IVM::new_with_config(IvmConfig::adaptive(GAS_LIMIT)),
            &program,
        ),
        observe_stack_top(
            IVM::new_with_config(IvmConfig::deterministic(GAS_LIMIT)),
            &program,
        ),
        observe_stack_top(
            IVM::builder(GAS_LIMIT).suppress_startup_banner().build(),
            &program,
        ),
    ];
    assert_eq!(observed, [expected; 4]);
}
#[test]
fn guest_stack_top_is_an_exclusive_memory_boundary() {
    let inside = assemble_words(&[
        encoding::wide::encode_store(instruction::wide::memory::STORE64, 31, 1, -8),
        encoding::wide::encode_halt(),
    ]);
    let mut inside_vm = IVM::new(GAS_LIMIT);
    inside_vm.load_program(&inside).expect("inside probe loads");
    inside_vm.set_register(1, 0xA5);
    inside_vm
        .run()
        .expect("the final aligned word below r31 must be writable");
    let outside = assemble_words(&[
        encoding::wide::encode_store(instruction::wide::memory::STORE64, 31, 1, 0),
        encoding::wide::encode_halt(),
    ]);
    let mut outside_vm = IVM::new(GAS_LIMIT);
    outside_vm
        .load_program(&outside)
        .expect("outside probe loads");
    outside_vm.set_register(1, 0xA5);
    assert!(matches!(
        outside_vm.run(),
        Err(VMError::MemoryAccessViolation { .. })
    ));
}

/// Keep the daemon source guard strict while allowing its fixed-policy telemetry.
fn daemon_contains_retired_stack_setting(source: &str, setting: &str) -> bool {
    let mut checked = source.to_owned();
    if setting == "gas_to_stack_multiplier"
        && let Some((before, function_and_tail)) =
            source.split_once("fn ivm_stack_settings_snapshot(")
        && let Some((body, after)) = function_and_tail.split_once("\n}")
    {
        let body = body.split_whitespace().collect::<String>();
        // Accept only the field derived from this immutable local binding. A
        // mutable/rebound policy or another occurrence of the field still fails.
        if body.contains("letguest_policy=ivm::IvmStackPolicy::V1;")
            && body.matches("guest_policy=").count() == 1
        {
            let body = body.replacen(
                "gas_to_stack_multiplier:guest_policy.bytes_per_gas(),",
                "",
                1,
            );
            checked = format!("{before}fn ivm_stack_settings_snapshot({body}\n}}{after}");
        }
    }
    let mut checked = checked.split_whitespace().collect::<String>();
    // These exact read-only assertions exercise the telemetry snapshot. Do not
    // exempt arbitrary field reads, assignments, configuration or setter calls.
    for assertion in [
        "assert_eq!(snapshot.gas_to_stack_multiplier,guest_policy.bytes_per_gas());",
        "assert_eq!(published.gas_to_stack_multiplier,expected.gas_to_stack_multiplier);",
    ] {
        checked = checked.replace(assertion, "");
    }
    checked.contains(setting)
}

#[test]
fn daemon_stack_setting_guard_accepts_only_fixed_policy_telemetry() {
    let telemetry = "\
fn ivm_stack_settings_snapshot(outcome: Outcome) -> Snapshot {
    let guest_policy = ivm::IvmStackPolicy::V1;
    Snapshot {
        gas_to_stack_multiplier: guest_policy.bytes_per_gas(),
    }
}
assert_eq!(snapshot.gas_to_stack_multiplier, guest_policy.bytes_per_gas());
assert_eq!(published.gas_to_stack_multiplier, expected.gas_to_stack_multiplier);
";
    assert!(!daemon_contains_retired_stack_setting(
        telemetry,
        "gas_to_stack_multiplier"
    ));
    for changed in [
        telemetry.replace("ivm::IvmStackPolicy::V1", "configured_policy()"),
        telemetry.replace("let guest_policy", "let mut guest_policy"),
        telemetry.replace(
            "\n    Snapshot {",
            "\n    let guest_policy = configured_policy();\n    Snapshot {",
        ),
        telemetry.replace("fn ivm_stack_settings_snapshot", "fn mutable_configuration"),
        telemetry.replace("guest_policy.bytes_per_gas(),", "configured_multiplier(),"),
        format!("{telemetry}\nlet multiplier = config.gas_to_stack_multiplier;"),
        format!("{telemetry}\nsnapshot.gas_to_stack_multiplier = configured_multiplier();"),
        format!("{telemetry}\nlet multiplier = std::env::var(\"gas_to_stack_multiplier\");"),
    ] {
        assert!(
            daemon_contains_retired_stack_setting(&changed, "gas_to_stack_multiplier"),
            "a mutable stack-policy reference escaped the daemon source guard: {changed}"
        );
    }
    for setting in ["guest_stack_bytes", "memory_budget_profile"] {
        assert!(daemon_contains_retired_stack_setting(
            &format!("{telemetry}\nlet value = config.{setting};"),
            setting,
        ));
    }
}

#[test]
fn production_ivm_sources_have_no_mutable_guest_stack_policy() {
    const IVM_SOURCE: &str = include_str!("../src/ivm.rs");
    let sources = [
        include_str!("../src/lib.rs"),
        include_str!("../src/memory.rs"),
        IVM_SOURCE,
    ];
    let forbidden = [
        "static GAS_TO_STACK_MULTIPLIER",
        "static DEFAULT_STACK_LIMIT",
        "static STACK_BUDGET_LIMIT",
        "pub fn gas_to_stack_multiplier",
        "pub fn set_gas_to_stack_multiplier",
        "pub fn guest_stack_limit",
        "pub fn set_guest_stack_limit",
        "Memory::default_stack_limit",
        "Memory::stack_budget_limit",
        "with_stack_budget_bytes",
        "with_stack_limit_bytes",
    ];
    for needle in forbidden {
        assert!(
            sources.iter().all(|source| !source.contains(needle)),
            "production IVM source contains retired guest-stack control `{needle}`"
        );
    }
    assert!(
        IVM_SOURCE.contains("Memory::new_with_stack_limit(config.stack_limit_for_gas())?"),
        "the production VM constructor must propagate the fallible one-argument memory constructor"
    );
    assert!(
        IVM_SOURCE
            .contains("Memory::new_with_stack_limit_funded(config.stack_limit_for_gas(), budget)?"),
        "the budget-funded VM constructor must derive the same stack limit and propagate refusal"
    );
    let node_configuration_sources = [
        include_str!("../../iroha_config/src/parameters/actual.rs"),
        include_str!("../../iroha_config/src/parameters/defaults.rs"),
        include_str!("../../iroha_config/src/parameters/user.rs"),
    ];
    let daemon_source = include_str!("../../irohad/src/main.rs");
    for retired_setting in [
        "guest_stack_bytes",
        "gas_to_stack_multiplier",
        "memory_budget_profile",
    ] {
        assert!(
            node_configuration_sources
                .iter()
                .all(|source| !source.contains(retired_setting)),
            "node configuration contains retired guest-stack setting `{retired_setting}`",
        );
        assert!(
            !daemon_contains_retired_stack_setting(daemon_source, retired_setting),
            "node startup contains retired guest-stack setting `{retired_setting}` outside fixed-policy telemetry",
        );
    }
}

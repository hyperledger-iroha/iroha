//! Typed X509 queue emission and invocation-local nominal token boundaries.
use super::*;
use ivm_abi::call::CallTypeNodeV1;

const SOURCE: &str = r#"seiyaku Credential {
    fn relay(PendingAction _ queued) -> PendingAction { queued }
    fn queue(bytes _ request) -> PendingAction {
        ledger::credentials::present_x509(request: request)
    }
    kotoage fn present(bytes request) authorize(anyone) {
        let PendingAction queued = queue(request);
        let PendingAction relayed = relay(queued);
    }
}"#;

#[test]
fn x509_queue_has_result_bearing_ir_and_nominal_internal_call_schemas() {
    let parsed = parse(SOURCE).unwrap();
    let typed = analyze(&parsed).unwrap();
    let lowered = ir::lower(&typed).unwrap();
    let operations = lowered
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instrs)
        .filter(|instruction| matches!(instruction, ir::Instr::PresentX509 { .. }))
        .count();
    assert_eq!(operations, 1);
    let output = test_mode_compiler()
        .compile_source_output(SOURCE, None)
        .expect("compile X509 queue with its native interface sidecar");
    let bytes = output.artifact;
    let parsed = ProgramMetadata::parse(&bytes).unwrap();
    assert!(
        parsed.contract_interface.is_none(),
        "test-mode artifacts keep their interface in the compiler-owned sidecar"
    );
    let interface = output.contract_interface;
    assert!(
        interface
            .callables
            .iter()
            .any(
                |callable| callable.arguments.nodes == [CallTypeNodeV1::PendingAction]
                    && callable.results.nodes == [CallTypeNodeV1::PendingAction]
            )
    );
    let code = &bytes[parsed.code_offset..];
    assert_eq!(
        emitted_syscalls(&bytes)
            .into_iter()
            .filter(|number| *number == syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION)
            .count(),
        1
    );
    assert!(
        code.windows(4)
            .any(|word| word == encode_addi(11, 0, 2).unwrap().to_le_bytes())
    );
}

#[test]
fn pending_action_cannot_be_forged_observed_encoded_or_persisted() {
    for operation in [
        "let bool accepted = queued;",
        "let int ordinal = queued;",
        "if queued { }",
        "let bool accepted = queued == queued;",
        "let bool accepted = queued < queued;",
        "let PendingAction forged = 0;",
        "let bytes encoded = codec::to_norito(value: queued);",
        "debug::info(queued);",
    ] {
        let source = format!(
            "seiyaku Reject {{ kotoage fn run(bytes request) authorize(anyone) {{ let PendingAction queued = ledger::credentials::present_x509(request: request); {operation} }} }}"
        );
        let parsed = parse(&source)
            .unwrap_or_else(|error| panic!("invalid test syntax: {source}: {error:?}"));
        assert!(analyze(&parsed).is_err(), "{source}");
        assert!(
            test_mode_compiler().compile_source(&source).is_err(),
            "{source}"
        );
    }
    for source in [
        "seiyaku Reject { state PendingAction saved; }",
        "seiyaku Reject { state Option<PendingAction> saved; }",
        "seiyaku Reject { kotoage fn run(PendingAction queued) authorize(anyone) {} }",
        "seiyaku Reject { kotoage fn run(bytes request) authorize(anyone) -> PendingAction { ledger::credentials::present_x509(request: request) } }",
        "seiyaku Reject { struct Wrapped { PendingAction queued; } kotoage fn run(Wrapped value) authorize(anyone) {} }",
    ] {
        let parsed = parse(source)
            .unwrap_or_else(|error| panic!("invalid test syntax: {source}: {error:?}"));
        assert!(analyze(&parsed).is_err(), "{source}");
        assert!(
            test_mode_compiler().compile_source(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn x509_queue_rejects_wrong_types_views_and_internal_spelling() {
    for source in [
        SOURCE.replace("request: request", "request: true"),
        SOURCE.replace("kotoage fn present", "view fn present"),
        SOURCE.replace("ledger::credentials::present_x509", "present_x509"),
        SOURCE.replace("ledger::credentials::present_x509", "execute_instruction"),
        SOURCE.replace("request: request", "payload: request"),
    ] {
        assert!(
            test_mode_compiler().compile_source(&source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn pending_action_internal_products_preserve_nominal_identity_without_public_encoding() {
    let source = r#"seiyaku Products {
        fn relay((PendingAction, bool) _ value) -> (PendingAction, bool) { value }
        kotoage fn run(bytes request) authorize(anyone) {
            let PendingAction queued = ledger::credentials::present_x509(request: request);
            let value = relay((queued, true));
        }
    }"#;
    let output = test_mode_compiler()
        .compile_source_output(source, None)
        .expect("compile nominal internal products with their interface sidecar");
    let parsed = ProgramMetadata::parse(&output.artifact).unwrap();
    assert!(
        parsed.contract_interface.is_none(),
        "test-mode artifacts keep their interface in the compiler-owned sidecar"
    );
    assert!(output.contract_interface.callables.iter().any(|callable| {
        callable
            .results
            .nodes
            .contains(&CallTypeNodeV1::PendingAction)
    }));
}

#[test]
fn pending_action_local_sum_and_list_helpers_preserve_nominal_types() {
    let source = r#"seiyaku LocalTokens {
        fn relay_option(Option<PendingAction> _ value) -> Option<PendingAction> { value }
        fn relay_list(List<PendingAction, 2> _ value) -> List<PendingAction, 2> { value }
        kotoage fn run(bytes request) authorize(anyone) {
            let PendingAction queued = ledger::credentials::present_x509(request: request);
            let Option<PendingAction> maybe = Option::some(queued);
            let Option<PendingAction> copied = relay_option(maybe);
            let List<PendingAction, 2> pending = [queued];
            let List<PendingAction, 2> copied_list = relay_list(pending);
        }
    }"#;
    let output = test_mode_compiler()
        .compile_source_output(source, None)
        .expect("compile nominal local containers with their interface sidecar");
    let parsed = ProgramMetadata::parse(&output.artifact).unwrap();
    assert!(
        parsed.contract_interface.is_none(),
        "test-mode artifacts keep their interface in the compiler-owned sidecar"
    );
    let interface = output.contract_interface;
    for container in [CallTypeNodeV1::Option, CallTypeNodeV1::List { capacity: 2 }] {
        assert!(
            interface
                .callables
                .iter()
                .any(|callable| callable.arguments.nodes
                    == [container.clone(), CallTypeNodeV1::PendingAction]
                    && callable.results.nodes
                        == [container.clone(), CallTypeNodeV1::PendingAction])
        );
    }
}

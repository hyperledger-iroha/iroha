//! Runtime regressions for pointer-backed literals projected from aggregate values.
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;
fn run_program(source: &str) -> IVM {
    let code = KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile Kotodama aggregate-state fixture");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load aggregate-state fixture");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run aggregate-state fixture");
    vm
}
#[test]
fn mint_request_shape_roundtrips_all_eleven_fields() {
    let source =
        include_str!("../fixtures/koto_v1/kotodama_state_aggregate_literal_runtime/001.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline");
    let vm = run_program(source);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn mixed_pointer_and_scalar_literal_fields_keep_their_exact_types() {
    let source =
        include_str!("../fixtures/koto_v1/kotodama_state_aggregate_literal_runtime/002.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline");
    let vm = run_program(source);
    assert_eq!(vm.register(10), 1);
}

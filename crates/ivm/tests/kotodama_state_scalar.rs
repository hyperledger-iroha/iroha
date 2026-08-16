//! Ensure scalar state loads from durable storage at function entry.
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;
#[test]
fn kotodama_state_scalar_reads_durable() {
    let src = include_str!("../fixtures/koto_v1/kotodama_state_scalar/001.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile scalar state reader");
    let mut host = CoreHost::new();
    host.insert_state_value("counter", common::encode_int_state_value(42));
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute reader");
    assert_eq!(common::decode_i64_register(&vm, 10), 42);
}
#[test]
fn kotodama_state_struct_helper_param_reads_flattened_fields() {
    let src = include_str!("../fixtures/koto_v1/kotodama_state_scalar/002.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile struct state helper");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute struct state helper");
    assert_eq!(common::decode_i64_register(&vm, 10), 8);
}
fn run_named_struct_order(source: &str) -> (i64, i64) {
    let code = KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile named-struct source-order fixture");
    let mut host = CoreHost::new();
    host.insert_state_value("trace", common::encode_int_state_value(0));
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code)
        .expect("load named-struct source-order fixture");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute named-struct source-order fixture");
    let result = common::decode_i64_register(&vm, 10);
    let trace = {
        let host = vm.host_mut_any().expect("CoreHost available");
        let host = host.downcast_mut::<CoreHost>().expect("CoreHost type");
        let stored = host
            .state_bytes("trace")
            .expect("source-order trace persisted");
        common::decode_int_state_value(&stored)
    };
    (result, trace)
}
#[test]
fn out_of_order_named_struct_fields_match_explicit_source_order_at_runtime() {
    let named = include_str!("../fixtures/koto_v1/kotodama_state_scalar/003.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let explicit = include_str!("../fixtures/koto_v1/kotodama_state_scalar/004.ko").strip_suffix('\n').expect("fixture sentinel newline");
    let named = run_named_struct_order(named);
    let explicit = run_named_struct_order(explicit);
    assert_eq!(named, explicit, "named and explicit forms must agree");
    assert_eq!(named, (2112, 21), "fields evaluate second, then first");
}

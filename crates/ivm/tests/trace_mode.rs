use ivm::{CoreHost, IVM, KotodamaCompiler, TraceMode};

#[test]
fn runtime_trace_mode_collects_pcs_and_register_deltas() {
    let code = KotodamaCompiler::new()
        .compile_source(
            r#"
            fn main() {
                let a = 1;
                let b = a + 2;
                assert_eq(b, 3);
            }
            "#,
        )
        .expect("compile kotodama program");

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    vm.set_trace_mode(TraceMode::PcOnly);
    vm.run().expect("run with pc trace");
    assert!(!vm.trace_pcs().is_empty(), "pc trace should not be empty");
    assert!(vm.delta_register_trace().is_empty(), "pc-only mode should not record deltas");

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    vm.set_trace_mode(TraceMode::DeltaRegisters);
    vm.run().expect("run with delta trace");
    assert!(
        !vm.delta_register_trace().is_empty(),
        "delta trace should contain at least one cycle"
    );
}

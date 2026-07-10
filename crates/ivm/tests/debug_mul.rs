//! Regression coverage for scalar multiplication in compiled Kotodama.

#[test]
fn debug_mul() {
    let src = r#"
        seiyaku Multiply {
            fn f() -> i64 {
                let t = 1;
                return t * 2;
            }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("run");
    assert_eq!(vm.register(10), 2);
}

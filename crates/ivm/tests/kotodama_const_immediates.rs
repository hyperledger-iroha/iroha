//! Kotodama integer constant lowering regression tests.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn compile_large_positive_constant_executes() {
    let src = r#"
        fn main() {
            let x = 123_456_789_012i64;
            assert_eq(x, 123_456_789_012);
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile large positive constant");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load large positive constant program");
    vm.run().expect("run large positive constant program");
}

#[test]
fn compile_large_negative_constant_executes() {
    let src = r#"
        fn main() {
            let x = -987_654_321_098i64;
            assert_eq(x, -987_654_321_098);
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile large negative constant");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load large negative constant program");
    vm.run().expect("run large negative constant program");
}

//! Kotodama ternary conditional lowering regression tests.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn kotodama_ternary_executes() {
    let src = r#"
        fn main() {
            let a = 5;
            let b = 9;
            let min = (a < b) ? a : b;
            let max = (a > b) ? a : b;
            assert_eq(min, 5);
            assert_eq(max, 9);
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile ternary program");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load ternary program");
    vm.run().expect("run ternary program");
}

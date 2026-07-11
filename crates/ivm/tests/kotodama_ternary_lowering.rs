//! Kotodama ternary conditional lowering regression tests.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn kotodama_ternary_executes() {
    let src = r#"
        seiyaku TernaryLowering {
        view fn main() -> int {
            let a = 5;
            let b = 9;
            let min = (a < b) ? a : b;
            let max = (a > b) ? a : b;
            return min * 100 + max;
        }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile ternary program");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load ternary program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run ternary program");
    assert_eq!(vm.register(10), 509);
}

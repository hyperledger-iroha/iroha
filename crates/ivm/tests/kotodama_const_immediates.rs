//! Kotodama integer constant lowering regression tests.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn compile_large_positive_constant_executes() {
    let src = r#"
        seiyaku LargePositiveConstant {
            view fn main() -> i64 {
                let x = 123_456_789_012i64;
                return x;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile large positive constant");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load large positive constant program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run large positive constant program");
    assert_eq!(vm.register(10) as i64, 123_456_789_012);
}

#[test]
fn compile_large_negative_constant_executes() {
    let src = r#"
        seiyaku LargeNegativeConstant {
            view fn main() -> i64 {
                let x = -987_654_321_098i64;
                return x;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile large negative constant");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load large negative constant program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run large negative constant program");
    assert_eq!(vm.register(10) as i64, -987_654_321_098);
}

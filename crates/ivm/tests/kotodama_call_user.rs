//! Tests for user-defined function calls in Kotodama.

use ivm::{IVM, KotodamaCompiler};
mod common;

#[test]
fn user_defined_call_returns_42() {
    // Repeated parameter types make the call named-only in V1.
    let src = r#"
        seiyaku UserCalls {
            fn add(a: i64, b: i64) -> i64 { return a + b; }
            view fn main() -> i64 { let z = add(a: 20, b: 22); return z; }
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let program = compiler.compile_source(src).expect("compile kotodama");

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &program, "main");
    vm.run().expect("run VM");
    // Entry function returns via r10
    assert_eq!(vm.register(10), 42);
}

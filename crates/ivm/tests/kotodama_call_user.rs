//! Tests for user-defined function calls in Kotodama.

use ivm::{IVM, KotodamaCompiler};

#[test]
fn user_defined_call_returns_42() {
    // Define two functions: `add` and `main` calling `add(20, 22)`.
    let src = r#"
        fn add(a, b) -> int { return a + b; }
        fn main() -> int { let z = add(20, 22); return z; }
    "#;
    let compiler = KotodamaCompiler::new();
    let program = compiler.compile_source(src).expect("compile kotodama");

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).expect("load program");
    vm.run().expect("run VM");
    // Entry function returns via r10
    assert_eq!(vm.register(10), 42);
}

//! Kotodama function calls and calling convention tests (nested calls, multi-returns).

use ivm::{IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn nested_function_calls_work() {
    // Without saving/restoring RA, nested calls would clobber return addresses.
    let src = r#"
        seiyaku NestedCalls {
            fn inc(x: i64) -> i64 { return x + 1; }
            fn add_two(x: i64) -> i64 { let y = inc(x); return inc(y); }
            view fn main() -> i64 { return add_two(5); }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile nested calls");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute nested calls");
    assert_eq!(vm.register(10), 7);
}

#[test]
fn multi_return_call_and_tuple_use() {
    // Pair returns two values; caller uses them via tuple members
    let src = r#"
        seiyaku MultiReturn {
            fn pair(x: i64) -> (i64, i64) { return (x, x + 1); }
            fn sum_pair(x: i64) -> i64 { let t = pair(x); return t.0 + t.1; }
            view fn main() -> i64 { return sum_pair(5); }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile multi-return");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute multi-return");
    assert_eq!(vm.register(10), 11);
}

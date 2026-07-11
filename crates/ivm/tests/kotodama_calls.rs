//! Kotodama function calls and calling convention tests (nested calls, multi-returns).

use ivm::{IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn nested_function_calls_work() {
    // Without saving/restoring RA, nested calls would clobber return addresses.
    let src = r#"
        seiyaku NestedCalls {
            fn inc(int x) -> int { return x + 1; }
            fn add_two(int x) -> int { let y = inc(x); return inc(y); }
            view fn main() -> int { return add_two(5); }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile nested calls");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute nested calls");
    assert_eq!(common::decode_i64_register(&vm, 10), 7);
}

#[test]
fn multi_return_call_and_tuple_use() {
    // Pair returns two values; caller uses them via tuple members
    let src = r#"
        seiyaku MultiReturn {
            fn pair(int x) -> (int, int) { return (x, x + 1); }
            fn sum_pair(int x) -> int { let t = pair(x); return t.0 + t.1; }
            view fn main() -> int { return sum_pair(5); }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile multi-return");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute multi-return");
    assert_eq!(common::decode_i64_register(&vm, 10), 11);
}

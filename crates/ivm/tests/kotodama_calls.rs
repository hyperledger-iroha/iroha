//! Kotodama function calls and calling convention tests (nested calls, multi-returns).

use ivm::{IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn nested_function_calls_work() {
    // Without saving/restoring RA, nested calls would clobber return addresses.
    let src = r#"
        fn inc(x: int) -> int { return x + 1; }
        fn add_two(x: int) -> int { let y = inc(x); return inc(y); }
        fn main() -> int { return add_two(5); }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile nested calls");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute nested calls");
    assert_eq!(vm.register(10), 7);
}

#[test]
fn multi_return_call_and_tuple_use() {
    // Pair returns two values; caller uses them via tuple members
    let src = r#"
        fn pair(x: int) -> (int, int) { return (x, x + 1); }
        fn sum_pair(x: int) -> int { let t = pair(x); return t.0 + t.1; }
        fn main() -> int { return sum_pair(5); }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile multi-return");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute multi-return");
    assert_eq!(vm.register(10), 11);
}

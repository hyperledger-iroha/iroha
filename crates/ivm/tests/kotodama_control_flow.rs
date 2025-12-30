//! Kotodama control-flow codegen coverage for `break`/`continue`.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn break_exits_while_loop() {
    let src = r#"
        fn main() -> int {
            let i = 0;
            while (i < 10) {
                if (i == 3) {
                    break;
                }
                i = i + 1;
            }
            return i;
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile while/break program");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    vm.run().expect("execute break program");
    assert_eq!(vm.register(10), 3);
}

#[test]
fn continue_skips_range_iteration() {
    let src = r#"
        fn main() -> int {
            let i = 0;
            let sum = 0;
            while (i < 5) {
                if (i == 2) {
                    i = i + 1;
                    continue;
                }
                sum = sum + i;
                i = i + 1;
            }
            return sum;
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile continue program");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    vm.run().expect("execute continue program");
    assert_eq!(vm.register(10), 8);
}

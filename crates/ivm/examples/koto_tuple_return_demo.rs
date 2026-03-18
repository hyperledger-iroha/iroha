//! Kotodama tuple-return demo: compile a function returning a tuple and read results.
use ivm::{IVM, kotodama::compiler::Compiler as KotodamaCompiler};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Inline Kotodama source: a main that returns a pair (a+1, b+1)
    let src = r#"
        fn main(a: int, b: int) -> (int, int) {
            let t = (a + 1, b + 1);
            let x = t.0;
            let y = t.1;
            return (x, y);
        }
    "#;

    // 2) Compile to IVM bytecode
    let compiler = KotodamaCompiler::new();
    let code = compiler.compile_source(src).expect("compile tuple return");

    // 3) Prepare VM with arguments in r10.. and run
    let mut vm = IVM::new(1_000_000);
    vm.load_program(&code).expect("load program");
    vm.set_register(10, 3); // a
    vm.set_register(11, 5); // b
    vm.run().expect("run");

    // 4) Read tuple results from r10 (first) and r11 (second)
    let out0 = vm.register(10);
    let out1 = vm.register(11);
    println!("tuple return -> ({out0} , {out1})");
    assert_eq!(out0, 4);
    assert_eq!(out1, 6);
    Ok(())
}

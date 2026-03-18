//! Ensure scalar state loads from durable storage at function entry.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
use norito::to_bytes;

#[test]
fn kotodama_state_scalar_reads_durable() {
    let src = r#"
        state int counter;
        fn main() -> int {
            return counter;
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile scalar state reader");

    let mut host = CoreHost::new();
    host.insert_state_value("counter", to_bytes(&42_i64).expect("encode counter"));

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load program");
    vm.run().expect("execute reader");
    assert_eq!(vm.register(10), 42);
}

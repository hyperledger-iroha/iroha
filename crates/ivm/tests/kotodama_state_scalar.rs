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

#[test]
fn kotodama_state_struct_helper_param_reads_flattened_fields() {
    let src = r#"
        struct Ledger { counter: int; flag: bool; }
        state Ledger ledger;

        fn read_counter(state Ledger entry) -> int {
            return entry.counter;
        }

        fn score(state Ledger entry) -> int {
            let value = read_counter(entry);
            if (entry.flag) {
                value = value + 1;
            }
            return value;
        }

        fn main() -> int {
            ledger = Ledger(7, true);
            return score(ledger);
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile struct state helper");

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    vm.run().expect("execute struct state helper");
    assert_eq!(vm.register(10), 8);
}

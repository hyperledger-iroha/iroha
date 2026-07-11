//! Ensure scalar state loads from durable storage at function entry.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn kotodama_state_scalar_reads_durable() {
    let src = r#"
        seiyaku ScalarState {
            state counter: i64;
            hajimari() { counter = 0; }
            view fn main() -> i64 {
                return counter;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile scalar state reader");

    let mut host = CoreHost::new();
    host.insert_state_value("counter", common::encode_i64_state_value(42));

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute reader");
    assert_eq!(vm.register(10), 42);
}

#[test]
fn kotodama_state_struct_helper_param_reads_flattened_fields() {
    let src = r#"
        seiyaku StructState {
            struct Ledger { counter: i64, flag: bool }
            state ledger: Ledger;

            hajimari() {
                ledger = Ledger { counter: 0, flag: false };
            }

            fn read_counter(entry: Ledger) -> i64 {
                return entry.counter;
            }

            fn score(entry: Ledger) -> i64 {
                var value = read_counter(entry);
                if (entry.flag) {
                    value = value + 1;
                }
                return value;
            }

            kotoage fn main() -> i64 authorize("WriteState") {
                ledger = Ledger { counter: 7, flag: true };
                return score(ledger);
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile struct state helper");

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute struct state helper");
    assert_eq!(vm.register(10), 8);
}

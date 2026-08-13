//! Ensure scalar state loads from durable storage at function entry.
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;
#[test]
fn kotodama_state_scalar_reads_durable() {
    let src = r#"
        seiyaku ScalarState {
            state int counter;
            hajimari() { counter = 0; }
            view fn main() -> int {
                return counter;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile scalar state reader");
    let mut host = CoreHost::new();
    host.insert_state_value("counter", common::encode_int_state_value(42));
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute reader");
    assert_eq!(common::decode_i64_register(&vm, 10), 42);
}
#[test]
fn kotodama_state_struct_helper_param_reads_flattened_fields() {
    let src = r#"
        seiyaku StructState {
            struct Ledger { int counter, bool flag }
            state Ledger ledger;

            hajimari() {
                ledger = Ledger { counter: 0, flag: false };
            }

            fn read_counter(Ledger entry) -> int {
                return entry.counter;
            }

            fn score(Ledger entry) -> int {
                var value = read_counter(entry);
                if (entry.flag) {
                    value = value + 1;
                }
                return value;
            }

            kotoage fn main() -> int authorize("WriteState") {
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
    assert_eq!(common::decode_i64_register(&vm, 10), 8);
}
fn run_named_struct_order(source: &str) -> (i64, i64) {
    let code = KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile named-struct source-order fixture");
    let mut host = CoreHost::new();
    host.insert_state_value("trace", common::encode_int_state_value(0));
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code)
        .expect("load named-struct source-order fixture");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute named-struct source-order fixture");
    let result = common::decode_i64_register(&vm, 10);
    let trace = {
        let host = vm.host_mut_any().expect("CoreHost available");
        let host = host.downcast_mut::<CoreHost>().expect("CoreHost type");
        let stored = host
            .state_bytes("trace")
            .expect("source-order trace persisted");
        common::decode_int_state_value(&stored)
    };
    (result, trace)
}
#[test]
fn out_of_order_named_struct_fields_match_explicit_source_order_at_runtime() {
    let named = r#"
        seiyaku NamedStruct {
            struct Pair { int first, int second }
            state int trace;

            hajimari() { trace = 0; }

            fn record(int value) -> int {
                trace = trace * 10 + value;
                value
            }

            kotoage fn main() -> int authorize("WriteState") {
                let pair = Pair {
                    second: record(2),
                    first: record(1),
                };
                trace * 100 + pair.first * 10 + pair.second
            }
        }
    "#;
    let explicit = r#"
        seiyaku NamedStruct {
            struct Pair { int first, int second }
            state int trace;

            hajimari() { trace = 0; }

            fn record(int value) -> int {
                trace = trace * 10 + value;
                value
            }

            kotoage fn main() -> int authorize("WriteState") {
                let int second_value = record(2);
                let int first_value = record(1);
                let pair = Pair {
                    first: first_value,
                    second: second_value,
                };
                trace * 100 + pair.first * 10 + pair.second
            }
        }
    "#;
    let named = run_named_struct_order(named);
    let explicit = run_named_struct_order(explicit);
    assert_eq!(named, explicit, "named and explicit forms must agree");
    assert_eq!(named, (2112, 21), "fields evaluate second, then first");
}

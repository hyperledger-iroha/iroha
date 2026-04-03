//! Focused runtime coverage for durable `state` helper parameters.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
use norito::to_bytes;

fn run_with_host<F>(src: &str, seed: F) -> IVM
where
    F: FnOnce(&mut CoreHost),
{
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile kotodama");
    let mut host = CoreHost::new();
    seed(&mut host);
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load program");
    vm.run().expect("run program");
    vm
}

#[test]
fn state_scalar_helper_param_reads_durable_root() {
    let src = r#"
        state int counter;

        fn read_counter(state int value) -> int {
            return value;
        }

        fn main() -> int {
            return read_counter(counter);
        }
    "#;

    let vm = run_with_host(src, |host| {
        host.insert_state_value("counter", to_bytes(&42_i64).expect("encode counter"));
    });
    assert_eq!(vm.register(10), 42);
}

#[test]
fn state_scalar_helper_param_accepts_struct_child_handle() {
    let src = r#"
        struct Ledger { counter: int; flag: bool; }
        state Ledger ledger;

        fn read_counter(state int value) -> int {
            return value;
        }

        fn main() -> int {
            return read_counter(ledger.counter);
        }
    "#;

    let vm = run_with_host(src, |host| {
        host.insert_state_value(
            "ledger_counter",
            to_bytes(&7_i64).expect("encode ledger counter"),
        );
    });
    assert_eq!(vm.register(10), 7);
}

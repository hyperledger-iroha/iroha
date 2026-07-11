//! Kotodama calls to durable state helpers through the public `state` namespace.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn kotodama_host_state_calls_run() {
    // Store a small bytes payload under a path, then read and delete it. The
    // compiler owns the pointer-ABI wrapping; source cannot construct raw
    // `NoritoBytes` values.
    // We do not attempt to decode the bytes to ints here; the purpose is to
    // ensure pointer-ABI plumbing and syscalls are wired end-to-end.
    let src = r#"
        seiyaku StateHostCalls {
          kotoage fn main() authorize("StateHostCalls") {
            state::set(Name::parse("demo"), b"hello");
            let _b = state::get(Name::parse("demo"));
            state::delete(Name::parse("demo"));
          }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run");
}

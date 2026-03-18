//! Kotodama calls to durable state helpers via host::state_{get,set,del}.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn kotodama_host_state_calls_run() {
    // Store a small NoritoBytes payload under a path, then read and delete it.
    // We do not attempt to decode the bytes to ints here; the purpose is to
    // ensure pointer-ABI plumbing and syscalls are wired end-to-end.
    let src = r#"
        fn main() {
            host::state_set(name("demo"), norito_bytes("hello"));
            let _b = host::state_get(name("demo"));
            host::state_del(name("demo"));
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load");
    vm.run().expect("run");
}

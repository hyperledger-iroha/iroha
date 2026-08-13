//! Kotodama calls to durable state helpers through the public `state` namespace.
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;
fn encoded_state_path(name: &str, key: i64) -> String {
    let key = ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(
        i128::from(key),
    ))
    .expect("encode canonical pointer-backed StateMap key");
    format!("{name}/{}", hex::encode(key))
}
#[test]
fn kotodama_host_state_calls_run() {
    // Store a small bytes payload under a canonical StateMap path, then read and
    // delete it. The path helper returns a `bytes` carrier containing canonical
    // Norito `StatePath`; a legacy `Name` carrier is intentionally not accepted.
    // We do not attempt to decode the bytes to ints here; the purpose is to
    // ensure pointer-ABI plumbing and syscalls are wired end-to-end.
    let src = r#"
        seiyaku StateHostCalls {
          state StateMap<int, bytes> demo;

          hajimari() {
            demo[1] = b"";
          }

          kotoage fn main() authorize("StateHostCalls") {
            demo[1] = b"hello";
            let path = Name::parse("demo").path(1);
            let encoded = state::get(path);
            state::set(path: path, value: encoded);
            state::delete(path);
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
    let host = vm.host_mut_any().expect("CoreHost available");
    let host = host.downcast_mut::<CoreHost>().expect("CoreHost type");
    assert!(host.state_bytes(&encoded_state_path("demo", 1)).is_none());
}

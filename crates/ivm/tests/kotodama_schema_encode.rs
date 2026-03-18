//! Kotodama schema encode/decode roundtrip via CoreHost.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn kotodama_schema_encode_decode_roundtrip() {
    let src = r#"
        fn main() {
            let schema = name!("Order");
            let payload = json!{ qty: 7, side: "buy" };
            let bytes = encode_schema(schema, payload);
            let decoded = decode_schema(schema, bytes);
            let _bytes2 = encode_schema(schema, decoded);
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile schema roundtrip");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load");
    vm.run().expect("run");
}

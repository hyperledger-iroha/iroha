//! Kotodama schema encode/decode roundtrip via CoreHost.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn kotodama_schema_encode_decode_roundtrip() {
    let src = r#"
        seiyaku SchemaCodecRoundtrip {
        view fn main() {
            let schema = Name::parse("Order");
            let payload = Json::parse("{\"qty\":7,\"side\":\"buy\"}");
            let bytes = codec::schema::encode(schema, payload);
            let decoded = codec::schema::decode(schema, bytes);
            let _bytes2 = codec::schema::encode(schema, decoded);
        }
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

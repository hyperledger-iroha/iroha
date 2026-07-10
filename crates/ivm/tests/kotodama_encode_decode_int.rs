//! Kotodama encode_int/decode_int helpers end-to-end via CoreHost.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn kotodama_encode_decode_int_roundtrip() {
    let src = r#"
        seiyaku IntegerCodecRoundtrip {
            view fn main() {
                let encoded = codec::encode_i64(7);
                let decoded = codec::decode_i64(encoded);
                test::assert_eq(decoded, 7);
            }
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

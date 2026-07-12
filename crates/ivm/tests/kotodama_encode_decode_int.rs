//! Kotodama rejects retired source-level integer codec plumbing.

use ivm::kotodama::compiler::Compiler as KotodamaCompiler;

#[test]
fn kotodama_source_rejects_retired_integer_codec_helpers() {
    let src = r#"
        seiyaku IntegerCodecRoundtrip {
            view fn main() {
                let encoded = codec::encode_i64(7);
                let decoded = codec::decode_i64(encoded);
                let _ = decoded;
            }
        }
    "#;
    let error = KotodamaCompiler::new()
        .compile_source(src)
        .expect_err("source-level integer codec helpers are compiler-internal");
    assert!(error.contains("codec::encode_i64"), "{error}");
    assert!(error.contains("codec::decode_i64"), "{error}");
}

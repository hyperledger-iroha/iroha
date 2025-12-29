//! Kotodama pointer-ABI intrinsic compilation coverage.

use ivm::kotodama::compiler::Compiler;

#[test]
fn kotodama_pointer_intrinsics_cover_axt_and_dataspace_types() {
    let src = r#"
        seiyaku PointerIntrinsics {
            fn main() {
                let ds = dataspace_id("0xdeadbeef");
                let desc_bytes = blob("0x00112233445566778899aabbccddeeff");
                let desc = axt_descriptor(desc_bytes);
                let handle_bytes = blob("0x01010101");
                let handle = asset_handle(handle_bytes);
                let proof_bytes = blob("0x02020202");
                let proof = proof_blob(proof_bytes);
                // Keep values live so the compiler emits pointer literals.
                let _a = ds;
                let _b = desc;
                let _c = handle;
                let _d = proof;
            }
        }
    "#;

    // Compilation exercises semantic/type lowering and pointer-ABI fixups.
    Compiler::new()
        .compile_source(src)
        .expect("compile pointer intrinsics");
}

#[test]
fn kotodama_zk_verify_intrinsic_accepts_blob_pointer() {
    let src = r#"
        seiyaku ZkVerifyIntrinsic {
            fn main() {
                let env = blob("0x01020304");
                zk_verify_batch(env);
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("compile zk_verify_batch intrinsic");
}

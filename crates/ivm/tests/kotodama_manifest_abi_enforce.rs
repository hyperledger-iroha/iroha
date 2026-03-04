//! Kotodama manifest ABI enforcement tests (first release policy).
//!
//! For the first release, only `abi_version = 1` is supported for manifests
//! emitted by the compiler. Attempting to produce a manifest for any other
//! version must return an error.

#[test]
fn compile_source_with_manifest_rejects_non_v1_abi() {
    use ivm::kotodama::compiler::{Compiler, CompilerOptions};

    // Program is trivial; we only care about header `abi_version` plumbing.
    let src = "fn f() { let x = 1 + 2; }";

    // Request abi_version = 2 via compiler options; the compiler should reject
    // manifest emission for the first release policy.
    let opts = CompilerOptions {
        abi_version: 2,
        ..Default::default()
    };
    let compiler = Compiler::new_with_options(opts);
    let err = compiler
        .compile_source_with_manifest(src)
        .expect_err("expected rejection for abi_version != 1");
    assert!(
        err.contains("unsupported abi_version 2") && err.contains("expected 1"),
        "unexpected error: {err}"
    );

    // Compiling without a manifest must also reject non-v1 ABI, because the
    // bytecode header is part of the on-chain safety surface.
    let err = compiler
        .compile_source(src)
        .expect_err("expected rejection for abi_version != 1");
    assert!(
        err.contains("unsupported abi_version 2") && err.contains("expected 1"),
        "unexpected error: {err}"
    );
}

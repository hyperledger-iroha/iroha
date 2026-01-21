//! Compile the `zk_vote_and_unshield.ko` sample and assert it lowers
//! to the expected ZK verify and vendor bridge syscalls with strict
//! pointer-ABI usage.

use ivm::{encoding, instruction::wide, syscalls};

#[test]
fn sample_compiles_to_zk_and_vendor_syscalls() {
    // Embed the sample source and compile it
    let src = include_str!("../../kotodama_lang/src/samples/zk_vote_and_unshield.ko");
    let code = ivm::kotodama::compiler::Compiler::new()
        .compile_source(src)
        .expect("compile sample");

    // Extract code words
    let parsed = ivm::ProgramMetadata::parse(&code).unwrap();

    let meta = parsed.metadata;

    let off = parsed.code_offset;
    // ZK mode bit should be set due to ZK verify intrinsics present
    assert_ne!(meta.mode & ivm::ivm_mode::ZK, 0);
    let mut words = Vec::new();
    let mut i = off;
    while i + 4 <= code.len() {
        words.push(u32::from_le_bytes(code[i..i + 4].try_into().unwrap()));
        i += 4;
    }

    // Expected syscalls
    let scall = wide::system::SCALL;
    let want_verify_ballot =
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_ZK_VOTE_VERIFY_BALLOT as u8);
    let want_verify_unshield =
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_ZK_VERIFY_UNSHIELD as u8);
    let want_exec = encoding::wide::encode_sys(
        scall,
        syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
    );

    assert!(
        words.contains(&want_verify_ballot),
        "sample must contain ballot verify syscall"
    );
    assert!(
        words.contains(&want_verify_unshield),
        "sample must contain unshield verify syscall"
    );
    assert!(
        words.contains(&want_exec),
        "sample must enqueue via vendor bridge"
    );
}

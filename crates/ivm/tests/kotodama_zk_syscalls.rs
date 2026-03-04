//! Kotodama ZK-related builtin tests: ensure compiler emits expected syscalls
//! and uses the NoritoBytes pointer-ABI when requested.

use ivm::{encoding, instruction::wide, syscalls};

#[test]
fn compile_zk_verify_and_execute_instruction() {
    // Program: verify transfer with a NoritoBytes env, then enqueue an instruction via vendor syscall
    // The env payloads are provided as string literals and wrapped via norito_bytes("...")
    let src = r#"
fn main() {
  let ok = zk_verify_transfer(norito_bytes("ENV1"));
  let ok2 = zk_verify_unshield(norito_bytes("ENV2"));
  execute_instruction(norito_bytes("IB0"));
  execute_query(norito_bytes("QB0"));
}
"#;
    let code = ivm::kotodama::compiler::Compiler::new()
        .compile_source(src)
        .expect("compile zk program");
    let off = ivm::ProgramMetadata::parse(&code).unwrap().code_offset;
    let mut words = Vec::new();
    let mut i = off;
    while i + 4 <= code.len() {
        words.push(u32::from_le_bytes(code[i..i + 4].try_into().unwrap()));
        i += 4;
    }
    let scall = wide::system::SCALL;
    let want_verify_transfer =
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_ZK_VERIFY_TRANSFER as u8);
    let want_verify_unshield =
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_ZK_VERIFY_UNSHIELD as u8);
    let want_exec = encoding::wide::encode_sys(
        scall,
        syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
    );
    let want_query =
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY as u8);
    assert!(words.contains(&want_verify_transfer));
    assert!(words.contains(&want_verify_unshield));
    assert!(words.contains(&want_exec));
    assert!(words.contains(&want_query));
}

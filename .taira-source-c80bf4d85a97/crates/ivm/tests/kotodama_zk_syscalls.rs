//! Kotodama ZK-related builtin tests for the namespaced, typed V1 surface.

use ivm::{encoding, instruction::wide, syscalls};

#[test]
fn compile_namespaced_zk_verification_without_opaque_submission() {
    let src = r#"
seiyaku ZkVerification {
  kotoage fn verify() authorize("VerifyZk") {
    let ok = crypto::zk::verify_transfer(b"ENV1");
    let ok2 = crypto::zk::verify_unshield(b"ENV2");
  }
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
    assert!(words.contains(&want_verify_transfer));
    assert!(words.contains(&want_verify_unshield));
    assert!(!words.iter().any(|word| {
        *word
            == encoding::wide::encode_sys(
                scall,
                syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
            )
            || *word
                == encoding::wide::encode_sys(
                    scall,
                    syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY as u8,
                )
    }));
}

#[test]
fn raw_norito_and_opaque_submission_are_not_source_apis() {
    let diagnostics = ivm::kotodama::session::CompilerSession::default()
        .build(ivm::kotodama::session::CompileRequest {
            source: r#"
seiyaku RawSubmission {
  kotoage fn submit() authorize("Submit") {
    execute_instruction(norito_bytes(b"opaque"));
  }
}
"#,
            source_name: Some("raw_submission.ko"),
        })
        .expect_err("raw Norito construction and instruction submission must fail");
    assert!(diagnostics.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E_INTERNAL_BUILTIN"
            || diagnostic.message.contains("execute_instruction")
    }));
}

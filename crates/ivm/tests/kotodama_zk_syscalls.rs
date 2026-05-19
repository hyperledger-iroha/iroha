//! Kotodama ZK-related builtin tests: ensure compiler emits expected syscalls
//! and uses the NoritoBytes pointer-ABI when requested.

use ivm::{encoding, instruction::wide, syscalls};

#[test]
fn compile_zk_verify_and_execute_instruction() {
    // Program: verify transfer with a NoritoBytes env, then enqueue an instruction via vendor syscall
    // The vendor payloads are complete canonical Norito literals so production access metadata stays complete.
    use iroha_data_model::{
        account::AccountId,
        asset::id::{AssetDefinitionId, AssetId},
        domain::DomainId,
        isi::{InstructionBox, Mint},
        query::{QueryRequest, SingularQueryBox, asset::FindAssetById},
    };

    let account = AccountId::new(
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            .parse()
            .expect("public key"),
    );
    let asset_def = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "rose".parse().expect("asset name"),
    );
    let asset_id = AssetId::of(asset_def, account);
    let instruction = InstructionBox::from(Mint::asset_numeric(1_u32, asset_id.clone()));
    let instruction_payload = format!(
        "0x{}",
        hex::encode(norito::to_bytes(&instruction).expect("encode InstructionBox"))
    );
    let query = QueryRequest::Singular(SingularQueryBox::FindAssetById(FindAssetById::new(
        asset_id,
    )));
    let query_payload = format!(
        "0x{}",
        hex::encode(norito::to_bytes(&query).expect("encode QueryRequest"))
    );

    let src = format!(
        r#"
fn main() {{
  let ok = zk_verify_transfer(norito_bytes("ENV1"));
  let ok2 = zk_verify_unshield(norito_bytes("ENV2"));
  execute_instruction(norito_bytes("{instruction_payload}"));
  execute_query(norito_bytes("{query_payload}"));
}}
"#
    );
    let code = ivm::kotodama::compiler::Compiler::new()
        .compile_source(&src)
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

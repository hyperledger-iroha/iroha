//! Quick helper to compile an inline Kotodama source string and dump SCALLs.

use ivm::{
    Memory, ProgramMetadata, decode as ivm_decode, kotodama::compiler::Compiler as KotodamaCompiler,
};

fn dump(src: &str) {
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let parsed = ProgramMetadata::parse(&code).expect("parse meta");
    let bytes = &code[parsed.code_offset..];
    let mut mem = Memory::new(bytes.len() as u64);
    mem.load_code(bytes);
    println!("code_len={}, off={}", code.len(), parsed.code_offset);
    let mut count = 0;
    let mut pc = 0u64;
    while pc + 2 <= mem.code_len() {
        let (w, len) = ivm_decode(&mem, pc).unwrap();
        let opcode = (w >> 24) as u8;
        let (_, imm) = ivm::encoding::wide::decode_sys(w);
        if opcode == ivm::instruction::wide::system::SCALL {
            println!("SCALL @+{pc} imm=0x{imm:02x}");
        }
        if count < 32 {
            println!("+{pc:03}: op=0x{opcode:02x} imm=0x{imm:02x} raw=0x{w:08x}");
            count += 1;
        }
        pc += len as u64;
    }
}

fn main() {
    // Case 1: create_nfts_for_all_users + set_execution_depth + set_account_detail
    let src1 = "seiyaku Sample1 { kotoage fn run() { create_nfts_for_all_users(); set_execution_depth(111); set_account_detail(authority(), name!(\"cursor\"), json!{ query: \"sc_dummy\", cursor: 1 }); } }";
    println!("-- case 1 --");
    dump(src1);

    // Case 2: typed NFT syscalls
    let src2 = "seiyaku Sample2 { kotoage fn run() { nft_mint_asset(nft_id!(\"n0$wonderland\"), account!(\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\")); nft_transfer_asset(account!(\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\"), nft_id!(\"n0$wonderland\"), account!(\"sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76\")); } }";
    println!("-- case 2 --");
    dump(src2);
}

//! Kotodama pointer roundtrip tests.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

fn run_prog(body: &str) {
    let src = format!(
        "seiyaku PointerRoundtrip {{ kotoage fn main() authorize(\"PointerRoundtrip\") {{\n{body}\n}} }}"
    );
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(&src).expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load");
    common::select_kotodama_entrypoint(&mut vm, &prog, "main");
    vm.run()
        .expect("program should run with CoreHost TLV validation");
}

#[test]
fn roundtrip_nft_mint_asset() {
    let src = r#"
          ledger::nft::mint(
            NftId::parse("rose:uuid:0123$wonderland.universal"),
            AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
          );
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_set_metadata() {
    let src = r#"
          ledger::nft::set_metadata(nft: NftId::parse("rose:uuid:ffff$wonderland.universal"), key: Name::parse("dpn_metadata"), value: Json::parse("{\"meta\":1}"));
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_transfer_asset() {
    let src = r#"
          ledger::asset::transfer(source: AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), destination: AccountId::parse("sorauﾛ1PｦﾔJdﾐww6ﾆfgｾ73xJkｺﾓｺﾀEｿGzQuﾄg3ﾐeﾕｳｶﾒﾚｻY1FC8K"), asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), amount: 1, dataspace: DataSpaceId::parse("0"));
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_burn_asset() {
    let src = r#"
          ledger::nft::burn(NftId::parse("rose:uuid:bead$wonderland.universal"));
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_mint_asset_accepts_runtime_owner() {
    let src = r#"
          let owner = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
          ledger::nft::mint(
            NftId::parse("rose:uuid:0123$wonderland.universal"),
            owner
          );
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_transfer_asset_accepts_runtime_from() {
    let src = r#"
          let from = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
          let to = AccountId::parse("sorauﾛ1PｦﾔJdﾐww6ﾆfgｾ73xJkｺﾓｺﾀEｿGzQuﾄg3ﾐeﾕｳｶﾒﾚｻY1FC8K");
          let nft = NftId::parse("rose:uuid:bead$wonderland.universal");
          ledger::nft::transfer(source: from, nft: nft, destination: to);
    "#;
    run_prog(src);
}

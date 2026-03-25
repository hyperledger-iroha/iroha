//! Kotodama pointer roundtrip tests.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

fn run_prog(src: &str) {
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("program should run with CoreHost TLV validation");
}

#[test]
fn roundtrip_nft_mint_asset() {
    let src = r#"
        fn main() {
          nft_mint_asset(
            nft_id("rose:uuid:0123$wonderland"),
            account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
          );
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_set_metadata() {
    let src = r#"
        fn main() {
          nft_set_metadata(nft_id("rose:uuid:ffff$wonderland"), json("{\"meta\":1}"));
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_transfer_asset() {
    let src = r#"
        fn main() {
          transfer_asset(
            account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
            account_id("6cmzPVPX8dTmJWnCc8X5MpcZLb7UjrvR5Y1VdRmfj9pbb93hFbJfpLb"),
            asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            1
          );
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_burn_asset() {
    let src = r#"
        fn main() {
          nft_burn_asset(nft_id("rose:uuid:bead$wonderland"));
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_mint_asset_accepts_runtime_owner() {
    let src = r#"
        fn main() {
          let owner = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
          nft_mint_asset(
            nft_id("rose:uuid:0123$wonderland"),
            owner
          );
        }
    "#;
    run_prog(src);
}

#[test]
fn roundtrip_nft_transfer_asset_accepts_runtime_from() {
    let src = r#"
        fn main() {
          let from = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
          let to = account_id("6cmzPVPX8dTmJWnCc8X5MpcZLb7UjrvR5Y1VdRmfj9pbb93hFbJfpLb");
          let nft = nft_id("rose:uuid:bead$wonderland");
          nft_transfer_asset(from, nft, to);
        }
    "#;
    run_prog(src);
}
